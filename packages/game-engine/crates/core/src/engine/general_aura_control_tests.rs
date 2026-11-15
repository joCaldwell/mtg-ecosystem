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

fn prepared(rows: &[Row]) -> Prepared {
    let p = prepared_with_sources(rows, &[CONFISCATE]);
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
fn general_aura_control_paid_value_dependencies_noop_chain_reclaim_and_long_chain() {
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
                if scenario == "reclaim" { 0 } else { 2 },
                if scenario == "reclaim" { 0 } else { 2 },
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
                own_main(g, q, 2, turn);
                cast_paid(g, q, 2, target(g, 1));
                // A already applied: B initially leaves A's controller at 0.
                // C then B change both Auras to 2, but A is not applied twice.
                control(g, 0, 2, 0, 0);
                control(g, 1, 2, 0, 0);
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
