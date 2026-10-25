//! Test-host diagnostics only. No production search deadline or policy changes.
//! The producer finishes an immutable complete worklist before any worker runs.
use super::*;
use mtg_contracts::{canonical, sha256};
use std::collections::BTreeMap;

mod persistent;

const FIXED_WORK_VERSION: &str = "foundations-fixed-search-work-v1";
const MAX_DIAGNOSTIC_WORKERS: usize = 64;

#[derive(Clone, Debug, Serialize)]
pub(crate) struct FrozenWorld {
    pub world_index: u64,
    pub sampling_attempt: u64,
    pub assignment_digest: String,
    pub proposal: WorldProposal,
}

/// Not deserializable or publicly constructible: only the reviewed producer can
/// freeze it. Serialized proposals are trusted diagnostic evidence, not client data.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct FrozenWork {
    version: &'static str,
    context_id: String,
    public_job_seed: [u8; 32],
    horizon: u64,
    candidate_policy: CandidateSet,
    worlds: Vec<FrozenWorld>,
}
impl FrozenWork {
    pub(crate) fn candidates(&self) -> &CandidateSet {
        &self.candidate_policy
    }
    pub(crate) fn worlds(&self) -> &[FrozenWorld] {
        &self.worlds
    }
    pub(crate) fn digest(&self) -> Result<String, String> {
        Ok(sha256(canonical(self)?.as_bytes()))
    }
}

pub(crate) fn freeze(
    catalog: &Catalog,
    capture: &SearchCapture,
    public_job_seed: [u8; 32],
    world_count: usize,
) -> Result<FrozenWork, String> {
    if !(MINIMUM_WORLDS..=DESIRED_WORLDS).contains(&world_count) {
        return Err("fixed work requires eight to thirty-two distinct worlds".into());
    }
    let candidates =
        search_candidates::generate(catalog, &capture.data().context.observation, &mut || Ok(()))?;
    if candidates.candidates.is_empty() || candidates.truncated_at_limit {
        return Err(
            "fixed diagnostic requires a complete nonempty candidate adapter result".into(),
        );
    }
    let mut worlds = Vec::with_capacity(world_count);
    let mut distinct = BTreeSet::new();
    for world_index in 0..world_count {
        let mut next = None;
        // Exact production sampler order, including rejected duplicate worlds.
        for attempt in 0..search_sampler::MAX_DISTINCT_ATTEMPTS {
            let proposal = search_sampler::propose(
                capture,
                &public_job_seed,
                world_index as u64,
                attempt,
                &mut || Ok(()),
            )?;
            let assignment_digest = proposal.assignment_digest()?;
            if distinct.insert(assignment_digest.clone()) {
                next = Some(FrozenWorld {
                    world_index: world_index as u64,
                    sampling_attempt: attempt,
                    assignment_digest,
                    proposal,
                });
                break;
            }
        }
        worlds.push(next.ok_or("fixed diagnostic distinct-world support shortfall")?);
    }
    Ok(FrozenWork {
        version: FIXED_WORK_VERSION,
        context_id: capture.context_id().into(),
        public_job_seed,
        horizon: HORIZON,
        candidate_policy: candidates,
        worlds,
    })
}

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

/// Direct authoritative position reference, not a reconstructed hypothesis.
/// Slot labels denote eight deterministic repeats of ONE true world. Every
/// repeat preserves the real origin/RNG; they must never satisfy eight-world
/// diversity or be reported as eight independent hidden assignments.
pub(crate) fn evaluate_real(
    catalog: &Catalog,
    root: &mtg_core::Game,
    capture: &SearchCapture,
    candidates: &CandidateSet,
) -> Result<Vec<TrialResult>, String> {
    let viewer = capture.data().context.observation.view.viewer;
    let control = no_deadline();
    let mut checks = TrialCounts::default();
    let mut timings = TrialTimings::default();
    let observed = observation(root, viewer, &control, &mut checks, &mut timings)?;
    if observed != capture.data().context.observation {
        return Err("true reference Game does not match the captured permitted root".into());
    }
    let expected = search_candidates::generate(catalog, &observed, &mut || Ok(()))?;
    if *candidates != expected {
        return Err("true reference changed the permitted candidate worklist".into());
    }
    let before = root.state_digest()?;
    let mut trials = Vec::with_capacity(MINIMUM_WORLDS * candidates.candidates.len());
    for slot in 0..MINIMUM_WORLDS {
        for (candidate_index, candidate) in candidates.candidates.iter().enumerate() {
            let (counts, result, timings) = run_trial_body(
                root,
                || Ok(PlanSteps::identity(candidate)),
                viewer,
                catalog,
                &control,
            );
            trials.push(TrialResult {
                world_index: slot as u64,
                assignment_digest: before.clone(),
                candidate_index,
                candidate_id: candidate.candidate_id.clone(),
                counts,
                timings,
                result,
            });
        }
    }
    if root.state_digest()? != before {
        return Err("true reference mutated its authoritative root".into());
    }
    Ok(trials)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct SemanticTrial {
    pub world_index: u64,
    pub assignment_digest: String,
    pub candidate_index: usize,
    pub candidate_id: String,
    pub counts: TrialCounts,
    /// Includes exact evaluations and complete State SHA for every useful trial.
    pub result: TrialOutcome,
}
impl From<&TrialResult> for SemanticTrial {
    fn from(value: &TrialResult) -> Self {
        Self {
            world_index: value.world_index,
            assignment_digest: value.assignment_digest.clone(),
            candidate_index: value.candidate_index,
            candidate_id: value.candidate_id.clone(),
            counts: value.counts.clone(),
            result: value.result.clone(),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub(crate) struct FixedSemantics {
    pub work_sha256: String,
    pub world_build_calls: Vec<(u64, u64)>,
    pub trials: Vec<SemanticTrial>,
    pub aggregates: Vec<CandidateAggregate>,
}
impl FixedSemantics {
    pub(crate) fn digest(&self) -> Result<String, String> {
        Ok(sha256(canonical(self)?.as_bytes()))
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct WorkerMeasurement {
    pub worker_index: usize,
    pub cpu_id: Option<usize>,
    pub completed_world_indices: Vec<u64>,
    pub elapsed_micros: u128,
}

#[derive(Debug, Serialize)]
pub(crate) struct FixedScheduleResult {
    pub workers: usize,
    pub worker_measurements: Vec<WorkerMeasurement>,
    pub worker_faults: Vec<String>,
    pub worlds: Vec<WorldResult>,
    pub semantics: FixedSemantics,
    pub elapsed_micros: u128,
    pub complete_useful: bool,
    pub strategy_operations: u64,
}

fn validate_order(work: &FrozenWork, workers: usize, order: &[usize]) -> Result<(), String> {
    if workers == 0 || workers > MAX_DIAGNOSTIC_WORKERS {
        return Err("fixed diagnostic worker bound".into());
    }
    if order.len() != work.worlds.len()
        || order.iter().copied().collect::<BTreeSet<_>>() != (0..work.worlds.len()).collect()
    {
        return Err("fixed diagnostic requires exactly one visit to every frozen world".into());
    }
    Ok(())
}

/// One allowed logical CPU per physical core, sorted by logical CPU ID.
/// Only the host diagnostic adapter reads Linux topology; core is untouched.
fn physical_cpu_ids() -> Result<Vec<usize>, String> {
    let allowed = core_affinity::get_core_ids().ok_or("allowed CPU discovery unavailable")?;
    let mut physical = BTreeMap::new();
    for cpu in allowed {
        let read = |field| {
            fs::read_to_string(format!(
                "/sys/devices/system/cpu/cpu{}/topology/{field}",
                cpu.id
            ))
            .map(|s| s.trim().to_owned())
            .map_err(|e| e.to_string())
        };
        let key = (read("physical_package_id")?, read("core_id")?);
        physical
            .entry(key)
            .and_modify(|id: &mut usize| *id = (*id).min(cpu.id))
            .or_insert(cpu.id);
    }
    let mut ids = physical.into_values().collect::<Vec<_>>();
    ids.sort_unstable();
    Ok(ids)
}

fn validate_affinity(ids: &[usize], workers: usize) -> Result<(), String> {
    if ids.len() != workers || ids.iter().copied().collect::<BTreeSet<_>>().len() != workers {
        return Err("one distinct physical CPU per diagnostic worker required".into());
    }
    // Validate the supplied IDs themselves, rather than restricting to only the
    // lowest sibling chosen by physical_cpu_ids(). Any allowed distinct sibling
    // is a valid pinned CPU, but two siblings of the same core are not.
    let allowed = core_affinity::get_core_ids().ok_or("allowed CPU discovery unavailable")?;
    let mut physical = BTreeSet::new();
    for id in ids {
        if !allowed.iter().any(|cpu| cpu.id == *id) {
            return Err("diagnostic CPU outside allowed affinity mask".into());
        }
        let read = |field| {
            fs::read_to_string(format!("/sys/devices/system/cpu/cpu{id}/topology/{field}"))
                .map(|s| s.trim().to_owned())
                .map_err(|e| e.to_string())
        };
        if !physical.insert((read("physical_package_id")?, read("core_id")?)) {
            return Err("diagnostic CPU IDs alias a physical core".into());
        }
    }
    Ok(())
}

pub(crate) fn execute(
    catalog: &Catalog,
    capture: &SearchCapture,
    work: &FrozenWork,
    workers: usize,
    cpu_ids: Option<&[usize]>,
    world_order: &[usize],
) -> Result<FixedScheduleResult, String> {
    validate_order(work, workers, world_order)?;
    if work.context_id != capture.context_id() || work.horizon != HORIZON {
        return Err("fixed work belongs to another capture or horizon".into());
    }
    if let Some(ids) = cpu_ids {
        validate_affinity(ids, workers)?;
    }
    let work_sha256 = work.digest()?;
    let started = Instant::now();
    let control = no_deadline();
    let mut worlds = Vec::with_capacity(work.worlds.len());
    let mut measurements = vec![];
    let mut faults = vec![];
    thread::scope(|scope| {
        let mut handles = vec![];
        for worker_index in 0..workers {
            let control = &control;
            let name = format!("fixed-search-{worker_index}");
            // Static round-robin partition, not a deadline-dependent queue. The
            // same world is built once and runs every candidate in frozen order.
            let spawned = thread::Builder::new()
                .name(name)
                .spawn_scoped(scope, move || {
                    let worker_started = Instant::now();
                    let cpu_id = cpu_ids.map(|ids| ids[worker_index]);
                    if let Some(id) = cpu_id {
                        // Same documented safe core_affinity API as production.
                        if !core_affinity::set_for_current(core_affinity::CoreId { id }) {
                            return Err(format!("diagnostic affinity failed for CPU{id}"));
                        }
                        let actual = core_affinity::get_core_ids()
                            .ok_or("pinned CPU readback unavailable")?;
                        if actual.len() != 1 || actual[0].id != id {
                            return Err("diagnostic pinned CPU readback differs".into());
                        }
                    }
                    let mut local = vec![];
                    let mut completed = vec![];
                    for &index in world_order.iter().skip(worker_index).step_by(workers) {
                        let result = world_job(
                            catalog,
                            capture,
                            work.worlds[index].proposal.clone(),
                            &work.candidate_policy.candidates,
                            control,
                            false,
                        );
                        completed.push(result.world_index);
                        local.push(result);
                    }
                    Ok((
                        local,
                        WorkerMeasurement {
                            worker_index,
                            cpu_id,
                            completed_world_indices: completed,
                            elapsed_micros: worker_started.elapsed().as_micros(),
                        },
                    ))
                });
            match spawned {
                Ok(handle) => handles.push((worker_index, handle)),
                Err(error) => faults.push(format!("worker{worker_index} spawn: {error}")),
            }
        }
        // Join every started thread, including on partial spawn failure or panic.
        // There are no channels and no sender/disconnect deadlock on cleanup.
        for (worker_index, handle) in handles {
            match handle.join() {
                Ok(Ok((local, measurement))) => {
                    worlds.extend(local);
                    measurements.push(measurement);
                }
                Ok(Err(error)) => faults.push(format!("worker{worker_index}: {error}")),
                Err(_) => faults.push(format!("worker{worker_index} panicked")),
            }
        }
    });
    let mut seen = worlds
        .iter()
        .map(|world| world.world_index)
        .collect::<BTreeSet<_>>();
    // Failures never erase requested work. An unavailable worker result cannot
    // honestly claim NotStarted: it might have executed before the panic.
    for world in &work.worlds {
        if seen.insert(world.world_index) {
            worlds.push(WorldResult {
                world_index: world.world_index,
                assignment_digest: world.assignment_digest.clone(),
                build_calls: 0,
                build_micros: 0,
                base_disposal_micros: 0,
                trials: work
                    .candidate_policy
                    .candidates
                    .iter()
                    .enumerate()
                    .map(|(candidate_index, candidate)| TrialResult {
                        world_index: world.world_index,
                        assignment_digest: world.assignment_digest.clone(),
                        candidate_index,
                        candidate_id: candidate.candidate_id.clone(),
                        counts: TrialCounts::default(),
                        timings: TrialTimings::default(),
                        result: TrialOutcome::PolicyFailure {
                            reason:
                                "diagnostic worker result unavailable; execution counts unknown"
                                    .into(),
                        },
                    })
                    .collect(),
            });
        }
    }
    worlds.sort_by_key(|world| world.world_index);
    measurements.sort_by_key(|worker| worker.worker_index);
    faults.sort();
    let trials = worlds
        .iter()
        .flat_map(|world| world.trials.iter().cloned())
        .collect::<Vec<_>>();
    let semantic = FixedSemantics {
        work_sha256,
        world_build_calls: worlds
            .iter()
            .map(|world| (world.world_index, world.build_calls))
            .collect(),
        trials: trials.iter().map(SemanticTrial::from).collect(),
        aggregates: aggregates(&work.candidate_policy.candidates, &trials),
    };
    let complete_useful = faults.is_empty()
        && trials.len() == work.worlds.len() * work.candidate_policy.candidates.len()
        && trials.iter().all(|trial| trial.result.useful())
        && semantic.aggregates.iter().all(|row| row.minimum_met);
    Ok(FixedScheduleResult {
        workers,
        worker_measurements: measurements,
        worker_faults: faults,
        worlds,
        semantics: semantic,
        elapsed_micros: started.elapsed().as_micros(),
        complete_useful,
        strategy_operations: control.operations.load(Ordering::SeqCst),
    })
}

fn public_seed(root: usize) -> [u8; 32] {
    let hash = sha256(format!("{WORKLOAD}/job/{root}").as_bytes());
    std::array::from_fn(|i| u8::from_str_radix(&hash[2 * i..2 * i + 2], 16).expect("SHA256 hex"))
}

fn fixture(root: usize) -> (Catalog, SearchCapture) {
    let catalog = mtg_compiler::catalog::embedded_catalog("match").unwrap();
    let root = super::super::search_roots::prepare(&catalog, root).unwrap();
    let capture = super::super::search_roots::capture(&root.game, root.case.viewer).unwrap();
    drop(root);
    (catalog, capture)
}

#[test]
fn fixed_work_rejects_incomplete_duplicate_out_of_range_and_zero_worker_schedules() {
    let (catalog, capture) = fixture(0);
    let work = freeze(&catalog, &capture, public_seed(0), MINIMUM_WORLDS).unwrap();
    for (workers, order) in [
        (0, (0..8).collect::<Vec<_>>()),
        (1, vec![0, 1, 2]),
        (1, vec![0, 1, 2, 3, 4, 5, 6, 6]),
        (1, vec![0, 1, 2, 3, 4, 5, 6, 8]),
    ] {
        assert!(validate_order(&work, workers, &order).is_err());
    }
    assert!(freeze(&catalog, &capture, public_seed(0), 7).is_err());
    assert!(freeze(&catalog, &capture, public_seed(0), 33).is_err());
    assert!(validate_affinity(&[0, 0], 2).is_err());
}

#[test]
fn fixed_work_one_two_four_schedules_preserve_every_trial_and_full_state() {
    let (catalog, capture) = fixture(0);
    let work = freeze(&catalog, &capture, public_seed(0), MINIMUM_WORLDS).unwrap();
    assert_eq!(work.worlds().len(), MINIMUM_WORLDS);
    assert_eq!(work.candidates().candidates.len(), 3);
    let before = canonical(&work).unwrap();
    let serial = execute(
        &catalog,
        &capture,
        &work,
        1,
        None,
        &(0..8).collect::<Vec<_>>(),
    )
    .unwrap();
    assert!(serial.complete_useful);
    assert_eq!(serial.semantics.trials.len(), 24);
    for workers in [2, 4] {
        let parallel = execute(
            &catalog,
            &capture,
            &work,
            workers,
            None,
            &[7, 0, 6, 1, 5, 2, 4, 3],
        )
        .unwrap();
        assert!(parallel.complete_useful);
        assert_eq!(parallel.semantics, serial.semantics);
        assert_eq!(
            parallel.semantics.digest().unwrap(),
            serial.semantics.digest().unwrap()
        );
        assert_eq!(canonical(&work).unwrap(), before);
        assert_eq!(parallel.worker_measurements.len(), workers);
    }
    assert_eq!(HORIZON, 256);
}

// Frozen pre-generic P8 trial body, correctness oracle only.
// Original search_jobs.rs SHA256 fe36477a0921b139db60157c726771d1a991a95bbb90e6508e61f6abca32cb17
fn original_run_trial_oracle(
    base: &SampledGame,
    map: &RootHandleMap,
    candidate: &Candidate,
    viewer: PlayerId,
    catalog: &Catalog,
    control: &Control,
) -> (TrialCounts, TrialOutcome, TrialTimings) {
    let inclusive_started = Instant::now();
    let mut timings = TrialTimings::default();
    let mut counts = TrialCounts::default();
    let fork_started = Instant::now();
    let mut game = base.fork();
    timings.fork_micros = fork_started.elapsed().as_micros();
    let beginning = game.rule_boundary();
    let mut root_complete = false;
    let result = (|| -> Result<TrialOutcome, String> {
        let mut plan = PlanSteps::new(candidate, map)?;
        loop {
            control.check()?;
            counts.advance()?;
            let advance_started = Instant::now();
            let next = game.advance(Budget::new(QUANTUM));
            timings.advance_micros += advance_started.elapsed().as_micros();
            counts.rule_steps = game
                .rule_boundary()
                .checked_sub(beginning)
                .ok_or("sample boundary moved backwards")?;
            match next {
                Advance::Suspended { .. } => continue,
                Advance::Fault(reason) => return Ok(TrialOutcome::EngineFault { reason }),
                Advance::Finished { winner } => {
                    let view = observation(&game, viewer, control, &mut counts, &mut timings)?;
                    let eval_started = Instant::now();
                    let values = evaluations(&view, Some(Some(winner)));
                    timings.evaluation_micros += eval_started.elapsed().as_micros();
                    let digest_started = Instant::now();
                    let digest = game.state_digest();
                    timings.full_state_digest_micros += digest_started.elapsed().as_micros();
                    return Ok(TrialOutcome::Terminal {
                        winner: Some(winner),
                        evaluations: values,
                        state_sha256: digest?,
                    });
                }
                Advance::Draw => {
                    let view = observation(&game, viewer, control, &mut counts, &mut timings)?;
                    let eval_started = Instant::now();
                    let values = evaluations(&view, Some(None));
                    timings.evaluation_micros += eval_started.elapsed().as_micros();
                    let digest_started = Instant::now();
                    let digest = game.state_digest();
                    timings.full_state_digest_micros += digest_started.elapsed().as_micros();
                    return Ok(TrialOutcome::Terminal {
                        winner: None,
                        evaluations: values,
                        state_sha256: digest?,
                    });
                }
                Advance::AwaitingDecision(request) => {
                    if counts.accepted_inputs >= HORIZON {
                        let view = observation(&game, viewer, control, &mut counts, &mut timings)?;
                        let eval_started = Instant::now();
                        let values = evaluations(&view, None);
                        timings.evaluation_micros += eval_started.elapsed().as_micros();
                        let digest_started = Instant::now();
                        let digest = game.state_digest();
                        timings.full_state_digest_micros += digest_started.elapsed().as_micros();
                        return Ok(TrialOutcome::Horizon {
                            evaluations: values,
                            state_sha256: digest?,
                        });
                    }
                    // Trusted host uses only chooser identity from the raw
                    // request; the policy receives the scoped observation.
                    let view =
                        observation(&game, request.player(), control, &mut counts, &mut timings)?;
                    let policy_started = Instant::now();
                    let chosen = (|| -> Result<PlayerResponse, String> {
                        if let Some(response) = plan.next(&view)? {
                            Ok(response)
                        } else {
                            root_complete = true;
                            search_policy::choose(catalog, &view, &mut || control.check())
                        }
                    })();
                    timings.policy_micros += policy_started.elapsed().as_micros();
                    let response = chosen?;
                    respond(
                        &mut game,
                        &view,
                        response,
                        control,
                        &mut counts,
                        &mut timings,
                    )?;
                }
            }
        }
    })();
    let outcome = match result {
        Ok(outcome) => outcome,
        Err(reason) if reason.starts_with("resource:") => {
            TrialOutcome::ResourceTruncated { reason }
        }
        Err(reason) if !root_complete => TrialOutcome::InfeasibleCandidate { reason },
        Err(reason) => TrialOutcome::PolicyFailure { reason },
    };
    let disposal_started = Instant::now();
    drop(game);
    timings.disposal_micros = disposal_started.elapsed().as_micros();
    timings.inclusive_trial_micros = inclusive_started.elapsed().as_micros();
    (counts, outcome, timings)
}

#[test]
fn fixed_work_generic_sampled_rollout_matches_original_production_loop() {
    let (catalog, capture) = fixture(0);
    let work = freeze(&catalog, &capture, public_seed(0), MINIMUM_WORLDS).unwrap();
    let control = no_deadline();
    let mut calls = 0;
    let (base, map) = build_world(
        &catalog,
        &capture,
        work.worlds()[0].proposal.clone(),
        &control,
        &mut calls,
    )
    .unwrap();
    for candidate in &work.candidates().candidates {
        let old = original_run_trial_oracle(
            &base,
            &map,
            candidate,
            capture.data().context.observation.view.viewer,
            &catalog,
            &control,
        );
        let new = run_trial(
            &base,
            &map,
            candidate,
            capture.data().context.observation.view.viewer,
            &catalog,
            &control,
        );
        assert_eq!(old.0, new.0);
        assert_eq!(old.1, new.1);
        assert!(old.1.useful());
    }
}

#[test]
fn fixed_work_real_reference_preserves_actual_origin_and_single_world_identity() {
    let catalog = mtg_compiler::catalog::embedded_catalog("match").unwrap();
    let fixture = super::super::search_roots::prepare(&catalog, 0).unwrap();
    let capture = super::super::search_roots::capture(&fixture.game, fixture.case.viewer).unwrap();
    let work = freeze(&catalog, &capture, public_seed(0), MINIMUM_WORLDS).unwrap();
    let before_sha = fixture.game.state_digest().unwrap();
    let before_snapshot = fixture.game.snapshot().unwrap();
    let before_journal = canonical(&fixture.game.journal()).unwrap();
    let trials = evaluate_real(&catalog, &fixture.game, &capture, work.candidates()).unwrap();
    assert_eq!(
        trials.len(),
        MINIMUM_WORLDS * work.candidates().candidates.len()
    );
    assert_eq!(trials.len(), 24);
    assert_eq!(
        trials
            .iter()
            .map(|trial| &trial.assignment_digest)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([&before_sha])
    );
    for candidate in 0..work.candidates().candidates.len() {
        let repeated = trials
            .iter()
            .filter(|trial| trial.candidate_index == candidate)
            .collect::<Vec<_>>();
        assert_eq!(repeated.len(), 8);
        assert_eq!(
            repeated
                .iter()
                .map(|trial| trial.world_index)
                .collect::<Vec<_>>(),
            (0..8).collect::<Vec<_>>()
        );
        for trial in &repeated {
            assert_eq!(trial.counts, repeated[0].counts);
            assert_eq!(trial.result, repeated[0].result);
            assert!(trial.result.useful());
            assert!(trial.counts.accepted_inputs > 0 && trial.counts.accepted_inputs <= HORIZON);
        }
    }
    let sums = aggregates(&work.candidates().candidates, &trials);
    assert!(
        sums.iter()
            .all(|row| row.useful_distinct_worlds == 1 && !row.minimum_met)
    );
    assert_eq!(fixture.game.state_digest().unwrap(), before_sha);
    assert_eq!(fixture.game.snapshot().unwrap(), before_snapshot);
    assert_eq!(canonical(&fixture.game.journal()).unwrap(), before_journal);
    let mut missing = work.candidates().clone();
    missing.candidates.pop();
    assert!(evaluate_real(&catalog, &fixture.game, &capture, &missing).is_err());
    let other = super::super::search_roots::prepare(&catalog, 1).unwrap();
    assert!(evaluate_real(&catalog, &other.game, &capture, work.candidates()).is_err());
    assert_eq!(HORIZON, 256);
}

/// One worker-count row per process. The external host owns the outer watchdog,
/// RSS/cgroup cap, CPU user/sys measurement and sequential 1/2/4/all-core runs.
/// Kernel VmHWM below includes fixture, serial oracle, warmup and timed samples;
/// it is never presented as a resettable per-sample heap/allocation measurement.
#[test]
#[ignore = "explicit host diagnostic; never a product latency acceptance test"]
fn fixed_work_diagnostic_row() {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let output = std::env::var("MTG_FIXED_SEARCH_OUTPUT").expect("explicit new output path");
    let output = std::path::PathBuf::from(output);
    assert!(output.is_absolute() && !output.exists());
    assert_eq!(
        fs::metadata(output.parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    let root: usize = std::env::var("MTG_FIXED_SEARCH_ROOT")
        .unwrap_or_else(|_| "0".into())
        .parse()
        .unwrap();
    let cpu_ids: Vec<usize> = serde_json::from_str(
        &std::env::var("MTG_FIXED_SEARCH_CPUS").expect("explicit frozen physical CPU IDs JSON"),
    )
    .unwrap();
    validate_affinity(&cpu_ids, cpu_ids.len()).unwrap();
    let available = physical_cpu_ids().unwrap();
    assert!(
        [1, 2, 4, available.len()].contains(&cpu_ids.len()),
        "only 1/2/4/all available physical cores are diagnostic rows"
    );
    let world_count: usize = std::env::var("MTG_FIXED_SEARCH_WORLDS")
        .unwrap_or_else(|_| "8".into())
        .parse()
        .unwrap();
    let prepared = Instant::now();
    let (catalog, capture) = fixture(root);
    let work = freeze(&catalog, &capture, public_seed(root), world_count).unwrap();
    let preparation_micros = prepared.elapsed().as_micros();
    let order = (0..world_count).collect::<Vec<_>>();
    let (_, before_peak) = memory().unwrap();
    // Independent schedule oracle, with exact same frozen proposals and horizon.
    let reference = execute(&catalog, &capture, &work, 1, Some(&cpu_ids[..1]), &order).unwrap();
    let reference_digest = reference.semantics.digest().unwrap();
    let mut report = serde_json::json!({"schema":"foundations-fixed-search-row-v1","status":"running",
        "classification":"fixed-work diagnostic, not the deadline-limited product benchmark",
        "engine_build_id":mtg_core::BUILD_ID,"root_index":root,"frozen_work":work,
        "frozen_work_sha256":work.digest().unwrap(),"horizon":HORIZON,"workers":cpu_ids.len(),
        "worker_cpu_ids":cpu_ids,"available_physical_cpu_ids":available,"preparation_micros":preparation_micros,
        "reference_semantics":reference.semantics,"reference_semantics_sha256":reference_digest,
        "reference_complete_useful":reference.complete_useful,"warmup":null,"samples":[],
        "kernel_peak_before_bytes":before_peak,
        "timing_scope":"worker pool creation, pinned world construction, every trial/query/rules/full State hash/disposal, joins and sorted aggregation; excludes producer and parity audit",
        "kernel_peak_scope":"whole process including producer, serial reference, warmup and all five samples",
        "outer_limits":"must be enforced and recorded by the external host adapter; none relaxed by this diagnostic"});
    let save = |value: &serde_json::Value, first: bool| {
        let mut options = fs::OpenOptions::new();
        options.write(true).mode(0o600);
        if first {
            options.create_new(true);
        } else {
            options.truncate(true);
        }
        let mut file = options.open(&output).unwrap();
        file.write_all(canonical(value).unwrap().as_bytes())
            .unwrap();
        file.write_all(b"\n").unwrap();
        file.sync_all().unwrap();
    };
    save(&report, true);
    let mut valid = reference.complete_useful;
    for repetition in 0..6 {
        if !valid {
            break;
        }
        let result = execute(
            &catalog,
            &capture,
            &work,
            cpu_ids.len(),
            Some(&cpu_ids),
            &order,
        );
        match result {
            Ok(result) => {
                let equal = result.semantics == reference.semantics;
                valid &= equal && result.complete_useful;
                let (_, peak) = memory().unwrap();
                let sample = serde_json::json!({"repetition":repetition,"warmup":repetition==0,
                    "semantic_equality":equal,"kernel_peak_bytes":peak,"result":result});
                if repetition == 0 {
                    report["warmup"] = sample;
                } else {
                    report["samples"].as_array_mut().unwrap().push(sample);
                }
            }
            Err(error) => {
                valid = false;
                report["failure"] = serde_json::json!({"repetition":repetition,"error":error});
                break;
            }
        }
        save(&report, false);
        if !valid {
            break;
        }
    }
    report["status"] = serde_json::json!(if valid
        && report["samples"].as_array().unwrap().len() == 5
    {
        "fixed_row_verified"
    } else {
        "fixed_row_failed"
    });
    save(&report, false);
    assert!(valid, "diagnostic failed; raw row preserved");
    // No speedup from one row. Cross-row comparison must match full frozen work,
    // every reference/result/count/State SHA before dividing median durations.
}
