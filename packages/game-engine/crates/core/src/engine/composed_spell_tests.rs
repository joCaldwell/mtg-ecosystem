//! Paid, private scenarios from the exact frozen complete sources.
//! CR 109.5, 400.7, 608.2b/c/h, 611.2c, 702.26:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::aura_control_tests::prepared_with_sources;
use super::equipped_indestructible_tests::{
    answer, both, cast, object, pay_to_priority, recover, request,
};
use super::temporary_indestructible_tests::{announce, pass, target};
use super::*;

const GIFT: &str =
    include_str!("../../../../tests/fixtures/2cb95721-56c8-4008-8055-579a5a97fc8b.json");
const SLIP: &str =
    include_str!("../../../../tests/fixtures/82506e51-3fe1-446f-83c9-69e67583cefc.json");
const PROMOTION: &str =
    include_str!("../../../../tests/fixtures/303d74d0-84db-4694-bb1a-470cf470bc4f.json");

fn cursor(g: &mut Game, expected: u32) {
    for _ in 0..100 {
        if matches!(&g.state.rules.frame, Frame::Resolve { frame } if frame.cursor == expected) {
            assert!(g.state.rules.pending.is_none());
            return;
        }
        match g.advance(Budget::new(1)) {
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(r @ Request::Priority { .. }) if expected == 0 => {
                g.respond(r.player(), r.decision_id(), Response::Pass)
                    .unwrap();
            }
            other => panic!("resolution cursor {expected}: {other:?}"),
        }
    }
    panic!("resolution did not hit cursor {expected}");
}

#[test]
fn composed_spell_paid_counter_and_grant_have_distinct_atomic_boundaries_and_exact_lki() {
    both(
        prepared_with_sources(
            &[
                ("Gaea's Gift", 0, 0, true),
                ("Grizzly Bears", 1, 0, false),
                ("Grizzly Bears", 1, 1, false),
                ("Forest", 0, 0, false),
                ("Plains", 0, 0, false),
                ("Plains", 0, 0, false),
                ("Swords to Plowshares", 0, 0, true),
            ],
            &[GIFT],
        ),
        |g, q| {
            let bear = object(g, 1);
            let opponent = object(g, 2);
            answer(g, q, Response::Cast { card: CardId(0) });
            let r = request(g, q);
            let before = g.state.clone();
            assert!(
                g.respond(
                    r.player(),
                    r.decision_id(),
                    Response::Targets {
                        objects: vec![opponent],
                    }
                )
                .is_err()
            );
            assert_eq!(before, g.state);
            pay_to_priority(g, q, target(g, 1));

            cursor(g, 0);
            assert_eq!(g.current(bear).unwrap().plus_one_counters, 0.into());
            cursor(g, 1);
            assert_eq!(g.current(bear).unwrap().plus_one_counters, 1.into());
            assert!(g.current_keywords(bear).is_empty());
            recover(g);

            cursor(g, 2);
            let grant = g.state.rules.effects.last().unwrap();
            assert_eq!(grant.origin.cursor, 1);
            assert!(matches!(&grant.kind, ResolvedEffectKind::GrantKeywords {
                target, modifier: None, keywords,
            } if *target == bear && keywords == &[
                KeywordAbility::Reach, KeywordAbility::Trample, KeywordAbility::Hexproof,
                KeywordAbility::Indestructible,
            ]));
            assert_eq!(g.characteristics(bear).unwrap().power, Some(3.into()));
            recover(g);
            assert!(matches!(
                request(g, q),
                Request::Priority {
                    player: PlayerId(0),
                    ..
                }
            ));

            // Swords reads the last battlefield appearance after exile. The
            // borrowed target's controller gains the modified power, not its owner.
            cast(g, q, 6, target(g, 1));
            let successor = object(g, 1);
            assert_ne!(successor, bear);
            assert_eq!(g.current(successor).unwrap().zone, Zone::Exile);
            assert_eq!(g.state.rules.players[0].life, 43.into());
            assert_eq!(g.state.rules.players[1].life, 40.into());
            assert_eq!(g.current(opponent).unwrap().plus_one_counters, 0.into());
        },
    );
}

#[test]
fn composed_spell_paid_phasing_keeps_attachment_and_incarnation_through_recovery() {
    both(
        prepared_with_sources(
            &[
                ("Slip Out the Back", 0, 0, true),
                ("Grizzly Bears", 0, 0, false),
                ("Rancor", 0, 0, true),
                ("Forest", 0, 0, false),
                ("Island", 0, 0, false),
            ],
            &[SLIP],
        ),
        |g, q| {
            cast(g, q, 2, target(g, 1));
            let bear = object(g, 1);
            let aura = object(g, 2);
            assert_eq!(g.current(aura).unwrap().attached_to, Some(bear));
            announce(g, q, 0, target(g, 1));

            cursor(g, 0);
            cursor(g, 1);
            assert_eq!(g.current(bear).unwrap().plus_one_counters, 1.into());
            assert!(g.current(aura).unwrap().present_on_battlefield());
            recover(g);
            cursor(g, 2);
            let phased = g.current(bear).unwrap();
            assert!(phased.phased_out.is_some());
            assert_eq!(phased.plus_one_counters, 1.into());
            assert_eq!(g.current(aura).unwrap().attached_to, Some(bear));
            assert!(g.current(aura).unwrap().phased_out.is_some());
            recover(g);

            for _ in 0..500 {
                let r = request(g, q);
                if g.turn().number > 0 && g.current(bear).unwrap().present_on_battlefield() {
                    assert_eq!(object(g, 1), bear);
                    assert_eq!(object(g, 2), aura);
                    assert_eq!(g.current(aura).unwrap().attached_to, Some(bear));
                    assert_eq!(g.characteristics(bear).unwrap().power, Some(5.into()));
                    return;
                }
                g.respond(r.player(), r.decision_id(), pass(&r)).unwrap();
            }
            panic!("controller untap did not phase in the same permanents");
        },
    );
}

#[test]
fn composed_spell_all_illegal_skips_the_controller_life_rider() {
    for leaves in [false, true] {
        both(
            prepared_with_sources(
                &[
                    ("Battlefield Promotion", 0, 0, true),
                    ("Grizzly Bears", 1, 1, false),
                    ("Plains", 0, 0, false),
                    ("Plains", 0, 0, false),
                ],
                &[PROMOTION],
            ),
            |g, q| {
                let bear = object(g, 1);
                announce(g, q, 0, target(g, 1));
                if leaves {
                    g.concede(PlayerId(1)).unwrap();
                }
                for _ in 0..10 {
                    let r = request(g, q);
                    if g.state.rules.stack.is_empty() {
                        assert_eq!(
                            g.state.rules.players[0].life,
                            if leaves { 40.into() } else { 42.into() }
                        );
                        if !leaves {
                            assert_eq!(g.current(bear).unwrap().plus_one_counters, 1.into());
                            assert!(g.has_keyword(bear, KeywordAbility::FirstStrike));
                        }
                        return;
                    }
                    g.respond(r.player(), r.decision_id(), Response::Pass)
                        .unwrap();
                }
                panic!("ordinary spell did not finish");
            },
        );
    }
}
