//! Private paid typed Aura witnesses; native/schema-export/Ajv tests UNRUN here.
//! Archive d705222b280a71cb: Steal Artifact 30992, Annex 14277, Conquer 28632,
//! Silver Myr 15462, Seat of the Synod 8573 (complete records, not card stubs).
//! CR20260925 205.2b, 301.5d, 302.6, 400.7, 601.6, 608.2b/3c,
//! 613.7e, 702.11, 800.4a-c; rules SHA256
//! 8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

const SOURCES: [&str; 7] = [
    include_str!("../../../../tests/fixtures/cd8ae9f2-edac-473a-8846-c08219e617c3.json"),
    include_str!("../../../../tests/fixtures/5f1f49dd-b7ce-494a-9b4b-ac702d3664c5.json"),
    include_str!("../../../../tests/fixtures/be15e131-0753-4ceb-a36b-4f9172241080.json"),
    include_str!("../../../../tests/fixtures/66e8f7f8-3a6d-46ba-837c-b9713ddf7f40.json"),
    include_str!("../../../../tests/fixtures/39451b4d-cd7a-40da-b457-cb51b609173f.json"),
    include_str!("../../../../tests/fixtures/24882fa2-3fe9-4c1b-aa3d-0e6488b9db27.json"),
    include_str!("../../../../tests/fixtures/2fa763ad-4d94-4c3a-a099-13ac22c09ce4.json"),
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
        assert_eq!(mtg_contracts::aura_control_target_type(d), Some(kind));
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
            announce_paid(g, q, 0, target(g, 1));
            until_player(g, q, PlayerId(1));
            answer(
                g,
                q,
                Response::ActivateMana {
                    object: object(g, 2),
                },
            );
            assert_eq!(
                g.state.rules.players[1].mana.get(&Color::U),
                Some(&1.into())
            );
            resolve(g, q, |_| {});
            control(g, 1, 0, 2, 1);
            assert!(!g.current(permanent).unwrap().tapped);
            assert!(
                g.validate_permanent_mana_activation(PlayerId(1), permanent)
                    .is_err()
            );
            assert!(
                g.validate_permanent_mana_activation(PlayerId(0), permanent)
                    .is_ok()
            );
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
        // Only the new Equipment controller announces its real equip ability.
        equip(g, q, 1, 3);
        assert_eq!(
            g.current(equipment).unwrap().attached_to,
            Some(object(g, 3))
        );
        control(g, 2, 0, 2, 0);
        assert_eq!(g.characteristics(creature).unwrap().power, Some(2.into()));
    });
}

#[test]
fn typed_aura_control_artifact_creature_sickness_combat_and_actual_untap() {
    let mut rows = vec![("Steal Artifact", 0, 0, true), ("Silver Myr", 2, 1, false)];
    lands(&mut rows, "Island", 0, 4);
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
