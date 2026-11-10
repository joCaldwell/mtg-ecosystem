use super::*;

fn catalog() -> Catalog {
    Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-effects-ir.json"
    ))
    .unwrap()
}
fn game(config: ScenarioConfig) -> Game {
    Game::new(Prepared::admit(catalog(), config).unwrap())
}
fn object(g: &Game, id: u32) -> ObjectId {
    g.object_id(CardId(id)).unwrap()
}
fn origin(g: &mut Game, source: u32) -> EffectOrigin {
    let source = object(g, source);
    EffectOrigin {
        mode: None,
        stack_id: StackId(0),
        source,
        controller: g.current(source).unwrap().controller,
        source_handles: g.capture_trigger_source_handles(source).unwrap(),
        cursor: 0,
        source_block: 0,
    }
}
fn seed(g: &mut Game, source: u32, kind: ResolvedEffectKind) -> EffectId {
    let id = EffectId(g.state.rules.next_effect_id);
    let origin = origin(g, source);
    let target_handles = match &kind {
        ResolvedEffectKind::ModifyPowerToughness { target, .. } => {
            g.capture_trigger_source_handles(*target).unwrap()
        }
        ResolvedEffectKind::PreventNextDamage {
            target: TargetEntity::Object { object },
            ..
        } => g.capture_trigger_source_handles(*object).unwrap(),
        _ => vec![],
    };
    g.state.rules.next_effect_id += 1;
    g.state.rules.effects.push(ResolvedEffect {
        target_handles,
        id,
        origin,
        expires_in_cleanup_of_turn: g.state.rules.turn.number,
        kind,
    });
    id
}
fn damage(g: &Game, source: u32, recipient: DamageRecipient, amount: i64) -> DamageOccurrence {
    let source = object(g, source);
    DamageOccurrence {
        source,
        controller: g.current(source).unwrap().controller,
        recipient,
        amount: amount.into(),
    }
}
fn apply(g: &mut Game, effect: EffectKey, damage: DamageId) -> EventOutcome {
    let tx = g.damage_transaction().unwrap().id;
    g.commit(
        ProposedEvent::ApplyDamageEffect {
            transaction: tx,
            effect,
            damage,
        },
        BatchCause::Fixture,
    )
    .unwrap()
}

#[test]
fn effects_admission_is_exact_and_retains_historical_catalog_closure() {
    for config in [
        ScenarioConfig::effects(),
        ScenarioConfig::effects_furnace(),
        ScenarioConfig::effects_two_furnaces(),
        ScenarioConfig::effects_borrowed(),
        ScenarioConfig::effects_low(),
    ] {
        assert_eq!(config.cards.len(), 46);
        Prepared::admit(catalog(), config.clone()).unwrap();
        for seat in 0..4 {
            assert!(
                config
                    .cards
                    .iter()
                    .filter(|c| c.owner == PlayerId(seat))
                    .count()
                    <= 16
            );
        }
        let mut changed = config.clone();
        changed.cards[1].oracle_id = BEARS.into();
        assert!(Prepared::admit(catalog(), changed).is_err());
        for version in 3..=8 {
            let mut c = catalog();
            c.schema_version = version;
            assert!(Prepared::admit(c, config.clone()).is_err());
        }
    }
    assert!(Prepared::admit(catalog(), ScenarioConfig::triggers()).is_err());
    let mut wrong = catalog();
    wrong
        .definitions
        .iter_mut()
        .find(|d| d.oracle_id == GROWTH)
        .unwrap()
        .name
        .push('x');
    assert!(Prepared::admit(wrong, ScenarioConfig::effects()).is_err());
}
#[test]
fn signed_modifiers_are_additive_fixed_incarnation_and_cleanup_is_atomic() {
    let mut g = game(ScenarioConfig::effects());
    let ox = object(&g, 1);
    seed(
        &mut g,
        7,
        ResolvedEffectKind::ModifyPowerToughness {
            target: ox,
            power: (-2).into(),
            toughness: (-2).into(),
        },
    );
    assert_eq!(g.characteristics(ox).unwrap().power, Some((-2).into()));
    assert_eq!(g.characteristics(ox).unwrap().toughness, Some(2.into()));
    let pinger = object(&g, 3);
    seed(
        &mut g,
        5,
        ResolvedEffectKind::ModifyPowerToughness {
            target: pinger,
            power: 3.into(),
            toughness: 3.into(),
        },
    );
    g.state
        .rules
        .objects
        .get_mut(&CardId(3))
        .unwrap()
        .marked_damage = 3.into();
    let before = g.state.clone();
    let batch = g
        .commit_batch(
            BatchCause::Cleanup,
            EventGroup::Occurrence {
                proposed: ProposedEvent::CleanupEffects,
            },
        )
        .unwrap();
    assert_eq!(batch.occurrences.len(), 1);
    assert!(
        matches!(&batch.occurrences[0].actual,EventOutcome::EffectsCleaned{expired,previous} if expired.len()==2&&previous==&vec![(pinger,3.into())])
    );
    assert_eq!(g.characteristics(pinger).unwrap().toughness, Some(1.into()));
    assert_eq!(g.current(pinger).unwrap().marked_damage, 0.into());
    assert!(before.rules.objects.contains_key(&CardId(3)));
    assert!(g.current(pinger).is_ok());
    // A modifier never follows the physical card to a successor incarnation.
    seed(
        &mut g,
        5,
        ResolvedEffectKind::ModifyPowerToughness {
            target: ox,
            power: 3.into(),
            toughness: 3.into(),
        },
    );
    let hand = g
        .move_object_for(ox, Zone::Hand { owner: PlayerId(0) }, BatchCause::Fixture)
        .unwrap()
        .successor;
    let next = g
        .move_object_for(hand, Zone::Battlefield, BatchCause::Fixture)
        .unwrap()
        .successor;
    assert_eq!(g.characteristics(next).unwrap().power, Some(0.into()));
}
#[test]
fn applied_prefix_and_finite_shared_allocation_are_not_vector_order() {
    let mut g = game(ScenarioConfig::effects_furnace());
    let shield = seed(
        &mut g,
        2,
        ResolvedEffectKind::PreventNextDamage {
            target: TargetEntity::Player {
                player: PlayerId(1),
            },
            remaining: 1.into(),
        },
    );
    let ds = vec![
        damage(
            &g,
            0,
            DamageRecipient::Player {
                player: PlayerId(1),
            },
            2,
        ),
        damage(
            &g,
            16,
            DamageRecipient::Player {
                player: PlayerId(1),
            },
            2,
        ),
    ];
    g.prepare_combat_damage(ds).unwrap();
    g.refresh_visibility().unwrap();
    let Request::DamageEffect {
        player, candidates, ..
    } = g.damage_effect_request().unwrap().unwrap()
    else {
        panic!()
    };
    assert_eq!(player, PlayerId(1));
    assert_eq!(candidates.len(), 4);
    let shield_key = EffectKey::Resolved { id: shield };
    apply(&mut g, shield_key.clone(), DamageId(1));
    assert_eq!(g.state.rules.players[1].life, 40.into());
    assert_eq!(g.damage_transaction().unwrap().entries[0].amount, 2.into());
    assert_eq!(g.damage_transaction().unwrap().entries[1].amount, 1.into());
    let before = g.state.clone();
    let tx = g.damage_transaction().unwrap().id;
    assert!(
        g.commit(
            ProposedEvent::ApplyDamageEffect {
                transaction: tx,
                effect: shield_key,
                damage: DamageId(0)
            },
            BatchCause::Fixture
        )
        .is_err()
    );
    assert_eq!(g.state, before);
    let furnace = EffectKey::Static {
        source: object(&g, 32),
        ability: 0,
    };
    apply(&mut g, furnace.clone(), DamageId(0));
    apply(&mut g, furnace, DamageId(1));
    let outcome = g
        .commit(
            ProposedEvent::CommitDamage { transaction: tx },
            BatchCause::Fixture,
        )
        .unwrap();
    assert!(
        matches!(outcome,EventOutcome::DamageCommitted{actual,results,..} if actual.iter().map(|d|d.amount.to_string()).collect::<Vec<_>>()==["4","2"]&&results[0].current==34.into())
    );
    assert_eq!(
        g.state
            .rules
            .commanders
            .history(CardId(0))
            .unwrap()
            .combat_damage[&PlayerId(1)],
        4.into()
    );
}
#[test]
fn forged_damage_ledger_and_late_failure_leave_every_write_unchanged() {
    let mut g = game(ScenarioConfig::effects_two_furnaces());
    let ds = vec![
        damage(
            &g,
            0,
            DamageRecipient::Creature {
                object: object(&g, 16),
            },
            2,
        ),
        damage(
            &g,
            38,
            DamageRecipient::Player {
                player: PlayerId(0),
            },
            2,
        ),
    ];
    g.prepare_combat_damage(ds).unwrap();
    let baseline = g.state.clone();
    for mutation in 0..7 {
        g.state = baseline.clone();
        let Frame::DamageEffects { transaction: tx } = &mut g.state.rules.frame else {
            panic!()
        };
        match mutation {
            0 => tx.entries[1].source.owner = PlayerId(3),
            1 => tx.entries[1].source.object.card = CardId(999),
            2 => tx.entries[1].source.default_controller = PlayerId(255),
            3 => tx.entries[1].original_amount = 3.into(),
            4 => tx.entries[1].amount = 0.into(),
            5 => tx.entries[1].source.characteristics.legendary = true,
            _ => tx.entries[1].applied.push(EffectKey::Static {
                source: ObjectId {
                    card: CardId(31),
                    incarnation: 0,
                },
                ability: 0,
            }),
        }
        let before = g.state.clone();
        let id = g.damage_transaction().unwrap().id;
        assert!(
            g.commit(
                ProposedEvent::ApplyDamageEffect {
                    transaction: id,
                    damage: DamageId(0),
                    effect: EffectKey::Static {
                        source: object(&g, 32),
                        ability: 0
                    }
                },
                BatchCause::Fixture
            )
            .is_err(),
            "mutant {mutation}"
        );
        assert_eq!(g.state, before);
    }
    g.state = baseline;
    let tx = g.damage_transaction().unwrap().id;
    let before = g.state.clone();
    // Ordered-group rollback includes an earlier valid semantic application.
    assert!(
        g.commit_batch(
            BatchCause::Fixture,
            EventGroup::Ordered {
                children: vec![
                    EventGroup::Occurrence {
                        proposed: ProposedEvent::ApplyDamageEffect {
                            transaction: tx,
                            damage: DamageId(0),
                            effect: EffectKey::Static {
                                source: object(&g, 32),
                                ability: 0
                            }
                        }
                    },
                    EventGroup::Occurrence {
                        proposed: ProposedEvent::CommitDamage { transaction: tx }
                    }
                ]
            }
        )
        .is_err()
    );
    assert_eq!(g.state, before);
}
#[test]
fn fog_and_shield_orders_preserve_distinct_budgets_and_no_actual_zero_damage() {
    let mut g = game(ScenarioConfig::effects());
    let fog = seed(&mut g, 6, ResolvedEffectKind::PreventCombatDamage);
    let shield = seed(
        &mut g,
        2,
        ResolvedEffectKind::PreventNextDamage {
            target: TargetEntity::Player {
                player: PlayerId(1),
            },
            remaining: 1.into(),
        },
    );
    let ds = vec![damage(
        &g,
        0,
        DamageRecipient::Player {
            player: PlayerId(1),
        },
        2,
    )];
    g.prepare_combat_damage(ds).unwrap();
    let mut other = g.fork();
    apply(&mut g, EffectKey::Resolved { id: fog }, DamageId(0));
    let id = g.damage_transaction().unwrap().id;
    let out = g
        .commit(
            ProposedEvent::CommitDamage { transaction: id },
            BatchCause::Fixture,
        )
        .unwrap();
    assert!(
        matches!(out,EventOutcome::DamageCommitted{actual,commander_damage,..} if actual.is_empty()&&commander_damage.is_empty())
    );
    assert!(
        matches!(&g.state.rules.effects.iter().find(|e|e.id==shield).unwrap().kind,ResolvedEffectKind::PreventNextDamage{remaining,..} if *remaining==1.into())
    );
    apply(&mut other, EffectKey::Resolved { id: shield }, DamageId(0));
    apply(&mut other, EffectKey::Resolved { id: fog }, DamageId(0));
    assert!(
        matches!(&other.state.rules.effects.iter().find(|e|e.id==shield).unwrap().kind,ResolvedEffectKind::PreventNextDamage{remaining,..} if *remaining==0.into())
    );
    // Life loss is not damage and bypasses every damage modifier/prevention.
    g.commit(
        ProposedEvent::LoseLife {
            player: PlayerId(1),
            amount: 4.into(),
        },
        BatchCause::Fixture,
    )
    .unwrap();
    assert_eq!(g.state.rules.players[1].life, 36.into());
}
#[test]
fn repeated_cleanup_expires_effects_created_in_cleanup_priority_exception() {
    let mut g = game(ScenarioConfig::effects());
    g.state.rules.turn.step = TurnStep::Cleanup;
    g.state.rules.cleanup_repeat = true;
    let pinger = object(&g, 3);
    seed(
        &mut g,
        5,
        ResolvedEffectKind::ModifyPowerToughness {
            target: pinger,
            power: 3.into(),
            toughness: 3.into(),
        },
    );
    g.state
        .rules
        .objects
        .get_mut(&CardId(3))
        .unwrap()
        .marked_damage = 3.into();
    // Domain fixture: the exact tables cannot naturally reach cleanup priority,
    // but 514.3a requires another cleanup after effects created in that window.
    g.state.rules.frame = Frame::CleanupClear;
    g.combat_step().unwrap();
    assert!(g.state.rules.effects.is_empty());
    assert_eq!(g.current(pinger).unwrap().marked_damage, 0.into());
    for _ in 0..20 {
        if !matches!(g.state.rules.frame, Frame::Stabilize { .. }) {
            break;
        }
        g.combat_step().unwrap();
    }
    assert!(g.current(pinger).is_ok());
    assert_eq!(g.characteristics(pinger).unwrap().toughness, Some(1.into()));
    assert_eq!(g.state.rules.turn.number, 1);
}
