//! Private complete-source casts, not pool/profile/consumer admission.
//! All new native tests are UNRUN here; the parent owns combined execution.
//! CR 109.5, 400.7, 608.2b/c, 611.2a/c, 702.11b, 702.12, 514.2:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
//! No same-incarnation control-change acceptance: this base has no control-
//! changing Instruction/StaticEffect or supported Control Magic source fixture.
use super::equipped_indestructible_tests::{answer, both, cast, object, recover, request};
use super::temporary_indestructible_tests::{
    announce, pass, target, through_cleanup, until_player,
};
use super::*;

struct Witness {
    name: &'static str,
    raw: &'static str,
    land: &'static str,
    cost: usize,
    modifier: Option<(i64, i64)>,
    keywords: &'static [KeywordAbility],
}
const SOURCES: &[Witness] = &[
    Witness {
        name: "Masterful Flourish",
        raw: include_str!("../../../../tests/fixtures/1cb419cc-d8e4-4676-987f-089cebccc374.json"),
        land: "Swamp",
        cost: 1,
        modifier: Some((1, 0)),
        keywords: &[KeywordAbility::Indestructible],
    },
    Witness {
        name: "Dive Down",
        raw: include_str!("../../../../tests/fixtures/2f990b54-fbf3-4949-85bb-9ba39710e72a.json"),
        land: "Island",
        cost: 1,
        modifier: Some((0, 3)),
        keywords: &[KeywordAbility::Hexproof],
    },
    Witness {
        name: "Blossoming Defense",
        raw: include_str!("../../../../tests/fixtures/5a851367-1c4a-4cc9-a9c3-3d2775986b4c.json"),
        land: "Forest",
        cost: 1,
        modifier: Some((2, 2)),
        keywords: &[KeywordAbility::Hexproof],
    },
    Witness {
        name: "Ranger's Guile",
        raw: include_str!("../../../../tests/fixtures/694cb93a-bcc5-44b0-a76c-19ae4a4e5e0f.json"),
        land: "Forest",
        cost: 1,
        modifier: Some((1, 1)),
        keywords: &[KeywordAbility::Hexproof],
    },
    Witness {
        name: "Alesha's Legacy",
        raw: include_str!("../../../../tests/fixtures/c4229cc8-11e5-4221-b00d-86969701e21b.json"),
        land: "Swamp",
        cost: 2,
        modifier: None,
        keywords: &[KeywordAbility::Deathtouch, KeywordAbility::Indestructible],
    },
];

// These private ScenarioConfig owner/controller differences are initial setup,
// never a claim that gameplay changed a permanent's controller.
fn prepared(rows: &[(&str, u8, u8, bool)]) -> Prepared {
    let placeholders: Vec<_> = rows
        .iter()
        .map(|(name, _, _, hand)| {
            (
                if SOURCES.iter().any(|s| s.name == *name) {
                    "Giant Growth"
                } else {
                    *name
                },
                *hand,
            )
        })
        .collect();
    let mut p = equipped_indestructible_tests::prepared(&placeholders, false);
    let mut c = (*p.catalog).clone();
    for witness in SOURCES {
        let mtg_compiler::CompileResult::Lowered { definition } = mtg_compiler::compile(
            include_str!("../../../../tests/fixtures/archive-manifest-current.json"),
            witness.raw,
        ) else {
            panic!("complete frozen source {}", witness.name)
        };
        assert_eq!(definition.name, witness.name);
        assert_eq!(
            definition.spell.as_ref().unwrap().targets[0].domain,
            TargetDomain::ControlledBattlefieldCreature
        );
        for (i, (name, _, _, _)) in rows.iter().enumerate() {
            if *name == witness.name {
                p.config.cards[i].oracle_id = definition.oracle_id.clone();
            }
        }
        c.definitions.push(*definition);
    }
    for (i, (_, owner, controller, hand)) in rows.iter().enumerate() {
        let card = &mut p.config.cards[i];
        card.owner = PlayerId(*owner);
        card.controller = PlayerId(*controller);
        card.zone = if *hand {
            Zone::Hand { owner: card.owner }
        } else {
            Zone::Battlefield
        };
    }
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    p.catalog_digest = digest(&c).unwrap();
    p.catalog = Arc::new(c);
    p.profile_digest = digest(&p.config).unwrap();
    p
}
fn stats(g: &Game, id: u32, p: i64, t: i64) {
    let c = g.derived_characteristics(object(g, id)).unwrap();
    assert_eq!(c.power, Some(p.into()));
    assert_eq!(c.toughness, Some(t.into()));
}
fn assert_grant(g: &Game, witness: &Witness, id: u32) {
    let (p, t) = witness.modifier.unwrap_or((0, 0));
    stats(g, id, 2 + p, 2 + t);
    assert_eq!(g.state.rules.effects.len(), 1);
    let grant = g.state.rules.effects.last().unwrap();
    assert!(
        matches!(&grant.kind, ResolvedEffectKind::GrantKeywords { target, modifier, keywords }
        if *target == object(g, id) && keywords == witness.keywords
        && modifier.as_ref().map(|m| (m.power.clone(), m.toughness.clone()))
            == witness.modifier.map(|(p, t)| (p.into(), t.into())))
    );
    assert_eq!(grant.origin.source_block, 0);
    assert_eq!(grant.origin.cursor, 0);
    for keyword in witness.keywords {
        assert_eq!(
            g.current_keywords(object(g, id))
                .iter()
                .filter(|k| *k == keyword)
                .count(),
            1
        );
    }
}
fn resolve(g: &mut Game, q: u64) {
    while !g.state.rules.stack.is_empty() {
        answer(g, q, Response::Pass);
        request(g, q);
    }
}
fn own_main_after_untap(g: &mut Game, q: u64, seat: PlayerId, after_turn: u64) {
    for _ in 0..2000 {
        let r = request(g, q);
        if g.turn().number > after_turn
            && g.active_player() == Some(seat)
            && g.turn().step == TurnStep::PrecombatMain
            && matches!(r, Request::Priority { player, .. } if player == seat)
        {
            return;
        }
        answer(g, q, pass(&r));
    }
    panic!("actual own untap/main not reached");
}
fn finish_query(g: &Game, mut result: QueryResult) -> QueryValue {
    for _ in 0..10000 {
        result = match result {
            QueryResult::Complete(value) => return value,
            QueryResult::Incomplete { work, .. } => g.resume_work(work, Budget::new(1)),
            QueryResult::Rejected(reason) => panic!("query rejected {reason:?}"),
        };
    }
    panic!("bounded query");
}

#[test]
fn controlled_creature_protection_paid_five_sources_cleanup_and_normalized_forgery() {
    for witness in SOURCES {
        both(
            prepared(&[
                (witness.name, 0, 0, true),
                ("Grizzly Bears", 2, 0, false),
                (witness.land, 0, 0, false),
                (witness.land, 0, 0, false),
            ]),
            |g, q| {
                cast(g, q, 0, target(g, 1));
                assert_grant(g, witness, 1);
                assert_eq!(g.state.rules.cards[&CardId(1)].owner, PlayerId(2));
                assert_eq!(g.current(object(g, 1)).unwrap().controller, PlayerId(0));
                assert!(matches!(
                    g.current(object(g, 0)).unwrap().zone,
                    Zone::Graveyard { .. }
                ));
                assert_eq!(
                    g.state.rules.objects.values().filter(|o| o.tapped).count(),
                    witness.cost
                );
                recover(g); // Reachable positive restore/replay before normalized forgeries.
                let wire = g.snapshot().unwrap();
                for variant in 0..8 {
                    let mut bad: Snapshot = strict_read(&wire).unwrap();
                    let e = bad.state.rules.effects.last_mut().unwrap();
                    let ResolvedEffectKind::GrantKeywords {
                        target, keywords, ..
                    } = &mut e.kind
                    else {
                        unreachable!()
                    };
                    match variant {
                        0 => keywords.clear(),
                        1 => keywords.push(keywords[0]),
                        2 => *keywords = vec![KeywordAbility::Haste],
                        3 => {
                            *keywords = if witness.keywords == [KeywordAbility::Hexproof] {
                                vec![KeywordAbility::Indestructible]
                            } else {
                                vec![KeywordAbility::Hexproof]
                            }
                        }
                        4 => target.incarnation += 1,
                        5 => e.origin.source_block += 1,
                        6 => e.origin.cursor += 1,
                        _ => e.expires_in_cleanup_of_turn += 1,
                    }
                    bad.state_sha256 = digest(&bad.state).unwrap();
                    assert!(
                        Game::restore(g.prepared.as_ref().clone(), &canonical(&bad).unwrap())
                            .is_err(),
                        "{} normalized forgery {variant}",
                        witness.name
                    );
                }
                through_cleanup(g, q);
                assert!(g.state.rules.effects.is_empty());
                stats(g, 1, 2, 2);
                assert!(g.current_keywords(object(g, 1)).is_empty());
                for controller in 0..4 {
                    assert!(g.targetable_by(object(g, 1), PlayerId(controller)));
                }
            },
        );
    }
}

#[test]
fn controlled_creature_protection_controller_candidates_privacy_validation_and_stale_work() {
    for witness in SOURCES {
        both(
            prepared(&[
                (witness.name, 0, 0, true),
                ("Grizzly Bears", 2, 0, false), // Friendly, physically owned by seat 2.
                ("Grizzly Bears", 0, 1, false), // Physically ours, currently opposing.
                (witness.land, 0, 0, false),
                (witness.land, 0, 0, false),
            ]),
            |g, q| {
                answer(g, q, Response::Cast { card: CardId(0) });
                let r = request(g, q);
                let Request::Targets { candidates, .. } = &r else {
                    panic!("target request")
                };
                assert_eq!(candidates, &[object(g, 1)]);
                let before = g.state.clone();
                let trace = g.trace.clone();
                let QueryValue::Decision(Some(PlayerRequest::Targets {
                    token, candidates, ..
                })) = finish_query(g, g.player_decision_for(PlayerId(0), Budget::new(0)))
                else {
                    panic!("public target request")
                };
                assert_eq!(candidates.len(), 1);
                for seat in 1..4 {
                    assert!(matches!(
                        finish_query(g, g.player_decision_for(PlayerId(seat), Budget::new(0))),
                        QueryValue::Decision(None)
                    ));
                }
                let QueryValue::Observation(view) =
                    finish_query(g, g.observe(PlayerId(0), Budget::new(0)))
                else {
                    panic!("own observation")
                };
                let borrowed = view
                    .view
                    .objects
                    .iter()
                    .find(|o| o.owner == PlayerId(2) && o.zone == Zone::Battlefield)
                    .unwrap();
                assert_eq!(borrowed.controller, PlayerId(0));
                assert_eq!(candidates, vec![borrowed.handle.clone()]);
                let opposing = view
                    .view
                    .objects
                    .iter()
                    .find(|o| o.controller == PlayerId(1) && o.zone == Zone::Battlefield)
                    .unwrap();
                for (seat, objects) in [
                    (0, vec![]),
                    (0, vec![opposing.handle.clone()]),
                    (0, vec![candidates[0].clone(), candidates[0].clone()]),
                    (0, vec![ObjectHandle("foreign".into())]),
                    (1, candidates.clone()),
                ] {
                    assert!(matches!(
                        g.check_player_response(
                            PlayerId(seat),
                            token.clone(),
                            PlayerResponse::Targets { objects },
                            Budget::unlimited()
                        ),
                        QueryResult::Rejected(QueryRejection::InvalidResponse)
                    ));
                }
                let mut stale_object = object(g, 1);
                stale_object.incarnation += 1;
                for objects in [vec![object(g, 2)], vec![stale_object], vec![object(g, 3)]] {
                    assert!(
                        g.respond(r.player(), r.decision_id(), Response::Targets { objects })
                            .is_err()
                    );
                }
                let QueryResult::Incomplete { work, .. } = g.check_player_response(
                    PlayerId(0),
                    token.clone(),
                    PlayerResponse::Targets {
                        objects: candidates.clone(),
                    },
                    Budget::new(0),
                ) else {
                    panic!("suspended valid target validation")
                };
                let saved = g.save_work(&work).unwrap();
                let restored =
                    Game::restore(g.prepared.as_ref().clone(), &g.snapshot().unwrap()).unwrap();
                let recovered = restored.restore_work(&saved).unwrap();
                assert!(matches!(
                    finish_query(&restored, restored.resume_work(recovered, Budget::new(1))),
                    QueryValue::Validated(_)
                ));
                let QueryResult::Incomplete { work: stale, .. } =
                    g.player_decision_for(PlayerId(0), Budget::new(0))
                else {
                    panic!("suspended target query")
                };
                assert_eq!(g.state, before);
                assert_eq!(g.trace, trace);
                let QueryValue::Validated(receipt) =
                    finish_query(g, g.resume_work(work, Budget::new(1)))
                else {
                    panic!("target receipt")
                };
                g.submit(receipt).unwrap();
                assert!(matches!(
                    g.resume_work(stale, Budget::unlimited()),
                    QueryResult::Rejected(QueryRejection::StaleOrForeignWork)
                ));
                assert!(matches!(
                    g.check_player_response(
                        PlayerId(0),
                        token,
                        PlayerResponse::Targets {
                            objects: candidates
                        },
                        Budget::unlimited()
                    ),
                    QueryResult::Rejected(QueryRejection::InvalidResponse)
                ));
                equipped_indestructible_tests::pay_and_resolve(g, q, vec![]);
                assert_grant(g, witness, 1);
            },
        );
    }
}

#[test]
fn controlled_creature_protection_paid_opponent_destruction_and_hexproof_own_target() {
    for witness in SOURCES {
        let mut rows = vec![
            (witness.name, 0, 0, true),
            ("Grizzly Bears", 2, 0, false),
            ("Murder", 1, 1, true),
            (witness.name, 0, 0, true),
        ];
        rows.extend(vec![(witness.land, 0, 0, false); witness.cost]);
        rows.extend([
            (witness.land, 0, 0, false),
            ("Swamp", 1, 1, false),
            ("Swamp", 1, 1, false),
            ("Swamp", 1, 1, false),
        ]);
        both(prepared(&rows), |g, q| {
            until_player(g, q, PlayerId(1));
            announce(g, q, 2, target(g, 1));
            assert_eq!(g.state.rules.stack.len(), 1);
            until_player(g, q, PlayerId(0));
            cast(g, q, 0, target(g, 1));
            assert!(g.state.rules.stack.is_empty());
            assert_eq!(g.current(object(g, 1)).unwrap().zone, Zone::Battlefield);
            assert_grant(g, witness, 1);
            if witness.keywords.contains(&KeywordAbility::Hexproof) {
                for seat in 0..4 {
                    assert_eq!(g.targetable_by(object(g, 1), PlayerId(seat)), seat == 0);
                }
                // The controller can genuinely announce and pay a second spell
                // targeting the Hexproof creature, even though seat 2 owns it.
                until_player(g, q, PlayerId(0));
                cast(g, q, 3, target(g, 1));
                let (p, t) = witness.modifier.unwrap();
                stats(g, 1, 2 + 2 * p, 2 + 2 * t);
                assert_eq!(
                    g.current_keywords(object(g, 1))
                        .iter()
                        .filter(|k| **k == KeywordAbility::Hexproof)
                        .count(),
                    1
                );
            }
            recover(g);
        });
    }
}

#[test]
fn controlled_creature_protection_departure_reentry_skips_whole_program() {
    // One compound Hexproof and one grant-only vector exercise both shapes.
    for witness in [&SOURCES[1], &SOURCES[4]] {
        for before_resolution in [false, true] {
            let mut rows = vec![
                (witness.name, 0, 0, true),
                ("Grizzly Bears", 0, 0, false),
                ("Unsummon", 0, 0, true),
                ("Giant Growth", 0, 0, true),
            ];
            rows.extend(vec![(witness.land, 0, 0, false); witness.cost]);
            rows.extend([
                ("Island", 0, 0, false),
                ("Forest", 0, 0, false),
                ("Forest", 0, 0, false),
                ("Forest", 0, 0, false),
            ]);
            both(prepared(&rows), |g, q| {
                let old = object(g, 1);
                let QueryValue::Observation(view) =
                    finish_query(g, g.observe(PlayerId(0), Budget::new(0)))
                else {
                    panic!("old incarnation view")
                };
                let old_handle = view
                    .view
                    .objects
                    .iter()
                    .find(|o| o.oracle_id == BEARS && o.zone == Zone::Battlefield)
                    .unwrap()
                    .handle
                    .clone();
                if before_resolution {
                    announce(g, q, 0, target(g, 1));
                } else {
                    cast(g, q, 0, target(g, 1));
                    assert_grant(g, witness, 1);
                }
                cast(g, q, 2, target(g, 1));
                assert!(matches!(
                    g.current(object(g, 1)).unwrap().zone,
                    Zone::Hand { owner: PlayerId(0) }
                ));
                if before_resolution {
                    assert!(g.state.rules.effects.is_empty(), "skip modifier AND grant");
                }
                cast(g, q, 1, vec![]);
                assert_ne!(object(g, 1), old);
                stats(g, 1, 2, 2);
                assert!(g.current_keywords(object(g, 1)).is_empty());
                assert!(g.targetable_by(object(g, 1), PlayerId(1)));
                if !before_resolution {
                    for seat in 0..4 {
                        assert!(
                            matches!(g.visible_effects(PlayerId(seat)).unwrap().last().unwrap(),
                            VisibleEffect::GrantKeywords { target, .. } if target.current.is_none())
                        );
                    }
                }
                answer(g, q, Response::Cast { card: CardId(3) });
                let r = request(g, q);
                let QueryValue::Decision(Some(PlayerRequest::Targets {
                    token, candidates, ..
                })) = finish_query(g, g.player_decision_for(PlayerId(0), Budget::new(0)))
                else {
                    panic!("reentered target query")
                };
                assert_eq!(candidates.len(), 1);
                assert_ne!(candidates[0], old_handle);
                let before = g.state.clone();
                let trace = g.trace.clone();
                assert!(matches!(
                    g.check_player_response(
                        PlayerId(0),
                        token.clone(),
                        PlayerResponse::Targets {
                            objects: vec![old_handle]
                        },
                        Budget::unlimited()
                    ),
                    QueryResult::Rejected(QueryRejection::InvalidResponse)
                ));
                assert!(
                    g.respond(
                        r.player(),
                        r.decision_id(),
                        Response::Targets { objects: vec![old] }
                    )
                    .is_err()
                );
                assert_eq!(g.state, before);
                assert_eq!(g.trace, trace);
                let QueryValue::Validated(receipt) = finish_query(
                    g,
                    g.check_player_response(
                        PlayerId(0),
                        token,
                        PlayerResponse::Targets {
                            objects: candidates,
                        },
                        Budget::new(0),
                    ),
                ) else {
                    panic!("current incarnation validates")
                };
                g.submit(receipt).unwrap();
                equipped_indestructible_tests::pay_and_resolve(g, q, vec![]);
                stats(g, 1, 5, 5);
                assert!(g.current_keywords(object(g, 1)).is_empty());
                recover(g);
            });
        }
    }
}

#[test]
fn controlled_creature_protection_hexproof_spell_ability_candidates_and_cleanup_reopen() {
    both(
        prepared(&[
            ("Dive Down", 0, 0, true),
            ("Grizzly Bears", 2, 0, false),
            ("Prodigal Sorcerer", 1, 1, false),
            ("Prodigal Sorcerer", 1, 1, false),
            ("Murder", 1, 1, true),
            ("Island", 0, 0, false),
            ("Swamp", 1, 1, false),
            ("Swamp", 1, 1, false),
            ("Swamp", 1, 1, false),
        ]),
        |g, q| {
            let turn = g.turn().number;
            own_main_after_untap(g, q, PlayerId(1), turn);
            answer(
                g,
                q,
                Response::ActivateAbility {
                    object: object(g, 2),
                    ability: 0,
                },
            );
            answer(
                g,
                q,
                Response::EntityTargets {
                    entities: target(g, 1),
                },
            );
            assert!(matches!(request(g, q), Request::Priority { .. }));
            assert!(g.current(object(g, 2)).unwrap().tapped);
            until_player(g, q, PlayerId(0));
            announce(g, q, 0, target(g, 1));
            let protection = g.state.rules.stack.last().unwrap().id;
            for _ in 0..100 {
                if !g.state.rules.stack.iter().any(|e| e.id == protection) {
                    break;
                }
                answer(g, q, Response::Pass);
                request(g, q);
            }
            assert_eq!(
                g.state.rules.stack.len(),
                1,
                "old opposing ping remains announced"
            );
            assert_grant(g, &SOURCES[1], 1);
            until_player(g, q, PlayerId(1));
            answer(
                g,
                q,
                Response::ActivateAbility {
                    object: object(g, 3),
                    ability: 0,
                },
            );
            let r = request(g, q);
            let Request::EntityTargets { candidates, .. } = &r else {
                panic!("ping targets")
            };
            assert!(!candidates.contains(&TargetEntity::Object {
                object: object(g, 1)
            }));
            let before = g.state.clone();
            let trace = g.trace.clone();
            let QueryValue::Observation(view) =
                finish_query(g, g.observe(PlayerId(1), Budget::new(0)))
            else {
                panic!("opponent observation")
            };
            let handle = view
                .view
                .objects
                .iter()
                .find(|o| o.owner == PlayerId(2) && o.zone == Zone::Battlefield)
                .unwrap()
                .handle
                .clone();
            let PlayerRequest::EntityTargets {
                token, candidates, ..
            } = view.decision.unwrap()
            else {
                panic!("public ping targets")
            };
            let illegal = PlayerTargetEntity::Object { object: handle };
            assert!(!candidates.contains(&illegal));
            assert!(matches!(
                g.check_player_response(
                    PlayerId(1),
                    token,
                    PlayerResponse::EntityTargets {
                        entities: vec![illegal]
                    },
                    Budget::unlimited()
                ),
                QueryResult::Rejected(QueryRejection::InvalidResponse)
            ));
            assert!(
                g.respond(
                    r.player(),
                    r.decision_id(),
                    Response::EntityTargets {
                        entities: target(g, 1)
                    }
                )
                .is_err()
            );
            assert_eq!(g.state, before);
            assert_eq!(g.trace, trace);
            answer(
                g,
                q,
                Response::EntityTargets {
                    entities: vec![TargetEntity::Player {
                        player: PlayerId(0),
                    }],
                },
            );
            request(g, q);
            resolve(g, q);
            assert_eq!(
                g.current(object(g, 1)).unwrap().marked_damage,
                0.into(),
                "old ping rechecks new Hexproof"
            );
            assert_eq!(
                g.state.rules.players[0].life,
                39.into(),
                "second ping's legal player target resolves"
            );
            until_player(g, q, PlayerId(1));
            answer(g, q, Response::Cast { card: CardId(4) });
            let r = request(g, q);
            let Request::Targets { candidates, .. } = &r else {
                panic!("Murder targets")
            };
            assert!(!candidates.contains(&object(g, 1)));
            assert!(candidates.contains(&object(g, 3)));
            let QueryValue::Decision(Some(PlayerRequest::Targets {
                token, candidates, ..
            })) = finish_query(g, g.player_decision_for(PlayerId(1), Budget::new(0)))
            else {
                panic!("public Murder targets")
            };
            let QueryValue::Observation(view) =
                finish_query(g, g.observe(PlayerId(1), Budget::new(0)))
            else {
                panic!("opponent view")
            };
            let protected = view
                .view
                .objects
                .iter()
                .find(|o| o.owner == PlayerId(2) && o.zone == Zone::Battlefield)
                .unwrap()
                .handle
                .clone();
            assert!(!candidates.contains(&protected));
            assert!(matches!(
                g.check_player_response(
                    PlayerId(1),
                    token,
                    PlayerResponse::Targets {
                        objects: vec![protected]
                    },
                    Budget::unlimited()
                ),
                QueryResult::Rejected(QueryRejection::InvalidResponse)
            ));
            equipped_indestructible_tests::pay_and_resolve(g, q, target(g, 3));
            assert!(matches!(
                g.current(object(g, 3)).unwrap().zone,
                Zone::Graveyard { .. }
            ));
            recover(g);
            through_cleanup(g, q);
            assert!(g.state.rules.effects.is_empty());
            assert!(g.targetable_by(object(g, 1), PlayerId(1)));
            let turn = g.turn().number;
            own_main_after_untap(g, q, PlayerId(1), turn);
            assert!(
                !g.current(object(g, 2)).unwrap().tapped,
                "wait for actual source controller's untap"
            );
            answer(
                g,
                q,
                Response::ActivateAbility {
                    object: object(g, 2),
                    ability: 0,
                },
            );
            let r = request(g, q);
            assert!(matches!(&r, Request::EntityTargets { candidates, .. }
            if candidates.contains(&TargetEntity::Object { object: object(g, 1) })));
            answer(
                g,
                q,
                Response::EntityTargets {
                    entities: target(g, 1),
                },
            );
            request(g, q);
            resolve(g, q);
            assert_eq!(g.current(object(g, 1)).unwrap().marked_damage, 1.into());
            stats(g, 1, 2, 2);
        },
    );
}

#[test]
fn controlled_creature_protection_paused_compound_resolution_query_restore_replay() {
    for witness in SOURCES.iter().filter(|w| w.modifier.is_some()) {
        both(
            prepared(&[
                (witness.name, 0, 0, true),
                ("Grizzly Bears", 2, 0, false),
                (witness.land, 0, 0, false),
            ]),
            |g, q| {
                announce(g, q, 0, target(g, 1));
                let mut captured = false;
                for _ in 0..2000 {
                    match g.advance(Budget::new(q)) {
                        Advance::Suspended { .. } => {
                            if g.state.rules.effects.is_empty() {
                                stats(g, 1, 2, 2);
                                assert!(g.current_keywords(object(g, 1)).is_empty());
                            } else {
                                assert_grant(g, witness, 1);
                            }
                            if !captured
                                && !g.state.rules.effects.is_empty()
                                && matches!(g.state.rules.frame, Frame::Resolve { .. })
                            {
                                assert!(g.state.rules.pending.is_none());
                                let before = g.state.clone();
                                let trace = g.trace.clone();
                                for seat in 0..4 {
                                    assert!(matches!(
                                        finish_query(
                                            g,
                                            g.player_decision_for(PlayerId(seat), Budget::new(0))
                                        ),
                                        QueryValue::Decision(None)
                                    ));
                                }
                                let QueryResult::Incomplete { work, .. } =
                                    g.observe(PlayerId(1), Budget::new(0))
                                else {
                                    panic!("paused observation query")
                                };
                                let saved = g.save_work(&work).unwrap();
                                let restored = Game::restore(
                                    g.prepared.as_ref().clone(),
                                    &g.snapshot().unwrap(),
                                )
                                .unwrap();
                                let work = restored.restore_work(&saved).unwrap();
                                let QueryValue::Observation(view) = finish_query(
                                    &restored,
                                    restored.resume_work(work, Budget::new(1)),
                                ) else {
                                    panic!("restored paused observation")
                                };
                                assert!(view.decision.is_none());
                                assert_eq!(g.state, before);
                                assert_eq!(g.trace, trace);
                                recover(g);
                                captured = true;
                            }
                        }
                        Advance::AwaitingDecision(r) => {
                            if g.state.rules.stack.is_empty() {
                                break;
                            }
                            g.respond(r.player(), r.decision_id(), pass(&r)).unwrap();
                        }
                        x => panic!("compound resolution {x:?}"),
                    }
                }
                if q == 1 {
                    assert!(captured, "actual suspended complete atomic grant");
                }
                assert_grant(g, witness, 1);
            },
        );
    }
}
