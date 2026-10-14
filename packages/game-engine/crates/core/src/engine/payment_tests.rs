use super::*;

fn prepared() -> Prepared {
    Prepared::admit(
        Catalog::read(include_str!("../../../../tests/fixtures/expected-ir.json")).unwrap(),
        ScenarioConfig::stack_response(),
    )
    .unwrap()
}
fn land(player: u8) -> CardId {
    if player == 0 {
        CardId(0)
    } else {
        CardId(2 * u32::from(player) + 1)
    }
}
fn spell(player: u8) -> CardId {
    CardId(land(player).0 + 1)
}
fn recover(game: &Game) {
    let restored = Game::restore(game.prepared.clone(), &game.snapshot().unwrap()).unwrap();
    assert_eq!(restored.state, game.state);
    assert_eq!(restored.trace, game.trace);
}
fn settle(game: &mut Game) -> Advance {
    loop {
        let result = game.advance(Budget::new(1));
        recover(game);
        if !matches!(result, Advance::Suspended { .. }) {
            return result;
        }
    }
}
fn input(game: &mut Game, player: u8, response: Response) {
    let Advance::AwaitingDecision(request) = settle(game) else {
        panic!("missing decision")
    };
    assert_eq!(request.player(), PlayerId(player));
    game.respond(request.player(), request.decision_id(), response)
        .unwrap();
    recover(game);
}
fn announce(game: &mut Game, player: u8) {
    input(
        game,
        player,
        Response::Cast {
            card: spell(player),
        },
    );
    input(
        game,
        player,
        Response::Targets {
            objects: vec![ObjectId {
                card: CardId(2),
                incarnation: 0,
            }],
        },
    );
    assert!(
        matches!(settle(game), Advance::AwaitingDecision(Request::Mana { player: p, .. }) if p == PlayerId(player))
    );
}
fn activate(game: &mut Game, player: u8, card: CardId) {
    input(
        game,
        player,
        Response::ActivateMana {
            object: game.object_id(card).unwrap(),
        },
    );
    assert!(
        matches!(settle(game), Advance::AwaitingDecision(Request::Mana { player: p, .. }) if p == PlayerId(player))
    );
}
fn finish(game: &mut Game, player: u8, undo_mana: bool) {
    input(game, player, Response::FinishMana { undo_mana });
    assert!(
        matches!(settle(game), Advance::AwaitingDecision(Request::Priority { player: p, .. }) if p == PlayerId(player))
    );
}
fn paid_cast(game: &mut Game, player: u8) {
    announce(game, player);
    activate(game, player, land(player));
    finish(game, player, true);
}
fn extra_land(p: &mut Prepared, owner: u8, controller: u8, oracle: &str) -> CardId {
    // Private interpreter fixture, not another admitted production configuration.
    let card = CardId(p.config.cards.len() as u32);
    p.config.cards.push(CardSetup {
        id: card,
        oracle_id: oracle.into(),
        owner: PlayerId(owner),
        controller: PlayerId(controller),
        zone: Zone::Battlefield,
    });
    card
}
fn wrong_color_prepared() -> (Prepared, CardId) {
    // The original Island becomes a Forest only in this private interpreter fixture.
    let mut p = prepared();
    p.config
        .cards
        .iter_mut()
        .find(|c| c.id == land(1))
        .unwrap()
        .oracle_id = FOREST.into();
    let extra = extra_land(&mut p, 1, 1, FOREST);
    (p, extra)
}

#[test]
fn casting_mana_is_a_saved_decision_before_atomic_payment_in_all_trace_modes() {
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        let mut game = Game::with_trace(prepared(), mode);
        announce(&mut game, 0);
        assert!(game.state.stack[0].paid.is_none());
        assert!(game.state.players[0].mana.is_empty());
        let Frame::Mana { cast } = &game.state.frame else {
            panic!()
        };
        assert!(cast.mana_activations.is_empty());
        activate(&mut game, 0, land(0));
        let Frame::Mana { cast } = &game.state.frame else {
            panic!()
        };
        assert_eq!(cast.mana_activations.len(), 1);
        assert_eq!(game.mana(PlayerId(0), Color::U), Some(Quantity::from(1)));
        input(&mut game, 0, Response::FinishMana { undo_mana: true });
        assert!(matches!(
            game.advance(Budget::new(1)),
            Advance::Suspended { .. }
        ));
        recover(&game);
        assert!(matches!(game.state.frame, Frame::Payment { .. }));
        assert!(game.state.stack[0].paid.is_none());
        assert_eq!(game.mana(PlayerId(0), Color::U), Some(Quantity::from(1)));
        assert!(matches!(
            settle(&mut game),
            Advance::AwaitingDecision(Request::Priority {
                player: PlayerId(0),
                ..
            })
        ));
        assert!(game.state.stack[0].paid.is_some());
        assert!(game.state.objects[&land(0)].tapped);
        assert_eq!(game.mana(PlayerId(0), Color::U), Some(Quantity::from(0)));
        let replay = Game::replay_with_trace(prepared(), &game.replay_journal(), mode).unwrap();
        assert_eq!(replay.state, game.state);
        assert_eq!(replay.trace, game.trace);
    }
}

#[test]
fn affordable_spell_still_offers_mana_and_extra_mana_remains_after_payment() {
    let mut p = prepared();
    let extra = extra_land(&mut p, 0, 0, ISLAND);
    let mut game = Game::new(p);
    let object = game.object_id(land(0)).unwrap();
    input(&mut game, 0, Response::ActivateMana { object });
    announce(&mut game, 0);
    // CR 601.2g gives a chance to activate even when the initial pool suffices.
    activate(&mut game, 0, extra);
    finish(&mut game, 0, true);
    assert!(game.state.objects[&extra].tapped);
    assert!(game.state.objects[&land(0)].tapped);
    assert_eq!(game.mana(PlayerId(0), Color::U), Some(Quantity::from(1)));
    assert!(game.state.stack[0].paid.is_some());
}

#[test]
fn failed_payment_can_undo_or_keep_casting_mana_without_undoing_floated_mana() {
    for undo_mana in [false, true] {
        let (p, extra) = wrong_color_prepared();
        let mut game = Game::new(p);
        paid_cast(&mut game, 0);
        input(&mut game, 0, Response::Pass);
        let object = game.object_id(extra).unwrap();
        input(&mut game, 1, Response::ActivateMana { object });
        let _ = settle(&mut game);
        let before_pool = game.state.players[1].mana.clone();
        announce(&mut game, 1);
        activate(&mut game, 1, land(1));
        finish(&mut game, 1, undo_mana);
        assert_eq!(game.state.stack.len(), 1);
        assert_eq!(game.object_id(spell(1)).unwrap().incarnation, 0);
        assert_eq!(
            game.zone(game.object_id(spell(1)).unwrap()),
            Some(Zone::Hand { owner: PlayerId(1) })
        );
        assert!(game.state.objects[&extra].tapped);
        assert_eq!(game.state.objects[&land(1)].tapped, !undo_mana);
        if undo_mana {
            assert_eq!(game.state.players[1].mana, before_pool);
        } else {
            assert_eq!(game.mana(PlayerId(1), Color::G), Some(Quantity::from(2)));
        }
    }
}

#[test]
fn only_undone_mana_activations_restore_the_prior_pass_round() {
    for undo_mana in [false, true] {
        let (p, _) = wrong_color_prepared();
        let mut game = Game::new(p);
        paid_cast(&mut game, 0);
        input(&mut game, 0, Response::Pass);
        announce(&mut game, 1);
        activate(&mut game, 1, land(1));
        finish(&mut game, 1, undo_mana);
        assert_eq!(
            game.state.passes,
            if undo_mana { vec![PlayerId(0)] } else { vec![] }
        );
        assert_eq!(game.state.priority, PlayerId(1));
        if undo_mana {
            assert!(game.state.players[1].mana.is_empty());
        }
    }
}

#[test]
fn mana_stage_rejects_opponents_pass_cast_and_stale_or_repeated_activations_atomically() {
    let mut game = Game::new(prepared());
    announce(&mut game, 0);
    let request = game.decision_for(PlayerId(0)).unwrap().unwrap();
    let before = game.snapshot().unwrap();
    for (player, response) in [
        (PlayerId(1), Response::FinishMana { undo_mana: true }),
        (PlayerId(0), Response::Pass),
        (PlayerId(0), Response::Cast { card: spell(0) }),
        (
            PlayerId(0),
            Response::ActivateMana {
                object: game.object_id(land(1)).unwrap(),
            },
        ),
        (
            PlayerId(0),
            Response::ActivateMana {
                object: ObjectId {
                    card: land(0),
                    incarnation: 1,
                },
            },
        ),
    ] {
        assert!(
            game.respond(player, request.decision_id(), response)
                .is_err()
        );
        assert_eq!(before, game.snapshot().unwrap());
    }
    let original_id = request.decision_id();
    activate(&mut game, 0, land(0));
    let request = game.decision_for(PlayerId(0)).unwrap().unwrap();
    let before = game.snapshot().unwrap();
    assert!(
        game.respond(
            PlayerId(0),
            request.decision_id(),
            Response::ActivateMana {
                object: game.object_id(land(0)).unwrap()
            }
        )
        .is_err()
    );
    assert!(
        game.respond(
            PlayerId(0),
            original_id,
            Response::FinishMana { undo_mana: true }
        )
        .is_err()
    );
    assert_eq!(before, game.snapshot().unwrap());
    for player in 1..4 {
        assert!(game.decision_for(PlayerId(player)).unwrap().is_none());
    }
}

#[test]
fn unrelated_departure_preserves_mana_stage_and_target_departure_undoes_only_casting_mana() {
    let mut game = Game::new(prepared());
    announce(&mut game, 0);
    activate(&mut game, 0, land(0));
    game.concede(PlayerId(3)).unwrap();
    recover(&game);
    assert!(game.decision_for(PlayerId(0)).unwrap().is_none());
    assert!(matches!(
        settle(&mut game),
        Advance::AwaitingDecision(Request::Mana {
            player: PlayerId(0),
            ..
        })
    ));
    assert!(game.state.objects[&land(0)].tapped);
    assert_eq!(game.mana(PlayerId(0), Color::U), Some(Quantity::from(1)));
    let departed = game.state.players[3].last_known.clone();
    game.concede(PlayerId(1)).unwrap();
    assert!(matches!(
        settle(&mut game),
        Advance::AwaitingDecision(Request::Priority {
            player: PlayerId(0),
            ..
        })
    ));
    assert!(game.state.stack.is_empty());
    assert_eq!(game.object_id(spell(0)).unwrap().incarnation, 0);
    assert!(!game.state.objects[&land(0)].tapped);
    assert!(game.state.players[0].mana.is_empty());
    assert_eq!(game.state.players[3].last_known, departed);
    assert!(game.object_id(CardId(2)).is_none());
    let replay = Game::replay(prepared(), &game.replay_journal()).unwrap();
    assert_eq!(replay.state, game.state);
}

#[test]
fn payer_departure_retains_last_known_mana_and_does_not_reverse_foreign_lands() {
    let mut p = prepared();
    let borrowed = extra_land(&mut p, 3, 0, ISLAND);
    let mut game = Game::new(p);
    announce(&mut game, 0);
    activate(&mut game, 0, borrowed);
    game.concede(PlayerId(0)).unwrap();
    assert!(matches!(
        settle(&mut game),
        Advance::AwaitingDecision(Request::Priority {
            player: PlayerId(1),
            ..
        })
    ));
    assert_eq!(
        game.state.players[0]
            .last_known
            .as_ref()
            .unwrap()
            .mana
            .get(&Color::U),
        Some(&Quantity::from(1))
    );
    // Departing controller's remaining foreign permanent is exiled by CR 800.4a.
    assert_eq!(
        game.zone(game.object_id(borrowed).unwrap()),
        Some(Zone::Exile)
    );
    assert!(game.state.stack.is_empty());
    assert!(game.object_id(spell(0)).is_none());
}

#[test]
fn vanished_mana_source_is_not_resurrected_when_target_departure_reverses_cast() {
    let mut p = prepared();
    let borrowed = extra_land(&mut p, 1, 0, ISLAND);
    let mut game = Game::new(p);
    announce(&mut game, 0);
    activate(&mut game, 0, borrowed);
    game.concede(PlayerId(1)).unwrap();
    assert!(matches!(
        settle(&mut game),
        Advance::AwaitingDecision(Request::Priority {
            player: PlayerId(0),
            ..
        })
    ));
    assert!(game.object_id(borrowed).is_none());
    assert!(game.object_id(CardId(2)).is_none());
    assert!(game.state.players[0].mana.is_empty());
    assert_eq!(
        game.zone(game.object_id(spell(0)).unwrap()),
        Some(Zone::Hand { owner: PlayerId(0) })
    );
    assert_eq!(game.state.players[1].status, PlayerStatus::Left);
}

#[test]
fn accepted_activation_is_discarded_when_departure_removes_its_source_before_execution() {
    let mut p = prepared();
    let borrowed = extra_land(&mut p, 3, 0, ISLAND);
    let mut game = Game::new(p);
    announce(&mut game, 0);
    let object = game.object_id(borrowed).unwrap();
    input(&mut game, 0, Response::ActivateMana { object });
    assert!(game.state.pending.is_some());
    game.concede(PlayerId(3)).unwrap();
    assert!(matches!(
        settle(&mut game),
        Advance::AwaitingDecision(Request::Mana {
            player: PlayerId(0),
            ..
        })
    ));
    assert!(game.state.pending.is_none());
    assert!(game.object_id(borrowed).is_none());
    assert!(game.state.players[0].mana.is_empty());
    let Frame::Mana { cast } = &game.state.frame else {
        panic!()
    };
    assert!(cast.mana_activations.is_empty());
    assert!(game.state.stack[0].paid.is_none());
    activate(&mut game, 0, land(0));
    finish(&mut game, 0, true);
    assert!(game.state.stack[0].paid.is_some());
}

#[test]
fn reversal_preserves_a_preexisting_zero_mana_entry() {
    // Private two-U-cost interpreter program with a second physical spell.
    // Produce an existing zero entry through a real first payment, then undo
    // one activation during the unpayable second proposal. No state injection.
    let mut p = prepared();
    let second_land = extra_land(&mut p, 0, 0, ISLAND);
    let second_spell = CardId(p.config.cards.len() as u32);
    p.config.cards.push(CardSetup {
        id: second_spell,
        oracle_id: SPELL.into(),
        owner: PlayerId(0),
        controller: PlayerId(0),
        zone: Zone::Hand { owner: PlayerId(0) },
    });
    let third_land = extra_land(&mut p, 0, 0, ISLAND);
    let mut catalog = p.catalog.as_ref().clone();
    catalog
        .definitions
        .iter_mut()
        .find(|d| d.oracle_id == SPELL)
        .unwrap()
        .characteristics
        .mana_cost
        .colored = vec![Color::U, Color::U];
    p.catalog = Arc::new(catalog);
    let mut game = Game::new(p);
    announce(&mut game, 0);
    activate(&mut game, 0, land(0));
    activate(&mut game, 0, second_land);
    finish(&mut game, 0, false);
    assert_eq!(
        game.state.players[0].mana.get(&Color::U),
        Some(&Quantity::from(0))
    );
    let before = game.state.players[0].mana.clone();
    input(&mut game, 0, Response::Cast { card: second_spell });
    let target = game.object_id(CardId(2)).unwrap();
    input(
        &mut game,
        0,
        Response::Targets {
            objects: vec![target],
        },
    );
    let _ = settle(&mut game);
    activate(&mut game, 0, third_land);
    let Frame::Mana { cast } = &game.state.frame else {
        panic!()
    };
    assert!(!cast.mana_activations[0].mana_was_absent);
    finish(&mut game, 0, true);
    assert_eq!(game.state.players[0].mana, before);
    assert!(!game.state.objects[&third_land].tapped);
    assert_eq!(game.state.stack.len(), 1);
    assert_eq!(
        game.zone(game.object_id(second_spell).unwrap()),
        Some(Zone::Hand { owner: PlayerId(0) })
    );
}

#[test]
fn restore_rejects_rehashed_forged_casting_mana_receipts() {
    let mut game = Game::new(prepared());
    announce(&mut game, 0);
    activate(&mut game, 0, land(0));
    let snapshot = game.snapshot().unwrap();
    for mutation in 0..7 {
        let mut saved: Snapshot = strict_read(&snapshot).unwrap();
        let Frame::Mana { cast } = &mut saved.state.frame else {
            panic!()
        };
        match mutation {
            0 => cast.mana_activations[0].player = PlayerId(1),
            1 => cast.mana_activations[0].color = Color::G,
            2 => cast.mana_activations[0].amount = Quantity::from(2),
            3 => cast.mana_activations[0].object.incarnation = 1,
            4 => cast.mana_activations[0].mana_was_absent = false,
            5 => cast.undo_mana = true,
            _ => cast.mana_activations.clear(),
        }
        saved.state_sha256 = digest(&saved.state).unwrap();
        assert!(Game::restore(prepared(), &canonical(&saved).unwrap()).is_err());
    }
}

#[test]
fn departing_caster_keeps_mana_action_and_requires_a_fresh_lower_spell_pass_round() {
    let mut game = Game::new(prepared());
    announce(&mut game, 0);
    activate(&mut game, 0, land(0));
    finish(&mut game, 0, false);
    input(&mut game, 0, Response::Pass);
    let _ = settle(&mut game);
    assert_eq!(game.state.passes, vec![PlayerId(0)]);
    announce(&mut game, 1);
    activate(&mut game, 1, land(1));
    game.concede(PlayerId(1)).unwrap();
    let _ = settle(&mut game);
    assert!(game.state.passes.is_empty());
    assert_eq!(game.state.stack.len(), 1);
    assert_eq!(game.state.priority, PlayerId(2));
    assert_eq!(
        game.state.players[1].last_known.as_ref().unwrap().mana[&Color::U],
        Quantity::from(1)
    );
    for player in [2, 3] {
        input(&mut game, player, Response::Pass);
    }
    let _ = settle(&mut game);
    assert_eq!(game.state.stack.len(), 1);
    assert_eq!(game.state.priority, PlayerId(0));
    input(&mut game, 0, Response::Pass);
    let _ = settle(&mut game);
    assert!(game.state.stack.is_empty());
    assert!(game.object_id(CardId(2)).is_none());
    let replay = Game::replay(prepared(), &game.replay_journal()).unwrap();
    assert_eq!(replay.state, game.state);
    assert_eq!(replay.trace, game.trace);
}
