//! Batched four-seat Commander simulation.
mod buffers;
mod ffi;
mod game;
use buffers::Storage;
use game::{Config, Lane, Source};
use std::sync::Arc;
struct Gym {
    source: Source,
    config: Config,
    seed: u64,
    lanes: Vec<Vec<Lane>>,
    storage: Arc<Storage>,
    initialized: bool,
}
impl Gym {
    fn new(
        source: Source,
        config: Config,
        seed: u64,
        n: usize,
        threads: usize,
    ) -> Result<Self, String> {
        if n == 0 || threads == 0 {
            return Err("num_envs and threads must be positive".into());
        }
        let workers = threads.min(n);
        Ok(Self {
            source,
            config,
            seed,
            lanes: (0..workers).map(|_| Vec::new()).collect(),
            storage: Arc::new(Storage::new(n)),
            initialized: false,
        })
    }
    fn advance(&mut self, actions: Option<&[i32]>) -> Result<(), String> {
        let buf = Arc::get_mut(&mut self.storage)
            .ok_or("batch is leased; release all arrays and derived views")?;
        let workers = self.lanes.len();
        let n = buf.len;
        if let Some(a) = actions {
            if !self.initialized {
                return Err("reset required".into());
            }
            if a.len() != n {
                return Err("expected one action per environment".into());
            }
            for (i, &action) in a.iter().enumerate() {
                let mask = if buf.head[i] == 0 {
                    &buf.ptr_mask[131 * i..131 * (i + 1)]
                } else {
                    &buf.num_mask[64 * i..64 * (i + 1)]
                };
                if usize::try_from(action).ok().and_then(|j| mask.get(j)) != Some(&1) {
                    buf.info[i * 8] = buf.info[i * 8].saturating_add(1);
                    self.lanes[i % workers][i / workers].illegal = buf.info[i * 8];
                    return Err(format!("masked action at batch index {i}"));
                }
            }
        }
        self.initialized = false;
        let seed = self.seed;
        let source = &self.source;
        let config = self.config;
        std::thread::scope(|scope| {
            let mut tasks = vec![];
            for (worker, (lanes, mut rows)) in self
                .lanes
                .iter_mut()
                .zip(buf.partition(workers))
                .enumerate()
            {
                tasks.push(scope.spawn(move || -> Result<(), String> {
                    if actions.is_none() {
                        lanes.clear();
                    }
                    for (j, out) in rows.iter_mut().enumerate() {
                        let i = worker + j * workers;
                        if actions.is_none() {
                            lanes.push(Lane::new(source, seed, i, 0)?);
                        }
                        let row = lanes[j].run(source, config, actions.map(|a| a[i] as usize))?;
                        out.write(&row);
                    }
                    Ok(())
                }));
            }
            for task in tasks {
                task.join().map_err(|_| "environment worker panicked")??;
            }
            Ok::<_, String>(())
        })?;
        self.initialized = true;
        Ok(())
    }
}
#[cfg(test)]
mod tests;
