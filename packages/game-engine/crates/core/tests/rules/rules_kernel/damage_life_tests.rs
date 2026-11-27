use super::*;

fn catalog() -> Catalog {
    FixtureCatalog::Match.get().clone()
}
fn game() -> Game {
    let mut catalog = catalog();
    let config = ScenarioConfig::damage_life();
    for (card, amount) in [(0, 1), (14, 3)] {
        let definition = catalog
            .definitions
            .iter_mut()
            .find(|d| d.oracle_id == config.cards[card].oracle_id)
            .unwrap();
        Driver::add_damage_activation(definition, amount.into());
    }
    Game::new(witness_scenario(catalog, config))
}

fn hit(g: &mut Game, source: u32, recipient: DamageRecipient) -> ProposedEvent {
    let source = Driver::object(g, source);
    let ability = (g.definition(source.card).unwrap().activated.len() - 1) as u32;
    Driver::prepare_damage_activation(g, source, ability, recipient);
    ProposedEvent::CommitDamage {
        transaction: g.damage_transaction().unwrap().id,
    }
}

fn commit_damage_group(g: &mut Game, group: EventGroup) -> Result<CommittedBatch, String> {
    let cause = g.damage_transaction().unwrap().cause.clone();
    g.commit_batch(cause, group)
}

fn leaf(proposed: ProposedEvent) -> EventGroup {
    EventGroup::Occurrence { proposed }
}

#[test]
fn damage_event_captures_consumed_source_and_actual_creature_marks() {
    let mut g = game();
    let target = g.object_id(CardId(15)).unwrap();
    let proposed = hit(&mut g, 0, DamageRecipient::Permanent { object: target });
    let damage = g.damage_transaction().unwrap().entries[0].clone();
    let receipt = commit_damage_group(&mut g, leaf(proposed)).unwrap();
    assert_eq!(
        receipt.before.damage_sources.as_slice(),
        std::slice::from_ref(&damage.source)
    );
    let before = receipt
        .before
        .objects
        .iter()
        .find(|fact| fact.object == target)
        .unwrap();
    assert_eq!(before.object, target);
    assert_eq!(before.marked_damage, Quantity::zero());
    let Some(EventOutcome::DamageCommitted {
        actual, results, ..
    }) = receipt.result(&[])
    else {
        panic!("missing typed damage outcome")
    };
    assert_eq!(
        actual,
        &[ActualDamage {
            id: damage.id,
            source: damage.source,
            controller: damage.controller,
            recipient: damage.recipient,
            amount: damage.amount,
        }]
    );
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].recipient, damage.recipient);
    assert_eq!(results[0].previous, Quantity::zero());
    assert_eq!(results[0].current, 1.into());
    assert_eq!(g.current(target).unwrap().marked_damage, 1.into());
    assert_eq!(g.state.rules.players[1].life, 40.into());
}

#[test]
fn commander_noncombat_damage_and_life_changes_never_write_combat_history() {
    // A private activated ability keeps the designated commander as the
    // source while exercising ordinary noncombat resolution.
    let mut g = game();
    let ledger = g.state.rules.commanders.clone();
    Driver::pass(&mut g);
    let proposed = hit(
        &mut g,
        14,
        DamageRecipient::Player {
            player: PlayerId(0),
        },
    );
    let receipt = commit_damage_group(
        &mut g,
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
        matches!(receipt.result(&[0]), Some(EventOutcome::DamageCommitted { results, commander_damage, .. })
            if matches!(results.as_slice(), [DamageResult { previous, current, .. }]
                if *previous == 40.into() && *current == 37.into()) && commander_damage.is_empty())
    );
    assert!(
        matches!(receipt.result(&[1]),Some(EventOutcome::LifeLost {previous,current,..}) if *previous==37.into() && *current==33.into())
    );
    assert!(
        matches!(receipt.result(&[2]),Some(EventOutcome::LifeGained {previous,current,..}) if *previous==33.into() && *current==37.into())
    );
    assert_eq!(g.state.rules.commanders, ledger);
}

#[test]
fn ordered_life_events_measure_zero_then_recover_before_state_based_actions() {
    let mut g = Game::new(witness_scenario(
        catalog(),
        ScenarioConfig::damage_life_low(),
    ));
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
    assert_eq!(g.state.rules.players[0].life, 4.into());
    assert!(g.is_live(PlayerId(0)));
}

#[test]
fn ordered_damage_and_life_compose_and_damage_rejects_simultaneous_groups() {
    let mut initial = game();
    let creature = initial.object_id(CardId(15)).unwrap();
    let damage = hit(
        &mut initial,
        0,
        DamageRecipient::Permanent { object: creature },
    );
    let mut g = initial.fork();
    let gain_life = ProposedEvent::GainLife {
        player: PlayerId(1),
        amount: 4.into(),
    };
    let receipt = commit_damage_group(
        &mut g,
        EventGroup::Ordered {
            children: vec![leaf(damage.clone()), leaf(gain_life.clone())],
        },
    )
    .unwrap();
    assert_eq!(receipt.occurrences.len(), 2);
    assert_eq!(g.current(creature).unwrap().marked_damage, 1.into());
    assert_eq!(g.state.rules.players[1].life, 44.into());

    // Independent life and object writes can still share a simultaneous group.
    let mut g = game();
    let receipt = g
        .commit_batch(
            BatchCause::Fixture,
            EventGroup::Simultaneous {
                occurrences: vec![ProposedEvent::Tap { object: creature }, gain_life.clone()],
            },
        )
        .unwrap();
    assert_eq!(receipt.occurrences.len(), 2);
    assert!(g.current(creature).unwrap().tapped);
    assert_eq!(g.state.rules.players[1].life, 44.into());

    let mut player_hit = game();
    let source = player_hit.object_id(CardId(0)).unwrap();
    let hit_player = hit(
        &mut player_hit,
        0,
        DamageRecipient::Player {
            player: PlayerId(1),
        },
    );
    for (pre_commit, occurrences) in [
        (&initial, vec![damage.clone(), gain_life.clone()]),
        (&initial, vec![damage.clone(), damage]),
        (&player_hit, vec![hit_player.clone(), gain_life]),
        (
            &player_hit,
            vec![
                hit_player,
                ProposedEvent::Move {
                    object: source,
                    destination: Zone::Exile,
                },
            ],
        ),
    ] {
        let mut branch = pre_commit.fork();
        assert!(
            commit_damage_group(&mut branch, EventGroup::Simultaneous { occurrences }).is_err()
        );
        assert_eq!(branch.state, pre_commit.state);
        assert_eq!(branch.trace, pre_commit.trace);
    }
}

#[test]
fn invalid_damage_life_quantities_recipients_and_source_metadata_are_atomic() {
    let mut g = game();
    let recipient = DamageRecipient::Player {
        player: PlayerId(1),
    };
    let proposed = hit(&mut g, 0, recipient);
    let bad_recipients = [
        DamageRecipient::Player {
            player: PlayerId(4),
        },
        DamageRecipient::Permanent {
            object: g.object_id(CardId(1)).unwrap(),
        },
        DamageRecipient::Permanent {
            object: ObjectId {
                face: Default::default(),
                card: CardId(15),
                incarnation: 99,
            },
        },
    ];
    let mutations: [fn(&mut PreparedDamage); 10] = [
        |damage| damage.amount = 0.into(),
        |damage| damage.amount = (-1).into(),
        |damage| damage.source.tapped = !damage.source.tapped,
        |damage| damage.source.controller = PlayerId(1),
        |damage| damage.source.owner = PlayerId(2),
        |damage| damage.source.object.incarnation = 99,
        |damage| damage.controller = PlayerId(4),
        |damage| damage.source.zone = Zone::Hand { owner: PlayerId(0) },
        |damage| damage.source.default_controller = PlayerId(1),
        |damage| damage.source.object.card = CardId(999),
    ];
    for mutate in mutations {
        let mut branch = g.fork();
        mutate(prepared_member(&mut branch));
        assert_damage_rejected(&mut branch, leaf(proposed.clone()));
    }
    for recipient in bad_recipients {
        let mut branch = g.fork();
        prepared_member(&mut branch).recipient = recipient;
        assert_damage_rejected(&mut branch, leaf(proposed.clone()));
    }

    // Layered characteristics can legitimately change between preparation
    // and commit. Replay authenticates power at the capture boundary.
    let prepared = g.prepared.as_ref().clone();
    let valid = g.snapshot().unwrap();
    assert!(Game::restore(prepared.clone(), &valid).is_ok());
    let mut forged = g.fork();
    prepared_member(&mut forged).source.characteristics.power = Some(100.into());
    let forged_snapshot = forged.snapshot().unwrap();
    assert!(Game::restore(prepared, &forged_snapshot).is_err());

    let mut initial = game();
    for player in [PlayerId(0), PlayerId(4)] {
        for amount in [0, -1] {
            for event in [
                ProposedEvent::GainLife {
                    player,
                    amount: amount.into(),
                },
                ProposedEvent::LoseLife {
                    player,
                    amount: amount.into(),
                },
            ] {
                assert_unchanged(&mut initial, |g| {
                    g.commit_batch(BatchCause::Fixture, leaf(event))
                });
            }
        }
    }
    assert_unchanged(&mut initial, |g| {
        g.commit_batch(
            BatchCause::Fixture,
            leaf(ProposedEvent::GainLife {
                player: PlayerId(4),
                amount: 1.into(),
            }),
        )
    });
}

fn prepared_member(g: &mut Game) -> &mut PreparedDamage {
    let Frame::DamageEffects { transaction } = &mut g.state.rules.frame else {
        panic!("missing damage transaction")
    };
    &mut transaction.entries[0]
}

fn assert_unchanged(
    g: &mut Game,
    rejected: impl FnOnce(&mut Game) -> Result<CommittedBatch, String>,
) {
    let before = g.state.clone();
    let trace = g.trace.clone();
    let token = g.state_token.clone();
    assert!(rejected(g).is_err());
    assert_eq!(g.state, before);
    assert_eq!(g.trace, trace);
    assert!(Arc::ptr_eq(&token, &g.state_token));
}

fn assert_damage_rejected(g: &mut Game, group: EventGroup) {
    assert_unchanged(g, |g| commit_damage_group(g, group));
}

#[test]
fn ordered_event_failure_rolls_back_prior_life_move_source_and_trace_writes() {
    let mut initial = game();
    let source = initial.object_id(CardId(0)).unwrap();
    let damage = hit(
        &mut initial,
        0,
        DamageRecipient::Player {
            player: PlayerId(1),
        },
    );
    let movement = ProposedEvent::Move {
        object: source,
        destination: Zone::Hand { owner: PlayerId(0) },
    };

    // A legitimate departure still leaves the activation's source bound by LKI.
    let mut g = initial.fork();
    let receipt = commit_damage_group(
        &mut g,
        EventGroup::Ordered {
            children: vec![leaf(movement.clone()), leaf(damage.clone())],
        },
    )
    .unwrap();
    assert!(
        matches!(receipt.result(&[1]), Some(EventOutcome::DamageCommitted { actual, .. })
        if actual[0].source.object == source && actual[0].amount == 1.into())
    );
    assert_eq!(g.state.rules.players[1].life, 39.into());

    let mut g = initial.fork();
    prepared_member(&mut g).amount = 0.into();
    assert_damage_rejected(
        &mut g,
        EventGroup::Ordered {
            children: vec![
                leaf(ProposedEvent::GainLife {
                    player: PlayerId(0),
                    amount: 4.into(),
                }),
                leaf(damage),
            ],
        },
    );

    let wrong_transaction = DamageTransactionId(initial.damage_transaction().unwrap().id.0 + 1);
    assert_damage_rejected(
        &mut initial,
        EventGroup::Ordered {
            children: vec![
                leaf(movement),
                leaf(ProposedEvent::CommitDamage {
                    transaction: wrong_transaction,
                }),
            ],
        },
    );
}
