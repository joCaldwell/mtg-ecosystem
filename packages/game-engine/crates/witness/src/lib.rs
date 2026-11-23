//! Shared helpers for the mtg-witness integration binaries in `tests/`.
//! Development-only: exact frozen source compiled by mtg-compiler and run
//! through mtg-core's `testkit` surface.
use mtg_compiler::CompileResult;
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

/// The Archive R4 header every witness source row is pinned to.
pub const R4_MANIFEST: &str =
    include_str!("../../../tests/fixtures/archive-manifest-phase1-r4.json");

pub mod equipped;
pub mod temporary;

/// `mtg_compiler::compile`, memoized per test process. Lowering is a pure
/// function of the exact manifest and source bytes, so every witness that
/// rebuilds a scenario (per seed, trace mode or case) reuses one result.
pub fn compile(manifest: &str, raw: &str) -> CompileResult {
    type Cache = Mutex<HashMap<(String, String), CompileResult>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let key = (manifest.to_owned(), raw.to_owned());
    if let Some(hit) = cache.lock().unwrap().get(&key) {
        return hit.clone();
    }
    // Compile outside the lock; a concurrent duplicate is identical.
    let result = mtg_compiler::compile(manifest, raw);
    cache.lock().unwrap().insert(key, result.clone());
    result
}

/// Runs `run` over a small batch on scoped threads, returning results in batch
/// order; a failing item re-raises its own panic. Private to `OrderedVerifier`
/// so test helpers never nest thread pools under libtest's `--test-threads`.
fn parallel<T: Send, R: Send>(
    items: impl IntoIterator<Item = T>,
    run: impl Fn(T) -> R + Sync,
) -> Vec<R> {
    std::thread::scope(|scope| {
        let run = &run;
        let handles: Vec<_> = items
            .into_iter()
            .map(|item| scope.spawn(move || run(item)))
            .collect();
        handles
            .into_iter()
            .map(|h| {
                h.join()
                    .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
            })
            .collect()
    })
}

/// `SnapshotVerifier::verify` for a long snapshot sequence. `verify` decodes
/// a snapshot (independent of every other snapshot) and then proves it is the
/// next state reachable from the verifier's cursor (strictly ordered). This
/// decodes a small batch concurrently and proves every snapshot in push order,
/// so each snapshot gets exactly the checks `verify` performs. The batch is
/// tiny (bounded threads and resident snapshots); never call it from inside
/// another thread pool. Call `finish` to prove the remaining batch.
pub struct OrderedVerifier {
    verifier: mtg_core::SnapshotVerifier,
    prepared: mtg_core::Prepared,
    pending: Vec<String>,
}

impl OrderedVerifier {
    const BATCH: usize = 4;
    pub fn new(prepared: mtg_core::Prepared, mode: mtg_core::TraceMode) -> Self {
        Self {
            verifier: mtg_core::SnapshotVerifier::new(prepared.clone(), mode),
            prepared,
            pending: Vec::new(),
        }
    }
    pub fn verify(&mut self, snapshot: String) {
        self.pending.push(snapshot);
        if self.pending.len() == Self::BATCH {
            self.finish();
        }
    }
    pub fn finish(&mut self) {
        use mtg_core::testkit::{GameTestkit, SnapshotVerifierTestkit};
        let prepared = &self.prepared;
        let decoded = parallel(self.pending.drain(..), |json| {
            let decoded = mtg_core::Game::read_snapshot(prepared.clone(), &json).unwrap();
            drop(json);
            decoded
        });
        for (game, digest) in decoded {
            self.verifier.verify_game(game, digest).unwrap();
        }
    }
}
