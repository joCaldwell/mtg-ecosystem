use super::*;

fn game() -> Game {
    Game::new(
        Prepared::admit(
            Catalog::read(include_str!("../../../../tests/fixtures/expected-ir.json")).unwrap(),
            ScenarioConfig::combat_interactions(),
        )
        .unwrap(),
    )
}
fn object(game: &Game, card: u32) -> ObjectId {
    game.object_id(CardId(card)).unwrap()
}
fn reach_attackers(game: &mut Game) {
    for _ in 0..32 {
        match game.advance(Budget::unlimited()) {
            Advance::AwaitingDecision(Request::Attackers { .. }) => return,
            Advance::AwaitingDecision(r) => game
                .respond(r.player(), r.decision_id(), Response::Pass)
                .unwrap(),
            other => panic!("unexpected {other:?}"),
        }
    }
    panic!("no attackers")
}
fn answer(game: &mut Game, response: Response) {
    let Advance::AwaitingDecision(r) = game.advance(Budget::unlimited()) else {
        panic!("decision")
    };
    game.respond(r.player(), r.decision_id(), response).unwrap();
}
#[test]
fn simultaneous_all_player_loss_is_draw_without_intermediate_winner() {
    let mut g = game();
    for p in &mut g.state.players {
        p.life = Quantity::zero();
    }
    g.stabilize().unwrap();
    assert_eq!(
        g.advance(Budget::new(0)),
        Advance::Suspended {
            required_minimum: 1
        }
    );
    assert_eq!(g.advance(Budget::new(1)), Advance::Draw);
    assert!(g.live_players().is_empty());
    assert!(
        !g.events()
            .iter()
            .any(|e| matches!(e, Event::GameWon { .. }))
    );
    assert_eq!(
        g.events()
            .iter()
            .filter(|e| matches!(e, Event::GameDrawn))
            .count(),
        1
    );
}
#[test]
fn empty_library_requires_attempt_and_loss_preserves_surviving_turn_order() {
    let mut g = game();
    g.stabilize().unwrap();
    assert!(matches!(
        g.advance(Budget::unlimited()),
        Advance::AwaitingDecision(_)
    ));
    g.commit(
        ProposedEvent::FailedDraw {
            player: PlayerId(1),
        },
        BatchCause::DrawStep {
            player: PlayerId(1),
        },
    )
    .unwrap();
    g.stabilize().unwrap();
    assert!(matches!(
        g.advance(Budget::unlimited()),
        Advance::AwaitingDecision(_)
    ));
    assert_eq!(
        g.live_players(),
        vec![PlayerId(0), PlayerId(2), PlayerId(3)]
    );
    assert!(g.events().iter().any(|e|matches!(e,Event::PlayersLost {losses} if losses==&vec![(PlayerId(1),PlayerLossReason::FailedDraw)])));
}
#[test]
fn zero_toughness_and_lethal_damage_are_distinct_typed_state_based_moves() {
    let mut g = game();
    Arc::make_mut(&mut g.prepared.catalog)
        .definitions
        .iter_mut()
        .find(|d| d.oracle_id == BEARS)
        .unwrap()
        .characteristics
        .toughness = Some(Quantity::zero());
    g.stabilize().unwrap();
    g.advance(Budget::unlimited());
    assert_eq!(
        g.events()
            .iter()
            .filter(|e| matches!(
                e,
                Event::Committed {
                    actual: EventOutcome::StateBasedMoved {
                        reason: CreatureLossReason::ZeroToughness,
                        ..
                    },
                    ..
                }
            ))
            .count(),
        6
    );
    assert!(!g.events().iter().any(|e| matches!(
        e,
        Event::Committed {
            actual: EventOutcome::StateBasedMoved {
                reason: CreatureLossReason::LethalDamage,
                ..
            },
            ..
        }
    )));
}
#[test]
fn damage_aggregate_sums_shared_recipients_and_preserves_pre_state_facts() {
    let mut g = game();
    let a = object(&g, 0);
    let b = object(&g, 3);
    let c = object(&g, 4);
    let batch = g
        .commit_batch(
            BatchCause::Combat,
            EventGroup::Occurrence {
                proposed: ProposedEvent::CombatDamage {
                    damage: vec![
                        DamageOccurrence {
                            source: b,
                            controller: PlayerId(1),
                            recipient: DamageRecipient::Creature { object: a },
                            amount: Quantity::from(1),
                        },
                        DamageOccurrence {
                            source: c,
                            controller: PlayerId(1),
                            recipient: DamageRecipient::Creature { object: a },
                            amount: Quantity::from(1),
                        },
                        DamageOccurrence {
                            source: a,
                            controller: PlayerId(0),
                            recipient: DamageRecipient::Creature { object: b },
                            amount: Quantity::from(2),
                        },
                    ],
                },
            },
        )
        .unwrap();
    assert_eq!(g.current(a).unwrap().marked_damage, Quantity::from(2));
    assert_eq!(
        g.characteristics(a).unwrap().toughness,
        Some(Quantity::from(2))
    );
    assert!(
        batch
            .before
            .objects
            .iter()
            .all(|o| o.marked_damage == Quantity::zero())
    );
    assert!(
        matches!(batch.result(&[]),Some(EventOutcome::DamageDealt {results,..}) if results.len()==2)
    );
    g.stabilize().unwrap();
    g.advance(Budget::new(1));
    assert_eq!(
        g.zone(object(&g, 0)),
        Some(Zone::Graveyard { owner: PlayerId(0) })
    );
    assert_eq!(
        g.zone(object(&g, 3)),
        Some(Zone::Graveyard { owner: PlayerId(1) })
    );
}
#[test]
fn overlapping_clear_damage_composition_and_invalid_aggregate_roll_back_atomically() {
    let mut g = game();
    let a = object(&g, 0);
    let b = object(&g, 3);
    let damage = ProposedEvent::CombatDamage {
        damage: vec![DamageOccurrence {
            source: a,
            controller: PlayerId(0),
            recipient: DamageRecipient::Creature { object: b },
            amount: Quantity::from(1),
        }],
    };
    let before = g.snapshot().unwrap();
    assert!(
        g.commit_batch(
            BatchCause::Fixture,
            EventGroup::Simultaneous {
                occurrences: vec![ProposedEvent::ClearDamage, damage.clone()]
            }
        )
        .is_err()
    );
    assert_eq!(g.snapshot().unwrap(), before);
    assert!(
        g.commit_batch(
            BatchCause::Fixture,
            EventGroup::Ordered {
                children: vec![
                    EventGroup::Occurrence { proposed: damage },
                    EventGroup::Occurrence {
                        proposed: ProposedEvent::CombatDamage {
                            damage: vec![DamageOccurrence {
                                source: a,
                                controller: PlayerId(3),
                                recipient: DamageRecipient::Creature { object: b },
                                amount: Quantity::from(1)
                            }]
                        }
                    }
                ]
            }
        )
        .is_err()
    );
    assert_eq!(g.snapshot().unwrap(), before);
}
#[test]
fn cleanup_sba_exception_grants_priority_then_repeats_cleanup_and_clears_damage() {
    let mut g = game();
    g.state.turn.step = TurnStep::Cleanup;
    g.state.frame = Frame::CleanupClear;
    g.state.players[1].life = Quantity::zero();
    g.state.objects.get_mut(&CardId(0)).unwrap().marked_damage = Quantity::from(1);
    assert!(matches!(
        g.advance(Budget::unlimited()),
        Advance::AwaitingDecision(Request::Priority {
            player: PlayerId(0),
            ..
        })
    ));
    assert!(g.state.cleanup_repeat);
    assert_eq!(g.state.objects[&CardId(0)].marked_damage, Quantity::zero());
    for _ in 0..3 {
        answer(&mut g, Response::Pass);
    }
    assert!(matches!(
        g.advance(Budget::unlimited()),
        Advance::AwaitingDecision(Request::Priority {
            player: PlayerId(2),
            ..
        })
    ));
    assert_eq!(g.state.turn.number, 2);
    assert_eq!(g.state.turn.step, TurnStep::Upkeep);
    assert_eq!(
        g.events()
            .iter()
            .filter(|e| matches!(
                e,
                Event::Committed {
                    actual: EventOutcome::DamageCleared { .. },
                    ..
                }
            ))
            .count(),
        2
    );
}
#[test]
fn control_change_and_reentry_reset_readiness_even_before_untap_processing() {
    let mut g = game();
    let a = object(&g, 0);
    g.state.control_effects.push(ControlEffect {
        id: 0,
        object: a,
        controller: PlayerId(1),
        source: ControlSource::Persistent,
    });
    g.state.objects.get_mut(&CardId(0)).unwrap().controller = PlayerId(1);
    g.state.objects.get_mut(&CardId(0)).unwrap().attack_ready = true;
    g.apply_departure(PlayerId(1)).unwrap();
    assert_eq!(g.current(a).unwrap().controller, PlayerId(0));
    assert!(!g.current(a).unwrap().attack_ready);
    let moved = g.move_object(a, Zone::Hand { owner: PlayerId(0) }).unwrap();
    let entered = g.move_object(moved.successor, Zone::Battlefield).unwrap();
    assert!(!g.current(entered.successor).unwrap().attack_ready);
    g.state.turn.step = TurnStep::Cleanup;
    g.state.turn_owner = PlayerId(3);
    g.state.frame = Frame::Transition;
    g.transition_turn_step().unwrap();
    assert!(g.current(entered.successor).unwrap().attack_ready);
    g.state.objects.get_mut(&CardId(0)).unwrap().attack_ready = false;
    g.enter_turn_step().unwrap();
    assert!(!g.current(entered.successor).unwrap().attack_ready);
}
#[test]
fn zero_power_has_no_positive_damage_occurrence_and_empty_attack_skips_damage() {
    let mut g = game();
    reach_attackers(&mut g);
    answer(&mut g, Response::Attackers { attacks: vec![] });
    for _ in 0..4 {
        answer(&mut g, Response::Pass);
    }
    g.advance(Budget::unlimited());
    assert_eq!(g.turn().step, TurnStep::EndCombat);
    assert!(!g.events().iter().any(|e| matches!(
        e,
        Event::Committed {
            actual: EventOutcome::DamageDealt { .. },
            ..
        }
    )));
    let mut g = game();
    Arc::make_mut(&mut g.prepared.catalog)
        .definitions
        .iter_mut()
        .find(|d| d.oracle_id == BEARS)
        .unwrap()
        .characteristics
        .power = Some(Quantity::zero());
    let a = object(&g, 0);
    g.state.combat = vec![Attacker {
        attacker: a,
        defender: PlayerId(1),
        blocked: false,
        blockers: vec![],
    }];
    g.state.frame = Frame::Damage;
    g.advance(Budget::unlimited());
    assert_eq!(g.state.players[1].life, Quantity::from(2));
    assert!(!g.events().iter().any(|e| matches!(
        e,
        Event::Committed {
            actual: EventOutcome::DamageDealt { .. },
            ..
        }
    )));
}
#[test]
fn nonactive_priority_mana_and_cast_completion_retain_priority_through_stabilization() {
    let mut g = game();
    g.state.priority = PlayerId(3);
    g.refresh_visibility().unwrap();
    let mana = object(&g, 6);
    answer(&mut g, Response::ActivateMana { object: mana });
    assert!(matches!(
        g.advance(Budget::unlimited()),
        Advance::AwaitingDecision(Request::Priority {
            player: PlayerId(3),
            ..
        })
    ));
    answer(&mut g, Response::Cast { card: CardId(7) });
    let target = object(&g, 0);
    answer(
        &mut g,
        Response::Targets {
            objects: vec![target],
        },
    );
    answer(&mut g, Response::FinishMana { undo_mana: false });
    assert!(matches!(
        g.advance(Budget::unlimited()),
        Advance::AwaitingDecision(Request::Priority {
            player: PlayerId(3),
            ..
        })
    ));
}
#[test]
fn simultaneous_losers_remove_all_owned_objects_before_control_restoration() {
    let mut g = game();
    let owned = object(&g, 3);
    let survivor = object(&g, 0);
    g.state.objects.get_mut(&CardId(0)).unwrap().controller = PlayerId(1);
    g.state.control_effects = vec![
        ControlEffect {
            id: 0,
            object: survivor,
            controller: PlayerId(1),
            source: ControlSource::Static { object: owned },
        },
        ControlEffect {
            id: 1,
            object: survivor,
            controller: PlayerId(2),
            source: ControlSource::Persistent,
        },
    ];
    g.state.players[1].life = Quantity::zero();
    g.state.players[2].life = Quantity::zero();
    g.stabilize().unwrap();
    g.advance(Budget::new(1));
    assert_eq!(g.current(survivor).unwrap().controller, PlayerId(0));
    assert!(!g.events().iter().any(|e| matches!(
        e,
        Event::ControlChanged {
            controller: PlayerId(2),
            ..
        }
    )));
    let last_removed = g
        .events()
        .iter()
        .rposition(|e| matches!(e, Event::ObjectLeftGame { .. }))
        .unwrap();
    let first_control = g
        .events()
        .iter()
        .position(|e| matches!(e, Event::ControlEffectEnded { .. }))
        .unwrap();
    assert!(last_removed < first_control);
}

#[test]
fn last_unpassed_departure_during_stabilization_advances_without_redundant_pass() {
    let mut g = game();
    answer(&mut g, Response::Pass);
    answer(&mut g, Response::Pass);
    answer(&mut g, Response::Pass);
    assert_eq!(
        g.advance(Budget::new(1)),
        Advance::Suspended {
            required_minimum: 1
        }
    );
    assert!(matches!(g.state.frame, Frame::Stabilize { .. }));
    g.concede(PlayerId(3)).unwrap();
    assert_eq!(
        g.advance(Budget::new(1)),
        Advance::Suspended {
            required_minimum: 1
        }
    );
    let restored = Game::restore(g.prepared.clone(), &g.snapshot().unwrap()).unwrap();
    assert_eq!(restored.state, g.state);
    let Advance::AwaitingDecision(Request::Priority {
        player: PlayerId(0),
        ..
    }) = g.advance(Budget::unlimited())
    else {
        panic!("expected next step priority")
    };
    assert_eq!(g.turn().step, TurnStep::BeginningCombat);
    assert!(g.state.passes.is_empty());
}

#[test]
fn oversized_public_declarations_reject_before_handle_conversion() {
    let mut g = game();
    reach_attackers(&mut g);
    let QueryResult::Complete(QueryValue::Decision(Some(PlayerRequest::Attackers {
        token,
        candidates,
        ..
    }))) = g.player_decision_for(PlayerId(0), Budget::unlimited())
    else {
        panic!("request")
    };
    let before = g.state_digest().unwrap();
    let oversized = PlayerResponse::Attackers {
        attacks: vec![(candidates[0].clone(), PlayerId(1)); g.state.objects.len() + 1],
    };
    assert!(matches!(
        g.check_player_response(PlayerId(0), token, oversized, Budget::unlimited()),
        QueryResult::Rejected(QueryRejection::InvalidResponse)
    ));
    assert_eq!(g.state_digest().unwrap(), before);
}

#[test]
fn turn_start_consumes_control_history_facts_and_rejects_simultaneous_object_writes() {
    let mut g = game();
    g.state.turn.step = TurnStep::Cleanup;
    g.state.turn_owner = PlayerId(3);
    g.state.frame = Frame::Transition;
    let a = object(&g, 0);
    g.state.objects.get_mut(&CardId(0)).unwrap().attack_ready = false;
    let (next, owner) = g.next_turn_state().unwrap();
    let before = g.snapshot().unwrap();
    assert!(
        g.commit_batch(
            BatchCause::TurnProgression,
            EventGroup::Simultaneous {
                occurrences: vec![
                    ProposedEvent::TurnTransition {
                        next: next.clone(),
                        owner
                    },
                    ProposedEvent::Tap { object: a }
                ]
            }
        )
        .is_err()
    );
    assert_eq!(g.snapshot().unwrap(), before);
    let batch = g
        .commit_batch(
            BatchCause::TurnProgression,
            EventGroup::Occurrence {
                proposed: ProposedEvent::TurnTransition { next, owner },
            },
        )
        .unwrap();
    assert!(
        !batch
            .before
            .objects
            .iter()
            .find(|o| o.object == a)
            .unwrap()
            .attack_ready
    );
    assert!(g.current(a).unwrap().attack_ready);
}

#[test]
fn hidden_library_order_and_opponent_hand_do_not_change_public_projection() {
    let prepared = Prepared::admit(
        Catalog::read(include_str!("../../../../tests/fixtures/expected-ir.json")).unwrap(),
        ScenarioConfig::combat_game(),
    )
    .unwrap();
    let left = Game::new(prepared);
    let mut right = left.fork();
    right
        .state
        .libraries
        .get_mut(&PlayerId(0))
        .unwrap()
        .reverse();
    let a = right.state.cards[&CardId(18)].oracle_id.clone();
    let b = right.state.cards[&CardId(19)].oracle_id.clone();
    right.state.cards.get_mut(&CardId(18)).unwrap().oracle_id = b;
    right.state.cards.get_mut(&CardId(19)).unwrap().oracle_id = a;
    right.refresh_visibility().unwrap();
    let QueryResult::Complete(QueryValue::Observation(a)) =
        left.observe(PlayerId(0), Budget::unlimited())
    else {
        panic!("observation")
    };
    let QueryResult::Complete(QueryValue::Observation(b)) =
        right.observe(PlayerId(0), Budget::unlimited())
    else {
        panic!("observation")
    };
    assert_eq!(a, b);
}
