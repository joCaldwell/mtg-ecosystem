use super::*;

fn prepared(config: ScenarioConfig) -> Prepared {
    let source = if config.library_profile() {
        include_str!("../../../../tests/fixtures/expected-library-ir.json")
    } else {
        include_str!("../../../../tests/fixtures/expected-damage-ir.json")
    };
    Prepared::admit(Catalog::read(source).unwrap(), config).unwrap()
}
fn settle(g: &mut Game) -> Request {
    let Advance::AwaitingDecision(request) = g.advance(Budget::unlimited()) else {
        panic!("expected request")
    };
    request
}
fn answer(g: &mut Game, response: Response) {
    let r = settle(g);
    g.respond(r.player(), r.decision_id(), response).unwrap();
}
fn recover(g: &Game) {
    let restored = Game::restore(g.prepared.as_ref().clone(), &g.snapshot().unwrap()).unwrap();
    assert_eq!(restored.state, g.state);
    assert_eq!(restored.trace, g.trace);
}
fn view(g: &Game, player: u8) -> Observation {
    let QueryResult::Complete(QueryValue::Observation(observation)) =
        g.observe(PlayerId(player), Budget::unlimited())
    else {
        panic!("observation")
    };
    observation
}
fn passive(request: Request) -> Response {
    match request {
        Request::Priority { .. } => Response::Pass,
        Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
        Request::Blockers { .. } => Response::Blockers { blocks: vec![] },
        Request::Discard {
            count, candidates, ..
        } => Response::Discard {
            objects: candidates.into_iter().take(count).collect(),
        },
        other => panic!("unexpected {other:?}"),
    }
}
fn tuck_choice(g: &mut Game, target: u32) {
    answer(g, Response::Cast { card: CardId(0) });
    answer(
        g,
        Response::Targets {
            objects: vec![g.object_id(CardId(target)).unwrap()],
        },
    );
    for land in 1..=3 {
        answer(
            g,
            Response::ActivateMana {
                object: g.object_id(CardId(land)).unwrap(),
            },
        );
    }
    answer(g, Response::FinishMana { undo_mana: true });
    for _ in 0..4 {
        answer(g, Response::Pass);
    }
    settle(g);
}
fn reach_draw_choice(g: &mut Game) {
    for _ in 0..1000 {
        let request = settle(g);
        if matches!(
            request,
            Request::CommanderReturn {
                destination: Zone::Hand { .. },
                ..
            }
        ) {
            return;
        }
        answer(g, passive(request));
    }
    panic!("no draw choice")
}

#[test]
fn natural_commander_damage_counts_actual_hits_and_loses_at_next_sba() {
    for borrowed in [false, true] {
        let mut g = Game::new(prepared(if borrowed {
            ScenarioConfig::commander_damage_borrowed()
        } else {
            ScenarioConfig::commander_damage()
        }));
        let attacker = if borrowed { PlayerId(1) } else { PlayerId(0) };
        let mut witnessed = vec![];
        for _ in 0..5000 {
            let result = g.advance(Budget::new(1));
            let damage = g
                .commander_ledger()
                .history(CardId(0))
                .unwrap()
                .combat_damage
                .get(&PlayerId(2))
                .cloned()
                .unwrap_or_default();
            if damage > Quantity::zero() && witnessed.last() != Some(&damage) {
                witnessed.push(damage.clone());
                assert!(g.is_live(PlayerId(2)), "loss must await next SBA");
                assert_eq!(
                    g.state.players[2].life,
                    if damage == Quantity::from(18) {
                        Quantity::from(22)
                    } else {
                        Quantity::from(4)
                    }
                );
                recover(&g);
            }
            if !g.is_live(PlayerId(2)) {
                break;
            }
            if let Advance::AwaitingDecision(request) = result {
                let response = if matches!(request,Request::Attackers {player,..} if player==attacker)
                {
                    Response::Attackers {
                        attacks: vec![(g.object_id(CardId(0)).unwrap(), PlayerId(2))],
                    }
                } else {
                    passive(request.clone())
                };
                g.respond(request.player(), request.decision_id(), response)
                    .unwrap();
            } else {
                assert!(matches!(result, Advance::Suspended { .. }));
            }
        }
        assert_eq!(witnessed, vec![Quantity::from(18), Quantity::from(36)]);
        assert!(!g.is_live(PlayerId(2)));
        assert_eq!(g.state.players[2].life, Quantity::from(4));
        assert!(g.events().iter().any(|event|matches!(event,Event::PlayersLost {losses} if losses==&vec![(PlayerId(2),PlayerLossReason::CommanderDamage)])));
        recover(&g);
        settle(&mut g);
        let replay = Game::replay(g.prepared.as_ref().clone(), &g.replay_journal()).unwrap();
        assert_eq!(replay.state_digest().unwrap(), g.state_digest().unwrap());
    }
}
#[test]
fn top_library_replacement_and_draw_replacement_are_separate_owner_choices() {
    for (tuck_return, draw_return) in [(true, false), (false, true), (false, false)] {
        let mut g = Game::new(prepared(ScenarioConfig::commander_library()));
        let original = g.object_id(CardId(4)).unwrap();
        tuck_choice(&mut g, 4);
        assert!(
            matches!(settle(&mut g),Request::CommanderReturn {player:PlayerId(1),object,destination:Zone::Library {owner:PlayerId(1)},..} if object==original)
        );
        recover(&g);
        let own = view(&g, 1);
        let Some(PlayerRequest::CommanderReturn {
            object: Some(handle),
            designation,
            ..
        }) = own.decision
        else {
            panic!("visible tuck choice")
        };
        assert_eq!(designation, own.view.commanders[0].designation);
        answer(
            &mut g,
            Response::CommanderReturn {
                to_command: tuck_return,
            },
        );
        settle(&mut g);
        assert!(g.resolve_handle(PlayerId(1), &handle).is_err());
        if tuck_return {
            assert_eq!(
                g.state.libraries[&PlayerId(1)],
                vec![CardId(10), CardId(11)]
            );
            assert_eq!(
                g.state.objects[&CardId(4)].zone,
                Zone::Command { owner: PlayerId(1) }
            );
        } else {
            assert_eq!(
                g.state.libraries[&PlayerId(1)],
                vec![CardId(4), CardId(10), CardId(11)]
            );
            for player in 0..4 {
                assert!(
                    !view(&g, player)
                        .view
                        .objects
                        .iter()
                        .any(|object| object.oracle_id == ISAMARU)
                );
            }
            reach_draw_choice(&mut g);
            let observation = view(&g, 1);
            let Some(PlayerRequest::CommanderReturn {
                object: None,
                designation: draw_designation,
                destination: Zone::Hand { owner: PlayerId(1) },
                ..
            }) = observation.decision
            else {
                panic!("hidden draw choice")
            };
            assert_eq!(draw_designation, designation);
            assert_eq!(observation.view.players[1].library_count, 3);
            assert_eq!(observation.view.priority, None);
            for player in 0..4 {
                assert!(
                    !view(&g, player)
                        .view
                        .objects
                        .iter()
                        .any(|object| object.oracle_id == ISAMARU)
                );
            }
            recover(&g);
            answer(
                &mut g,
                Response::CommanderReturn {
                    to_command: draw_return,
                },
            );
            settle(&mut g);
            assert_eq!(
                g.state.libraries[&PlayerId(1)],
                vec![CardId(10), CardId(11)]
            );
            assert_eq!(
                g.state.objects[&CardId(4)].zone,
                if draw_return {
                    Zone::Command { owner: PlayerId(1) }
                } else {
                    Zone::Hand { owner: PlayerId(1) }
                }
            );
            assert_eq!(g.object_id(CardId(4)).unwrap().incarnation, 2);
        }
        recover(&g);
    }
}
#[test]
fn ordinary_top_library_instruction_precedes_existing_order_and_draws_once() {
    let mut g = Game::new(prepared(ScenarioConfig::commander_library()));
    tuck_choice(&mut g, 16);
    assert_eq!(
        g.state.libraries[&PlayerId(2)],
        vec![CardId(16), CardId(12), CardId(13)]
    );
    for _ in 0..1000 {
        let request = settle(&mut g);
        if g.state.objects[&CardId(16)].zone == (Zone::Hand { owner: PlayerId(2) }) {
            break;
        }
        assert!(!matches!(request, Request::CommanderReturn { .. }));
        answer(&mut g, passive(request));
    }
    assert_eq!(
        g.state.objects[&CardId(16)].zone,
        Zone::Hand { owner: PlayerId(2) }
    );
    assert_eq!(
        g.state.libraries[&PlayerId(2)],
        vec![CardId(12), CardId(13)]
    );
    let event = g
        .trace
        .records
        .iter()
        .find_map(|record| match &record.record {
            EvidenceRecord::Batch { batch }
                if matches!(
                    batch
                        .occurrences
                        .first()
                        .map(|occurrence| &occurrence.proposed),
                    Some(ProposedEvent::PutOnTopOwnerLibrary { .. })
                ) =>
            {
                Some(batch)
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(
        event.before.libraries,
        vec![LibraryBefore {
            owner: PlayerId(2),
            cards: vec![CardId(12), CardId(13)]
        }]
    );
    assert!(matches!(
        event.occurrences[0].actual,
        EventOutcome::LibraryTopMoved {
            previous_top: Some(CardId(12)),
            ..
        }
    ));
    recover(&g);
}

fn damage(g: &mut Game, source: u32, player: u8, amount: i64) {
    let source = g.object_id(CardId(source)).unwrap();
    g.commit(
        ProposedEvent::CombatDamage {
            damage: vec![DamageOccurrence {
                source,
                controller: g.current(source).unwrap().controller,
                recipient: DamageRecipient::Player {
                    player: PlayerId(player),
                },
                amount: Quantity::from(amount),
            }],
        },
        BatchCause::Combat,
    )
    .unwrap();
}
#[test]
fn physical_commander_history_ignores_control_zone_life_and_noncombat_changes() {
    // Private kernel interactions: control grants, lifegain and extra physical
    // commanders are not source-backed mechanics in these exact scenarios.
    let mut p = prepared(ScenarioConfig::commander_damage());
    p.config.cards.push(CardSetup {
        id: CardId(99),
        oracle_id: YARGLE.into(),
        owner: PlayerId(1),
        controller: PlayerId(1),
        zone: Zone::Battlefield,
    });
    p.config.commanders.push(CardId(99));
    let mut g = Game::new(p);
    let initial_ledger = g.commander_ledger().clone();
    g.state
        .commanders
        .record_damage(CardId(0), PlayerId(2), true, &Quantity::zero())
        .unwrap();
    assert_eq!(g.commander_ledger(), &initial_ledger);
    let source = g.object_id(CardId(0)).unwrap();
    let target = g.object_id(CardId(99)).unwrap();
    g.commit(
        ProposedEvent::CombatDamage {
            damage: vec![DamageOccurrence {
                source,
                controller: PlayerId(0),
                recipient: DamageRecipient::Creature { object: target },
                amount: Quantity::from(1),
            }],
        },
        BatchCause::Combat,
    )
    .unwrap();
    assert_eq!(g.commander_ledger(), &initial_ledger);
    damage(&mut g, 0, 2, 10);
    damage(&mut g, 99, 2, 10);
    assert!(g.commander_ledger().combat_loss_candidates().is_empty());
    g.state
        .commanders
        .record_damage(CardId(0), PlayerId(2), false, &Quantity::from(100))
        .unwrap();
    let old = g.object_id(CardId(0)).unwrap();
    let left = g
        .move_object(old, Zone::Hand { owner: PlayerId(0) })
        .unwrap();
    let returned = g
        .move_object(left.successor, Zone::Battlefield)
        .unwrap()
        .successor;
    g.state.control_effects.push(ControlEffect {
        id: 0,
        object: returned,
        controller: PlayerId(3),
        source: ControlSource::Persistent,
    });
    g.state.objects.get_mut(&CardId(0)).unwrap().controller = PlayerId(3);
    g.state.players[2].life = Quantity::from(100);
    damage(&mut g, 0, 2, 11);
    assert_eq!(
        g.commander_ledger().combat_loss_candidates(),
        vec![PlayerId(2)]
    );
    assert_eq!(
        g.commander_ledger()
            .history(CardId(0))
            .unwrap()
            .combat_damage[&PlayerId(2)],
        Quantity::from(21)
    );
    assert_eq!(
        g.commander_ledger()
            .history(CardId(99))
            .unwrap()
            .combat_damage[&PlayerId(2)],
        Quantity::from(10)
    );
    assert_eq!(g.state.players[2].life, Quantity::from(89));
    g.state.control_effects.clear();
    g.state.objects.get_mut(&CardId(0)).unwrap().controller = PlayerId(0);
    g.apply_departure(PlayerId(0)).unwrap();
    assert!(g.commander_ledger().history(CardId(0)).is_some());
    assert!(!g.state.objects.contains_key(&CardId(0)));
}
#[test]
fn commander_and_life_loss_candidates_commit_one_simultaneous_wave() {
    let mut g = Game::new(prepared(ScenarioConfig::commander_damage()));
    damage(&mut g, 0, 1, 21);
    g.state.players[2].life = Quantity::zero();
    g.state.players[3].failed_draw = true;
    g.stabilize().unwrap();
    assert_eq!(
        g.advance(Budget::new(1)),
        Advance::Finished {
            winner: PlayerId(0)
        }
    );
    let losses = g
        .events()
        .iter()
        .find_map(|event| {
            if let Event::PlayersLost { losses } = event {
                Some(losses)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(
        losses,
        &vec![
            (PlayerId(1), PlayerLossReason::CommanderDamage),
            (PlayerId(2), PlayerLossReason::Life),
            (PlayerId(3), PlayerLossReason::FailedDraw)
        ]
    );
    let batch = g
        .trace
        .records
        .iter()
        .find_map(|record| match &record.record {
            EvidenceRecord::Batch { batch } if batch.cause == BatchCause::StateBased => Some(batch),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        batch.before.commander_damage,
        vec![CommanderDamageBefore {
            commander: CardId(0),
            recipient: PlayerId(1),
            previous: Quantity::from(21)
        }]
    );
    assert_eq!(
        g.events()
            .iter()
            .filter(|event| matches!(event, Event::GameWon { .. }))
            .count(),
        1
    );
}

#[test]
fn library_order_dependencies_and_late_damage_failure_roll_back_atomically() {
    let mut g = Game::new(prepared(ScenarioConfig::commander_library()));
    let before = g.snapshot().unwrap();
    for occurrences in [
        vec![
            ProposedEvent::PutOnTopOwnerLibrary {
                object: g.object_id(CardId(16)).unwrap(),
            },
            ProposedEvent::Move {
                object: g.object_id(CardId(1)).unwrap(),
                destination: Zone::Library { owner: PlayerId(0) },
            },
        ],
        vec![
            ProposedEvent::Move {
                object: g.object_id(CardId(1)).unwrap(),
                destination: Zone::Library { owner: PlayerId(0) },
            },
            ProposedEvent::Move {
                object: g.object_id(CardId(2)).unwrap(),
                destination: Zone::Library { owner: PlayerId(0) },
            },
        ],
    ] {
        assert!(
            g.commit_batch(
                BatchCause::Fixture,
                EventGroup::Simultaneous { occurrences }
            )
            .is_err()
        );
        assert_eq!(g.snapshot().unwrap(), before);
    }
    assert!(
        g.commit_batch(
            BatchCause::Fixture,
            EventGroup::Ordered {
                children: vec![
                    EventGroup::Occurrence {
                        proposed: ProposedEvent::PutOnTopOwnerLibrary {
                            object: g.object_id(CardId(16)).unwrap()
                        }
                    },
                    EventGroup::Occurrence {
                        proposed: ProposedEvent::Tap {
                            object: g.object_id(CardId(16)).unwrap()
                        }
                    },
                ]
            }
        )
        .is_err()
    );
    assert_eq!(g.snapshot().unwrap(), before);
    let mut g = Game::new(prepared(ScenarioConfig::commander_damage()));
    let before = g.snapshot().unwrap();
    let source = g.object_id(CardId(0)).unwrap();
    assert!(
        g.commit(
            ProposedEvent::CombatDamage {
                damage: vec![
                    DamageOccurrence {
                        source,
                        controller: PlayerId(0),
                        recipient: DamageRecipient::Player {
                            player: PlayerId(1)
                        },
                        amount: Quantity::from(18)
                    },
                    DamageOccurrence {
                        source,
                        controller: PlayerId(0),
                        recipient: DamageRecipient::Player {
                            player: PlayerId(2)
                        },
                        amount: Quantity::from(-1)
                    },
                ]
            },
            BatchCause::Combat
        )
        .is_err()
    );
    assert_eq!(g.snapshot().unwrap(), before);
}
