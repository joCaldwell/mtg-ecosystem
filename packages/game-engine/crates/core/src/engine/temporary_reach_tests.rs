//! Exact paid Snare/Tower witnesses through existing combat/effect APIs.
//! CR702.9/702.17/611.2c/514.2; https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
const SNARE: &str =
    include_str!("../../../../tests/fixtures/7d011e22-335b-465f-9111-e95a72ebffe9.json");
const TOWER: &str =
    include_str!("../../../../tests/fixtures/58853200-efbe-41cc-87f1-720841308f4d.json");
fn prepared(rows: &[Row]) -> Prepared {
    prepared_with_sources(rows, &[SNARE, TOWER])
}
fn through_cleanup(g: &mut Game, q: u64) {
    let turn = g.turn().number;
    for _ in 0..300 {
        let r = request(g, q);
        if g.turn().number > turn {
            return;
        }
        answer(g, q, pass(&r));
    }
    panic!("cleanup")
}
#[test]
fn temporary_reach_paid_snare_changes_real_flying_block_and_damage() {
    for grant in [false, true] {
        both(
            prepared(&[
                ("Snare the Skies", 1, 1, true),
                ("Storm Crow", 0, 0, false),
                ("Grizzly Bears", 1, 1, false),
                ("Forest", 1, 1, false),
            ]),
            |g, q| {
                if grant {
                    until_player(g, q, PlayerId(1));
                    cast(g, q, 0, target(g, 2));
                    assert!(g.has_keyword(object(g, 2), KeywordAbility::Reach));
                    assert_eq!(g.state.rules.effects.len(), 1);
                }
                for _ in 0..100 {
                    let r = request(g, q);
                    if matches!(r, Request::Attackers { .. }) {
                        break;
                    }
                    answer(g, q, pass(&r));
                }
                answer(
                    g,
                    q,
                    Response::Attackers {
                        attacks: vec![(object(g, 1), PlayerId(1))],
                    },
                );
                for _ in 0..100 {
                    let r = request(g, q);
                    if matches!(
                        r,
                        Request::Blockers {
                            player: PlayerId(1),
                            ..
                        }
                    ) {
                        break;
                    }
                    answer(g, q, pass(&r));
                }
                let QueryValue::Observation(o) =
                    finish_query(g, g.observe(PlayerId(1), Budget::new(0)))
                else {
                    panic!("observation")
                };
                let Some(PlayerRequest::Blockers {
                    legal_attackers, ..
                }) = o.decision
                else {
                    panic!("block hints")
                };
                assert_eq!(legal_attackers.len(), 1);
                assert_eq!(legal_attackers[0].attackers.len(), usize::from(grant));
                let r = request(g, q);
                let before = g.state.clone();
                let trace = g.trace.clone();
                let result = g.respond(
                    r.player(),
                    r.decision_id(),
                    Response::Blockers {
                        blocks: vec![(object(g, 2), object(g, 1))],
                    },
                );
                assert_eq!(result.is_ok(), grant);
                if !grant {
                    assert_eq!(g.state, before);
                    assert_eq!(g.trace, trace);
                    answer(g, q, Response::Blockers { blocks: vec![] });
                }
                recover(g);
                through_cleanup(g, q);
                assert_eq!(
                    g.state.rules.players[1].life,
                    Quantity::from(if grant { 40 } else { 39 })
                );
                assert!(!g.has_keyword(object(g, 2), KeywordAbility::Reach));
                assert_eq!(
                    g.characteristics(object(g, 2)).unwrap().toughness,
                    Some(2.into())
                );
                if grant {
                    assert!(matches!(
                        g.current(object(g, 1)).unwrap().zone,
                        Zone::Graveyard { .. }
                    ));
                }
            },
        );
    }
}
#[test]
fn temporary_reach_paid_tower_captures_only_existing_controlled_creatures() {
    both(
        prepared(&[
            ("Tower Defense", 0, 0, true),
            ("Grizzly Bears", 0, 0, false),
            ("Grizzly Bears", 0, 0, true),
            ("Grizzly Bears", 1, 1, false),
            ("Forest", 0, 0, false),
            ("Forest", 0, 0, false),
            ("Forest", 0, 0, false),
            ("Forest", 0, 0, false),
        ]),
        |g, q| {
            cast(g, q, 0, vec![]);
            assert!(g.has_keyword(object(g, 1), KeywordAbility::Reach));
            assert!(!g.has_keyword(object(g, 3), KeywordAbility::Reach));
            assert_eq!(
                g.characteristics(object(g, 1)).unwrap().toughness,
                Some(7.into())
            );
            assert_eq!(g.state.rules.effects.len(), 1);
            for viewer in 0..4 {
                let e = g.visible_effects(PlayerId(viewer)).unwrap();
                assert!(
                    matches!(&e[0],VisibleEffect::GrantKeywordsSet{keywords,..}if keywords==&[KeywordAbility::Reach])
                );
            }
            until_player(g, q, PlayerId(0));
            cast(g, q, 2, vec![]);
            assert!(!g.has_keyword(object(g, 2), KeywordAbility::Reach));
            assert_eq!(
                g.characteristics(object(g, 2)).unwrap().toughness,
                Some(2.into())
            );
            recover(g);
            through_cleanup(g, q);
            assert!(!g.has_keyword(object(g, 1), KeywordAbility::Reach));
            assert!(g.state.rules.effects.is_empty());
        },
    );
}
#[test]
fn temporary_reach_bounce_reentry_and_effect_forgery_do_not_rebind() {
    both(
        prepared(&[
            ("Snare the Skies", 0, 0, true),
            ("Grizzly Bears", 0, 0, false),
            ("Unsummon", 0, 0, true),
            ("Forest", 0, 0, false),
            ("Island", 0, 0, false),
            ("Forest", 0, 0, false),
            ("Forest", 0, 0, false),
        ]),
        |g, q| {
            cast(g, q, 0, target(g, 1));
            let old = object(g, 1);
            let mut bad: Snapshot = strict_read(&g.snapshot().unwrap()).unwrap();
            let ResolvedEffectKind::GrantKeywords { keywords, .. } =
                &mut bad.state.rules.effects[0].kind
            else {
                panic!("effect")
            };
            *keywords = vec![KeywordAbility::Flying];
            bad.state_sha256 = digest(&bad.state).unwrap();
            assert!(Game::restore(g.prepared.as_ref().clone(), &canonical(&bad).unwrap()).is_err());
            until_player(g, q, PlayerId(0));
            cast(g, q, 2, target(g, 1));
            until_player(g, q, PlayerId(0));
            cast(g, q, 1, vec![]);
            assert_ne!(object(g, 1), old);
            assert!(!g.has_keyword(object(g, 1), KeywordAbility::Reach));
            assert_eq!(
                g.characteristics(object(g, 1)).unwrap().power,
                Some(2.into())
            );
            recover(g);
            through_cleanup(g, q);
            assert!(g.state.rules.effects.is_empty());
        },
    );
}
