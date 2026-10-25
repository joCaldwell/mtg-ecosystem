//! Trusted test-host comparison, excluded from every production executable.
//!
//! The sampled branch receives only the sealed permitted capture. The separately
//! labeled reference forks the actual fixture Game through the shared rollout
//! body. No actual hidden assignment, RNG or snapshot enters candidate generation,
//! the sampler, client responses or the production search entry point.
use super::{public_job_seed, search_jobs, search_roots};
use crate::{BUILD_ID, current_source_manifest};
use mtg_contracts::{Catalog, Quantity, canonical, sha256};
use search_jobs::{TrialCounts, TrialOutcome, TrialResult, fixed_work};
use serde::Serialize;
use std::{
    collections::BTreeSet,
    fs,
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::Path,
    time::Instant,
};

const CONTRACT: &str = "foundations-same-policy-true-root-diagnostic-v1";
const REPEATS: usize = search_jobs::MINIMUM_WORLDS;

#[derive(Debug, Serialize)]
struct CandidateSummary {
    candidate_index: usize,
    candidate_id: String,
    completed_trials: usize,
    distinct_assignments: usize,
    terminal_trials: usize,
    horizon_trials: usize,
    utility_sums: Vec<Quantity>,
    counts: TrialCounts,
}

fn summaries(
    candidates: &crate::search_candidates::CandidateSet,
    trials: &[TrialResult],
) -> Result<Vec<CandidateSummary>, String> {
    if trials.len() != candidates.candidates.len() * REPEATS {
        return Err("diagnostic requires every frozen candidate in every slot".into());
    }
    let mut pairs = BTreeSet::new();
    for trial in trials {
        if trial.world_index >= REPEATS as u64
            || !pairs.insert((trial.world_index, trial.candidate_index))
            || candidates
                .candidates
                .get(trial.candidate_index)
                .is_none_or(|candidate| candidate.candidate_id != trial.candidate_id)
        {
            return Err("diagnostic trial identity or Cartesian denominator differs".into());
        }
    }
    candidates
        .candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| {
            let selected = trials
                .iter()
                .filter(|trial| trial.candidate_index == index)
                .collect::<Vec<_>>();
            let mut row = CandidateSummary {
                candidate_index: index,
                candidate_id: candidate.candidate_id.clone(),
                completed_trials: selected.len(),
                distinct_assignments: selected
                    .iter()
                    .map(|trial| &trial.assignment_digest)
                    .collect::<BTreeSet<_>>()
                    .len(),
                terminal_trials: 0,
                horizon_trials: 0,
                utility_sums: vec![Quantity::zero(); 4],
                counts: TrialCounts::default(),
            };
            for trial in selected {
                let evaluations = match &trial.result {
                    TrialOutcome::Terminal { evaluations, .. } => {
                        row.terminal_trials += 1;
                        evaluations
                    }
                    TrialOutcome::Horizon { evaluations, .. } => {
                        row.horizon_trials += 1;
                        evaluations
                    }
                    other => {
                        return Err(format!(
                            "non-useful diagnostic trial remains a failure: {other:?}"
                        ));
                    }
                };
                if evaluations.len() != 4
                    || evaluations
                        .iter()
                        .map(|value| value.player.0)
                        .collect::<BTreeSet<_>>()
                        != BTreeSet::from([0, 1, 2, 3])
                {
                    return Err("diagnostic must retain all four exact utility evaluations".into());
                }
                for value in evaluations {
                    row.utility_sums[usize::from(value.player.0)].add(&value.utility);
                }
                row.counts.accepted_inputs = row
                    .counts
                    .accepted_inputs
                    .checked_add(trial.counts.accepted_inputs)
                    .ok_or("accepted input overflow")?;
                row.counts.advance_calls = row
                    .counts
                    .advance_calls
                    .checked_add(trial.counts.advance_calls)
                    .ok_or("advance count overflow")?;
                row.counts.query_calls = row
                    .counts
                    .query_calls
                    .checked_add(trial.counts.query_calls)
                    .ok_or("query count overflow")?;
                row.counts.rule_steps = row
                    .counts
                    .rule_steps
                    .checked_add(trial.counts.rule_steps)
                    .ok_or("rule count overflow")?;
            }
            Ok(row)
        })
        .collect()
}

/// Same exact utility-sum maximization and first-candidate tie rule used by the
/// production selector. Both branches have eight completed slots per candidate.
/// A real-world repeat is deliberately NOT relabeled as a distinct sampled world
/// merely to pass production's separate eight-distinct-world eligibility guard.
fn select(rows: &[CandidateSummary], viewer: usize) -> Result<usize, String> {
    if rows.is_empty()
        || viewer >= 4
        || rows
            .iter()
            .any(|row| row.completed_trials != REPEATS || row.utility_sums.len() != 4)
    {
        return Err("incomplete diagnostic selection denominator".into());
    }
    let mut best = 0;
    for index in 1..rows.len() {
        if rows[index].utility_sums[viewer] > rows[best].utility_sums[viewer] {
            best = index;
        }
    }
    Ok(rows[best].candidate_index)
}

fn root_comparison(catalog: &Catalog, index: usize) -> Result<serde_json::Value, String> {
    let fixture_started = Instant::now();
    let fixture = search_roots::prepare(catalog, index)?;
    let real_before = fixture.game.state_digest()?;
    let fixture_micros = fixture_started.elapsed().as_micros();
    let capture_started = Instant::now();
    let capture = search_roots::capture(&fixture.game, fixture.case.viewer)?;
    let capture_micros = capture_started.elapsed().as_micros();
    let freeze_started = Instant::now();
    let work = fixed_work::freeze(catalog, &capture, public_job_seed(index), REPEATS)?;
    let freeze_micros = freeze_started.elapsed().as_micros();
    let candidates = work.candidates();
    let candidate_sha = sha256(canonical(candidates)?.as_bytes());
    let order = (0..REPEATS).collect::<Vec<_>>();
    // One worker and fixed complete work: no deadline-dependent omissions or
    // hidden candidate/root selection. This is not the product latency workload.
    let sampled = fixed_work::execute(catalog, &capture, &work, 1, None, &order)?;
    if !sampled.complete_useful {
        return Err("sampled fixed-work diagnostic did not complete every trial".into());
    }
    let sampled_rows = summaries(
        candidates,
        &sampled
            .worlds
            .iter()
            .flat_map(|world| world.trials.clone())
            .collect::<Vec<_>>(),
    )?;
    if sampled_rows
        .iter()
        .any(|row| row.distinct_assignments != REPEATS)
    {
        return Err("sampled branch lacks eight genuinely distinct hidden assignments".into());
    }
    let real_started = Instant::now();
    // Trusted actual Game input is confined to this cfg(test) adapter/helper.
    // Actual origin, library order, private histories and RNG stay untouched.
    let real_trials = fixed_work::evaluate_real(catalog, &fixture.game, &capture, candidates)?;
    let real_micros = real_started.elapsed().as_micros();
    let real_rows = summaries(candidates, &real_trials)?;
    if real_rows.iter().any(|row| row.distinct_assignments != 1)
        || real_trials
            .iter()
            .any(|trial| trial.assignment_digest != real_before)
    {
        return Err("true reference must remain one actual world, not eight samples".into());
    }
    for candidate_index in 0..candidates.candidates.len() {
        let repeats = real_trials
            .iter()
            .filter(|trial| trial.candidate_index == candidate_index)
            .collect::<Vec<_>>();
        if repeats
            .iter()
            .any(|trial| trial.result != repeats[0].result || trial.counts != repeats[0].counts)
        {
            return Err(
                "deterministic true-root repeats changed semantic result or work counts".into(),
            );
        }
    }
    if fixture.game.state_digest()? != real_before {
        return Err("trusted reference mutated the authoritative root".into());
    }
    let viewer = usize::from(fixture.case.viewer.0);
    let sampled_choice = select(&sampled_rows, viewer)?;
    let real_choice = select(&real_rows, viewer)?;
    if canonical(candidates)? != canonical(work.candidates())? {
        return Err("frozen candidate identity changed".into());
    }
    Ok(serde_json::json!({
        "root_index":index,"case":fixture.case,"root_journal":fixture.game.replay_journal(),
        "trusted_actual_root_state_sha256":real_before,"public_job_seed":public_job_seed(index),
        "capture_context_id":capture.context_id(),"frozen_work":work,"frozen_work_sha256":work.digest()?,
        "candidate_set_sha256":candidate_sha,"horizon_accepted_inputs":search_jobs::HORIZON,
        "sampled":{"summary":sampled_rows,"execution":sampled,"distinct_worlds":REPEATS},
        "true_reference":{"summary":real_rows,"trials":real_trials,"distinct_worlds":1,"repeat_slots":REPEATS},
        "selection":{"sampled_candidate_index":sampled_choice,"true_reference_candidate_index":real_choice,
            "same_candidate":sampled_choice==real_choice,
            "sampled_candidate_id":candidates.candidates[sampled_choice].candidate_id,
            "true_reference_candidate_id":candidates.candidates[real_choice].candidate_id,
            "policy":"viewer utility-sum maximum; first generated candidate wins exact ties",
            "true_world_utility_sum_of_sampled_choice":real_rows[sampled_choice].utility_sums[viewer],
            "true_world_utility_sum_of_true_choice":real_rows[real_choice].utility_sums[viewer]},
        "observed_cost_micros":{"fixture":fixture_micros,"common_permitted_capture":capture_micros,
            "candidate_and_sample_producer":freeze_micros,"sampled_pool_build_rollout_aggregate":sampled.elapsed_micros,
            "true_root_validation_and_repeated_rollouts":real_micros},
        "cost_scope":"Serial sampled-then-true in one process; shared caches/order effects not controlled. Absolute diagnostics only, no causal ratio or acceptance latency. Per-trial component times overlap inclusive totals."
    }))
}

fn save_private(file: &mut fs::File, report: &serde_json::Value) -> Result<(), String> {
    file.set_len(0).map_err(|e| e.to_string())?;
    use std::io::{Seek, SeekFrom};
    file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    file.write_all(canonical(report)?.as_bytes())
        .map_err(|e| e.to_string())?;
    file.write_all(b"\n").map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())
}

fn private_output(path: &Path) -> Result<fs::File, String> {
    if !path.is_absolute() || path.exists() || path.is_symlink() {
        return Err("new absolute private diagnostic output required".into());
    }
    let parent = path.parent().ok_or("output parent missing")?;
    for ancestor in parent.ancestors() {
        if ancestor.is_symlink() {
            return Err("diagnostic output ancestry contains symlink".into());
        }
    }
    let metadata = fs::metadata(parent).map_err(|e| e.to_string())?;
    if !metadata.is_dir() || metadata.permissions().mode() & 0o777 != 0o700 {
        return Err("diagnostic output parent must be private0700".into());
    }
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| e.to_string())
}

#[test]
#[ignore = "explicit trusted omniscient diagnostic; no product acceptance or quality proof"]
fn same_strategy_omniscient_reference_diagnostic() {
    let output =
        std::env::var("MTG_OMNISCIENT_SEARCH_OUTPUT").expect("explicit new private output path");
    let mut file = private_output(Path::new(&output)).expect("private exclusive output");
    let source = current_source_manifest().expect("exact diagnostic source identity");
    let catalog =
        mtg_compiler::catalog::embedded_catalog("match").expect("frozen admitted catalog");
    let mut report = serde_json::json!({"schema":CONTRACT,"status":"running","engine_build_id":BUILD_ID,
        "source_manifest":source,"root_indices":(0..search_roots::ROOT_COUNT).collect::<Vec<_>>(),
        "horizon_accepted_inputs":search_jobs::HORIZON,"repeat_slots":REPEATS,"roots":[],
        "rollout_policy":crate::search_policy::ROLLOUT_POLICY,
        "evaluation_policy":"same shared production evaluator: terminal winner +/-1000000, draw0; otherwise live player's life+hand count+positive controlled battlefield power, departed -1000000",
        "sampled_information":"sealed permitted capture only","true_reference_information":"trusted actual Game; direct forks preserve actual origin and RNG",
        "quality_claim":false,"acceptance_claim":false,"separate_from_product_useful_trial_counts":true,
        "strategy_randomness":"Current candidate/rollout/evaluation/tie policies are deterministic. Public job seed and identical slot indices are frozen; true Game RNG remains actual, not replaced by a sampled future seed.",
        "external_resource_limits":"Host must record outer watchdog, whole-process peak and cleanup; this cfg(test) diagnostic does not waive product or hosting limits."});
    save_private(&mut file, &report).unwrap();
    assert_eq!(search_jobs::HORIZON, 256);
    for index in 0..search_roots::ROOT_COUNT {
        match root_comparison(&catalog, index) {
            Ok(root) => report["roots"].as_array_mut().unwrap().push(root),
            Err(error) => {
                report["status"] = serde_json::json!("diagnostic_failed");
                report["failure"] = serde_json::json!({"root_index":index,"error":error});
                save_private(&mut file, &report).unwrap();
                panic!("trusted diagnostic failed; private partial evidence retained");
            }
        }
        save_private(&mut file, &report).unwrap();
    }
    assert_eq!(current_source_manifest().unwrap(), source);
    report["status"] = serde_json::json!("same_strategy_true_root_diagnostic_complete");
    save_private(&mut file, &report).unwrap();
}

#[test]
fn diagnostic_denominators_do_not_promote_repeat_labels_to_distinct_worlds() {
    use crate::search_candidates::{ActionPlan, Candidate, CandidateSet};
    use mtg_core::{PlayerId, PlayerStatus};
    let candidates = CandidateSet {
        policy: "test-only".into(),
        candidates: vec![Candidate {
            candidate_id: "one".into(),
            plan: ActionPlan::Pass,
        }],
        truncated_at_limit: false,
        coalesced_basic_hand_copies: 0,
        standalone_mana_actions_omitted: false,
    };
    let mut trials = (0..REPEATS)
        .map(|slot| TrialResult {
            world_index: slot as u64,
            assignment_digest: "the-same-actual-world".into(),
            candidate_index: 0,
            candidate_id: "one".into(),
            counts: TrialCounts::default(),
            timings: search_jobs::TrialTimings::default(),
            result: TrialOutcome::Horizon {
                evaluations: (0..4)
                    .map(|player| search_jobs::PlayerEvaluation {
                        player: PlayerId(player),
                        public_status: PlayerStatus::Live,
                        utility: Quantity::from(i64::from(player)),
                    })
                    .collect(),
                state_sha256: "identical-result".into(),
            },
        })
        .collect::<Vec<_>>();
    let rows = summaries(&candidates, &trials).unwrap();
    assert_eq!(rows[0].completed_trials, 8);
    assert_eq!(rows[0].distinct_assignments, 1);
    assert_eq!(rows[0].utility_sums[3], Quantity::from(24));
    trials[7].world_index = 6;
    assert!(
        summaries(&candidates, &trials).is_err(),
        "duplicate slot must not conceal missing work"
    );
    trials[7].world_index = 7;
    trials[7].candidate_id = "other".into();
    assert!(
        summaries(&candidates, &trials).is_err(),
        "candidate identity cannot drift across branches"
    );
    trials[7].candidate_id = "one".into();
    trials[7].result = TrialOutcome::ResourceTruncated {
        reason: "host stop".into(),
    };
    assert!(
        summaries(&candidates, &trials).is_err(),
        "resource stops are never a utility sample"
    );
}

#[test]
fn diagnostic_selection_uses_exact_values_and_first_candidate_on_ties() {
    let row = |candidate_index, utility| CandidateSummary {
        candidate_index,
        candidate_id: format!("{candidate_index}"),
        completed_trials: REPEATS,
        distinct_assignments: 1,
        terminal_trials: 0,
        horizon_trials: REPEATS,
        utility_sums: vec![Quantity::from(utility); 4],
        counts: TrialCounts::default(),
    };
    let mut rows = vec![row(0, i64::MAX), row(1, i64::MAX)];
    assert_eq!(select(&rows, 0).unwrap(), 0);
    rows[1].utility_sums[0].add(&Quantity::from(1));
    assert_eq!(
        select(&rows, 0).unwrap(),
        1,
        "no float narrowing at or above i64 range"
    );
    assert_eq!(select(&rows, 1).unwrap(), 0, "utility is chooser-specific");
    rows[1].completed_trials -= 1;
    assert!(select(&rows, 0).is_err());
}
