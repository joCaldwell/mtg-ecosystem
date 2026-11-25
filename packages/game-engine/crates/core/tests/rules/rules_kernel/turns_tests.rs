use super::*;

fn input(game: &mut Game, response: Answer) {
    let finish = matches!(
        response,
        Answer::Pay {
            pay: PayAnswer::Finish { .. }
        }
    );
    let Advance::AwaitingDecision(request) = Driver::settle(game, u64::MAX) else {
        panic!("missing decision at {:?}", game.state.rules.frame)
    };
    let response = if response
        == (Answer::Pick {
            picks: vec![vec![0]],
        }) {
        match &request {
            Decision {
                reason: Reason::Attackers,
                ..
            } => request.attack(vec![]),
            Decision {
                reason: Reason::Blockers,
                ..
            } => request.block(vec![]),
            _ => response,
        }
    } else {
        response
    };
    game.respond(request.player, request.id, response).unwrap();
    if finish {
        game.complete_test_mana();
    }
}

fn pass(game: &mut Game) {
    input(
        game,
        Answer::Pick {
            picks: vec![vec![0]],
        },
    );
    let _ = Driver::settle(game, u64::MAX);
}

fn until(game: &mut Game, number: u64, step: TurnStep) {
    for _ in 0..180 {
        let result = Driver::settle(game, u64::MAX);
        assert!(matches!(result, Advance::AwaitingDecision(_)), "{result:?}");
        if game.state.rules.turn.number == number && game.state.rules.turn.step == step {
            return;
        }
        input(
            game,
            Answer::Pick {
                picks: vec![vec![0]],
            },
        );
    }
    panic!("did not reach turn {number}, {step:?}")
}

fn play(game: &mut Game, card: u32) {
    input(game, game.next_test_decision().land(CardId(card)));
    assert!(matches!(
        Driver::settle(game, u64::MAX),
        Advance::AwaitingDecision(_)
    ));
}

fn cast_bears(game: &mut Game) {
    input(game, game.next_test_decision().cast(CardId(2)));
    assert!(matches!(
        Driver::settle(game, u64::MAX),
        Advance::AwaitingDecision(Decision {
            player: PlayerId(0),
            reason: Reason::Mana { .. },
            ..
        })
    ));
    for card in [0, 1] {
        input(
            game,
            game.next_test_decision()
                .activate_mana(game.object_id(CardId(card)).unwrap()),
        );
    }
    input(
        game,
        Answer::Pay {
            pay: PayAnswer::Finish {
                undo_on_failure: true,
            },
        },
    );
    assert!(matches!(
        Driver::settle(game, u64::MAX),
        Advance::AwaitingDecision(_)
    ));
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

fn reject(game: &mut Game, player: PlayerId, response: Answer) {
    let Advance::AwaitingDecision(request) = Driver::settle(game, u64::MAX) else {
        panic!()
    };
    let before = game.snapshot().unwrap();
    assert!(game.respond(player, request.id, response).is_err());
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
    let mut game = Game::new(FixtureCatalog::Base.admit(ScenarioConfig::turns_permanents()));
    assert_eq!(game.state.rules.turn.number, 1);
    assert_eq!(game.state.rules.turn.step, TurnStep::PostcombatMain);
    play(&mut game, 1);
    assert_eq!(game.state.rules.turn.lands_played, 1);
    assert!(game.state.rules.stack.is_empty());
    {
        let selected = game.next_test_decision().land(CardId(3));
        reject(&mut game, PlayerId(0), selected)
    };
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
    {
        let selected = game.next_test_decision().land(CardId(4));
        reject(&mut game, PlayerId(0), selected)
    };
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
    let replay = Game::replay(
        FixtureCatalog::Base.admit(ScenarioConfig::turns_permanents()),
        &game.replay_journal(),
    )
    .unwrap();
    assert_eq!(replay.state, game.state);
    assert_eq!(replay.trace, game.trace);
}

#[test]
fn lands_and_creatures_reject_nonactive_timing_stack_and_wrong_source() {
    let mut game = Game::new(FixtureCatalog::Base.admit(ScenarioConfig::turns_permanents()));
    for response in [
        game.next_test_decision().land(CardId(2)),
        game.next_test_decision().land(CardId(0)),
        game.next_test_decision().land(CardId(4)),
        game.next_test_decision().cast(CardId(1)),
    ] {
        reject(&mut game, PlayerId(0), response);
    }
    pass(&mut game);
    {
        let selected = game.next_test_decision().land(CardId(1));
        reject(&mut game, PlayerId(1), selected)
    };
    {
        let selected = game.next_test_decision().cast(CardId(2));
        reject(&mut game, PlayerId(1), selected)
    };
    for _ in 0..3 {
        pass(&mut game);
    }
    assert_eq!(game.state.rules.turn.step, TurnStep::End);
    {
        let selected = game.next_test_decision().land(CardId(1));
        reject(&mut game, PlayerId(0), selected)
    };
    {
        let selected = game.next_test_decision().cast(CardId(2));
        reject(&mut game, PlayerId(0), selected)
    };
    until(&mut game, 2, TurnStep::Draw);
    assert_eq!(
        game.zone(game.object_id(CardId(5)).unwrap()),
        Some(Zone::Hand { owner: PlayerId(1) })
    );
    {
        let selected = game.next_test_decision().land(CardId(5));
        reject(&mut game, PlayerId(1), selected)
    };
    until(&mut game, 2, TurnStep::PrecombatMain);
    pass(&mut game);
    pass(&mut game);
    pass(&mut game);
    assert_eq!(game.state.rules.priority, PlayerId(0));
    {
        let selected = game.next_test_decision().cast(CardId(2));
        reject(&mut game, PlayerId(0), selected)
    };
    {
        let selected = game.next_test_decision().land(CardId(1));
        reject(&mut game, PlayerId(0), selected)
    };

    let mut game = Game::new(FixtureCatalog::Base.admit(ScenarioConfig::turns_permanents()));
    // Pay with private pre-floated mana to leave the land allowance unused.
    // No synthetic setup is admitted by production admission.
    game.state.rules.players[0]
        .mana
        .insert(Color::G, Quantity::from(2));
    let request = game.decision_for(PlayerId(0)).unwrap().unwrap();
    game.respond(
        PlayerId(0),
        request.id,
        game.next_test_decision().cast(CardId(2)),
    )
    .unwrap();
    let Advance::AwaitingDecision(request) = game.advance(Budget::unlimited()) else {
        panic!()
    };
    game.respond(
        PlayerId(0),
        request.id,
        Answer::Pay {
            pay: PayAnswer::Finish {
                undo_on_failure: true,
            },
        },
    )
    .unwrap();
    let Advance::AwaitingDecision(request) = game.advance(Budget::unlimited()) else {
        panic!()
    };
    let before = game.state.clone();
    assert!(
        game.respond(
            PlayerId(0),
            request.id,
            game.next_test_decision().land(CardId(1))
        )
        .is_err()
    );
    assert_eq!(game.state, before);
}

#[test]
fn land_limit_spans_both_mains_and_special_action_resets_passes() {
    let mut game = Game::new(FixtureCatalog::Base.admit(ScenarioConfig::turns_permanents()));
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
        let mut game = Game::new(FixtureCatalog::Base.admit(ScenarioConfig::turns_permanents()));
        {
            let selected_answer = game.next_test_decision().cast(CardId(2));
            input(&mut game, selected_answer)
        };
        assert!(matches!(
            Driver::settle(&mut game, u64::MAX),
            Advance::AwaitingDecision(Decision {
                reason: Reason::Mana { .. },
                ..
            })
        ));
        {
            let selected_answer = game.next_test_decision().activate_mana(ObjectId {
                card: CardId(0),
                incarnation: 0,
            });
            input(&mut game, selected_answer)
        };
        {
            let selected_answer = Answer::Pay {
                pay: PayAnswer::Finish {
                    undo_on_failure: undo_mana,
                },
            };
            input(&mut game, selected_answer)
        };
        assert!(matches!(
            Driver::settle(&mut game, u64::MAX),
            Advance::AwaitingDecision(Decision {
                reason: Reason::Priority,
                ..
            })
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
        let replay = Game::replay(
            FixtureCatalog::Base.admit(ScenarioConfig::turns_permanents()),
            &game.replay_journal(),
        )
        .unwrap();
        assert_eq!(replay.state, game.state);
    }
}

#[test]
fn mana_empties_at_step_end_and_untap_and_draw_offer_no_early_priority() {
    let mut game = Game::new(FixtureCatalog::Base.admit(ScenarioConfig::turns_permanents()));
    {
        let selected_answer = game.next_test_decision().activate_mana(ObjectId {
            card: CardId(0),
            incarnation: 0,
        });
        input(&mut game, selected_answer)
    };
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
    for land in [true, false] {
        for departed in [PlayerId(3), PlayerId(0)] {
            let mut game =
                Game::new(FixtureCatalog::Base.admit(ScenarioConfig::turns_permanents()));
            let d = game.next_test_decision();
            input(
                &mut game,
                if land {
                    d.land(CardId(1))
                } else {
                    d.cast(CardId(2))
                },
            );
            game.concede(departed).unwrap();

            assert!(game.decision_for(PlayerId(0)).unwrap().is_none());
            let result = Driver::settle(&mut game, u64::MAX);
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
            } else if land {
                assert_eq!(
                    game.zone(game.object_id(CardId(1)).unwrap()),
                    Some(Zone::Battlefield)
                );
                assert_eq!(game.state.rules.turn.lands_played, 1);
            } else {
                assert!(matches!(
                    result,
                    Advance::AwaitingDecision(Decision {
                        reason: Reason::Mana { .. },
                        ..
                    })
                ));
            }
            let replay = Game::replay(
                FixtureCatalog::Base.admit(ScenarioConfig::turns_permanents()),
                &game.replay_journal(),
            )
            .unwrap();
            assert_eq!(replay.state, game.state);
        }
    }
}

#[test]
fn departing_last_unpassed_player_resolves_without_a_redundant_pass() {
    let mut game = Game::new(FixtureCatalog::Base.admit(ScenarioConfig::turns_permanents()));
    play(&mut game, 1);
    cast_bears(&mut game);
    for _ in 0..3 {
        pass(&mut game);
    }
    assert_eq!(game.state.rules.priority, PlayerId(3));
    game.concede(PlayerId(3)).unwrap();
    let _ = Driver::settle(&mut game, u64::MAX);
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
    let mut game = Game::new(FixtureCatalog::Base.admit(ScenarioConfig::turns_permanents()));
    game.concede(PlayerId(0)).unwrap();
    let _ = Driver::settle(&mut game, u64::MAX);
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
    let replay = Game::replay(
        FixtureCatalog::Base.admit(ScenarioConfig::turns_permanents()),
        &game.replay_journal(),
    )
    .unwrap();
    assert_eq!(replay.state, game.state);

    let mut game = Game::new(FixtureCatalog::Base.admit(ScenarioConfig::turns_permanents()));
    for _ in 0..3 {
        pass(&mut game);
    }
    {
        let selected_answer = Answer::Pick {
            picks: vec![vec![0]],
        };
        input(&mut game, selected_answer)
    };
    for player in [PlayerId(1), PlayerId(2), PlayerId(3)] {
        game.concede(player).unwrap();
    }
    assert_eq!(
        Driver::settle(&mut game, u64::MAX),
        Advance::Finished {
            winner: PlayerId(0)
        }
    );
    assert_eq!(game.state.rules.turn.number, 1);
    assert_eq!(game.state.rules.turn.step, TurnStep::PostcombatMain);
    assert_eq!(
        Game::replay(
            FixtureCatalog::Base.admit(ScenarioConfig::turns_permanents()),
            &game.replay_journal()
        )
        .unwrap()
        .state,
        game.state
    );
}

#[test]
fn public_land_queries_recover_and_handles_tokens_and_receipts_expire() {
    let mut game = Game::new(FixtureCatalog::Base.admit(ScenarioConfig::turns_permanents()));
    let own = observe(&game, PlayerId(0));
    assert_eq!(
        own.observation_version,
        mtg_core::testkit::formats::OBSERVATION_VERSION
    );
    let token = own.decision.unwrap().id.clone();
    let hand = game
        .handle_for(PlayerId(0), game.object_id(CardId(1)).unwrap())
        .unwrap();
    let QueryResult::Incomplete { work, .. } = game.check_player_response(
        PlayerId(0),
        token.clone(),
        game.next_test_viewer_decision().select(Candidate::Action {
            action: Action::PlayLand {
                object: hand.clone(),
            },
        }),
        Budget::new(0),
    ) else {
        panic!()
    };
    let saved = game.save_work(&work).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&saved).unwrap()["work_version"],
        mtg_core::testkit::formats::WORK_VERSION
    );
    let restored = Game::restore(
        FixtureCatalog::Base.admit(ScenarioConfig::turns_permanents()),
        &game.snapshot().unwrap(),
    )
    .unwrap();
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
            game.decision_for(PlayerId(0)).unwrap().unwrap().id,
            game.next_test_decision().land(CardId(1)),
        )
        .unwrap();
    let mut fork = game.fork();
    assert!(fork.submit(foreign).is_err());
    game.submit(receipt).unwrap();
    assert!(matches!(
        game.resume_work(stale, Budget::unlimited()),
        QueryResult::Rejected(QueryRejection::StaleOrForeignWork)
    ));
    let _ = Driver::settle(&mut game, u64::MAX);
    assert!(matches!(
        game.check_player_response(
            PlayerId(0),
            token,
            fork.next_test_viewer_decision().select(Candidate::Action {
                action: Action::PlayLand {
                    object: hand.clone()
                }
            }),
            Budget::unlimited()
        ),
        QueryResult::Rejected(QueryRejection::InvalidResponse)
    ));
    let own = observe(&game, PlayerId(0));
    let token = own.decision.unwrap().id.clone();
    assert!(matches!(
        game.check_player_response(
            PlayerId(0),
            token,
            fork.next_test_viewer_decision().activate_mana(hand),
            Budget::unlimited()
        ),
        QueryResult::Rejected(QueryRejection::InvalidResponse)
    ));
    assert_ne!(game.state, fork.state);
    assert_eq!(fork.object_id(CardId(1)).unwrap().incarnation, 0);
}

#[test]
fn draw_visibility_queries_and_search_are_pure_and_do_not_reveal_libraries_or_other_hands() {
    let mut game = Game::new(FixtureCatalog::Base.admit(ScenarioConfig::turns_permanents()));
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
    let mut game = Game::new(FixtureCatalog::Base.admit(ScenarioConfig::turns_permanents()));
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
        assert!(
            Game::restore(
                FixtureCatalog::Base.admit(ScenarioConfig::turns_permanents()),
                &canonical(&saved).unwrap()
            )
            .is_err()
        );
    }
}

#[test]
fn creature_combat_and_empty_library_are_explicit_recoverable_faults() {
    let mut game = Game::new(FixtureCatalog::Base.admit(ScenarioConfig::turns_permanents()));
    play(&mut game, 1);
    cast_bears(&mut game);
    resolve_bears(&mut game);
    until(&mut game, 5, TurnStep::BeginningCombat);
    for _ in 0..4 {
        pass(&mut game);
    }
    assert!(matches!(
        Driver::settle(&mut game, u64::MAX),
        Advance::AwaitingDecision(Decision {
            reason: Reason::Attackers,
            ..
        })
    ));

    let mut game = Game::new(FixtureCatalog::Base.admit(ScenarioConfig::turns_permanents()));
    until(&mut game, 6, TurnStep::Upkeep);
    for _ in 0..4 {
        pass(&mut game);
    }
    assert!(matches!(
        Driver::settle(&mut game, u64::MAX),
        Advance::AwaitingDecision(_)
    ));
    assert_eq!(game.state.rules.players[1].status, PlayerStatus::Left);
}

#[test]
fn admission_does_not_accept_variants_or_enable_new_actions_in_old_profiles() {
    let catalog = FixtureCatalog::Base
        .admit(ScenarioConfig::turns_permanents())
        .catalog
        .as_ref()
        .clone();
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
    {
        let selected = old.next_test_decision().land(CardId(1));
        reject(&mut old, PlayerId(0), selected)
    };
    {
        let selected = old.next_test_decision().cast(CardId(2));
        reject(&mut old, PlayerId(0), selected)
    };
}

fn scripted_response(game: &Game, request: &Decision<Rules>) -> Answer {
    match request {
        Decision {
            reason: Reason::Attackers,
            ..
        } => return request.attack(vec![]),
        Decision {
            reason: Reason::Blockers,
            ..
        } => return request.block(vec![]),
        Decision {
            player,
            shape:
                Shape::Pay {
                    cost,
                    pool:
                        ManaPool {
                            unrestricted: available,
                        },
                    ..
                },
            reason: Reason::SpendMana { .. },
            ..
        } => {
            let remaining = game.payment_pool(*player, cost).unwrap();
            return Answer::Pay {
                pay: PayAnswer::Spend {
                    unrestricted: available
                        .iter()
                        .filter_map(|(color, amount)| {
                            let mut n = amount.clone();
                            n.subtract(&remaining.get(color).cloned().unwrap_or_default());
                            (n > Quantity::zero()).then_some((*color, n))
                        })
                        .collect(),
                },
            };
        }
        _ => (),
    }

    if let Decision {
        reason: Reason::Mana { .. },
        ..
    } = request
    {
        for card in [CardId(0), CardId(1)] {
            let object = &game.state.rules.objects[&card];
            if !object.tapped {
                return request.activate_mana(object.id);
            }
        }
        return Answer::Pay {
            pay: PayAnswer::Finish {
                undo_on_failure: true,
            },
        };
    }
    if game.state.rules.turn.number == 1 && request.player == PlayerId(0) {
        if matches!(game.state.rules.objects[&CardId(1)].zone, Zone::Hand { .. }) {
            return request.land(CardId(1));
        }
        if matches!(game.state.rules.objects[&CardId(2)].zone, Zone::Hand { .. }) {
            return request.cast(CardId(2));
        }
    }
    if game.state.rules.turn.number == 5
        && game.state.rules.turn.step == TurnStep::PrecombatMain
        && request.player == PlayerId(0)
    {
        return request.land(CardId(3));
    }
    Answer::Pick {
        picks: vec![vec![0]],
    }
}

fn run_script(mode: TraceMode, budget: u64) -> Game {
    let mut game = Game::with_trace(
        FixtureCatalog::Base.admit(ScenarioConfig::turns_permanents()),
        mode,
    );
    for _ in 0..6000 {
        let outcome = game.advance(Budget::new(budget));
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
                game.respond(request.player, request.id, response).unwrap();
            }
            other => panic!("script failed: {other:?}"),
        }
    }
    panic!("script exceeded bounded work")
}

#[test]
fn turn_script_reaches_fifth_precombat_main_after_a_land_play() {
    let game = run_script(TraceMode::Off, u64::MAX);
    assert_eq!(game.turn().number, 5);
    assert_eq!(game.turn().lands_played, 1);
}

#[test]
fn concessions_at_each_casting_and_scheduler_stage_remain_recoverable() {
    let mut game = Game::new(FixtureCatalog::Base.admit(ScenarioConfig::turns_permanents()));
    let mut samples = BTreeMap::new();
    for _ in 0..6000 {
        // Distinct pending/automatic stages for each step; one representative
        // seat rotation suffices here, with all four possible departing seats.
        let frame = serde_json::to_value(&game.state.rules.frame).unwrap();
        let key = format!(
            "{:?}/{}/{:?}",
            game.state.rules.turn.step,
            frame["stage"],
            game.state.rules.pending.as_ref().map(|p| &p.answer)
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
                game.respond(request.player, request.id, response).unwrap();
            }
            other => panic!("unexpected sample outcome: {other:?}"),
        }
    }
    assert!(samples.len() >= 20, "missing scheduler/casting boundaries");
    for (boundary, snapshot) in samples {
        for player in 0..4 {
            let mut branch = Game::restore(
                FixtureCatalog::Base.admit(ScenarioConfig::turns_permanents()),
                &snapshot,
            )
            .unwrap();
            branch.concede(PlayerId(player)).unwrap();
            let result = Driver::settle(&mut branch, u64::MAX);
            assert!(
                matches!(
                    result,
                    Advance::AwaitingDecision(_) | Advance::Finished { .. }
                ),
                "{boundary}: {result:?}"
            );
            let replay = Game::replay(
                FixtureCatalog::Base.admit(ScenarioConfig::turns_permanents()),
                &branch.replay_journal(),
            )
            .unwrap();
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
    let mut game = Game::new(FixtureCatalog::Base.admit(ScenarioConfig::turns_permanents()));
    {
        let selected_answer = game.next_test_decision().land(CardId(1));
        input(&mut game, selected_answer)
    };
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
    let _ = Driver::settle(&mut fork, u64::MAX);
    let _ = Driver::settle(&mut game, u64::MAX);
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
        let mut p = FixtureCatalog::Base.admit(ScenarioConfig::turns_permanents());
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
        {
            let selected_answer = Answer::Pick {
                picks: vec![vec![0]],
            };
            input(&mut game, selected_answer)
        };
        let outcome = Driver::settle(&mut game, u64::MAX);
        if cleanup {
            assert!(matches!(
                outcome,
                Advance::AwaitingDecision(d @ Decision{reason:Reason::Discard,..}) if d.required_count()==1
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
fn flash_end_step_draw() {
    let controller = PlayerId(0);
    let opponent = PlayerId(1);
    let mut fixture = FixtureCatalog::Base.admit(ScenarioConfig::turns_permanents());
    let catalog = Arc::make_mut(&mut fixture.catalog);
    // Private board fixture. Compilation and source fidelity have their own witness.
    let mut source = catalog.definition_by_oracle_id(BEARS).unwrap().clone();
    source.oracle_id = "test-flash-end-step-draw".into();
    source.name = source.oracle_id.clone();
    source.source_labels = None;
    source.characteristics.legendary = true;
    source.keywords = vec![KeywordAbility::Flash];
    source.spell.as_mut().unwrap().timing = SpellTiming::Instant;
    source.triggered = vec![TriggeredAbility {
        functioning_zone: FunctioningZone::Battlefield,
        condition: TriggerCondition::on(mtg_contracts::kir::EventPattern::StepBegins {
            step: BeginningStep::End,
            active: mtg_contracts::kir::PlayerExpr::you(mtg_contracts::kir::Context::Triggered),
        }),
        program: SpellProgram {
            additional_sacrifice: None,
            optional_additional_costs: vec![],
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            functioning_zone: FunctioningZone::Stack,
            targets: vec![],
            instructions: vec![Instruction::draw(
                PlayerExpr::you(Context::Triggered),
                7.into(),
                1,
            )],
        },
    }];
    source.capabilities = source.requirements();
    fixture.config.cards[0].oracle_id = source.oracle_id.clone();
    catalog.definitions.push(source);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
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
        let Advance::AwaitingDecision(
            request @ Decision {
                reason: Reason::Priority,
                ..
            },
        ) = game.advance(Budget::new(100))
        else {
            panic!("expected priority");
        };
        game.respond(
            request.player,
            request.id,
            Answer::Pick {
                picks: vec![vec![0]],
            },
        )
        .unwrap();
    };
    assert_eq!(game.state.rules.turn.step, TurnStep::PostcombatMain);
    for _ in 0..4 {
        pass_priority(&mut game);
    }
    assert!(matches!(
        game.advance(Budget::new(100)),
        Advance::AwaitingDecision(Decision{player,reason:Reason::Priority,..}) if player == controller
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
            assert_eq!(progress.completed, Quantity::from(completed as i64));
            assert_eq!(progress.remaining, Quantity::from(7 - completed as i64));
        }
    }
    for player in [opponent, PlayerId(2), PlayerId(3)] {
        assert_eq!(game.state.rules.libraries[&player], libraries[&player]);
        assert!(game.action_hand(player).is_empty());
    }
    // Play through the opponent's whole turn, including its end step: the
    // controller-only end-step trigger must not fire there (CR 603.2).
    for _ in 0..256 {
        if game.state.rules.turn.number > 2 {
            break;
        }
        let Advance::AwaitingDecision(request) = game.advance(Budget::new(100)) else {
            continue;
        };
        let response = match &request {
            Decision {
                reason: Reason::Priority,
                ..
            } => Answer::Pick {
                picks: vec![vec![0]],
            },
            Decision {
                reason: Reason::Attackers,
                ..
            } => request.attack(vec![]),
            Decision {
                reason: Reason::Blockers,
                ..
            } => request.block(vec![]),
            other => panic!("unexpected turn decision {other:?}"),
        };
        game.respond(request.player, request.id, response).unwrap();
    }
    assert!(
        game.state.rules.turn.number > 2,
        "opponent turn did not end"
    );
    assert_eq!(game.state.rules.next_trigger_id, 1);
    assert_eq!(game.action_hand(controller).len(), 7);
}

#[test]
fn land_movement_and_counting_batch_failure_is_atomic() {
    let mut game = Game::new(FixtureCatalog::Base.admit(ScenarioConfig::turns_permanents()));
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
