//! Private exact-source scenarios, accepted casts/payment/responses, no admission.
//! CR 611.2a/c,613.1f,514.2,608.2b,400.7,702.12:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::equipped_indestructible_tests::{answer, both, cast, equip, object, recover, request};
use super::*;
fn prepared(rows: &[(&str, bool)]) -> Prepared {
    let placeholders: Vec<_> = rows
        .iter()
        .map(|(n, h)| {
            (
                if *n == "Withstand Death" {
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
        include_str!("../../../../tests/fixtures/c462adf3-4ade-4b90-bdec-7247b080471f.json"),
    ) else {
        panic!("exact source")
    };
    for (i, (n, _)) in rows.iter().enumerate() {
        if *n == "Withstand Death" {
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
fn target(g: &Game, id: u32) -> Vec<TargetEntity> {
    vec![TargetEntity::Object {
        object: object(g, id),
    }]
}
fn protected(g: &Game, id: u32) -> bool {
    g.has_keyword(object(g, id), KeywordAbility::Indestructible)
}
fn pass(r: &Request) -> Response {
    match r {
        Request::Priority { .. } => Response::Pass,
        Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
        Request::Blockers { .. } => Response::Blockers { blocks: vec![] },
        Request::Discard {
            candidates, count, ..
        } => Response::Discard {
            objects: candidates.iter().take(*count).copied().collect(),
        },
        x => panic!("turn {x:?}"),
    }
}
fn through_cleanup(g: &mut Game, q: u64) {
    let turn = g.state.rules.turn.number;
    for _ in 0..200 {
        let r = request(g, q);
        if g.state.rules.turn.number > turn {
            return;
        }
        answer(g, q, pass(&r));
    }
    panic!("cleanup bound")
}
fn until_player(g: &mut Game, q: u64, p: PlayerId) {
    for _ in 0..500 {
        let r = request(g, q);
        if matches!(r,Request::Priority {player,..} if player==p) {
            return;
        }
        answer(g, q, pass(&r));
    }
    panic!("priority bound")
}
// Announce/pay a spell, stopping at priority before its resolution.
fn announce(g: &mut Game, q: u64, id: u32, t: Vec<TargetEntity>) {
    answer(g, q, Response::Cast { card: CardId(id) });
    loop {
        let r = request(g, q);
        let response = match &r {
            Request::Targets { .. } => Response::Targets {
                objects: t
                    .iter()
                    .map(|t| match t {
                        TargetEntity::Object { object } => *object,
                        _ => panic!("object"),
                    })
                    .collect(),
            },
            Request::EntityTargets { .. } => Response::EntityTargets {
                entities: t.clone(),
            },
            Request::Mana { cost, .. } => {
                if g.payment_pool(r.player(), cost).is_ok() {
                    Response::FinishMana { undo_mana: false }
                } else {
                    let o = g
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
                    Response::ActivateMana { object: o.id }
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
            Request::Priority { .. } => return,
            x => panic!("pay {x:?}"),
        };
        g.respond(r.player(), r.decision_id(), response).unwrap();
    }
}
#[test]
fn temporary_indestructible_paid_response_source_departure_and_replay() {
    both(
        prepared(&[
            ("Doom Blade", true),
            ("Grizzly Bears", false),
            ("Withstand Death", true),
            ("Swamp", false),
            ("Swamp", false),
            ("Forest", false),
        ]),
        |g, q| {
            announce(g, q, 0, target(g, 1));
            assert_eq!(g.state.rules.stack.len(), 1);
            cast(g, q, 2, target(g, 1));
            assert!(protected(g, 1));
            assert_eq!(g.current(object(g, 1)).unwrap().zone, Zone::Battlefield);
            assert!(matches!(
                g.current(object(g, 2)).unwrap().zone,
                Zone::Graveyard { .. }
            ));
            assert_eq!(
                g.state.rules.objects.values().filter(|o| o.tapped).count(),
                3
            );
            assert!(
                matches!(&g.state.rules.effects[0].kind,ResolvedEffectKind::GrantKeywords {target,..} if *target==object(g,1))
            );
            let before = g.state.clone();
            let trace = g.trace.clone();
            for viewer in 0..4 {
                let visible = g.visible_effects(PlayerId(viewer)).unwrap();
                assert!(
                    matches!(&visible[0],VisibleEffect::GrantKeywords {target,..} if target.current.is_some())
                );
            }
            assert_eq!(g.state, before);
            assert_eq!(g.trace, trace);
            recover(g);
        },
    );
}
#[test]
fn temporary_indestructible_lethal_survives_real_cleanup_then_fresh_destroy_kills() {
    both(
        prepared(&[
            ("Withstand Death", true),
            ("Grizzly Bears", false),
            ("Lightning Bolt", true),
            ("Doom Blade", true),
            ("Forest", false),
            ("Mountain", false),
            ("Swamp", false),
            ("Swamp", false),
        ]),
        |g, q| {
            cast(g, q, 0, target(g, 1));
            cast(g, q, 2, target(g, 1));
            assert_eq!(g.current(object(g, 1)).unwrap().marked_damage, 3.into());
            assert!(protected(g, 1));
            recover(g);
            through_cleanup(g, q);
            assert_eq!(g.current(object(g, 1)).unwrap().zone, Zone::Battlefield);
            assert_eq!(g.current(object(g, 1)).unwrap().marked_damage, 0.into());
            assert!(!protected(g, 1));
            assert!(g.state.rules.effects.is_empty());
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
fn temporary_indestructible_bounce_reentry_does_not_rebind() {
    both(
        prepared(&[
            ("Withstand Death", true),
            ("Grizzly Bears", false),
            ("Unsummon", true),
            ("Forest", false),
            ("Island", false),
            ("Forest", false),
            ("Forest", false),
        ]),
        |g, q| {
            cast(g, q, 0, target(g, 1));
            let old = object(g, 1);
            cast(g, q, 2, target(g, 1));
            assert!(matches!(
                g.current(object(g, 1)).unwrap().zone,
                Zone::Hand { .. }
            ));
            cast(g, q, 1, vec![]);
            assert_ne!(object(g, 1), old);
            assert!(!protected(g, 1));
            assert!(
                matches!(&g.visible_effects(PlayerId(0)).unwrap()[0],VisibleEffect::GrantKeywords {target,..} if target.current.is_none())
            );
            recover(g);
        },
    );
}
#[test]
fn temporary_indestructible_opponent_target_and_invalid_choices_atomic() {
    let mut p = prepared(&[
        ("Withstand Death", true),
        ("Grizzly Bears", false),
        ("Forest", false),
    ]);
    p.config.cards[1].controller = PlayerId(1);
    p.config.cards[1].owner = PlayerId(1);
    p.profile_digest = digest(&p.config).unwrap();
    both(p, |g, q| {
        answer(g, q, Response::Cast { card: CardId(0) });
        let r = request(g, q);
        assert!(matches!(r, Request::Targets { .. }));
        let before = g.state.clone();
        let trace = g.trace.clone();
        let mut stale = object(g, 1);
        stale.incarnation += 1;
        for o in [object(g, 2), stale] {
            assert!(
                g.respond(
                    r.player(),
                    r.decision_id(),
                    Response::Targets { objects: vec![o] }
                )
                .is_err()
            );
            assert_eq!(g.state, before);
            assert_eq!(g.trace, trace);
        }
        // Complete the same paid cast through the normal request loop.
        g.respond(
            r.player(),
            r.decision_id(),
            Response::Targets {
                objects: vec![object(g, 1)],
            },
        )
        .unwrap();
        let r = request(g, q);
        assert!(matches!(r, Request::Mana { .. }));
        answer(
            g,
            q,
            Response::ActivateMana {
                object: object(g, 2),
            },
        );
        answer(g, q, Response::FinishMana { undo_mana: false });
        let r = request(g, q);
        if matches!(r, Request::SpendMana { .. }) {
            answer(
                g,
                q,
                Response::SpendMana {
                    spending: BTreeMap::from([(Color::G, 1.into())]),
                },
            );
        }
        while !g.state.rules.stack.is_empty() {
            answer(g, q, Response::Pass);
            request(g, q);
        }
        assert!(protected(g, 1));
        recover(g);
    });
}
#[test]
fn temporary_indestructible_union_keeps_equipped_grant_after_cleanup() {
    let mut rows = vec![
        ("Withstand Death", true),
        ("Grizzly Bears", false),
        ("Darksteel Plate", false),
        ("Forest", false),
    ];
    rows.extend(vec![("Mountain", false); 2]);
    // Keep a second green source after the generic-payment helper taps the first.
    rows.push(("Forest", false));
    both(prepared(&rows), |g, q| {
        equip(g, q, 2, 1);
        cast(g, q, 0, target(g, 1));
        assert_eq!(
            g.current_keywords(object(g, 1))
                .iter()
                .filter(|k| **k == KeywordAbility::Indestructible)
                .count(),
            1
        );
        through_cleanup(g, q);
        assert!(g.state.rules.effects.is_empty());
        assert!(protected(g, 1));
    });
}

#[test]
fn temporary_indestructible_resolution_rechecks_departed_target() {
    both(
        prepared(&[
            ("Withstand Death", true),
            ("Grizzly Bears", false),
            ("Unsummon", true),
            ("Forest", false),
            ("Island", false),
            ("Forest", false),
            ("Forest", false),
        ]),
        |g, q| {
            announce(g, q, 0, target(g, 1));
            cast(g, q, 2, target(g, 1));
            assert!(g.state.rules.effects.is_empty());
            assert!(matches!(
                g.current(object(g, 1)).unwrap().zone,
                Zone::Hand { .. }
            ));
            cast(g, q, 1, vec![]);
            assert!(!protected(g, 1));
            recover(g);
        },
    );
}
