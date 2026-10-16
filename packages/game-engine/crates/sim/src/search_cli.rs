//! Trusted local fixture/evidence adapter. Search policies receive captures,
//! never these authoritative fixture seeds, journals or real-state hashes.
use super::{search_jobs, search_roots, *};
use mtg_core::{CaptureOutcome, SearchCapture};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SearchEvidence {
    evidence_version: u32,
    kind: String,
    build_id: String,
    measurement_classification: String,
    source_manifest: serde_json::Value,
    case: search_roots::RootCase,
    root_journal: ReplayJournal,
    root_state_sha256: String,
    report: search_jobs::SearchReport,
}

fn public_job_seed(index: usize) -> [u8; 32] {
    // Independent public strategy input; never derived from an actual match RNG.
    let hash = sha256(format!("commander-open-search-v1/job/{index}").as_bytes());
    std::array::from_fn(|i| u8::from_str_radix(&hash[i * 2..i * 2 + 2], 16).expect("SHA256 hex"))
}

fn capture_resource_check(started: Instant) -> Result<(), String> {
    let (rss, hwm) = search_jobs::memory()?;
    if rss > 2 * 1024 * 1024 * 1024 || hwm > 2 * 1024 * 1024 * 1024 {
        return Err("search capture memory limit; zero useful trials".into());
    }
    if started.elapsed() >= std::time::Duration::from_millis(5_000) {
        return Err("search capture deadline; zero useful trials".into());
    }
    Ok(())
}
fn timed_capture(game: &Game, viewer: PlayerId, started: Instant) -> Result<SearchCapture, String> {
    capture_resource_check(started)?;
    let mut outcome = game.capture_search(viewer, Budget::new(64));
    for _ in 0..100_000 {
        // Also check completed capture work before handing its allocations to
        // the runner. Each subsequent resume is preceded by this same guard.
        capture_resource_check(started)?;
        match outcome {
            CaptureOutcome::Complete(capture) => return Ok(capture),
            CaptureOutcome::Rejected(reason) => return Err(format!("search capture: {reason:?}")),
            CaptureOutcome::Incomplete { work, .. } => {
                outcome = game.resume_search_capture(work, Budget::new(64));
            }
        }
    }
    Err("search capture call limit; zero useful trials".into())
}

pub(super) fn diagnostic(
    catalog: &Catalog,
    index: usize,
    cancel: bool,
) -> Result<SearchEvidence, String> {
    let source_manifest = current_source_manifest()?;
    let evidence = evaluate(
        catalog,
        index,
        cancel,
        "concurrent-development-diagnostic; not P7 acceptance",
        &source_manifest,
    )?;
    if current_source_manifest()? != source_manifest {
        return Err("search source identity changed during diagnostic".into());
    }
    Ok(evidence)
}

fn evaluate(
    catalog: &Catalog,
    index: usize,
    cancel: bool,
    classification: &str,
    source_manifest: &serde_json::Value,
) -> Result<SearchEvidence, String> {
    let fixture = search_roots::prepare(catalog, index)?;
    let started = Instant::now();
    let capture = timed_capture(&fixture.game, fixture.case.viewer, started)?;
    let request = search_jobs::SearchRequest {
        request_version: 1,
        root_index: u32::try_from(index).map_err(|_| "root index overflow")?,
        public_job_seed: public_job_seed(index),
        worker_cpu_ids: [0, 1, 2, 3],
    };
    let mut report = if cancel {
        let cancellation = search_jobs::Cancellation::default();
        cancellation.cancel();
        search_jobs::run_cancellable(catalog, capture, request, started, cancellation)?
    } else {
        search_jobs::run(catalog, capture, request, started)?
    };
    // Include the runner's return and final owned-input disposal, not only its
    // internal stopwatch. Evidence encoding below is outside the native caller.
    report.elapsed_micros = started.elapsed().as_micros();
    report.hard_latency_met = report.elapsed_micros <= 5_000_000;
    report.p95_root_target_met = report.elapsed_micros <= 3_000_000;
    Ok(SearchEvidence {
        evidence_version: 1,
        kind: "trusted-search-fixture-evidence".into(),
        build_id: BUILD_ID.into(),
        measurement_classification: classification.into(),
        source_manifest: source_manifest.clone(),
        case: fixture.case,
        root_journal: fixture.game.replay_journal(),
        root_state_sha256: fixture.game.state_digest()?,
        report,
    })
}

pub(super) fn benchmark(catalog: &Catalog, directory: &Path) -> Result<serde_json::Value, String> {
    if BUILD_PROFILE != "release" || BUILD_DEBUG_ASSERTIONS != "false" {
        return Err("search acceptance requires the pinned release build".into());
    }
    let source_manifest = current_source_manifest()?;
    let source_manifest_sha256 = sha256(canonical(&source_manifest)?.as_bytes());
    // The caller explicitly declares quiescence. Preserve actual load and all
    // failed attempts; the declaration itself is not proof of numeric success.
    fs::create_dir(directory).map_err(|e| format!("fresh benchmark directory required: {e}"))?;
    let workload_bytes = include_str!("../../../tests/fixtures/search-workload.json");
    let workload: serde_json::Value = strict_read(workload_bytes)?;
    fs::write(directory.join("workload.json"), workload_bytes).map_err(|e| e.to_string())?;
    let cpu_info = fs::read_to_string("/proc/cpuinfo").map_err(|e| e.to_string())?;
    let memory = fs::read_to_string("/proc/meminfo").map_err(|e| e.to_string())?;
    let reference_cpu = cpu_info
        .lines()
        .any(|l| l.starts_with("model name") && l.contains("AMD Ryzen 9 9950X"));
    let manifest = serde_json::json!({
        "manifest_version": 1, "build_id": BUILD_ID, "rustc": BUILD_RUSTC,
        "target": BUILD_TARGET, "profile": BUILD_PROFILE,
        "workload_sha256": sha256(workload_bytes.as_bytes()),
        "quiescence_declared": true, "cpu_info": cpu_info, "memory_info": memory,
        "reference_cpu_verified": reference_cpu,
        "jobs": 60, "repetitions_per_root": 5,
        "native_source_manifest_sha256":source_manifest_sha256,
        "native_source_manifest":source_manifest,
        "measurement": workload["measurement"],
    });
    fs::write(directory.join("manifest.json"), canonical(&manifest)?).map_err(|e| e.to_string())?;
    let mut jobs = vec![];
    let mut latencies = vec![];
    let mut all_minimum = true;
    let mut hard_latency = true;
    let mut memory_met = true;
    let mut faults = false;
    let mut all_worker_cpu_ids_verified = true;
    for repetition in 0..5 {
        for index in 0..search_roots::ROOT_COUNT {
            let filename = format!("root-{index:02}-repeat-{repetition}.json");
            let load = fs::read_to_string("/proc/loadavg").map_err(|e| e.to_string())?;
            match evaluate(
                catalog,
                index,
                false,
                "declared-quiescent-reference-native",
                &source_manifest,
            ) {
                Ok(evidence) => {
                    let report = &evidence.report;
                    latencies.push(report.elapsed_micros);
                    all_minimum &= report.minimum_met;
                    hard_latency &= report.hard_latency_met;
                    memory_met &= report.memory_met;
                    let worker_cpu_ids_verified = report.request.worker_cpu_ids == [0, 1, 2, 3]
                        && report.actual_worker_cpu_ids == [0, 1, 2, 3];
                    all_worker_cpu_ids_verified &= worker_cpu_ids_verified;
                    faults |= !report.worker_faults.is_empty()
                        || report.trials.iter().any(|trial| {
                            matches!(
                                trial.result,
                                search_jobs::TrialOutcome::EngineFault { .. }
                                    | search_jobs::TrialOutcome::PolicyFailure { .. }
                                    | search_jobs::TrialOutcome::InvalidWorld { .. }
                            )
                        });
                    jobs.push(serde_json::json!({
                        "index": index, "repetition": repetition, "evidence": filename,
                        "elapsed_micros": report.elapsed_micros,
                        "minimum_met": report.minimum_met,
                        "desired_complete": report.desired_complete,
                        "maximum_process_rss_bytes": report.maximum_process_rss_bytes,
                        "load_before": load,
                        "worker_cpu_ids_verified":worker_cpu_ids_verified,
                    }));
                    fs::write(directory.join(&filename), canonical(&evidence)?)
                        .map_err(|e| e.to_string())?;
                }
                Err(error) => {
                    all_minimum = false;
                    hard_latency = false;
                    memory_met = false;
                    faults = true;
                    all_worker_cpu_ids_verified = false;
                    let failure = serde_json::json!({"index":index,"repetition":repetition,"error":error,"phase":"fixture-capture-or-job","useful_trials_claimed":0,"load_before":load});
                    fs::write(directory.join(&filename), canonical(&failure)?)
                        .map_err(|e| e.to_string())?;
                    jobs.push(failure);
                }
            }
        }
    }
    if current_source_manifest()? != source_manifest {
        return Err("search source identity changed during benchmark".into());
    }
    latencies.sort_unstable();
    let p95 = if latencies.len() == 60 {
        latencies.get(56).copied()
    } else {
        None
    };
    let p95_met = p95.is_some_and(|n| n <= 3_000_000);
    let report = serde_json::json!({
        "status": "measurement-complete; independent replay pending",
        "build_id": BUILD_ID, "workload": search_roots::WORKLOAD,
        "attempted_jobs": 60, "reported_job_latencies":latencies.len(),
        "p95_elapsed_micros": p95, "p95_nearest_rank":57,
        "all_candidates_minimum_met": all_minimum,
        "all_hard_latency_met": hard_latency, "memory_met":memory_met,
        "engine_or_policy_or_job_faults":faults, "reference_cpu_verified":reference_cpu,
        "native_source_manifest_sha256":source_manifest_sha256,
        "source_identity_end_verified":true,
        "all_worker_cpu_ids_verified":all_worker_cpu_ids_verified,
        "numeric_gate_met":reference_cpu && all_worker_cpu_ids_verified && all_minimum && hard_latency && memory_met && p95_met && !faults,
        "jobs": jobs,
    });
    fs::write(directory.join("report.json"), canonical(&report)?).map_err(|e| e.to_string())?;
    Ok(report)
}

pub(super) fn replay(catalog: &Catalog, json: &str) -> Result<serde_json::Value, String> {
    let evidence: SearchEvidence = strict_read(json)?;
    if evidence.evidence_version != 1
        || evidence.kind != "trusted-search-fixture-evidence"
        || evidence.build_id != BUILD_ID
        || evidence.source_manifest != current_source_manifest()?
        || evidence.case != search_roots::RootCase::named(evidence.case.index)?
        || usize::try_from(evidence.report.request.root_index).ok() != Some(evidence.case.index)
        || evidence.report.request.public_job_seed != public_job_seed(evidence.case.index)
    {
        return Err("search fixture evidence identity mismatch".into());
    }
    let fixture = search_roots::prepare(catalog, evidence.case.index)?;
    if fixture.game.replay_journal() != evidence.root_journal
        || fixture.game.state_digest()? != evidence.root_state_sha256
    {
        return Err("search root is not the frozen accepted-input fixture".into());
    }
    let capture = search_roots::capture(&fixture.game, fixture.case.viewer)?;
    search_jobs::replay_report(catalog, &capture, &evidence.report)
}
