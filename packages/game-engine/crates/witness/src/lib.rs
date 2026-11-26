//! Shared helpers for the mtg-witness card witnesses in `tests/cards`.
//! Development-only: exact frozen source compiled by mtg-compiler and run
//! through mtg-core's `testkit` surface.
use mtg_compiler::CompileResult;
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

/// The Archive R5 header every witness source row is pinned to.
pub const R5_MANIFEST: &str =
    include_str!("../../../tests/fixtures/archive-manifest-phase1-r5.json");

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
