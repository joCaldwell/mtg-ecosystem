//! Private cfg(test) full-pod throughput diagnostic.
//! Same fixed18cases as released batch(); no new strategy or admission scope.
use super::*;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::PathBuf;

#[derive(Default, Serialize)]
struct PodClocks {
    admission_ns: u128,
    advance_ns: Vec<u128>,
    observation_query_ns: Vec<u128>,
    policy_ns: Vec<u128>,
    validation_query_ns: Vec<u128>,
    submit_ns: Vec<u128>,
    journal_serialization_ns: u128,
    journal_io_ns: u128,
    snapshot_envelope_ns: u128,
    final_state_hash_ns: u128,
    disposal_ns: u128,
    generation_ns: u128,
}
fn sample<T>(values: &mut Vec<u128>, f: impl FnOnce() -> T) -> T {
    let clock = Instant::now();
    let out = f();
    values.push(clock.elapsed().as_nanos());
    out
}
fn snapshot(game: &Game, nanos: &mut u128) -> Result<String, String> {
    let clock = Instant::now();
    let value = game.snapshot();
    *nanos += clock.elapsed().as_nanos();
    value
}
struct TimedJournal {
    file: fs::File,
}
impl TimedJournal {
    fn new(
        path: &Path,
        config: &MatchConfig,
        policy: Policy,
        mulligans: [u8; 4],
        clocks: &mut PodClocks,
    ) -> Result<Self, String> {
        let mut out = Self {
            file: fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(path)
                .map_err(|e| e.to_string())?,
        };
        out.append(&serde_json::json!({"kind":"header","build_id":BUILD_ID,"config":config,"policy":policy,"mulligans":mulligans}),clocks)?;
        Ok(out)
    }
    fn append(&mut self, value: &impl Serialize, clocks: &mut PodClocks) -> Result<(), String> {
        let clock = Instant::now();
        let mut line = canonical(value)?.into_bytes();
        line.push(b'\n');
        clocks.journal_serialization_ns += clock.elapsed().as_nanos();
        let clock = Instant::now();
        self.file.write_all(&line).map_err(|e| e.to_string())?;
        clocks.journal_io_ns += clock.elapsed().as_nanos();
        Ok(())
    }
}
fn fixed_case(index: usize) -> Result<(MatchConfig, [u8; 4]), String> {
    if index >= 18 {
        return Err("fixed pod index0..17 required".into());
    }
    let mut config = MatchConfig::foundations(seed(index as u32), PlayerId((index % 4) as u8));
    if index == 16 {
        config.decks = std::array::from_fn(|_| mtg_core::white_deck());
    }
    if index == 17 {
        config.decks = std::array::from_fn(|_| mtg_core::black_green_deck());
    }
    Ok((config, std::array::from_fn(|p| ((index + p) % 3) as u8)))
}
fn run_profiled(
    catalog: &Catalog,
    config: MatchConfig,
    policy: Policy,
    mulligans: [u8; 4],
    quantum: u64,
    stream: &Path,
) -> Result<(Evidence, PodClocks), String> {
    let mut clocks = PodClocks::default();
    let start = Instant::now();
    if quantum == 0 {
        return Err("positive match quantum required".into());
    }
    let admission = Instant::now();
    let prepared =
        Prepared::admit_pod(catalog.clone(), config.clone()).map_err(|e| format!("{e:?}"))?;
    clocks.admission_ns = admission.elapsed().as_nanos();
    let trace_mode = if config.seed == seed(0) || policy == Policy::DrawOnly {
        TraceMode::Detailed
    } else {
        TraceMode::Off
    };
    let mut game = Game::with_trace(prepared, trace_mode);
    let mut live_journal = TimedJournal::new(stream, &config, policy, mulligans, &mut clocks)?;
    let mut strategy = Strategy {
        catalog,
        policy,
        mulligans,
        beneficial: false,
        mana_plan: Default::default(),
        mana_color: None,
    };
    let selected_checkpoints = config.seed == seed(0) && policy == Policy::Develop;
    let mut checkpoints = vec![];
    if selected_checkpoints {
        checkpoints.push(Checkpoint {
            decisions: 0,
            rule_boundary: game.rule_boundary(),
            snapshot: snapshot(&game, &mut clocks.snapshot_envelope_ns)?,
        });
    }
    let mut survivor_counts = vec![4];

    let mut decisions = 0;
    let mut advances = 0;
    let mut work = 0;
    let mut query_calls = 0;
    let mut winner = None;
    let mut outcome = "advance_limit".to_string();
    while work < WORK_LIMIT {
        advances += 1;
        work += 1;
        let next = sample(&mut clocks.advance_ns, || {
            game.advance(Budget::new(quantum))
        });
        let living = game.live_players().len();
        if survivor_counts.last() != Some(&living) {
            survivor_counts.push(living)
        }
        match next {
            Advance::Suspended { .. } => continue,
            Advance::Finished { winner: p } => {
                winner = Some(p);
                outcome = "finished".into();
                break;
            }
            Advance::Draw => {
                outcome = "draw".into();
                break;
            }
            Advance::Fault(e) => {
                outcome = format!("fault: {e}");
                break;
            }
            Advance::AwaitingDecision(r) => {
                if selected_checkpoints
                    && ((checkpoints.len() == 1 && matches!(r, mtg_core::Request::Priority { .. }))
                        || (checkpoints.len() == 2 && game.turn().number >= 16))
                {
                    checkpoints.push(Checkpoint {
                        decisions,
                        rule_boundary: game.rule_boundary(),
                        snapshot: snapshot(&game, &mut clocks.snapshot_envelope_ns)?,
                    });
                }
                if decisions >= DECISION_LIMIT || game.turn().number > TURN_LIMIT {
                    outcome = "host_limit".into();
                    break;
                }
                let response_result = (|| -> Result<(), String> {
                    let observation = sample(&mut clocks.observation_query_ns, || {
                        observe(&game, r.player(), quantum, &mut work, &mut query_calls)
                    })?;
                    let response = sample(&mut clocks.policy_ns, || strategy.choose(&observation))
                        .map_err(|e| {
                            format!(
                                "turn{} decision{decisions} {:?}: {e}",
                                game.turn().number,
                                observation.decision
                            )
                        })?;
                    let token = observation
                        .decision
                        .as_ref()
                        .ok_or("missing decision token")?
                        .token()
                        .clone();
                    let validated = sample(&mut clocks.validation_query_ns, || {
                        complete(
                            &game,
                            || {
                                game.check_player_response(
                                    r.player(),
                                    token,
                                    response,
                                    Budget::new(quantum),
                                )
                            },
                            quantum,
                            &mut work,
                            &mut query_calls,
                        )
                    })?;
                    match validated {
                        QueryValue::Validated(receipt) => {
                            sample(&mut clocks.submit_ns, || game.submit(receipt))?
                        }
                        _ => return Err("wrong match receipt type".into()),
                    }
                    Ok(())
                })();
                if let Err(e) = response_result {
                    outcome = format!("policy_or_query_failure: {e}");
                    break;
                }
                live_journal.append(&serde_json::json!({"kind":"accepted","input":game.journal().last().ok_or("missing accepted journal entry")?}), &mut clocks)?;
                decisions += 1;
            }
        }
    }
    let clock = Instant::now();
    let final_state_sha256 = game.state_digest()?;
    clocks.final_state_hash_ns = clock.elapsed().as_nanos();
    if selected_checkpoints {
        checkpoints.push(Checkpoint {
            decisions,
            rule_boundary: game.rule_boundary(),
            snapshot: snapshot(&game, &mut clocks.snapshot_envelope_ns)?,
        });
    }
    live_journal.append(&serde_json::json!({"kind":"footer","outcome":outcome,"winner":winner,"state_sha256":final_state_sha256,"rule_boundary":game.rule_boundary()}), &mut clocks)?;
    let clock = Instant::now();
    live_journal.file.sync_data().map_err(|e| e.to_string())?;
    clocks.journal_io_ns += clock.elapsed().as_nanos();
    let survivors = game.live_players();
    let evidence = Evidence {
        config,
        policy,
        mulligans,
        journal: game.replay_journal(),
        trace_mode,
        losses: losses(&game),
        checkpoints,
        outcome,
        winner,
        decisions,
        advances,
        rule_steps: game.rule_boundary(),
        query_calls,
        work_limit: WORK_LIMIT,
        quantum,
        turns: game.turn().number,
        final_state_sha256,
        surviving_seats: survivors,
        survivor_counts,
        elapsed_millis: start.elapsed().as_millis(),
    };
    let clock = Instant::now();
    drop(live_journal);
    drop(game);
    clocks.disposal_ns = clock.elapsed().as_nanos();
    clocks.generation_ns = start.elapsed().as_nanos();
    Ok((evidence, clocks))
}

#[test]
fn current_pod_instrumentation_matches_unchanged_headless_run() {
    // Correctness only: index1 avoids the heavyweight seed0 checkpoint workload.
    // The measured batch below still includes seed0 and ALL18 unchanged cases.
    let catalog = mtg_compiler::match_fixture_catalog().unwrap();
    let (config, mulls) = fixed_case(1).unwrap();
    let reference = run(&catalog, config.clone(), Policy::Develop, mulls, 1024).unwrap();
    let directory = artifact_root().join(format!("pod-profile-guard-{}", std::process::id()));
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&directory)
        .unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(directory.clone());
    let (actual, clocks) = run_profiled(
        &catalog,
        config,
        Policy::Develop,
        mulls,
        1024,
        &directory.join("inputs.jsonl"),
    )
    .unwrap();
    verify_suite_entry(1, &actual).unwrap();
    verify_replay(&catalog, &actual).unwrap();
    let mut a = serde_json::to_value(&actual).unwrap();
    let mut b = serde_json::to_value(&reference).unwrap();
    a.as_object_mut().unwrap().remove("elapsed_millis");
    b.as_object_mut().unwrap().remove("elapsed_millis");
    assert_eq!(
        a, b,
        "all actual Evidence fields unchanged except host elapsed clock"
    );
    assert_eq!(clocks.advance_ns.len(), actual.advances);
    assert_eq!(clocks.submit_ns.len(), actual.decisions);
    assert_eq!(clocks.observation_query_ns.len(), actual.decisions);
    assert_eq!(clocks.policy_ns.len(), actual.decisions);
    assert_eq!(clocks.validation_query_ns.len(), actual.decisions);
    assert!(fixed_case(18).is_err());
}

#[test]
#[ignore = "explicit current18pod throughput host"]
fn current_fixed18_pod_throughput_diagnostic() {
    // Future host enforces120s/2GiB/onephysicalCPU and private fresh output.
    // A timeout leaves actual per-game streams/progress, never an18pod claim.
    let directory = PathBuf::from(std::env::var("MTG_CURRENT_POD_OUTPUT").unwrap());
    assert!(directory.is_absolute() && !directory.exists());
    assert_eq!(
        fs::metadata(directory.parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&directory)
        .unwrap();
    let clock = Instant::now();
    let catalog = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-match-ir.json"
    ))
    .unwrap();
    let catalog_decode_ns = clock.elapsed().as_nanos();
    let compiler_clock = Instant::now();
    assert_eq!(
        canonical(&catalog).unwrap(),
        canonical(&mtg_compiler::match_fixture_catalog().unwrap()).unwrap()
    );
    let compiler_verify_ns = compiler_clock.elapsed().as_nanos();
    let whole = Instant::now();
    let mut rows = vec![];
    let mut failures = vec![];
    let mut outcomes = BTreeMap::<String, usize>::new();
    let mut wins = [0usize; 4];
    let mut generation_ns = 0u128;
    let mut proof_ns = 0u128;
    let mut completed = 0usize;
    let mut reasons = vec![];
    for index in 0..18 {
        println!(
            "CURRENT_POD_START={}",
            canonical(&serde_json::json!({"index":index,"expected_pods":18,"build_id":BUILD_ID}))
                .unwrap()
        );
        let (config, mulls) = fixed_case(index).unwrap();
        let stream = directory.join(format!("game-{index:02}.jsonl"));
        let attempt_clock = Instant::now();
        match run_profiled(&catalog, config, Policy::Develop, mulls, 1024, &stream) {
            Err(error) => {
                let elapsed = attempt_clock.elapsed().as_nanos();
                generation_ns += elapsed;
                *outcomes.entry("failed_before_evidence".into()).or_default() += 1;
                let failure = serde_json::json!({"index":index,"phase":"before complete Evidence","error":error,"partial_stream":stream,"attempt_wall_ns":elapsed,"accepted_input_counts":"unknown until retained stream is independently read; never fabricated as zero"});
                println!("CURRENT_POD_FAILED={}", canonical(&failure).unwrap());
                rows.push(failure.clone());
                failures.push(failure);
            }
            Ok((e, clocks)) => {
                generation_ns += clocks.generation_ns;
                *outcomes.entry(e.outcome.clone()).or_default() += 1;
                if let Some(w) = e.winner {
                    wins[w.0 as usize] += 1;
                }
                let clock = Instant::now();
                let encoded = canonical(&e).unwrap();
                let evidence_encode_ns = clock.elapsed().as_nanos();
                let path = directory.join(format!("game-{index:02}.json"));
                let clock = Instant::now();
                let mut f = fs::OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .mode(0o600)
                    .open(&path)
                    .unwrap();
                f.write_all(encoded.as_bytes()).unwrap();
                f.sync_data().unwrap();
                let evidence_io_ns = clock.elapsed().as_nanos();
                let clock = Instant::now();
                let proof =
                    verify_suite_entry(index, &e).and_then(|()| verify_replay(&catalog, &e));
                let actual_proof_ns = clock.elapsed().as_nanos();
                proof_ns += actual_proof_ns;
                let terminal = matches!(e.outcome.as_str(), "finished" | "draw");
                if terminal && proof.is_ok() {
                    completed += 1;
                }
                if let Ok(losses) = &proof {
                    reasons.extend(losses.iter().map(|(_, reason)| *reason));
                }
                if !terminal || proof.is_err() {
                    failures.push(serde_json::json!({"index":index,"outcome":e.outcome,"replay_error":proof.as_ref().err()}));
                }
                let row = serde_json::json!({"index":index,"outcome":e.outcome,"winner":e.winner,"survivor_counts":e.survivor_counts,"turns":e.turns,"decisions":e.decisions,"advances":e.advances,"rule_steps":e.rule_steps,"query_calls":e.query_calls,"final_state_sha256":e.final_state_sha256,"journal_sha256":mtg_contracts::digest(&e.journal).unwrap(),"evidence_file":path,"evidence_sha256":sha256(encoded.as_bytes()),"clocks":clocks,"evidence_encode_ns":evidence_encode_ns,"evidence_io_ns":evidence_io_ns,"independent_replay_ns":actual_proof_ns,"verified":terminal&&proof.is_ok()});
                println!("CURRENT_POD_RESULT={}", canonical(&row).unwrap());
                rows.push(row);
            }
        }
    }
    let whole_ns = whole.elapsed().as_nanos();
    let loss_coverage = reasons.contains(&mtg_core::PlayerLossReason::CommanderDamage)
        && reasons.contains(&mtg_core::PlayerLossReason::Life);
    let passed = completed == 18 && rows.len() == 18 && failures.is_empty() && loss_coverage;
    let report = serde_json::json!({"schema":"current-fixed18-pod-throughput-v1","build_id":BUILD_ID,"passed":passed,"planned_pods":18,"completed_pods":completed,"outcome_histogram":outcomes,"winner_counts":wins,"failures":failures,"loss_reason_coverage":loss_coverage,"catalog_decode_ns":catalog_decode_ns,"compiler_verify_ns":compiler_verify_ns,"generation_sum_ns":generation_ns,"independent_proof_sum_ns":proof_ns,"whole_batch_ns":whole_ns,"rows":rows,"generation_pods_per_second":if passed{Some(18e9/generation_ns as f64)}else{None},"whole_batch_pods_per_second":if passed{Some(18e9/whole_ns as f64)}else{None},"timing_scope":"generation includes admitted setup, all policy/observe/validation/core transitions, live journal serialization/IO, required seed0 snapshots, finalStateSHA and Game disposal; proof/persist clocks separate. Whole batch includes all18 generation+independent replay+Evidence serialization/persistence, excludes separate catalog decode/source validation and final report persistence.","repetitions":"one predeclared heterogeneous18pod batch; no stable tail/confidence or parallel fullpod scaling claim","no_search_acceptance_or_strategy_quality_claim":true});
    let mut f = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(directory.join("report.json"))
        .unwrap();
    f.write_all(canonical(&report).unwrap().as_bytes()).unwrap();
    f.sync_data().unwrap();
    assert!(
        passed,
        "all18 outcomes/proofs/counts required; failures never removed from throughput denominator"
    );
}
