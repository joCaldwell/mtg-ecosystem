//! Private source-backed configurations. No pool or consumer admission.
//! CR 702.12a-c,701.8a-b,704.5f-h,701.21a,120.3,514.2,614.17c:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
pub(super) fn prepared(rows: &[(&str, bool)], synthetic: bool) -> Prepared {
    let mut c = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap();
    for raw in [
        include_str!("../../../../tests/fixtures/9d8fe3f8-5027-4f9e-999e-e277caa96478.json"),
        include_str!("../../../../tests/fixtures/b5b4cf54-ed5e-42d0-9d98-5fec76b0b0b8.json"),
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
    if synthetic {
        // Typed nonintrinsic equipment control, NOT a real card source claim.
        let d = c
            .definitions
            .iter_mut()
            .find(|d| d.name == "Darksteel Plate")
            .unwrap();
        d.name = "Typed nonintrinsic equipment grant control".into();
        d.keywords.clear();
        d.capabilities = d.requirements();
    }
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    let mut config = ScenarioConfig::turns_permanents();
    config.initial.turn_step = TurnStep::PrecombatMain;
    config.profile = "equipped-indestructible-private".into();
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
pub(super) fn object(g: &Game, id: u32) -> ObjectId {
    g.object_id(CardId(id)).unwrap()
}
pub(super) fn request(g: &mut Game, q: u64) -> Request {
    for _ in 0..1000 {
        match g.advance(Budget::new(q)) {
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(r) => return r,
            x => panic!("{x:?}"),
        }
    }
    panic!("bounded stabilization")
}
pub(super) fn answer(g: &mut Game, q: u64, response: Response) {
    let r = request(g, q);
    g.respond(r.player(), r.decision_id(), response).unwrap();
}
pub(super) fn recover(g: &Game) {
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
pub(super) fn both(p: Prepared, run: impl Fn(&mut Game, u64)) {
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
pub(super) fn cast(g: &mut Game, q: u64, id: u32, targets: Vec<TargetEntity>) {
    let obj = object(g, id);
    answer(g, q, Response::Cast { card: obj.card });
    pay_and_resolve(g, q, targets);
}
fn pay_and_resolve(g: &mut Game, q: u64, targets: Vec<TargetEntity>) {
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

pub(super) fn equip(g: &mut Game, q: u64, source: u32, target: u32) {
    let source = object(g, source);
    let target = object(g, target);
    answer(
        g,
        q,
        Response::ActivateAbility {
            object: source,
            ability: 0,
        },
    );
    pay_and_resolve(g, q, vec![TargetEntity::Object { object: target }]);
    assert_eq!(g.current(source).unwrap().attached_to, Some(target));
}
fn protected(g: &Game, id: u32) -> bool {
    g.has_keyword(object(g, id), KeywordAbility::Indestructible)
}
#[test]
fn equipped_indestructible_paid_plate_reequip_lethal_loss_public_and_replay() {
    both(
        prepared(
            &[
                ("Darksteel Plate", true),
                ("Grizzly Bears", false),
                ("Grizzly Bears", false),
                ("Lightning Bolt", true),
                ("Shatter", true),
                ("Mountain", false),
                ("Mountain", false),
                ("Mountain", false),
                ("Mountain", false),
                ("Mountain", false),
                ("Mountain", false),
                ("Mountain", false),
                ("Mountain", false),
                ("Mountain", false),
                ("Mountain", false),
            ],
            false,
        ),
        |g, q| {
            cast(g, q, 0, vec![]);
            assert_eq!(
                g.state.rules.objects.values().filter(|o| o.tapped).count(),
                3
            );
            equip(g, q, 0, 1);
            assert_eq!(
                g.state.rules.objects.values().filter(|o| o.tapped).count(),
                5
            );
            assert!(protected(g, 1));
            assert!(!protected(g, 2));
            let QueryResult::Complete(QueryValue::Observation(view)) =
                g.observe(PlayerId(0), Budget::unlimited())
            else {
                panic!("public observation")
            };
            let bear_id = &g.definition(CardId(1)).unwrap().oracle_id;
            assert_eq!(
                view.view
                    .objects
                    .iter()
                    .filter(|o| &o.oracle_id == bear_id
                        && o.keywords.contains(&KeywordAbility::Indestructible))
                    .count(),
                1
            );

            assert!(
                g.visible_object(PlayerId(0), g.current(object(g, 1)).unwrap())
                    .unwrap()
                    .unwrap()
                    .keywords
                    .contains(&KeywordAbility::Indestructible)
            );
            cast(
                g,
                q,
                3,
                vec![TargetEntity::Object {
                    object: object(g, 1),
                }],
            );
            assert_eq!(g.current(object(g, 1)).unwrap().marked_damage, 3.into());
            recover(g);
            equip(g, q, 0, 2);
            assert!(matches!(
                g.current(object(g, 1)).unwrap().zone,
                Zone::Graveyard { .. }
            ));
            assert!(protected(g, 2));
            cast(
                g,
                q,
                4,
                vec![TargetEntity::Object {
                    object: object(g, 0),
                }],
            );
            assert_eq!(g.current(object(g, 0)).unwrap().zone, Zone::Battlefield);
            assert!(protected(g, 2));
        },
    );
}
#[test]
fn equipped_indestructible_paid_cleansing_common_prior_in_both_physical_orders() {
    for source_first in [false, true] {
        let (source, target, rows) = if source_first {
            (
                0,
                1,
                vec![
                    ("Typed nonintrinsic equipment grant control", false),
                    ("Grizzly Bears", false),
                ],
            )
        } else {
            (
                1,
                0,
                vec![
                    ("Grizzly Bears", false),
                    ("Typed nonintrinsic equipment grant control", false),
                ],
            )
        };
        let mut rows = rows;
        rows.push(("Planar Cleansing", true));
        rows.extend(vec![("Plains", false); 8]);
        both(prepared(&rows, true), |g, q| {
            equip(g, q, source, target);
            assert!(protected(g, target));
            cast(g, q, 2, vec![]);
            assert!(matches!(
                g.current(object(g, source)).unwrap().zone,
                Zone::Graveyard { .. }
            ));
            assert_eq!(
                g.current(object(g, target)).unwrap().zone,
                Zone::Battlefield,
                "source_first={source_first}"
            );
            assert!(!protected(g, target));
            if g.trace.mode == TraceMode::Detailed {
                assert!(g.events().iter().any(|e|matches!(e,Event::Committed {actual:EventOutcome::DestructionPrevented {object:o},..} if *o==object(g,target))));
                for missing in [false, true] {
                    let mut snapshot: Snapshot = strict_read(&g.snapshot().unwrap()).unwrap();
                    let index = snapshot.trace.records.iter().position(|r| matches!(&r.record,
                        EvidenceRecord::Batch { batch } if batch.occurrences.iter().any(|o| matches!(o.actual, EventOutcome::DestructionPrevented {..})))).unwrap();
                    let EvidenceRecord::Batch { batch } = &mut snapshot.trace.records[index].record
                    else {
                        unreachable!()
                    };
                    let occurrence = batch
                        .occurrences
                        .iter_mut()
                        .find(|o| matches!(o.actual, EventOutcome::DestructionPrevented { .. }))
                        .unwrap();
                    if missing {
                        occurrence.before.objects.clear();
                    } else {
                        occurrence.before.objects[0].keywords.clear();
                    }
                    assert!(
                        Game::restore(g.prepared.as_ref().clone(), &canonical(&snapshot).unwrap())
                            .is_err()
                    );
                }
            }
        });
    }
}
#[test]
fn equipped_indestructible_equip_timing_foreign_and_stale_responses_are_atomic() {
    let mut p = prepared(
        &[
            ("Darksteel Plate", false),
            ("Grizzly Bears", false),
            ("Grizzly Bears", false),
            ("Disperse", true),
            ("Island", false),
            ("Island", false),
            ("Island", false),
            ("Island", false),
            ("Island", false),
            ("Island", false),
            ("Island", false),
        ],
        false,
    );
    p.config.cards[2].controller = PlayerId(1);
    p.profile_digest = digest(&p.config).unwrap();
    both(p, |g, q| {
        let stale = object(g, 0);
        cast(g, q, 3, vec![TargetEntity::Object { object: stale }]);
        assert!(matches!(
            g.current(object(g, 0)).unwrap().zone,
            Zone::Hand { .. }
        ));
        cast(g, q, 0, vec![]);
        let source = object(g, 0);
        assert_ne!(stale, source);
        assert_eq!(g.current(source).unwrap().zone, Zone::Battlefield);
        let r = request(g, q);
        let snapshot = g.snapshot().unwrap();
        assert!(
            g.respond(
                r.player(),
                r.decision_id(),
                Response::ActivateAbility {
                    object: stale,
                    ability: 0
                }
            )
            .is_err()
        );
        assert_eq!(g.snapshot().unwrap(), snapshot);
        answer(
            g,
            q,
            Response::ActivateAbility {
                object: source,
                ability: 0,
            },
        );
        let r = request(g, q);
        let snapshot = g.snapshot().unwrap();
        assert!(matches!(r, Request::Targets { .. }));
        assert!(
            g.respond(
                r.player(),
                r.decision_id(),
                Response::Targets {
                    objects: vec![object(g, 2)]
                }
            )
            .is_err()
        );
        assert_eq!(g.snapshot().unwrap(), snapshot);
        pay_and_resolve(
            g,
            q,
            vec![TargetEntity::Object {
                object: object(g, 1),
            }],
        );
        for _ in 0..30 {
            if g.turn().step != TurnStep::PrecombatMain {
                break;
            }
            answer(g, q, Response::Pass);
        }
        let r = request(g, q);
        let snapshot = g.snapshot().unwrap();
        assert!(
            g.respond(
                r.player(),
                r.decision_id(),
                Response::ActivateAbility {
                    object: source,
                    ability: 0
                }
            )
            .is_err()
        );
        assert_eq!(g.snapshot().unwrap(), snapshot);
    });
}
#[test]
fn equipped_indestructible_internal_ordered_destruction_and_atomic_failure() {
    // Direct internal event invariant probe, not an accepted-input replay witness.
    let p = prepared(
        &[
            ("Typed nonintrinsic equipment grant control", false),
            ("Grizzly Bears", false),
            ("Mountain", false),
            ("Mountain", false),
        ],
        true,
    );
    let mut g = Game::with_trace(p, TraceMode::Detailed);
    request(&mut g, 1);
    equip(&mut g, 1, 0, 1);
    let source = object(&g, 0);
    let target = object(&g, 1);
    let before = g.snapshot().unwrap();
    assert!(
        g.commit_batch(
            BatchCause::Fixture,
            EventGroup::Ordered {
                children: vec![
                    EventGroup::Occurrence {
                        proposed: ProposedEvent::DestroyPermanent { object: source }
                    },
                    EventGroup::Occurrence {
                        proposed: ProposedEvent::DestroyCreature {
                            object: object(&g, 2)
                        }
                    },
                ]
            }
        )
        .is_err()
    );
    assert_eq!(before, g.snapshot().unwrap());
    assert!(
        g.commit_batch(
            BatchCause::Fixture,
            EventGroup::Simultaneous {
                occurrences: vec![
                    ProposedEvent::DestroyPermanent { object: source },
                    ProposedEvent::DestroyCreature { object: target },
                    ProposedEvent::DestroyCreature {
                        object: object(&g, 2)
                    },
                ]
            }
        )
        .is_err()
    );
    assert_eq!(before, g.snapshot().unwrap());
    let mut probe = g.fork();
    let receipt = probe
        .commit_batch(
            BatchCause::Fixture,
            EventGroup::Occurrence {
                proposed: ProposedEvent::DestroyCreature { object: target },
            },
        )
        .unwrap();
    let frozen = receipt.occurrences[0].before.clone();
    for variant in 0..3 {
        let mut bad = frozen.clone();
        match variant {
            0 => bad.objects.clear(),
            1 => bad.objects[0].object.incarnation += 1,
            _ => bad.objects[0].zone = Zone::Exile,
        }
        assert!(
            g.apply_occurrence_with_prior(
                &ProposedEvent::DestroyCreature { object: target },
                Some(&bad)
            )
            .is_err()
        );
        assert_eq!(before, g.snapshot().unwrap());
    }
    // Internal authority probe: a grant gained after the frozen view cannot
    // retroactively prevent that destruction; direct calls use current facts.
    let mut no_grant = g.fork();
    no_grant
        .apply_occurrence(&ProposedEvent::Attach {
            object: source,
            target: None,
        })
        .unwrap();
    let mut capture = no_grant.fork();
    let prior = capture
        .commit_batch(
            BatchCause::Fixture,
            EventGroup::Occurrence {
                proposed: ProposedEvent::DestroyCreature { object: target },
            },
        )
        .unwrap()
        .occurrences[0]
        .before
        .clone();
    let mut gained = g.fork();
    assert!(matches!(
        gained
            .apply_occurrence_with_prior(
                &ProposedEvent::DestroyCreature { object: target },
                Some(&prior)
            )
            .unwrap(),
        EventOutcome::Moved { .. }
    ));
    assert!(matches!(
        g.apply_occurrence(&ProposedEvent::DestroyCreature { object: target })
            .unwrap(),
        EventOutcome::DestructionPrevented { .. }
    ));
    let receipt = g
        .commit_batch(
            BatchCause::Fixture,
            EventGroup::Ordered {
                children: vec![
                    EventGroup::Occurrence {
                        proposed: ProposedEvent::DestroyPermanent { object: source },
                    },
                    EventGroup::Occurrence {
                        proposed: ProposedEvent::DestroyCreature { object: target },
                    },
                ],
            },
        )
        .unwrap();
    assert!(
        !receipt.occurrences[1].before.objects[0]
            .keywords
            .contains(&KeywordAbility::Indestructible)
    );
    assert!(matches!(
        g.current(object(&g, 1)).unwrap().zone,
        Zone::Graveyard { .. }
    ));
}

#[test]
fn equipped_indestructible_paid_exile_removes_grant_before_lethal_sba() {
    both(
        prepared(
            &[
                ("Darksteel Plate", false),
                ("Grizzly Bears", false),
                ("Lightning Bolt", true),
                ("Revoke Existence", true),
                ("Mountain", false),
                ("Mountain", false),
                ("Mountain", false),
                ("Plains", false),
                ("Plains", false),
            ],
            false,
        ),
        |g, q| {
            equip(g, q, 0, 1);
            cast(
                g,
                q,
                2,
                vec![TargetEntity::Object {
                    object: object(g, 1),
                }],
            );
            assert_eq!(g.current(object(g, 1)).unwrap().marked_damage, 3.into());
            cast(
                g,
                q,
                3,
                vec![TargetEntity::Object {
                    object: object(g, 0),
                }],
            );
            assert_eq!(g.current(object(g, 0)).unwrap().zone, Zone::Exile);
            assert!(matches!(
                g.current(object(g, 1)).unwrap().zone,
                Zone::Graveyard { .. }
            ));
        },
    );
}
