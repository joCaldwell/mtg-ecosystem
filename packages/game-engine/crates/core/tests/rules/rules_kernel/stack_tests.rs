use super::*;
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
fn input(game: &mut Game, player: u8, response: Answer) {
    let finish = matches!(
        response,
        Answer::Pay {
            pay: PayAnswer::Finish { .. }
        }
    );
    let Advance::AwaitingDecision(r) = Driver::settle(game, u64::MAX) else {
        panic!("missing decision")
    };
    assert_eq!(r.player, PlayerId(player));
    game.respond(r.player, r.id, response).unwrap();
    if finish {
        game.complete_test_mana();
    }
}
fn cast(game: &mut Game, player: u8, pay: bool) {
    if pay {
        input(
            game,
            player,
            game.next_test_decision()
                .activate_mana(game.object_id(land(player)).unwrap()),
        );
    }
    input(game, player, game.next_test_decision().cast(spell(player)));
    input(
        game,
        player,
        game.next_test_decision().targets(vec![ObjectId {
            card: CardId(2),
            incarnation: 0,
        }]),
    );
    input(
        game,
        player,
        Answer::Pay {
            pay: PayAnswer::Finish {
                undo_on_failure: false,
            },
        },
    );
    assert!(
        matches!(Driver::settle(game, u64::MAX),Advance::AwaitingDecision(Decision{player:p,reason:Reason::Priority,..}) if p==PlayerId(player))
    );
}
fn pass_round(game: &mut Game, start: u8) {
    for offset in 0..4 {
        let player = (start + offset) % 4;
        if game.is_live(PlayerId(player)) {
            input(
                game,
                player,
                Answer::Pick {
                    picks: vec![vec![0]],
                },
            );
        }
    }
    assert!(matches!(
        Driver::settle(game, u64::MAX),
        Advance::AwaitingDecision(_)
    ));
}
fn observe(game: &Game, viewer: u8) -> Observation {
    let QueryResult::Complete(QueryValue::Observation(o)) =
        game.observe(PlayerId(viewer), Budget::unlimited())
    else {
        panic!()
    };
    o
}
#[test]
fn each_opponent_can_respond_and_each_top_requires_its_own_pass_round() {
    for responder in 1..4 {
        let mut game = Game::new(FixtureCatalog::Base.admit(ScenarioConfig::stack_response()));
        cast(&mut game, 0, true);
        for player in 0..responder {
            {
                let selected_answer = Answer::Pick {
                    picks: vec![vec![0]],
                };
                input(&mut game, player, selected_answer)
            };
        }
        cast(&mut game, responder, true);
        assert_eq!(game.state.rules.stack.len(), 2);
        assert!(game.state.rules.passes.is_empty());
        let before = observe(&game, 0);
        assert_eq!(
            before
                .view
                .stack
                .iter()
                .map(|s| s.controller)
                .collect::<Vec<_>>(),
            vec![PlayerId(0), PlayerId(responder)]
        );
        assert!(
            before
                .view
                .stack
                .iter()
                .all(|s| s.cast_complete && s.targets[0].current.is_some())
        );
        pass_round(&mut game, responder);
        assert_eq!(game.state.rules.stack.len(), 1);
        assert_eq!(game.state.rules.priority, PlayerId(0));
        assert!(game.state.rules.passes.is_empty());
        assert_eq!(
            game.zone(game.object_id(CardId(2)).unwrap()),
            Some(Zone::Hand { owner: PlayerId(1) })
        );
        let after = observe(&game, 0);
        assert_eq!(
            after.view.stack[0].targets[0].announced,
            before.view.stack[0].targets[0].announced
        );
        assert!(after.view.stack[0].targets[0].current.is_none());
        assert!(
            game.resolve_handle(PlayerId(0), &after.view.stack[0].targets[0].announced)
                .is_err()
        );
        assert!(!after.view.objects.iter().any(|o| o.oracle_id == BEARS));
        assert!(
            observe(&game, 1)
                .view
                .objects
                .iter()
                .any(|o| o.oracle_id == BEARS)
        );
        pass_round(&mut game, 0);
        assert!(game.state.rules.stack.is_empty());
        let completed = game
            .events()
            .iter()
            .filter_map(|e| match e {
                Event::SpellCompleted { stack_id, resolved } => Some((*stack_id, *resolved)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(completed, vec![(StackId(1), true), (StackId(0), false)]);
        let replay = Game::replay(
            FixtureCatalog::Base.admit(ScenarioConfig::stack_response()),
            &game.replay_journal(),
        )
        .unwrap();
        assert_eq!(replay.state, game.state);
        assert_eq!(replay.trace, game.trace);
    }
}
#[test]
fn four_entry_chain_is_lifo_and_recovery_preserves_all_lower_spells() {
    let mut full = Game::new(FixtureCatalog::Base.admit(ScenarioConfig::stack_response()));
    for player in 0..4 {
        if player > 0 {
            input(
                &mut full,
                player - 1,
                Answer::Pick {
                    picks: vec![vec![0]],
                },
            );
        }
        cast(&mut full, player, true);
    }
    assert_eq!(full.state.rules.stack.len(), 4);
    assert_eq!(
        observe(&full, 2)
            .view
            .stack
            .iter()
            .map(|s| s.controller)
            .collect::<Vec<_>>(),
        vec![PlayerId(0), PlayerId(1), PlayerId(2), PlayerId(3)]
    );
    pass_round(&mut full, 3);
    for _ in 0..3 {
        pass_round(&mut full, 0);
    }
    assert!(full.state.rules.stack.is_empty());
    let complete = full
        .events()
        .iter()
        .filter_map(|e| match e {
            Event::SpellCompleted { stack_id, resolved } => Some((*stack_id, *resolved)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        complete,
        vec![
            (StackId(3), true),
            (StackId(2), false),
            (StackId(1), false),
            (StackId(0), false)
        ]
    );
}
#[test]
fn unpaid_response_reversal_restores_prior_passes_and_keeps_lower_spell() {
    let mut game = Game::new(FixtureCatalog::Base.admit(ScenarioConfig::stack_response()));
    cast(&mut game, 0, true);
    {
        let selected_answer = Answer::Pick {
            picks: vec![vec![0]],
        };
        input(&mut game, 0, selected_answer)
    };
    let _ = Driver::settle(&mut game, u64::MAX);
    assert_eq!(game.state.rules.passes, vec![PlayerId(0)]);
    cast(&mut game, 1, false);
    assert_eq!(game.state.rules.passes, vec![PlayerId(0)]);
    assert_eq!(game.state.rules.stack.len(), 1);
    assert_eq!(game.object_id(spell(1)).unwrap().incarnation, 0);
    for player in 1..4 {
        {
            let selected_answer = Answer::Pick {
                picks: vec![vec![0]],
            };
            input(&mut game, player, selected_answer)
        };
    }
    let _ = Driver::settle(&mut game, u64::MAX);
    assert!(game.state.rules.stack.is_empty());
    assert!(game.events().iter().any(|e| matches!(
        e,
        Event::SpellCompleted {
            stack_id: StackId(0),
            resolved: true
        }
    )));
}
#[test]
fn lower_source_departure_preserves_top_announcement_payment_and_resolution() {
    for boundary in 0..3 {
        let mut game = Game::new(FixtureCatalog::Base.admit(ScenarioConfig::stack_response()));
        cast(&mut game, 0, true);
        {
            let selected_answer = Answer::Pick {
                picks: vec![vec![0]],
            };
            input(&mut game, 0, selected_answer)
        };
        {
            let selected_answer = game.next_test_decision().activate_mana(ObjectId {
                card: land(1),
                incarnation: 0,
            });
            input(&mut game, 1, selected_answer)
        };
        {
            let selected_answer = game.next_test_decision().cast(spell(1));
            input(&mut game, 1, selected_answer)
        };
        let _ = Driver::settle(&mut game, u64::MAX);
        if boundary > 0 {
            {
                let selected_answer = game.next_test_decision().targets(vec![ObjectId {
                    card: CardId(2),
                    incarnation: 0,
                }]);
                input(&mut game, 1, selected_answer)
            };
            let _ = game.advance(Budget::new(1));

            if boundary == 1 {
                let Advance::AwaitingDecision(r) = Driver::settle(&mut game, u64::MAX) else {
                    panic!("mana request")
                };
                game.respond(
                    r.player,
                    r.id,
                    Answer::Pay {
                        pay: PayAnswer::Finish {
                            undo_on_failure: false,
                        },
                    },
                )
                .unwrap();
            } else {
                {
                    let selected_answer = Answer::Pay {
                        pay: PayAnswer::Finish {
                            undo_on_failure: false,
                        },
                    };
                    input(&mut game, 1, selected_answer)
                };
            }
            let _ = game.advance(Budget::new(1));
        }
        if boundary == 2 {
            let _ = Driver::settle(&mut game, u64::MAX);
            for player in [1, 2, 3, 0] {
                {
                    let selected_answer = Answer::Pick {
                        picks: vec![vec![0]],
                    };
                    input(&mut game, player, selected_answer)
                };
            }
            let _ = game.advance(Budget::new(1));
        }
        game.concede(PlayerId(0)).unwrap();

        let _ = game.advance(Budget::new(1));

        assert_eq!(game.state.rules.stack.len(), 1);
        assert_eq!(game.state.rules.stack[0].id, StackId(1));
        match boundary {
            0 => assert!(matches!(game.state.rules.frame, Frame::Targets { .. })),
            1 => assert!(matches!(game.state.rules.frame, Frame::SpendMana { .. })),
            _ => assert!(matches!(game.state.rules.frame, Frame::Resolve { .. })),
        }
    }
}
#[test]
fn reversing_after_departure_filters_saved_passes_and_removes_only_its_source() {
    let mut game = Game::new(FixtureCatalog::Base.admit(ScenarioConfig::stack_response()));
    cast(&mut game, 0, true);
    {
        let selected_answer = Answer::Pick {
            picks: vec![vec![0]],
        };
        input(&mut game, 0, selected_answer)
    };
    {
        let selected_answer = game.next_test_decision().cast(spell(1));
        input(&mut game, 1, selected_answer)
    };
    {
        let selected_answer = game.next_test_decision().targets(vec![ObjectId {
            card: CardId(2),
            incarnation: 0,
        }]);
        input(&mut game, 1, selected_answer)
    };
    let _ = game.advance(Budget::new(1));
    game.concede(PlayerId(0)).unwrap();
    let _ = game.advance(Budget::new(1));

    {
        let selected_answer = Answer::Pay {
            pay: PayAnswer::Finish {
                undo_on_failure: false,
            },
        };
        input(&mut game, 1, selected_answer)
    };
    let _ = Driver::settle(&mut game, u64::MAX);
    assert!(game.state.rules.passes.is_empty());
    assert!(game.state.rules.stack.is_empty());
    assert_eq!(game.state.rules.priority, PlayerId(1));
    assert_eq!(game.object_id(spell(1)).unwrap().incarnation, 0);
    let mut game = Game::new(FixtureCatalog::Base.admit(ScenarioConfig::stack_response()));
    cast(&mut game, 0, true);
    {
        let selected_answer = Answer::Pick {
            picks: vec![vec![0]],
        };
        input(&mut game, 0, selected_answer)
    };
    {
        let selected_answer = game.next_test_decision().cast(spell(1));
        input(&mut game, 1, selected_answer)
    };
    let _ = Driver::settle(&mut game, u64::MAX);
    game.concede(PlayerId(1)).unwrap();
    let _ = game.advance(Budget::new(1));

    // P1 owns the target too, so the lower spell remains cast but will not resolve.
    assert_eq!(game.state.rules.stack.len(), 1);
    assert_eq!(game.state.rules.stack[0].id, StackId(0));
    assert_eq!(game.state.rules.passes, vec![PlayerId(0)]);
}
#[test]
fn detached_top_resolution_continues_and_lower_spell_waits_for_new_passes() {
    let mut game = Game::new(FixtureCatalog::Base.admit(ScenarioConfig::stack_response()));
    cast(&mut game, 0, true);
    for player in [0, 1, 2] {
        {
            let selected_answer = Answer::Pick {
                picks: vec![vec![0]],
            };
            input(&mut game, player, selected_answer)
        };
    }
    cast(&mut game, 3, true);
    for player in [3, 0, 1, 2] {
        {
            let selected_answer = Answer::Pick {
                picks: vec![vec![0]],
            };
            input(&mut game, player, selected_answer)
        };
    }
    let _ = game.advance(Budget::new(1));

    game.concede(PlayerId(3)).unwrap();
    let _ = game.advance(Budget::new(1));

    assert!(matches!(game.state.rules.frame, Frame::Resolve { .. }));
    assert_eq!(game.state.rules.stack.len(), 1);
    assert!(game.object_id(spell(3)).is_none());
    let _ = Driver::settle(&mut game, u64::MAX);
    assert_eq!(game.state.rules.stack.len(), 1);
    assert_eq!(game.state.rules.priority, PlayerId(0));
    assert!(game.state.rules.passes.is_empty());
    assert_eq!(
        game.zone(game.object_id(CardId(2)).unwrap()),
        Some(Zone::Hand { owner: PlayerId(1) })
    );
    pass_round(&mut game, 0);
    assert!(game.state.rules.stack.is_empty());
    assert!(game.events().iter().any(|e| matches!(
        e,
        Event::SpellCompleted {
            stack_id: StackId(1),
            resolved: true
        }
    )));
}
#[test]
fn timing_ownership_and_rehashed_stack_metadata_reject_without_changing_state() {
    let mut game = Game::new(FixtureCatalog::Base.admit(ScenarioConfig::stack_response()));
    cast(&mut game, 0, true);
    let before = game.snapshot().unwrap();
    let request = game.decision_for(PlayerId(0)).unwrap().unwrap();
    assert!(
        game.respond(
            PlayerId(0),
            request.id,
            game.next_test_decision().cast(spell(2))
        )
        .is_err()
    );
    assert_eq!(before, game.snapshot().unwrap());
    {
        let selected_answer = Answer::Pick {
            picks: vec![vec![0]],
        };
        input(&mut game, 0, selected_answer)
    };
    {
        let selected_answer = game.next_test_decision().cast(spell(1));
        input(&mut game, 1, selected_answer)
    };
    let _ = Driver::settle(&mut game, u64::MAX);
    let before = game.snapshot().unwrap();
    let request = game.decision_for(PlayerId(1)).unwrap().unwrap();
    assert!(
        game.respond(
            PlayerId(1),
            request.id,
            Answer::Pick {
                picks: vec![vec![u32::MAX]]
            }
        )
        .is_err()
    );
    assert_eq!(before, game.snapshot().unwrap());
    for mutation in 0..5 {
        let mut s: Snapshot = strict_read(&before).unwrap();
        match mutation {
            0 => s.state.rules.stack.swap(0, 1),
            1 => s.state.rules.stack[0].paid = None,
            2 => s.state.rules.stack[1].id = s.state.rules.stack[0].id,
            3 => s.state.rules.stack[0].announced_targets[0][0] = "forged".into(),
            _ => {
                if let Frame::Targets { cast } = &mut s.state.rules.frame {
                    cast.prior_passes.push(PlayerId(3));
                }
            }
        }
        s.state_sha256 = digest(&s.state).unwrap();
        assert!(
            Game::restore(
                FixtureCatalog::Base.admit(ScenarioConfig::stack_response()),
                &canonical(&s).unwrap()
            )
            .is_err()
        );
    }
}
#[test]
fn empty_stack_pass_round_advances_without_ending_the_game() {
    let mode = TraceMode::Detailed;
    let mut game = Game::with_trace(
        FixtureCatalog::Base.admit(ScenarioConfig::stack_response()),
        mode,
    );
    for player in 0..4 {
        {
            let selected_answer = Answer::Pick {
                picks: vec![vec![0]],
            };
            input(&mut game, player, selected_answer)
        };
    }
    assert!(matches!(
        game.advance(Budget::unlimited()),
        Advance::AwaitingDecision(_)
    ));

    assert_eq!(game.live_players().len(), 4);
    assert!(!matches!(game.state.rules.frame, Frame::Finished { .. }));
}
#[test]
fn stack_query_rows_suspend_and_sidecars_preserve_order_without_hidden_targets() {
    let mut game = Game::new(FixtureCatalog::Base.admit(ScenarioConfig::stack_response()));
    cast(&mut game, 0, true);
    {
        let selected_answer = Answer::Pick {
            picks: vec![vec![0]],
        };
        input(&mut game, 0, selected_answer)
    };
    cast(&mut game, 1, true);
    let reference = observe(&game, 2);
    let before = game.state_digest().unwrap();
    let QueryResult::Incomplete { work, .. } = game.observe(PlayerId(2), Budget::new(13)) else {
        panic!("stack rows need their own work")
    };
    let saved = game.save_work(&work).unwrap();
    let restored = Game::restore(
        FixtureCatalog::Base.admit(ScenarioConfig::stack_response()),
        &game.snapshot().unwrap(),
    )
    .unwrap();
    let resumed = restored.restore_work(&saved).unwrap();
    let QueryResult::Complete(QueryValue::Observation(actual)) =
        restored.resume_work(resumed, Budget::unlimited())
    else {
        panic!()
    };
    assert_eq!(actual, reference);
    assert_eq!(before, game.state_digest().unwrap());
    assert!(
        reference
            .view
            .objects
            .iter()
            .filter(|o| matches!(o.zone, Zone::Hand { .. }))
            .all(|o| o.owner == PlayerId(2))
    );
}

#[test]
fn each_lower_entry_is_validated_against_its_own_program() {
    // Private interpreter fixture, not an admitted real card or compiler bypass.
    let mut p = FixtureCatalog::Base.admit(ScenarioConfig::stack_response());
    let mut catalog = p.catalog.as_ref().clone();
    let mut second = catalog
        .definitions
        .iter()
        .find(|d| d.oracle_id == SPELL)
        .unwrap()
        .clone();
    second.oracle_id = "synthetic-two-target-program".into();
    second.name = "Synthetic interpreter fixture".into();
    let program = second.spell.as_mut().unwrap();
    let mut target = program.targets[0].clone();
    target.slot = 1;
    target.selection_id = "synthetic:second".into();
    program.targets.push(target);
    program.instructions.push(Instruction::move_target(
        1,
        mtg_contracts::kir::Destination::OwnerHand,
        1,
    ));
    p.config
        .cards
        .iter_mut()
        .find(|c| c.id == spell(1))
        .unwrap()
        .oracle_id = second.oracle_id.clone();
    catalog.definitions.push(second);
    p.catalog = Arc::new(catalog);
    let mut game = Game::new(p);
    cast(&mut game, 0, true);
    {
        let selected_answer = Answer::Pick {
            picks: vec![vec![0]],
        };
        input(&mut game, 0, selected_answer)
    };
    {
        let selected_answer = game.next_test_decision().activate_mana(ObjectId {
            card: land(1),
            incarnation: 0,
        });
        input(&mut game, 1, selected_answer)
    };
    {
        let selected_answer = game.next_test_decision().cast(spell(1));
        input(&mut game, 1, selected_answer)
    };
    {
        let selected_answer = game.next_test_decision().targets(vec![
            ObjectId {
                card: CardId(2),
                incarnation: 0
            };
            2
        ]);
        input(&mut game, 1, selected_answer)
    };
    {
        let selected_answer = Answer::Pay {
            pay: PayAnswer::Finish {
                undo_on_failure: false,
            },
        };
        input(&mut game, 1, selected_answer)
    };
    let _ = Driver::settle(&mut game, u64::MAX);
    assert_eq!(game.state.rules.stack[0].targets.len(), 1);
    assert_eq!(game.state.rules.stack[1].targets.len(), 2);

    pass_round(&mut game, 1);
    assert_eq!(game.state.rules.stack.len(), 1);
    pass_round(&mut game, 0);
    assert!(game.state.rules.stack.is_empty());
}
