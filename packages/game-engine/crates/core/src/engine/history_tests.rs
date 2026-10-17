use super::*;
use mtg_contracts::{canonical, digest};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicUsize, Ordering};

fn entry<T>(history: &History<T>, index: usize) -> &Arc<HistoryEntry<T>> {
    let chunk = index / CHUNK;
    if chunk < history.chunks.len() {
        &history.chunks[chunk][index % CHUNK]
    } else {
        &history.tail[index % CHUNK]
    }
}
fn check<T>(history: &History<T>, reference: &[T])
where
    T: Clone + Eq + fmt::Debug + Serialize + for<'de> Deserialize<'de>,
{
    assert_eq!(history.len(), reference.len());
    assert_eq!(history.is_empty(), reference.is_empty());
    assert_eq!(history.last(), reference.last());
    assert_eq!(
        history.iter().collect::<Vec<_>>(),
        reference.iter().collect::<Vec<_>>()
    );
    assert_eq!(
        history.iter().rev().collect::<Vec<_>>(),
        reference.iter().rev().collect::<Vec<_>>()
    );
    assert_eq!(format!("{history:?}"), format!("{reference:?}"));
    assert_eq!(
        serde_json::to_string(history).unwrap(),
        serde_json::to_string(reference).unwrap()
    );
    assert_eq!(canonical(history).unwrap(), canonical(&reference).unwrap());
    assert_eq!(digest(history).unwrap(), digest(&reference).unwrap());
    let restored: History<T> = serde_json::from_str(&canonical(&reference).unwrap()).unwrap();
    assert_eq!(&restored, history);
    assert_eq!(history.as_slice(), reference);
    if let Some(first) = reference.first() {
        assert!(history.contains(first));
    }
}
fn value(n: usize) -> Value {
    json!({
        "z": [format!("view{n}: 雪\n\u{0}\"\\"), -((n as i64) + 1)],
        "a": {"objects": [{"handle": format!("v0-o{n}"), "oracle": "private memory"}],
              "knowledge": ["bottom first", "bottom last"]},
        "numericKeys": {"2": n, "100": n, "11": n}
    })
}
#[test]
fn complete_arrays_match_vec_oracle_across_every_chunk_edge_and_branch() {
    let mut original = History::default();
    let mut reference = vec![];
    for len in 0..=132 {
        check(&original, &reference);
        let mut branch = original.clone();
        let mut branch_reference = reference.clone();
        assert!(branch.flat.get().is_none());
        assert_eq!(branch, original);
        for i in 0..len {
            assert!(Arc::ptr_eq(entry(&branch, i), entry(&original, i)));
        }
        branch.push(value(len + 1000));
        branch_reference.push(value(len + 1000));
        check(&branch, &branch_reference);
        if len > 0 {
            let index = len / 2;
            branch[index] = value(len + 2000);
            branch_reference[index] = value(len + 2000);
            assert!(branch.flat.get().is_none());
            for i in 0..len {
                assert_eq!(
                    Arc::ptr_eq(entry(&branch, i), entry(&original, i)),
                    i != index
                );
            }
            check(&branch, &branch_reference);
        }
        check(&original, &reference);
        branch.clear();
        check(&branch, &[]);
        check(&original, &reference);
        original.push(value(len));
        reference.push(value(len));
    }
}

struct Counted {
    value: usize,
    clones: Arc<AtomicUsize>,
}
impl Clone for Counted {
    fn clone(&self) -> Self {
        self.clones.fetch_add(1, Ordering::Relaxed);
        Self {
            value: self.value,
            clones: self.clones.clone(),
        }
    }
}
#[test]
fn forks_clone_no_entries_and_mutation_copies_only_the_selected_shared_entry() {
    let clones = Arc::new(AtomicUsize::new(0));
    let mut original: History<Counted> = History::default();
    for len in 0..=132 {
        clones.store(0, Ordering::Relaxed);
        let fork = original.clone();
        assert_eq!(clones.load(Ordering::Relaxed), 0, "fork length {len}");
        assert_eq!(fork.len(), len);
        // Explicitly contrast the former deep-cloned tail representation.
        let former_tail: Vec<_> = original.iter().skip(len / CHUNK * CHUNK).cloned().collect();
        assert_eq!(clones.load(Ordering::Relaxed), len % CHUNK);
        drop(former_tail);
        if len > 0 {
            for index in [0, len / 2, len - 1] {
                clones.store(0, Ordering::Relaxed);
                let mut branch = original.clone();
                branch[index].value = usize::MAX;
                assert_eq!(
                    clones.load(Ordering::Relaxed),
                    1,
                    "mutating {index} of {len}"
                );
                assert_eq!(original[index].value, index);
                assert_eq!(fork[index].value, index);
                for i in 0..len {
                    assert_eq!(
                        Arc::ptr_eq(entry(&branch, i), entry(&original, i)),
                        i != index
                    );
                }
                // Its entry is now unique; editing it again performs no clone.
                branch[index].value = usize::MAX - 1;
                assert_eq!(clones.load(Ordering::Relaxed), 1);
            }
            // A populated compatibility cache is deliberately not cloned.
            original.as_slice();
            clones.store(0, Ordering::Relaxed);
            let branch = original.clone();
            assert_eq!(clones.load(Ordering::Relaxed), 0);
            assert!(branch.flat.get().is_none());
            assert!(original.flat.get().is_some());
        }
        original.push(Counted {
            value: len,
            clones: clones.clone(),
        });
    }
}

#[test]
fn chunk_and_entry_cow_preserves_original_branches_and_flat_views() {
    let mut original = History::default();
    for n in 0..130 {
        original.push(n);
    }
    let wire = serde_json::to_string(&(0..130).collect::<Vec<_>>()).unwrap();
    assert_eq!(serde_json::to_string(&original).unwrap(), wire);
    let restored: History<i32> = serde_json::from_str(&wire).unwrap();
    assert_eq!(original, restored);
    original.as_slice();
    let mut branch = original.clone();
    assert!(Arc::ptr_eq(&original.chunks[0], &branch.chunks[0]));
    assert!(branch.flat.get().is_none());
    branch.push(130);
    assert!(Arc::ptr_eq(&original.chunks[0], &branch.chunks[0]));
    branch.as_slice();
    branch[31] = -1;
    assert!(!Arc::ptr_eq(&original.chunks[0], &branch.chunks[0]));
    assert!(!Arc::ptr_eq(entry(&original, 31), entry(&branch, 31)));
    assert!(Arc::ptr_eq(entry(&original, 30), entry(&branch, 30)));
    assert!(Arc::ptr_eq(&original.chunks[1], &branch.chunks[1]));
    assert_eq!(original.as_slice()[31], 31);
    assert_eq!(branch.as_slice()[31], -1);
    branch.clear();
    assert!(branch.is_empty());
    assert_eq!(original.len(), 130);
}

#[derive(Clone)]
struct CountedCanonical {
    value: usize,
    serializations: Arc<AtomicUsize>,
}
impl Serialize for CountedCanonical {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.serializations.fetch_add(1, Ordering::SeqCst);
        self.value.serialize(serializer)
    }
}
#[test]
fn canonical_entry_memos_are_shared_once_across_forks_chunks_and_threads() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut original = History::default();
    for value in 0..97 {
        original.push(CountedCanonical {
            value,
            serializations: calls.clone(),
        });
    }
    let before_warming = original.clone();
    for index in 0..original.len() {
        assert_eq!(
            original.canonical_entry_bytes(index).unwrap(),
            index.to_string().as_bytes()
        );
        assert!(std::ptr::eq(
            original.canonical_entry_bytes(index).unwrap(),
            before_warming.canonical_entry_bytes(index).unwrap()
        ));
    }
    assert_eq!(calls.load(Ordering::SeqCst), 97);
    let shared = Arc::new(original);
    std::thread::scope(|scope| {
        for _ in 0..8 {
            let history = shared.clone();
            scope.spawn(move || {
                for index in 0..history.len() {
                    assert_eq!(
                        history.canonical_entry_bytes(index).unwrap(),
                        index.to_string().as_bytes()
                    );
                }
            });
        }
    });
    assert_eq!(calls.load(Ordering::SeqCst), 97);
    // Cold simultaneous initialization also invokes Serialize exactly once.
    let mut cold = History::default();
    cold.push(CountedCanonical {
        value: 999,
        serializations: calls.clone(),
    });
    std::thread::scope(|scope| {
        for _ in 0..8 {
            let history = &cold;
            scope.spawn(move || {
                assert_eq!(history.canonical_entry_bytes(0).unwrap(), b"999");
            });
        }
    });
    assert_eq!(calls.load(Ordering::SeqCst), 98);
}
#[test]
fn custom_canonical_initializer_skips_warm_entries_and_never_installs_panics() {
    let mut history = History::default();
    for value in 0..3usize {
        history.push(value);
    }
    let calls = AtomicUsize::new(0);
    let initialize = |value: &usize| {
        calls.fetch_add(1, Ordering::SeqCst);
        Ok(canonical(value).unwrap().into_bytes().into_boxed_slice())
    };
    assert_eq!(
        history.canonical_entry_bytes_with(0, initialize).unwrap(),
        b"0"
    );
    assert_eq!(
        history
            .canonical_entry_bytes_with(0, |_| panic!("warm initializer must not run"))
            .unwrap(),
        b"0"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    history[0] = 100;
    assert_eq!(
        history.canonical_entry_bytes_with(0, initialize).unwrap(),
        b"100"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(
        history.canonical_entry_bytes_with(1, |_| Err("injected canonical error".into())),
        Err("injected canonical error".into())
    );
    assert_eq!(
        history.canonical_entry_bytes_with(1, |_| panic!("cached error must not rerun")),
        Err("injected canonical error".into())
    );
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = history.canonical_entry_bytes_with(2, |_| panic!("injected initializer panic"));
    }));
    assert!(panic.is_err());
    assert!(entry(&history, 2).canonical.get().is_none());
    assert_eq!(
        history.canonical_entry_bytes_with(2, initialize).unwrap(),
        b"2"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 3);
}
#[test]
fn canonical_memos_invalidate_on_both_shared_and_unique_mutation_without_wire_changes() {
    let mut original = History::default();
    for index in 0..97 {
        original.push(value(index));
        original.canonical_entry_bytes(index).unwrap();
    }
    let original_wire = canonical(&original).unwrap();
    for index in [0, 31, 32, 63, 64, 95, 96] {
        let mut branch = original.clone();
        let old = original.canonical_entry_bytes(index).unwrap().to_vec();
        assert!(entry(&branch, index).canonical.get().is_some());
        branch[index] = value(index + 1000);
        assert!(entry(&branch, index).canonical.get().is_none());
        assert_ne!(branch.canonical_entry_bytes(index).unwrap(), old);
        assert_eq!(original.canonical_entry_bytes(index).unwrap(), old);
        // After COW this entry is unique. A second mutable borrow MUST evict
        // the warm memo even though Arc::make_mut no longer clones anything.
        assert_eq!(Arc::strong_count(entry(&branch, index)), 1);
        branch[index] = value(index + 2000);
        assert!(entry(&branch, index).canonical.get().is_none());
        assert_eq!(
            branch.canonical_entry_bytes(index).unwrap(),
            canonical(&value(index + 2000)).unwrap().as_bytes()
        );
        let flat = branch.as_slice().to_vec();
        assert_eq!(canonical(&branch).unwrap(), canonical(&flat).unwrap());
        let restored: History<Value> = serde_json::from_str(&canonical(&branch).unwrap()).unwrap();
        assert_eq!(restored, branch);
        assert!(
            entry(&restored, index).canonical.get().is_none(),
            "derived bytes must never enter serde"
        );
        assert_eq!(
            restored.canonical_entry_bytes(index).unwrap(),
            branch.canonical_entry_bytes(index).unwrap()
        );
        assert_eq!(canonical(&original).unwrap(), original_wire);
    }
    let mut unique = History::default();
    unique.push(value(0));
    unique.canonical_entry_bytes(0).unwrap();
    unique[0]["z"][0] = "unique 雪\u{0}\n".into();
    assert!(entry(&unique, 0).canonical.get().is_none());
    assert_eq!(
        unique.canonical_entry_bytes(0).unwrap(),
        canonical(&unique[0]).unwrap().as_bytes()
    );
}
