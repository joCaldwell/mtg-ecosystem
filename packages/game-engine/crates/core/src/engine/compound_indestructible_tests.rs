//! Private complete-source casts. No catalog/profile/consumer admission.
//! CR 608.2b/c,611.2a/c,613.1f,400.7,514.2,704.3,702.12:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::equipped_indestructible_tests::{answer, both, cast, equip, object, recover, request};
use super::temporary_indestructible_tests::{
    announce, pass, target, through_cleanup, until_player,
};
use super::*;
fn prepared(rows: &[(&str, bool)]) -> Prepared {
    let placeholders: Vec<_> = rows
        .iter()
        .map(|(n, h)| {
            (
                if *n == "Mortal's Resolve" {
                    "Giant Growth"
                } else {
                    *n
                },
                *h,
            )
        })
        .collect();
    let mut p = equipped_indestructible_tests::prepared(&placeholders, false);
    let mut c = (*p.catalog).clone();
    let mtg_compiler::CompileResult::Lowered { definition: d } = mtg_compiler::compile(
        include_str!("../../../../tests/fixtures/archive-manifest-current.json"),
        include_str!("../../../../tests/fixtures/ef29f979-1a6d-47bb-83f5-4679389471da.json"),
    ) else {
        panic!("complete archived Mortal's Resolve")
    };
    for (i, (n, _)) in rows.iter().enumerate() {
        if *n == "Mortal's Resolve" {
            p.config.cards[i].oracle_id = d.oracle_id.clone();
        }
    }
    c.definitions.push(*d);
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    p.catalog_digest = digest(&c).unwrap();
    p.catalog = Arc::new(c);
    p.profile_digest = digest(&p.config).unwrap();
    p
}
fn protected(g: &Game, id: u32) -> bool {
    g.has_keyword(object(g, id), KeywordAbility::Indestructible)
}
fn boosted(g: &Game, id: u32) {
    assert_eq!(
        g.derived_characteristics(object(g, id))
            .unwrap()
            .power
            .unwrap(),
        3.into()
    );
    assert_eq!(
        g.derived_characteristics(object(g, id))
            .unwrap()
            .toughness
            .unwrap(),
        3.into()
    );
    assert!(protected(g, id));
    assert_eq!(
        g.current_keywords(object(g, id))
            .iter()
            .filter(|k| **k == KeywordAbility::Indestructible)
            .count(),
        1
    );
    assert_eq!(g.state.rules.effects.len(), 2);
    assert!(
        matches!(&g.state.rules.effects[0].kind,ResolvedEffectKind::ModifyPowerToughness {target,..} if *target==object(g,id))
    );
    assert!(
        matches!(&g.state.rules.effects[1].kind,ResolvedEffectKind::GrantKeywords {target,..} if *target==object(g,id))
    );
}
#[test]
fn compound_indestructible_paid_response_to_destroy_source_departure_and_union() {
    both(
        prepared(&[
            ("Doom Blade", true),
            ("Grizzly Bears", false),
            ("Mortal's Resolve", true),
            ("Swamp", false),
            ("Swamp", false),
            ("Forest", false),
            ("Forest", false),
        ]),
        |g, q| {
            announce(g, q, 0, target(g, 1));
            cast(g, q, 2, target(g, 1));
            boosted(g, 1);
            assert_eq!(g.current(object(g, 1)).unwrap().zone, Zone::Battlefield);
            assert!(matches!(
                g.current(object(g, 2)).unwrap().zone,
                Zone::Graveyard { .. }
            ));
            assert_eq!(
                g.state.rules.objects.values().filter(|o| o.tapped).count(),
                4
            );
            recover(g);
        },
    );
    both(
        prepared(&[
            ("Mortal's Resolve", true),
            ("Grizzly Bears", false),
            ("Darksteel Plate", false),
            ("Mountain", false),
            ("Mountain", false),
            ("Forest", false),
            ("Forest", false),
        ]),
        |g, q| {
            equip(g, q, 2, 1);
            cast(g, q, 0, target(g, 1));
            boosted(g, 1);
            through_cleanup(g, q);
            assert!(protected(g, 1));
            assert!(g.state.rules.effects.is_empty());
            assert_eq!(
                g.derived_characteristics(object(g, 1))
                    .unwrap()
                    .power
                    .unwrap(),
                2.into()
            );
        },
    );
}
#[test]
fn compound_indestructible_marked_lethal_true_cleanup_and_fresh_destroy() {
    both(
        prepared(&[
            ("Mortal's Resolve", true),
            ("Grizzly Bears", false),
            ("Lightning Bolt", true),
            ("Doom Blade", true),
            ("Forest", false),
            ("Forest", false),
            ("Mountain", false),
            ("Swamp", false),
            ("Swamp", false),
        ]),
        |g, q| {
            cast(g, q, 0, target(g, 1));
            cast(g, q, 2, target(g, 1));
            boosted(g, 1);
            assert_eq!(g.current(object(g, 1)).unwrap().marked_damage, 3.into());
            recover(g);
            through_cleanup(g, q);
            assert_eq!(g.current(object(g, 1)).unwrap().zone, Zone::Battlefield);
            assert_eq!(g.current(object(g, 1)).unwrap().marked_damage, 0.into());
            assert_eq!(
                g.derived_characteristics(object(g, 1))
                    .unwrap()
                    .power
                    .unwrap(),
                2.into()
            );
            assert_eq!(
                g.derived_characteristics(object(g, 1))
                    .unwrap()
                    .toughness
                    .unwrap(),
                2.into()
            );
            assert!(!protected(g, 1));
            assert!(g.state.rules.effects.is_empty());
            recover(g);
            until_player(g, q, PlayerId(0));
            cast(g, q, 3, target(g, 1));
            assert!(matches!(
                g.current(object(g, 1)).unwrap().zone,
                Zone::Graveyard { .. }
            ));
        },
    );
}
#[test]
fn compound_indestructible_bounce_reentry_and_resolution_target_recheck() {
    let rows = [
        ("Mortal's Resolve", true),
        ("Grizzly Bears", false),
        ("Unsummon", true),
        ("Forest", false),
        ("Forest", false),
        ("Island", false),
        ("Forest", false),
        ("Forest", false),
    ];
    both(prepared(&rows), |g, q| {
        cast(g, q, 0, target(g, 1));
        boosted(g, 1);
        let old = object(g, 1);
        cast(g, q, 2, target(g, 1));
        cast(g, q, 1, vec![]);
        assert_ne!(object(g, 1), old);
        assert!(!protected(g, 1));
        assert_eq!(
            g.derived_characteristics(object(g, 1))
                .unwrap()
                .power
                .unwrap(),
            2.into()
        );
        let before = g.state.clone();
        let trace = g.trace.clone();
        for viewer in 0..4 {
            let visible = g.visible_effects(PlayerId(viewer)).unwrap();
            assert_eq!(visible.len(), 2);
            assert!(
                matches!(&visible[1],VisibleEffect::GrantKeywords{target,..} if target.current.is_none())
            );
        }
        assert_eq!(g.state, before);
        assert_eq!(g.trace, trace);
        recover(g);
    });
    both(prepared(&rows), |g, q| {
        announce(g, q, 0, target(g, 1));
        cast(g, q, 2, target(g, 1));
        assert!(g.state.rules.effects.is_empty());
        cast(g, q, 1, vec![]);
        assert!(!protected(g, 1));
        assert_eq!(
            g.derived_characteristics(object(g, 1))
                .unwrap()
                .power
                .unwrap(),
            2.into()
        );
        recover(g);
    });
}
#[test]
fn compound_indestructible_opponent_target_query_and_response_rejections_are_atomic() {
    let mut p = prepared(&[
        ("Mortal's Resolve", true),
        ("Grizzly Bears", false),
        ("Forest", false),
        ("Forest", false),
    ]);
    p.config.cards[1].owner = PlayerId(1);
    p.config.cards[1].controller = PlayerId(1);
    p.profile_digest = digest(&p.config).unwrap();
    both(p, |g, q| {
        answer(g, q, Response::Cast { card: CardId(0) });
        let r = request(g, q);
        assert!(matches!(r, Request::Targets { .. }));
        let before = g.state.clone();
        let trace = g.trace.clone();
        let QueryResult::Complete(QueryValue::Decision(Some(PlayerRequest::Targets {
            token,
            candidates,
            ..
        }))) = g.player_decision_for(PlayerId(0), Budget::unlimited())
        else {
            panic!("public target query")
        };
        assert_eq!(candidates.len(), 1);
        for (player, objects) in [
            (PlayerId(0), vec![ObjectHandle("foreign".into())]),
            (PlayerId(1), candidates.clone()),
        ] {
            assert!(matches!(
                g.check_player_response(
                    player,
                    token.clone(),
                    PlayerResponse::Targets { objects },
                    Budget::unlimited()
                ),
                QueryResult::Rejected(QueryRejection::InvalidResponse)
            ));
            assert_eq!(g.state, before);
            assert_eq!(g.trace, trace);
        }
        let mut stale = object(g, 1);
        stale.incarnation += 1;
        for objects in [
            vec![object(g, 2)],
            vec![stale],
            vec![object(g, 1), object(g, 1)],
        ] {
            assert!(
                g.respond(r.player(), r.decision_id(), Response::Targets { objects })
                    .is_err()
            );
            assert_eq!(g.state, before);
            assert_eq!(g.trace, trace);
        }
        // Reuse the normal paid announcement loop from its next mana request.
        g.respond(
            r.player(),
            r.decision_id(),
            Response::Targets {
                objects: vec![object(g, 1)],
            },
        )
        .unwrap();
        for id in [2, 3] {
            answer(
                g,
                q,
                Response::ActivateMana {
                    object: object(g, id),
                },
            );
        }
        answer(g, q, Response::FinishMana { undo_mana: false });
        let r = request(g, q);
        if matches!(r, Request::SpendMana { .. }) {
            answer(
                g,
                q,
                Response::SpendMana {
                    spending: BTreeMap::from([(Color::G, 2.into())]),
                },
            );
        }
        while !g.state.rules.stack.is_empty() {
            answer(g, q, Response::Pass);
            request(g, q);
        }
        boosted(g, 1);
        recover(g);
    });
}
#[test]
fn compound_indestructible_paused_invocation_prefix_snapshot_replay() {
    both(
        prepared(&[
            ("Mortal's Resolve", true),
            ("Grizzly Bears", false),
            ("Forest", false),
            ("Forest", false),
        ]),
        |g, q| {
            announce(g, q, 0, target(g, 1));
            let mut prefixes = 0;
            for _ in 0..2000 {
                match g.advance(Budget::new(q)) {
                    Advance::Suspended { .. } => {
                        if g.state.rules.effects.len() == 1 {
                            prefixes += 1;
                            assert_eq!(
                                g.derived_characteristics(object(g, 1))
                                    .unwrap()
                                    .power
                                    .unwrap(),
                                3.into()
                            );
                            assert!(!protected(g, 1));
                            // No priority/SBA boundary exposes a half-completed invocation.
                            assert!(g.state.rules.pending.is_none());
                            assert!(matches!(g.state.rules.frame, Frame::Resolve { .. }));
                            recover(g);
                        }
                    }
                    Advance::AwaitingDecision(r) => {
                        if g.state.rules.stack.is_empty() {
                            break;
                        }
                        g.respond(r.player(), r.decision_id(), pass(&r)).unwrap();
                    }
                    x => panic!("compound invocation {x:?}"),
                }
            }
            if q == 1 {
                assert!(
                    prefixes > 0,
                    "actual suspended prefix between effect instructions"
                );
            }
            boosted(g, 1);
            recover(g);
        },
    );
}
