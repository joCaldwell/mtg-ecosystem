//! Python owns a lease for every exported ndarray (including sliced views).
//! Workers can borrow the Rust allocations mutably only after the last owner drops.
use super::*;
use mtg_observe::catalog::CardTable;
use numpy::{
    IntoPyArray, PyArray, PyArrayMethods, PyReadonlyArray1,
    ndarray::{ArrayViewD, IxDyn},
};
use pyo3::{
    exceptions::{PyKeyError, PyRuntimeError, PyValueError},
    prelude::*,
    types::PyDict,
};

#[pyclass(frozen)]
struct Lease {
    storage: Arc<Storage>,
}
#[pyclass]
struct Batch {
    arrays: Option<Py<PyDict>>,
}
#[pymethods]
impl Batch {
    fn __getitem__<'py>(&self, py: Python<'py>, key: &str) -> PyResult<Bound<'py, PyAny>> {
        self.arrays
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("released batch"))?
            .bind(py)
            .get_item(key)?
            .ok_or_else(|| PyKeyError::new_err(key.to_string()))
    }
    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }
    fn release(&mut self) {
        self.arrays = None;
    }
    fn __exit__(
        &mut self,
        _ty: &Bound<'_, PyAny>,
        _value: &Bound<'_, PyAny>,
        _trace: &Bound<'_, PyAny>,
    ) {
        self.release();
    }
}
fn export<'py, T: numpy::Element>(
    field: fn(&Storage) -> &[T],
    shape: &[usize],
    owner: &Bound<'py, Lease>,
) -> PyResult<Bound<'py, PyAny>> {
    let view = ArrayViewD::from_shape(IxDyn(shape), field(&owner.get().storage))
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
    // SAFETY: caller takes data from owner's Arc, never resizes that allocation,
    // and the ndarray base keeps the Arc alive. Arc::get_mut excludes all readers.
    #[allow(unsafe_code)]
    let array = unsafe { PyArray::borrow_from_array(&view, owner.clone().into_any()) };
    array.try_readwrite()?.make_nonwriteable();
    Ok(array.into_any())
}
#[pyclass(name = "VecEnv")]
struct VecEnv {
    gym: Gym,
    cards: Py<PyDict>,
}
#[pymethods]
impl VecEnv {
    #[new]
    #[pyo3(signature=(catalog_dir_or_profile, pods=None, num_envs=1, threads=32, seed=0, config=None))]
    fn new(
        py: Python<'_>,
        catalog_dir_or_profile: String,
        pods: Option<Vec<String>>,
        num_envs: usize,
        threads: usize,
        seed: u64,
        config: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        mtg_core::install_build_identity(mtg_sim::SIM_BUILD).map_err(PyRuntimeError::new_err)?;
        let mut settings = Config::default();
        let mut pin = None;
        if let Some(config) = config {
            for (k, v) in config.iter() {
                match k.extract::<String>()?.as_str() {
                    "payment" => {
                        settings.auto_pay = match v.extract::<String>()?.as_str() {
                            "auto" => true,
                            "manual" => false,
                            _ => {
                                return Err(PyValueError::new_err(
                                    "expected auto or manual payment",
                                ));
                            }
                        }
                    }
                    "opponents" => {
                        settings.baseline = match v.extract::<String>()?.as_str() {
                            "self_play" => false,
                            "baseline" => true,
                            _ => {
                                return Err(PyValueError::new_err(
                                    "expected self_play or baseline opponents",
                                ));
                            }
                        }
                    }
                    "mask_unfundable" => settings.mask_unfundable = v.extract()?,
                    "manifest_sha256" => pin = Some(v.extract::<String>()?),
                    _ => {
                        return Err(PyValueError::new_err(
                            "unknown environment configuration key",
                        ));
                    }
                }
            }
        }
        let source = Source::load(
            &catalog_dir_or_profile,
            &pods.unwrap_or_default(),
            pin.as_deref(),
        )
        .map_err(PyValueError::new_err)?;
        let table = CardTable::new(source.catalog()).map_err(PyValueError::new_err)?;
        let cards = PyDict::new(py);
        let n = table.card_len.len();
        macro_rules! arr {
            ($key:literal,$values:expr,$shape:expr) => {{
                let a = $values.into_pyarray(py).reshape($shape)?;
                a.try_readwrite()?.make_nonwriteable();
                cards.set_item($key, a)?;
            }};
        }
        arr!("card_tokens", table.card_tokens.into_flattened(), [n, 192]);
        arr!("card_num", table.card_num.into_flattened(), [n, 16]);
        arr!("card_len", table.card_len, [n]);
        cards.set_item("vocabulary", table.vocabulary)?;
        cards.set_item("vocabulary_sha256", table.vocabulary_sha256)?;
        let gym =
            Gym::new(source, settings, seed, num_envs, threads).map_err(PyValueError::new_err)?;
        Ok(Self {
            gym,
            cards: cards.unbind(),
        })
    }
    fn catalog<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        self.cards.bind(py).copy()
    }
    fn reset(&mut self, py: Python<'_>) -> PyResult<()> {
        py.detach(|| self.gym.advance(None))
            .map_err(PyRuntimeError::new_err)
    }
    fn step(&mut self, py: Python<'_>, actions: PyReadonlyArray1<'_, i32>) -> PyResult<()> {
        // Strided inputs are accepted. Detach only after taking a Rust-owned copy.
        let owned = actions.as_array().iter().copied().collect::<Vec<_>>();
        py.detach(|| self.gym.advance(Some(&owned)))
            .map_err(PyRuntimeError::new_err)
    }
    fn state_digests(&self) -> PyResult<Vec<String>> {
        let g = &self.gym;
        if !g.initialized {
            return Err(PyRuntimeError::new_err("reset required"));
        }
        (0..g.storage.len)
            .map(|i| {
                g.lanes[i % g.lanes.len()][i / g.lanes.len()]
                    .game
                    .state_digest()
                    .map_err(PyRuntimeError::new_err)
            })
            .collect()
    }
    fn buffers(&self, py: Python<'_>) -> PyResult<Batch> {
        if !self.gym.initialized {
            return Err(PyRuntimeError::new_err("reset required"));
        }
        let buf = &self.gym.storage;
        let owner = Bound::new(
            py,
            Lease {
                storage: buf.clone(),
            },
        )?;
        let arrays = PyDict::new(py);
        macro_rules! give{($name:ident $(,$dim:literal)*)=>{arrays.set_item(stringify!($name),export(|s| &s.$name,&[buf.len$(,$dim)*],&owner)?)?};}
        give!(globals, 48);
        give!(players, 4, 24);
        give!(ent_int, 256, 10);
        give!(ent_num, 256, 24);
        give!(ent_mask, 256);
        give!(cand, 128, 6);
        give!(ptr_mask, 131);
        give!(num_mask, 64);
        give!(head);
        give!(seat);
        give!(reward, 4);
        give!(done);
        give!(info, 8);
        Ok(Batch {
            arrays: Some(arrays.unbind()),
        })
    }
}
#[pymodule]
fn mtg_env(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<VecEnv>()?;
    module.add_class::<Batch>()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn numpy_slice_keeps_the_lease_after_release() {
        Python::initialize();
        Python::attach(|py| {
            let source = Source::load("commander-foundations-v1", &[], None).unwrap();
            let mut env = VecEnv {
                gym: Gym::new(source, Config::default(), 0, 1, 1).unwrap(),
                cards: PyDict::new(py).unbind(),
            };
            env.reset(py).unwrap();
            let mut batch = env.buffers(py).unwrap();
            let arr = batch.__getitem__(py, "globals").unwrap();
            let slice = arr.call_method1("reshape", (-1,)).unwrap();
            drop(arr);
            batch.release();
            assert!(env.reset(py).unwrap_err().to_string().contains("leased"));
            assert!(slice.call_method1("setflags", (true,)).is_err());
            drop(slice);
            env.reset(py).unwrap();
        });
    }
}
