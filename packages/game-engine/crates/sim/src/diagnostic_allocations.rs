//! Test-executable instrumentation only. No production allocator policy changes.
//! Source authority: stats_alloc0.1.10 lib.rs forwarding implementation and Region.
//! https://docs.rs/stats_alloc/0.1.10/stats_alloc/
//! Registry crate SHA-256: 5c0e04424e733e69714ca1bbb9204c1a57f09f5493439520f9f68c132ad25eec
//! https://docs.rs/stats_alloc/0.1.10/src/stats_alloc/lib.rs.html
use serde::Serialize;
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, Stats, StatsAlloc};
use std::alloc::System;

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

pub(crate) const ID: &str = "stats-alloc-system-phase-v1";
pub(crate) const CRATE_VERSION: &str = "0.1.10";

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub(crate) struct Counters {
    pub allocations: usize,
    pub deallocations: usize,
    pub reallocations: usize,
    pub bytes_allocated: usize,
    pub bytes_deallocated: usize,
    pub bytes_reallocated: isize,
}
impl From<Stats> for Counters {
    fn from(value: Stats) -> Self {
        Self {
            allocations: value.allocations,
            deallocations: value.deallocations,
            reallocations: value.reallocations,
            bytes_allocated: value.bytes_allocated,
            bytes_deallocated: value.bytes_deallocated,
            bytes_reallocated: value.bytes_reallocated,
        }
    }
}

/// Region snapshots are process-global, not thread-local. Six atomic loads do
/// not constitute an atomic vector; the exact single test must be quiescent.
pub(crate) fn begin() -> Region<'static, System> {
    Region::new(GLOBAL)
}
pub(crate) fn snapshot() -> Counters {
    GLOBAL.stats().into()
}
pub(crate) fn initial(region: &Region<'_, System>) -> Counters {
    region.initial().into()
}

/// Checked raw-snapshot difference, rather than Region::change's unchecked
/// integer subtraction. Counts are requests before underlying allocation, not
/// a successful-allocation census or theoretical source-level object count.
pub(crate) fn difference(before: Counters, after: Counters) -> Result<Counters, String> {
    let unsigned = |a: usize, b: usize| {
        b.checked_sub(a)
            .ok_or_else(|| "allocation counter wrapped or regressed".to_string())
    };
    Ok(Counters {
        allocations: unsigned(before.allocations, after.allocations)?,
        deallocations: unsigned(before.deallocations, after.deallocations)?,
        reallocations: unsigned(before.reallocations, after.reallocations)?,
        bytes_allocated: unsigned(before.bytes_allocated, after.bytes_allocated)?,
        bytes_deallocated: unsigned(before.bytes_deallocated, after.bytes_deallocated)?,
        bytes_reallocated: after
            .bytes_reallocated
            .checked_sub(before.bytes_reallocated)
            .ok_or("signed realloc byte counter overflow")?,
    })
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(crate) struct ThreadGuard {
    process: u32,
    current_thread: u32,
    observed_threads: Vec<u32>,
}
impl ThreadGuard {
    /// Linux-only trusted diagnostic. Reject worker pools/other live tests.
    /// The libtest main thread remains present and waiting; counters still
    /// include it. Stable IDs plus an empty interval control never imply TLS.
    pub(crate) fn capture() -> Result<Self, String> {
        let args = std::env::args().collect::<Vec<_>>();
        if !args.iter().any(|a| a == "--exact") || !args.iter().any(|a| a == "--test-threads=1") {
            return Err("allocation diagnostic requires one exact test/thread".into());
        }
        let process = std::process::id();
        let link = std::fs::read_link("/proc/thread-self").map_err(|e| e.to_string())?;
        let current_thread: u32 = link
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or("current Linux TID missing")?
            .parse()
            .map_err(|_| "current Linux TID invalid")?;
        let mut observed_threads = std::fs::read_dir("/proc/self/task")
            .map_err(|e| e.to_string())?
            .map(|e| {
                e.map_err(|e| e.to_string())?
                    .file_name()
                    .to_str()
                    .ok_or("Linux TID invalid")?
                    .parse::<u32>()
                    .map_err(|_| "Linux TID invalid".to_string())
            })
            .collect::<Result<Vec<_>, String>>()?;
        observed_threads.sort_unstable();
        let mut expected = vec![process, current_thread];
        expected.sort_unstable();
        if process == current_thread || observed_threads != expected {
            return Err("unexpected allocating worker/test threads".into());
        }
        Ok(Self {
            process,
            current_thread,
            observed_threads,
        })
    }
    pub(crate) fn unchanged(&self) -> Result<(), String> {
        if self != &Self::capture()? {
            return Err("allocation diagnostic thread identity changed".into());
        }
        Ok(())
    }
}

/// Diagnostic control only. No subtraction of this interval from real counts.
/// Any unexpected allocation is a hard failure, not estimated noise removal.
pub(crate) fn empty_control() -> Result<Counters, String> {
    let region = begin();
    std::thread::sleep(std::time::Duration::from_millis(25));
    let after = snapshot();
    let delta = difference(initial(&region), after)?;
    if delta != Counters::from(Stats::default()) {
        return Err("unrelated allocations in empty control interval".into());
    }
    Ok(delta)
}

#[test]
fn diagnostic_allocation_checked_difference_rejects_regression_and_signed_overflow() {
    let zero = Counters::from(Stats::default());
    let mut before = zero;
    before.allocations = 1;
    assert!(difference(before, zero).is_err());
    before = zero;
    before.bytes_reallocated = isize::MIN;
    let mut after = zero;
    after.bytes_reallocated = isize::MAX;
    assert!(difference(before, after).is_err());
    before = zero;
    before.bytes_reallocated = 32;
    assert_eq!(difference(before, zero).unwrap().bytes_reallocated, -32);
}

#[test]
#[ignore = "explicit fresh exact diagnostic process only; process-global counters"]
fn diagnostic_allocation_known_vector_requests() {
    let threads = ThreadGuard::capture().unwrap();
    empty_control().unwrap();
    let region = begin();
    let mut bytes = Vec::<u8>::with_capacity(32);
    bytes.resize(32, 7);
    std::hint::black_box(&mut bytes);
    bytes.reserve_exact(32);
    bytes.truncate(16);
    bytes.shrink_to_fit();
    std::hint::black_box(&mut bytes);
    drop(bytes);
    let after = snapshot();
    let delta = difference(initial(&region), after).unwrap();
    threads.unchanged().unwrap();
    // This checks actual optimized std Vec behavior on the pinned compiler.
    // It is not an unsafe assumption that Rust must always allocate this way.
    assert_eq!(delta.allocations, 1);
    assert_eq!(delta.reallocations, 2);
    assert_eq!(delta.deallocations, 1);
    assert_eq!(delta.bytes_allocated, 64);
    assert_eq!(delta.bytes_deallocated, 64);
    assert_eq!(delta.bytes_reallocated, -16);
    empty_control().unwrap();
}
