//! Private persistent-worker diagnostic under cfg(test) search_jobs::fixed_work.
//! No product worker, horizon, candidate, sampler or evaluator change.
use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc::{self, Receiver, SyncSender};

const PERSISTENT_REPETITIONS: usize = 6; // one warmup, then five measured runs

// Failure injection is private to this already cfg(test)-only diagnostic.
// Lifecycle bitmasks independently observe actual entry/exit/join of each lane;
// no timeout/sleep is used to decide whether cleanup happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum InjectFailure {
    #[default]
    None,
    SpawnBefore(usize),
    StartupError(usize),
    StartupPanic(usize),
    JobPanic {
        worker: usize,
        generation: usize,
    },
    ReplyDisconnected {
        worker: usize,
        generation: usize,
    },
    ControllerEarlyError(usize),
}
#[derive(Default)]
struct Lifecycle {
    created: AtomicU64,
    entered: AtomicU64,
    exited: AtomicU64,
    joined: AtomicU64,
}
impl Lifecycle {
    fn mark(counter: &AtomicU64, worker: usize) {
        counter.fetch_or(1u64 << worker, Ordering::SeqCst);
    }
    fn assert_all_joined(&self, expected: u64) {
        assert_eq!(self.created.load(Ordering::SeqCst), expected);
        assert_eq!(self.entered.load(Ordering::SeqCst), expected);
        assert_eq!(self.exited.load(Ordering::SeqCst), expected);
        assert_eq!(self.joined.load(Ordering::SeqCst), expected);
    }
}
struct WorkerLifetime {
    lifecycle: Arc<Lifecycle>,
    worker: usize,
}
impl WorkerLifetime {
    fn enter(lifecycle: Arc<Lifecycle>, worker: usize) -> Self {
        Lifecycle::mark(&lifecycle.entered, worker);
        Self { lifecycle, worker }
    }
}
impl Drop for WorkerLifetime {
    fn drop(&mut self) {
        Lifecycle::mark(&self.lifecycle.exited, self.worker);
    }
}

struct WorkerRun {
    worlds: Vec<WorldResult>,
    measurement: WorkerMeasurement,
    strategy_operations: u64,
}
enum WorkerEvent {
    Ready(Result<(), String>),
    Complete {
        generation: usize,
        result: Result<WorkerRun, String>,
    },
}
struct RunCommand {
    generation: usize,
    indices: Vec<usize>,
}
#[derive(Serialize)]
struct PersistentScheduleResult {
    startup_micros: u128,
    shutdown_micros: u128,
    lifecycle_micros: u128,
    schedules: Vec<FixedScheduleResult>,
}

fn initialize_worker(cpu_id: Option<usize>) -> Result<(), String> {
    if let Some(id) = cpu_id {
        if !core_affinity::set_for_current(core_affinity::CoreId { id }) {
            return Err(format!("persistent affinity failed for CPU{id}"));
        }
        let actual = core_affinity::get_core_ids().ok_or("persistent affinity readback")?;
        if actual.len() != 1 || actual[0].id != id {
            return Err("persistent pinned CPU readback differs".into());
        }
    }
    Ok(())
}

// Aggregation is deliberately ordinary data work, not a different rollout.
// Normal correctness compares the COMPLETE result with unchanged execute().
fn assemble_schedule(
    work: &FrozenWork,
    workers: usize,
    work_sha256: &str,
    elapsed_micros: u128,
    results: Vec<WorkerRun>,
) -> Result<FixedScheduleResult, String> {
    let mut worlds = vec![];
    let mut measurements = vec![];
    let mut strategy_operations = 0u64;
    for result in results {
        worlds.extend(result.worlds);
        measurements.push(result.measurement);
        strategy_operations = strategy_operations
            .checked_add(result.strategy_operations)
            .ok_or("persistent operation count overflow")?;
    }
    worlds.sort_by_key(|w| w.world_index);
    measurements.sort_by_key(|w| w.worker_index);
    if worlds.iter().map(|w| w.world_index).collect::<Vec<_>>()
        != work
            .worlds
            .iter()
            .map(|w| w.world_index)
            .collect::<Vec<_>>()
    {
        return Err("persistent missing/duplicate world result".into());
    }
    let trials = worlds
        .iter()
        .flat_map(|w| w.trials.iter().cloned())
        .collect::<Vec<_>>();
    let semantics = FixedSemantics {
        work_sha256: work_sha256.into(),
        world_build_calls: worlds
            .iter()
            .map(|w| (w.world_index, w.build_calls))
            .collect(),
        trials: trials.iter().map(SemanticTrial::from).collect(),
        aggregates: aggregates(&work.candidate_policy.candidates, &trials),
    };
    let complete_useful = trials.len()
        == work.worlds.len() * work.candidate_policy.candidates.len()
        && trials.iter().all(|t| t.result.useful())
        && semantics.aggregates.iter().all(|a| a.minimum_met);
    Ok(FixedScheduleResult {
        workers,
        worker_measurements: measurements,
        worker_faults: vec![],
        worlds,
        semantics,
        elapsed_micros,
        complete_useful,
        strategy_operations,
    })
}

fn execute_persistent(
    catalog: &Catalog,
    capture: &SearchCapture,
    work: &FrozenWork,
    workers: usize,
    cpu_ids: Option<&[usize]>,
    orders: &[Vec<usize>],
) -> Result<PersistentScheduleResult, String> {
    execute_persistent_injected(
        catalog,
        capture,
        work,
        workers,
        cpu_ids,
        orders,
        InjectFailure::None,
        Arc::new(Lifecycle::default()),
    )
}

#[allow(clippy::too_many_arguments)]
fn execute_persistent_injected(
    catalog: &Catalog,
    capture: &SearchCapture,
    work: &FrozenWork,
    workers: usize,
    cpu_ids: Option<&[usize]>,
    orders: &[Vec<usize>],
    injected: InjectFailure,
    lifecycle_proof: Arc<Lifecycle>,
) -> Result<PersistentScheduleResult, String> {
    if orders.is_empty() || orders.len() > PERSISTENT_REPETITIONS {
        return Err("persistent requires one to six predeclared complete schedules".into());
    }
    for order in orders {
        validate_order(work, workers, order)?;
    }
    if work.context_id != capture.context_id() || work.horizon != HORIZON {
        return Err("persistent work belongs to another capture or horizon".into());
    }
    if let Some(ids) = cpu_ids {
        validate_affinity(ids, workers)?;
    }
    let work_sha256 = work.digest()?;
    let lifecycle = Instant::now();
    thread::scope(|scope| {
        let mut inputs: Vec<SyncSender<RunCommand>> = vec![];
        let mut outputs: Vec<Receiver<WorkerEvent>> = vec![];
        let mut handles = vec![];
        // Never return from the scope before closing every input and joining
        // every created thread. One outstanding command/lane, one result/lane;
        // output capacity1 therefore also covers error-path shutdown.
        let outcome = (|| -> Result<PersistentScheduleResult, String> {
            let startup = Instant::now();
            for worker_index in 0..workers {
                if injected == InjectFailure::SpawnBefore(worker_index) {
                    return Err("injected persistent partial spawn failure".into());
                }
                let (send, receive) = mpsc::sync_channel::<RunCommand>(1);
                let (reply, result) = mpsc::sync_channel::<WorkerEvent>(1);
                let cpu_id = cpu_ids.map(|ids| ids[worker_index]);
                let worker_lifecycle = Arc::clone(&lifecycle_proof);
                let handle = thread::Builder::new()
                    .name(format!("persistent-fixed-{worker_index}"))
                    .spawn_scoped(scope, move || {
                        let _lifetime = WorkerLifetime::enter(worker_lifecycle, worker_index);
                        if injected == InjectFailure::StartupPanic(worker_index) {
                            panic!("injected persistent startup panic");
                        }
                        let ready = if injected == InjectFailure::StartupError(worker_index) {
                            Err("injected persistent startup error".into())
                        } else {
                            initialize_worker(cpu_id)
                        };
                        let failed = ready.is_err();
                        if reply.send(WorkerEvent::Ready(ready)).is_err() || failed {
                            return;
                        }
                        while let Ok(command) = receive.recv() {
                            let generation = command.generation;
                            if injected
                                == (InjectFailure::ReplyDisconnected {
                                    worker: worker_index,
                                    generation,
                                })
                            {
                                // Close this worker's reply and task receiver naturally.
                                // Controller must close/join every peer on disconnection.
                                return;
                            }
                            let result = catch_unwind(AssertUnwindSafe(|| {
                                if injected
                                    == (InjectFailure::JobPanic {
                                        worker: worker_index,
                                        generation,
                                    })
                                {
                                    panic!("injected persistent job panic");
                                }
                                let started = Instant::now();
                                // Fresh per-run control/counts and fresh worlds;
                                // the only persistent resource is the OS worker.
                                let control = no_deadline();
                                let mut worlds = vec![];
                                let mut completed = vec![];
                                for index in command.indices {
                                    let result = world_job(
                                        catalog,
                                        capture,
                                        work.worlds[index].proposal.clone(),
                                        &work.candidate_policy.candidates,
                                        &control,
                                        false,
                                    );
                                    completed.push(result.world_index);
                                    worlds.push(result);
                                }
                                WorkerRun {
                                    worlds,
                                    measurement: WorkerMeasurement {
                                        worker_index,
                                        cpu_id,
                                        completed_world_indices: completed,
                                        elapsed_micros: started.elapsed().as_micros(),
                                    },
                                    strategy_operations: control.operations.load(Ordering::SeqCst),
                                }
                            }))
                            .map_err(|_| format!("persistent worker{worker_index} panicked"));
                            if reply
                                .send(WorkerEvent::Complete { generation, result })
                                .is_err()
                            {
                                return;
                            }
                        }
                    })
                    .map_err(|e| format!("persistent worker spawn:{e}"))?;
                Lifecycle::mark(&lifecycle_proof.created, worker_index);
                inputs.push(send);
                outputs.push(result);
                handles.push((worker_index, handle));
            }
            for output in &outputs {
                match output
                    .recv()
                    .map_err(|_| "persistent startup disconnected")?
                {
                    WorkerEvent::Ready(result) => result?,
                    _ => return Err("persistent unexpected startup result".into()),
                }
            }
            let startup_micros = startup.elapsed().as_micros();
            let mut schedules = vec![];
            for (generation, order) in orders.iter().enumerate() {
                println!("PERSISTENT_SCHEDULE_START={}", canonical(&serde_json::json!({"generation":generation,"world_order":order,"workers":workers})).unwrap());
                let started = Instant::now();
                for (worker, input) in inputs.iter().enumerate() {
                    input
                        .send(RunCommand {
                            generation,
                            indices: order
                                .iter()
                                .skip(worker)
                                .step_by(workers)
                                .copied()
                                .collect(),
                        })
                        .map_err(|_| "persistent command disconnected")?;
                }
                if injected == InjectFailure::ControllerEarlyError(generation) {
                    return Err("injected persistent controller early error".into());
                }
                let mut results = vec![];
                for output in &outputs {
                    match output
                        .recv()
                        .map_err(|_| "persistent result disconnected")?
                    {
                        WorkerEvent::Complete {
                            generation: actual,
                            result,
                        } if actual == generation => results.push(result?),
                        _ => return Err("persistent stale/duplicate result generation".into()),
                    }
                }
                // Aggregation is INSIDE steady-state schedule elapsed, matching
                // unchanged execute() except its spawn/pin/join lifecycle.
                let mut row = assemble_schedule(work, workers, &work_sha256, 0, results)?;
                row.elapsed_micros = started.elapsed().as_micros();
                println!(
                    "PERSISTENT_SCHEDULE_RESULT={}",
                    canonical(&serde_json::json!({"generation":generation,"complete_row":row}))
                        .unwrap()
                );
                schedules.push(row);
            }
            Ok(PersistentScheduleResult {
                startup_micros,
                shutdown_micros: 0,
                lifecycle_micros: 0,
                schedules,
            })
        })();
        let shutdown = Instant::now();
        drop(inputs); // before ANY join, including partial spawn/error paths
        let mut join_failed = false;
        for (worker_index, handle) in handles {
            if handle.join().is_err() {
                join_failed = true;
            }
            Lifecycle::mark(&lifecycle_proof.joined, worker_index);
        }
        let shutdown_micros = shutdown.elapsed().as_micros();
        if join_failed {
            return Err("persistent worker join panic".into());
        }
        let mut report = outcome?;
        report.shutdown_micros = shutdown_micros;
        report.lifecycle_micros = lifecycle.elapsed().as_micros();
        Ok(report)
    })
}

#[test]
fn persistent_same_work_matches_unchanged_execute_in_one_two_four_workers() {
    let (catalog, capture) = fixture(0);
    let work = freeze(&catalog, &capture, public_seed(0), 8).unwrap();
    let normal = (0..work.worlds.len()).collect::<Vec<_>>();
    let reverse = normal.iter().rev().copied().collect::<Vec<_>>();
    let reference = execute(&catalog, &capture, &work, 1, None, &normal).unwrap();
    assert!(reference.complete_useful);
    for workers in [1, 2, 4] {
        let actual = execute_persistent(
            &catalog,
            &capture,
            &work,
            workers,
            None,
            &[normal.clone(), reverse.clone(), normal.clone()],
        )
        .unwrap();
        assert_eq!(actual.schedules.len(), 3);
        for row in actual.schedules {
            assert!(row.complete_useful);
            assert_eq!(row.semantics, reference.semantics);
            assert_eq!(row.strategy_operations, reference.strategy_operations);
        }
    }
    assert!(execute_persistent(&catalog, &capture, &work, 1, None, &[]).is_err());
    assert!(execute_persistent(&catalog, &capture, &work, 1, None, &[vec![0]]).is_err());
}

#[test]
fn persistent_failure_paths_close_inputs_and_join_every_created_worker() {
    let (catalog, capture) = fixture(0);
    let work = freeze(&catalog, &capture, public_seed(0), 8).unwrap();
    let order = (0..work.worlds.len()).collect::<Vec<_>>();
    let cases = [
        (InjectFailure::SpawnBefore(0), 0, "partial spawn"),
        (InjectFailure::SpawnBefore(2), 0b0011, "partial spawn"),
        (InjectFailure::StartupError(0), 0b1111, "startup error"),
        (InjectFailure::StartupError(3), 0b1111, "startup error"),
        (InjectFailure::StartupPanic(0), 0b1111, "join panic"),
        (InjectFailure::StartupPanic(3), 0b1111, "join panic"),
        (
            InjectFailure::JobPanic {
                worker: 0,
                generation: 0,
            },
            0b1111,
            "panicked",
        ),
        (
            InjectFailure::JobPanic {
                worker: 3,
                generation: 0,
            },
            0b1111,
            "panicked",
        ),
        (
            InjectFailure::ReplyDisconnected {
                worker: 0,
                generation: 0,
            },
            0b1111,
            "disconnected",
        ),
        (
            InjectFailure::ReplyDisconnected {
                worker: 3,
                generation: 0,
            },
            0b1111,
            "disconnected",
        ),
        (
            InjectFailure::ControllerEarlyError(0),
            0b1111,
            "controller early",
        ),
    ];
    for (injected, created_mask, expected_error) in cases {
        let lifecycle = Arc::new(Lifecycle::default());
        let error = execute_persistent_injected(
            &catalog,
            &capture,
            &work,
            4,
            None,
            std::slice::from_ref(&order),
            injected,
            Arc::clone(&lifecycle),
        )
        .err()
        .expect("injected failure must never return a successful schedule");
        assert!(error.contains(expected_error), "{injected:?}: {error}");
        lifecycle.assert_all_joined(created_mask);
    }
}

#[test]
fn persistent_later_generation_failure_joins_reused_workers() {
    let (catalog, capture) = fixture(0);
    let work = freeze(&catalog, &capture, public_seed(0), 8).unwrap();
    let order = (0..work.worlds.len()).collect::<Vec<_>>();
    let lifecycle = Arc::new(Lifecycle::default());
    let result = execute_persistent_injected(
        &catalog,
        &capture,
        &work,
        2,
        None,
        &[order.clone(), order],
        InjectFailure::ControllerEarlyError(1),
        Arc::clone(&lifecycle),
    );
    assert!(result.err().unwrap().contains("controller early"));
    lifecycle.assert_all_joined(0b11);
}

#[test]
#[ignore = "explicit persistent pool host120s/2GiB row"]
fn fixed_work_persistent_diagnostic_row() {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let output = std::path::PathBuf::from(std::env::var("MTG_FIXED_SEARCH_OUTPUT").unwrap());
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
        .unwrap()
        .parse()
        .unwrap();
    let cpu_ids: Vec<usize> =
        serde_json::from_str(&std::env::var("MTG_FIXED_SEARCH_CPUS").unwrap()).unwrap();
    let workers = cpu_ids.len();
    let available = physical_cpu_ids().unwrap();
    assert!([1, 2, 4, available.len()].contains(&workers));
    assert_eq!(
        std::env::var("MTG_FIXED_SEARCH_WORLDS").as_deref(),
        Ok("32")
    );
    let (catalog, capture) = fixture(root);
    let work = freeze(&catalog, &capture, public_seed(root), 32).unwrap();
    let order = (0..work.worlds.len()).collect::<Vec<_>>();
    let reference = execute(&catalog, &capture, &work, 1, Some(&cpu_ids[..1]), &order).unwrap();
    assert!(reference.complete_useful);
    let rows = execute_persistent(
        &catalog,
        &capture,
        &work,
        workers,
        Some(&cpu_ids),
        &vec![order; PERSISTENT_REPETITIONS],
    )
    .unwrap();
    for row in &rows.schedules {
        assert!(row.complete_useful);
        assert_eq!(row.semantics, reference.semantics);
        assert_eq!(row.strategy_operations, reference.strategy_operations);
    }
    let report = canonical(&serde_json::json!({
        "schema":"fixed-persistent-pool-diagnostic-v1","build_id":mtg_core::BUILD_ID,
        "root":root,"frozen_work":work,"reference":reference.semantics,
        "worker_cpu_ids":cpu_ids,"available_physical_cpu_ids":available,
        "warmups":1,"measured_repeats":5,"rows":rows,
        "startup_excluded_from_steady_clock":true,"shutdown_excluded_from_steady_clock":true,
        "steady_scope":"dispatch+fresh world builds+all existing trials/queries/StateSHA/disposal+results+sorted aggregation; existing256 horizon unchanged",
        "whole_host_still_includes_startup_oracle_warmup_audit_shutdown":true,
        "no_product_latency_claim":true
    })).unwrap();
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(output)
        .unwrap();
    file.write_all(report.as_bytes()).unwrap();
    file.sync_data().unwrap();
}
