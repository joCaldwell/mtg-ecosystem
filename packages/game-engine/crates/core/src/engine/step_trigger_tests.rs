//! Source-backed private scenarios plus compositional IR boundary tests. These
//! are not additional hosted/search profiles or claims of legal pod admission.
use super::*;
const ARENA: &str = "ee579a32-a048-4335-b966-231ba731cdea";
const RAM: &str = "5a20113c-7ea8-4edf-af9f-148ccd326a88";
const FORCE: &str = "8add6996-6c5c-4aa2-93d4-a39efcfd22b2";
const BALEFUL: &str = "f26fa9e4-f528-4e22-8f7c-329fc3b633e4";
const SWAMP: &str = "56719f6a-1a6c-4c0a-8d21-18f7d7350b68";
fn catalog() -> Catalog {
    let mut c = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap();
    let add = Catalog::read(include_str!(
        "../../../../tests/fixtures/cards-data-d2c/expected-ir.json"
    ))
    .unwrap();
    let next = Catalog::read(include_str!(
        "../../../../tests/fixtures/cards-data-d2c/expected-d2c-b-ir.json"
    ))
    .unwrap();
    c.definitions.extend(next.definitions);
    c.schema_version = 22;
    c.definitions.extend(add.definitions);
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    c
}
fn prepared(sources: &[(&str, u8, u8, Zone)], step: TurnStep) -> Prepared {
    let c = catalog();
    let mut config = ScenarioConfig::turns_permanents();
    config.profile = "step-program-domain-witness".into();
    config.initial.turn_step = step;
    config.cards.clear();
    for (id, (oracle, owner, controller, zone)) in sources.iter().enumerate() {
        config.cards.push(CardSetup {
            id: CardId(id as u32),
            oracle_id: (*oracle).into(),
            owner: PlayerId(*owner),
            controller: PlayerId(*controller),
            zone: *zone,
        });
    }
    for seat in 0..4 {
        for _ in 0..8 {
            config.cards.push(CardSetup {
                id: CardId(config.cards.len() as u32),
                oracle_id: SWAMP.into(),
                owner: PlayerId(seat),
                controller: PlayerId(seat),
                zone: Zone::Library {
                    owner: PlayerId(seat),
                },
            });
        }
    }
    Prepared {
        catalog_digest: digest(&c).unwrap(),
        catalog: Arc::new(c),
        profile_digest: digest(&config).unwrap(),
        config,
        match_config: None,
    }
}
fn settle(g: &mut Game) -> Request {
    for _ in 0..500 {
        match g.advance(Budget::new(1)) {
            Advance::AwaitingDecision(r) => return r,
            Advance::Suspended { .. } => (),
            other => panic!("step witness at {:?}: {other:?}", g.state.rules.frame),
        }
    }
    panic!("step witness budget")
}
fn answer(g: &mut Game, response: Response) {
    let r = settle(g);
    g.respond(r.player(), r.decision_id(), response).unwrap();
}
fn passive(g: &mut Game) {
    let r = settle(g);
    let (player, decision_id) = (r.player(), r.decision_id());
    let response = match r {
        Request::Priority { .. } => Response::Pass,
        Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
        Request::Blockers { .. } => Response::Blockers { blocks: vec![] },
        Request::OrderTriggers { candidates, .. } => Response::OrderTriggers { order: candidates },
        Request::Discard {
            count, candidates, ..
        } => Response::Discard {
            objects: candidates.into_iter().take(count).collect(),
        },
        other => panic!("unexpected passive request {other:?}"),
    };
    g.respond(player, decision_id, response).unwrap();
}
fn recover(g: &Game) {
    let restored = Game::restore(g.prepared.as_ref().clone(), &g.snapshot().unwrap()).unwrap();
    assert!(restored.state == g.state);
    assert_eq!(restored.trace, g.trace);
    let replayed = Game::replay_to(
        g.prepared.as_ref().clone(),
        &g.replay_journal(),
        Some(g.state.rules.rule_steps),
        g.trace.mode,
    )
    .unwrap();
    assert!(replayed.state == g.state);
    assert_eq!(replayed.trace, g.trace);
}
fn to_step(g: &mut Game, number: u64, step: TurnStep) {
    for _ in 0..500 {
        let _ = settle(g);
        if g.state.rules.turn.number == number && g.state.rules.turn.step == step {
            return;
        }
        passive(g);
    }
    panic!("missing step {number}/{step:?}")
}
fn placed(g: &mut Game) {
    loop {
        match settle(g) {
            Request::Priority { .. } => return,
            Request::OrderTriggers { candidates, .. } => {
                answer(g, Response::OrderTriggers { order: candidates })
            }
            r => panic!("placement {r:?}"),
        }
    }
}
fn resolve(g: &mut Game) {
    for _ in 0..4 {
        answer(g, Response::Pass);
    }
    let _ = settle(g);
}
#[test]
fn d2c_source_cast_then_wrong_and_correct_upkeep_ordered_draw_life_all_traces() {
    let mut final_states = vec![];
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        let mut p = prepared(
            &[(ARENA, 0, 0, Zone::Hand { owner: PlayerId(0) })],
            TurnStep::PrecombatMain,
        );
        for id in 1..=3 {
            p.config.cards[id].zone = Zone::Battlefield;
        }
        p.profile_digest = digest(&p.config).unwrap();
        let mut g = Game::with_trace(p, mode);
        answer(&mut g, Response::Cast { card: CardId(0) });
        for id in 1..=3 {
            let object = g.object_id(CardId(id)).unwrap();
            answer(&mut g, Response::ActivateMana { object });
        }
        answer(&mut g, Response::FinishMana { undo_mana: false });
        g.complete_test_mana();
        resolve(&mut g);
        assert_eq!(g.state.rules.objects[&CardId(0)].zone, Zone::Battlefield);
        assert!(g.state.rules.stack.is_empty());
        assert_eq!(g.state.rules.next_trigger_id, 0);
        to_step(&mut g, 2, TurnStep::Upkeep);
        assert_eq!(
            g.state.rules.next_trigger_id, 0,
            "wrong controller's upkeep"
        );
        to_step(&mut g, 5, TurnStep::Upkeep);
        assert_eq!(g.state.rules.stack.len(), 1);
        assert_eq!(g.state.rules.next_trigger_id, 1);
        let top_card = g.state.rules.libraries[&PlayerId(0)][0];
        let mut life = g.state.rules.players[0].life.clone();
        life.subtract(&1.into());
        recover(&g);
        resolve(&mut g);
        assert_eq!(g.state.rules.players[0].life, life);
        assert_eq!(
            g.state.rules.objects[&top_card].zone,
            Zone::Hand { owner: PlayerId(0) }
        );
        if mode == TraceMode::Detailed {
            let ops = g
                .trace
                .records
                .iter()
                .filter_map(|r| {
                    if let EvidenceRecord::Batch { batch } = &r.record {
                        if matches!(batch.cause, BatchCause::Instruction { .. }) {
                            Some(&batch.occurrences)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .flatten()
                .map(|o| &o.actual)
                .collect::<Vec<_>>();
            assert!(matches!(ops[ops.len() - 2], EventOutcome::Moved { .. }));
            assert!(matches!(ops[ops.len() - 1], EventOutcome::LifeLost { .. }));
        }
        recover(&g);
        final_states.push(g.state);
    }
    assert_eq!(final_states[0], final_states[1]);
    assert_eq!(final_states[1], final_states[2]);
}
#[test]
fn d2c_collect_once_before_entry_restore_every_frame_and_apnap_ordering() {
    let p = prepared(
        &[
            (FORCE, 0, 0, Zone::Battlefield),
            (BALEFUL, 1, 1, Zone::Battlefield),
            (RAM, 0, 0, Zone::Battlefield),
            (FORCE, 3, 3, Zone::Battlefield),
        ],
        TurnStep::Untap,
    );
    let mut g = Game::new(p);
    for _ in 0..4 {
        answer(&mut g, Response::Pass);
    }
    let mut entered = false;
    let mut ordered = false;
    for _ in 0..80 {
        if matches!(g.state.rules.frame, Frame::EnterStep) {
            assert_eq!(g.state.rules.pending_triggers.len(), 4);
            assert!(g.state.rules.stack.is_empty());
            entered = true;
        }
        recover(&g);
        match g.advance(Budget::new(1)) {
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(Request::OrderTriggers {
                candidates, player, ..
            }) => {
                assert_eq!(player, PlayerId(0));
                ordered = true;
                answer(
                    &mut g,
                    Response::OrderTriggers {
                        order: candidates.into_iter().rev().collect(),
                    },
                );
            }
            Advance::AwaitingDecision(Request::Priority { .. })
                if g.state.rules.turn.step == TurnStep::Upkeep =>
            {
                break;
            }
            other => panic!("APNAP boundary {other:?}"),
        }
    }
    assert!(entered && ordered);
    assert_eq!(g.state.rules.next_trigger_id, 4);
    assert_eq!(
        g.state
            .rules
            .stack
            .iter()
            .map(|s| s.controller)
            .collect::<Vec<_>>(),
        vec![PlayerId(0), PlayerId(0), PlayerId(1), PlayerId(3)]
    );
    assert_eq!(
        g.state
            .rules
            .stack
            .iter()
            .take(2)
            .map(|s| s.source.card)
            .collect::<Vec<_>>(),
        vec![CardId(2), CardId(0)]
    );
    recover(&g);
    for _ in 0..4 {
        resolve(&mut g);
    }
    assert_eq!(g.state.rules.next_trigger_id, 4);
    recover(&g);
    for event in g.events() {
        if let Event::TriggerCollected {
            occurrence,
            witness,
            ..
        } = event
        {
            assert!(witness.subject().is_none());
            assert!(g.trace.records.iter().any(|r| matches!(&r.record, EvidenceRecord::Batch { batch } if batch.occurrences.iter().any(|o| TriggerOccurrenceId::Batch(o.id.clone()) == *occurrence && matches!(o.actual, EventOutcome::TurnTransitioned { .. })))));
        }
    }
}
#[test]
fn d2c_draw_trigger_waits_for_normal_draw_and_commander_replacement() {
    let mut p = prepared(&[(ARENA, 0, 0, Zone::Battlefield)], TurnStep::Upkeep);
    let c = Arc::make_mut(&mut p.catalog);
    c.definitions
        .iter_mut()
        .find(|d| d.oracle_id == ARENA)
        .unwrap()
        .triggered[0]
        .condition = TriggerCondition::StepBegan {
        step: BeginningStep::Draw,
        turn_relation: TurnRelation::Controller,
    };
    c.validate().unwrap();
    p.catalog_digest = digest(c).unwrap();
    let commander = p.config.cards[1].id;
    p.config.cards[1].oracle_id = YARGLE.into();
    p.config.commanders.push(commander);
    p.profile_digest = digest(&p.config).unwrap();
    let mut g = Game::new(p);
    for _ in 0..4 {
        answer(&mut g, Response::Pass);
    }
    loop {
        if matches!(g.state.rules.frame, Frame::EnterStep) {
            assert_eq!(g.state.rules.pending_triggers.len(), 1);
            recover(&g);
            break;
        }
        assert!(matches!(
            g.advance(Budget::new(1)),
            Advance::Suspended { .. }
        ));
    }
    let r = settle(&mut g);
    assert!(matches!(r, Request::CommanderReturn { .. }));
    assert!(g.state.rules.stack.is_empty());
    assert_eq!(g.state.rules.pending_triggers.len(), 1);
    recover(&g);
    answer(&mut g, Response::CommanderReturn { to_command: true });
    placed(&mut g);
    assert_eq!(
        g.state.rules.objects[&commander].zone,
        Zone::Command { owner: PlayerId(0) }
    );
    assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), 7);
    assert_eq!(g.state.rules.stack.len(), 1);
    recover(&g);
    resolve(&mut g);
    assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), 6);
}
#[test]
fn d2c_empty_library_and_lethal_life_finish_program_before_sbas() {
    for empty in [false, true] {
        let mut p = prepared(&[(ARENA, 0, 0, Zone::Battlefield)], TurnStep::Untap);
        if empty {
            p.config
                .cards
                .retain(|c| c.owner != PlayerId(0) || c.zone == Zone::Battlefield);
        } else {
            p.config.initial.life[0] = 1.into();
        }
        p.profile_digest = digest(&p.config).unwrap();
        let mut g = Game::new(p);
        to_step(&mut g, 1, TurnStep::Upkeep);
        assert_eq!(g.state.rules.stack.len(), 1);
        resolve(&mut g);
        assert!(!g.is_live(PlayerId(0)));
        assert_eq!(
            g.state.rules.players[0].last_known.as_ref().unwrap().life,
            if empty { 39.into() } else { 0.into() }
        );
        recover(&g);
    }
}
#[test]
fn d2c_collected_source_departure_preserves_frozen_controller() {
    let mut g = Game::new(prepared(
        &[(ARENA, 1, 0, Zone::Battlefield)],
        TurnStep::Untap,
    ));
    for _ in 0..4 {
        answer(&mut g, Response::Pass);
    }
    while !matches!(g.state.rules.frame, Frame::EnterStep) {
        assert!(matches!(
            g.advance(Budget::new(1)),
            Advance::Suspended { .. }
        ));
    }
    assert_eq!(g.state.rules.pending_triggers.len(), 1);
    g.concede(PlayerId(1)).unwrap();
    placed(&mut g);
    assert!(!g.state.rules.objects.contains_key(&CardId(0)));
    assert_eq!(g.state.rules.stack[0].controller, PlayerId(0));
    recover(&g);
    resolve(&mut g);
    assert_eq!(g.state.rules.players[0].life, 39.into());
    recover(&g);
}
#[test]
fn d2c_controller_departure_discards_collected_trigger() {
    let mut g = Game::new(prepared(
        &[(ARENA, 1, 0, Zone::Battlefield)],
        TurnStep::Untap,
    ));
    for _ in 0..4 {
        answer(&mut g, Response::Pass);
    }
    while !matches!(g.state.rules.frame, Frame::EnterStep) {
        assert!(matches!(
            g.advance(Budget::new(1)),
            Advance::Suspended { .. }
        ));
    }
    g.concede(PlayerId(0)).unwrap();
    placed(&mut g);
    assert!(g.state.rules.pending_triggers.is_empty() && g.state.rules.stack.is_empty());
    recover(&g);
}
#[test]
fn d2c_no_active_player_unqualified_steps_only_and_next_live_priority() {
    let mut p = prepared(
        &[
            (FORCE, 1, 1, Zone::Battlefield),
            (RAM, 2, 2, Zone::Battlefield),
            (ARENA, 3, 3, Zone::Battlefield),
        ],
        TurnStep::Untap,
    );
    let c = Arc::make_mut(&mut p.catalog);
    c.definitions
        .iter_mut()
        .find(|d| d.oracle_id == ARENA)
        .unwrap()
        .triggered[0]
        .condition = TriggerCondition::StepBegan {
        step: BeginningStep::Upkeep,
        turn_relation: TurnRelation::Opponent,
    };
    p.catalog_digest = digest(c).unwrap();
    let mut g = Game::new(p);
    g.concede(PlayerId(0)).unwrap();
    to_step(&mut g, 1, TurnStep::Upkeep);
    assert_eq!(g.state.rules.active, None);
    assert_eq!(g.state.rules.priority, PlayerId(1));
    assert_eq!(g.state.rules.next_trigger_id, 1);
    assert_eq!(g.state.rules.stack[0].source.card, CardId(0));
    resolve(&mut g);
    assert_eq!(g.state.rules.players[1].life, 43.into());
    recover(&g);
}
#[test]
fn d2c_rehashed_pending_stack_and_trace_witness_forgeries_reject() {
    let mut g = Game::new(prepared(
        &[(ARENA, 0, 0, Zone::Battlefield)],
        TurnStep::Untap,
    ));
    for _ in 0..4 {
        answer(&mut g, Response::Pass);
    }
    while !matches!(g.state.rules.frame, Frame::EnterStep) {
        assert!(matches!(
            g.advance(Budget::new(1)),
            Advance::Suspended { .. }
        ));
    }
    recover(&g);
    for placed_already in [false, true] {
        if placed_already {
            placed(&mut g);
        }
        for field in ["step", "number", "owner", "active", "event", "unknown"] {
            let mut snapshot: serde_json::Value =
                serde_json::from_str(&g.snapshot().unwrap()).unwrap();
            let trigger = if placed_already {
                &mut snapshot["state"]["stack"][0]["kind"]
            } else {
                &mut snapshot["state"]["pending_triggers"][0]
            };
            match field {
                "step" => trigger["witness"]["turn"]["step"] = "draw".into(),
                "number" => trigger["witness"]["turn"]["number"] = 99.into(),
                "owner" => trigger["witness"]["owner"] = 3.into(),
                "active" => trigger["witness"]["active"] = serde_json::Value::Null,
                "event" => trigger["event"]["path"] = serde_json::json!([99]),
                _ => trigger["witness"]["subject"] = serde_json::json!({"card":0,"incarnation":0}),
            }
            snapshot["state_sha256"] = digest(&snapshot["state"]).unwrap().into();
            assert!(
                Game::restore(g.prepared.as_ref().clone(), &snapshot.to_string()).is_err(),
                "{field}/{placed_already}"
            );
        }
    }
    let mut snapshot: serde_json::Value = serde_json::from_str(&g.snapshot().unwrap()).unwrap();
    fn forge(v: &mut serde_json::Value) -> bool {
        match v {
            serde_json::Value::Object(m) => {
                if m.contains_key("witness") && m.contains_key("source_facts") {
                    m["witness"]["turn"]["number"] = 99.into();
                    return true;
                }
                m.values_mut().any(forge)
            }
            serde_json::Value::Array(a) => a.iter_mut().any(forge),
            _ => false,
        }
    }
    assert!(forge(&mut snapshot["trace"]));
    assert!(Game::restore(g.prepared.as_ref().clone(), &snapshot.to_string()).is_err());
}
#[test]
fn d2c_failed_transition_rolls_back_turn_mana_handles_and_collection() {
    let mut g = Game::new(prepared(
        &[(FORCE, 0, 0, Zone::Battlefield)],
        TurnStep::Untap,
    ));
    g.state.rules.frame = Frame::Transition;
    g.state.rules.next_trigger_id = u64::MAX;
    g.state.rules.players[0].mana.insert(Color::B, 2.into());
    let token = g.state_token.clone();
    let before = g.fork();
    assert!(g.transition_turn_step().is_err());
    assert_eq!(g.state, before.state);
    assert_eq!(g.trace, before.trace);
    assert!(Arc::ptr_eq(&g.state_token, &token));
}
#[test]
fn d2c_new_capability_is_unreviewed_for_hosting_and_search() {
    let profile = match_profile("commander-foundations-v1").unwrap();
    let c = catalog();
    for (search, hosted, feature) in [
        (true, false, Feature::Search),
        (false, true, Feature::Hosting),
    ] {
        let mut probe = profile.clone();
        probe.search = search;
        probe.hosted = hosted;
        assert!(
            matches!(probe.validate_requested_features(&c),Err(ProfileError::UnreviewedCapabilities { feature:f,capabilities }) if f==feature && capabilities.contains(&Capability::StepBeginningTriggerV1))
        );
    }
}

#[test]
fn d2c_opponent_end_step_and_graveyard_sources_use_independent_predicates() {
    let mut p = prepared(
        &[
            (RAM, 0, 0, Zone::Battlefield),
            (FORCE, 1, 1, Zone::Graveyard { owner: PlayerId(1) }),
        ],
        TurnStep::PostcombatMain,
    );
    let c = Arc::make_mut(&mut p.catalog);
    c.definitions
        .iter_mut()
        .find(|d| d.oracle_id == RAM)
        .unwrap()
        .triggered[0]
        .condition = TriggerCondition::StepBegan {
        step: BeginningStep::End,
        turn_relation: TurnRelation::Opponent,
    };
    c.validate().unwrap();
    p.catalog_digest = digest(c).unwrap();
    let mut g = Game::new(p);
    to_step(&mut g, 1, TurnStep::End);
    assert_eq!(g.state.rules.next_trigger_id, 0);
    to_step(&mut g, 2, TurnStep::Upkeep);
    assert_eq!(
        g.state.rules.next_trigger_id, 0,
        "graveyard source is inactive"
    );
    to_step(&mut g, 2, TurnStep::End);
    assert_eq!(g.state.rules.next_trigger_id, 1);
    recover(&g);
    resolve(&mut g);
    assert_eq!(g.state.rules.players[0].life, 41.into());
    recover(&g);
}
#[test]
fn d2c_step_trigger_composes_tokens_and_targeted_programs_on_ordinary_artifact() {
    for name in ["Dragon Trainer", "Ravenous Chupacabra"] {
        let mut p = prepared(
            &[
                (ARENA, 0, 0, Zone::Battlefield),
                (RAM, 1, 1, Zone::Battlefield),
            ],
            TurnStep::Untap,
        );
        let c = Arc::make_mut(&mut p.catalog);
        let program = c
            .definitions
            .iter()
            .find(|d| d.name == name)
            .unwrap()
            .triggered[0]
            .program
            .clone();
        let d = c
            .definitions
            .iter_mut()
            .find(|d| d.oracle_id == ARENA)
            .unwrap();
        d.characteristics.kind = CardKind::Artifact;
        d.characteristics.subtypes.clear();
        d.triggered[0].program = program;
        d.capabilities = d.requirements();
        c.validate().unwrap();
        p.catalog_digest = digest(c).unwrap();
        let mut g = Game::new(p);
        for _ in 0..4 {
            answer(&mut g, Response::Pass);
        }
        let request = settle(&mut g);
        if name == "Ravenous Chupacabra" {
            assert!(matches!(request, Request::Targets { .. }));
            // The opposing Ram is a legal printed creature target.
            let source = g.object_id(CardId(1)).unwrap();
            answer(
                &mut g,
                Response::Targets {
                    objects: vec![source],
                },
            );
        }
        placed(&mut g);
        recover(&g);
        resolve(&mut g);
        if name == "Dragon Trainer" {
            assert_eq!(g.state.rules.tokens.len(), 1);
        } else {
            assert!(matches!(
                g.state.rules.objects[&CardId(1)].zone,
                Zone::Graveyard { .. }
            ));
        }
        recover(&g);
    }
}
#[test]
fn d2c_stack_capacity_fault_rolls_back_transition_and_typed_collection() {
    let mut g = Game::new(prepared(
        &[(
            "d26b27db-a567-4631-b4b6-7294222fbdd1",
            0,
            0,
            Zone::Battlefield,
        )],
        TurnStep::Upkeep,
    ));
    g.state.rules.frame = Frame::Transition;
    g.transition_turn_step().unwrap();
    let t = g.state.rules.pending_triggers[0].clone();
    g.state
        .rules
        .pending_triggers
        .resize(capacities::STACK_LIMIT, t);
    // Re-run a valid supported boundary with the full pending collection. The
    // private domain setup isolates the shared capacity guard, not reachability.
    g.state.rules.turn.step = TurnStep::Upkeep;
    g.state.rules.frame = Frame::Transition;
    g.state.rules.players[0].mana.insert(Color::B, 2.into());
    let state_hash = digest(&g.state).unwrap();
    let trace_hash = digest(&g.trace).unwrap();
    assert!(g.transition_turn_step().is_err());
    assert!(
        matches!(g.typed_fault(),Some(RuleFault::StackCapacityExceeded { requested,.. }) if *requested==capacities::STACK_LIMIT+1)
    );
    g.state.rules.typed_fault = None;
    assert_eq!(digest(&g.state).unwrap(), state_hash);
    assert_eq!(digest(&g.trace).unwrap(), trace_hash);
}

#[test]
fn d2c_archived_program_families_execute_complete_step_effects() {
    for name in [
        "Fountain of Renewal",
        "Fretwork Colony",
        "Verdant Force",
        "Magmatic Force",
        "Bloodgift Demon",
        "Dogged Pursuit",
    ] {
        let c = catalog();
        let id = c
            .definitions
            .iter()
            .find(|d| d.name == name)
            .unwrap()
            .oracle_id
            .clone();
        let step = if name == "Dogged Pursuit" {
            TurnStep::PostcombatMain
        } else {
            TurnStep::Untap
        };
        let mut g = Game::new(prepared(&[(&id, 0, 0, Zone::Battlefield)], step));
        for _ in 0..4 {
            answer(&mut g, Response::Pass);
        }
        let r = settle(&mut g);
        match name {
            "Magmatic Force" => {
                assert!(matches!(r, Request::EntityTargets { .. }));
                answer(
                    &mut g,
                    Response::EntityTargets {
                        entities: vec![TargetEntity::Player {
                            player: PlayerId(1),
                        }],
                    },
                );
            }
            "Bloodgift Demon" => {
                assert!(matches!(r, Request::PlayerTargets { .. }));
                answer(
                    &mut g,
                    Response::PlayerTargets {
                        players: vec![PlayerId(1)],
                    },
                );
            }
            _ => assert!(matches!(r, Request::Priority { .. })),
        }
        placed(&mut g);
        recover(&g);
        resolve(&mut g);
        match name {
            "Fountain of Renewal" => assert_eq!(g.state.rules.players[0].life, 41.into()),
            "Fretwork Colony" => {
                assert_eq!(g.state.rules.players[0].life, 39.into());
                assert_eq!(
                    g.state.rules.objects[&CardId(0)].plus_one_counters,
                    1.into()
                );
            }
            "Verdant Force" => assert_eq!(g.state.rules.tokens.len(), 1),
            "Magmatic Force" => assert_eq!(g.state.rules.players[1].life, 37.into()),
            "Bloodgift Demon" => {
                assert_eq!(g.state.rules.players[1].life, 39.into());
                assert_eq!(g.state.rules.libraries[&PlayerId(1)].len(), 7);
            }
            "Dogged Pursuit" => {
                assert_eq!(g.state.rules.players[0].life, 41.into());
                for seat in 1..4 {
                    assert_eq!(g.state.rules.players[seat].life, 39.into());
                }
            }
            _ => unreachable!(),
        }
        recover(&g);
    }
}
#[test]
fn d2c_draw_collection_precedes_normal_draw_and_placement_follows_it() {
    let mut p = prepared(&[(ARENA, 0, 0, Zone::Battlefield)], TurnStep::Upkeep);
    let c = Arc::make_mut(&mut p.catalog);
    c.definitions
        .iter_mut()
        .find(|d| d.oracle_id == ARENA)
        .unwrap()
        .triggered[0]
        .condition = TriggerCondition::StepBegan {
        step: BeginningStep::Draw,
        turn_relation: TurnRelation::Controller,
    };
    p.catalog_digest = digest(c).unwrap();
    let mut g = Game::new(p);
    for _ in 0..4 {
        answer(&mut g, Response::Pass);
    }
    while !matches!(g.state.rules.frame, Frame::EnterStep) {
        assert!(matches!(
            g.advance(Budget::new(1)),
            Advance::Suspended { .. }
        ));
    }
    assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), 8);
    assert_eq!(g.state.rules.pending_triggers.len(), 1);
    recover(&g);
    placed(&mut g);
    assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), 7);
    assert_eq!(g.state.rules.stack.len(), 1);
    recover(&g);
    resolve(&mut g);
    assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), 6);
    assert_eq!(g.state.rules.next_trigger_id, 1);
    recover(&g);
}

#[path = "event_player_tests.rs"]
mod event_player_tests;
