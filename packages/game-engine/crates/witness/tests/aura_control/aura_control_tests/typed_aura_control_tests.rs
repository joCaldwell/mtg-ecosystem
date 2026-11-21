//! Private paid typed Aura witnesses; no pool or consumer admission.
//! Archive d705222b280a71cb: Steal Artifact 30992, Annex 14277, Conquer 28632,
//! Silver Myr 15462, Seat of the Synod 8573 (complete records, not card stubs).
//! CR20260925 205.2b, 301.5d, 302.6, 400.7, 601.6, 608.2b/3c,
//! 613.7e, 702.11, 800.4a-c; rules SHA256
//! 8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

const SOURCES: [&str; 7] = [
    include_str!("../../../../../tests/fixtures/cd8ae9f2-edac-473a-8846-c08219e617c3.json"),
    include_str!("../../../../../tests/fixtures/5f1f49dd-b7ce-494a-9b4b-ac702d3664c5.json"),
    include_str!("../../../../../tests/fixtures/be15e131-0753-4ceb-a36b-4f9172241080.json"),
    include_str!("../../../../../tests/fixtures/66e8f7f8-3a6d-46ba-837c-b9713ddf7f40.json"),
    include_str!("../../../../../tests/fixtures/39451b4d-cd7a-40da-b457-cb51b609173f.json"),
    include_str!("../../../../../tests/fixtures/24882fa2-3fe9-4c1b-aa3d-0e6488b9db27.json"),
    include_str!("../../../../../tests/fixtures/2fa763ad-4d94-4c3a-a099-13ac22c09ce4.json"),
];

fn prepared(rows: &[Row]) -> Prepared {
    let p = prepared_with_sources(rows, &SOURCES);
    for (name, kind, index, generic, color) in [
        ("Steal Artifact", CardKind::Artifact, 30992, 2, Color::U),
        ("Annex", CardKind::Land, 14277, 2, Color::U),
        ("Conquer", CardKind::Land, 28632, 3, Color::R),
    ] {
        let d = p
            .catalog
            .definitions
            .iter()
            .find(|d| d.name == name)
            .unwrap();
        let domain = match kind {
            CardKind::Creature => mtg_contracts::AuraControlDomain::Creature,
            CardKind::Artifact => mtg_contracts::AuraControlDomain::Artifact,
            CardKind::Land => mtg_contracts::AuraControlDomain::Land,
            _ => panic!("unexpected typed Aura fixture domain"),
        };
        assert_eq!(mtg_contracts::aura_control_domain(d), Some(domain));
        assert!(mtg_contracts::aura_control_source(d));
        assert!(d.capabilities.contains(&Capability::AuraControlV1));
        assert_eq!(d.provenance.source_index, index);
        assert_eq!(d.characteristics.mana_cost.generic, generic.into());
        assert_eq!(d.characteristics.mana_cost.colored, [color, color]);
    }
    p
}

// Prefund colored symbols through real mana activations. The inherited payment
// helper supplies remaining generic mana and records the actual SpendMana.
fn announce_paid(g: &mut Game, q: u64, id: u32, targets: Vec<TargetEntity>) {
    let colors = g
        .definition(CardId(id))
        .unwrap()
        .characteristics
        .mana_cost
        .colored
        .clone();
    for c in colors {
        fund(
            g,
            q,
            match c {
                Color::U => "Island",
                Color::R => "Mountain",
                Color::G => "Forest",
                Color::W => "Plains",
                _ => panic!("witness color"),
            },
            1,
        );
    }
    announce(g, q, id, targets);
    assert!(g.state.rules.stack.last().unwrap().paid.is_some());
}

fn cast_paid(g: &mut Game, q: u64, id: u32, targets: Vec<TargetEntity>) {
    announce_paid(g, q, id, targets);
    resolve(g, q, |_| {});
}

fn zone(g: &Game, id: u32, expected: Zone) {
    assert_eq!(g.current(object(g, id)).unwrap().zone, expected);
}

#[test]
fn typed_aura_control_paid_exact_domains_atomic_entry_and_invalid_targets() {
    for (name, kind, chosen, wrong) in [
        ("Steal Artifact", CardKind::Artifact, 1, 3),
        ("Annex", CardKind::Land, 3, 1),
        ("Conquer", CardKind::Land, 3, 1),
    ] {
        let mut rows = vec![
            (name, 0, 0, true),
            ("Sol Ring", 2, 1, false),
            ("Silver Myr", 2, 1, false),
            ("Forest", 2, 1, false),
            ("Seat of the Synod", 2, 1, false),
            ("Grizzly Bears", 1, 1, false),
        ];
        lands(&mut rows, "Island", 0, 5);
        lands(&mut rows, "Mountain", 0, 5);
        both(prepared(&rows), |g, q| {
            let selected = object(g, chosen);
            let mut probe = g.fork();
            answer(&mut probe, q, Response::Cast { card: CardId(0) });
            let r = request(&mut probe, q);
            let Request::Targets { candidates, .. } = &r else {
                panic!("typed Aura targets")
            };
            let expected: Vec<_> = probe
                .state
                .rules
                .objects
                .values()
                .filter(|o| {
                    o.zone == Zone::Battlefield
                        && probe
                            .definition(o.id.card)
                            .unwrap()
                            .characteristics
                            .has_type(kind)
                        && probe.targetable_by(o.id, PlayerId(0))
                })
                .map(|o| o.id)
                .collect();
            assert_eq!(candidates, &expected);
            assert!(
                candidates.contains(&object(g, 4)),
                "both artifact and land membership"
            );
            assert_eq!(
                candidates.contains(&object(g, 2)),
                kind == CardKind::Artifact
            );
            let before = probe.state.clone();
            let trace = probe.trace.clone();
            for objects in [
                vec![],
                vec![object(g, wrong)],
                vec![ObjectId {
                    incarnation: selected.incarnation + 1,
                    ..selected
                }],
                vec![object(g, 0)],
                vec![selected, object(g, 4)],
            ] {
                assert!(
                    probe
                        .respond(r.player(), r.decision_id(), Response::Targets { objects })
                        .is_err()
                );
                assert_eq!(probe.state, before);
                assert_eq!(probe.trace, trace);
            }
            announce_paid(g, q, 0, target(g, chosen));
            let stack = g.state.rules.stack.last().unwrap().clone();
            assert_eq!(
                stack.paid.as_ref().unwrap().colored,
                g.definition(CardId(0))
                    .unwrap()
                    .characteristics
                    .mana_cost
                    .colored
            );
            assert_eq!(
                stack.paid.as_ref().unwrap().generic,
                g.definition(CardId(0))
                    .unwrap()
                    .characteristics
                    .mana_cost
                    .generic
            );
            recover(g);
            let mut observed = false;
            resolve(g, q, |g| {
                if g.current(object(g, 0)).unwrap().zone == Zone::Battlefield {
                    observed = true;
                    assert_eq!(g.current(object(g, 0)).unwrap().attached_to, Some(selected));
                    control(g, chosen, 0, 2, 1);
                    visible(g, 0, chosen, 0);
                }
            });
            assert!(observed);
            assert_eq!(g.state.rules.next_attachment_timestamp, 2);
            if g.trace.mode == TraceMode::Detailed {
                let entries: Vec<_> = batches(g).into_iter().filter(|b| matches!(b.cause, BatchCause::SpellCompletion { stack_id, .. } if stack_id == stack.id)).collect();
                assert_eq!(entries.len(), 1);
                assert_eq!(entries[0].occurrences.len(), 1);
                let EventOutcome::EnteredAttached {
                    movement,
                    target,
                    attachment_timestamp,
                } = &entries[0].occurrences[0].actual
                else {
                    panic!("one atomic attached entry")
                };
                assert_eq!(movement.previous, stack.source);
                assert_eq!(movement.successor, object(g, 0));
                assert_eq!(movement.from, Zone::Stack);
                assert_eq!(movement.destination, Zone::Battlefield);
                assert_eq!(*target, selected);
                assert_eq!(*attachment_timestamp, 1);
                assert_eq!(g.events().iter().filter(|e| matches!(e, Event::ControlChanged { object, previous, controller } if *object == selected && *previous == PlayerId(1) && *controller == PlayerId(0))).count(), 1);
            }
        });
    }
}

#[test]
fn typed_aura_control_noncreature_mana_and_attached_equipment_authority() {
    for (aura, name, color, amount) in [
        ("Steal Artifact", "Sol Ring", Color::C, 2),
        ("Annex", "Forest", Color::G, 1),
        ("Conquer", "Forest", Color::G, 1),
    ] {
        let mut rows = vec![
            (aura, 0, 0, true),
            (name, 2, 1, false),
            ("Island", 1, 1, false),
        ];
        lands(&mut rows, "Island", 0, 6);
        lands(&mut rows, "Mountain", 0, 5);
        both(prepared(&rows), |g, q| {
            let permanent = object(g, 1);
            assert!(g.validate_mana_activation(PlayerId(1), permanent).is_ok());
            announce_paid(g, q, 0, target(g, 1));
            until_player(g, q, PlayerId(1));
            answer(
                g,
                q,
                Response::ActivateMana {
                    object: object(g, 2),
                },
            );
            request(g, q);
            assert_eq!(
                g.state.rules.players[1].mana.get(&Color::U),
                Some(&1.into())
            );
            resolve(g, q, |_| {});
            control(g, 1, 0, 2, 1);
            assert!(!g.current(permanent).unwrap().tapped);
            assert!(g.validate_mana_activation(PlayerId(1), permanent).is_err());
            assert!(g.validate_mana_activation(PlayerId(0), permanent).is_ok());
            let mut expected = g.state.rules.players[0]
                .mana
                .get(&color)
                .cloned()
                .unwrap_or_default();
            expected.add(&amount.into());
            answer(g, q, Response::ActivateMana { object: permanent });
            request(g, q);
            assert!(g.current(permanent).unwrap().tapped);
            assert_eq!(g.state.rules.players[0].mana.get(&color), Some(&expected));
            assert_eq!(
                g.state.rules.players[1].mana.get(&Color::U),
                Some(&1.into()),
                "already floated mana stays with its producer"
            );
        });
    }
    let mut rows = vec![
        ("Steal Artifact", 1, 1, true),
        ("Bonesplitter", 0, 0, false),
        ("Grizzly Bears", 2, 0, false),
        ("Grizzly Bears", 1, 1, false),
    ];
    lands(&mut rows, "Island", 0, 1);
    lands(&mut rows, "Island", 1, 6);
    both(prepared(&rows), |g, q| {
        equip(g, q, 1, 2);
        let equipment = object(g, 1);
        let creature = object(g, 2);
        let timestamp = g.current(equipment).unwrap().attachment_timestamp;
        let turn = g.turn().number;
        own_main(g, q, 1, turn);
        cast_paid(g, q, 0, target(g, 1));
        control(g, 1, 1, 0, 0);
        control(g, 2, 0, 2, 0);
        assert_eq!(g.current(equipment).unwrap().attached_to, Some(creature));
        assert_eq!(
            g.current(equipment).unwrap().attachment_timestamp,
            timestamp
        );
        assert_eq!(g.characteristics(creature).unwrap().power, Some(4.into()));
        let r = request(g, q);
        let before = g.state.clone();
        let trace = g.trace.clone();
        assert!(
            g.respond(
                PlayerId(0),
                r.decision_id(),
                Response::ActivateAbility {
                    object: equipment,
                    ability: 0
                }
            )
            .is_err()
        );
        assert_eq!(g.state, before);
        assert_eq!(g.trace, trace);
        assert!(
            g.validate_actions_response(
                PlayerId(1),
                &Response::ActivateAbility {
                    object: equipment,
                    ability: 0
                }
            )
            .unwrap()
            .is_ok()
        );
        // Only the new Equipment controller announces its real equip ability.
        equip(g, q, 1, 3);
        assert_eq!(
            g.current(equipment).unwrap().attached_to,
            Some(object(g, 3))
        );
        control(g, 2, 0, 2, 0);
        assert_eq!(g.characteristics(creature).unwrap().power, Some(2.into()));
    });

    // A genuinely paid Steal Artifact supplies authority for the existing
    // nested Signet mana ability. No synthetic control or sacrifice source.
    for depart_target in [false, true] {
        let mut rows = vec![
            ("Steal Artifact", 0, 0, true),
            ("Golgari Signet", 1, 1, false),
            ("Annex", 0, 0, true),
            ("Forest", 2, 2, false),
            ("Mountain", 0, 0, false),
        ];
        lands(&mut rows, "Island", 0, 6);
        both(prepared(&rows), |g, q| {
            // Keep the Mountain for the later Signet payment: otherwise the
            // generic-payment helper selects it before the remaining Islands.
            fund(g, q, "Island", 2);
            cast_paid(g, q, 0, target(g, 1));
            control(g, 1, 0, 1, 1);
            assert!(!g.current(object(g, 4)).unwrap().tapped);
            fund(g, q, "Island", 2); // Pre-cast mana is independent of undo.
            answer(g, q, Response::Cast { card: CardId(2) });
            answer(
                g,
                q,
                Response::Targets {
                    objects: vec![object(g, 3)],
                },
            );
            assert!(matches!(request(g, q), Request::Mana { .. }));
            answer(
                g,
                q,
                Response::ActivateMana {
                    object: object(g, 1),
                },
            );
            assert!(
                matches!(request(g, q), Request::Mana { cost, .. } if cost.generic == 1.into() && cost.colored.is_empty())
            );
            answer(
                g,
                q,
                Response::ActivateMana {
                    object: object(g, 4),
                },
            );
            answer(g, q, Response::FinishMana { undo_mana: false });
            assert!(matches!(request(g, q), Request::SpendMana { .. }));
            answer(
                g,
                q,
                Response::SpendMana {
                    spending: BTreeMap::from([(Color::R, 1.into())]),
                },
            );
            assert!(matches!(request(g, q), Request::Mana { .. }));
            assert!(g.current(object(g, 1)).unwrap().tapped);
            assert_eq!(
                g.state.rules.players[0].mana.get(&Color::B),
                Some(&1.into())
            );
            assert_eq!(
                g.state.rules.players[0].mana.get(&Color::G),
                Some(&1.into())
            );
            recover(g);
            g.concede(PlayerId(1)).unwrap();
            assert!(matches!(request(g, q), Request::Mana { .. }));
            assert!(g.object_id(CardId(1)).is_none());
            // Owner departure cannot refund the surviving payer's pre-cast
            // mana or the Signet's already completed mana production.
            assert_eq!(
                g.state.rules.players[0].mana.get(&Color::U),
                Some(&2.into())
            );
            recover(g);
            // FinishMana's flag requests undo only if payment cannot complete.
            // This payable spell must proceed to an actual spending decision.
            answer(g, q, Response::FinishMana { undo_mana: true });
            assert!(matches!(request(g, q), Request::SpendMana { .. }));
            if depart_target {
                g.concede(PlayerId(2)).unwrap();
                assert!(matches!(request(g, q), Request::SpendMana { .. }));
                assert!(g.object_id(CardId(3)).is_none());
                recover(g);
            }
            answer(
                g,
                q,
                Response::SpendMana {
                    spending: BTreeMap::from([
                        (Color::U, 2.into()),
                        (Color::B, 1.into()),
                        (Color::G, 1.into()),
                    ]),
                },
            );
            assert!(matches!(request(g, q), Request::Priority { .. }));
            let paid = g.state.rules.stack.last().unwrap().clone();
            assert!(
                paid.paid.is_some(),
                "CR601.6: target loss during payment does not undo the completed proposal"
            );
            assert_eq!(paid.source.card, CardId(2));
            assert!(g.current(object(g, 4)).unwrap().tapped);
            recover(g);
            resolve(g, q, |_| {});
            zone(g, 0, Zone::Graveyard { owner: PlayerId(0) });
            if depart_target {
                zone(g, 2, Zone::Graveyard { owner: PlayerId(0) });
                if g.trace.mode == TraceMode::Detailed {
                    assert!(g.events().iter().any(|e| matches!(e, Event::SpellCompleted { stack_id, resolved: false } if *stack_id == paid.id)));
                }
            } else {
                control(g, 3, 0, 2, 2);
                zone(g, 2, Zone::Battlefield);
            }
            for color in [Color::U, Color::B, Color::G] {
                assert_eq!(g.state.rules.players[0].mana.get(&color), Some(&0.into()));
            }
        });
    }
}

#[test]
fn typed_aura_control_artifact_creature_sickness_combat_and_actual_untap() {
    let mut rows = vec![
        ("Steal Artifact", 0, 0, true),
        ("Silver Myr", 2, 1, false),
        ("Disperse", 1, 1, true),
    ];
    lands(&mut rows, "Island", 0, 4);
    lands(&mut rows, "Island", 1, 2);
    both(prepared(&rows), |g, q| {
        cast_paid(g, q, 0, target(g, 1));
        let myr = object(g, 1);
        control(g, 1, 0, 2, 1);
        assert!(!g.current(myr).unwrap().attack_ready);
        for player in [PlayerId(0), PlayerId(1)] {
            assert!(g.validate_permanent_mana_activation(player, myr).is_err());
        }
        let r = request(g, q);
        let before = g.state.clone();
        let trace = g.trace.clone();
        assert!(
            g.respond(
                r.player(),
                r.decision_id(),
                Response::ActivateMana { object: myr }
            )
            .is_err()
        );
        assert_eq!(g.state, before);
        assert_eq!(g.trace, trace);
        let mut declaration = false;
        for _ in 0..100 {
            let r = request(g, q);
            if let Request::Attackers { candidates, .. } = &r {
                declaration = true;
                assert!(!candidates.contains(&myr));
                let before = g.state.clone();
                assert!(
                    g.respond(
                        r.player(),
                        r.decision_id(),
                        Response::Attackers {
                            attacks: vec![(myr, PlayerId(1))]
                        }
                    )
                    .is_err()
                );
                assert_eq!(g.state, before);
                break;
            }
            answer(g, q, pass(&r));
        }
        assert!(declaration);
        let turn = g.turn().number;
        own_main(g, q, 0, turn);
        assert!(g.current(myr).unwrap().attack_ready);
        assert!(g.creatures(PlayerId(0), true).contains(&myr));
        assert!(
            g.validate_permanent_mana_activation(PlayerId(1), myr)
                .is_err()
        );
        answer(g, q, Response::ActivateMana { object: myr });
        request(g, q);
        assert!(g.current(myr).unwrap().tapped);
        assert_eq!(
            g.state.rules.players[0].mana.get(&Color::U),
            Some(&1.into())
        );
        let turn = g.turn().number;
        own_main(g, q, 0, turn);
        assert!(!g.current(myr).unwrap().tapped);
        let mut declared = false;
        for _ in 0..100 {
            let r = request(g, q);
            if let Request::Attackers { candidates, .. } = &r {
                assert!(candidates.contains(&myr));
                answer(
                    g,
                    q,
                    Response::Attackers {
                        attacks: vec![(myr, PlayerId(1))],
                    },
                );
                declared = true;
                break;
            }
            answer(g, q, pass(&r));
        }
        assert!(declared);
        until_player(g, q, PlayerId(1));
        assert!(g.state.rules.combat.iter().any(|a| a.attacker == myr));
        cast_paid(g, q, 2, target(g, 0));
        control(g, 1, 1, 2, 1);
        assert!(g.current(myr).unwrap().tapped);
        assert!(!g.current(myr).unwrap().attack_ready);
        assert!(
            !g.state
                .rules
                .combat
                .iter()
                .any(|a| a.attacker == myr || a.blockers.contains(&myr))
        );
    });
}

#[test]
fn typed_aura_control_artifact_land_shared_clock_both_orders_reveal_older() {
    for land_aura in ["Annex", "Conquer"] {
        for first in [0, 1] {
            let second = 1 - first;
            let mut rows = vec![
                ("Steal Artifact", 0, 0, true),
                (land_aura, 1, 1, true),
                ("Seat of the Synod", 3, 2, false),
                ("Disperse", 1, 1, true),
                ("Disperse", 1, 1, true),
            ];
            rows[first as usize].1 = 0;
            rows[first as usize].2 = 0;
            rows[second as usize].1 = 1;
            rows[second as usize].2 = 1;
            lands(&mut rows, "Island", 0, 6);
            lands(&mut rows, "Mountain", 0, 5);
            lands(&mut rows, "Island", 1, 10);
            lands(&mut rows, "Mountain", 1, 5);
            both(prepared(&rows), |g, q| {
                cast_paid(g, q, first, target(g, 2));
                control(g, 2, 0, 3, 2);
                let turn = g.turn().number;
                own_main(g, q, 1, turn);
                cast_paid(g, q, second, target(g, 2));
                control(g, 2, 1, 3, 2);
                assert_eq!(
                    g.current(object(g, first)).unwrap().attachment_timestamp,
                    Some(1)
                );
                assert_eq!(
                    g.current(object(g, second)).unwrap().attachment_timestamp,
                    Some(2)
                );
                visible(g, second, 2, 1);
                recover(g);
                cast_paid(g, q, 3, target(g, second));
                control(g, 2, 0, 3, 2);
                cast_paid(g, q, 4, target(g, first));
                control(g, 2, 2, 3, 2);
                assert_eq!(g.state.rules.next_attachment_timestamp, 3);
            });
        }
    }
}

#[test]
fn typed_aura_control_paid_bounce_reincarnation_and_resolution_fizzle() {
    for (aura, name) in [
        ("Steal Artifact", "Sol Ring"),
        ("Annex", "Forest"),
        ("Conquer", "Forest"),
    ] {
        for fizzle in [false, true] {
            let mut rows = vec![
                (aura, 0, 0, true),
                (name, 0, 1, false),
                ("Disperse", 0, 0, true),
                ("Regress", 0, 0, true),
                ("Regress", 1, 1, true),
            ];
            lands(&mut rows, "Island", 0, 18);
            lands(&mut rows, "Mountain", 0, 10);
            lands(&mut rows, "Island", 1, 3);
            both(prepared(&rows), |g, q| {
                let original = object(g, 1);
                if fizzle {
                    announce_paid(g, q, 0, target(g, 1));
                    let paid = g.state.rules.stack.last().unwrap().clone();
                    recover(g);
                    until_player(g, q, PlayerId(1));
                    cast_paid(g, q, 4, target(g, 1));
                    zone(g, 0, Zone::Graveyard { owner: PlayerId(0) });
                    assert_eq!(g.state.rules.next_attachment_timestamp, 1);
                    if g.trace.mode == TraceMode::Detailed {
                        assert!(g.events().iter().any(|e| matches!(e, Event::SpellCompleted { stack_id, resolved: false } if *stack_id == paid.id)));
                        assert!(!batches(g).iter().flat_map(|b| &b.occurrences).any(|o| matches!(&o.actual, EventOutcome::EnteredAttached { movement, .. } if movement.previous == paid.source)));
                    }
                } else {
                    cast_paid(g, q, 0, target(g, 1));
                    let aura_object = object(g, 0);
                    cast_paid(g, q, 2, target(g, 0));
                    control(g, 1, 1, 0, 1);
                    assert!(g.current(aura_object).is_err());
                    assert_eq!(g.current(object(g, 0)).unwrap().attachment_timestamp, None);
                    cast_paid(g, q, 0, target(g, 1));
                    assert_ne!(object(g, 0), aura_object);
                    assert_eq!(
                        g.current(object(g, 0)).unwrap().attachment_timestamp,
                        Some(2)
                    );
                    cast_paid(g, q, 3, target(g, 1));
                    zone(g, 0, Zone::Graveyard { owner: PlayerId(0) });
                    assert_eq!(g.state.rules.next_attachment_timestamp, 3);
                }
                zone(g, 1, Zone::Hand { owner: PlayerId(0) });
                assert!(g.current(original).is_err());
                until_player(g, q, PlayerId(0));
                if name == "Forest" {
                    answer(g, q, Response::PlayLand { card: CardId(1) });
                    request(g, q);
                } else {
                    cast_paid(g, q, 1, vec![]);
                }
                assert_ne!(object(g, 1), original);
                control(g, 1, 0, 0, 0);
                assert_eq!(g.current(object(g, 0)).unwrap().attached_to, None);
                recover(g);
            });
        }
    }
}

// Finish a target choice submitted through a public validation receipt. Every
// mana unit is already floated; choose exact colors rather than auto-canceling.
fn pay_funded(g: &mut Game, q: u64) {
    let r = request(g, q);
    assert!(matches!(r, Request::Mana { .. }));
    answer(g, q, Response::FinishMana { undo_mana: false });
    let r = request(g, q);
    let Request::SpendMana {
        cost, available, ..
    } = r
    else {
        panic!("exact funded payment")
    };
    let mut rest = available;
    let mut spending = BTreeMap::<Color, Quantity>::new();
    for c in cost.colored {
        rest.get_mut(&c).unwrap().subtract(&1.into());
        spending.entry(c).or_default().add(&1.into());
    }
    let mut generic = cost.generic;
    for (c, n) in rest {
        let used = n.min(generic.clone());
        generic.subtract(&used);
        if used > Quantity::zero() {
            spending.entry(c).or_default().add(&used);
        }
    }
    assert_eq!(generic, Quantity::zero());
    answer(g, q, Response::SpendMana { spending });
    assert!(matches!(request(g, q), Request::Priority { .. }));
    assert!(g.state.rules.stack.last().unwrap().paid.is_some());
}

#[test]
fn typed_aura_control_current_controller_hexproof_receipts_and_query_privacy() {
    for (aura, name) in [
        ("Steal Artifact", "Sol Ring"),
        ("Annex", "Forest"),
        ("Conquer", "Forest"),
    ] {
        let mut rows = vec![
            (aura, 0, 0, true),
            (name, 2, 1, false),
            ("Heroic Intervention", 0, 0, true),
            ("Disperse", 0, 0, true),
            ("Regress", 0, 0, true),
            ("Regress", 1, 1, true),
        ];
        lands(&mut rows, "Island", 0, 12);
        lands(&mut rows, "Mountain", 0, 5);
        lands(&mut rows, "Forest", 0, 2);
        lands(&mut rows, "Island", 1, 6);
        both(prepared(&rows), |g, q| {
            cast_paid(g, q, 0, target(g, 1));
            cast_paid(g, q, 2, vec![]);
            let permanent = object(g, 1);
            assert!(g.has_keyword(permanent, KeywordAbility::Hexproof));
            assert!(g.targetable_by(permanent, PlayerId(0)));
            assert!(!g.targetable_by(permanent, PlayerId(1)));
            let mut opposing = g.fork();
            until_player(&mut opposing, q, PlayerId(1));
            answer(&mut opposing, q, Response::Cast { card: CardId(5) });
            let r = request(&mut opposing, q);
            assert!(
                matches!(&r, Request::Targets { candidates, .. } if !candidates.contains(&permanent))
            );
            let before = opposing.state.clone();
            let trace = opposing.trace.clone();
            assert!(
                opposing
                    .respond(
                        r.player(),
                        r.decision_id(),
                        Response::Targets {
                            objects: vec![permanent]
                        }
                    )
                    .is_err()
            );
            assert_eq!(opposing.state, before);
            assert_eq!(opposing.trace, trace);
            // Own Hexproof permits the actual paid permanent target. Only the
            // announcing viewer receives its token/candidates or a receipt.
            fund(g, q, "Island", 3);
            answer(g, q, Response::Cast { card: CardId(4) });
            let r = request(g, q);
            assert!(
                matches!(&r, Request::Targets { candidates, .. } if candidates.contains(&permanent))
            );
            let before = g.state.clone();
            let trace = g.trace.clone();
            let QueryValue::Decision(Some(PlayerRequest::Targets { token, .. })) =
                finish_query(g, g.player_decision_for(PlayerId(0), Budget::new(0)))
            else {
                panic!("own target query")
            };
            let handle = g.handle_for(PlayerId(0), permanent).unwrap();
            for seat in 1..4 {
                assert!(matches!(
                    finish_query(g, g.player_decision_for(PlayerId(seat), Budget::new(0))),
                    QueryValue::Decision(None)
                ));
            }
            let QueryResult::Incomplete { work: stale, .. } =
                g.observe(PlayerId(0), Budget::new(0))
            else {
                panic!("private suspended work")
            };
            let response = PlayerResponse::Targets {
                objects: vec![handle],
            };
            let QueryValue::Validated(receipt) = finish_query(
                g,
                g.check_player_response(
                    PlayerId(0),
                    token.clone(),
                    response.clone(),
                    Budget::new(0),
                ),
            ) else {
                panic!("own validation receipt")
            };
            let QueryValue::Validated(duplicate) = finish_query(
                g,
                g.check_player_response(PlayerId(0), token, response, Budget::new(0)),
            ) else {
                panic!("duplicate pre-change receipt")
            };
            for viewer in 0..4 {
                let QueryValue::Observation(o) =
                    finish_query(g, g.observe(PlayerId(viewer), Budget::new(0)))
                else {
                    panic!("permitted observation")
                };
                assert!(
                    !o.view
                        .objects
                        .iter()
                        .any(|o| matches!(o.zone, Zone::Library { .. }))
                );
            }
            assert_eq!(g.state, before);
            assert_eq!(g.trace, trace);
            g.submit(receipt).unwrap();
            pay_funded(g, q);
            let paid = g.state.rules.stack.last().unwrap().id;
            recover(g);
            // The Aura also has Hexproof: its current controller can bounce
            // it with a genuine paid Disperse. The lower paid Regress then
            // encounters an opposing Hexproof target and fails at resolution.
            cast_paid(g, q, 3, target(g, 0));
            zone(g, 0, Zone::Hand { owner: PlayerId(0) });
            zone(g, 4, Zone::Graveyard { owner: PlayerId(0) });
            if g.trace.mode == TraceMode::Detailed {
                assert!(g.events().iter().any(|e| matches!(e, Event::SpellCompleted { stack_id, resolved: false } if *stack_id == paid)));
            }
            control(g, 1, 1, 2, 1);
            assert!(g.has_keyword(permanent, KeywordAbility::Hexproof));
            assert!(g.targetable_by(permanent, PlayerId(1)));
            assert!(!g.targetable_by(permanent, PlayerId(2)));
            assert!(!g.state.rules.stack.iter().any(|s| s.id == paid));
            let before = g.state.clone();
            let trace = g.trace.clone();
            assert!(g.submit(duplicate).is_err());
            assert!(matches!(
                g.resume_work(stale, Budget::unlimited()),
                QueryResult::Rejected(QueryRejection::StaleOrForeignWork)
            ));
            assert_eq!(g.state, before);
            assert_eq!(g.trace, trace);
            until_player(g, q, PlayerId(1));
            cast_paid(g, q, 5, target(g, 1));
            zone(g, 1, Zone::Hand { owner: PlayerId(2) });
        });
    }
}

#[test]
fn typed_aura_control_departures_and_common_prior_wipe() {
    for (aura, name) in [
        ("Steal Artifact", "Sol Ring"),
        ("Annex", "Forest"),
        ("Conquer", "Forest"),
    ] {
        for departure in ["source_owner", "target_owner", "default"] {
            let mut rows = vec![
                (aura, 0, 0, true),
                (name, 3, 2, false),
                ("Disperse", 0, 0, true),
            ];
            lands(&mut rows, "Island", 0, 8);
            lands(&mut rows, "Mountain", 0, 5);
            both(prepared(&rows), |g, q| {
                cast_paid(g, q, 0, target(g, 1));
                let permanent = object(g, 1);
                control(g, 1, 0, 3, 2);
                match departure {
                    "source_owner" => {
                        g.concede(PlayerId(0)).unwrap();
                        request(g, q);
                        assert!(g.object_id(CardId(0)).is_none());
                        control(g, 1, 2, 3, 2);
                    }
                    "target_owner" => {
                        g.concede(PlayerId(3)).unwrap();
                        request(g, q);
                        assert!(g.object_id(CardId(1)).is_none());
                        zone(g, 0, Zone::Graveyard { owner: PlayerId(0) });
                    }
                    _ => {
                        g.concede(PlayerId(2)).unwrap();
                        request(g, q);
                        control(g, 1, 0, 3, 2);
                        until_player(g, q, PlayerId(0));
                        cast_paid(g, q, 2, target(g, 0));
                        assert!(g.current(permanent).is_err());
                        zone(g, 1, Zone::Exile);
                    }
                }
            });
        }
    }
    for target_first in [false, true] {
        let mut rows = vec![
            ("Steal Artifact", 0, 0, true),
            ("Annex", 0, 0, true),
            ("Sol Ring", 3, 2, false),
            ("Seat of the Synod", 3, 2, false),
            ("Planar Cleansing", 0, 0, true),
        ];
        let (artifact_aura, artifact) = if target_first {
            rows.swap(0, 2);
            (2, 0)
        } else {
            (0, 2)
        };
        lands(&mut rows, "Island", 0, 8);
        lands(&mut rows, "Plains", 0, 6);
        both(prepared(&rows), |g, q| {
            cast_paid(g, q, artifact_aura, target(g, artifact));
            cast_paid(g, q, 1, target(g, 3));
            let moved = object(g, artifact);
            let surviving = object(g, 3);
            let start = g.events().len();
            cast_paid(g, q, 4, vec![]);
            zone(g, artifact, Zone::Graveyard { owner: PlayerId(3) });
            zone(g, artifact_aura, Zone::Graveyard { owner: PlayerId(0) });
            zone(g, 1, Zone::Graveyard { owner: PlayerId(0) });
            zone(g, 3, Zone::Battlefield); // Its land type excludes the wipe.
            control(g, 3, 2, 3, 2);
            if g.trace.mode == TraceMode::Detailed {
                assert!(!g.events().iter().skip(start).any(
                    |e| matches!(e, Event::ControlChanged { object, .. } if *object == moved)
                ));
                assert_eq!(g.events().iter().skip(start).filter(|e| matches!(e, Event::ControlChanged { object, previous, controller } if *object == surviving && *previous == PlayerId(0) && *controller == PlayerId(2))).count(), 1);
                let wipe = batches(g).into_iter().find(|b| b.occurrences.iter().any(|o| matches!(&o.actual, EventOutcome::Moved { movement } if movement.previous == moved))).unwrap();
                assert!(wipe.occurrences.len() >= 3);
                for occurrence in &wipe.occurrences {
                    let authority = occurrence.before.control.as_ref().unwrap();
                    for target in [moved, surviving] {
                        assert_eq!(
                            authority
                                .objects
                                .iter()
                                .find(|o| o.object == target)
                                .unwrap()
                                .controller,
                            PlayerId(0)
                        );
                    }
                }
            }
        });
    }
}

#[test]
fn typed_aura_control_normalized_timestamp_controller_attachment_domain_forgery() {
    let mut rows = vec![
        ("Steal Artifact", 0, 0, true),
        ("Annex", 1, 1, true),
        ("Seat of the Synod", 3, 2, false),
        ("Seat of the Synod", 3, 2, false),
        ("Forest", 3, 2, false),
    ];
    lands(&mut rows, "Island", 0, 4);
    lands(&mut rows, "Island", 1, 4);
    both(prepared(&rows), |g, q| {
        cast_paid(g, q, 0, target(g, 2));
        let turn = g.turn().number;
        own_main(g, q, 1, turn);
        cast_paid(g, q, 1, target(g, 2));
        recover(g);
        let wire = g.snapshot().unwrap();
        for variant in 0..4 {
            let mut bad: Snapshot = strict_read(&wire).unwrap();
            match variant {
                0 => {
                    bad.state
                        .rules
                        .objects
                        .get_mut(&CardId(0))
                        .unwrap()
                        .attachment_timestamp = Some(2);
                    bad.state
                        .rules
                        .objects
                        .get_mut(&CardId(1))
                        .unwrap()
                        .attachment_timestamp = Some(1);
                    bad.state
                        .rules
                        .objects
                        .get_mut(&CardId(2))
                        .unwrap()
                        .controller = PlayerId(0);
                }
                1 => {
                    bad.state
                        .rules
                        .objects
                        .get_mut(&CardId(2))
                        .unwrap()
                        .controller = PlayerId(2)
                }
                2 => {
                    // Both typed domains still fit the other actual artifact
                    // land. Locally coherent authority cannot replace history.
                    for id in [0, 1] {
                        bad.state
                            .rules
                            .objects
                            .get_mut(&CardId(id))
                            .unwrap()
                            .attached_to = Some(object(g, 3));
                    }
                    bad.state
                        .rules
                        .objects
                        .get_mut(&CardId(2))
                        .unwrap()
                        .controller = PlayerId(2);
                    bad.state
                        .rules
                        .objects
                        .get_mut(&CardId(3))
                        .unwrap()
                        .controller = PlayerId(1);
                }
                _ => {
                    // A real basic land fits Annex but not Steal Artifact.
                    bad.state
                        .rules
                        .objects
                        .get_mut(&CardId(0))
                        .unwrap()
                        .attached_to = Some(object(g, 4));
                    bad.state
                        .rules
                        .objects
                        .get_mut(&CardId(4))
                        .unwrap()
                        .controller = PlayerId(0);
                }
            }
            let mut structural = g.fork();
            structural.state = bad.state.clone();
            if variant == 0 || variant == 2 {
                structural.validate_state().unwrap();
            }
            if variant == 1 {
                assert!(structural.validate_control_state().is_err());
            }
            if variant == 3 {
                assert!(structural.validate_state().is_err());
            }
            bad.state_sha256 = digest(&bad.state).unwrap();
            assert!(
                Game::restore(g.prepared.as_ref().clone(), &canonical(&bad).unwrap()).is_err(),
                "normalized forgery {variant}"
            );
        }
    });
}
