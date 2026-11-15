//! Private complete-source paid graphs; no catalog, profile or consumer admission.
//! Archive d705222b280a71cb, Confiscate physical 32251/sourceIndex 32249;
//! record SHA256 0f1e193c598d5f453cc11ded4c44dc40e8909d34e21d0ed0aba2dd518a2c202f.
//! CR20260925 109.5, 110.2, 301.5d, 303.4, 400.7, 608.3c, 613.8a-c,
//! 702.11, 704.3/5m, 800.4a-c; rules SHA256
//! 8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

const CONFISCATE: &str =
    include_str!("../../../../tests/fixtures/d599a38c-7719-443e-bb59-55bd43a8fee6.json");
const HEROIC: &str =
    include_str!("../../../../tests/fixtures/24882fa2-3fe9-4c1b-aa3d-0e6488b9db27.json");

fn prepared(rows: &[Row]) -> Prepared {
    let p = prepared_with_sources(rows, &[CONFISCATE, HEROIC]);
    let d = p
        .catalog
        .definitions
        .iter()
        .find(|d| d.name == "Confiscate")
        .unwrap();
    assert_eq!(
        mtg_contracts::aura_control_domain(d),
        Some(mtg_contracts::AuraControlDomain::Permanent)
    );
    assert_eq!(d.provenance.source_index, 32249);
    assert_eq!(d.characteristics.mana_cost.generic, 4.into());
    assert_eq!(d.characteristics.mana_cost.colored, [Color::U, Color::U]);
    p
}

// Float the exact colored cost with real public mana activations, then reuse
// the ordinary target/payment helper. No stack entry or receipt is synthesized.
fn announce_paid(g: &mut Game, q: u64, id: u32, targets: Vec<TargetEntity>) {
    let colors = g
        .definition(CardId(id))
        .unwrap()
        .characteristics
        .mana_cost
        .colored
        .clone();
    for color in colors {
        fund(
            g,
            q,
            match color {
                Color::U => "Island",
                Color::G => "Forest",
                Color::W => "Plains",
                Color::R => "Mountain",
                _ => panic!("witness color"),
            },
            1,
        );
    }
    announce(g, q, id, targets);
    let paid = g.state.rules.stack.last().unwrap().paid.as_ref().unwrap();
    let cost = &g.definition(CardId(id)).unwrap().characteristics.mana_cost;
    assert_eq!(paid.colored, cost.colored);
    assert_eq!(paid.generic, cost.generic);
}

fn cast_paid(g: &mut Game, q: u64, id: u32, targets: Vec<TargetEntity>) {
    announce_paid(g, q, id, targets);
    resolve(g, q, |_| {});
}

fn zone(g: &Game, id: u32, expected: Zone) {
    assert_eq!(g.current(object(g, id)).unwrap().zone, expected);
}

#[test]
fn general_aura_control_paid_atomic_entry_controls_aura_and_creature() {
    let mut rows = vec![
        ("Control Magic", 0, 0, true),
        ("Confiscate", 1, 1, true),
        // Explicit foreign-default setup, not a claim about a prior hand cast.
        ("Grizzly Bears", 3, 2, false),
    ];
    lands(&mut rows, "Island", 0, 4);
    lands(&mut rows, "Island", 1, 6);
    both(prepared(&rows), |g, q| {
        cast_paid(g, q, 0, target(g, 2));
        control(g, 2, 0, 3, 2);
        let turn = g.turn().number;
        own_main(g, q, 1, turn);
        let aura = object(g, 0);
        let creature = object(g, 2);
        let mut probe = g.fork();
        answer(&mut probe, q, Response::Cast { card: CardId(1) });
        let r = request(&mut probe, q);
        let Request::Targets { candidates, .. } = &r else {
            panic!("general permanent target")
        };
        let expected: Vec<_> = probe
            .state
            .rules
            .objects
            .values()
            .filter(|o| o.zone == Zone::Battlefield && probe.targetable_by(o.id, PlayerId(1)))
            .map(|o| o.id)
            .collect();
        assert_eq!(candidates, &expected);
        assert!(candidates.contains(&aura));
        let before = probe.state.clone();
        let trace = probe.trace.clone();
        for objects in [
            vec![],
            vec![object(&probe, 1)],
            vec![ObjectId {
                incarnation: aura.incarnation + 1,
                ..aura
            }],
            vec![aura, creature],
        ] {
            assert!(
                probe
                    .respond(r.player(), r.decision_id(), Response::Targets { objects })
                    .is_err()
            );
            assert_eq!(probe.state, before);
            assert_eq!(probe.trace, trace);
        }
        announce_paid(g, q, 1, target(g, 0));
        let stack = g.state.rules.stack.last().unwrap().clone();
        assert_eq!(stack.paid.as_ref().unwrap().generic, 4.into());
        assert_eq!(stack.paid.as_ref().unwrap().colored, [Color::U, Color::U]);
        recover(g);
        let mut entered = false;
        resolve(g, q, |g| {
            if g.current(object(g, 1)).unwrap().zone == Zone::Battlefield {
                entered = true;
                assert_eq!(g.current(object(g, 1)).unwrap().attached_to, Some(aura));
                assert_eq!(g.current(aura).unwrap().attached_to, Some(creature));
                control(g, 0, 1, 0, 0);
                control(g, 2, 1, 3, 2);
                visible(g, 1, 0, 1);
                visible(g, 0, 2, 1);
            }
        });
        assert!(entered);
        assert_eq!(g.state.rules.next_attachment_timestamp, 3);
        assert_eq!(
            g.state
                .rules
                .objects
                .values()
                .filter(|o| o.controller == PlayerId(1) && o.tapped)
                .count(),
            6
        );
        if g.trace.mode == TraceMode::Detailed {
            let entry = batches(g).into_iter().find(|b| matches!(b.cause, BatchCause::SpellCompletion { stack_id, .. } if stack_id == stack.id)).unwrap();
            assert_eq!(entry.occurrences.len(), 1);
            assert!(
                matches!(&entry.occurrences[0].actual, EventOutcome::EnteredAttached { movement, target, attachment_timestamp } if movement.previous == stack.source && movement.successor == object(g, 1) && *target == aura && *attachment_timestamp == 2)
            );
        }
    });
}

#[test]
fn general_aura_control_paid_dependency_chronology_and_removal_rederive_authority() {
    for b_before_c in [false, true] {
        let mut rows = vec![
            ("Control Magic", 0, 0, true), // A
            ("Confiscate", 2, 2, true),    // B
            ("Mind Control", 1, 1, true),  // C
            ("Grizzly Bears", 3, 3, false),
            ("Disperse", 1, 1, true),
            ("Disperse", 1, 1, true),
        ];
        lands(&mut rows, "Island", 0, 4);
        lands(&mut rows, "Island", 1, 9);
        lands(&mut rows, "Island", 2, 6);
        both(prepared(&rows), |g, q| {
            cast_paid(g, q, 0, target(g, 3));
            for (seat, source, target_id) in if b_before_c {
                [(2, 1, 0), (1, 2, 3)]
            } else {
                [(1, 2, 3), (2, 1, 0)]
            } {
                let turn = g.turn().number;
                own_main(g, q, seat, turn);
                cast_paid(g, q, source, target(g, target_id));
            }
            control(g, 0, 2, 0, 0);
            // A,C,B -> C,B,A; A,B,C -> B,A,C. CR 613.8a-c,
            // not recursively copying final source controllers after layering.
            control(g, 3, if b_before_c { 1 } else { 2 }, 3, 3);
            visible(g, 1, 0, 2);
            recover(g);
            until_player(g, q, PlayerId(1));
            cast_paid(g, q, 4, target(g, 1));
            control(g, 0, 0, 0, 0);
            control(g, 3, 1, 3, 3); // C still later than A once B leaves.
            zone(g, 1, Zone::Hand { owner: PlayerId(2) });
            cast_paid(g, q, 5, target(g, 2));
            control(g, 3, 0, 3, 3);
        });
    }
}

#[test]
fn general_aura_control_paid_noop_chain_reclaim_and_long_chain() {
    for scenario in ["noop", "reclaim", "long"] {
        let mut rows = vec![
            ("Control Magic", 0, 0, true),
            (
                "Confiscate",
                if scenario == "noop" { 0 } else { 1 },
                if scenario == "noop" { 0 } else { 1 },
                true,
            ),
            (
                "Confiscate",
                if scenario == "reclaim" {
                    0
                } else if scenario == "noop" {
                    1
                } else {
                    2
                },
                if scenario == "reclaim" {
                    0
                } else if scenario == "noop" {
                    1
                } else {
                    2
                },
                true,
            ),
            ("Confiscate", 3, 3, true),
            ("Grizzly Bears", 3, 3, false),
        ];
        for seat in 0..4 {
            lands(&mut rows, "Island", seat, 16);
        }
        both(prepared(&rows), |g, q| {
            cast_paid(g, q, 0, target(g, 4));
            if scenario == "noop" {
                cast_paid(g, q, 1, target(g, 0));
                let turn = g.turn().number;
                own_main(g, q, 1, turn);
                cast_paid(g, q, 2, target(g, 1));
                // CR-derived inference (not an official worked example):
                // compare the attempted assignment, not its target-state delta.
                // The middle Aura initially leaves the first Aura at 0, so
                // its attempted assignment to the creature is still 0.
                // First, outer, middle apply; do not reapply the first Aura.
                control(g, 0, 1, 0, 0);
                control(g, 1, 1, 0, 0);
                control(g, 4, 0, 3, 3);
            } else {
                let turn = g.turn().number;
                own_main(g, q, 1, turn);
                cast_paid(g, q, 1, target(g, 0));
                control(g, 4, 1, 3, 3);
                let turn = g.turn().number;
                let seat = if scenario == "reclaim" { 0 } else { 2 };
                own_main(g, q, seat, turn);
                cast_paid(
                    g,
                    q,
                    2,
                    target(g, if scenario == "reclaim" { 0 } else { 1 }),
                );
                if scenario == "reclaim" {
                    // B changes A to 1; reevaluation makes A wait for C.
                    // The owner reclaims its stolen Aura through an actual cast.
                    control(g, 0, 0, 0, 0);
                    control(g, 4, 0, 3, 3);
                    assert_eq!(
                        g.current(object(g, 1)).unwrap().attached_to,
                        Some(object(g, 0))
                    );
                    assert_eq!(
                        g.current(object(g, 2)).unwrap().attached_to,
                        Some(object(g, 0))
                    );
                } else {
                    let turn = g.turn().number;
                    own_main(g, q, 3, turn);
                    cast_paid(g, q, 3, target(g, 2));
                    for (id, owner) in [(0, 0), (1, 1), (2, 2), (3, 3), (4, 3)] {
                        control(g, id, 3, owner, owner);
                    }
                    for (aura, target_id) in [(0, 4), (1, 0), (2, 1), (3, 2)] {
                        visible(g, aura, target_id, 3);
                    }
                }
            }
            recover(g);
        });
    }
}

#[test]
fn general_aura_control_paid_removal_each_chain_level_and_successive_sba_waves() {
    for removed in 0..4 {
        for destroy in [false, true] {
            if removed == 3 && destroy {
                continue;
            } // Naturalize cannot target the creature.
            let mut rows = vec![
                ("Control Magic", 0, 0, true),
                ("Confiscate", 1, 1, true),
                ("Confiscate", 2, 2, true),
                ("Grizzly Bears", 3, 3, false),
                (if destroy { "Naturalize" } else { "Disperse" }, 2, 2, true),
            ];
            lands(&mut rows, "Island", 0, 4);
            lands(&mut rows, "Island", 1, 6);
            lands(&mut rows, "Island", 2, 8);
            lands(&mut rows, "Forest", 2, 2);
            both(prepared(&rows), |g, q| {
                cast_paid(g, q, 0, target(g, 3));
                let turn = g.turn().number;
                own_main(g, q, 1, turn);
                cast_paid(g, q, 1, target(g, 0));
                let turn = g.turn().number;
                own_main(g, q, 2, turn);
                cast_paid(g, q, 2, target(g, 1));
                control(g, 3, 2, 3, 3);
                let old: Vec<_> = (0..4).map(|id| object(g, id)).collect();
                announce_paid(g, q, 4, target(g, removed));
                let mut between_waves = false;
                resolve(g, q, |g| {
                    if (removed == 0 || removed == 3)
                        && g.current(old[1]).is_err()
                        && g.current(old[2]).is_ok_and(|o| o.zone == Zone::Battlefield)
                    {
                        between_waves = true;
                        assert_eq!(g.current(old[2]).unwrap().attached_to, Some(old[1]));
                        recover(g); // Authentic pause retaining the departed exact target.
                    }
                });
                if q == 1 && (removed == 0 || removed == 3) {
                    assert!(between_waves);
                }
                if removed == 3 {
                    assert!(g.current(old[3]).is_err());
                    zone(g, 3, Zone::Hand { owner: PlayerId(3) });
                    for id in 0..3 {
                        zone(
                            g,
                            id,
                            Zone::Graveyard {
                                owner: PlayerId(id as u8),
                            },
                        );
                        assert_eq!(g.current(object(g, id)).unwrap().attached_to, None);
                    }
                    assert_eq!(g.state.rules.next_attachment_timestamp, 4);
                    recover(g);
                    return;
                }
                control(
                    g,
                    3,
                    if removed == 2 {
                        1
                    } else if removed == 1 {
                        0
                    } else {
                        3
                    },
                    3,
                    3,
                );
                assert!(g.current(old[removed as usize]).is_err());
                zone(
                    g,
                    removed,
                    if destroy {
                        Zone::Graveyard {
                            owner: PlayerId(removed as u8),
                        }
                    } else {
                        Zone::Hand {
                            owner: PlayerId(removed as u8),
                        }
                    },
                );
                for id in (removed + 1)..3 {
                    zone(
                        g,
                        id,
                        Zone::Graveyard {
                            owner: PlayerId(id as u8),
                        },
                    );
                }
                for id in 0..removed {
                    assert_eq!(object(g, id), old[id as usize]);
                }
                assert_eq!(object(g, 3), old[3]);
                assert_eq!(g.state.rules.next_attachment_timestamp, 4);
                if removed == 0 && !destroy {
                    let turn = g.turn().number;
                    own_main(g, q, 0, turn);
                    cast_paid(g, q, 0, target(g, 3));
                    assert_ne!(object(g, 0), old[0]);
                    assert_eq!(
                        g.current(object(g, 0)).unwrap().attachment_timestamp,
                        Some(4)
                    );
                    control(g, 3, 0, 3, 3);
                    for id in [1, 2] {
                        assert_eq!(g.current(object(g, id)).unwrap().attached_to, None);
                    }
                }
            });
        }
    }
}

#[test]
fn general_aura_control_paid_equipment_authority_and_pacifism_restriction() {
    for equipment in [false, true] {
        let mut rows = vec![
            (
                if equipment {
                    "Bonesplitter"
                } else {
                    "Pacifism"
                },
                0,
                0,
                !equipment,
            ),
            ("Confiscate", 1, 1, true),
            ("Grizzly Bears", 0, 0, false),
            ("Grizzly Bears", 1, 1, false),
        ];
        lands(&mut rows, "Island", 0, 2);
        lands(&mut rows, "Plains", 0, 2);
        lands(&mut rows, "Island", 1, 8);
        both(prepared(&rows), |g, q| {
            if equipment {
                equip(g, q, 0, 2);
            } else {
                cast_paid(g, q, 0, target(g, 2));
            }
            let creature = g.current(object(g, 2)).unwrap().clone();
            let timestamp = g.current(object(g, 0)).unwrap().attachment_timestamp;
            let turn = g.turn().number;
            own_main(g, q, 1, turn);
            cast_paid(g, q, 1, target(g, 0));
            control(g, 0, 1, 0, 0);
            control(g, 2, 0, 0, 0);
            assert_eq!(
                g.current(object(g, 0)).unwrap().attached_to,
                Some(creature.id)
            );
            assert_eq!(
                g.current(object(g, 0)).unwrap().attachment_timestamp,
                timestamp
            );
            visible(g, 0, 2, 0);
            if equipment {
                // New controller can pay the real equip activation on its own
                // main phase; the equipped creature never follows control.
                let mut former = g.fork();
                let turn = former.turn().number;
                own_main(&mut former, q, 0, turn);
                let r = request(&mut former, q);
                let source = object(&former, 0);
                let before = former.state.clone();
                let trace = former.trace.clone();
                assert!(
                    former
                        .respond(
                            PlayerId(0),
                            r.decision_id(),
                            Response::ActivateAbility {
                                object: source,
                                ability: 0
                            }
                        )
                        .is_err()
                );
                assert_eq!(former.state, before);
                assert_eq!(former.trace, trace);
                equip(g, q, 0, 3);
                control(g, 2, 0, 0, 0);
                control(g, 3, 1, 1, 1);
                assert_eq!(
                    g.characteristics(object(g, 2)).unwrap().power,
                    Some(2.into())
                );
                assert_eq!(
                    g.characteristics(object(g, 3)).unwrap().power,
                    Some(4.into())
                );
                assert_eq!(
                    g.current(object(g, 1)).unwrap().attached_to,
                    Some(object(g, 0))
                );
            } else {
                assert!(g.has_restriction(object(g, 2), CreatureRestriction::CantAttackOrBlock));
                assert!(!g.creatures(PlayerId(0), true).contains(&object(g, 2)));
            }
        });
    }
}

// Continue a public target-validation receipt through genuine mana/spending
// requests, stopping at priority so an opposing legality change can intervene.
fn pay_funded(g: &mut Game, q: u64) {
    assert!(matches!(request(g, q), Request::Mana { .. }));
    answer(g, q, Response::FinishMana { undo_mana: false });
    let Request::SpendMana {
        cost, available, ..
    } = request(g, q)
    else {
        panic!("funded payment")
    };
    let mut rest = available;
    let mut spending = BTreeMap::<Color, Quantity>::new();
    for color in cost.colored {
        rest.get_mut(&color).unwrap().subtract(&1.into());
        spending.entry(color).or_default().add(&1.into());
    }
    let mut generic = cost.generic;
    for (color, n) in rest {
        let used = n.min(generic.clone());
        generic.subtract(&used);
        if used > Quantity::zero() {
            spending.entry(color).or_default().add(&used);
        }
    }
    assert_eq!(generic, Quantity::zero());
    answer(g, q, Response::SpendMana { spending });
    assert!(matches!(request(g, q), Request::Priority { .. }));
    assert!(g.state.rules.stack.last().unwrap().paid.is_some());
}

#[test]
fn general_aura_control_paid_hexproof_current_controller_and_private_stale_queries() {
    let mut rows = vec![
        ("Control Magic", 0, 0, true),
        ("Confiscate", 1, 1, true),
        ("Grizzly Bears", 2, 2, false),
        ("Heroic Intervention", 1, 1, true),
        ("Disperse", 1, 1, true),
        ("Disperse", 1, 1, true),
        ("Disperse", 0, 0, true),
    ];
    lands(&mut rows, "Island", 0, 4);
    lands(&mut rows, "Island", 1, 14);
    lands(&mut rows, "Forest", 1, 2);
    both(prepared(&rows), |g, q| {
        cast_paid(g, q, 0, target(g, 2));
        let turn = g.turn().number;
        own_main(g, q, 1, turn);
        cast_paid(g, q, 1, target(g, 0));
        cast_paid(g, q, 3, vec![]);
        let aura = object(g, 0);
        let creature = object(g, 2);
        for id in [aura, creature] {
            assert!(g.has_keyword(id, KeywordAbility::Hexproof));
            assert!(g.targetable_by(id, PlayerId(1)));
            assert!(!g.targetable_by(id, PlayerId(0)));
        }
        assert_eq!(g.current(aura).unwrap().attached_to, Some(creature));
        // Existing attachment persists; a new opposing target is forbidden.
        let mut opposing = g.fork();
        until_player(&mut opposing, q, PlayerId(0));
        answer(&mut opposing, q, Response::Cast { card: CardId(6) });
        let r = request(&mut opposing, q);
        assert!(matches!(&r, Request::Targets { candidates, .. } if !candidates.contains(&aura)));
        let before_opposing = opposing.state.clone();
        let trace_opposing = opposing.trace.clone();
        assert!(
            opposing
                .respond(
                    r.player(),
                    r.decision_id(),
                    Response::Targets {
                        objects: vec![aura]
                    }
                )
                .is_err()
        );
        assert_eq!(opposing.state, before_opposing);
        assert_eq!(opposing.trace, trace_opposing);
        fund(g, q, "Island", 2);
        answer(g, q, Response::Cast { card: CardId(4) });
        assert!(matches!(request(g, q), Request::Targets { .. }));
        let before = g.state.clone();
        let trace = g.trace.clone();
        let QueryValue::Decision(Some(PlayerRequest::Targets { token, .. })) =
            finish_query(g, g.player_decision_for(PlayerId(1), Budget::new(0)))
        else {
            panic!("own private target query")
        };
        for seat in [0, 2, 3] {
            assert!(matches!(
                finish_query(g, g.player_decision_for(PlayerId(seat), Budget::new(0))),
                QueryValue::Decision(None)
            ));
        }
        let response = PlayerResponse::Targets {
            objects: vec![g.handle_for(PlayerId(1), aura).unwrap()],
        };
        let QueryValue::Validated(receipt) = finish_query(
            g,
            g.check_player_response(PlayerId(1), token.clone(), response.clone(), Budget::new(0)),
        ) else {
            panic!("public receipt")
        };
        let QueryValue::Validated(duplicate) = finish_query(
            g,
            g.check_player_response(PlayerId(1), token, response, Budget::new(0)),
        ) else {
            panic!("duplicate receipt")
        };
        let QueryResult::Incomplete { work: stale, .. } = g.observe(PlayerId(1), Budget::new(0))
        else {
            panic!("suspended private work")
        };
        for seat in 0..4 {
            let QueryValue::Observation(o) =
                finish_query(g, g.observe(PlayerId(seat), Budget::new(0)))
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
        // Bouncing the outer Aura returns the current controller of both
        // captured recipients to 0. Their captured protection does not end.
        cast_paid(g, q, 5, target(g, 1));
        control(g, 0, 0, 0, 0);
        control(g, 2, 0, 2, 2);
        zone(g, 1, Zone::Hand { owner: PlayerId(1) });
        zone(g, 4, Zone::Graveyard { owner: PlayerId(1) });
        for id in [aura, creature] {
            assert!(g.has_keyword(id, KeywordAbility::Hexproof));
            assert!(g.targetable_by(id, PlayerId(0)));
            assert!(!g.targetable_by(id, PlayerId(1)));
        }
        assert_eq!(g.current(aura).unwrap().attached_to, Some(creature));
        if g.trace.mode == TraceMode::Detailed {
            assert!(g.events().iter().any(|e| matches!(e, Event::SpellCompleted { stack_id, resolved: false } if *stack_id == paid)));
        }
        let before = g.state.clone();
        let trace = g.trace.clone();
        assert!(g.submit(duplicate).is_err());
        assert!(matches!(
            g.resume_work(stale, Budget::unlimited()),
            QueryResult::Rejected(QueryRejection::StaleOrForeignWork)
        ));
        assert_eq!(g.state, before);
        assert_eq!(g.trace, trace);
        let turn = g.turn().number;
        own_main(g, q, 0, turn);
        for id in [aura, creature] {
            assert!(!g.has_keyword(id, KeywordAbility::Hexproof));
        }
        visible(g, 0, 2, 0);
    });
}

#[test]
fn general_aura_control_paid_departures_and_common_prior_simultaneous_wipe() {
    for scenario in [
        "source_owner",
        "middle_owner",
        "outer_owner",
        "target_owner",
        "dead_default",
        "losses",
        "wipe_source_first",
        "wipe_target_first",
    ] {
        let mut rows = vec![
            ("Control Magic", 0, 0, true),
            ("Confiscate", 1, 1, true),
            ("Confiscate", 2, 2, true),
            // Foreign default controller is explicit scenario setup, not a
            // purported owner-hand casting history.
            ("Grizzly Bears", 3, 2, false),
            ("Planar Cleansing", 2, 2, true),
            ("Disperse", 0, 0, true),
            ("Disperse", 0, 0, true),
            ("Earthquake", 2, 2, true),
        ];
        let (a, creature) = if scenario == "wipe_target_first" {
            rows.swap(0, 3);
            (3, 0)
        } else {
            (0, 3)
        };
        lands(&mut rows, "Island", 0, 8);
        lands(&mut rows, "Island", 1, 6);
        lands(&mut rows, "Island", 2, 6);
        lands(&mut rows, "Plains", 2, 6);
        lands(&mut rows, "Mountain", 2, 2);
        let mut p = prepared(&rows);
        if scenario == "losses" {
            p.config.initial.life = [1.into(), 1.into(), 40.into(), 40.into()];
            p.profile_digest = digest(&p.config).unwrap();
        }
        both(p, |g, q| {
            cast_paid(g, q, a, target(g, creature));
            let turn = g.turn().number;
            own_main(g, q, 1, turn);
            cast_paid(g, q, 1, target(g, a));
            let turn = g.turn().number;
            own_main(g, q, 2, turn);
            cast_paid(g, q, 2, target(g, 1));
            control(g, creature, 2, 3, 2);
            let prior: Vec<_> = [a, 1, 2, creature].map(|id| object(g, id)).into();
            recover(g);
            match scenario {
                "source_owner" => {
                    g.concede(PlayerId(0)).unwrap();
                    request(g, q);
                    assert!(g.object_id(CardId(a)).is_none());
                    zone(g, 1, Zone::Graveyard { owner: PlayerId(1) });
                    zone(g, 2, Zone::Graveyard { owner: PlayerId(2) });
                    control(g, creature, 2, 3, 2);
                }
                "middle_owner" => {
                    g.concede(PlayerId(1)).unwrap();
                    request(g, q);
                    assert!(g.object_id(CardId(1)).is_none());
                    zone(g, 2, Zone::Graveyard { owner: PlayerId(2) });
                    control(g, a, 0, 0, 0);
                    control(g, creature, 0, 3, 2);
                }
                "outer_owner" | "dead_default" => {
                    g.concede(PlayerId(2)).unwrap();
                    request(g, q);
                    assert!(g.object_id(CardId(2)).is_none());
                    control(g, a, 1, 0, 0);
                    control(g, creature, 1, 3, 2);
                    if scenario == "dead_default" {
                        until_player(g, q, PlayerId(0));
                        cast_paid(g, q, 5, target(g, 1));
                        control(g, creature, 0, 3, 2);
                        cast_paid(g, q, 6, target(g, a));
                        assert!(g.current(prior[3]).is_err());
                        zone(g, creature, Zone::Exile);
                    }
                }
                "target_owner" => {
                    g.concede(PlayerId(3)).unwrap();
                    request(g, q);
                    assert!(g.object_id(CardId(creature)).is_none());
                    for (id, owner) in [(a, 0), (1, 1), (2, 2)] {
                        zone(
                            g,
                            id,
                            Zone::Graveyard {
                                owner: PlayerId(owner),
                            },
                        );
                    }
                }
                "losses" => {
                    fund(g, q, "Mountain", 2);
                    answer(g, q, Response::Cast { card: CardId(7) });
                    answer(g, q, Response::AnnounceX { value: 1.into() });
                    super::super::equipped_indestructible_tests::pay_and_resolve(g, q, vec![]);
                    assert!(!g.is_live(PlayerId(0)));
                    assert!(!g.is_live(PlayerId(1)));
                    assert!(g.object_id(CardId(a)).is_none());
                    assert!(g.object_id(CardId(1)).is_none());
                    zone(g, 2, Zone::Graveyard { owner: PlayerId(2) });
                    control(g, creature, 2, 3, 2);
                    if g.trace.mode == TraceMode::Detailed {
                        assert!(g.events().iter().any(|e| matches!(e, Event::PlayersLost { losses } if losses.iter().map(|(p, _)| *p).collect::<Vec<_>>() == vec![PlayerId(0), PlayerId(1)])));
                    }
                }
                _ => {
                    let start = g.events().len();
                    cast_paid(g, q, 4, vec![]);
                    for (id, owner) in [(a, 0), (1, 1), (2, 2), (creature, 3)] {
                        zone(
                            g,
                            id,
                            Zone::Graveyard {
                                owner: PlayerId(owner),
                            },
                        );
                    }
                    if g.trace.mode == TraceMode::Detailed {
                        assert!(!g.events().iter().skip(start).any(|e| matches!(e, Event::ControlChanged { object, .. } if prior.contains(object))));
                        let wipe = batches(g).into_iter().find(|b| b.occurrences.iter().any(|o| matches!(&o.actual, EventOutcome::Moved { movement } if movement.previous == prior[3]))).unwrap();
                        assert!(wipe.occurrences.len() >= 4);
                        for occurrence in &wipe.occurrences {
                            let authority = occurrence.before.control.as_ref().unwrap();
                            for object in &prior {
                                assert_eq!(
                                    authority
                                        .objects
                                        .iter()
                                        .find(|o| o.object == *object)
                                        .unwrap()
                                        .controller,
                                    PlayerId(2)
                                );
                            }
                        }
                    }
                }
            }
        });
    }
}

#[test]
fn general_aura_control_paid_normalized_structural_forgery_fails_authenticated_replay() {
    let mut rows = vec![
        ("Control Magic", 0, 0, true),
        ("Confiscate", 2, 2, true),
        ("Mind Control", 1, 1, true),
        ("Grizzly Bears", 3, 3, false),
        ("Grizzly Bears", 3, 3, false),
    ];
    lands(&mut rows, "Island", 0, 4);
    lands(&mut rows, "Island", 1, 5);
    lands(&mut rows, "Island", 2, 6);
    both(prepared(&rows), |g, q| {
        cast_paid(g, q, 0, target(g, 3));
        let turn = g.turn().number;
        own_main(g, q, 1, turn);
        cast_paid(g, q, 2, target(g, 3));
        let turn = g.turn().number;
        own_main(g, q, 2, turn);
        cast_paid(g, q, 1, target(g, 0));
        control(g, 3, 2, 3, 3); // A,C,B -> C,B,A.
        recover(g);
        let wire = g.snapshot().unwrap();
        for variant in 0..3 {
            let mut bad: Snapshot = strict_read(&wire).unwrap();
            match variant {
                0 => {
                    // Forge A,B,C and its coherent derived controller.
                    bad.state
                        .rules
                        .objects
                        .get_mut(&CardId(1))
                        .unwrap()
                        .attachment_timestamp = Some(2);
                    bad.state
                        .rules
                        .objects
                        .get_mut(&CardId(2))
                        .unwrap()
                        .attachment_timestamp = Some(3);
                    bad.state
                        .rules
                        .objects
                        .get_mut(&CardId(3))
                        .unwrap()
                        .controller = PlayerId(1);
                }
                1 => {
                    // Move both creature Auras, preserving the chain's valid
                    // domains and a coherent controller for each real creature.
                    for id in [0, 2] {
                        bad.state
                            .rules
                            .objects
                            .get_mut(&CardId(id))
                            .unwrap()
                            .attached_to = Some(object(g, 4));
                    }
                    bad.state
                        .rules
                        .objects
                        .get_mut(&CardId(3))
                        .unwrap()
                        .controller = PlayerId(3);
                    bad.state
                        .rules
                        .objects
                        .get_mut(&CardId(4))
                        .unwrap()
                        .controller = PlayerId(2);
                    bad.state
                        .rules
                        .objects
                        .get_mut(&CardId(4))
                        .unwrap()
                        .attack_ready = false;
                }
                _ => {
                    // A structurally legal different dependency: B controls C
                    // instead of A. Both attempted grants then give seat 2 the
                    // creature, so this changes attachment history nontrivially.
                    bad.state
                        .rules
                        .objects
                        .get_mut(&CardId(1))
                        .unwrap()
                        .attached_to = Some(object(g, 2));
                    bad.state
                        .rules
                        .objects
                        .get_mut(&CardId(0))
                        .unwrap()
                        .controller = PlayerId(0);
                    bad.state
                        .rules
                        .objects
                        .get_mut(&CardId(2))
                        .unwrap()
                        .controller = PlayerId(2);
                }
            }
            assert_ne!(bad.state, g.state);
            let mut structural = g.fork();
            structural.state = bad.state.clone();
            structural.validate_state().unwrap();
            bad.state_sha256 = digest(&bad.state).unwrap();
            assert!(
                Game::restore(g.prepared.as_ref().clone(), &canonical(&bad).unwrap()).is_err(),
                "normalized historically false dependency {variant}"
            );
        }
    });
}
