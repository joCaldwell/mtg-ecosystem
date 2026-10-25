use super::*;

fn catalog() -> Catalog {
    Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-damage-life-ir.json"
    ))
    .unwrap()
}
fn game() -> Game {
    Game::new(Prepared::admit(catalog(), ScenarioConfig::damage_life()).unwrap())
}
fn settle(g: &mut Game) -> Request {
    for _ in 0..500 {
        match g.advance(Budget::new(1)) {
            Advance::AwaitingDecision(r) => return r,
            Advance::Suspended { .. } => (),
            other => panic!("unexpected boundary {other:?}"),
        }
    }
    panic!("decision budget exhausted")
}
fn answer(g: &mut Game, response: Response) {
    let r = settle(g);
    g.respond(r.player(), r.decision_id(), response).unwrap();
}
fn hit(g: &Game, source: u32, recipient: DamageRecipient, amount: i64) -> ProposedEvent {
    let fact = g
        .object_before(g.object_id(CardId(source)).unwrap())
        .unwrap();
    ProposedEvent::NoncombatDamage {
        damage: Box::new(NoncombatDamageOccurrence {
            controller: fact.controller,
            source: fact,
            recipient,
            amount: amount.into(),
        }),
    }
}
fn leaf(proposed: ProposedEvent) -> EventGroup {
    EventGroup::Occurrence { proposed }
}
fn recover(g: &Game) {
    let restored = Game::restore(g.prepared.as_ref().clone(), &g.snapshot().unwrap()).unwrap();
    let replayed = Game::replay(g.prepared.as_ref().clone(), &g.replay_journal()).unwrap();
    assert_eq!(restored.state_digest().unwrap(), g.state_digest().unwrap());
    assert_eq!(replayed.state_digest().unwrap(), g.state_digest().unwrap());
    assert_eq!(restored.trace(), g.trace());
    assert_eq!(replayed.trace(), g.trace());
}
fn detached_pinger() -> Game {
    let mut g = game();
    let source = g.object_id(CardId(0)).unwrap();
    answer(
        &mut g,
        Response::ActivateAbility {
            object: source,
            ability: 0,
        },
    );
    answer(
        &mut g,
        Response::EntityTargets {
            entities: vec![TargetEntity::Player {
                player: PlayerId(1),
            }],
        },
    );
    answer(&mut g, Response::Pass);
    answer(&mut g, Response::Cast { card: CardId(21) });
    answer(
        &mut g,
        Response::Targets {
            objects: vec![source],
        },
    );
    let land = g.object_id(CardId(18)).unwrap();
    answer(&mut g, Response::ActivateMana { object: land });
    answer(&mut g, Response::FinishMana { undo_mana: false });
    answer(
        &mut g,
        Response::SpendMana {
            spending: BTreeMap::from([(Color::U, 1.into())]),
        },
    );
    for _ in 0..4 {
        answer(&mut g, Response::Pass);
    }
    settle(&mut g);
    assert_eq!(g.state.stack.len(), 1);
    assert!(g.current(source).is_err());
    assert_eq!(
        g.current(g.object_id(CardId(0)).unwrap()).unwrap().zone,
        Zone::Hand { owner: PlayerId(0) }
    );
    g
}

#[test]
fn damage_event_captures_consumed_source_and_actual_creature_marks() {
    let mut g = game();
    let target = g.object_id(CardId(15)).unwrap();
    let proposed = hit(&g, 0, DamageRecipient::Creature { object: target }, 1);
    let ProposedEvent::NoncombatDamage { damage } = proposed.clone() else {
        panic!()
    };
    let receipt = g.commit_batch(BatchCause::Fixture, leaf(proposed)).unwrap();
    assert_eq!(
        receipt.before.damage_sources.as_slice(),
        std::slice::from_ref(&damage.source)
    );
    assert_eq!(receipt.before.objects[0].object, target);
    assert_eq!(receipt.before.objects[0].marked_damage, Quantity::zero());
    let Some(EventOutcome::NoncombatDamageDealt {
        damage: actual,
        result,
        ..
    }) = receipt.result(&[])
    else {
        panic!("missing typed damage outcome")
    };
    assert_eq!(actual, &damage);
    assert_eq!(result.recipient, damage.recipient);
    assert_eq!(result.previous, Quantity::zero());
    assert_eq!(result.current, 1.into());
    assert_eq!(g.current(target).unwrap().marked_damage, 1.into());
    assert_eq!(g.state.players[1].life, 40.into());
}

#[test]
fn commander_noncombat_damage_and_life_changes_never_write_combat_history() {
    // Private domain fixture: the damage kernel supports any physical source,
    // including a designated commander, without making combat-damage claims.
    let mut g = game();
    let ledger = g.state.commanders.clone();
    let proposed = hit(
        &g,
        14,
        DamageRecipient::Player {
            player: PlayerId(0),
        },
        3,
    );
    let receipt = g
        .commit_batch(
            BatchCause::Fixture,
            EventGroup::Ordered {
                children: vec![
                    leaf(proposed),
                    leaf(ProposedEvent::LoseLife {
                        player: PlayerId(0),
                        amount: 4.into(),
                    }),
                    leaf(ProposedEvent::GainLife {
                        player: PlayerId(0),
                        amount: 4.into(),
                    }),
                ],
            },
        )
        .unwrap();
    assert!(
        matches!(receipt.result(&[0]),Some(EventOutcome::NoncombatDamageDealt {result:DamageResult {previous,current,..},..}) if *previous==40.into() && *current==37.into())
    );
    assert!(
        matches!(receipt.result(&[1]),Some(EventOutcome::LifeLost {previous,current,..}) if *previous==37.into() && *current==33.into())
    );
    assert!(
        matches!(receipt.result(&[2]),Some(EventOutcome::LifeGained {previous,current,..}) if *previous==33.into() && *current==37.into())
    );
    assert_eq!(g.state.commanders, ledger);
    assert!(receipt.before.commander_damage.is_empty());
}

#[test]
fn ordered_life_events_measure_zero_then_recover_before_state_based_actions() {
    let mut g = Game::new(Prepared::admit(catalog(), ScenarioConfig::damage_life_low()).unwrap());
    let receipt = g
        .commit_batch(
            BatchCause::Fixture,
            EventGroup::Ordered {
                children: vec![
                    leaf(ProposedEvent::LoseLife {
                        player: PlayerId(0),
                        amount: 4.into(),
                    }),
                    leaf(ProposedEvent::GainLife {
                        player: PlayerId(0),
                        amount: 4.into(),
                    }),
                ],
            },
        )
        .unwrap();
    assert_eq!(
        receipt.occurrences[1].before.players[0].life,
        Quantity::zero()
    );
    assert!(
        matches!(receipt.result(&[0]),Some(EventOutcome::LifeLost {current,..}) if *current==Quantity::zero())
    );
    assert!(
        matches!(receipt.result(&[1]),Some(EventOutcome::LifeGained {previous,current,..}) if *previous==Quantity::zero() && *current==4.into())
    );
    assert_eq!(g.state.players[0].life, 4.into());
    assert!(g.is_live(PlayerId(0)));
}

#[test]
fn independent_damage_and_life_occurrences_compose_but_shared_writes_reject() {
    let initial = game();
    let creature = initial.object_id(CardId(15)).unwrap();
    let damage = hit(
        &initial,
        0,
        DamageRecipient::Creature { object: creature },
        1,
    );
    let mut g = initial.fork();
    let receipt = g
        .commit_batch(
            BatchCause::Fixture,
            EventGroup::Simultaneous {
                occurrences: vec![
                    damage.clone(),
                    ProposedEvent::GainLife {
                        player: PlayerId(1),
                        amount: 4.into(),
                    },
                ],
            },
        )
        .unwrap();
    assert_eq!(receipt.occurrences.len(), 2);
    assert_eq!(g.current(creature).unwrap().marked_damage, 1.into());
    assert_eq!(g.state.players[1].life, 44.into());
    for group in [
        EventGroup::Simultaneous {
            occurrences: vec![damage.clone(), damage],
        },
        EventGroup::Simultaneous {
            occurrences: vec![
                hit(
                    &initial,
                    0,
                    DamageRecipient::Player {
                        player: PlayerId(1),
                    },
                    1,
                ),
                ProposedEvent::GainLife {
                    player: PlayerId(1),
                    amount: 4.into(),
                },
            ],
        },
        EventGroup::Simultaneous {
            occurrences: vec![
                hit(
                    &initial,
                    0,
                    DamageRecipient::Player {
                        player: PlayerId(1),
                    },
                    1,
                ),
                ProposedEvent::Move {
                    object: initial.object_id(CardId(0)).unwrap(),
                    destination: Zone::Exile,
                },
            ],
        },
    ] {
        let mut branch = initial.fork();
        assert!(branch.commit_batch(BatchCause::Fixture, group).is_err());
        assert_eq!(branch.state, initial.state);
        assert_eq!(branch.trace, initial.trace);
    }
}

#[test]
fn invalid_damage_life_quantities_recipients_and_source_metadata_are_atomic() {
    let initial = game();
    let recipient = DamageRecipient::Player {
        player: PlayerId(1),
    };
    let mut invalid = vec![
        hit(&initial, 0, recipient.clone(), 0),
        hit(&initial, 0, recipient.clone(), -1),
        hit(
            &initial,
            0,
            DamageRecipient::Player {
                player: PlayerId(4),
            },
            1,
        ),
        hit(
            &initial,
            0,
            DamageRecipient::Creature {
                object: initial.object_id(CardId(1)).unwrap(),
            },
            1,
        ),
        hit(
            &initial,
            0,
            DamageRecipient::Creature {
                object: ObjectId {
                    card: CardId(15),
                    incarnation: 99,
                },
            },
            1,
        ),
    ];
    for amount in [0, -1] {
        invalid.push(ProposedEvent::GainLife {
            player: PlayerId(0),
            amount: amount.into(),
        });
        invalid.push(ProposedEvent::LoseLife {
            player: PlayerId(0),
            amount: amount.into(),
        });
    }
    invalid.push(ProposedEvent::GainLife {
        player: PlayerId(4),
        amount: 1.into(),
    });
    for mutation in 0..9 {
        let ProposedEvent::NoncombatDamage { mut damage } = hit(&initial, 0, recipient.clone(), 1)
        else {
            panic!()
        };
        match mutation {
            0 => damage.source.tapped = true,
            1 => damage.source.controller = PlayerId(1),
            2 => damage.source.owner = PlayerId(2),
            3 => damage.source.object.incarnation = 99,
            4 => damage.source.characteristics.power = Some(100.into()),
            5 => damage.controller = PlayerId(4),
            6 => damage.source.zone = Zone::Hand { owner: PlayerId(0) },
            7 => damage.source.default_controller = PlayerId(1),
            _ => damage.source.object.card = CardId(999),
        };
        invalid.push(ProposedEvent::NoncombatDamage { damage });
    }
    for proposed in invalid {
        let mut g = initial.fork();
        let token = g.state_token.clone();
        assert!(g.commit_batch(BatchCause::Fixture, leaf(proposed)).is_err());
        assert_eq!(g.state, initial.state);
        assert_eq!(g.trace, initial.trace);
        assert!(Arc::ptr_eq(&token, &g.state_token));
    }
}

#[test]
fn ordered_event_failure_rolls_back_prior_life_move_source_and_trace_writes() {
    let initial = game();
    let source = initial.object_id(CardId(0)).unwrap();
    for group in [
        EventGroup::Ordered {
            children: vec![
                leaf(ProposedEvent::GainLife {
                    player: PlayerId(0),
                    amount: 4.into(),
                }),
                leaf(hit(
                    &initial,
                    0,
                    DamageRecipient::Player {
                        player: PlayerId(1),
                    },
                    0,
                )),
            ],
        },
        EventGroup::Ordered {
            children: vec![
                leaf(ProposedEvent::Move {
                    object: source,
                    destination: Zone::Hand { owner: PlayerId(0) },
                }),
                leaf(hit(
                    &initial,
                    0,
                    DamageRecipient::Player {
                        player: PlayerId(1),
                    },
                    1,
                )),
            ],
        },
    ] {
        let mut g = initial.fork();
        let token = g.state_token.clone();
        assert!(g.commit_batch(BatchCause::Fixture, group).is_err());
        assert_eq!(g.state, initial.state);
        assert_eq!(g.trace, initial.trace);
        assert!(Arc::ptr_eq(&token, &g.state_token));
    }
}

#[test]
fn detached_paid_damage_uses_exact_last_known_facts_and_rejects_rehashed_forgery() {
    let initial = detached_pinger();
    recover(&initial);
    let fact = initial.state.stack[0].source_facts.clone().unwrap();
    assert!(fact.tapped);
    assert_eq!(fact.zone, Zone::Battlefield);
    assert_eq!(fact.object.incarnation, 0);
    for mutation in 0..5 {
        let mut g = initial.fork();
        let mut damage = NoncombatDamageOccurrence {
            source: fact.clone(),
            controller: PlayerId(0),
            recipient: DamageRecipient::Player {
                player: PlayerId(1),
            },
            amount: 1.into(),
        };
        match mutation {
            0 => damage.source.tapped = false,
            1 => damage.source.marked_damage = 9.into(),
            2 => damage.source.controller = PlayerId(1),
            3 => damage.controller = PlayerId(1),
            _ => damage.source.object.incarnation = 90,
        };
        assert!(
            g.commit_batch(
                BatchCause::Fixture,
                leaf(ProposedEvent::NoncombatDamage {
                    damage: Box::new(damage)
                })
            )
            .is_err()
        );
        assert_eq!(g.state, initial.state);
        assert_eq!(g.trace, initial.trace);
        let mut saved: Snapshot = strict_read(&initial.snapshot().unwrap()).unwrap();
        let source = saved.state.stack[0].source_facts.as_mut().unwrap();
        match mutation {
            0 => source.tapped = false,
            1 => source.marked_damage = 9.into(),
            2 => source.controller = PlayerId(1),
            3 => source.zone = Zone::Stack,
            _ => source.object.incarnation = 90,
        };
        saved.state_sha256 = digest(&saved.state).unwrap();
        assert!(
            Game::restore(
                initial.prepared.as_ref().clone(),
                &canonical(&saved).unwrap()
            )
            .is_err()
        );
    }
    let mut g = initial.fork();
    for _ in 0..4 {
        answer(&mut g, Response::Pass);
    }
    settle(&mut g);
    let damage = g
        .events()
        .iter()
        .find_map(|event| match event {
            Event::Committed {
                actual: EventOutcome::DamageCommitted { actual, .. },
                ..
            } => Some(actual),
            _ => None,
        })
        .unwrap();
    assert_eq!(damage.len(), 1);
    assert_eq!(damage[0].source, fact);
    assert_eq!(g.state.players[1].life, 39.into());
    recover(&g);
}

#[test]
fn damage_life_admission_binds_complete_catalog_version_and_exact_five_profiles() {
    let c = catalog();
    for config in [
        ScenarioConfig::damage_life(),
        ScenarioConfig::damage_life_low(),
        ScenarioConfig::damage_life_borrowed(),
        ScenarioConfig::damage_life_empty_library(),
        ScenarioConfig::damage_life_library(),
    ] {
        Prepared::admit(c.clone(), config).unwrap();
    }
    let old = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-stack-ir.json"
    ))
    .unwrap();
    assert!(Prepared::admit(old, ScenarioConfig::damage_life()).is_err());
    for mutation in 0..5 {
        let mut bad = c.clone();
        match mutation {
            0 => bad.schema_version = 6,
            1 => {
                bad.definitions.pop();
            }
            2 => {
                bad.definitions
                    .iter_mut()
                    .find(|d| d.name == "Soul Feast")
                    .unwrap()
                    .characteristics
                    .mana_cost
                    .generic = 2.into()
            }
            3 => {
                bad.definitions
                    .iter_mut()
                    .find(|d| d.name == "Lightning Bolt")
                    .unwrap()
                    .spell
                    .as_mut()
                    .unwrap()
                    .instructions[0] = Instruction::DealDamage {
                    target_slot: 0,
                    amount: 4.into(),
                    source_block: 0,
                }
            }
            _ => {
                let mut extra = bad.definitions[0].clone();
                extra.oracle_id = "zz-extra".into();
                bad.definitions.push(extra);
            }
        };
        assert!(Prepared::admit(bad, ScenarioConfig::damage_life()).is_err());
    }
    for mutation in 0..5 {
        let mut bad = ScenarioConfig::damage_life();
        match mutation {
            0 => bad.cards[0].controller = PlayerId(1),
            1 => bad.commanders = vec![CardId(0)],
            2 => bad.cards[11].zone = Zone::Battlefield,
            3 => bad.cards[3].oracle_id = bad.cards[1].oracle_id.clone(),
            _ => {
                let mut extra = bad.cards[1].clone();
                extra.id = CardId(100);
                bad.cards.push(extra);
            }
        };
        assert!(Prepared::admit(c.clone(), bad).is_err());
    }
}
