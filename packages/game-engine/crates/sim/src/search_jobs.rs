//! Bounded host search lifecycle. Rules remain in the opaque sampled core;
//! candidate generation and every rollout policy consume permitted views only.
use super::{
    search_candidates::{self, ActionPlan, Candidate, CandidateSet, TargetPlan},
    search_policy, search_sampler,
};
use mtg_contracts::{Catalog, Quantity};
use mtg_core::{
    Advance, Budget, Observation, PlayerId, PlayerRequest, PlayerResponse, PlayerStatus,
    QueryResult, QueryValue, RootHandleMap, SampleBuildOutcome, SampledGame, SearchCapture,
    WorldProposal, Zone,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeSet, VecDeque},
    fs,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

pub(super) const WORKLOAD: &str = "commander-open-search-v1";
pub(super) const SCHEDULING_POLICY: &str = "first-eight-all-candidates-then-extras-soft2500-v1";
pub(super) const DESIRED_WORLDS: usize = 32;
pub(super) const MINIMUM_WORLDS: usize = 8;
pub(super) const HORIZON: u64 = 256;
const WORKERS: usize = 4;
const HARD_MS: u64 = 5_000;
const EXTRA_ADMISSION_MS: u64 = 2_500;
const MEMORY_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const QUANTUM: u64 = 64;
const CORE_CALL_LIMIT: u64 = 100_000;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SearchRequest {
    pub request_version: u32,
    pub root_index: u32,
    pub public_job_seed: [u8; 32],
    pub worker_cpu_ids: [usize; WORKERS],
}
#[derive(Clone, Default)]
pub(super) struct Cancellation(Arc<AtomicBool>);
impl Cancellation {
    pub(super) fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
}
struct Control {
    started: Instant,
    cancellation: Cancellation,
    stopped: AtomicBool,
    reason: Mutex<Option<String>>,
    operations: AtomicU64,
    enforce_deadline: bool,
}
impl Control {
    fn stop(&self, reason: &str) {
        self.cancellation.cancel();
        self.stopped.store(true, Ordering::SeqCst);
        if let Ok(mut stored) = self.reason.lock()
            && stored.is_none()
        {
            *stored = Some(reason.into());
        }
    }
    fn check(&self) -> Result<(), String> {
        self.operations.fetch_add(1, Ordering::Relaxed);
        if self.cancellation.0.load(Ordering::SeqCst) {
            self.stop("cancelled");
        }
        if self.enforce_deadline && self.started.elapsed() >= Duration::from_millis(HARD_MS) {
            self.stop("deadline");
        }
        if self.stopped.load(Ordering::SeqCst) {
            let reason = self
                .reason
                .lock()
                .ok()
                .and_then(|r| r.clone())
                .unwrap_or_else(|| "cancelled".into());
            return Err(format!("resource:{reason}"));
        }
        Ok(())
    }
    fn extras_open(&self) -> bool {
        self.started.elapsed() < Duration::from_millis(EXTRA_ADMISSION_MS)
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TrialCounts {
    pub accepted_inputs: u64,
    pub advance_calls: u64,
    pub query_calls: u64,
    pub rule_steps: u64,
}
impl TrialCounts {
    fn query(&mut self) -> Result<(), String> {
        self.bound()?;
        self.query_calls = self
            .query_calls
            .checked_add(1)
            .ok_or("resource:counter_overflow")?;
        Ok(())
    }
    fn advance(&mut self) -> Result<(), String> {
        self.bound()?;
        self.advance_calls = self
            .advance_calls
            .checked_add(1)
            .ok_or("resource:counter_overflow")?;
        Ok(())
    }
    fn bound(&self) -> Result<(), String> {
        if self
            .advance_calls
            .checked_add(self.query_calls)
            .is_none_or(|n| n >= CORE_CALL_LIMIT)
        {
            Err("resource:trial_core_call_limit".into())
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TrialTimings {
    pub fork_micros: u128,
    pub query_micros: u128,
    pub policy_micros: u128,
    pub apply_micros: u128,
    pub advance_micros: u128,
    pub evaluation_micros: u128,
    pub full_state_digest_micros: u128,
    pub disposal_micros: u128,
    /// Inclusive duration overlaps the component fields and concurrent workers.
    pub inclusive_trial_micros: u128,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WorldTimings {
    pub world_index: u64,
    pub build_calls: u64,
    pub build_micros: u128,
    pub base_disposal_micros: u128,
}
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RootTimings {
    pub capture_and_caller_handling_micros: u128,
    pub candidate_generation_micros: u128,
    pub sampling_micros: u128,
    /// Inclusive worker/pool lifecycle; overlaps per-world and per-trial times.
    pub pool_lifecycle_micros: u128,
    pub aggregation_micros: u128,
    pub capture_disposal_micros: u128,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PlayerEvaluation {
    pub player: PlayerId,
    pub public_status: PlayerStatus,
    pub utility: Quantity,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum TrialOutcome {
    Terminal {
        winner: Option<PlayerId>,
        evaluations: Vec<PlayerEvaluation>,
        state_sha256: String,
    },
    Horizon {
        evaluations: Vec<PlayerEvaluation>,
        state_sha256: String,
    },
    InfeasibleCandidate {
        reason: String,
    },
    InvalidWorld {
        reason: String,
    },
    PolicyFailure {
        reason: String,
    },
    EngineFault {
        reason: String,
    },
    ResourceTruncated {
        reason: String,
    },
    NotStarted {
        reason: String,
    },
}
impl TrialOutcome {
    fn useful(&self) -> bool {
        matches!(self, Self::Terminal { .. } | Self::Horizon { .. })
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TrialResult {
    pub world_index: u64,
    pub assignment_digest: String,
    pub candidate_index: usize,
    pub candidate_id: String,
    pub counts: TrialCounts,
    pub timings: TrialTimings,
    pub result: TrialOutcome,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WorldResult {
    pub world_index: u64,
    pub assignment_digest: String,
    pub build_calls: u64,
    pub build_micros: u128,
    pub base_disposal_micros: u128,
    pub trials: Vec<TrialResult>,
}
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CandidateAggregate {
    pub candidate_index: usize,
    pub candidate_id: String,
    pub useful_distinct_worlds: usize,
    pub minimum_met: bool,
    pub terminal_games: usize,
    pub horizon_trials: usize,
    pub winner_counts: [u64; 4],
    /// Exact utility numerator, divided by useful_distinct_worlds. This strategy
    /// vector is not a zero-sum claim or a rules-defined horizon winner.
    pub utility_sums: Vec<Quantity>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WorldSpec {
    pub world_index: u64,
    pub sampling_attempt: u64,
    pub assignment_digest: String,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SearchReport {
    pub report_version: u32,
    pub workload: String,
    pub context_id: String,
    pub request: SearchRequest,
    pub scheduling_policy: String,
    pub candidate_policy: CandidateSet,
    pub sampler_rng: String,
    pub rollout_policy: String,
    pub desired_worlds: usize,
    pub produced_distinct_worlds: usize,
    pub sampling_attempts: u64,
    pub world_specs: Vec<WorldSpec>,
    pub sampling_shortfall: Option<String>,
    pub requested_pairs: usize,
    pub completed_world_jobs: usize,
    pub trials: Vec<TrialResult>,
    pub aggregates: Vec<CandidateAggregate>,
    pub selection_policy: String,
    pub selection_aggregates: Vec<CandidateAggregate>,
    pub chosen_candidate_index: Option<usize>,
    pub chosen_candidate_id: Option<String>,
    pub chosen_plan: Option<ActionPlan>,
    pub components: RootTimings,
    pub world_components: Vec<WorldTimings>,
    pub actual_worker_cpu_ids: Vec<usize>,
    pub worker_faults: Vec<String>,
    pub elapsed_micros: u128,
    pub maximum_process_rss_bytes: u64,
    pub strategy_operations: u64,
    pub minimum_met: bool,
    pub desired_complete: bool,
    pub hard_latency_met: bool,
    pub p95_root_target_met: bool,
    pub memory_met: bool,
    pub stop_reason: Option<String>,
}

fn complete_query(
    game: &SampledGame,
    control: &Control,
    counts: &mut TrialCounts,
    timings: &mut TrialTimings,
    call: impl FnOnce() -> QueryResult,
) -> Result<QueryValue, String> {
    let started = Instant::now();
    let result = (|| {
        control.check()?;
        counts.query()?;
        let mut result = call();
        loop {
            match result {
                QueryResult::Complete(value) => return Ok(value),
                QueryResult::Rejected(reason) => return Err(format!("query_rejected:{reason:?}")),
                QueryResult::Incomplete {
                    work,
                    required_minimum,
                } => {
                    control.check()?;
                    counts.query()?;
                    if required_minimum > QUANTUM {
                        return Err("resource:unsupported_query_quantum".into());
                    }
                    result = game.resume_work(work, Budget::new(QUANTUM));
                }
            }
        }
    })();
    timings.query_micros += started.elapsed().as_micros();
    result
}

fn observation(
    game: &SampledGame,
    viewer: PlayerId,
    control: &Control,
    counts: &mut TrialCounts,
    timings: &mut TrialTimings,
) -> Result<Observation, String> {
    match complete_query(game, control, counts, timings, || {
        game.observe(viewer, Budget::new(QUANTUM))
    })? {
        QueryValue::Observation(value) => Ok(value),
        _ => Err("unexpected observation query result".into()),
    }
}
fn respond(
    game: &mut SampledGame,
    observation: &Observation,
    response: PlayerResponse,
    control: &Control,
    counts: &mut TrialCounts,
    timings: &mut TrialTimings,
) -> Result<(), String> {
    let token = observation
        .decision
        .as_ref()
        .ok_or("missing sampled decision")?
        .token()
        .clone();
    let value = complete_query(game, control, counts, timings, || {
        game.check_player_response(
            observation.view.viewer,
            token,
            response,
            Budget::new(QUANTUM),
        )
    })?;
    let QueryValue::Validated(receipt) = value else {
        return Err("unexpected validation query result".into());
    };
    control.check()?;
    let apply_started = Instant::now();
    let submitted = game.submit_validated(receipt);
    timings.apply_micros += apply_started.elapsed().as_micros();
    submitted?;
    counts.accepted_inputs = counts
        .accepted_inputs
        .checked_add(1)
        .ok_or("resource:counter_overflow")?;
    Ok(())
}
fn evaluations(
    observation: &Observation,
    terminal: Option<Option<PlayerId>>,
) -> Vec<PlayerEvaluation> {
    observation
        .view
        .players
        .iter()
        .map(|player| {
            let utility =
                match terminal {
                    Some(Some(winner)) => Quantity::from(if winner == player.player {
                        1_000_000
                    } else {
                        -1_000_000
                    }),
                    Some(None) => Quantity::zero(),
                    None if player.status != PlayerStatus::Live => Quantity::from(-1_000_000),
                    None => {
                        let mut value = player.life.clone();
                        value.add(&Quantity::from(i64::from(player.hand_count)));
                        for object in observation.view.objects.iter().filter(|o| {
                            o.controller == player.player && o.zone == Zone::Battlefield
                        }) {
                            if let Some(power) = object
                                .characteristics
                                .as_ref()
                                .and_then(|c| c.power.as_ref())
                                .filter(|p| **p > Quantity::zero())
                            {
                                value.add(power);
                            }
                        }
                        value
                    }
                };
            PlayerEvaluation {
                player: player.player,
                public_status: player.status,
                utility,
            }
        })
        .collect()
}
struct PlanSteps {
    initial: PlayerResponse,
    initial_sent: bool,
    mode: Option<u32>,
    targets: TargetPlan,
    mana_sources: VecDeque<mtg_core::ObjectHandle>,
    spending: std::collections::BTreeMap<mtg_contracts::Color, Quantity>,
    invocation: bool,
    complete: bool,
}
impl PlanSteps {
    fn new(candidate: &Candidate, map: &RootHandleMap) -> Result<Self, String> {
        let (mode, targets, mana, spending, invocation) = match &candidate.plan {
            ActionPlan::Cast {
                mode,
                targets,
                mana_sources,
                spending,
                ..
            } => (
                *mode,
                targets.clone(),
                mana_sources.clone(),
                spending.clone(),
                true,
            ),
            ActionPlan::Activate {
                targets,
                mana_sources,
                spending,
                ..
            } => (
                None,
                targets.clone(),
                mana_sources.clone(),
                spending.clone(),
                true,
            ),
            _ => (None, TargetPlan::None, vec![], Default::default(), false),
        };
        Ok(Self {
            initial: map.response(&search_policy::initial_response(&candidate.plan))?,
            initial_sent: false,
            mode,
            targets: targets.mapped(map)?,
            mana_sources: mana
                .iter()
                .map(|h| map.object(h))
                .collect::<Result<_, _>>()?,
            spending,
            invocation,
            complete: false,
        })
    }
    fn next(&mut self, observation: &Observation) -> Result<Option<PlayerResponse>, String> {
        if self.complete {
            return Ok(None);
        }
        let request = observation
            .decision
            .as_ref()
            .ok_or("root staged action lost its decision")?;
        if !self.initial_sent {
            if !matches!(request, PlayerRequest::Priority { .. }) {
                return Err("root action did not start at ordinary priority".into());
            }
            self.initial_sent = true;
            return Ok(Some(self.initial.clone()));
        }
        if !self.invocation {
            self.complete = true;
            return Ok(None);
        }
        let entry = observation
            .view
            .stack
            .first()
            .ok_or("root invocation disappeared before paid completion")?;
        if entry.cast_complete {
            self.complete = true;
            return Ok(None);
        }
        Ok(Some(match request {
            PlayerRequest::Modes { .. } => PlayerResponse::Modes {
                mode: self.mode.ok_or("root plan omitted mandatory mode")?,
            },
            PlayerRequest::Targets { .. }
            | PlayerRequest::PlayerTargets { .. }
            | PlayerRequest::EntityTargets { .. } => {
                search_policy::target_response(&self.targets, request)?
            }
            PlayerRequest::Mana { .. } => match self.mana_sources.pop_front() {
                Some(object) => PlayerResponse::ActivateMana { object },
                None => PlayerResponse::FinishMana { undo_mana: true },
            },
            PlayerRequest::SpendMana { .. } => PlayerResponse::SpendMana {
                spending: self.spending.clone(),
            },
            _ => return Err("unexpected root announcement continuation".into()),
        }))
    }
}
fn run_trial(
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
fn build_world(
    catalog: &Catalog,
    capture: &SearchCapture,
    proposal: WorldProposal,
    control: &Control,
    calls: &mut u64,
) -> Result<(SampledGame, RootHandleMap), String> {
    control.check()?;
    *calls += 1;
    let mut result = SampledGame::build(catalog.clone(), capture, proposal, Budget::new(QUANTUM));
    loop {
        match result {
            SampleBuildOutcome::Complete { game, root_map } => return Ok((game, root_map)),
            SampleBuildOutcome::Rejected(reason) => {
                return Err(format!("world_rejected:{reason:?}"));
            }
            SampleBuildOutcome::Incomplete {
                work,
                required_minimum,
            } => {
                control.check()?;
                if *calls >= CORE_CALL_LIMIT || required_minimum > QUANTUM {
                    return Err("resource:world_build_limit".into());
                }
                *calls += 1;
                result = work.resume(Budget::new(QUANTUM));
            }
        }
    }
}
fn world_job(
    catalog: &Catalog,
    capture: &SearchCapture,
    proposal: WorldProposal,
    candidates: &[Candidate],
    control: &Control,
    extra: bool,
) -> WorldResult {
    let world_index = proposal.world_index;
    let assignment_digest = proposal
        .assignment_digest()
        .unwrap_or_else(|_| "invalid-assignment".into());
    let mut trials = vec![];
    let mut build_calls = 0;
    let mut base_disposal_micros = 0;
    let build_started = Instant::now();
    let built = if extra && !control.extras_open() {
        Err("not_started:soft_extra_admission_stop".into())
    } else {
        build_world(catalog, capture, proposal, control, &mut build_calls)
    };
    let build_micros = build_started.elapsed().as_micros();
    match built {
        Ok((base, map)) => {
            for (candidate_index, candidate) in candidates.iter().enumerate() {
                let (counts, result, timings) = if extra && !control.extras_open() {
                    (
                        TrialCounts::default(),
                        TrialOutcome::NotStarted {
                            reason: "soft_extra_admission_stop".into(),
                        },
                        TrialTimings::default(),
                    )
                } else if let Err(reason) = control.check() {
                    (
                        TrialCounts::default(),
                        TrialOutcome::NotStarted { reason },
                        TrialTimings::default(),
                    )
                } else {
                    run_trial(
                        &base,
                        &map,
                        candidate,
                        capture.data().context.observation.view.viewer,
                        catalog,
                        control,
                    )
                };
                trials.push(TrialResult {
                    world_index,
                    assignment_digest: assignment_digest.clone(),
                    candidate_index,
                    candidate_id: candidate.candidate_id.clone(),
                    counts,
                    timings,
                    result,
                });
            }
            let dispose_started = Instant::now();
            drop(base);
            base_disposal_micros = dispose_started.elapsed().as_micros();
        }
        Err(reason) => {
            for (candidate_index, candidate) in candidates.iter().enumerate() {
                let result =
                    if reason.starts_with("not_started:") || reason.starts_with("resource:") {
                        TrialOutcome::NotStarted {
                            reason: reason.clone(),
                        }
                    } else {
                        TrialOutcome::InvalidWorld {
                            reason: reason.clone(),
                        }
                    };
                trials.push(TrialResult {
                    world_index,
                    assignment_digest: assignment_digest.clone(),
                    candidate_index,
                    candidate_id: candidate.candidate_id.clone(),
                    counts: TrialCounts::default(),
                    timings: TrialTimings::default(),
                    result,
                });
            }
        }
    }
    WorldResult {
        world_index,
        assignment_digest,
        build_calls,
        build_micros,
        base_disposal_micros,
        trials,
    }
}
pub(super) fn memory() -> Result<(u64, u64), String> {
    let status = fs::read_to_string("/proc/self/status")
        .map_err(|e| format!("RSS measurement unavailable: {e}"))?;
    let read = |name: &str| -> Result<u64, String> {
        let value = status
            .lines()
            .find_map(|line| line.strip_prefix(name))
            .and_then(|s| s.split_whitespace().next())
            .ok_or("missing process RSS field")?
            .parse::<u64>()
            .map_err(|e| e.to_string())?;
        value
            .checked_mul(1024)
            .ok_or("RSS byte count overflow".into())
    };
    Ok((read("VmRSS:")?, read("VmHWM:")?))
}
fn validate_cpu_ids(ids: [usize; 4]) -> Result<(), String> {
    let allowed = core_affinity::get_core_ids().ok_or("cannot enumerate allowed CPUs")?;
    let mut physical = BTreeSet::new();
    for id in ids {
        if !allowed.iter().any(|core| core.id == id) {
            return Err(format!("worker CPU{id} is outside allowed affinity mask"));
        }
        let read = |name: &str| {
            fs::read_to_string(format!("/sys/devices/system/cpu/cpu{id}/topology/{name}"))
                .map(|s| s.trim().to_owned())
                .map_err(|e| e.to_string())
        };
        let pair = (read("physical_package_id")?, read("core_id")?);
        if !physical.insert(pair) {
            return Err("worker CPU IDs alias a physical core".into());
        }
    }
    Ok(())
}
fn aggregates(candidates: &[Candidate], trials: &[TrialResult]) -> Vec<CandidateAggregate> {
    candidates
        .iter()
        .enumerate()
        .map(|(candidate_index, candidate)| {
            let selected = trials
                .iter()
                .filter(|trial| trial.candidate_index == candidate_index)
                .collect::<Vec<_>>();
            let useful = selected
                .iter()
                .filter(|trial| trial.result.useful())
                .map(|trial| trial.assignment_digest.as_str())
                .collect::<BTreeSet<_>>()
                .len();
            let mut aggregate = CandidateAggregate {
                candidate_index,
                candidate_id: candidate.candidate_id.clone(),
                useful_distinct_worlds: useful,
                minimum_met: useful >= MINIMUM_WORLDS,
                terminal_games: 0,
                horizon_trials: 0,
                winner_counts: [0; 4],
                utility_sums: vec![Quantity::zero(); 4],
            };
            for trial in selected {
                let values = match &trial.result {
                    TrialOutcome::Terminal {
                        winner,
                        evaluations,
                        ..
                    } => {
                        aggregate.terminal_games += 1;
                        if let Some(player) = winner {
                            aggregate.winner_counts[usize::from(player.0)] += 1;
                        }
                        Some(evaluations)
                    }
                    TrialOutcome::Horizon { evaluations, .. } => {
                        aggregate.horizon_trials += 1;
                        Some(evaluations)
                    }
                    _ => None,
                };
                if let Some(values) = values {
                    for value in values {
                        aggregate.utility_sums[usize::from(value.player.0)].add(&value.utility);
                    }
                }
            }
            aggregate
        })
        .collect()
}

fn select_candidate(aggregates: &[CandidateAggregate], viewer: usize) -> Option<usize> {
    if aggregates.is_empty()
        || aggregates.iter().any(|a| {
            !a.minimum_met
                || a.useful_distinct_worlds != MINIMUM_WORLDS
                || a.utility_sums.len() != 4
        })
    {
        return None;
    }
    let mut best = 0usize;
    for index in 1..aggregates.len() {
        if aggregates[index].utility_sums[viewer] > aggregates[best].utility_sums[viewer] {
            best = index;
        }
    }
    Some(aggregates[best].candidate_index)
}

pub(super) fn run(
    catalog: &Catalog,
    capture: SearchCapture,
    request: SearchRequest,
    started: Instant,
) -> Result<SearchReport, String> {
    run_cancellable(catalog, capture, request, started, Cancellation::default())
}
pub(super) fn run_cancellable(
    catalog: &Catalog,
    capture: SearchCapture,
    request: SearchRequest,
    started: Instant,
    cancellation: Cancellation,
) -> Result<SearchReport, String> {
    if request.request_version != 1 || request.root_index >= 12 {
        return Err("invalid frozen search request".into());
    }
    let mut components = RootTimings {
        capture_and_caller_handling_micros: started.elapsed().as_micros(),
        ..RootTimings::default()
    };
    validate_cpu_ids(request.worker_cpu_ids)?;
    let (_, initial_peak) = memory()?;
    let control = Arc::new(Control {
        started,
        cancellation,
        stopped: AtomicBool::new(false),
        reason: Mutex::new(None),
        operations: AtomicU64::new(0),
        enforce_deadline: true,
    });
    if initial_peak > MEMORY_BYTES {
        control.stop("memory_limit")
    }
    let candidate_started = Instant::now();
    let candidates =
        search_candidates::generate(catalog, &capture.data().context.observation, &mut || {
            control.check()
        })?;
    components.candidate_generation_micros = candidate_started.elapsed().as_micros();
    let sampling_started = Instant::now();
    let mut proposals = vec![];
    let mut world_specs = vec![];
    let mut assignments = BTreeSet::new();
    let mut attempts = 0u64;
    let mut shortfall = None;
    'worlds: for index in 0..DESIRED_WORLDS {
        for attempt in 0..search_sampler::MAX_DISTINCT_ATTEMPTS {
            attempts += 1;
            let proposal = match search_sampler::propose(
                &capture,
                &request.public_job_seed,
                index as u64,
                attempt,
                &mut || control.check(),
            ) {
                Ok(proposal) => proposal,
                Err(reason) => {
                    shortfall = Some(reason);
                    break 'worlds;
                }
            };
            let assignment_digest = proposal.assignment_digest()?;
            if assignments.insert(assignment_digest.clone()) {
                world_specs.push(WorldSpec {
                    world_index: index as u64,
                    sampling_attempt: attempt,
                    assignment_digest,
                });
                proposals.push(proposal);
                break;
            }
            if attempt + 1 == search_sampler::MAX_DISTINCT_ATTEMPTS {
                shortfall = Some(
                    "distinct assignment retry bound reached; support cardinality not established"
                        .into(),
                );
                break 'worlds;
            }
        }
    }
    components.sampling_micros = sampling_started.elapsed().as_micros();
    let produced = proposals.len();
    let pool_started = Instant::now();
    let pinned = Mutex::new(Vec::new());
    let mut worlds = vec![];
    let mut worker_faults = vec![];
    let monitor_done = AtomicBool::new(false);
    thread::scope(|scope| {
        let monitor_control = control.clone();
        let done = &monitor_done;
        let monitor = scope.spawn(move || {
            while !done.load(Ordering::SeqCst) {
                match memory() {
                    Ok((rss, peak)) if rss > MEMORY_BYTES || peak > MEMORY_BYTES => {
                        monitor_control.stop("memory_limit")
                    }
                    Err(_) => monitor_control.stop("rss_unavailable"),
                    _ => {}
                }
                thread::sleep(Duration::from_millis(5));
            }
        });
        let (send, receive) = mpsc::sync_channel::<(WorldProposal, bool)>(DESIRED_WORLDS);
        let receive = Arc::new(Mutex::new(receive));
        let (results, outcomes) =
            mpsc::sync_channel::<Result<WorldResult, String>>(DESIRED_WORLDS + WORKERS);
        let mut workers = vec![];
        for cpu_id in request.worker_cpu_ids {
            let queue = receive.clone();
            let results = results.clone();
            let control = control.clone();
            let candidates = &candidates.candidates;
            let capture = &capture;
            let pinned = &pinned;
            workers.push(scope.spawn(move || {
                // Official safe API: https://docs.rs/core_affinity/0.8.3/core_affinity/
                // The parent/root manifest supplies four verified physical cores.
                if !core_affinity::set_for_current(core_affinity::CoreId { id: cpu_id }) {
                    control.stop("worker_affinity_failed");
                    let _ = results.send(Err(format!("failed to pin worker CPU{cpu_id}")));
                    return;
                }
                if let Ok(mut ids) = pinned.lock() {
                    ids.push(cpu_id);
                }
                loop {
                    let job = match queue.lock() {
                        Ok(queue) => queue.recv(),
                        Err(_) => {
                            control.stop("worker_queue_poisoned");
                            return;
                        }
                    };
                    let Ok((proposal, extra)) = job else { break };
                    let result = world_job(catalog, capture, proposal, candidates, &control, extra);
                    if results.send(Ok(result)).is_err() {
                        break;
                    }
                }
            }));
        }
        drop(results);
        let first = std::cmp::min(MINIMUM_WORLDS, produced);
        let mut remaining = proposals.into_iter();
        for proposal in remaining.by_ref().take(first) {
            if send.send((proposal, false)).is_err() {
                control.stop("worker_queue_closed");
                break;
            }
        }
        let mut received = 0;
        while received < first {
            if control.check().is_err() {
                break;
            }
            match outcomes.recv_timeout(Duration::from_millis(5)) {
                Ok(Ok(world)) => {
                    worlds.push(world);
                    received += 1
                }
                Ok(Err(error)) => {
                    worker_faults.push(error);
                    control.stop("worker_fault")
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    control.stop("workers_disconnected");
                    break;
                }
            }
        }
        let initial_trials = worlds
            .iter()
            .flat_map(|world| world.trials.iter().cloned())
            .collect::<Vec<_>>();
        let minimum = received == MINIMUM_WORLDS
            && aggregates(&candidates.candidates, &initial_trials)
                .iter()
                .all(|a| a.minimum_met);
        if minimum && control.check().is_ok() {
            for proposal in remaining {
                if send.send((proposal, true)).is_err() {
                    control.stop("worker_queue_closed");
                    break;
                }
            }
        } else if shortfall.is_none() {
            shortfall = Some("minimum phase incomplete; no extra worlds admitted".into())
        }
        drop(send);
        for worker in workers {
            if worker.join().is_err() {
                worker_faults.push("worker panicked".into());
                control.stop("worker_panic")
            }
        }
        for outcome in outcomes.try_iter() {
            match outcome {
                Ok(world) => worlds.push(world),
                Err(error) => worker_faults.push(error),
            }
        }
        monitor_done.store(true, Ordering::SeqCst);
        if monitor.join().is_err() {
            worker_faults.push("RSS monitor panicked".into());
            control.stop("rss_monitor_panic")
        }
    });
    components.pool_lifecycle_micros = pool_started.elapsed().as_micros();
    let aggregation_started = Instant::now();
    worlds.sort_by_key(|world| world.world_index);
    let world_components = worlds
        .iter()
        .map(|world| WorldTimings {
            world_index: world.world_index,
            build_calls: world.build_calls,
            build_micros: world.build_micros,
            base_disposal_micros: world.base_disposal_micros,
        })
        .collect();
    let mut actual_worker_cpu_ids = pinned
        .into_inner()
        .map_err(|_| "worker affinity report poisoned")?;
    actual_worker_cpu_ids.sort_unstable();
    let completed_world_jobs = worlds.len();
    let mut trials = worlds
        .into_iter()
        .flat_map(|world| world.trials)
        .collect::<Vec<_>>();
    trials.sort_by_key(|trial| (trial.world_index, trial.candidate_index));
    let aggregate = aggregates(&candidates.candidates, &trials);
    // Fixed common first-eight selection avoids completion-order bias from
    // optional wall-limited extras. All extras remain reported diagnostics.
    let minimum_trials = trials
        .iter()
        .filter(|t| t.world_index < MINIMUM_WORLDS as u64)
        .cloned()
        .collect::<Vec<_>>();
    let selection_aggregates = aggregates(&candidates.candidates, &minimum_trials);
    let minimum = selection_aggregates.iter().all(|a| a.minimum_met);
    let desired = produced == DESIRED_WORLDS
        && trials.len() == produced * candidates.candidates.len()
        && trials.iter().all(|trial| trial.result.useful());
    let viewer = usize::from(capture.data().context.observation.view.viewer.0);
    let chosen_candidate_index = select_candidate(&selection_aggregates, viewer);
    let chosen_candidate_id =
        chosen_candidate_index.map(|index| candidates.candidates[index].candidate_id.clone());
    let chosen_plan = chosen_candidate_index.map(|index| candidates.candidates[index].plan.clone());
    components.aggregation_micros = aggregation_started.elapsed().as_micros();
    let (_, peak) = memory()?;
    let context_id = capture.context_id().to_owned();
    let capture_disposal_started = Instant::now();
    drop(capture);
    components.capture_disposal_micros = capture_disposal_started.elapsed().as_micros();
    let stop_reason = control.reason.lock().ok().and_then(|r| r.clone());
    let strategy_operations = control.operations.load(Ordering::Relaxed);
    drop(control);
    let mut report = SearchReport {
        report_version: 1,
        workload: WORKLOAD.into(),
        context_id,
        request,
        scheduling_policy: SCHEDULING_POLICY.into(),
        candidate_policy: candidates,
        sampler_rng: search_sampler::ASSIGNMENT_RNG.into(),
        rollout_policy: search_policy::ROLLOUT_POLICY.into(),
        desired_worlds: DESIRED_WORLDS,
        produced_distinct_worlds: produced,
        sampling_attempts: attempts,
        world_specs,
        sampling_shortfall: shortfall,
        requested_pairs: DESIRED_WORLDS * aggregate.len(),
        completed_world_jobs,
        trials,
        aggregates: aggregate,
        selection_policy: "fixed-first-eight-only-v1".into(),
        selection_aggregates,
        chosen_candidate_index,
        chosen_candidate_id,
        chosen_plan,
        components,
        world_components,
        actual_worker_cpu_ids,
        worker_faults,
        elapsed_micros: 0,
        maximum_process_rss_bytes: peak,
        strategy_operations,
        minimum_met: minimum,
        desired_complete: desired,
        hard_latency_met: false,
        p95_root_target_met: false,
        memory_met: peak <= MEMORY_BYTES,
        stop_reason,
    };
    report.elapsed_micros = started.elapsed().as_micros();
    report.hard_latency_met = report.elapsed_micros <= u128::from(HARD_MS) * 1000;
    report.p95_root_target_met = report.elapsed_micros <= 3_000_000;
    Ok(report)
}

/// Reconstruct the recorded completed worklist without wall deadlines. This is
/// semantic verification, not a second performance measurement. Diagnostic
/// stops remain explicitly unverified and never count as useful rollouts.
pub(super) fn replay_report(
    catalog: &Catalog,
    capture: &SearchCapture,
    report: &SearchReport,
) -> Result<serde_json::Value, String> {
    if report.report_version != 1
        || report.workload != WORKLOAD
        || report.context_id != capture.context_id()
        || report.scheduling_policy != SCHEDULING_POLICY
        || report.selection_policy != "fixed-first-eight-only-v1"
        || report.sampler_rng != search_sampler::ASSIGNMENT_RNG
        || report.rollout_policy != search_policy::ROLLOUT_POLICY
        || report.desired_worlds != DESIRED_WORLDS
        || report.request.request_version != 1
        || report.request.root_index >= 12
    {
        return Err("search report identity mismatch".into());
    }
    let candidates =
        search_candidates::generate(catalog, &capture.data().context.observation, &mut || Ok(()))?;
    if candidates != report.candidate_policy {
        return Err("search candidate manifest differs from permitted root".into());
    }
    if report.world_specs.len() != report.produced_distinct_worlds
        || report.produced_distinct_worlds > DESIRED_WORLDS
    {
        return Err("search world manifest cardinality".into());
    }
    validate_report_manifest(report, &candidates)?;
    let control = Control {
        started: Instant::now(),
        cancellation: Cancellation::default(),
        stopped: AtomicBool::new(false),
        reason: Mutex::new(None),
        operations: AtomicU64::new(0),
        enforce_deadline: false,
    };
    let mut seen = BTreeSet::new();
    let mut verified = 0usize;
    let mut diagnostic = 0usize;
    for (index, spec) in report.world_specs.iter().enumerate() {
        if spec.world_index != index as u64
            || spec.sampling_attempt >= search_sampler::MAX_DISTINCT_ATTEMPTS
        {
            return Err("search world index/attempt mismatch".into());
        }
        // A declared attempt cannot skip a distinct earlier hypothesis. This
        // binds the retained worklist to the frozen sampler, rather than merely
        // proving that each selected world was possible to generate.
        for earlier in 0..spec.sampling_attempt {
            let skipped = search_sampler::propose(
                capture,
                &report.request.public_job_seed,
                spec.world_index,
                earlier,
                &mut || Ok(()),
            )?;
            if !seen.contains(&skipped.assignment_digest()?) {
                return Err("search sampling skipped an earlier distinct assignment".into());
            }
        }
        let proposal = search_sampler::propose(
            capture,
            &report.request.public_job_seed,
            spec.world_index,
            spec.sampling_attempt,
            &mut || Ok(()),
        )?;
        if proposal.assignment_digest() != Ok(spec.assignment_digest.clone())
            || !seen.insert(spec.assignment_digest.clone())
        {
            return Err("search assignment identity/uniqueness mismatch".into());
        }
        let trials = report
            .trials
            .iter()
            .filter(|t| t.world_index == spec.world_index)
            .collect::<Vec<_>>();
        if trials.is_empty() {
            continue;
        }
        let mut candidate_indices = BTreeSet::new();
        let mut build_calls = 0;
        let (base, map) = build_world(catalog, capture, proposal, &control, &mut build_calls)?;
        for trial in trials {
            if trial.assignment_digest != spec.assignment_digest
                || trial.candidate_index >= candidates.candidates.len()
                || !candidate_indices.insert(trial.candidate_index)
                || trial.candidate_id != candidates.candidates[trial.candidate_index].candidate_id
            {
                return Err("search recorded trial identity mismatch".into());
            }
            if matches!(
                trial.result,
                TrialOutcome::NotStarted { .. } | TrialOutcome::ResourceTruncated { .. }
            ) {
                if matches!(trial.result, TrialOutcome::NotStarted { .. })
                    && trial.counts != TrialCounts::default()
                {
                    return Err("unstarted trial claims executed work".into());
                }
                diagnostic += 1;
                continue;
            }
            let (counts, outcome, _) = run_trial(
                &base,
                &map,
                &candidates.candidates[trial.candidate_index],
                capture.data().context.observation.view.viewer,
                catalog,
                &control,
            );
            if counts != trial.counts || outcome != trial.result {
                return Err(format!(
                    "search semantic replay differs world{} candidate{}",
                    trial.world_index, trial.candidate_index
                ));
            }
            verified += 1;
        }
    }
    if report
        .trials
        .iter()
        .any(|t| t.world_index >= report.produced_distinct_worlds as u64)
    {
        return Err("trial references absent world".into());
    }
    let all = aggregates(&candidates.candidates, &report.trials);
    let first = aggregates(
        &candidates.candidates,
        &report
            .trials
            .iter()
            .filter(|t| t.world_index < MINIMUM_WORLDS as u64)
            .cloned()
            .collect::<Vec<_>>(),
    );
    if all != report.aggregates
        || first != report.selection_aggregates
        || report.minimum_met != first.iter().all(|a| a.minimum_met)
    {
        return Err("search aggregate proof mismatch".into());
    }
    let chosen = select_candidate(
        &first,
        usize::from(capture.data().context.observation.view.viewer.0),
    );
    if report.chosen_candidate_index != chosen
        || report.chosen_candidate_id
            != chosen.map(|i| candidates.candidates[i].candidate_id.clone())
        || report.chosen_plan != chosen.map(|i| candidates.candidates[i].plan.clone())
    {
        return Err("search chosen action differs from fixed fair-phase evaluation".into());
    }

    Ok(
        serde_json::json!({"status":"semantic_worklist_verified","verified_trials":verified,"unverified_diagnostic_stops":diagnostic,"measurements":"not_remeasured","context_id":capture.context_id()}),
    )
}

/// Structural claims are checked before expensive semantic replay. Diagnostic
/// timings and resource stops remain measurements, never reconstructed facts.
fn validate_report_manifest(
    report: &SearchReport,
    candidates: &CandidateSet,
) -> Result<(), String> {
    let count = candidates.candidates.len();
    let mut worlds = BTreeSet::new();
    let mut previous = None;
    let mut by_world = std::collections::BTreeMap::<u64, usize>::new();
    for trial in &report.trials {
        let key = (trial.world_index, trial.candidate_index);
        if previous.is_some_and(|p| p >= key)
            || trial.world_index >= report.produced_distinct_worlds as u64
            || trial.candidate_index >= count
            || trial.candidate_id != candidates.candidates[trial.candidate_index].candidate_id
            || trial.counts.accepted_inputs > HORIZON
            || trial
                .counts
                .advance_calls
                .checked_add(trial.counts.query_calls)
                .is_none_or(|calls| calls > CORE_CALL_LIMIT)
        {
            return Err("search trial worklist or hard cap mismatch".into());
        }
        previous = Some(key);
        worlds.insert(trial.world_index);
        *by_world.entry(trial.world_index).or_default() += 1;
    }
    let mut component_worlds = BTreeSet::new();
    for component in &report.world_components {
        if !component_worlds.insert(component.world_index)
            || component.build_calls > CORE_CALL_LIMIT
        {
            return Err("search world component identity/cap mismatch".into());
        }
    }
    let attempts = report
        .world_specs
        .iter()
        .try_fold(0u64, |total, spec| {
            total.checked_add(spec.sampling_attempt.checked_add(1)?)
        })
        .ok_or("search sampling counter overflow")?;
    let desired = report.produced_distinct_worlds == DESIRED_WORLDS
        && report.trials.len() == DESIRED_WORLDS * count
        && report.trials.iter().all(|trial| trial.result.useful());
    if by_world.values().any(|n| *n != count)
        || report.completed_world_jobs != worlds.len()
        || component_worlds != worlds
        || report.requested_pairs != DESIRED_WORLDS * count
        || report.desired_complete != desired
        || report.sampling_attempts < attempts
        || report.sampling_attempts > attempts + search_sampler::MAX_DISTINCT_ATTEMPTS
        || (report.produced_distinct_worlds == DESIRED_WORLDS
            && report.sampling_attempts != attempts)
        || report.hard_latency_met != (report.elapsed_micros <= u128::from(HARD_MS) * 1000)
        || report.p95_root_target_met != (report.elapsed_micros <= 3_000_000)
        || report.memory_met != (report.maximum_process_rss_bytes <= MEMORY_BYTES)
    {
        return Err("search report coverage, counters or claimed thresholds differ".into());
    }
    Ok(())
}

#[cfg(test)]
mod schedule_equivalence;

#[cfg(test)]
mod tests {
    use super::*;

    fn stopped_report() -> SearchReport {
        let candidates = CandidateSet {
            policy: search_candidates::CANDIDATE_POLICY.into(),
            candidates: vec![Candidate {
                candidate_id: "pass".into(),
                plan: ActionPlan::Pass,
            }],
            truncated_at_limit: false,
            coalesced_basic_hand_copies: 0,
            standalone_mana_actions_omitted: true,
        };
        SearchReport {
            report_version: 1,
            workload: WORKLOAD.into(),
            context_id: "context".into(),
            request: SearchRequest {
                request_version: 1,
                root_index: 0,
                public_job_seed: [0; 32],
                worker_cpu_ids: [0, 1, 2, 3],
            },
            scheduling_policy: SCHEDULING_POLICY.into(),
            candidate_policy: candidates,
            sampler_rng: search_sampler::ASSIGNMENT_RNG.into(),
            rollout_policy: search_policy::ROLLOUT_POLICY.into(),
            desired_worlds: DESIRED_WORLDS,
            produced_distinct_worlds: 1,
            sampling_attempts: 1,
            world_specs: vec![WorldSpec {
                world_index: 0,
                sampling_attempt: 0,
                assignment_digest: "world".into(),
            }],
            sampling_shortfall: Some("resource:deadline".into()),
            requested_pairs: DESIRED_WORLDS,
            completed_world_jobs: 1,
            trials: vec![TrialResult {
                world_index: 0,
                assignment_digest: "world".into(),
                candidate_index: 0,
                candidate_id: "pass".into(),
                counts: TrialCounts::default(),
                timings: TrialTimings::default(),
                result: TrialOutcome::NotStarted {
                    reason: "resource:deadline".into(),
                },
            }],
            aggregates: vec![],
            selection_policy: "fixed-first-eight-only-v1".into(),
            selection_aggregates: vec![],
            chosen_candidate_index: None,
            chosen_candidate_id: None,
            chosen_plan: None,
            components: RootTimings::default(),
            world_components: vec![WorldTimings {
                world_index: 0,
                build_calls: 0,
                build_micros: 0,
                base_disposal_micros: 0,
            }],
            actual_worker_cpu_ids: vec![0, 1, 2, 3],
            worker_faults: vec![],
            elapsed_micros: 5_000_001,
            maximum_process_rss_bytes: 1000,
            strategy_operations: 1,
            minimum_met: false,
            desired_complete: false,
            hard_latency_met: false,
            p95_root_target_met: false,
            memory_met: true,
            stop_reason: Some("deadline".into()),
        }
    }
    #[test]
    fn search_job_manifest_rejects_missing_duplicate_and_overcap_work() {
        let valid = stopped_report();
        validate_report_manifest(&valid, &valid.candidate_policy).unwrap();
        for mutate in [
            (|r: &mut SearchReport| {
                r.trials.clear();
            }) as fn(&mut SearchReport),
            |r| {
                r.trials.push(r.trials[0].clone());
            },
            |r| {
                r.trials[0].candidate_id = "other".into();
            },
            |r| {
                r.trials[0].counts.accepted_inputs = HORIZON + 1;
            },
            |r| {
                r.trials[0].counts.query_calls = u64::MAX;
                r.trials[0].counts.advance_calls = 1;
            },
            |r| {
                r.world_components.clear();
            },
            |r| {
                r.completed_world_jobs = 0;
            },
            |r| {
                r.requested_pairs += 1;
            },
            |r| {
                r.desired_complete = true;
            },
            |r| {
                r.hard_latency_met = true;
            },
            |r| {
                r.memory_met = false;
            },
            |r| {
                r.sampling_attempts = 0;
            },
        ] {
            let mut forged: SearchReport =
                serde_json::from_value(serde_json::to_value(&valid).unwrap()).unwrap();
            mutate(&mut forged);
            assert!(validate_report_manifest(&forged, &forged.candidate_policy).is_err());
        }
    }
    #[test]
    fn search_job_fair_selection_requires_same_eight_worlds_and_ties_by_index() {
        let item = |index, utility| CandidateAggregate {
            candidate_index: index,
            candidate_id: format!("c{index}"),
            useful_distinct_worlds: 8,
            minimum_met: true,
            terminal_games: 0,
            horizon_trials: 8,
            winner_counts: [0; 4],
            utility_sums: vec![
                Quantity::from(utility),
                Quantity::zero(),
                Quantity::zero(),
                Quantity::zero(),
            ],
        };
        let mut candidates = vec![item(0, 8), item(1, 8)];
        assert_eq!(select_candidate(&candidates, 0), Some(0));
        candidates[1].utility_sums[0] = Quantity::from(9);
        assert_eq!(select_candidate(&candidates, 0), Some(1));
        candidates[1].useful_distinct_worlds = 7;
        assert_eq!(select_candidate(&candidates, 0), None);
        candidates[1].useful_distinct_worlds = 9;
        assert_eq!(select_candidate(&candidates, 0), None);
    }
    #[test]
    fn search_job_call_caps_and_cancellation_stop_before_work() {
        let mut counts = TrialCounts {
            query_calls: CORE_CALL_LIMIT - 1,
            ..TrialCounts::default()
        };
        counts.advance().unwrap();
        let before = counts.clone();
        assert!(counts.query().is_err());
        assert_eq!(counts, before);
        let cancellation = Cancellation::default();
        let control = Control {
            started: Instant::now(),
            cancellation: cancellation.clone(),
            stopped: AtomicBool::new(false),
            reason: Mutex::new(None),
            operations: AtomicU64::new(0),
            enforce_deadline: false,
        };
        cancellation.cancel();
        assert_eq!(control.check(), Err("resource:cancelled".into()));
    }
    #[test]
    fn search_job_reference_verifier_binds_sampler_worklist_and_diagnostic_scope() {
        let catalog = mtg_compiler::match_fixture_catalog().unwrap();
        let root = super::super::search_roots::prepare(&catalog, 0).unwrap();
        let capture = super::super::search_roots::capture(&root.game, root.case.viewer).unwrap();
        let mut report = stopped_report();
        report.context_id = capture.context_id().into();
        report.candidate_policy =
            search_candidates::generate(&catalog, &capture.data().context.observation, &mut || {
                Ok(())
            })
            .unwrap();
        let proposal =
            search_sampler::propose(&capture, &report.request.public_job_seed, 0, 0, &mut || {
                Ok(())
            })
            .unwrap();
        let assignment = proposal.assignment_digest().unwrap();
        report.world_specs[0].assignment_digest = assignment.clone();
        report.trials = report
            .candidate_policy
            .candidates
            .iter()
            .enumerate()
            .map(|(candidate_index, candidate)| TrialResult {
                world_index: 0,
                assignment_digest: assignment.clone(),
                candidate_index,
                candidate_id: candidate.candidate_id.clone(),
                counts: TrialCounts::default(),
                timings: TrialTimings::default(),
                result: TrialOutcome::NotStarted {
                    reason: "resource:deadline".into(),
                },
            })
            .collect();
        report.requested_pairs = DESIRED_WORLDS * report.candidate_policy.candidates.len();
        report.aggregates = aggregates(&report.candidate_policy.candidates, &report.trials);
        report.selection_aggregates =
            aggregates(&report.candidate_policy.candidates, &report.trials);
        let proof = replay_report(&catalog, &capture, &report).unwrap();
        assert_eq!(proof["verified_trials"], 0);
        assert_eq!(proof["unverified_diagnostic_stops"], report.trials.len());
        report.chosen_candidate_index = Some(0);
        assert!(replay_report(&catalog, &capture, &report).is_err());
        report.chosen_candidate_index = None;
        // A valid but later hypothetical world must not replace the declared
        // sampler's first distinct result, even if all work is diagnostic.
        let later =
            search_sampler::propose(&capture, &report.request.public_job_seed, 0, 1, &mut || {
                Ok(())
            })
            .unwrap();
        let later_digest = later.assignment_digest().unwrap();
        assert_ne!(assignment, later_digest);
        report.world_specs[0].sampling_attempt = 1;
        report.world_specs[0].assignment_digest = later_digest.clone();
        report.sampling_attempts = 2;
        for trial in &mut report.trials {
            trial.assignment_digest = later_digest.clone();
        }
        assert!(
            replay_report(&catalog, &capture, &report)
                .unwrap_err()
                .contains("skipped an earlier distinct")
        );
    }
}
