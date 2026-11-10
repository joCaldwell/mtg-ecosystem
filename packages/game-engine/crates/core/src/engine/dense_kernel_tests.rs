//! Private synthetic diagnostic domain, not a new executable profile.
//!
//! The complete operation contract below is frozen before collecting timings.
//! Every history entry is produced by actual accepted pass/discard work. No
//! State or visibility history is fabricated to claim restore/replay validity.
//! OS clocks, allocation interception and process RSS belong to a later host
//! measurement adapter, not production core or these semantic worklists.
use super::*;

const CONTRACT: &str = r#"{
  "schema":"synthetic-dense-kernel-v1",
  "public_admission":"must_reject_every_configuration",
  "rule_family_selector":"four-seat-combat-interactions-v1",
  "other_zone_cards":400,
  "other_zone_initial_hands_per_seat":7,
  "battlefield_layout":"id modulo4 equals3: archived Bears; otherwise archived Forest",
  "ownership":"card id modulo4; controller equals owner",
  "history":"actual passes/empty attacks/mandatory cleanup discards until every viewer has minimum entries and P0 holds Priority",
  "query_quantums":[1,64,18446744073709551615],
  "first_write":"P0 validates/submits/advances intrinsic Forest0 mana activation",
  "advance_quantums":[1,64,18446744073709551615],
  "oracles":["complete generic canonical State bytes and SHA","direct typed projection","archived base characteristics with no effects","same accepted input across advance partitions","normal snapshot restore and replay from private initial fixture"],
  "fixtures":[
    {"name":"ordinary","battlefield_objects":8,"minimum_history_entries":8},
    {"name":"moderate","battlefield_objects":32,"minimum_history_entries":32},
    {"name":"dense","battlefield_objects":256,"minimum_history_entries":8},
    {"name":"large_synthetic","battlefield_objects":1024,"minimum_history_entries":8}
  ],
  "maximum_setup_inputs":4096,
  "maximum_query_resumes":100000,
  "maximum_advance_calls":100000,
  "timings_collected":false,
  "legal_Commander_or_new_mechanic_claim":false
}"#;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureSpec {
    name: String,
    battlefield_objects: u32,
    minimum_history_entries: usize,
}

#[derive(Debug, Serialize)]
struct KernelProof {
    fixture: FixtureSpec,
    contract_sha256: String,
    synthetic_profile_digest: String,
    physical_cards: usize,
    initial_other_zone_cards: usize,
    root_other_zone_cards: usize,
    history_lengths: Vec<usize>,
    setup_accepted_inputs: usize,
    root_rule_steps: u64,
    root_state_sha256: String,
    state_serialized_bytes: usize,
    visible_rows_by_viewer: Vec<usize>,
    first_write_state_sha256: String,
    first_write_added_inputs: usize,
    public_admission_rejected: bool,
    snapshot_and_replay_verified: bool,
    timings_collected: bool,
}

fn specifications() -> Vec<FixtureSpec> {
    let value: serde_json::Value = serde_json::from_str(CONTRACT).unwrap();
    serde_json::from_value(value["fixtures"].clone()).unwrap()
}

fn private_prepared(spec: &FixtureSpec) -> Prepared {
    let catalog =
        Catalog::read(include_str!("../../../../tests/fixtures/expected-ir.json")).unwrap();
    // Borrow only this existing rules-family selector; the modified configuration
    // has its own synthetic identity and is explicitly rejected by public admit.
    let mut prepared =
        Prepared::admit(catalog.clone(), ScenarioConfig::combat_interactions()).unwrap();
    let mut config = ScenarioConfig::combat_interactions();
    config.commanders.clear();
    config.cards = (0..spec.battlefield_objects + 400)
        .map(|index| {
            let owner = PlayerId((index % 4) as u8);
            let public = index < spec.battlefield_objects;
            let hidden_index = index.saturating_sub(spec.battlefield_objects);
            CardSetup {
                id: CardId(index),
                oracle_id: if public && index % 4 == 3 {
                    BEARS
                } else {
                    FOREST
                }
                .into(),
                owner,
                controller: owner,
                zone: if public {
                    Zone::Battlefield
                } else if hidden_index < 28 {
                    Zone::Hand { owner }
                } else {
                    Zone::Library { owner }
                },
            }
        })
        .collect();
    assert!(Prepared::admit(catalog, config.clone()).is_err());
    prepared.profile_digest = digest(&(
        "synthetic-dense-kernel-v1",
        sha256(CONTRACT.as_bytes()),
        &config,
    ))
    .unwrap();
    prepared.config = config;
    assert!(prepared.match_config.is_none());
    prepared
}

fn ready(game: &mut Game, quantum: u64) -> Request {
    for _ in 0..100_000 {
        match game.advance(Budget::new(quantum)) {
            Advance::AwaitingDecision(request) => return request,
            Advance::Suspended { required_minimum } => assert!(required_minimum <= quantum),
            other => panic!("synthetic kernel advance failed: {other:?}"),
        }
    }
    panic!("bounded synthetic advance call limit")
}

fn fixture(spec: &FixtureSpec, mode: TraceMode) -> Game {
    let mut game = Game::with_trace(private_prepared(spec), mode);
    assert_eq!(
        game.state.rules.cards.len(),
        spec.battlefield_objects as usize + 400
    );
    assert_eq!(
        game.state
            .rules
            .libraries
            .values()
            .map(Vec::len)
            .sum::<usize>(),
        372
    );
    for _ in 0..4096 {
        let request = ready(&mut game, 64);
        if matches!(
            request,
            Request::Priority {
                player: PlayerId(0),
                ..
            }
        ) && game
            .state
            .knowledge
            .visibility
            .iter()
            .all(|k| k.history.len() >= spec.minimum_history_entries)
        {
            assert!(game.state.rules.effects.is_empty());
            assert!(game.state.rules.pending.is_none());
            assert_eq!(
                game.state
                    .rules
                    .objects
                    .values()
                    .filter(|o| o.zone == Zone::Battlefield)
                    .count(),
                spec.battlefield_objects as usize
            );
            game.validate_state()
                .expect("synthetic fixture State invariants");
            return game;
        }
        let response = match &request {
            Request::Priority { .. } => Response::Pass,
            Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
            Request::Blockers { .. } => Response::Blockers { blocks: vec![] },
            Request::Discard {
                count, candidates, ..
            } => Response::Discard {
                objects: candidates.iter().rev().take(*count).copied().collect(),
            },
            other => panic!("undeclared synthetic history request: {other:?}"),
        };
        game.respond(request.player(), request.decision_id(), response)
            .unwrap();
    }
    panic!("bounded synthetic history setup limit")
}

fn finish_query(game: &Game, mut query: QueryResult, quantum: u64) -> QueryValue {
    for _ in 0..100_000 {
        query = match query {
            QueryResult::Complete(value) => return value,
            QueryResult::Rejected(reason) => panic!("synthetic query rejected: {reason:?}"),
            QueryResult::Incomplete {
                work,
                required_minimum,
            } => {
                assert!(required_minimum <= quantum);
                game.resume_work(work, Budget::new(quantum))
            }
        };
    }
    panic!("bounded synthetic query call limit")
}

fn observe(game: &Game, viewer: PlayerId, quantum: u64) -> Observation {
    match finish_query(game, game.observe(viewer, Budget::new(quantum)), quantum) {
        QueryValue::Observation(value) => value,
        _ => panic!("unexpected query value"),
    }
}

fn direct_projection(game: &Game, viewer: PlayerId) -> Observation {
    // Independent whole-table assembly versus the optimized/resumable query
    // cursor. Share reviewed field projections, not query caches or Work state.
    let mut view = game.blank_view(viewer).unwrap();
    assert!(game.state.rules.effects.is_empty());
    assert!(game.state.rules.stack.is_empty());
    for player in 0..4 {
        view.players
            .push(game.visible_player(PlayerId(player)).unwrap());
    }
    for object in game.state.rules.objects.values() {
        if let Some(visible) = game.visible_object(viewer, object).unwrap() {
            view.objects.push(visible);
        }
    }
    view.objects
        .sort_by(|left, right| left.handle.0.cmp(&right.handle.0));
    Observation {
        observation_version: 16,
        view,
        decision: game.request_header(viewer).unwrap(),
    }
}

fn canonical_oracle(game: &Game) -> (String, usize) {
    let generic = canonical(&game.state).unwrap();
    let expected = sha256(generic.as_bytes());
    let cold_state: State = serde_json::from_str(&generic).unwrap();
    assert_eq!(cold_state, game.state);
    let mut cold_bytes = vec![];
    state_digest::write_state(&cold_state, &mut cold_bytes).unwrap();
    assert_eq!(
        cold_bytes,
        generic.as_bytes(),
        "cold complete canonical State bytes"
    );
    let mut warm_bytes = vec![];
    state_digest::write_state(&cold_state, &mut warm_bytes).unwrap();
    assert_eq!(
        warm_bytes, cold_bytes,
        "warm complete canonical State bytes"
    );
    assert_eq!(state_digest::state_digest(&cold_state).unwrap(), expected);
    assert_eq!(game.state_digest().unwrap(), expected);
    (expected, generic.len())
}

fn first_write(game: &Game, quantum: u64) -> Game {
    let mut branch = game.fork();
    let observation = observe(&branch, PlayerId(0), quantum);
    let Some(PlayerRequest::Priority { token, .. }) = observation.decision else {
        panic!("fixture root is not P0 Priority")
    };
    let object = branch.object_id(CardId(0)).unwrap();
    assert!(!branch.current(object).unwrap().tapped);
    assert_eq!(branch.mana_abilities(object).unwrap(), vec![Color::G]);
    let response = PlayerResponse::ActivateMana {
        object: branch.handle_for(PlayerId(0), object).unwrap(),
    };
    let QueryValue::Validated(receipt) = finish_query(
        &branch,
        branch.check_player_response(PlayerId(0), token, response, Budget::new(quantum)),
        quantum,
    ) else {
        panic!("activation did not produce a validated receipt")
    };
    branch.submit(receipt).unwrap();
    let request = ready(&mut branch, quantum);
    assert!(matches!(
        request,
        Request::Priority {
            player: PlayerId(0),
            ..
        }
    ));
    assert!(branch.current(object).unwrap().tapped);
    let mut expected_mana = game.state.rules.players[0]
        .mana
        .get(&Color::G)
        .cloned()
        .unwrap_or_default();
    expected_mana.add(&Quantity::from(1));
    assert_eq!(
        branch.state.rules.players[0].mana.get(&Color::G),
        Some(&expected_mana)
    );
    for (id, original) in &game.state.rules.objects {
        if *id != CardId(0) {
            assert_eq!(
                branch.state.rules.objects.get(id),
                Some(original),
                "first-write object footprint"
            );
        }
    }
    assert_eq!(branch.journal().len(), game.journal().len() + 1);
    assert_eq!(branch.state.rules.cards, game.state.rules.cards);
    assert_eq!(branch.state.rules.libraries, game.state.rules.libraries);
    assert_eq!(branch.state.rules.commanders, game.state.rules.commanders);
    if branch.trace.mode == TraceMode::Detailed {
        let added = &branch.events()[game.events().len()..];
        assert_eq!(
            added
                .iter()
                .filter(|event| matches!(event,
            Event::Committed { proposed: ProposedEvent::Tap { object: source }, .. }
                if *source == object))
                .count(),
            1
        );
        assert_eq!(added.iter().filter(|event| matches!(event,
            Event::Committed {
                proposed: ProposedEvent::AddMana { player: PlayerId(0), source, color: Color::G, amount },
                actual: EventOutcome::ManaAdded { .. },
            } if *source == object && *amount == Quantity::from(1))).count(), 1);
    }
    branch.validate_state().unwrap();
    branch
}

fn prove(spec: &FixtureSpec, mode: TraceMode) -> KernelProof {
    let game = fixture(spec, mode);
    let initial = game.state.clone();
    let (root_sha, serialized_bytes) = canonical_oracle(&game);
    work::optimization_tests::compare_queries(&game);
    let mut rows = vec![];
    for viewer in 0..4 {
        let viewer = PlayerId(viewer);
        let reference = direct_projection(&game, viewer);
        rows.push(reference.view.objects.len());
        for quantum in [1, 64, u64::MAX] {
            assert_eq!(observe(&game, viewer, quantum), reference);
        }
    }
    for object in game.state.rules.objects.values() {
        assert_eq!(
            game.characteristics(object.id).unwrap(),
            game.definition(object.id.card).unwrap().characteristics,
            "no-effect characteristic oracle from archived definition",
        );
    }
    assert_eq!(
        game.state, initial,
        "queries/memo initialization are semantically pure"
    );
    let reference = first_write(&game, u64::MAX);
    let (after_sha, _) = canonical_oracle(&reference);
    for quantum in [1, 64] {
        let branch = first_write(&game, quantum);
        assert_eq!(
            branch.state, reference.state,
            "budget-partition first-write result"
        );
        assert_eq!(branch.state_digest().unwrap(), after_sha);
        assert_eq!(game.state, initial, "sibling branch isolation");
    }
    for current in [&game, &reference] {
        let snapshot = current.snapshot().unwrap();
        let restored = Game::restore(current.prepared.as_ref().clone(), &snapshot).unwrap();
        assert_eq!(
            restored.state, current.state,
            "normal snapshot restore from actual private fixture history"
        );
        let replayed =
            Game::replay(current.prepared.as_ref().clone(), &current.replay_journal()).unwrap();
        assert_eq!(replayed.state, current.state, "full accepted-input replay");
    }
    KernelProof {
        fixture: spec.clone(),
        contract_sha256: sha256(CONTRACT.as_bytes()),
        synthetic_profile_digest: game.prepared.profile_digest.clone(),
        physical_cards: game.state.rules.cards.len(),
        initial_other_zone_cards: 400,
        root_other_zone_cards: game
            .state
            .rules
            .objects
            .values()
            .filter(|o| o.zone != Zone::Battlefield)
            .count(),
        history_lengths: game
            .state
            .knowledge
            .visibility
            .iter()
            .map(|k| k.history.len())
            .collect(),
        setup_accepted_inputs: game.journal().len(),
        root_rule_steps: game.rule_boundary(),
        root_state_sha256: root_sha,
        state_serialized_bytes: serialized_bytes,
        visible_rows_by_viewer: rows,
        first_write_state_sha256: after_sha,
        first_write_added_inputs: reference.journal().len() - game.journal().len(),
        public_admission_rejected: true,
        snapshot_and_replay_verified: true,
        timings_collected: false,
    }
}

#[test]
fn dense_kernel_contract_is_bounded_and_not_a_new_profile() {
    let specs = specifications();
    assert_eq!(
        specs
            .iter()
            .map(|s| s.battlefield_objects)
            .collect::<Vec<_>>(),
        [8, 32, 256, 1024]
    );
    for spec in &specs {
        let p = private_prepared(spec);
        assert!(Prepared::admit((*p.catalog).clone(), p.config.clone()).is_err());
        assert_eq!(
            p.config.cards.len(),
            spec.battlefield_objects as usize + 400
        );
    }
}

#[test]
fn dense_kernel_actual_operations_match_complete_oracles() {
    let proofs = specifications()
        .iter()
        .map(|spec| prove(spec, TraceMode::Off))
        .collect::<Vec<_>>();
    println!("DENSE_KERNEL_PROOFS={}", canonical(&proofs).unwrap());
}

#[test]
fn dense_kernel_trace_modes_preserve_same_actual_operation_state() {
    let spec = specifications().remove(0);
    let off = prove(&spec, TraceMode::Off);
    for mode in [TraceMode::Compact, TraceMode::Detailed] {
        let proof = prove(&spec, mode);
        assert_eq!(proof.root_state_sha256, off.root_state_sha256);
        assert_eq!(proof.first_write_state_sha256, off.first_write_state_sha256);
    }
}

/// Host-only, explicitly ignored diagnostic adapter. None of this module is
/// compiled into production core or changes profile/storage/resource limits.
mod diagnostic {
    use super::*;
    use std::{hint::black_box, time::Instant};

    const HOST_CONTRACT: &str = r#"{
      "schema":"synthetic-dense-diagnostic-v1",
      "kernel_contract":"synthetic-dense-kernel-v1",
      "case_environment":"DENSE_KERNEL_CASE",
      "outer_guard_declaration_environment":"DENSE_KERNEL_OUTER_GUARD=120s-2gib-v1",
      "outer_guard_verification":"host runner must prove watchdog/process-memory enforcement; declaration is not proof",
      "warmup_repetitions":1,
      "measured_repetitions":5,
      "query_quantum":64,
      "advance_quantum":64,
      "kernel_cases":[
        {"name":"ordinary","battlefield_objects":8,"minimum_history_entries":8},
        {"name":"moderate","battlefield_objects":32,"minimum_history_entries":32},
        {"name":"dense","battlefield_objects":256,"minimum_history_entries":8},
        {"name":"large_synthetic","battlefield_objects":1024,"minimum_history_entries":8},
        {"name":"dense_history","battlefield_objects":32,"minimum_history_entries":256},
        {"name":"dense_combined","battlefield_objects":256,"minimum_history_entries":256}
      ],
      "timed_segments":["observe_all_four","characteristics_all_objects","fork","check_mana_response","submit_mana_response","advance_mana_to_next_request","full_state_hash_cold","full_state_hash_warm","dispose_changed_branch"],
      "oracle_placement":"all generic/reference/snapshot/outcome validation outside segment stopwatches",
      "effects_counts":[0,8,64,256],
      "effects_scope":"domain evaluator only; no costs/priority/creation/admitted-state throughput claim",
      "retained_branch_counts":[64,256,1024,10000],
      "retained_branch_case":"ordinary",
      "retention_repetition_policy":"one warmup plus five measured trials for EACH retained size; release all branches between trials; identical semantic baseline; allocator arenas are not promised to reset",
      "retention_progress":"emit start and completion per declared trial so outer failures do not hide denominators",
      "retained_rss_hard_bytes":2147483648,
      "retained_rss_reserve_bytes":67108864,
      "allocation_scope":"external glibc whole-process intercepted APIs include setup, warmup, oracles, all declared work and reporting; never pure timed-segment allocations",
      "production_or_legal_admission_change":false,
      "segment_count_scope":"logical operation count; budget resumptions are included in elapsed time, not mislabeled as single API calls",
      "acceptance_latency_claim":false
    }"#;

    #[derive(Debug, Serialize)]
    struct Segment {
        name: String,
        repetition: usize,
        warmup: bool,
        nanos: u128,
        logical_operations: usize,
        operation_index: Option<usize>,
    }
    #[derive(Debug, Serialize)]
    struct Memory {
        rss_bytes: u64,
        high_water_bytes: u64,
    }
    fn memory() -> Memory {
        // Test host adapter only, not production core. Kernel counters include
        // fixture/oracle/collector overhead and all retained branch allocations.
        let status = std::fs::read_to_string("/proc/self/status").expect("Linux diagnostic RSS");
        let field = |name: &str| {
            status
                .lines()
                .find_map(|line| {
                    let value = line.strip_prefix(name)?;
                    value.split_whitespace().next()?.parse::<u64>().ok()
                })
                .expect("kernel memory counter")
                .checked_mul(1024)
                .expect("RSS byte conversion")
        };
        Memory {
            rss_bytes: field("VmRSS:"),
            high_water_bytes: field("VmHWM:"),
        }
    }
    fn measure<T>(
        name: &str,
        repetition: usize,
        calls: usize,
        samples: &mut Vec<Segment>,
        operation: impl FnOnce() -> T,
    ) -> T {
        let start = Instant::now();
        let result = black_box(operation());
        let nanos = start.elapsed().as_nanos();
        samples.push(Segment {
            name: name.into(),
            repetition,
            warmup: repetition == 0,
            nanos,
            logical_operations: calls,
            operation_index: None,
        });
        result
    }
    #[derive(Default, Debug, Serialize)]
    struct CallCounts {
        initial_queries: u64,
        query_resumptions: u64,
        submits: u64,
        advance_calls: u64,
        committed_rule_steps: u64,
    }
    fn counted_query(game: &Game, mut query: QueryResult, counts: &mut CallCounts) -> QueryValue {
        counts.initial_queries += 1;
        for _ in 0..100_000 {
            query = match query {
                QueryResult::Complete(value) => return value,
                QueryResult::Rejected(reason) => panic!("diagnostic query: {reason:?}"),
                QueryResult::Incomplete { work, .. } => {
                    counts.query_resumptions += 1;
                    game.resume_work(work, Budget::new(64))
                }
            };
        }
        panic!("diagnostic query resource cap")
    }
    fn counted_observe(game: &Game, viewer: PlayerId, counts: &mut CallCounts) -> Observation {
        match counted_query(game, game.observe(viewer, Budget::new(64)), counts) {
            QueryValue::Observation(value) => value,
            _ => panic!("diagnostic observation result"),
        }
    }
    fn counted_advance(game: &mut Game, counts: &mut CallCounts) -> Request {
        let before = game.rule_boundary();
        for _ in 0..100_000 {
            counts.advance_calls += 1;
            match game.advance(Budget::new(64)) {
                Advance::AwaitingDecision(request) => {
                    counts.committed_rule_steps += game.rule_boundary() - before;
                    return request;
                }
                Advance::Suspended { .. } => {}
                other => panic!("diagnostic advance: {other:?}"),
            }
        }
        panic!("diagnostic advance resource cap")
    }

    fn named_case(name: &str) -> FixtureSpec {
        let mut cases = specifications();
        cases.extend([
            FixtureSpec {
                name: "dense_history".into(),
                battlefield_objects: 32,
                minimum_history_entries: 256,
            },
            FixtureSpec {
                name: "dense_combined".into(),
                battlefield_objects: 256,
                minimum_history_entries: 256,
            },
        ]);
        cases
            .into_iter()
            .find(|case| case.name == name)
            .expect("declared case name")
    }

    fn kernel_case(spec: &FixtureSpec) -> serde_json::Value {
        // Construction and all reference proofs are deliberately untimed.
        let oracle = prove(spec, TraceMode::Off);
        let game = fixture(spec, TraceMode::Off);
        let generic = canonical(&game.state).unwrap();
        let expected_sha = sha256(generic.as_bytes());
        let expected_first = first_write(&game, u64::MAX);
        let expected_first_sha = expected_first.state_digest().unwrap();
        let reference_views = (0..4)
            .map(|viewer| direct_projection(&game, PlayerId(viewer)))
            .collect::<Vec<_>>();
        let ids = game
            .state
            .rules
            .objects
            .values()
            .map(|object| object.id)
            .collect::<Vec<_>>();
        let reference_characteristics = ids
            .iter()
            .map(|id| game.definition(id.card).unwrap().characteristics.clone())
            .collect::<Vec<_>>();
        let observation = observe(&game, PlayerId(0), 64);
        let Some(PlayerRequest::Priority { token, .. }) = observation.decision else {
            panic!("P0 Priority")
        };
        let object = game.object_id(CardId(0)).unwrap();
        let response = PlayerResponse::ActivateMana {
            object: game.handle_for(PlayerId(0), object).unwrap(),
        };
        let mut samples = vec![];
        let mut work_counts = vec![];
        for repetition in 0..6 {
            let mut counts = CallCounts::default();
            let views = measure("observe_all_four", repetition, 4, &mut samples, || {
                (0..4)
                    .map(|viewer| counted_observe(&game, PlayerId(viewer), &mut counts))
                    .collect::<Vec<_>>()
            });
            assert_eq!(views, reference_views);
            let characteristics = measure(
                "characteristics_all_objects",
                repetition,
                ids.len(),
                &mut samples,
                || {
                    ids.iter()
                        .map(|id| game.characteristics(*id).unwrap())
                        .collect::<Vec<_>>()
                },
            );
            assert_eq!(characteristics, reference_characteristics);
            let mut branch = measure("fork", repetition, 1, &mut samples, || game.fork());
            assert_eq!(branch.state, game.state);
            // The source token and handle are identical in this fresh branch;
            // executor authority is supplied only by its own query receipt.
            let validation = measure("check_mana_response", repetition, 1, &mut samples, || {
                counted_query(
                    &branch,
                    branch.check_player_response(
                        PlayerId(0),
                        token.clone(),
                        response.clone(),
                        Budget::new(64),
                    ),
                    &mut counts,
                )
            });
            let QueryValue::Validated(receipt) = validation else {
                panic!("validated mana response")
            };
            let submitted = measure("submit_mana_response", repetition, 1, &mut samples, || {
                counts.submits += 1;
                branch.submit(receipt)
            });
            submitted.unwrap();
            let next = measure(
                "advance_mana_to_next_request",
                repetition,
                1,
                &mut samples,
                || counted_advance(&mut branch, &mut counts),
            );
            assert!(matches!(
                next,
                Request::Priority {
                    player: PlayerId(0),
                    ..
                }
            ));
            assert_eq!(branch.state, expected_first.state);
            assert_eq!(branch.state_digest().unwrap(), expected_first_sha);
            let cold: State = serde_json::from_str(&generic).unwrap();
            let cold_sha = measure("full_state_hash_cold", repetition, 1, &mut samples, || {
                state_digest::state_digest(&cold)
            });
            assert_eq!(cold_sha.unwrap(), expected_sha);
            let warm_sha = measure("full_state_hash_warm", repetition, 1, &mut samples, || {
                state_digest::state_digest(&cold)
            });
            assert_eq!(warm_sha.unwrap(), expected_sha);
            measure(
                "dispose_changed_branch",
                repetition,
                1,
                &mut samples,
                || drop(branch),
            );
            assert_eq!(game.state_digest().unwrap(), expected_sha);
            work_counts.push(counts);
        }
        serde_json::json!({"case":spec,"oracle":oracle,"samples":samples,"work_counts_by_repetition":work_counts,
            "memory_including_setup_and_oracles":memory(),"timed_oracle_work":false,
            "lives":game.state.rules.players.iter().map(|player| player.life.clone()).collect::<Vec<_>>(),
            "commanders":0,"physical_cards":game.state.rules.cards.len(),"other_zone_cards":400,
            "all_segment_inputs_and_results_verified":true})
    }

    fn effect_case(collect_timings: bool) -> serde_json::Value {
        let catalog = Catalog::read(include_str!(
            "../../../../tests/fixtures/expected-effects-ir.json"
        ))
        .unwrap();
        let prepared = Prepared::admit(catalog, ScenarioConfig::effects()).unwrap();
        let base = Game::new(prepared);
        let source_card = base
            .prepared
            .config
            .cards
            .iter()
            .find(|card| card.oracle_id == GROWTH)
            .unwrap()
            .id;
        let source = base.object_id(source_card).unwrap();
        let instruction = base
            .definition(source_card)
            .unwrap()
            .spell
            .as_ref()
            .unwrap()
            .instructions[0]
            .clone();
        let (power, toughness, source_block) = match &instruction {
            Instruction::ModifyPowerToughness {
                power,
                toughness,
                source_block,
                ..
            } => (power.clone(), toughness.clone(), *source_block),
            _ => panic!("archived complete signed P/T instruction"),
        };
        let targets = base
            .state
            .rules
            .objects
            .values()
            .filter(|object| {
                object.zone == Zone::Battlefield
                    && base
                        .definition(object.id.card)
                        .unwrap()
                        .characteristics
                        .kind
                        == CardKind::Creature
            })
            .map(|object| object.id)
            .collect::<Vec<_>>();
        assert!(!targets.is_empty());
        let mut rows = vec![];
        for count in [0usize, 8, 64, 256] {
            let mut domain = base.fork();
            for index in 0..count {
                let target = targets[index % targets.len()];
                let source_handles = domain.capture_trigger_source_handles(source).unwrap();
                let target_handles = domain.capture_trigger_source_handles(target).unwrap();
                domain.state.rules.effects.push(ResolvedEffect {
                    id: EffectId(index as u64),
                    origin: EffectOrigin {
                        mode: None,
                        stack_id: StackId(u32::try_from(index).unwrap()),
                        source,
                        controller: base.current(source).unwrap().controller,
                        source_handles,
                        cursor: 0,
                        source_block,
                    },
                    target_handles,
                    expires_in_cleanup_of_turn: base.state.rules.turn.number,
                    kind: ResolvedEffectKind::ModifyPowerToughness {
                        target,
                        power: power.clone(),
                        toughness: toughness.clone(),
                    },
                });
            }
            domain.state.rules.next_effect_id = count as u64;
            // These rows deliberately have no invocation history. Even the
            // below-capacity seeded row is domain-only, never a legal restore.
            let restore = Game::restore(
                domain.prepared.as_ref().clone(),
                &domain.snapshot().unwrap(),
            );
            assert_eq!(restore.is_err(), count != 0);
            let mut synthetic_config = domain.prepared.config.clone();
            synthetic_config.profile = format!("synthetic-evaluator-only-{count}");
            assert!(Prepared::admit((*domain.prepared.catalog).clone(), synthetic_config).is_err());
            let actual_limit = domain.effect_limit();
            if count > actual_limit {
                assert_eq!(
                    domain.validate_effect_state().unwrap_err(),
                    "effect capacity"
                );
            }
            let expected = targets
                .iter()
                .enumerate()
                .map(|(slot, target)| {
                    let mut value = base
                        .definition(target.card)
                        .unwrap()
                        .characteristics
                        .clone();
                    let occurrences =
                        count / targets.len() + usize::from(slot < count % targets.len());
                    for _ in 0..occurrences {
                        value.power.as_mut().unwrap().add(&power);
                        value.toughness.as_mut().unwrap().add(&toughness);
                    }
                    value
                })
                .collect::<Vec<_>>();
            let mut samples = vec![];
            let result = targets
                .iter()
                .map(|target| domain.characteristics(*target).unwrap())
                .collect::<Vec<_>>();
            assert_eq!(result, expected);
            for repetition in 0..if collect_timings { 6 } else { 0 } {
                let result = measure(
                    "pure_characteristics_modifier_sweep",
                    repetition,
                    targets.len(),
                    &mut samples,
                    || {
                        targets
                            .iter()
                            .map(|target| domain.characteristics(*target).unwrap())
                            .collect::<Vec<_>>()
                    },
                );
                assert_eq!(result, expected);
            }
            rows.push(serde_json::json!({"effect_instances":count,"actual_fixture_capacity":actual_limit,
                "released_match_capacity":32,"domain_evaluator_only":true,"supported_invocation_creation_claim":false,
                "production_restore_rejected":count!=0,"synthetic_profile_admission_rejected":true,
                "targets":targets.len(),"source_instruction":&instruction,"samples":samples,
                "memory":memory()}));
        }
        serde_json::json!({"case":"effects","rows":rows})
    }

    fn mana_in_place(branch: &mut Game) -> (Request, CallCounts) {
        let mut counts = CallCounts::default();
        let observation = counted_observe(branch, PlayerId(0), &mut counts);
        let Some(PlayerRequest::Priority { token, .. }) = observation.decision else {
            panic!("P0 Priority")
        };
        let object = branch.object_id(CardId(0)).unwrap();
        let response = PlayerResponse::ActivateMana {
            object: branch.handle_for(PlayerId(0), object).unwrap(),
        };
        let QueryValue::Validated(receipt) = counted_query(
            branch,
            branch.check_player_response(PlayerId(0), token, response, Budget::new(64)),
            &mut counts,
        ) else {
            panic!("validated retained branch mana input")
        };
        counts.submits += 1;
        branch.submit(receipt).unwrap();
        (counted_advance(branch, &mut counts), counts)
    }

    fn retained_case() -> serde_json::Value {
        const LIMIT: u64 = 2 * 1024 * 1024 * 1024;
        const RESERVE: u64 = 64 * 1024 * 1024;
        let spec = named_case("ordinary");
        let game = fixture(&spec, TraceMode::Off);
        let (sha, _) = canonical_oracle(&game);
        let expected = first_write(&game, u64::MAX);
        let mut rows = vec![];
        for repetition in 0..6 {
            for target in [64usize, 256, 1024, 10000] {
                // Progress records are outside stopwatches. An outer timeout or
                // failure leaves an explicit started-but-unfinished denominator.
                println!(
                    "DENSE_RETAINED_SWEEP_START={}",
                    canonical(&serde_json::json!({
                        "repetition":repetition,"warmup":repetition==0,"requested":target,
                        "baseline_state_sha256":sha
                    }))
                    .unwrap()
                );
                let before_retention = memory();
                let lifecycle = Instant::now();
                let mut branches = Vec::with_capacity(target);
                let mut samples = vec![];
                let mut resource_stop = None;
                for index in 0..target {
                    if memory().rss_bytes >= LIMIT - RESERVE {
                        resource_stop = Some("rss_reserve_before_next_fork");
                        break;
                    }
                    branches.push(measure(
                        "retained_fork",
                        repetition,
                        1,
                        &mut samples,
                        || game.fork(),
                    ));
                    samples.last_mut().unwrap().operation_index = Some(index);
                    if memory().rss_bytes >= LIMIT {
                        resource_stop = Some("rss_hard_limit_observed_after_fork");
                        break;
                    }
                }
                let retained = branches.len();
                let selected = [0, retained / 2, retained.saturating_sub(1)]
                    .into_iter()
                    .filter(|index| *index < retained)
                    .collect::<BTreeSet<_>>();
                let mut selected_work_counts = vec![];
                if resource_stop != Some("rss_hard_limit_observed_after_fork") {
                    for index in selected {
                        let branch = &mut branches[index];
                        assert_eq!(branch.state, game.state);
                        let (request, counts) = measure(
                            "retained_selected_first_write",
                            repetition,
                            1,
                            &mut samples,
                            || mana_in_place(branch),
                        );
                        samples.last_mut().unwrap().operation_index = Some(index);
                        assert!(matches!(
                            request,
                            Request::Priority {
                                player: PlayerId(0),
                                ..
                            }
                        ));
                        assert_eq!(branch.state, expected.state);
                        selected_work_counts
                            .push(serde_json::json!({"index":index,"counts":counts}));
                    }
                }
                let at_retention = memory();
                measure(
                    "dispose_retained_branches",
                    repetition,
                    retained,
                    &mut samples,
                    || drop(branches),
                );
                let lifecycle_nanos = lifecycle.elapsed().as_nanos();
                // Full oracle validation is outside both production-segment and
                // inclusive sweep clocks. The inclusive sweep still contains
                // memory guards and per-selected-branch field assertions.
                assert_eq!(game.state_digest().unwrap(), sha);
                let row = serde_json::json!({"requested":target,"retained":retained,
                    "repetition":repetition,"warmup":repetition==0,
                    "requested_count_reached":retained==target,
                    "accepted_full_size_sample":retained==target&&resource_stop.is_none(),
                    "resource_shortfall":resource_stop,
                    "baseline_state_sha256":sha,
                    "memory_before_retention":before_retention,"memory_at_retention":at_retention,
                    "memory_after_disposal":memory(),"samples":samples,
                    "selected_first_write_counts":selected_work_counts,
                    "sweep_lifecycle_nanos_including_guards_and_field_assertions":lifecycle_nanos});
                println!("DENSE_RETAINED_SWEEP_RESULT={}", canonical(&row).unwrap());
                rows.push(row);
                // Even a resource shortfall remains in every fixed trial/size
                // denominator. All branches are gone before the next trial.
            }
        }
        serde_json::json!({"case":"retained_branches","fixture":spec,"rows":rows,
            "declared_total_sweeps":24,"warmup_sweeps":4,"measured_sweeps":20,
            "individual_fork_samples_are_not_independent_sweep_repetitions":true,
            "no_tail_precision_claim_from_five_lifecycle_samples":true,
            "internal_rss_guard_is_not_outer_process_cap_proof":true,
            "rss_difference_is_not_exact_live_heap_allocation":true,
            "resource_shortfall_is_not_proof_target_can_never_fit":true})
    }

    #[test]
    fn dense_diagnostic_contract_is_explicit_and_no_auto_measurement() {
        let value: serde_json::Value = serde_json::from_str(HOST_CONTRACT).unwrap();
        assert_eq!(value["schema"], "synthetic-dense-diagnostic-v1");
        assert_eq!(value["kernel_cases"].as_array().unwrap().len(), 6);
        assert_eq!(value["effects_counts"], serde_json::json!([0, 8, 64, 256]));
        assert_eq!(
            value["retained_branch_counts"],
            serde_json::json!([64, 256, 1024, 10000])
        );
    }

    #[test]
    fn dense_extended_history_correctness_has_no_timing() {
        for name in ["dense_history", "dense_combined"] {
            let proof = prove(&named_case(name), TraceMode::Off);
            assert!(proof.history_lengths.iter().all(|length| *length >= 256));
            assert!(proof.snapshot_and_replay_verified);
        }
    }

    #[test]
    fn dense_modifier_evaluator_reference_has_no_timing() {
        let result = effect_case(false);
        assert_eq!(result["rows"].as_array().unwrap().len(), 4);
        for row in result["rows"].as_array().unwrap() {
            assert!(row["samples"].as_array().unwrap().is_empty());
        }
    }

    #[test]
    #[ignore = "explicit host-only profiling/timing; requires external watchdog and RSS cap"]
    fn dense_kernel_diagnostic_host_adapter() {
        assert_eq!(
            std::env::var("DENSE_KERNEL_OUTER_GUARD").unwrap(),
            "120s-2gib-v1"
        );
        let name = std::env::var("DENSE_KERNEL_CASE").expect("one declared case per process");
        let result = match name.as_str() {
            "effects" => effect_case(true),
            "retained_branches" => retained_case(),
            other => kernel_case(&named_case(other)),
        };
        println!("DENSE_DIAGNOSTIC={}", canonical(&serde_json::json!({
            "schema":"synthetic-dense-diagnostic-result-v1","build_id":BUILD_ID,
            "contract":serde_json::from_str::<serde_json::Value>(HOST_CONTRACT).unwrap(),
            "contract_sha256":sha256(HOST_CONTRACT.as_bytes()),"result":result,
            "outer_guard_declared_not_verified_here":true,
            "allocation_interception_scope":"whole fixed process includes fixture/oracle/setup/warmup/reporting overhead; no pure segment allocation claim",
            "acceptance_latency_claim":false,"legal_match_admission_claim":false
        })).unwrap());
    }
}

#[path = "dense_interaction_tests.rs"]
mod interactions;
