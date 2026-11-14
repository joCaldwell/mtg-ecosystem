//! Private exact-source scenarios, accepted casts/payment/responses, no admission.
//! CR 611.2a/c,613.1f,514.2,608.2b,400.7,702.9/17:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::equipped_indestructible_tests::{answer, both, cast, equip, object, recover, request};
use super::*;
fn prepared(rows: &[(&str, bool)]) -> Prepared {
    let placeholders: Vec<_> = rows
        .iter()
        .map(|(n, h)| (if *n == "Jump" { "Giant Growth" } else { *n }, *h))
        .collect();
    let mut p = equipped_indestructible_tests::prepared(&placeholders, false);
    let mut c = (*p.catalog).clone();
    let mtg_compiler::CompileResult::Lowered { definition: d } = mtg_compiler::compile(
        include_str!("../../../../tests/fixtures/archive-manifest-current.json"),
        include_str!("../../../../tests/fixtures/f7518456-45ed-41d4-bd3c-5aacea28eb35.json"),
    ) else {
        panic!("exact source")
    };
    for (i, (n, _)) in rows.iter().enumerate() {
        if *n == "Jump" {
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
fn flying(g: &Game, id: u32) -> bool {
    g.has_keyword(object(g, id), KeywordAbility::Flying)
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
fn temporary_flying_paid_grant_source_departure_expiry_and_replay() {
    both(
        prepared(&[("Jump", true), ("Grizzly Bears", false), ("Island", false)]),
        |g, q| {
            assert!(!flying(g, 1));
            cast(g, q, 0, target(g, 1));
            assert!(g.current(object(g, 2)).unwrap().tapped);
            assert!(matches!(
                g.current(object(g, 0)).unwrap().zone,
                Zone::Graveyard { .. }
            ));
            assert!(flying(g, 1));
            assert_eq!(g.state.rules.effects.len(), 1);
            recover(g);
            let before = g.state.clone();
            let trace = g.trace.clone();
            for viewer in 0..4 {
                assert!(
                    matches!(&g.visible_effects(PlayerId(viewer)).unwrap()[0], VisibleEffect::GrantKeywords {target,keywords,..} if target.current.is_some() && keywords == &[KeywordAbility::Flying])
                );
            }
            assert_eq!(g.state, before);
            assert_eq!(g.trace, trace);
            through_cleanup(g, q);
            assert!(!flying(g, 1));
            assert!(g.state.rules.effects.is_empty());
        },
    );
}
#[test]
fn temporary_flying_grounded_excluded_flying_and_reach_allowed() {
    for blocker in ["Grizzly Bears", "Angelic Wall", "Giant Spider"] {
        let legal = blocker != "Grizzly Bears";
        let mut p = prepared(&[
            ("Jump", true),
            ("Grizzly Bears", false),
            (blocker, false),
            ("Island", false),
        ]);
        p.config.cards[2].owner = PlayerId(1);
        p.config.cards[2].controller = PlayerId(1);
        p.profile_digest = digest(&p.config).unwrap();
        both(p, |g, q| {
            cast(g, q, 0, target(g, 1));
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
                if matches!(r, Request::Blockers { .. }) {
                    break;
                }
                answer(g, q, pass(&r));
            }
            let QueryResult::Complete(QueryValue::Observation(obs)) =
                g.observe(PlayerId(1), Budget::unlimited())
            else {
                panic!("observation")
            };
            let Some(PlayerRequest::Blockers {
                legal_attackers, ..
            }) = obs.decision
            else {
                panic!("block hints")
            };
            assert_eq!(legal_attackers.len(), 1);
            assert_eq!(legal_attackers[0].attackers.len(), usize::from(legal));
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
            assert_eq!(result.is_ok(), legal);
            if !legal {
                assert_eq!(g.state, before);
                assert_eq!(g.trace, trace);
                answer(g, q, Response::Blockers { blocks: vec![] });
            }
            recover(g);
            through_cleanup(g, q);
            assert_eq!(
                g.state.rules.players[1].life,
                Quantity::from(if legal { 40 } else { 38 })
            );
            assert!(g.state.rules.effects.is_empty());
        });
    }
}
#[test]
fn temporary_flying_bounce_reentry_keeps_historical_target_without_rebinding() {
    both(
        prepared(&[
            ("Jump", true),
            ("Grizzly Bears", false),
            ("Unsummon", true),
            ("Island", false),
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
            assert_ne!(old, object(g, 1));
            assert!(!flying(g, 1));
            for viewer in 0..4 {
                assert!(
                    matches!(&g.visible_effects(PlayerId(viewer)).unwrap()[0], VisibleEffect::GrantKeywords {target,..} if target.current.is_none())
                );
            }
            recover(g);
        },
    );
}
#[test]
fn temporary_flying_union_preserves_intrinsic_and_static_keywords() {
    // Serra's intrinsic flight deduplicates. Plate's independently resolved
    // equip grant remains while temporary flight on the Bears expires.
    both(
        prepared(&[
            ("Jump", true),
            ("Serra Angel", false),
            ("Jump", true),
            ("Grizzly Bears", false),
            ("Darksteel Plate", false),
            ("Mountain", false),
            ("Mountain", false),
            ("Island", false),
            ("Island", false),
        ]),
        |g, q| {
            equip(g, q, 4, 3);
            cast(g, q, 0, target(g, 1));
            cast(g, q, 2, target(g, 3));
            assert_eq!(
                g.current_keywords(object(g, 1))
                    .iter()
                    .filter(|k| **k == KeywordAbility::Flying)
                    .count(),
                1
            );
            assert!(flying(g, 3));
            assert!(g.has_keyword(object(g, 3), KeywordAbility::Indestructible));
            through_cleanup(g, q);
            assert!(flying(g, 1));
            assert!(!flying(g, 3));
            assert!(g.has_keyword(object(g, 1), KeywordAbility::Vigilance));
            assert!(g.has_keyword(object(g, 3), KeywordAbility::Indestructible));
        },
    );
}
#[test]
fn temporary_flying_opponent_target_invalid_response_atomic_and_restore_guard() {
    let mut p = prepared(&[("Jump", true), ("Grizzly Bears", false), ("Island", false)]);
    p.config.cards[1].owner = PlayerId(1);
    p.config.cards[1].controller = PlayerId(1);
    p.profile_digest = digest(&p.config).unwrap();
    both(p, |g, q| {
        answer(g, q, Response::Cast { card: CardId(0) });
        let r = request(g, q);
        assert!(matches!(r, Request::Targets { .. }));
        let before = g.state.clone();
        let trace = g.trace.clone();
        let mut stale = object(g, 1);
        stale.incarnation += 1;
        for objects in [
            vec![],
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
        answer(
            g,
            q,
            Response::Targets {
                objects: vec![object(g, 1)],
            },
        );
        answer(
            g,
            q,
            Response::ActivateMana {
                object: object(g, 2),
            },
        );
        answer(g, q, Response::FinishMana { undo_mana: false });
        if matches!(request(g, q), Request::SpendMana { .. }) {
            answer(
                g,
                q,
                Response::SpendMana {
                    spending: BTreeMap::from([(Color::U, 1.into())]),
                },
            );
        }
        while !g.state.rules.stack.is_empty() {
            answer(g, q, Response::Pass);
            request(g, q);
        }
        assert!(flying(g, 1));
        recover(g);
        for variant in 0..5 {
            let mut snapshot: Snapshot = strict_read(&g.snapshot().unwrap()).unwrap();
            let effect = &mut snapshot.state.rules.effects[0];
            if let ResolvedEffectKind::GrantKeywords { keywords, target } = &mut effect.kind {
                match variant {
                    0 => *keywords = vec![KeywordAbility::Reach],
                    1 => *keywords = vec![KeywordAbility::Indestructible],
                    2 => keywords.push(KeywordAbility::Flying),
                    3 => target.incarnation += 1,
                    _ => effect.origin.source_block += 1,
                }
            }
            snapshot.state_sha256 = digest(&snapshot.state).unwrap();
            assert!(
                Game::restore(g.prepared.as_ref().clone(), &canonical(&snapshot).unwrap()).is_err(),
                "tamper {variant}"
            );
        }
    });
}
#[test]
fn temporary_flying_resolution_rechecks_departed_target() {
    both(
        prepared(&[
            ("Jump", true),
            ("Grizzly Bears", false),
            ("Unsummon", true),
            ("Island", false),
            ("Island", false),
        ]),
        |g, q| {
            announce(g, q, 0, target(g, 1));
            cast(g, q, 2, target(g, 1));
            assert!(!flying(g, 1));
            assert!(g.state.rules.effects.is_empty());
            recover(g);
        },
    );
}
#[test]
fn temporary_flying_snapshots_at_suspended_grant_and_combat_boundaries() {
    let p = prepared(&[("Jump", true), ("Grizzly Bears", false), ("Island", false)]);
    let mut result = None;
    for q in [1, 7, 4096] {
        let mut g = Game::with_trace(p.clone(), TraceMode::Detailed);
        request(&mut g, q);
        let t = target(&g, 1);
        announce(&mut g, q, 0, t);
        let turn = g.state.rules.turn.number;
        let mut suspended = 0;
        let mut attacked = false;
        let mut damage_snapshots = 0;
        for _ in 0..20000 {
            match g.advance(Budget::new(q)) {
                Advance::Suspended { .. } => {
                    suspended += 1;
                    let snap = g.snapshot().unwrap();
                    let restored = Game::restore(p.clone(), &snap).unwrap();
                    assert_eq!(g.state, restored.state);
                    assert_eq!(g.trace(), restored.trace());
                    if matches!(
                        g.state.rules.frame,
                        Frame::DamageEffects { .. } | Frame::DamageAllocation { .. }
                    ) {
                        damage_snapshots += 1;
                        for keywords in [
                            vec![],
                            vec![KeywordAbility::Reach],
                            vec![KeywordAbility::Flying, KeywordAbility::Flying],
                        ] {
                            let mut bad: Snapshot = strict_read(&snap).unwrap();
                            let (Frame::DamageEffects { transaction }
                            | Frame::DamageAllocation { transaction, .. }) =
                                &mut bad.state.rules.frame
                            else {
                                unreachable!()
                            };
                            transaction.entries[0].source.keywords = keywords;
                            bad.state_sha256 = digest(&bad.state).unwrap();
                            assert!(Game::restore(p.clone(), &canonical(&bad).unwrap()).is_err());
                        }
                    }
                }
                Advance::AwaitingDecision(r) => {
                    if g.state.rules.turn.number > turn {
                        break;
                    }
                    let response = if matches!(r, Request::Attackers { .. }) {
                        attacked = true;
                        Response::Attackers {
                            attacks: vec![(object(&g, 1), PlayerId(1))],
                        }
                    } else {
                        pass(&r)
                    };
                    g.respond(r.player(), r.decision_id(), response).unwrap();
                }
                x => panic!("unexpected {x:?}"),
            }
        }
        assert!(attacked);
        assert!(g.state.rules.turn.number > turn);
        if q == 1 {
            assert!(suspended > 10);
            assert!(damage_snapshots > 0);
        }
        assert_eq!(g.state.rules.players[1].life, 38.into());
        assert!(!flying(&g, 1));
        recover(&g);
        let state = g.state_digest().unwrap();
        if let Some(prior) = &result {
            assert_eq!(&state, prior);
        }
        result = Some(state);
    }
}
