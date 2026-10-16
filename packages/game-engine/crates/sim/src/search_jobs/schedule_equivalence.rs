//! Fixed semantic work, not a wall-clock or isolated-performance assertion.
use super::*;
use mtg_contracts::{canonical, sha256};
use std::sync::Barrier;

fn no_deadline() -> Control {
    Control {
        started: Instant::now(),
        cancellation: Cancellation::default(),
        stopped: AtomicBool::new(false),
        reason: Mutex::new(None),
        operations: AtomicU64::new(0),
        enforce_deadline: false,
    }
}

/// Same index/attempt/rejection order as the production producer. Retain all
/// 32 specs to prove that taking the first eight does not change their sampling.
fn producer_worlds(capture: &SearchCapture, seed: &[u8; 32]) -> Vec<(WorldSpec, WorldProposal)> {
    let mut seen = BTreeSet::new();
    let mut worlds = vec![];
    for index in 0..DESIRED_WORLDS {
        let mut accepted = None;
        for attempt in 0..search_sampler::MAX_DISTINCT_ATTEMPTS {
            let proposal =
                search_sampler::propose(capture, seed, index as u64, attempt, &mut || Ok(()))
                    .unwrap();
            let assignment_digest = proposal.assignment_digest().unwrap();
            if seen.insert(assignment_digest.clone()) {
                accepted = Some((
                    WorldSpec {
                        world_index: index as u64,
                        sampling_attempt: attempt,
                        assignment_digest,
                    },
                    proposal,
                ));
                break;
            }
        }
        worlds.push(accepted.expect("fixed workload has a distinct next assignment"));
    }
    worlds
}

struct ScheduleResult {
    trials: Vec<TrialResult>,
    build_calls: Vec<(u64, u64)>,
}

fn execute(
    catalog: &Catalog,
    capture: &SearchCapture,
    candidates: &[Candidate],
    proposals: Vec<WorldProposal>,
    workers: usize,
) -> ScheduleResult {
    let queue = Mutex::new(VecDeque::from(proposals));
    let barrier = Barrier::new(workers);
    let control = no_deadline();
    let viewer = capture.data().context.observation.view.viewer;
    let mut trials = vec![];
    let mut builds = vec![];
    thread::scope(|scope| {
        let handles = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    // Reserve one job per worker before starting; the four-worker
                    // branch really exercises four independent executors.
                    let mut next = queue.lock().unwrap().pop_front();
                    barrier.wait();
                    let mut local_trials = vec![];
                    let mut local_builds = vec![];
                    while let Some(proposal) = next {
                        let world_index = proposal.world_index;
                        let assignment_digest = proposal.assignment_digest().unwrap();
                        let mut calls = 0;
                        // Only the permitted capture and an explicit hypothesis
                        // enter construction; no real Game/seed is supplied here.
                        let (base, map) =
                            build_world(catalog, capture, proposal, &control, &mut calls).unwrap();
                        local_builds.push((world_index, calls));
                        for (candidate_index, candidate) in candidates.iter().enumerate() {
                            let (counts, result, timings) =
                                run_trial(&base, &map, candidate, viewer, catalog, &control);
                            assert!(
                                result.useful(),
                                "w{world_index}/c{candidate_index}: {result:?}"
                            );
                            local_trials.push(TrialResult {
                                world_index,
                                assignment_digest: assignment_digest.clone(),
                                candidate_index,
                                candidate_id: candidate.candidate_id.clone(),
                                counts,
                                timings,
                                result,
                            });
                        }
                        next = queue.lock().unwrap().pop_front();
                    }
                    assert!(!local_builds.is_empty());
                    (local_trials, local_builds)
                })
            })
            .collect::<Vec<_>>();
        for handle in handles {
            let (local_trials, local_builds) = handle.join().expect("semantic worker panicked");
            trials.extend(local_trials);
            builds.extend(local_builds);
        }
    });
    assert!(queue.into_inner().unwrap().is_empty());
    assert!(!control.stopped.load(Ordering::SeqCst));
    trials.sort_by_key(|trial| (trial.world_index, trial.candidate_index));
    builds.sort_unstable();
    ScheduleResult {
        trials,
        build_calls: builds,
    }
}

#[test]
fn first_eight_worlds_all_candidates_are_exact_across_worker_schedules() {
    let catalog = mtg_compiler::match_fixture_catalog().unwrap();
    let fixture = super::super::search_roots::prepare(&catalog, 0).unwrap();
    let capture = super::super::search_roots::capture(&fixture.game, fixture.case.viewer).unwrap();
    drop(fixture);
    let candidates = search_candidates::generate(
        &catalog,
        &capture.data().context.observation,
        &mut || Ok(()),
    )
    .unwrap();
    assert_eq!(
        candidates.candidates.len(),
        3,
        "frozen root0 candidate closure"
    );
    assert!(!candidates.truncated_at_limit);
    let hash = sha256(b"commander-open-search-v1/job/0");
    let seed = std::array::from_fn(|i| u8::from_str_radix(&hash[2 * i..2 * i + 2], 16).unwrap());
    let produced = producer_worlds(&capture, &seed);
    let first = &produced[..MINIMUM_WORLDS];
    let mut seen = BTreeSet::new();
    for (index, (spec, proposal)) in first.iter().enumerate() {
        assert_eq!(spec.world_index, index as u64);
        // For this named root the producer accepts attempt0 every time. A later
        // attempt cannot silently substitute a different favorable world.
        assert_eq!(spec.sampling_attempt, 0);
        assert!(seen.insert(spec.assignment_digest.clone()));
        let direct =
            search_sampler::propose(&capture, &seed, index as u64, 0, &mut || Ok(())).unwrap();
        assert_eq!(canonical(proposal).unwrap(), canonical(&direct).unwrap());
        assert_eq!(
            proposal.assignment_digest().unwrap(),
            spec.assignment_digest
        );
    }
    // assignment_digest quotients interchangeable basics, hand order, symbols
    // and future RNG: these eight are genuinely distinct hidden assignments.
    assert_eq!(seen.len(), MINIMUM_WORLDS);
    let serial = execute(
        &catalog,
        &capture,
        &candidates.candidates,
        first.iter().map(|(_, proposal)| proposal.clone()).collect(),
        1,
    );
    let reordered = [7, 0, 6, 1, 5, 2, 4, 3];
    let parallel = execute(
        &catalog,
        &capture,
        &candidates.candidates,
        reordered.iter().map(|&i| first[i].1.clone()).collect(),
        4,
    );
    assert_eq!(serial.build_calls, parallel.build_calls);
    assert_eq!(serial.build_calls.len(), MINIMUM_WORLDS);
    assert_eq!(serial.trials.len(), 24);
    assert_eq!(parallel.trials.len(), 24);
    for (a, b) in serial.trials.iter().zip(&parallel.trials) {
        assert_eq!(
            (a.world_index, a.candidate_index),
            (b.world_index, b.candidate_index)
        );
        assert_eq!(a.assignment_digest, b.assignment_digest);
        assert_eq!(a.candidate_id, b.candidate_id);
        assert_eq!(a.counts, b.counts);
        // Includes full semantic State SHA, exact evaluations and terminal/horizon
        // classification. Timings and executor operation interleaving are not state.
        assert_eq!(a.result, b.result);
        assert!(a.counts.accepted_inputs > 0);
        assert!(a.counts.rule_steps > 0);
    }
    let a = aggregates(&candidates.candidates, &serial.trials);
    let b = aggregates(&candidates.candidates, &parallel.trials);
    assert_eq!(a, b);
    assert!(
        a.iter()
            .all(|row| row.minimum_met && row.useful_distinct_worlds == 8)
    );
    let viewer = usize::from(capture.data().context.observation.view.viewer.0);
    let chosen = select_candidate(&a, viewer);
    assert!(chosen.is_some());
    assert_eq!(chosen, select_candidate(&b, viewer));
    // SampledGame intentionally exposes no trace-mode override. Core event-slice
    // trace-equivalence tests complement this scheduler test without a new API.
}
