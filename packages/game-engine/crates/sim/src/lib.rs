//! Finite Commander simulation, public-view policy, analytics and bounded search.
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

pub mod analytics;
#[cfg(test)]
mod coverage_cli;
pub mod events;
pub mod paired;
pub mod play;
pub mod pods;
pub mod policy;
#[cfg(test)]
mod smoke;

#[cfg(test)]
const BUILD: mtg_core::BuildIdentity = mtg_core::compiled_build_identity!();
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recorded_debug_assertions_match_the_target_configuration() {
        assert_eq!(BUILD.debug_assertions, cfg!(debug_assertions).to_string());
    }
}
