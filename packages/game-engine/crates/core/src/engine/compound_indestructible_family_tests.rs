//! Private paid complete-source family witnesses. No profile or policy admission.
//! CR 608.2b/c, 611.2a/c, 613.1f/613.4c, 400.7, 514.2, 702.12:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::equipped_indestructible_tests::{answer, both, cast, object, recover, request};
use super::temporary_indestructible_tests::{announce, pass, target, through_cleanup};
use super::*;

const SOURCES: &[(&str, &str, &str, i64, i64)] = &[
    (
        "Mortal's Resolve",
        "Forest",
        include_str!("../../../../tests/fixtures/ef29f979-1a6d-47bb-83f5-4679389471da.json"),
        3,
        3,
    ),
    (
        "Adamant Will",
        "Plains",
        include_str!("../../../../tests/fixtures/467e22c3-6107-40ff-afc7-5960710c970b.json"),
        4,
        4,
    ),
    (
        "Unlikely Aid",
        "Swamp",
        include_str!("../../../../tests/fixtures/795e0ffb-fcd9-4cb9-ad18-8a0217229bfe.json"),
        4,
        2,
    ),
];

fn prepared(rows: &[(&str, bool)], name: &str, raw: &str) -> Prepared {
    let placeholders: Vec<_> = rows
        .iter()
        .map(|(n, hand)| (if *n == name { "Giant Growth" } else { *n }, *hand))
        .collect();
    let mut p = equipped_indestructible_tests::prepared(&placeholders, false);
    let mtg_compiler::CompileResult::Lowered { definition: d } = mtg_compiler::compile(
        include_str!("../../../../tests/fixtures/archive-manifest-current.json"),
        raw,
    ) else {
        panic!("complete source {name}")
    };
    assert_eq!(d.name, name);
    for (i, (n, _)) in rows.iter().enumerate() {
        if *n == name {
            p.config.cards[i].oracle_id = d.oracle_id.clone();
        }
    }
    let mut c = (*p.catalog).clone();
    c.definitions.push(*d);
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    p.catalog_digest = digest(&c).unwrap();
    p.catalog = Arc::new(c);
    p.profile_digest = digest(&p.config).unwrap();
    p
}
fn stats(g: &Game, id: u32, power: i64, toughness: i64, protected: bool) {
    let derived = g.derived_characteristics(object(g, id)).unwrap();
    assert_eq!(derived.power.unwrap(), power.into());
    assert_eq!(derived.toughness.unwrap(), toughness.into());
    assert_eq!(
        g.has_keyword(object(g, id), KeywordAbility::Indestructible),
        protected
    );
}
fn effects(g: &Game, id: u32) {
    assert_eq!(g.state.rules.effects.len(), 2);
    assert!(
        matches!(&g.state.rules.effects[0].kind, ResolvedEffectKind::ModifyPowerToughness { target, .. } if *target == object(g, id))
    );
    assert!(
        matches!(&g.state.rules.effects[1].kind, ResolvedEffectKind::GrantKeywords { target, .. } if *target == object(g, id))
    );
    assert_eq!(
        g.current_keywords(object(g, id))
            .iter()
            .filter(|k| **k == KeywordAbility::Indestructible)
            .count(),
        1
    );
}

#[test]
fn compound_indestructible_family_paid_destroy_damage_cleanup_and_replay() {
    for &(name, land, raw, power, toughness) in SOURCES {
        both(
            prepared(
                &[
                    (name, true),
                    ("Grizzly Bears", false),
                    ("Doom Blade", true),
                    ("Lightning Bolt", true),
                    ("Lightning Bolt", true),
                    ("Doom Blade", true),
                    (land, false),
                    (land, false),
                    ("Swamp", false),
                    ("Swamp", false),
                    ("Mountain", false),
                    ("Mountain", false),
                ],
                name,
                raw,
            ),
            |g, q| {
                announce(g, q, 2, target(g, 1));
                cast(g, q, 0, target(g, 1));
                stats(g, 1, power, toughness, true);
                effects(g, 1);
                assert_eq!(g.current(object(g, 1)).unwrap().zone, Zone::Battlefield);
                assert!(matches!(
                    g.current(object(g, 0)).unwrap().zone,
                    Zone::Graveyard { .. }
                ));
                // Both incoming destruction and lethal marked damage use the grant.
                cast(g, q, 3, target(g, 1));
                cast(g, q, 4, target(g, 1));
                assert_eq!(g.current(object(g, 1)).unwrap().marked_damage, 6.into());
                stats(g, 1, power, toughness, true);
                assert_eq!(
                    g.state.rules.objects.values().filter(|o| o.tapped).count(),
                    6
                );
                recover(g);
                through_cleanup(g, q);
                assert_eq!(g.current(object(g, 1)).unwrap().zone, Zone::Battlefield);
                assert_eq!(g.current(object(g, 1)).unwrap().marked_damage, 0.into());
                stats(g, 1, 2, 2, false);
                assert!(g.state.rules.effects.is_empty());
                recover(g);
                // All six lands paid for the preceding spells. Priority on the
                // next player's turn does not untap our sources (CR 502.3).
                // Reach our actual next turn before paying for fresh removal.
                let mut own_turn = false;
                for _ in 0..500 {
                    let r = request(g, q);
                    if g.state.rules.turn_owner == PlayerId(0)
                        && matches!(
                            r,
                            Request::Priority {
                                player: PlayerId(0),
                                ..
                            }
                        )
                    {
                        own_turn = true;
                        break;
                    }
                    answer(g, q, pass(&r));
                }
                assert!(own_turn, "bounded natural return to caster's untap");
                cast(g, q, 5, target(g, 1));
                assert!(matches!(
                    g.current(object(g, 1)).unwrap().zone,
                    Zone::Graveyard { .. }
                ));
            },
        );
    }
}

#[test]
fn compound_indestructible_family_incarnation_and_resolution_target_recheck() {
    for &(name, land, raw, power, toughness) in SOURCES {
        let rows = [
            (name, true),
            ("Grizzly Bears", false),
            ("Unsummon", true),
            (land, false),
            (land, false),
            ("Island", false),
            ("Forest", false),
            ("Forest", false),
        ];
        both(prepared(&rows, name, raw), |g, q| {
            cast(g, q, 0, target(g, 1));
            stats(g, 1, power, toughness, true);
            effects(g, 1);
            let old = object(g, 1);
            cast(g, q, 2, target(g, 1));
            cast(g, q, 1, vec![]);
            assert_ne!(object(g, 1), old);
            stats(g, 1, 2, 2, false);
            let before = g.state.clone();
            let trace = g.trace.clone();
            for viewer in 0..4 {
                let visible = g.visible_effects(PlayerId(viewer)).unwrap();
                assert_eq!(visible.len(), 2);
                assert!(
                    matches!(&visible[1], VisibleEffect::GrantKeywords { target, .. } if target.current.is_none())
                );
            }
            assert_eq!(g.state, before);
            assert_eq!(g.trace, trace);
            recover(g);
        });
        both(prepared(&rows, name, raw), |g, q| {
            announce(g, q, 0, target(g, 1));
            cast(g, q, 2, target(g, 1));
            assert!(g.state.rules.effects.is_empty());
            cast(g, q, 1, vec![]);
            stats(g, 1, 2, 2, false);
        });
    }
}

#[test]
fn compound_indestructible_family_queries_and_invalid_responses_are_atomic() {
    for &(name, land, raw, power, toughness) in SOURCES {
        both(
            prepared(
                &[
                    (name, true),
                    ("Grizzly Bears", false),
                    (land, false),
                    (land, false),
                ],
                name,
                raw,
            ),
            |g, q| {
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
                    panic!("target query")
                };
                assert_eq!(candidates.len(), 1);
                assert!(matches!(
                    g.check_player_response(
                        PlayerId(0),
                        token,
                        PlayerResponse::Targets {
                            objects: vec![ObjectHandle("foreign".into())]
                        },
                        Budget::unlimited()
                    ),
                    QueryResult::Rejected(QueryRejection::InvalidResponse)
                ));
                let mut stale = object(g, 1);
                stale.incarnation += 1;
                for objects in [
                    vec![stale],
                    vec![object(g, 2)],
                    vec![object(g, 1), object(g, 1)],
                ] {
                    assert!(
                        g.respond(r.player(), r.decision_id(), Response::Targets { objects })
                            .is_err()
                    );
                    assert_eq!(g.state, before);
                    assert_eq!(g.trace, trace);
                }
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
                    let color = g
                        .definition(CardId(0))
                        .unwrap()
                        .characteristics
                        .mana_cost
                        .colored[0];
                    answer(
                        g,
                        q,
                        Response::SpendMana {
                            spending: BTreeMap::from([(color, 2.into())]),
                        },
                    );
                }
                while !g.state.rules.stack.is_empty() {
                    answer(g, q, Response::Pass);
                    request(g, q);
                }
                stats(g, 1, power, toughness, true);
                effects(g, 1);
            },
        );
    }
}

#[test]
fn compound_indestructible_family_suspended_prefix_preserves_actual_modifier() {
    for &(name, land, raw, power, toughness) in SOURCES {
        both(
            prepared(
                &[
                    (name, true),
                    ("Grizzly Bears", false),
                    (land, false),
                    (land, false),
                ],
                name,
                raw,
            ),
            |g, q| {
                announce(g, q, 0, target(g, 1));
                let mut prefixes = 0;
                for _ in 0..2000 {
                    match g.advance(Budget::new(q)) {
                        Advance::Suspended { .. } => {
                            if g.state.rules.effects.len() == 1 {
                                prefixes += 1;
                                stats(g, 1, power, toughness, false);
                                assert!(g.state.rules.pending.is_none());
                                assert!(matches!(g.state.rules.frame, Frame::Resolve { .. }));
                                recover(g);
                            }
                        }
                        Advance::AwaitingDecision(r) => {
                            if g.state.rules.stack.is_empty() {
                                break;
                            }
                            g.respond(
                                r.player(),
                                r.decision_id(),
                                super::temporary_indestructible_tests::pass(&r),
                            )
                            .unwrap();
                        }
                        x => panic!("compound invocation {x:?}"),
                    }
                }
                if q == 1 {
                    assert!(prefixes > 0, "actual suspended prefix");
                }
                stats(g, 1, power, toughness, true);
                effects(g, 1);
            },
        );
    }
}
