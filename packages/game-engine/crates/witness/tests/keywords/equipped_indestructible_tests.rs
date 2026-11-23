//! Private source-backed configurations. No pool or consumer admission.
//! CR 702.12a-c,701.8a-b,704.5f-h,701.21a,120.3,514.2,614.17c:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_witness::equipped::*;
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
