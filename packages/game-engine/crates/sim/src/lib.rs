//! `mtg-sim` library surface. Phase 2D-1 exposes only bounded host search and
//! the exact mana spending its rollouts share with the simulator policies; the
//! remaining simulator modules move behind this library in Phase 2D-2.
pub mod mana_spend;
mod search_candidates;
mod search_jobs;
mod search_policy;
#[cfg(test)]
mod search_roots;
mod search_sampler;

/// Bounded host search over permitted views: candidate plans, sampled worlds,
/// rollouts and deterministic report replay. Rules stay inside opaque core.
pub mod search {
    pub use crate::search_candidates::{ActionPlan, Candidate, CandidateSet, TargetPlan};
    pub use crate::search_jobs::{
        Cancellation, CandidateAggregate, RootTimings, SearchReport, SearchRequest, TrialCounts,
        TrialOutcome, TrialResult, TrialTimings, WorldSpec, WorldTimings, replay_report, run,
        run_cancellable,
    };
}
