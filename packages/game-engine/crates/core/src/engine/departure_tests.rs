use super::*;

fn prepared(commanders: bool) -> Prepared {
    Prepared::admit(
        Catalog::read(include_str!("../../../../tests/fixtures/expected-ir.json")).unwrap(),
        if commanders {
            ScenarioConfig::commander_foundation()
        } else {
            ScenarioConfig::land_return()
        },
    )
    .unwrap()
}
fn respond(game: &mut Game, response: Response) {
    let Advance::AwaitingDecision(r) = game.advance(Budget::unlimited()) else {
        panic!("missing decision")
    };
    game.respond(r.player(), r.decision_id(), response).unwrap();
}
fn cast(game: &mut Game) {
    respond(
        game,
        Response::ActivateMana {
            object: game.object_id(CardId(0)).unwrap(),
        },
    );
    respond(game, Response::Cast { card: CardId(1) });
    assert!(matches!(
        game.advance(Budget::unlimited()),
        Advance::AwaitingDecision(Request::Targets { .. })
    ));
}
fn select(game: &mut Game) {
    respond(
        game,
        Response::Targets {
            objects: vec![game.object_id(CardId(2)).unwrap()],
        },
    );
}
fn recover(game: &Game) {
    let restored = Game::restore(game.prepared.clone(), &game.snapshot().unwrap()).unwrap();
    assert_eq!(restored.state, game.state);
    assert_eq!(restored.fork().state, game.state);
}
fn depart(game: &mut Game, player: u8) {
    game.concede(PlayerId(player)).unwrap();
    recover(game);
    let outcome = game.advance(Budget::new(1));
    assert!(!matches!(outcome, Advance::Fault(_)));
    recover(game);
}
fn observation(game: &Game, player: u8) -> Observation {
    let QueryResult::Complete(QueryValue::Observation(view)) =
        game.observe(PlayerId(player), Budget::unlimited())
    else {
        panic!("missing observation")
    };
    view
}
#[test]
fn owner_departure_removes_owned_objects_but_retains_physical_and_player_history() {
    let mut game = Game::new(prepared(false));
    let original = game.object_id(CardId(2)).unwrap();
    depart(&mut game, 1);
    assert!(game.object_id(CardId(2)).is_none());
    assert!(game.state.cards.contains_key(&CardId(2)));
    assert!(game.events().contains(&Event::ObjectLeftGame {
        object: original,
        owner: PlayerId(1)
    }));
    assert!(
        !game
            .events()
            .iter()
            .any(|e| matches!(e,Event::Committed{actual:EventOutcome::Moved{movement},..} if movement.previous==original))
    );
    assert_eq!(
        game.state.players[1].last_known.as_ref().unwrap().life,
        Quantity::from(40)
    );
    let view = observation(&game, 0);
    assert_eq!(view.view.players[1].status, PlayerStatus::Left);
    assert_eq!(
        game.live_players(),
        vec![PlayerId(0), PlayerId(2), PlayerId(3)]
    );
}
#[test]
fn departing_default_controller_exiles_instead_of_restoring_owner() {
    let mut game = Game::new(prepared(false));
    depart(&mut game, 2);
    let object = game.object_id(CardId(2)).unwrap();
    assert_eq!(object.incarnation, 1);
    assert_eq!(game.zone(object), Some(Zone::Exile));
    assert_eq!(
        Some(game.state.cards[&object.card].owner),
        Some(PlayerId(1))
    );
    assert_eq!(
        Some(game.current(object).unwrap().controller),
        Some(PlayerId(1))
    );
}
#[test]
fn control_provenance_restores_remaining_grants_and_ends_static_sources_only() {
    // Private kernel fixtures: control-grant cards are not admitted by this pool.
    let mut game = Game::new(prepared(false));
    let bear = game.object_id(CardId(2)).unwrap();
    game.state
        .objects
        .get_mut(&CardId(2))
        .unwrap()
        .default_controller = PlayerId(1);
    game.state.control_effects = vec![
        ControlEffect {
            id: 1,
            object: bear,
            controller: PlayerId(3),
            source: ControlSource::Persistent,
        },
        ControlEffect {
            id: 2,
            object: bear,
            controller: PlayerId(2),
            source: ControlSource::Persistent,
        },
    ];
    game.concede(PlayerId(2)).unwrap();
    let _ = game.advance(Budget::new(1));
    assert_eq!(
        Some(game.current(bear).unwrap().controller),
        Some(PlayerId(3))
    );
    assert_eq!(game.state.control_effects.len(), 1);
    game.concede(PlayerId(3)).unwrap();
    let _ = game.advance(Budget::new(1));
    assert_eq!(
        Some(game.current(bear).unwrap().controller),
        Some(PlayerId(1))
    );
    let mut static_game = Game::new(prepared(false));
    static_game
        .state
        .objects
        .get_mut(&CardId(2))
        .unwrap()
        .default_controller = PlayerId(1);
    static_game
        .state
        .objects
        .get_mut(&CardId(2))
        .unwrap()
        .controller = PlayerId(3);
    static_game.state.control_effects = vec![
        ControlEffect {
            id: 1,
            object: bear,
            controller: PlayerId(2),
            source: ControlSource::Persistent,
        },
        ControlEffect {
            id: 2,
            object: bear,
            controller: PlayerId(3),
            source: ControlSource::Static {
                object: static_game.object_id(CardId(0)).unwrap(),
            },
        },
    ];
    static_game.concede(PlayerId(0)).unwrap();
    let _ = static_game.advance(Budget::new(1));
    assert_eq!(
        Some(static_game.current(bear).unwrap().controller),
        Some(PlayerId(2))
    );
    assert_eq!(
        static_game
            .state
            .control_effects
            .iter()
            .map(|e| e.id)
            .collect::<Vec<_>>(),
        vec![1]
    );
}
#[test]
fn active_departure_keeps_turn_owner_and_surviving_priority_order() {
    let mut game = Game::new(prepared(false));
    depart(&mut game, 0);
    assert_eq!(game.active_player(), None);
    assert_eq!(game.turn_owner(), PlayerId(0));
    let Advance::AwaitingDecision(r) = game.advance(Budget::unlimited()) else {
        panic!()
    };
    assert_eq!(r.player(), PlayerId(1));
    respond(&mut game, Response::Pass);
    let Advance::AwaitingDecision(r) = game.advance(Budget::unlimited()) else {
        panic!()
    };
    assert_eq!(r.player(), PlayerId(2));
    recover(&game);
}
#[test]
fn target_departure_reverses_unfinished_cast_without_spending_mana() {
    // Test announcement, accepted targets, mana window, and automatic payment.
    for boundary in 0..4 {
        let mut game = Game::new(prepared(false));
        cast(&mut game);
        if boundary > 0 {
            select(&mut game);
        }
        if boundary >= 2 {
            let _ = game.advance(Budget::new(1));
            assert!(matches!(game.state.frame, Frame::Mana { .. }));
        }
        if boundary == 3 {
            respond(&mut game, Response::FinishMana { undo_mana: false });
            let _ = game.advance(Budget::new(1));
            assert!(matches!(game.state.frame, Frame::Payment { .. }));
        }
        depart(&mut game, 1);
        assert_eq!(
            game.zone(game.object_id(CardId(1)).unwrap()),
            Some(Zone::Hand { owner: PlayerId(0) })
        );
        assert_eq!(game.object_id(CardId(1)).unwrap().incarnation, 0);
        assert_eq!(game.mana(PlayerId(0), Color::U), Some(Quantity::from(1)));
        assert!(game.state.stack.is_empty());
        assert!(
            !game
                .events()
                .iter()
                .any(|e| matches!(e, Event::SpellCast { .. }))
        );
        assert_eq!(
            Game::replay(game.prepared.clone(), &game.replay_journal())
                .unwrap()
                .state,
            game.state
        );
    }
}
#[test]
fn source_departure_discards_casting_but_started_resolution_continues() {
    for boundary in 0..4 {
        let mut game = Game::new(prepared(false));
        cast(&mut game);
        if boundary > 0 {
            select(&mut game);
        }
        if boundary == 1 {
            let _ = game.advance(Budget::new(1));
        }
        if boundary >= 2 {
            respond(&mut game, Response::FinishMana { undo_mana: false });
            let _ = game.advance(Budget::unlimited());
        }
        if boundary == 3 {
            for _ in 0..4 {
                respond(&mut game, Response::Pass);
            }
            let _ = game.advance(Budget::new(1));
            assert!(matches!(game.state.frame, Frame::Resolve { .. }));
        }
        depart(&mut game, 0);
        assert!(game.state.stack.is_empty());
        if boundary == 3 {
            assert!(matches!(game.state.frame, Frame::Resolve { .. }));
            let _ = game.advance(Budget::unlimited());
            recover(&game);
            assert_eq!(
                game.zone(game.object_id(CardId(2)).unwrap()),
                Some(Zone::Hand { owner: PlayerId(1) })
            );
            assert!(
                game.events()
                    .iter()
                    .any(|e| matches!(e, Event::SpellCompleted { resolved: true, .. }))
            );
            assert_eq!(game.state.priority, PlayerId(1));
        } else {
            assert!(matches!(game.state.frame, Frame::Priority));
        }
        assert!(game.object_id(CardId(1)).is_none());
        assert_eq!(game.active_player(), None);
        assert!(
            game.events()
                .iter()
                .any(|e| matches!(e, Event::StackRemovedForDeparture { .. }))
        );
    }
}
#[test]
fn target_departure_after_casting_uses_resolution_legality_and_preserves_passes() {
    let mut game = Game::new(prepared(false));
    cast(&mut game);
    select(&mut game);
    respond(&mut game, Response::FinishMana { undo_mana: false });
    let _ = game.advance(Budget::unlimited());
    depart(&mut game, 1);
    for _ in 0..2 {
        respond(&mut game, Response::Pass);
    }
    let _ = game.advance(Budget::unlimited());
    assert_eq!(game.state.passes, vec![PlayerId(0), PlayerId(2)]);
    // P3 is the only remaining unpassed player; departure begins resolution.
    depart(&mut game, 3);
    assert!(matches!(game.state.frame, Frame::Resolve { .. }));
    let _ = game.advance(Budget::unlimited());
    recover(&game);
    assert!(game.events().iter().any(|e| matches!(
        e,
        Event::SpellCompleted {
            resolved: false,
            ..
        }
    )));
    assert!(!game.events().iter().any(|e| matches!(
        e,
        Event::PriorityPassed {
            player: PlayerId(3)
        }
    )));
}
#[test]
fn mid_resolution_target_departure_keeps_captured_legality_but_skips_removed_object() {
    let mut game = Game::new(prepared(false));
    cast(&mut game);
    select(&mut game);
    respond(&mut game, Response::FinishMana { undo_mana: false });
    for _ in 0..4 {
        respond(&mut game, Response::Pass);
    }
    let _ = game.advance(Budget::new(1));
    assert!(matches!(game.state.frame, Frame::Resolve { .. }));
    depart(&mut game, 1);
    let _ = game.advance(Budget::new(1));
    recover(&game);
    let Frame::Resolve { frame } = &game.state.frame else {
        panic!("expected instruction boundary")
    };
    assert!(matches!(
        frame.outcomes.as_slice(),
        [InstructionOutcome::ObjectNoLongerPresent { .. }]
    ));
    let _ = game.advance(Budget::unlimited());
    recover(&game);
    assert!(
        game.events()
            .iter()
            .any(|e| matches!(e, Event::SpellCompleted { resolved: true, .. }))
    );
}
#[test]
fn queued_concessions_invalidate_queries_and_receipts_and_end_at_one_survivor() {
    let mut game = Game::new(prepared(false));
    let QueryResult::Incomplete { work, .. } = game.observe(PlayerId(0), Budget::new(1)) else {
        panic!()
    };
    let request = game.decision_for(PlayerId(0)).unwrap().unwrap();
    let receipt = game
        .check_response(PlayerId(0), request.decision_id(), Response::Pass)
        .unwrap();
    for player in [0, 1, 2] {
        game.concede(PlayerId(player)).unwrap();
        recover(&game);
    }
    assert!(game.concede(PlayerId(3)).is_err());
    assert!(game.submit(receipt).is_err());
    assert!(matches!(
        game.resume_work(work, Budget::unlimited()),
        QueryResult::Rejected(QueryRejection::StaleOrForeignWork)
    ));
    assert!(matches!(
        game.observe(PlayerId(0), Budget::unlimited()),
        QueryResult::Rejected(QueryRejection::UnsupportedBoundary)
    ));
    assert!(game.decision_for(PlayerId(0)).unwrap().is_none());
    for _ in 0..3 {
        let _ = game.advance(Budget::new(1));
        recover(&game);
    }
    assert_eq!(
        game.advance(Budget::new(0)),
        Advance::Finished {
            winner: PlayerId(3)
        }
    );
    assert!(game.concede(PlayerId(3)).is_err());
    assert_eq!(game.live_players(), vec![PlayerId(3)]);
    assert_eq!(
        Game::replay(game.prepared.clone(), &game.replay_journal())
            .unwrap()
            .state,
        game.state
    );
    let mut journal = game.replay_journal();
    journal.concessions.swap(0, 1);
    // Same boundary/input-index arrivals can be ordered differently, but cannot
    // reproduce the retained snapshot/event ordering.
    assert_ne!(
        Game::replay(game.prepared.clone(), &journal).unwrap().state,
        game.state
    );
    journal.concessions[0].boundary = 99;
    assert!(Game::replay(game.prepared.clone(), &journal).is_err());
}
#[test]
fn physical_commander_designation_and_exact_history_survive_departure() {
    let mut game = Game::new(prepared(true));
    let id = CardId(3);
    let history = game.commander_ledger().history(id).unwrap().clone();
    assert_eq!(history.color_identity, vec![Color::W]);
    let before = observation(&game, 2).view.commanders;
    let mut ledger = game.commander_ledger().clone();
    ledger
        .record_cast(id, PlayerId(0), Zone::Hand { owner: PlayerId(0) })
        .unwrap();
    assert_eq!(ledger.tax(id, PlayerId(0)).unwrap(), Quantity::zero());
    for _ in 0..3 {
        ledger
            .record_cast(id, PlayerId(0), Zone::Command { owner: PlayerId(0) })
            .unwrap();
    }
    assert_eq!(ledger.tax(id, PlayerId(0)).unwrap(), Quantity::from(6));
    assert_eq!(ledger.tax(id, PlayerId(1)).unwrap(), Quantity::zero());
    assert!(
        ledger
            .record_cast(id, PlayerId(1), Zone::Command { owner: PlayerId(0) })
            .is_err()
    );
    ledger
        .record_damage(id, PlayerId(1), false, &Quantity::from(99))
        .unwrap();
    ledger
        .record_damage(id, PlayerId(1), true, &Quantity::from(20))
        .unwrap();
    assert!(ledger.combat_loss_candidates().is_empty());
    ledger
        .record_damage(id, PlayerId(1), true, &Quantity::from(1))
        .unwrap();
    assert_eq!(ledger.combat_loss_candidates(), vec![PlayerId(1)]);
    assert!(
        ledger
            .record_damage(id, PlayerId(1), true, &Quantity::from(-1))
            .is_err()
    );
    // Commander damage from different physical commanders is never combined.
    ledger.records.insert(
        CardId(99),
        CommanderHistory {
            owner: PlayerId(0),
            color_identity: vec![],
            command_casts: BTreeMap::new(),
            combat_damage: BTreeMap::new(),
        },
    );
    ledger
        .record_damage(id, PlayerId(2), true, &Quantity::from(11))
        .unwrap();
    ledger
        .record_damage(CardId(99), PlayerId(2), true, &Quantity::from(10))
        .unwrap();
    assert!(!ledger.combat_loss_candidates().contains(&PlayerId(2)));
    let commander = game.object_id(id).unwrap();
    game.move_object(commander, Zone::Exile).unwrap();
    assert_eq!(game.commander_ledger().history(id).unwrap(), &history);
    // Return to a reachable scenario before testing recovery of departure.
    game = Game::new(prepared(true));
    depart(&mut game, 0);
    assert!(game.object_id(id).is_none());
    assert_eq!(observation(&game, 2).view.commanders, before);
    assert!(
        Game::new(prepared(false))
            .commander_ledger()
            .history(CardId(2))
            .is_none()
    );
    let mut config = ScenarioConfig::commander_foundation();
    config.commanders.push(CardId(2));
    assert!(Prepared::admit(game.prepared.catalog.as_ref().clone(), config).is_err());
}

#[test]
fn departed_default_controller_is_safe_under_a_remaining_live_grant_until_it_ends() {
    let mut game = Game::new(prepared(false));
    let bear = game.object_id(CardId(2)).unwrap();
    game.state.objects.get_mut(&CardId(2)).unwrap().controller = PlayerId(3);
    game.state.control_effects.push(ControlEffect {
        id: 1,
        object: bear,
        controller: PlayerId(3),
        source: ControlSource::Persistent,
    });
    game.concede(PlayerId(2)).unwrap();
    let _ = game.advance(Budget::new(1));
    assert_eq!(game.zone(bear), Some(Zone::Battlefield));
    assert_eq!(game.current(bear).unwrap().controller, PlayerId(3));
    game.concede(PlayerId(3)).unwrap();
    let _ = game.advance(Budget::new(1));
    assert_eq!(
        game.zone(game.object_id(CardId(2)).unwrap()),
        Some(Zone::Exile)
    );
}
#[test]
fn queued_departure_budget_partitions_and_accepted_input_cancellation_restore_identically() {
    let mut initial = Game::new(prepared(true));
    respond(
        &mut initial,
        Response::ActivateMana {
            object: ObjectId {
                card: CardId(0),
                incarnation: 0,
            },
        },
    );
    // An already accepted action by the departing player is retained as input
    // history but never attempts to tap an object that has left the game.
    for player in [0, 2, 3] {
        initial.concede(PlayerId(player)).unwrap();
    }
    recover(&initial);
    let mut expected = initial.fork();
    let _ = expected.advance(Budget::unlimited());
    for units in [1, 2, 3, u64::MAX] {
        let mut game = initial.fork();
        loop {
            let result = game.advance(Budget::new(units));
            recover(&game);
            match result {
                Advance::Finished {
                    winner: PlayerId(1),
                } => break,
                Advance::Suspended { .. } => {}
                other => panic!("unexpected departure outcome: {other:?}"),
            }
        }
        assert_eq!(game.state, expected.state);
        assert!(!game.events().iter().any(|e| matches!(
            e,
            Event::Committed {
                proposed: ProposedEvent::Tap { .. },
                ..
            }
        )));
    }
}
#[test]
fn commander_and_departure_metadata_cannot_be_forged_in_rehashed_snapshots() {
    let mut game = Game::new(prepared(true));
    depart(&mut game, 0);
    for mutation in 0..4 {
        let mut snapshot: Snapshot = strict_read(&game.snapshot().unwrap()).unwrap();
        match mutation {
            0 => snapshot
                .state
                .commanders
                .records
                .get_mut(&CardId(3))
                .unwrap()
                .command_casts
                .insert(PlayerId(0), Quantity::from(1)),
            1 => {
                snapshot.state.players[0].last_known.as_mut().unwrap().life = Quantity::from(1);
                None
            }
            2 => {
                snapshot.state.original_seating.swap(1, 2);
                None
            }
            _ => {
                snapshot.state.pending_concessions.push(PlayerId(0));
                None
            }
        };
        snapshot.state_sha256 = digest(&snapshot.state).unwrap();
        assert!(Game::restore(game.prepared.clone(), &canonical(&snapshot).unwrap()).is_err());
    }
    let mut ledger = Game::new(prepared(true)).commander_ledger().clone();
    ledger
        .records
        .get_mut(&CardId(3))
        .unwrap()
        .command_casts
        .insert(PlayerId(0), "18446744073709551616".parse().unwrap());
    assert_eq!(
        ledger.tax(CardId(3), PlayerId(0)).unwrap().to_string(),
        "36893488147419103232"
    );
}
