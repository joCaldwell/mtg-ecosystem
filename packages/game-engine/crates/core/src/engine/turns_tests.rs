use super::*;

fn prepared() -> Prepared {
    Prepared::admit(
        Catalog::read(include_str!("../../../../tests/fixtures/expected-ir.json")).unwrap(),
        ScenarioConfig::turns_permanents(),
    )
    .unwrap()
}

fn recover(game: &Game) {
    let restored =
        Game::restore(game.prepared.as_ref().clone(), &game.snapshot().unwrap()).unwrap();
    assert_eq!(restored.state, game.state);
    assert_eq!(restored.trace, game.trace);
}

fn settle(game: &mut Game) -> Advance {
    for _ in 0..40 {
        let outcome = game.advance(Budget::new(1));
        recover(game);
        if !matches!(outcome, Advance::Suspended { .. }) {
            return outcome;
        }
    }
    panic!("automatic work did not reach a boundary")
}

fn input(game: &mut Game, response: Response) {
    let finish = matches!(response, Response::FinishMana { .. });
    let Advance::AwaitingDecision(request) = settle(game) else {
        panic!("missing decision at {:?}", game.state.rules.frame)
    };
    let response = if matches!(response, Response::Pass) {
        match &request {
            Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
            Request::Blockers { .. } => Response::Blockers { blocks: vec![] },
            _ => response,
        }
    } else {
        response
    };
    game.respond(request.player(), request.decision_id(), response)
        .unwrap();
    if finish {
        game.complete_test_mana();
    }
    recover(game);
}

fn pass(game: &mut Game) {
    input(game, Response::Pass);
    let _ = settle(game);
}

fn until(game: &mut Game, number: u64, step: TurnStep) {
    for _ in 0..180 {
        let result = settle(game);
        assert!(matches!(result, Advance::AwaitingDecision(_)), "{result:?}");
        if game.state.rules.turn.number == number && game.state.rules.turn.step == step {
            return;
        }
        input(game, Response::Pass);
    }
    panic!("did not reach turn {number}, {step:?}")
}

fn play(game: &mut Game, card: u32) {
    input(game, Response::PlayLand { card: CardId(card) });
    assert!(matches!(settle(game), Advance::AwaitingDecision(_)));
}

fn cast_bears(game: &mut Game) {
    input(game, Response::Cast { card: CardId(2) });
    assert!(matches!(
        settle(game),
        Advance::AwaitingDecision(Request::Mana {
            player: PlayerId(0),
            ..
        })
    ));
    for card in [0, 1] {
        input(
            game,
            Response::ActivateMana {
                object: game.object_id(CardId(card)).unwrap(),
            },
        );
    }
    input(game, Response::FinishMana { undo_mana: true });
    assert!(matches!(settle(game), Advance::AwaitingDecision(_)));
    assert_eq!(game.state.rules.stack.len(), 1);
    assert!(game.state.rules.stack[0].targets.is_empty());
    assert!(game.state.rules.stack[0].paid.is_some());
}

fn resolve_bears(game: &mut Game) {
    for player in 0..4 {
        assert_eq!(game.state.rules.priority, PlayerId(player));
        pass(game);
        if player < 3 {
            assert_eq!(game.state.rules.stack.len(), 1);
        }
    }
    assert!(game.state.rules.stack.is_empty());
    assert_eq!(game.state.rules.turn.step, TurnStep::PostcombatMain);
    assert_eq!(game.state.rules.priority, PlayerId(0));
    assert_eq!(
        game.zone(game.object_id(CardId(2)).unwrap()),
        Some(Zone::Battlefield)
    );
}

fn reject(game: &mut Game, player: PlayerId, response: Response) {
    let Advance::AwaitingDecision(request) = settle(game) else {
        panic!()
    };
    let before = game.snapshot().unwrap();
    assert!(
        game.respond(player, request.decision_id(), response)
            .is_err()
    );
    assert_eq!(game.snapshot().unwrap(), before);
}

fn finish_query(game: &Game, mut result: QueryResult) -> QueryValue {
    for _ in 0..400 {
        result = match result {
            QueryResult::Complete(value) => return value,
            QueryResult::Incomplete { work, .. } => game.resume_work(work, Budget::new(1)),
            QueryResult::Rejected(reason) => panic!("query rejected: {reason:?}"),
        };
    }
    panic!("query did not complete")
}

fn observe(game: &Game, player: PlayerId) -> Observation {
    let QueryValue::Observation(observation) =
        finish_query(game, game.observe(player, Budget::new(0)))
    else {
        panic!()
    };
    observation
}

#[test]
fn complete_turn_loop_preserves_exact_permanents_draws_timing_and_replay() {
    let mut game = Game::new(prepared());
    assert_eq!(game.state.rules.turn.number, 1);
    assert_eq!(game.state.rules.turn.step, TurnStep::PostcombatMain);
    play(&mut game, 1);
    assert_eq!(game.state.rules.turn.lands_played, 1);
    assert!(game.state.rules.stack.is_empty());
    reject(
        &mut game,
        PlayerId(0),
        Response::PlayLand { card: CardId(3) },
    );
    cast_bears(&mut game);
    resolve_bears(&mut game);
    assert_eq!(game.object_id(CardId(2)).unwrap().incarnation, 2);
    assert!(
        game.events()
            .iter()
            .any(|event| matches!(event, Event::SpellCompleted { resolved: true, .. }))
    );
    assert!(!game.events().iter().any(|event| matches!(
        event,
        Event::Committed {
            actual: EventOutcome::Moved { movement }, ..
        } if movement.previous.card == CardId(2)
            && matches!(movement.destination, Zone::Graveyard { .. })
    )));
    until(&mut game, 5, TurnStep::PrecombatMain);
    assert_eq!(game.state.rules.turn.lands_played, 0);
    play(&mut game, 3);
    reject(
        &mut game,
        PlayerId(0),
        Response::PlayLand { card: CardId(4) },
    );
    assert_eq!(game.state.rules.turn.lands_played, 1);
    assert_eq!(game.state.rules.active, Some(PlayerId(0)));
    assert_eq!(game.state.rules.priority, PlayerId(0));
    for card in [0, 1, 2, 3] {
        let object = &game.state.rules.objects[&CardId(card)];
        assert_eq!(object.zone, Zone::Battlefield);
        assert!(!object.tapped);
        assert_eq!(object.controller, PlayerId(0));
    }
    for (card, owner) in [(4, 0), (5, 1), (6, 2), (7, 3)] {
        let object = &game.state.rules.objects[&CardId(card)];
        assert_eq!(
            object.zone,
            Zone::Hand {
                owner: PlayerId(owner)
            }
        );
        assert_eq!(object.id.incarnation, 1);
    }
    assert!(
        game.state
            .rules
            .players
            .iter()
            .all(|p| p.mana.is_empty() && p.life == Quantity::from(40))
    );
    assert!(game.state.rules.stack.is_empty());
    let replay = Game::replay(prepared(), &game.replay_journal()).unwrap();
    assert_eq!(replay.state, game.state);
    assert_eq!(replay.trace, game.trace);
}

#[test]
fn lands_and_creatures_reject_nonactive_timing_stack_and_wrong_source() {
    let mut game = Game::new(prepared());
    for response in [
        Response::PlayLand { card: CardId(2) },
        Response::PlayLand { card: CardId(0) },
        Response::PlayLand { card: CardId(4) },
        Response::Cast { card: CardId(1) },
    ] {
        reject(&mut game, PlayerId(0), response);
    }
    pass(&mut game);
    reject(
        &mut game,
        PlayerId(1),
        Response::PlayLand { card: CardId(1) },
    );
    reject(&mut game, PlayerId(1), Response::Cast { card: CardId(2) });
    for _ in 0..3 {
        pass(&mut game);
    }
    assert_eq!(game.state.rules.turn.step, TurnStep::End);
    reject(
        &mut game,
        PlayerId(0),
        Response::PlayLand { card: CardId(1) },
    );
    reject(&mut game, PlayerId(0), Response::Cast { card: CardId(2) });
    until(&mut game, 2, TurnStep::Draw);
    assert_eq!(
        game.zone(game.object_id(CardId(5)).unwrap()),
        Some(Zone::Hand { owner: PlayerId(1) })
    );
    reject(
        &mut game,
        PlayerId(1),
        Response::PlayLand { card: CardId(5) },
    );
    until(&mut game, 2, TurnStep::PrecombatMain);
    pass(&mut game);
    pass(&mut game);
    pass(&mut game);
    assert_eq!(game.state.rules.priority, PlayerId(0));
    reject(&mut game, PlayerId(0), Response::Cast { card: CardId(2) });
    reject(
        &mut game,
        PlayerId(0),
        Response::PlayLand { card: CardId(1) },
    );

    let mut game = Game::new(prepared());
    // Pay with private pre-floated mana to leave the land allowance unused.
    // No synthetic setup is admitted by production admission.
    game.state.rules.players[0]
        .mana
        .insert(Color::G, Quantity::from(2));
    let request = game.decision_for(PlayerId(0)).unwrap().unwrap();
    game.respond(
        PlayerId(0),
        request.decision_id(),
        Response::Cast { card: CardId(2) },
    )
    .unwrap();
    let Advance::AwaitingDecision(request) = game.advance(Budget::unlimited()) else {
        panic!()
    };
    game.respond(
        PlayerId(0),
        request.decision_id(),
        Response::FinishMana { undo_mana: true },
    )
    .unwrap();
    let Advance::AwaitingDecision(request) = game.advance(Budget::unlimited()) else {
        panic!()
    };
    let before = game.state.clone();
    assert!(
        game.respond(
            PlayerId(0),
            request.decision_id(),
            Response::PlayLand { card: CardId(1) }
        )
        .is_err()
    );
    assert_eq!(game.state, before);
}

#[test]
fn land_limit_spans_both_mains_and_special_action_resets_passes() {
    let mut game = Game::new(prepared());
    until(&mut game, 2, TurnStep::PrecombatMain);
    // P1 receives priority again after the other three have passed only after
    // an action, so exercise the normal special action and subsequent pass round.
    play(&mut game, 5);
    assert!(game.state.rules.passes.is_empty());
    assert_eq!(game.state.rules.priority, PlayerId(1));
    assert_eq!(game.state.rules.turn.lands_played, 1);
    until(&mut game, 2, TurnStep::PostcombatMain);
    assert_eq!(game.state.rules.turn.lands_played, 1);
    assert_eq!(game.state.rules.turn.number, 2);
    until(&mut game, 3, TurnStep::PrecombatMain);
    assert_eq!(game.state.rules.turn.lands_played, 0);
}

#[test]
fn unpayable_targetless_creature_reversal_keeps_or_undoes_only_casting_mana() {
    for undo_mana in [false, true] {
        let mut game = Game::new(prepared());
        input(&mut game, Response::Cast { card: CardId(2) });
        assert!(matches!(
            settle(&mut game),
            Advance::AwaitingDecision(Request::Mana { .. })
        ));
        input(
            &mut game,
            Response::ActivateMana {
                object: ObjectId {
                    card: CardId(0),
                    incarnation: 0,
                },
            },
        );
        input(&mut game, Response::FinishMana { undo_mana });
        assert!(matches!(
            settle(&mut game),
            Advance::AwaitingDecision(Request::Priority { .. })
        ));
        assert!(game.state.rules.stack.is_empty());
        assert_eq!(game.object_id(CardId(2)).unwrap().incarnation, 0);
        assert_eq!(
            game.zone(game.object_id(CardId(2)).unwrap()),
            Some(Zone::Hand { owner: PlayerId(0) })
        );
        assert_eq!(game.state.rules.objects[&CardId(0)].tapped, !undo_mana);
        assert_eq!(
            game.mana(PlayerId(0), Color::G),
            Some(Quantity::from(i64::from(!undo_mana)))
        );
        assert_eq!(game.state.rules.turn.lands_played, 0);
        let replay = Game::replay(prepared(), &game.replay_journal()).unwrap();
        assert_eq!(replay.state, game.state);
    }
}

#[test]
fn mana_empties_at_step_end_and_untap_and_draw_offer_no_early_priority() {
    let mut game = Game::new(prepared());
    input(
        &mut game,
        Response::ActivateMana {
            object: ObjectId {
                card: CardId(0),
                incarnation: 0,
            },
        },
    );
    until(&mut game, 1, TurnStep::End);
    assert!(game.state.rules.players[0].mana.is_empty());
    assert!(game.state.rules.objects[&CardId(0)].tapped);
    until(&mut game, 2, TurnStep::Upkeep);
    assert_eq!(
        game.zone(game.object_id(CardId(5)).unwrap()),
        Some(Zone::Library { owner: PlayerId(1) })
    );
    until(&mut game, 2, TurnStep::Draw);
    assert_eq!(
        game.zone(game.object_id(CardId(5)).unwrap()),
        Some(Zone::Hand { owner: PlayerId(1) })
    );
    until(&mut game, 5, TurnStep::Upkeep);
    assert!(!game.state.rules.objects[&CardId(0)].tapped);
    assert_eq!(
        game.zone(game.object_id(CardId(4)).unwrap()),
        Some(Zone::Library { owner: PlayerId(0) })
    );
    until(&mut game, 5, TurnStep::PrecombatMain);
    play(&mut game, 1);
    cast_bears(&mut game);
    for _ in 0..4 {
        pass(&mut game);
    }
    assert_eq!(game.state.rules.turn.step, TurnStep::PrecombatMain);
    assert_eq!(
        game.zone(game.object_id(CardId(2)).unwrap()),
        Some(Zone::Battlefield)
    );
}

#[test]
fn queued_departures_preserve_accepted_land_and_cast_or_remove_departing_actor() {
    for response in [
        Response::PlayLand { card: CardId(1) },
        Response::Cast { card: CardId(2) },
    ] {
        for departed in [PlayerId(3), PlayerId(0)] {
            let mut game = Game::new(prepared());
            input(&mut game, response.clone());
            game.concede(departed).unwrap();
            recover(&game);
            assert!(game.decision_for(PlayerId(0)).unwrap().is_none());
            let result = settle(&mut game);
            assert!(matches!(result, Advance::AwaitingDecision(_)));
            assert!(game.state.rules.pending.is_none());
            assert_eq!(game.journal().len(), 1);
            if departed == PlayerId(0) {
                assert!(game.object_id(CardId(1)).is_none());
                assert!(game.object_id(CardId(2)).is_none());
                assert!(game.state.rules.stack.is_empty());
                assert_eq!(game.state.rules.active, None);
                assert_eq!(game.state.rules.turn.number, 1);
                assert_eq!(game.state.rules.turn_owner, PlayerId(0));
            } else if matches!(response, Response::PlayLand { .. }) {
                assert_eq!(
                    game.zone(game.object_id(CardId(1)).unwrap()),
                    Some(Zone::Battlefield)
                );
                assert_eq!(game.state.rules.turn.lands_played, 1);
            } else {
                assert!(matches!(
                    result,
                    Advance::AwaitingDecision(Request::Mana { .. })
                ));
            }
            let replay = Game::replay(prepared(), &game.replay_journal()).unwrap();
            assert_eq!(replay.state, game.state);
        }
    }
}

#[test]
fn departing_last_unpassed_player_resolves_without_a_redundant_pass() {
    let mut game = Game::new(prepared());
    play(&mut game, 1);
    cast_bears(&mut game);
    for _ in 0..3 {
        pass(&mut game);
    }
    assert_eq!(game.state.rules.priority, PlayerId(3));
    game.concede(PlayerId(3)).unwrap();
    let _ = settle(&mut game);
    assert!(game.state.rules.stack.is_empty());
    assert_eq!(
        game.zone(game.object_id(CardId(2)).unwrap()),
        Some(Zone::Battlefield)
    );
    assert_eq!(game.state.rules.priority, PlayerId(0));
    assert_eq!(game.state.rules.turn.step, TurnStep::PostcombatMain);
    assert_eq!(game.state.rules.turn.lands_played, 1);
}

#[test]
fn absent_active_turn_finishes_before_next_live_turn_and_queued_winner_precedes_work() {
    let mut game = Game::new(prepared());
    game.concede(PlayerId(0)).unwrap();
    let _ = settle(&mut game);
    assert_eq!(game.state.rules.active, None);
    assert_eq!(game.state.rules.turn.number, 1);
    assert_eq!(game.state.rules.turn.step, TurnStep::PostcombatMain);
    assert_eq!(game.state.rules.priority, PlayerId(1));
    until(&mut game, 1, TurnStep::End);
    assert_eq!(game.state.rules.active, None);
    until(&mut game, 2, TurnStep::Upkeep);
    assert_eq!(game.state.rules.active, Some(PlayerId(1)));
    assert_eq!(game.state.rules.turn_owner, PlayerId(1));
    assert_eq!(
        game.zone(game.object_id(CardId(5)).unwrap()),
        Some(Zone::Library { owner: PlayerId(1) })
    );
    let replay = Game::replay(prepared(), &game.replay_journal()).unwrap();
    assert_eq!(replay.state, game.state);

    let mut game = Game::new(prepared());
    for _ in 0..3 {
        pass(&mut game);
    }
    input(&mut game, Response::Pass);
    for player in [PlayerId(1), PlayerId(2), PlayerId(3)] {
        game.concede(player).unwrap();
    }
    assert_eq!(
        settle(&mut game),
        Advance::Finished {
            winner: PlayerId(0)
        }
    );
    assert_eq!(game.state.rules.turn.number, 1);
    assert_eq!(game.state.rules.turn.step, TurnStep::PostcombatMain);
    assert_eq!(
        Game::replay(prepared(), &game.replay_journal())
            .unwrap()
            .state,
        game.state
    );
}

#[test]
fn public_land_queries_recover_and_handles_tokens_and_receipts_expire() {
    let mut game = Game::new(prepared());
    let own = observe(&game, PlayerId(0));
    assert_eq!(own.observation_version, 28);
    let token = own.decision.unwrap().token().clone();
    let hand = game
        .handle_for(PlayerId(0), game.object_id(CardId(1)).unwrap())
        .unwrap();
    let QueryResult::Incomplete { work, .. } = game.check_player_response(
        PlayerId(0),
        token.clone(),
        PlayerResponse::PlayLand {
            object: hand.clone(),
        },
        Budget::new(0),
    ) else {
        panic!()
    };
    let saved = game.save_work(&work).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&saved).unwrap()["work_version"],
        27
    );
    let restored = Game::restore(prepared(), &game.snapshot().unwrap()).unwrap();
    let recovered_work = restored.restore_work(&saved).unwrap();
    assert!(matches!(
        finish_query(
            &restored,
            restored.resume_work(recovered_work, Budget::new(1))
        ),
        QueryValue::Validated(_)
    ));
    let QueryValue::Validated(receipt) =
        finish_query(&game, game.resume_work(work, Budget::new(1)))
    else {
        panic!()
    };
    let QueryResult::Incomplete { work: stale, .. } = game.observe(PlayerId(0), Budget::new(0))
    else {
        panic!()
    };
    let foreign = game
        .check_response(
            PlayerId(0),
            game.decision_for(PlayerId(0))
                .unwrap()
                .unwrap()
                .decision_id(),
            Response::PlayLand { card: CardId(1) },
        )
        .unwrap();
    let mut fork = game.fork();
    assert!(fork.submit(foreign).is_err());
    game.submit(receipt).unwrap();
    assert!(matches!(
        game.resume_work(stale, Budget::unlimited()),
        QueryResult::Rejected(QueryRejection::StaleOrForeignWork)
    ));
    let _ = settle(&mut game);
    assert!(matches!(
        game.check_player_response(
            PlayerId(0),
            token,
            PlayerResponse::PlayLand {
                object: hand.clone()
            },
            Budget::unlimited()
        ),
        QueryResult::Rejected(QueryRejection::InvalidResponse)
    ));
    let own = observe(&game, PlayerId(0));
    let token = own.decision.unwrap().token().clone();
    assert!(matches!(
        game.check_player_response(
            PlayerId(0),
            token,
            PlayerResponse::ActivateMana { object: hand },
            Budget::unlimited()
        ),
        QueryResult::Rejected(QueryRejection::InvalidResponse)
    ));
    assert_ne!(game.state, fork.state);
    assert_eq!(fork.object_id(CardId(1)).unwrap().incarnation, 0);
}

#[test]
fn draw_visibility_queries_and_search_are_pure_and_do_not_reveal_libraries_or_other_hands() {
    let mut game = Game::new(prepared());
    until(&mut game, 2, TurnStep::Draw);
    let before = game.snapshot().unwrap();
    for player in [
        PlayerId(3),
        PlayerId(1),
        PlayerId(0),
        PlayerId(2),
        PlayerId(1),
    ] {
        let own = observe(&game, player);
        assert!(
            own.view
                .objects
                .iter()
                .all(|o| !matches!(o.zone, Zone::Library { .. }))
        );
        assert!(
            own.view
                .objects
                .iter()
                .all(|o| !matches!(o.zone, Zone::Hand { owner } if owner != player))
        );
        assert_eq!(own.view.players[1].hand_count, 1);
        assert_eq!(own.view.players[1].library_count, 0);
        assert_eq!(
            own.view
                .objects
                .iter()
                .filter(|o| matches!(o.zone, Zone::Hand { owner: PlayerId(1) }))
                .count(),
            usize::from(player == PlayerId(1))
        );
        assert!(matches!(
            finish_query(
                &game,
                game.search_context(player, "turns-search".into(), Budget::new(1))
            ),
            QueryValue::SearchContext(_)
        ));
    }
    assert_eq!(game.snapshot().unwrap(), before);
}

#[test]
fn scheduler_snapshot_rejects_rehashed_forged_turn_land_and_frame_facts() {
    let mut game = Game::new(prepared());
    play(&mut game, 1);
    let snapshot = game.snapshot().unwrap();
    for mutation in 0..5 {
        let mut saved: Snapshot = strict_read(&snapshot).unwrap();
        match mutation {
            0 => saved.state.rules.turn.number += 1,
            1 => saved.state.rules.turn.step = TurnStep::PrecombatMain,
            2 => saved.state.rules.turn.lands_played = 0,
            3 => saved.state.rules.frame = Frame::EnterStep,
            _ => {
                saved.state.rules.objects.get_mut(&CardId(4)).unwrap().zone =
                    Zone::Hand { owner: PlayerId(0) }
            }
        }
        saved.state_sha256 = digest(&saved.state).unwrap();
        assert!(Game::restore(prepared(), &canonical(&saved).unwrap()).is_err());
    }
}

#[test]
fn creature_combat_and_empty_library_are_explicit_recoverable_faults() {
    let mut game = Game::new(prepared());
    play(&mut game, 1);
    cast_bears(&mut game);
    resolve_bears(&mut game);
    until(&mut game, 5, TurnStep::BeginningCombat);
    for _ in 0..4 {
        pass(&mut game);
    }
    assert!(matches!(
        settle(&mut game),
        Advance::AwaitingDecision(Request::Attackers { .. })
    ));
    recover(&game);
    let mut game = Game::new(prepared());
    until(&mut game, 6, TurnStep::Upkeep);
    for _ in 0..4 {
        pass(&mut game);
    }
    assert!(matches!(settle(&mut game), Advance::AwaitingDecision(_)));
    assert_eq!(game.state.rules.players[1].status, PlayerStatus::Left);
    recover(&game);
}

#[test]
fn admission_does_not_accept_variants_or_enable_new_actions_in_old_profiles() {
    let catalog = prepared().catalog.as_ref().clone();
    for mutation in 0..4 {
        let mut config = ScenarioConfig::turns_permanents();
        match mutation {
            0 => config.cards[4].owner = PlayerId(1),
            1 => config.cards[1].oracle_id = ISLAND.into(),
            2 => config.cards[2].zone = Zone::Battlefield,
            _ => config.cards.push(config.cards[4].clone()),
        }
        assert!(Prepared::admit(catalog.clone(), config).is_err());
    }
    let mut old = Game::new(Prepared::admit(catalog, ScenarioConfig::land_return()).unwrap());
    reject(
        &mut old,
        PlayerId(0),
        Response::PlayLand { card: CardId(1) },
    );
    reject(&mut old, PlayerId(0), Response::Cast { card: CardId(2) });
}

fn scripted_response(game: &Game, request: &Request) -> Response {
    match request {
        Request::Attackers { .. } => return Response::Attackers { attacks: vec![] },
        Request::Blockers { .. } => return Response::Blockers { blocks: vec![] },
        Request::SpendMana {
            player,
            cost,
            available,
            ..
        } => {
            let remaining = game.payment_pool(*player, cost).unwrap();
            return Response::SpendMana {
                spending: available
                    .iter()
                    .filter_map(|(color, amount)| {
                        let mut n = amount.clone();
                        n.subtract(&remaining.get(color).cloned().unwrap_or_default());
                        (n > Quantity::zero()).then_some((*color, n))
                    })
                    .collect(),
            };
        }
        _ => (),
    }

    if let Request::Mana { .. } = request {
        for card in [CardId(0), CardId(1)] {
            let object = &game.state.rules.objects[&card];
            if !object.tapped {
                return Response::ActivateMana { object: object.id };
            }
        }
        return Response::FinishMana { undo_mana: true };
    }
    if game.state.rules.turn.number == 1 && request.player() == PlayerId(0) {
        if matches!(game.state.rules.objects[&CardId(1)].zone, Zone::Hand { .. }) {
            return Response::PlayLand { card: CardId(1) };
        }
        if matches!(game.state.rules.objects[&CardId(2)].zone, Zone::Hand { .. }) {
            return Response::Cast { card: CardId(2) };
        }
    }
    if game.state.rules.turn.number == 5
        && game.state.rules.turn.step == TurnStep::PrecombatMain
        && request.player() == PlayerId(0)
    {
        return Response::PlayLand { card: CardId(3) };
    }
    Response::Pass
}

fn run_script(mode: TraceMode, budget: u64, recover_each: bool) -> Game {
    let mut game = Game::with_trace(prepared(), mode);
    for _ in 0..6000 {
        let outcome = game.advance(Budget::new(budget));
        if recover_each {
            recover(&game);
        }
        match outcome {
            Advance::Suspended { .. } => continue,
            Advance::AwaitingDecision(request) => {
                if game.state.rules.turn.number == 5
                    && game.state.rules.turn.step == TurnStep::PrecombatMain
                    && game.state.rules.turn.lands_played == 1
                {
                    return game;
                }
                let response = scripted_response(&game, &request);
                game.respond(request.player(), request.decision_id(), response)
                    .unwrap();
                if recover_each {
                    recover(&game);
                }
            }
            other => panic!("script failed: {other:?}"),
        }
    }
    panic!("script exceeded bounded work")
}

#[test]
fn turn_loop_is_budget_partition_and_trace_invariant_with_recovery() {
    let reference = run_script(TraceMode::Detailed, u64::MAX, false);
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        let game = run_script(mode, 1, true);
        assert_eq!(game.state, reference.state);
        let replay = Game::replay_with_trace(prepared(), &game.replay_journal(), mode).unwrap();
        assert_eq!(replay.state, game.state);
        assert_eq!(replay.trace, game.trace);
        if mode == TraceMode::Detailed {
            assert_eq!(game.trace, reference.trace);
        }
    }
    let game = run_script(TraceMode::Detailed, 3, false);
    assert_eq!(game.state, reference.state);
    assert_eq!(game.trace, reference.trace);
}

#[test]
fn concessions_at_each_casting_and_scheduler_stage_remain_recoverable() {
    let mut game = Game::new(prepared());
    let mut samples = BTreeMap::new();
    for _ in 0..6000 {
        // Distinct pending/automatic stages for each step; one representative
        // seat rotation suffices here, with all four possible departing seats.
        let frame = serde_json::to_value(&game.state.rules.frame).unwrap();
        let key = format!(
            "{:?}/{}/{:?}",
            game.state.rules.turn.step,
            frame["stage"],
            game.state.rules.pending.as_ref().map(|p| &p.response)
        );
        samples
            .entry(key)
            .or_insert_with(|| game.snapshot().unwrap());
        match game.advance(Budget::new(1)) {
            Advance::Suspended { .. } => {
                if matches!(game.state.rules.frame, Frame::EnterStep | Frame::Transition) {
                    for player in 0..4 {
                        assert!(game.decision_for(PlayerId(player)).unwrap().is_none());
                    }
                }
            }
            Advance::AwaitingDecision(request) => {
                if game.state.rules.turn.number == 3 {
                    break;
                }
                let response = scripted_response(&game, &request);
                game.respond(request.player(), request.decision_id(), response)
                    .unwrap();
            }
            other => panic!("unexpected sample outcome: {other:?}"),
        }
    }
    assert!(samples.len() >= 20, "missing scheduler/casting boundaries");
    for (boundary, snapshot) in samples {
        for player in 0..4 {
            let mut branch = Game::restore(prepared(), &snapshot).unwrap();
            branch.concede(PlayerId(player)).unwrap();
            let result = settle(&mut branch);
            assert!(
                matches!(
                    result,
                    Advance::AwaitingDecision(_) | Advance::Finished { .. }
                ),
                "{boundary}: {result:?}"
            );
            let replay = Game::replay(prepared(), &branch.replay_journal()).unwrap();
            assert_eq!(
                replay.state, branch.state,
                "departure {player} at {boundary}"
            );
            assert_eq!(
                replay.trace, branch.trace,
                "departure {player} at {boundary}"
            );
        }
    }
}

#[test]
fn zero_budget_and_forks_leave_pending_land_and_turn_work_independent() {
    let mut game = Game::new(prepared());
    input(&mut game, Response::PlayLand { card: CardId(1) });
    let before = game.snapshot().unwrap();
    assert_eq!(
        game.advance(Budget::new(0)),
        Advance::Suspended {
            required_minimum: 1
        }
    );
    assert_eq!(game.snapshot().unwrap(), before);
    let mut fork = game.fork();
    fork.concede(PlayerId(0)).unwrap();
    let _ = settle(&mut fork);
    let _ = settle(&mut game);
    assert_eq!(
        game.zone(game.object_id(CardId(1)).unwrap()),
        Some(Zone::Battlefield)
    );
    assert!(fork.object_id(CardId(1)).is_none());
    assert_eq!(game.state.rules.active, Some(PlayerId(0)));
    assert_eq!(fork.state.rules.active, None);
    assert_eq!(game.state.rules.turn.lands_played, 1);
    assert_eq!(fork.state.rules.turn.lands_played, 0);
}

#[test]
fn generic_cleanup_discard_and_ordered_library_draw_are_available() {
    for cleanup in [false, true] {
        // Private interpreter fixtures prove defensive runtime rejection; the
        // exact production admission rejects either larger configuration.
        let mut p = prepared();
        for extra in 0..if cleanup { 5 } else { 1 } {
            let owner = PlayerId(if cleanup { 0 } else { 1 });
            p.config.cards.push(CardSetup {
                id: CardId(8 + extra),
                oracle_id: FOREST.into(),
                owner,
                controller: owner,
                zone: if cleanup {
                    Zone::Hand { owner }
                } else {
                    Zone::Library { owner }
                },
            });
        }
        assert!(Prepared::admit(p.catalog.as_ref().clone(), p.config.clone()).is_err());
        let mut game = Game::new(p.clone());
        let boundary = if cleanup {
            (1, TurnStep::End)
        } else {
            (2, TurnStep::Upkeep)
        };
        until(&mut game, boundary.0, boundary.1);
        for _ in 0..3 {
            pass(&mut game);
        }
        input(&mut game, Response::Pass);
        let outcome = settle(&mut game);
        if cleanup {
            assert!(matches!(
                outcome,
                Advance::AwaitingDecision(Request::Discard { count: 1, .. })
            ));
            assert_eq!(
                game.state
                    .rules
                    .objects
                    .values()
                    .filter(|o| o.zone == Zone::Hand { owner: PlayerId(0) })
                    .count(),
                8
            );
        } else {
            assert!(matches!(outcome, Advance::AwaitingDecision(_)));
            assert_eq!(
                game.state
                    .rules
                    .objects
                    .values()
                    .filter(|o| o.zone == Zone::Library { owner: PlayerId(1) })
                    .count(),
                1
            );
        }
        let replay = Game::replay(p, &game.replay_journal()).unwrap();
        assert_eq!(replay.state, game.state);
    }
}

#[test]
fn unlimited_hand_size_tracks_current_controller_and_ends_after_paid_bounce() {
    let mut p = prepared();
    let catalog = Arc::make_mut(&mut p.catalog);
    let source = catalog
        .definitions
        .iter_mut()
        .find(|definition| definition.oracle_id == BEARS)
        .unwrap();
    // Private synthetic grant on an existing creature shape.
    source.static_abilities.push(StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::UnlimitedControllerHandSize,
    });
    source.capabilities = source.requirements();
    source.validate().unwrap();
    p.catalog_digest = digest(catalog).unwrap();

    let controller = PlayerId(0);
    let owner = PlayerId(1);
    let source = CardId(2);
    p.config.cards[2].owner = owner;
    p.config.cards[2].zone = Zone::Battlefield;
    let mana_source = CardId(8);
    let bounce = CardId(9);
    p.config.cards.extend([
        CardSetup {
            id: mana_source,
            oracle_id: ISLAND.into(),
            owner,
            controller: owner,
            zone: Zone::Battlefield,
        },
        CardSetup {
            id: bounce,
            oracle_id: SPELL.into(),
            owner,
            controller: owner,
            zone: Zone::Hand { owner },
        },
    ]);
    for (player, cards) in [(controller, 10..16), (owner, 16..22)] {
        p.config.cards.extend(cards.map(|id| CardSetup {
            id: CardId(id),
            oracle_id: FOREST.into(),
            owner: player,
            controller: player,
            zone: Zone::Hand { owner: player },
        }));
    }
    p.profile_digest = digest(&p.config).unwrap();

    let hand_count = |game: &Game, owner| {
        game.state
            .rules
            .objects
            .values()
            .filter(|object| object.zone == Zone::Hand { owner })
            .count()
    };
    let mut game = Game::new(p.clone());
    until(&mut game, 1, TurnStep::End);
    assert_eq!(hand_count(&game, controller), 8);
    until(&mut game, 2, TurnStep::PrecombatMain);
    assert_eq!(hand_count(&game, controller), 8);
    assert_eq!(hand_count(&game, owner), 8);

    let mana_object = game.object_id(mana_source).unwrap();
    let source_object = game.object_id(source).unwrap();
    input(
        &mut game,
        Response::ActivateMana {
            object: mana_object,
        },
    );
    input(&mut game, Response::Cast { card: bounce });
    input(
        &mut game,
        Response::Targets {
            objects: vec![source_object],
        },
    );
    input(&mut game, Response::FinishMana { undo_mana: false });
    assert!(matches!(settle(&mut game), Advance::AwaitingDecision(_)));
    assert!(game.state.rules.stack[0].paid.is_some());
    for _ in 0..4 {
        pass(&mut game);
    }
    assert_eq!(
        game.zone(game.object_id(source).unwrap()),
        Some(Zone::Hand { owner })
    );
    until(&mut game, 2, TurnStep::End);
    for _ in 0..4 {
        pass(&mut game);
    }
    let Advance::AwaitingDecision(Request::Discard {
        count,
        player,
        candidates,
        ..
    }) = settle(&mut game)
    else {
        panic!("expected cleanup discard after the source left the battlefield");
    };
    assert_eq!(player, owner);
    assert_eq!(count, 1);
    assert_eq!(candidates.len(), 8);
    input(
        &mut game,
        Response::Discard {
            objects: vec![candidates[0]],
        },
    );
    assert!(matches!(settle(&mut game), Advance::AwaitingDecision(_)));
    assert_eq!(hand_count(&game, owner), 7);
    assert_eq!(
        Game::replay(p, &game.replay_journal()).unwrap().state,
        game.state
    );
}

#[test]
fn reduced_hand_size_applies_during_cleanup_with_unlimited_sources_and_exact_floor() {
    let mut p = prepared();
    let catalog = Arc::make_mut(&mut p.catalog);
    let creature = catalog.definition_by_oracle_id(BEARS).unwrap().clone();
    let controller_reduction = "test-hand-size-controller";
    let opponent_reduction = "test-hand-size-opponents";
    let unlimited = "test-hand-size-unlimited";
    for (id, effect) in [
        (
            controller_reduction,
            StaticEffect::ReduceMaximumHandSize {
                scope: HandSizeScope::Controller,
                amount: Quantity::from(3),
            },
        ),
        (
            opponent_reduction,
            StaticEffect::ReduceMaximumHandSize {
                scope: HandSizeScope::Opponents,
                amount: Quantity::from(5),
            },
        ),
        (unlimited, StaticEffect::UnlimitedControllerHandSize),
    ] {
        let mut source = creature.clone();
        source.oracle_id = id.into();
        source.name = id.into();
        source.static_abilities.push(StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect,
        });
        source.capabilities = source.requirements();
        catalog.definitions.push(source);
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    p.catalog_digest = digest(catalog).unwrap();

    let player = PlayerId(0);
    let opponent = PlayerId(1);
    let inactive = Zone::Graveyard { owner: player };
    p.config.initial.turn_step = TurnStep::End;
    p.config.cards[0].oracle_id = unlimited.into();
    p.config.cards[0].zone = inactive;
    p.config.cards.extend(
        [
            (8, controller_reduction, opponent, player, Zone::Battlefield),
            (9, opponent_reduction, player, opponent, Zone::Battlefield),
            (10, unlimited, player, player, inactive),
            (11, FOREST, player, player, Zone::Hand { owner: player }),
        ]
        .map(|(id, oracle_id, owner, controller, zone)| CardSetup {
            id: CardId(id),
            oracle_id: oracle_id.into(),
            owner,
            controller,
            zone,
        }),
    );

    // Unlimited persists whether its source precedes or follows the reducers.
    for index in [0, 10] {
        let mut fixture = p.clone();
        fixture.config.cards[index].zone = Zone::Battlefield;
        let (game, boundary) = start_cleanup(fixture);
        assert_eq!(game.active_player(), Some(opponent));
        assert!(matches!(
            boundary,
            Advance::AwaitingDecision(Request::Priority { .. })
        ));
    }
    let (mut game, boundary) = start_cleanup(p);
    let Advance::AwaitingDecision(Request::Discard {
        player: discarding_player,
        decision_id,
        count,
        candidates,
    }) = boundary
    else {
        panic!("expected cleanup discard");
    };
    assert_eq!(discarding_player, player);
    assert_eq!(count, 4);
    assert_eq!(candidates.len(), 4);
    game.respond(
        player,
        decision_id,
        Response::Discard {
            objects: candidates.clone(),
        },
    )
    .unwrap();
    assert!(matches!(
        game.advance(Budget::new(100)),
        Advance::AwaitingDecision(Request::Priority { .. })
    ));
    assert_eq!(game.active_player(), Some(opponent));
    assert!(
        candidates
            .iter()
            .all(|object| game.zone(game.object_id(object.card).unwrap()) == Some(inactive))
    );
    recover(&game);
}

fn start_cleanup(mut fixture: Prepared) -> (Game, Advance) {
    fixture.profile_digest = digest(&fixture.config).unwrap();
    let mut game = Game::new(fixture);
    for _ in 0..4 {
        let Advance::AwaitingDecision(request @ Request::Priority { .. }) =
            game.advance(Budget::new(100))
        else {
            panic!("expected end-step priority");
        };
        game.respond(request.player(), request.decision_id(), Response::Pass)
            .unwrap();
    }
    let boundary = game.advance(Budget::new(100));
    (game, boundary)
}

#[test]
fn flash_end_step_draw() {
    let controller = PlayerId(0);
    let opponent = PlayerId(1);
    let mut fixture = prepared();
    let catalog = Arc::make_mut(&mut fixture.catalog);
    // Private board fixture. Compilation and source fidelity have their own witness.
    let mut source = catalog.definition_by_oracle_id(BEARS).unwrap().clone();
    source.oracle_id = "test-flash-end-step-draw".into();
    source.name = source.oracle_id.clone();
    source.source_labels = None;
    source.characteristics.legendary = true;
    source.keywords = vec![KeywordAbility::Flash];
    source.spell.as_mut().unwrap().timing = SpellTiming::Instant;
    source.static_abilities = vec![StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::ReduceMaximumHandSize {
            scope: HandSizeScope::Opponents,
            amount: 7.into(),
        },
    }];
    source.triggered = vec![TriggeredAbility {
        death_filter: None,
        functioning_zone: FunctioningZone::Battlefield,
        condition: TriggerCondition::StepBegan {
            step: BeginningStep::End,
            turn_relation: TurnRelation::Controller,
        },
        intervening_if: None,
        program: SpellProgram {
            additional_sacrifice: None,
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            functioning_zone: FunctioningZone::Stack,
            targets: vec![],
            instructions: vec![Instruction::DrawControllerCards {
                count: 7.into(),
                source_block: 1,
            }],
            modes: vec![],
        },
    }];
    source.capabilities = source.requirements();
    fixture.config.cards[0].oracle_id = source.oracle_id.clone();
    catalog.definitions.push(source);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    fixture.catalog_digest = digest(catalog).unwrap();

    for card in &mut fixture.config.cards[1..4] {
        card.zone = Zone::Library { owner: controller };
    }
    fixture.config.cards.extend((8..12).map(|id| CardSetup {
        id: CardId(id),
        oracle_id: FOREST.into(),
        owner: controller,
        controller,
        zone: Zone::Library { owner: controller },
    }));
    fixture.profile_digest = digest(&fixture.config).unwrap();

    let mut game = Game::new(fixture);
    let libraries = game.state.rules.libraries.clone();
    let pass_priority = |game: &mut Game| {
        let Advance::AwaitingDecision(request @ Request::Priority { .. }) =
            game.advance(Budget::new(100))
        else {
            panic!("expected priority");
        };
        game.respond(request.player(), request.decision_id(), Response::Pass)
            .unwrap();
    };
    assert_eq!(game.state.rules.turn.step, TurnStep::PostcombatMain);
    for _ in 0..4 {
        pass_priority(&mut game);
    }
    assert!(matches!(
        game.advance(Budget::new(100)),
        Advance::AwaitingDecision(Request::Priority { player, .. }) if player == controller
    ));
    assert!(matches!(
        game.state.rules.stack.as_slice(),
        [StackInstance {
            controller: player,
            kind: StackKind::Triggered {
                match_condition: None,
                witness: TriggerEventWitness::StepBegan { turn, active: Some(active), .. },
                ..
            },
            ..
        }] if *player == controller && *active == controller
            && turn.number == 1 && turn.step == TurnStep::End
    ));
    for _ in 0..4 {
        pass_priority(&mut game);
    }
    for _ in 0..100 {
        if matches!(game.state.rules.frame, Frame::Resolve { .. }) {
            break;
        }
        assert!(matches!(
            game.advance(Budget::new(1)),
            Advance::Suspended { .. }
        ));
    }
    assert!(matches!(game.state.rules.frame, Frame::Resolve { .. }));
    for completed in 1..=7 {
        assert!(matches!(
            game.advance(Budget::new(1)),
            Advance::Suspended { .. }
        ));
        let Frame::Resolve { frame } = &game.state.rules.frame else {
            panic!("counted draw left resolution after attempt {completed}");
        };
        let library = &libraries[&controller];
        let drawn = game.object_id(library[completed - 1]).unwrap();
        assert_eq!(game.zone(drawn), Some(Zone::Hand { owner: controller }));
        assert_eq!(
            game.state.rules.libraries[&controller],
            library[completed..]
        );
        if completed == 7 {
            assert_eq!(frame.draw_progress, None);
            assert!(matches!(
                frame.outcomes.as_slice(),
                [InstructionOutcome::DrawAttempts { player, attempted, failed }]
                    if *player == controller && *attempted == 7.into() && *failed == 0.into()
            ));
        } else {
            let progress = frame.draw_progress.as_ref().unwrap();
            assert_eq!(progress.source_block, 1);
            assert_eq!(progress.completed, (completed as u32).into());
            assert_eq!(progress.remaining, (7 - completed as u32).into());
        }
        if completed == 3 {
            recover(&game);
        }
    }
    for player in [opponent, PlayerId(2), PlayerId(3)] {
        assert_eq!(game.state.rules.libraries[&player], libraries[&player]);
        assert!(game.action_hand(player).is_empty());
    }

    for _ in 0..64 {
        let Advance::AwaitingDecision(request) = game.advance(Budget::new(100)) else {
            panic!("expected turn decision");
        };
        let response = match &request {
            Request::Discard { player, count, .. } => {
                assert_eq!(*player, opponent);
                assert_eq!(*count, 1);
                assert_eq!(game.state.rules.turn.step, TurnStep::Cleanup);
                assert_eq!(game.action_hand(controller).len(), 7);
                assert_eq!(game.state.rules.next_trigger_id, 1);
                return;
            }
            Request::Priority { .. } => Response::Pass,
            Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
            Request::Blockers { .. } => Response::Blockers { blocks: vec![] },
            other => panic!("unexpected turn decision {other:?}"),
        };
        game.respond(request.player(), request.decision_id(), response)
            .unwrap();
    }
    panic!("missing opponent cleanup");
}

#[test]
fn increased_hand_size_applies_during_cleanup_with_exact_signed_totals() {
    let mut p = prepared();
    let catalog = Arc::make_mut(&mut p.catalog);
    let creature = catalog.definition_by_oracle_id(BEARS).unwrap().clone();
    let increase = "test-hand-size-increase";
    let reduction = "test-hand-size-reduction";
    let huge_increase = "test-hand-size-huge-increase";
    for (id, effect) in [
        (
            increase,
            StaticEffect::IncreaseMaximumHandSize {
                amount: Quantity::from(4),
            },
        ),
        (
            reduction,
            StaticEffect::ReduceMaximumHandSize {
                scope: HandSizeScope::Opponents,
                amount: Quantity::from(9),
            },
        ),
        (
            huge_increase,
            StaticEffect::IncreaseMaximumHandSize {
                amount: "10000000000000000000000000000000000000000".parse().unwrap(),
            },
        ),
    ] {
        let mut source = creature.clone();
        source.oracle_id = id.into();
        source.name = id.into();
        source.static_abilities.push(StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect,
        });
        source.capabilities = source.requirements();
        catalog.definitions.push(source);
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    p.catalog_digest = digest(catalog).unwrap();

    let player = PlayerId(0);
    let opponent = PlayerId(1);
    p.config.initial.turn_step = TurnStep::End;
    p.config.cards.extend((10..19).map(|id| CardSetup {
        id: CardId(id),
        oracle_id: FOREST.into(),
        owner: player,
        controller: player,
        zone: Zone::Hand { owner: player },
    }));

    for (first, second, expected_count) in [
        (increase, FOREST, 1),
        // 7 - 9 + 4 leaves two cards in either source order.
        (reduction, increase, 10),
        (increase, reduction, 10),
        (huge_increase, FOREST, 0),
    ] {
        let mut fixture = p.clone();
        fixture
            .config
            .cards
            .extend(
                [first, second]
                    .into_iter()
                    .zip(8..10)
                    .map(|(oracle_id, id)| CardSetup {
                        id: CardId(id),
                        oracle_id: oracle_id.into(),
                        owner: opponent,
                        controller: if oracle_id == reduction {
                            opponent
                        } else {
                            player
                        },
                        zone: Zone::Battlefield,
                    }),
            );
        fixture.config.cards.sort_by_key(|card| card.id);
        let (mut game, boundary) = start_cleanup(fixture);
        if expected_count == 0 {
            assert_eq!(game.active_player(), Some(opponent));
            assert!(matches!(
                boundary,
                Advance::AwaitingDecision(Request::Priority { .. })
            ));
            continue;
        }
        let Advance::AwaitingDecision(Request::Discard {
            player: discarding_player,
            decision_id,
            count,
            candidates,
        }) = boundary
        else {
            panic!("expected cleanup discard");
        };
        assert_eq!(discarding_player, player);
        assert_eq!(count, expected_count);
        assert_eq!(candidates.len(), 12);
        game.respond(
            player,
            decision_id,
            Response::Discard {
                objects: candidates.into_iter().take(count).collect(),
            },
        )
        .unwrap();
        assert!(matches!(
            game.advance(Budget::new(100)),
            Advance::AwaitingDecision(Request::Priority { .. })
        ));
        assert_eq!(game.active_player(), Some(opponent));
        assert_eq!(
            game.state
                .rules
                .objects
                .values()
                .filter(|object| object.zone == Zone::Hand { owner: player })
                .count(),
            12 - count
        );
        if expected_count == 1 {
            recover(&game);
        }
    }
}

#[test]
fn land_movement_and_counting_batch_failure_is_atomic() {
    let mut game = Game::new(prepared());
    let before = game.snapshot().unwrap();
    let result = game.commit_batch(
        BatchCause::Fixture,
        EventGroup::Ordered {
            children: vec![
                EventGroup::Occurrence {
                    proposed: ProposedEvent::Move {
                        object: ObjectId {
                            card: CardId(1),
                            incarnation: 0,
                        },
                        destination: Zone::Battlefield,
                    },
                },
                EventGroup::Occurrence {
                    proposed: ProposedEvent::LandPlayed {
                        player: PlayerId(1),
                    },
                },
            ],
        },
    );
    assert!(result.is_err());
    assert_eq!(game.snapshot().unwrap(), before);
    assert_eq!(game.state.rules.turn.lands_played, 0);
    assert_eq!(game.object_id(CardId(1)).unwrap().incarnation, 0);
}
