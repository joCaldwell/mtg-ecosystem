//! Private source-backed configurations. No pool or consumer admission.
//! CR 702.12a-c,701.8a-b,704.5f-h,701.21a,120.3,514.2,614.17c:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
fn prepared(rows: &[(&str, bool)]) -> Prepared {
    let mut c = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap();
    for raw in [
        include_str!("../../../../tests/fixtures/f90fe86c-5cca-483b-93c2-6e856fa01c88.json"),
        include_str!("../../../../tests/fixtures/2bda1fb1-528c-4e66-89f1-bb1a85e9181e.json"),
    ] {
        let mtg_compiler::CompileResult::Lowered { definition } = mtg_compiler::compile(
            include_str!("../../../../tests/fixtures/archive-manifest-current.json"),
            raw,
        ) else {
            panic!("source")
        };
        c.definitions.push(*definition);
    }
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    let mut config = ScenarioConfig::turns_permanents();
    config.initial.turn_step = TurnStep::PrecombatMain;
    config.profile = "intrinsic-indestructible-private".into();
    config.commanders.clear();
    config.cards = rows
        .iter()
        .enumerate()
        .map(|(i, (name, hand))| CardSetup {
            id: CardId(i as u32),
            oracle_id: c
                .definitions
                .iter()
                .find(|d| d.name == *name)
                .unwrap()
                .oracle_id
                .clone(),
            owner: PlayerId(0),
            controller: PlayerId(0),
            zone: if *hand {
                Zone::Hand { owner: PlayerId(0) }
            } else {
                Zone::Battlefield
            },
        })
        .collect();
    for seat in 0..4 {
        for _ in 0..6 {
            config.cards.push(CardSetup {
                id: CardId(config.cards.len() as u32),
                oracle_id: FOREST.into(),
                owner: PlayerId(seat),
                controller: PlayerId(seat),
                zone: Zone::Library {
                    owner: PlayerId(seat),
                },
            });
        }
    }
    Prepared {
        catalog_digest: digest(&c).unwrap(),
        profile_digest: digest(&config).unwrap(),
        catalog: Arc::new(c),
        config,
        match_config: None,
    }
}
fn object(g: &Game, id: u32) -> ObjectId {
    g.object_id(CardId(id)).unwrap()
}
fn request(g: &mut Game, q: u64) -> Request {
    for _ in 0..1000 {
        match g.advance(Budget::new(q)) {
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(r) => return r,
            x => panic!("{x:?}"),
        }
    }
    panic!("bounded stabilization")
}
fn answer(g: &mut Game, q: u64, response: Response) {
    let r = request(g, q);
    g.respond(r.player(), r.decision_id(), response).unwrap();
}
fn recover(g: &Game) {
    let p = g.prepared.as_ref().clone();
    let restored = Game::restore(p.clone(), &g.snapshot().unwrap()).unwrap();
    assert_eq!(g.state, restored.state);
    assert_eq!(g.trace(), restored.trace());
    let replay = Game::replay_to(
        p,
        &g.replay_journal(),
        Some(g.rule_boundary()),
        g.trace.mode,
    )
    .unwrap();
    assert_eq!(g.state, replay.state);
    assert_eq!(g.trace(), replay.trace());
}
fn both(p: Prepared, run: impl Fn(&mut Game, u64)) {
    let mut prior = None;
    for (q, mode) in [(1, TraceMode::Detailed), (4096, TraceMode::Off)] {
        let mut g = Game::with_trace(p.clone(), mode);
        request(&mut g, q);
        run(&mut g, q);
        recover(&g);
        let hash = g.state_digest().unwrap();
        if let Some(old) = &prior {
            assert_eq!(&hash, old)
        }
        prior = Some(hash);
    }
}
fn cast(g: &mut Game, q: u64, id: u32, targets: Vec<TargetEntity>) {
    let obj = object(g, id);
    answer(g, q, Response::Cast { card: obj.card });
    loop {
        let r = request(g, q);
        let response = match &r {
            Request::EntityTargets { .. } => Response::EntityTargets {
                entities: targets.clone(),
            },
            Request::Targets { .. } => Response::Targets {
                objects: targets
                    .iter()
                    .map(|t| match t {
                        TargetEntity::Object { object } => *object,
                        _ => panic!("object target"),
                    })
                    .collect(),
            },
            Request::Mana { cost, .. } => {
                let enough = g.payment_pool(r.player(), cost).is_ok();
                if enough {
                    Response::FinishMana { undo_mana: false }
                } else {
                    let land = g
                        .state
                        .rules
                        .objects
                        .values()
                        .find(|o| {
                            o.controller == r.player()
                                && o.zone == Zone::Battlefield
                                && !o.tapped
                                && g.definition(o.id.card).unwrap().characteristics.kind
                                    == CardKind::Land
                        })
                        .unwrap();
                    Response::ActivateMana { object: land.id }
                }
            }
            Request::SpendMana {
                cost, available, ..
            } => {
                let mut rest = available.clone();
                let mut spending = BTreeMap::<Color, Quantity>::new();
                for c in &cost.colored {
                    rest.get_mut(c).unwrap().subtract(&1.into());
                    spending.entry(*c).or_default().add(&1.into());
                }
                let mut generic = cost.generic.clone();
                for (c, n) in rest {
                    let used = n.min(generic.clone());
                    generic.subtract(&used);
                    if used > Quantity::zero() {
                        spending.entry(c).or_default().add(&used);
                    }
                }
                assert_eq!(generic, Quantity::zero());
                Response::SpendMana { spending }
            }
            Request::Priority { .. } => break,
            x => panic!("cast request {x:?}"),
        };
        g.respond(r.player(), r.decision_id(), response).unwrap();
    }
    while !g.state.rules.stack.is_empty() {
        answer(g, q, Response::Pass);
        request(g, q);
    }
}

#[test]
fn indestructible_paid_myr_and_zero_relic_damage_cleanup_and_replay() {
    both(
        prepared(&[
            ("Darksteel Myr", true),
            ("Darksteel Relic", true),
            ("Lightning Bolt", true),
            ("Mountain", false),
            ("Mountain", false),
            ("Mountain", false),
            ("Mountain", false),
        ]),
        |g, q| {
            let r = request(g, q);
            let before = g.snapshot().unwrap();
            assert!(
                g.respond(
                    r.player(),
                    r.decision_id(),
                    Response::Cast { card: CardId(999) }
                )
                .is_err()
            );
            assert_eq!(before, g.snapshot().unwrap());
            cast(g, q, 0, vec![]);
            cast(g, q, 1, vec![]);
            let myr = object(g, 0);
            let relic = object(g, 1);
            assert_eq!(g.current(myr).unwrap().zone, Zone::Battlefield);
            assert_eq!(g.current(relic).unwrap().zone, Zone::Battlefield);
            assert_eq!(
                g.state.rules.objects.values().filter(|o| o.tapped).count(),
                3
            );
            let QueryResult::Complete(QueryValue::Observation(view)) =
                g.observe(PlayerId(0), Budget::unlimited())
            else {
                panic!("observation");
            };
            let view = serde_json::to_value(view).unwrap();
            assert!(view.to_string().matches("indestructible").count() >= 2);
            for permanent in [myr, relic] {
                assert_eq!(
                    g.visible_object(PlayerId(0), g.current(permanent).unwrap())
                        .unwrap()
                        .unwrap()
                        .keywords,
                    [KeywordAbility::Indestructible]
                );
            }
            cast(g, q, 2, vec![TargetEntity::Object { object: myr }]);
            assert_eq!(g.current(myr).unwrap().marked_damage, 3.into());
            recover(g);
            for _ in 0..200 {
                if g.current(myr).unwrap().marked_damage == Quantity::zero() {
                    break;
                }
                let r = request(g, q);
                let a = match r {
                    Request::Priority { .. } => Response::Pass,
                    Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
                    Request::Blockers { .. } => Response::Blockers { blocks: vec![] },
                    x => panic!("{x:?}"),
                };
                answer(g, q, a);
            }
            assert_eq!(g.current(myr).unwrap().marked_damage, Quantity::zero());
        },
    );
}
#[test]
fn indestructible_targeted_destruction_retains_both_kinds_and_destroys_control() {
    both(
        prepared(&[
            ("Darksteel Myr", false),
            ("Darksteel Relic", false),
            ("Grizzly Bears", false),
            ("Murder", true),
            ("Shatter", true),
            ("Murder", true),
            ("Swamp", false),
            ("Swamp", false),
            ("Swamp", false),
            ("Mountain", false),
            ("Mountain", false),
            ("Swamp", false),
            ("Swamp", false),
            ("Swamp", false),
        ]),
        |g, q| {
            for (spell, target) in [(3, 0), (4, 1), (5, 2)] {
                let o = object(g, target);
                cast(g, q, spell, vec![TargetEntity::Object { object: o }]);
            }
            assert_eq!(g.current(object(g, 0)).unwrap().zone, Zone::Battlefield);
            assert_eq!(g.current(object(g, 1)).unwrap().zone, Zone::Battlefield);
            assert!(matches!(
                g.current(object(g, 2)).unwrap().zone,
                Zone::Graveyard { .. }
            ));
            if g.trace.mode == TraceMode::Detailed {
                assert_eq!(
                    g.events()
                        .iter()
                        .filter(|e| matches!(
                            e,
                            Event::Committed {
                                actual: EventOutcome::DestructionPrevented { .. },
                                ..
                            }
                        ))
                        .count(),
                    2
                );
            }
        },
    );
}
#[test]
fn indestructible_fumigate_counts_only_actual_destroyed_creatures() {
    both(
        prepared(&[
            ("Darksteel Myr", false),
            ("Darksteel Relic", false),
            ("Grizzly Bears", false),
            ("Fumigate", true),
            ("Plains", false),
            ("Plains", false),
            ("Plains", false),
            ("Plains", false),
            ("Plains", false),
        ]),
        |g, q| {
            let life = g.state.rules.players[0].life.clone();
            cast(g, q, 3, vec![]);
            let mut expected = life;
            expected.add(&1.into());
            assert_eq!(g.state.rules.players[0].life, expected);
            assert_eq!(g.current(object(g, 0)).unwrap().zone, Zone::Battlefield);
            assert_eq!(g.current(object(g, 1)).unwrap().zone, Zone::Battlefield);
            assert!(matches!(
                g.current(object(g, 2)).unwrap().zone,
                Zone::Graveyard { .. }
            ));
        },
    );
}
#[test]
fn indestructible_anger_failed_destroy_then_sacrifice_exiles_and_zero_toughness_dies() {
    both(
        prepared(&[
            ("Darksteel Myr", false),
            ("Anger of the Gods", true),
            ("Murder", true),
            ("Innocent Blood", true),
            ("Mountain", false),
            ("Mountain", false),
            ("Mountain", false),
            ("Swamp", false),
            ("Swamp", false),
            ("Swamp", false),
            ("Swamp", false),
        ]),
        |g, q| {
            cast(g, q, 1, vec![]);
            let myr = object(g, 0);
            assert_eq!(g.current(myr).unwrap().marked_damage, 3.into());
            cast(g, q, 2, vec![TargetEntity::Object { object: myr }]);
            assert_eq!(g.current(myr).unwrap().zone, Zone::Battlefield);
            // Complete sacrifice invocation answers the real forced-sacrifice choice.
            answer(g, q, Response::Cast { card: CardId(3) });
            loop {
                let r = request(g, q);
                let a = match r {
                    Request::Mana { .. } => {
                        let land = g
                            .state
                            .rules
                            .objects
                            .values()
                            .find(|o| {
                                o.zone == Zone::Battlefield
                                    && !o.tapped
                                    && g.definition(o.id.card).unwrap().name == "Swamp"
                            })
                            .unwrap()
                            .id;
                        Response::ActivateMana { object: land }
                    }
                    Request::SpendMana { .. } => Response::SpendMana {
                        spending: BTreeMap::from([(Color::B, 1.into())]),
                    },
                    Request::Priority { .. } => break,
                    x => panic!("{x:?}"),
                };
                answer(g, q, a);
                if matches!(request(g, q), Request::Mana { .. }) {
                    answer(g, q, Response::FinishMana { undo_mana: false });
                }
            }
            while !g.state.rules.stack.is_empty() {
                let r = request(g, q);
                match r {
                    Request::Priority { .. } => answer(g, q, Response::Pass),
                    Request::ForcedSacrifice { .. } => {
                        answer(g, q, Response::ForcedSacrifice { object: myr })
                    }
                    x => panic!("{x:?}"),
                };
            }
            request(g, q);
            assert_eq!(g.current(object(g, 0)).unwrap().zone, Zone::Exile);
        },
    );
    both(
        prepared(&[
            ("Darksteel Myr", false),
            ("Disfigure", true),
            ("Swamp", false),
        ]),
        |g, q| {
            let myr = object(g, 0);
            cast(g, q, 1, vec![TargetEntity::Object { object: myr }]);
            assert!(matches!(
                g.current(object(g, 0)).unwrap().zone,
                Zone::Graveyard { .. }
            ));
        },
    );
    both(
        prepared(&[
            ("Darksteel Myr", false),
            ("Anger of the Gods", true),
            ("Disfigure", true),
            ("Mountain", false),
            ("Mountain", false),
            ("Mountain", false),
            ("Swamp", false),
        ]),
        |g, q| {
            cast(g, q, 1, vec![]);
            let myr = object(g, 0);
            cast(g, q, 2, vec![TargetEntity::Object { object: myr }]);
            assert_eq!(g.current(object(g, 0)).unwrap().zone, Zone::Exile);
        },
    );
}
#[test]
fn indestructible_exile_and_bounce_still_move_exact_incarnations() {
    for spell in ["Unsummon", "Swords to Plowshares"] {
        both(
            prepared(&[
                ("Darksteel Myr", false),
                (spell, true),
                (
                    if spell == "Unsummon" {
                        "Island"
                    } else {
                        "Plains"
                    },
                    false,
                ),
            ]),
            |g, q| {
                let myr = object(g, 0);
                cast(g, q, 1, vec![TargetEntity::Object { object: myr }]);
                assert!(g.current(myr).is_err());
                assert_eq!(
                    g.current(object(g, 0)).unwrap().zone,
                    if spell == "Unsummon" {
                        Zone::Hand { owner: PlayerId(0) }
                    } else {
                        Zone::Exile
                    }
                );
            },
        );
    }
}
#[test]
fn indestructible_typed_nonlethal_deathtouch_lifelink_and_atomic_fault_control() {
    // Typed event invariant control, not an invented card: Giant Growth is cast
    // normally, then an exact Vampire Nighthawk damage leaf exercises deathtouch.
    let p = prepared(&[
        ("Darksteel Myr", false),
        ("Vampire Nighthawk", false),
        ("Giant Growth", true),
        ("Forest", false),
    ]);
    let mut g = Game::with_trace(p, TraceMode::Detailed);
    request(&mut g, 1);
    let myr = object(&g, 0);
    cast(&mut g, 1, 2, vec![TargetEntity::Object { object: myr }]);
    let source = object(&g, 1);
    let life = g.state.rules.players[0].life.clone();
    g.commit(
        ProposedEvent::CombatDamage {
            damage: vec![DamageOccurrence {
                source,
                controller: PlayerId(0),
                recipient: DamageRecipient::Creature { object: myr },
                amount: 1.into(),
            }],
        },
        BatchCause::StateBased,
    )
    .unwrap();
    assert_eq!(g.current(myr).unwrap().marked_damage, 1.into());
    assert!(g.current(myr).unwrap().deathtouch_damage);
    let mut expected = life;
    expected.add(&1.into());
    assert_eq!(g.state.rules.players[0].life, expected);
    g.stabilization_wave_with_legends(Frame::Priority, false, false, None)
        .unwrap();
    assert!(matches!(g.state.rules.frame, Frame::Priority));
    assert_eq!(g.current(myr).unwrap().zone, Zone::Battlefield);
    let before = g.state.clone();
    let trace = g.trace().clone();
    let bad = ObjectId {
        card: myr.card,
        incarnation: myr.incarnation + 1,
    };
    assert!(
        g.commit_batch(
            BatchCause::StateBased,
            EventGroup::Ordered {
                children: vec![
                    EventGroup::Occurrence {
                        proposed: ProposedEvent::DestroyCreature { object: myr }
                    },
                    EventGroup::Occurrence {
                        proposed: ProposedEvent::DestroyCreature {
                            object: object(&g, 3)
                        }
                    }
                ]
            }
        )
        .is_err()
    );
    assert_eq!(g.state, before);
    assert_eq!(g.trace(), &trace);
    assert!(
        g.commit(
            ProposedEvent::DestroyPermanent { object: bad },
            BatchCause::StateBased
        )
        .is_err()
    );
    assert!(
        g.commit(
            ProposedEvent::StateBasedGraveyard {
                object: myr,
                reason: CreatureLossReason::Deathtouch
            },
            BatchCause::StateBased
        )
        .is_err()
    );
    g.commit(ProposedEvent::ClearDamage, BatchCause::StateBased)
        .unwrap();
    assert_eq!(g.current(myr).unwrap().marked_damage, Quantity::zero());
    assert!(!g.current(myr).unwrap().deathtouch_damage);
}
