//! Private exact-source scenarios, accepted casts/payment/responses, no admission.
//! CR 611.2a/c,613.1f,514.2,608.2b,400.7,702.2a-d,704.5h:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_witness::equipped::{answer, both, cast, equip, object, recover, request};
fn prepared(rows: &[(&str, bool)]) -> Prepared {
    let placeholders: Vec<_> = rows
        .iter()
        .map(|(n, h)| {
            (
                if matches!(*n, "Serpent's Gift" | "Ohran Frostfang") {
                    "Giant Growth"
                } else {
                    *n
                },
                *h,
            )
        })
        .collect();
    let mut p = mtg_witness::equipped::prepared(&placeholders, false);
    let mut c = (*p.catalog).clone();
    let mtg_compiler::CompileResult::Lowered { definition: d } = mtg_compiler::compile(
        include_str!("../../../../tests/fixtures/archive-manifest-current.json"),
        include_str!("../../../../tests/fixtures/b75fb789-6a78-4d23-b872-7627b478fb6f.json"),
    ) else {
        panic!("exact source")
    };
    for (i, (n, _)) in rows.iter().enumerate() {
        if *n == "Serpent's Gift" {
            p.config.cards[i].oracle_id = d.oracle_id.clone();
        }
    }
    c.definitions.push(*d);
    let mtg_compiler::CompileResult::Lowered {
        definition: frostfang,
    } = mtg_compiler::compile(
        include_str!("../../../../tests/fixtures/archive-manifest-current.json"),
        include_str!(
            "../../../../tests/fixtures/cards-data-d2ce/b99ada26-9a61-4175-9fb8-15a106960220.json"
        ),
    )
    else {
        panic!("exact Frostfang source")
    };
    for (i, (n, _)) in rows.iter().enumerate() {
        if *n == "Ohran Frostfang" {
            p.config.cards[i].oracle_id = frostfang.oracle_id.clone();
        }
    }
    c.definitions.push(*frostfang);
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
fn deathtouch(g: &Game, id: u32) -> bool {
    g.has_keyword(object(g, id), KeywordAbility::Deathtouch)
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
fn resolve(g: &mut Game, q: u64) {
    while !g.state.rules.stack.is_empty() {
        answer(g, q, Response::Pass);
        request(g, q);
    }
}
fn activate(g: &mut Game, q: u64, source: u32, victim: u32) {
    answer(
        g,
        q,
        Response::ActivateAbility {
            object: object(g, source),
            ability: 0,
        },
    );
    answer(
        g,
        q,
        Response::EntityTargets {
            entities: target(g, victim),
        },
    );
    assert!(matches!(request(g, q), Request::Priority { .. }));
}
fn grave(g: &Game, id: u32) -> bool {
    matches!(
        g.current(object(g, id)).unwrap().zone,
        Zone::Graveyard { .. }
    )
}
#[test]
fn temporary_deathtouch_paid_current_and_departed_sources_deal_one_lethal_damage() {
    for name in ["Prodigal Sorcerer", "Mogg Fanatic"] {
        both(
            prepared(&[
                ("Serpent's Gift", true),
                (name, false),
                ("Yoked Ox", false),
                ("Forest", false),
                ("Forest", false),
                ("Forest", false),
            ]),
            |g, q| {
                assert!(!deathtouch(g, 1));
                cast(g, q, 0, target(g, 1));
                assert_eq!(
                    g.state.rules.objects.values().filter(|o| o.tapped).count(),
                    3
                );
                assert!(grave(g, 0));
                assert!(deathtouch(g, 1));
                assert_eq!(g.state.rules.effects.len(), 1);
                assert!(matches!(
                    &g.state.rules.effects[0].kind,
                    ResolvedEffectKind::GrantKeywords { modifier: None, .. }
                ));
                activate(g, q, 1, 2);
                let departed = name == "Mogg Fanatic";
                assert_eq!(grave(g, 1), departed);
                recover(g);
                resolve(g, q);
                assert!(
                    grave(g, 2),
                    "one actual damage kills a four-toughness victim"
                );
                if g.trace.mode == TraceMode::Detailed {
                    assert!(g.events().iter().any(|e| matches!(
                        e,
                        Event::Committed {
                            proposed: ProposedEvent::StateBasedGraveyard {
                                reason: CreatureLossReason::Deathtouch,
                                ..
                            },
                            ..
                        }
                    )));
                }
            },
        );
    }
}
#[test]
fn temporary_deathtouch_damage_time_appearance_and_no_retroactive_damage() {
    both(
        prepared(&[
            ("Serpent's Gift", true),
            ("Prodigal Sorcerer", false),
            ("Yoked Ox", false),
            ("Forest", false),
            ("Forest", false),
            ("Forest", false),
        ]),
        |g, q| {
            activate(g, q, 1, 2);
            resolve(g, q);
            assert_eq!(g.current(object(g, 2)).unwrap().marked_damage, 1.into());
            assert!(!g.current(object(g, 2)).unwrap().deathtouch_damage);
            cast(g, q, 0, target(g, 1));
            assert_eq!(g.current(object(g, 2)).unwrap().zone, Zone::Battlefield);
            assert!(deathtouch(g, 1));
            recover(g);
            through_cleanup(g, q);
            assert!(!deathtouch(g, 1));
            assert_eq!(
                g.current(object(g, 2)).unwrap().marked_damage,
                Quantity::zero()
            );
        },
    );
    // Appearance is taken at damage, after a pending ordinary activation.
    both(
        prepared(&[
            ("Serpent's Gift", true),
            ("Prodigal Sorcerer", false),
            ("Yoked Ox", false),
            ("Forest", false),
            ("Forest", false),
            ("Forest", false),
        ]),
        |g, q| {
            activate(g, q, 1, 2);
            cast(g, q, 0, target(g, 1)); // Gift resolves first, then the damage activation.
            assert!(grave(g, 2));
        },
    );
}
#[test]
fn temporary_deathtouch_protected_window_consumed_before_plate_loss() {
    both(
        prepared(&[
            ("Serpent's Gift", true),
            ("Prodigal Sorcerer", false),
            ("Yoked Ox", false),
            ("Darksteel Plate", false),
            ("Disperse", true),
            ("Forest", false),
            ("Forest", false),
            ("Forest", false),
            ("Forest", false),
            ("Forest", false),
            ("Island", false),
            ("Island", false),
        ]),
        |g, q| {
            equip(g, q, 3, 2);
            cast(g, q, 0, target(g, 1));
            activate(g, q, 1, 2);
            resolve(g, q);
            let victim = g.current(object(g, 2)).unwrap();
            assert_eq!(victim.zone, Zone::Battlefield);
            assert_eq!(victim.marked_damage, 1.into());
            assert!(!victim.deathtouch_damage);
            cast(g, q, 4, target(g, 3));
            assert!(!g.has_keyword(object(g, 2), KeywordAbility::Indestructible));
            assert_eq!(g.current(object(g, 2)).unwrap().zone, Zone::Battlefield);
            assert_eq!(g.current(object(g, 2)).unwrap().marked_damage, 1.into());
        },
    );
}
#[test]
fn temporary_deathtouch_bounce_reentry_and_resolution_revalidation() {
    for before_resolution in [false, true] {
        both(
            prepared(&[
                ("Serpent's Gift", true),
                ("Grizzly Bears", false),
                ("Unsummon", true),
                ("Forest", false),
                ("Forest", false),
                ("Forest", false),
                ("Forest", false),
                ("Forest", false),
                ("Island", false),
            ]),
            |g, q| {
                let old = object(g, 1);
                if before_resolution {
                    announce(g, q, 0, target(g, 1));
                } else {
                    cast(g, q, 0, target(g, 1));
                }
                cast(g, q, 2, target(g, 1));
                assert_eq!(g.state.rules.effects.len(), usize::from(!before_resolution));
                cast(g, q, 1, vec![]);
                assert_ne!(old, object(g, 1));
                assert!(!deathtouch(g, 1));
                for viewer in 0..4 {
                    let state = g.state.clone();
                    let trace = g.trace.clone();
                    let visible = g.visible_effects(PlayerId(viewer)).unwrap();
                    if !before_resolution {
                        assert!(
                            matches!(&visible[0],VisibleEffect::GrantKeywords{target,..} if target.current.is_none())
                        );
                    }
                    assert_eq!(g.state, state);
                    assert_eq!(g.trace, trace);
                }
            },
        );
    }
}
#[test]
fn temporary_deathtouch_union_and_atomic_cleanup() {
    both(
        prepared(&[
            ("Serpent's Gift", true),
            ("Typhoid Rats", false),
            ("Darksteel Plate", false),
            ("Forest", false),
            ("Forest", false),
            ("Forest", false),
            ("Forest", false),
            ("Forest", false),
        ]),
        |g, q| {
            equip(g, q, 2, 1);
            cast(g, q, 0, target(g, 1));
            let k = g.current_keywords(object(g, 1));
            assert_eq!(
                k.iter()
                    .filter(|k| **k == KeywordAbility::Deathtouch)
                    .count(),
                1
            );
            assert!(k.contains(&KeywordAbility::Indestructible));
            through_cleanup(g, q);
            assert!(g.state.rules.effects.is_empty());
            assert!(deathtouch(g, 1));
            assert!(g.has_keyword(object(g, 1), KeywordAbility::Indestructible));
        },
    );
}
#[test]
fn temporary_deathtouch_opponent_targets_atomic_invalid_and_normalized_forgery() {
    let mut p = prepared(&[
        ("Serpent's Gift", true),
        ("Yoked Ox", false),
        ("Forest", false),
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
        let state = g.state.clone();
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
            assert_eq!(g.state, state);
            assert_eq!(g.trace, trace);
        }
        // Complete the already accepted announcement with actual mana inputs.
        answer(
            g,
            q,
            Response::Targets {
                objects: vec![object(g, 1)],
            },
        );
        for id in 2..5 {
            answer(
                g,
                q,
                Response::ActivateMana {
                    object: object(g, id),
                },
            );
        }
        answer(g, q, Response::FinishMana { undo_mana: false });
        if matches!(request(g, q), Request::SpendMana { .. }) {
            answer(
                g,
                q,
                Response::SpendMana {
                    spending: BTreeMap::from([(Color::G, 3.into())]),
                },
            );
        }
        resolve(g, q);
        assert!(deathtouch(g, 1));
        recover(g);
        let good = g.snapshot().unwrap();
        Game::restore(g.prepared.as_ref().clone(), &good).unwrap();
        for variant in 0..6 {
            let mut bad: Snapshot = strict_read(&good).unwrap();
            let e = &mut bad.state.rules.effects[0];
            if let ResolvedEffectKind::GrantKeywords {
                target, keywords, ..
            } = &mut e.kind
            {
                match variant {
                    0 => keywords.clear(),
                    1 => *keywords = vec![KeywordAbility::Flying],
                    2 => keywords.push(KeywordAbility::Deathtouch),
                    3 => target.incarnation += 1,
                    4 => e.origin.source_block += 1,
                    _ => e.origin.source.incarnation += 1,
                }
            }
            bad.state_sha256 = digest(&bad.state).unwrap();
            assert!(
                Game::restore(g.prepared.as_ref().clone(), &canonical(&bad).unwrap()).is_err(),
                "forgery {variant}"
            );
        }
    });
}
#[test]
fn temporary_deathtouch_suspended_damage_current_and_departed_appearance_replay() {
    for name in ["Prodigal Sorcerer", "Mogg Fanatic"] {
        let p = prepared(&[
            ("Serpent's Gift", true),
            (name, false),
            ("Yoked Ox", false),
            ("Forest", false),
            ("Forest", false),
            ("Forest", false),
        ]);
        let mut prior = None;
        for q in [1, 7, 4096] {
            let mut g = Game::with_trace(p.clone(), TraceMode::Detailed);
            request(&mut g, q);
            let t = target(&g, 1);
            cast(&mut g, q, 0, t);
            activate(&mut g, q, 1, 2);
            let mut snapshots = 0;
            for _ in 0..10000 {
                match g.advance(Budget::new(q)) {
                    Advance::Suspended { .. } => {
                        let wire = g.snapshot().unwrap();
                        let restored = Game::restore(p.clone(), &wire).unwrap();
                        assert_eq!(g.state, restored.state);
                        if let Frame::DamageEffects { transaction }
                        | Frame::DamageAllocation { transaction, .. } = &g.state.rules.frame
                        {
                            snapshots += 1;
                            assert_eq!(
                                transaction.entries[0].source.keywords,
                                [KeywordAbility::Deathtouch]
                            );
                            for k in [
                                vec![],
                                vec![KeywordAbility::Flying],
                                vec![KeywordAbility::Deathtouch, KeywordAbility::Deathtouch],
                            ] {
                                let mut bad: Snapshot = strict_read(&wire).unwrap();
                                let (Frame::DamageEffects { transaction }
                                | Frame::DamageAllocation { transaction, .. }) =
                                    &mut bad.state.rules.frame
                                else {
                                    unreachable!()
                                };
                                transaction.entries[0].source.keywords = k;
                                bad.state_sha256 = digest(&bad.state).unwrap();
                                assert!(
                                    Game::restore(p.clone(), &canonical(&bad).unwrap()).is_err()
                                );
                            }
                        }
                    }
                    Advance::AwaitingDecision(r) => {
                        if grave(&g, 2) {
                            break;
                        }
                        g.respond(r.player(), r.decision_id(), Response::Pass)
                            .unwrap();
                    }
                    x => panic!("{x:?}"),
                }
            }
            assert!(grave(&g, 2));
            if q == 1 {
                assert!(snapshots > 0);
            }
            recover(&g);
            let hash = g.state_digest().unwrap();
            if let Some(old) = prior {
                assert_eq!(hash, old);
            }
            prior = Some(hash);
        }
    }
}

#[test]
fn temporary_deathtouch_combat_union_with_source_backed_static_grant() {
    // CR 702.2b/c/613.1f: grant before combat, deduplicate the static attacking
    // grant, and let two actual damage destroy a four-toughness blocker.
    let mut p = prepared(&[
        ("Serpent's Gift", true),
        ("Grizzly Bears", false),
        ("Giant Spider", false),
        ("Ohran Frostfang", false),
        ("Forest", false),
        ("Forest", false),
        ("Forest", false),
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
        assert_eq!(
            g.current_keywords(object(g, 1))
                .iter()
                .filter(|k| **k == KeywordAbility::Deathtouch)
                .count(),
            1
        );
        answer(
            g,
            q,
            Response::Blockers {
                blocks: vec![(object(g, 2), object(g, 1))],
            },
        );
        for _ in 0..100 {
            let r = request(g, q);
            if grave(g, 2) {
                break;
            }
            answer(g, q, pass(&r));
        }
        assert!(grave(g, 2));
        assert!(grave(g, 1));
        assert_eq!(g.state.rules.players[1].life, 40.into());
    });
}
