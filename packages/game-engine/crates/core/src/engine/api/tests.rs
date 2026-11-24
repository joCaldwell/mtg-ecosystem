//! Narrow in-crate anchors over the executor: receipts, staging and
//! private-field forgery that an external test binary cannot build.
use crate::engine::*;
fn prepared() -> Prepared {
    Prepared::admit(
        Catalog::read(include_str!(
            "../../../../../tests/fixtures/expected-ir.json"
        ))
        .unwrap(),
        ScenarioConfig::land_return(),
    )
    .unwrap()
}
fn respond(game: &mut Game, choose: impl FnOnce(&Decision<Rules>) -> Answer) {
    let Advance::AwaitingDecision(r) = game.advance(Budget::unlimited()) else {
        panic!("expected decision")
    };
    let response = choose(&r);
    let finish = matches!(
        response,
        Answer::Pay {
            pay: PayAnswer::Finish { .. }
        }
    );
    game.respond(r.player, r.id, response).unwrap();
    if finish {
        game.complete_test_mana();
    }
}
fn cast(game: &mut Game) {
    respond(game, |d| {
        d.activate_mana(ObjectId {
            card: CardId(0),
            incarnation: 0,
        })
    });
    respond(game, |d| d.cast(CardId(1)));
}
fn complete(game: &mut Game) {
    respond(game, |d| {
        d.targets(vec![ObjectId {
            card: CardId(2),
            incarnation: 0,
        }])
    });
    respond(game, |_| Answer::Pay {
        pay: PayAnswer::Finish {
            undo_on_failure: false,
        },
    });
    for _ in 0..4 {
        respond(game, |d| d.pass());
    }
    assert!(matches!(
        game.advance(Budget::unlimited()),
        Advance::AwaitingDecision(_)
    ));
}
#[test]
fn complete_source_scenario_preserves_owner_and_incarnations() {
    let mut game = Game::new(prepared());
    cast(&mut game);
    complete(&mut game);
    assert_eq!(
        game.state.rules.objects[&CardId(2)].zone,
        Zone::Hand { owner: PlayerId(1) }
    );
    assert_eq!(game.state.rules.objects[&CardId(2)].id.incarnation, 1);
    assert_eq!(
        game.state.rules.objects[&CardId(1)].zone,
        Zone::Graveyard { owner: PlayerId(0) }
    );
    assert_eq!(game.state.rules.objects[&CardId(1)].id.incarnation, 2);
    assert_eq!(game.mana(PlayerId(0), Color::U), Some(Quantity::zero()));
    assert_eq!(game.state.rules.priority, PlayerId(0));
    assert_eq!(game.events().len(), 13);
}
#[test]
fn snapshot_fork_and_replay_agree_and_receipts_do_not_cross_executors() {
    let p = prepared();
    let mut game = Game::new(p.clone());
    cast(&mut game);
    let _ = game.advance(Budget::unlimited());
    let mut restored = Game::restore(p.clone(), &game.snapshot().unwrap()).unwrap();
    let mut fork = game.fork();
    let req = game.decision_for(PlayerId(0)).unwrap().unwrap();
    let response = req.targets(vec![game.object_id(CardId(2)).unwrap()]);
    let receipt = game.check_response(PlayerId(0), req.id, response).unwrap();
    assert!(fork.submit(receipt).is_err());
    complete(&mut game);
    complete(&mut restored);
    complete(&mut fork);
    assert_eq!(game.state, restored.state);
    assert_eq!(game.state, fork.state);
    let replay = Game::replay(p, &game.replay_journal()).unwrap();
    assert_eq!(game.state, replay.state);
}
#[test]
fn staged_priority_decision_follows_rules_mutation() {
    let mut stage = RulesTxn::new(prepared()).fork();
    assert!(stage.decision_for(PlayerId(0)).unwrap().is_some());
    stage.rules_mut().priority = PlayerId(1);
    assert!(stage.decision_for(PlayerId(0)).unwrap().is_none());
    assert!(stage.decision_for(PlayerId(1)).unwrap().is_some());
}
#[test]
fn queries_and_invalid_responses_leave_state_unchanged() {
    let mut game = Game::new(prepared());
    let before = game.state_digest().unwrap();
    for _ in 0..3 {
        for viewer in 0..4 {
            let _ = game.decision_for(PlayerId(viewer)).unwrap();
        }
        let _ = game
            .characteristics(game.object_id(CardId(2)).unwrap())
            .unwrap();
        let _ = game
            .mana_abilities(game.object_id(CardId(0)).unwrap())
            .unwrap();
    }
    assert!(
        game.respond(
            PlayerId(1),
            0,
            Answer::Pick {
                picks: vec![vec![0]]
            }
        )
        .is_err()
    );
    assert!(
        game.respond(
            PlayerId(0),
            99,
            Answer::Pick {
                picks: vec![vec![0]]
            }
        )
        .is_err()
    );
    assert!(
        game.respond(
            PlayerId(0),
            0,
            Answer::Pick {
                picks: vec![vec![u32::MAX]]
            }
        )
        .is_err()
    );
    assert_eq!(game.state_digest().unwrap(), before);
    cast(&mut game);
    let _ = game.advance(Budget::unlimited());
    let before = game.state_digest().unwrap();
    assert!(
        game.respond(
            PlayerId(0),
            game.state.rules.revision,
            Answer::Pick {
                picks: vec![vec![u32::MAX]]
            }
        )
        .is_err()
    );
    assert_eq!(game.state_digest().unwrap(), before);
}
#[test]
fn payment_reversal_retains_inputs_and_original_incarnation() {
    let mut game = Game::new(prepared());
    let original = game.object_id(CardId(1)).unwrap();
    respond(&mut game, |d| d.cast(CardId(1)));
    respond(&mut game, |d| {
        d.targets(vec![ObjectId {
            card: CardId(2),
            incarnation: 0,
        }])
    });
    respond(&mut game, |_| Answer::Pay {
        pay: PayAnswer::Finish {
            undo_on_failure: true,
        },
    });
    let _ = game.advance(Budget::unlimited());
    assert_eq!(game.zone(original), Some(Zone::Hand { owner: PlayerId(0) }));
    assert!(game.state.rules.stack.is_empty());
    assert_eq!(game.journal().len(), 3);
    assert!(matches!(
        game.events().last(),
        Some(Event::CastReversed { .. })
    ));
    let replay = Game::replay(prepared(), &game.replay_journal()).unwrap();
    assert_eq!(replay.state, game.state);
}
#[test]
fn pending_accepted_input_survives_restore_and_rehashed_impossible_state_rejects() {
    let p = prepared();
    let mut game = Game::new(p.clone());
    let Advance::AwaitingDecision(request) = game.advance(Budget::unlimited()) else {
        panic!("expected decision")
    };
    game.respond(
        PlayerId(0),
        request.id,
        request.activate_mana(ObjectId {
            card: CardId(0),
            incarnation: 0,
        }),
    )
    .unwrap();
    let mut restored = Game::restore(p.clone(), &game.snapshot().unwrap()).unwrap();
    assert_eq!(game.state, restored.state);
    let _ = game.advance(Budget::unlimited());
    let _ = restored.advance(Budget::unlimited());
    assert_eq!(game.state, restored.state);
    let mut snapshot: Snapshot = strict_read(&game.snapshot().unwrap()).unwrap();
    snapshot.state.rules.players[0]
        .mana
        .insert(Color::U, Quantity::from(99));
    snapshot.state_sha256 = digest(&snapshot.state).unwrap();
    assert!(Game::restore(p, &canonical(&snapshot).unwrap()).is_err());
}
#[test]
fn forest_intrinsic_is_derived_and_partial_multiblock_legality_keeps_other_block() {
    let mut game = Game::new(prepared());
    Arc::make_mut(&mut game.state.rules.cards)
        .get_mut(&CardId(0))
        .unwrap()
        .oracle_id = FOREST.into();
    assert_eq!(
        game.mana_abilities(game.object_id(CardId(0)).unwrap())
            .unwrap(),
        [Color::G]
    );
    let mut p = prepared();
    let mut c = (*p.catalog).clone();
    let program = c
        .definitions
        .iter_mut()
        .find(|d| d.oracle_id == SPELL)
        .unwrap()
        .spell
        .as_mut()
        .unwrap();
    let mut second = program.targets[0].clone();
    second.slot = 1;
    second.selection_id = "synthetic:second".into();
    program.targets.push(second);
    program.instructions.push(Instruction::move_target(
        1,
        mtg_contracts::kir::Destination::OwnerHand,
        1,
    ));
    p.catalog = Arc::new(c);
    let mut game = Game::new(p);
    cast(&mut game);
    respond(&mut game, |d| {
        d.targets(vec![
            ObjectId {
                card: CardId(2),
                incarnation: 0
            };
            2
        ])
    });
    respond(&mut game, |_| Answer::Pay {
        pay: PayAnswer::Finish {
            undo_on_failure: false,
        },
    });
    let _ = game.advance(Budget::unlimited());
    // Test-only inaccessible mutation makes one occurrence stale. Both
    // blocks still share one resolution legality check and stack object.
    game.state.rules.stack.last_mut().unwrap().targets[0].incarnation = 99;
    for _ in 0..4 {
        respond(&mut game, |d| d.pass());
    }
    let _ = game.advance(Budget::unlimited());
    assert_eq!(
        game.state.rules.objects[&CardId(2)].zone,
        Zone::Hand { owner: PlayerId(1) }
    );
    assert!(matches!(
        game.events().last(),
        Some(Event::SpellCompleted { resolved: true, .. })
    ));
}
#[test]
fn unsupported_configuration_and_modified_ir_reject_admission() {
    let p = prepared();
    let mut config = ScenarioConfig::land_return();
    config.mode = Mode::CommanderMatch;
    assert!(Prepared::admit((*p.catalog).clone(), config).is_err());
    let mut c = (*p.catalog).clone();
    c.definitions[0].name = "modified definition".into();
    assert!(Prepared::admit(c, ScenarioConfig::land_return()).is_err());
}
#[test]
fn stale_resolution_target_does_not_follow_physical_card() {
    let mut game = Game::new(prepared());
    cast(&mut game);
    respond(&mut game, |d| {
        d.targets(vec![ObjectId {
            card: CardId(2),
            incarnation: 0,
        }])
    });
    respond(&mut game, |_| Answer::Pay {
        pay: PayAnswer::Finish {
            undo_on_failure: false,
        },
    });
    let _ = game.advance(Budget::unlimited());
    let target = game.object_id(CardId(2)).unwrap();
    let moved = game
        .move_object(target, Zone::Hand { owner: PlayerId(1) })
        .unwrap();
    game.move_object(moved.successor, Zone::Battlefield)
        .unwrap();
    for _ in 0..4 {
        respond(&mut game, |d| d.pass());
    }
    let _ = game.advance(Budget::unlimited());
    assert_eq!(game.state.rules.objects[&CardId(2)].zone, Zone::Battlefield);
    assert!(matches!(
        game.events().last(),
        Some(Event::SpellCompleted {
            resolved: false,
            ..
        })
    ));
}
#[test]
fn synthetic_multiple_blocks_have_one_target_check_and_stack_frame() {
    let mut p = prepared();
    let mut c = (*p.catalog).clone();
    let d = c
        .definitions
        .iter_mut()
        .find(|d| d.oracle_id == SPELL)
        .unwrap();
    let program = d.spell.as_mut().unwrap();
    program.instructions.push(Instruction::move_target(
        0,
        mtg_contracts::kir::Destination::OwnerHand,
        1,
    ));
    // Test-only interpreter fixture. No public admission bypass is provided.
    p.catalog = Arc::new(c);
    let mut game = Game::new(p);
    cast(&mut game);
    complete(&mut game);
    let target_moves=game.events().iter().filter(|e|matches!(e,Event::Committed{proposed:ProposedEvent::Move{object,..},..} if object.card==CardId(2))).count();
    assert_eq!(target_moves, 1);
    assert_eq!(
        game.state.rules.objects[&CardId(2)].zone,
        Zone::Hand { owner: PlayerId(1) }
    );
}
#[test]
fn zone_intrinsics_and_corrupt_snapshot_fail_closed() {
    let p = prepared();
    let mut game = Game::new(p.clone());
    let id = game.object_id(CardId(0)).unwrap();
    assert_eq!(game.mana_abilities(id).unwrap(), vec![Color::U]);
    let movement = game
        .move_object(id, Zone::Hand { owner: PlayerId(0) })
        .unwrap();
    assert!(game.mana_abilities(movement.successor).unwrap().is_empty());
    assert!(
        Game::restore(
            p.clone(),
            &game.snapshot().unwrap().replace(build_id(), "wrong-build")
        )
        .is_err()
    );
    let mut snapshot: Snapshot = strict_read(&Game::new(p.clone()).snapshot().unwrap()).unwrap();
    snapshot
        .state
        .rules
        .objects
        .get_mut(&CardId(2))
        .unwrap()
        .id
        .card = CardId(99);
    snapshot.state_sha256 = digest(&snapshot.state).unwrap();
    assert!(Game::restore(p, &canonical(&snapshot).unwrap()).is_err());
}

/// Ported from the retired Node CLI contract: player projections and search
/// contexts keep private cards, recovery and executor fields out, and one
/// seat's handles never appear in another seat's view.
#[test]
fn player_projections_keep_private_cards_and_recovery_fields_out() {
    let game = Game::new(prepared());
    let observe = |viewer| {
        let QueryResult::Complete(QueryValue::Observation(o)) =
            game.observe(PlayerId(viewer), Budget::unlimited())
        else {
            panic!("observation")
        };
        serde_json::to_value(o).unwrap()
    };
    let (own, other) = (observe(0), observe(1));
    let catalog = Catalog::read(include_str!(
        "../../../../../tests/fixtures/expected-ir.json"
    ))
    .unwrap();
    let unsummon = catalog
        .definitions
        .iter()
        .find(|d| d.name == "Unsummon")
        .unwrap()
        .oracle_id
        .clone();
    let objects = |v: &serde_json::Value| v["view"]["objects"].as_array().unwrap().clone();
    assert!(objects(&own).iter().any(|o| o["oracle_id"] == unsummon));
    assert!(!objects(&other).iter().any(|o| o["oracle_id"] == unsummon));
    assert_eq!(other["view"]["players"][0]["hand_count"], 1);
    let QueryResult::Complete(QueryValue::SearchContext(context)) =
        game.search_context(PlayerId(1), "independent-seed".into(), Budget::unlimited())
    else {
        panic!("search context")
    };
    let context = serde_json::to_value(context).unwrap();
    assert_eq!(context["sampling_seed"], "independent-seed");
    const FORBIDDEN: [&str; 15] = [
        "state_sha256",
        "state",
        "initial_snapshot",
        "journal",
        "incarnation",
        "rule_steps",
        "source_json",
        "frame",
        "game_seed",
        "trace",
        "verification",
        "batch_id",
        "next_batch_id",
        "next_event_sequence",
        "before",
    ];
    fn check(value: &serde_json::Value) {
        match value {
            serde_json::Value::Array(items) => items.iter().for_each(check),
            serde_json::Value::Object(fields) => {
                for (key, child) in fields {
                    assert!(!FORBIDDEN.contains(&key.as_str()), "{key}");
                    check(child);
                }
            }
            _ => {}
        }
    }
    for value in [&own, &other, &context] {
        check(value);
    }
    let own_handles: Vec<_> = objects(&own).iter().map(|o| o["handle"].clone()).collect();
    assert!(
        !objects(&other)
            .iter()
            .any(|o| own_handles.contains(&o["handle"]))
    );
}
