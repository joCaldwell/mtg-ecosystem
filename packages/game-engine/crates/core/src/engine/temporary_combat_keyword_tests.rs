//! Private paid, complete-source family witnesses; no pool/policy admission.
//! CR 400.7, 608.2b/c/h, 611.2a/c, 613.1f/4c, 302.6, 510.4, 702.2/7/10/15/19, 704.5g/h, 514.2:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
//! New-source native tests are UNRUN here; the parent owns combined execution.
use super::equipped_indestructible_tests::{
    answer, both, cast, equip, object, pay_and_resolve, recover, request,
};
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
        name: "Tread Upon",
        raw: include_str!("../../../../tests/fixtures/0b3bdbc6-cde7-4187-b13c-82fd02ae4037.json"),
        land: "Forest",
        cost: 2,
        modifier: Some((2, 2)),
        keywords: &[KeywordAbility::Trample],
    },
    Witness {
        name: "Fervent Strike",
        raw: include_str!("../../../../tests/fixtures/35efb1e6-6be2-4a38-969a-199524d48e02.json"),
        land: "Mountain",
        cost: 1,
        modifier: Some((1, 0)),
        keywords: &[KeywordAbility::FirstStrike, KeywordAbility::Haste],
    },
    Witness {
        name: "Whirling Strike",
        raw: include_str!("../../../../tests/fixtures/6b37b0ef-7e89-447f-a189-b3ea7b42143f.json"),
        land: "Mountain",
        cost: 2,
        modifier: Some((2, 0)),
        keywords: &[KeywordAbility::FirstStrike, KeywordAbility::Trample],
    },
    Witness {
        name: "Uncanny Speed",
        raw: include_str!("../../../../tests/fixtures/85b0a2ea-dbdc-456e-9ce1-faa8c13f84d0.json"),
        land: "Mountain",
        cost: 2,
        modifier: Some((3, 0)),
        keywords: &[KeywordAbility::Haste],
    },
    Witness {
        name: "Unnatural Speed",
        raw: include_str!("../../../../tests/fixtures/fa46ced5-b701-4aa3-b4a1-36fb15b80696.json"),
        land: "Mountain",
        cost: 1,
        modifier: None,
        keywords: &[KeywordAbility::Haste],
    },
];

fn prepared(rows: &[(&str, bool)]) -> Prepared {
    let placeholders: Vec<_> = rows
        .iter()
        .map(|(name, hand)| {
            (
                if SOURCES.iter().any(|s| s.name == *name) || *name == "Basilisk Collar" {
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
    for raw in SOURCES.iter().map(|s| s.raw).chain([include_str!(
        "../../../../tests/fixtures/f5f4dd28-f4ae-4d39-b9b8-6ebfd63c93fe.json"
    )]) {
        let mtg_compiler::CompileResult::Lowered { definition: d } = mtg_compiler::compile(
            include_str!("../../../../tests/fixtures/archive-manifest-current.json"),
            raw,
        ) else {
            panic!("complete frozen source")
        };
        for (i, (name, _)) in rows.iter().enumerate() {
            if *name == d.name {
                p.config.cards[i].oracle_id = d.oracle_id.clone();
            }
        }
        c.definitions.push(*d);
    }
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    p.catalog_digest = digest(&c).unwrap();
    p.catalog = Arc::new(c);
    p.profile_digest = digest(&p.config).unwrap();
    p
}
fn other_controller(mut p: Prepared, card: usize, controller: u8, owner: u8) -> Prepared {
    p.config.cards[card].controller = PlayerId(controller);
    p.config.cards[card].owner = PlayerId(owner);
    p.profile_digest = digest(&p.config).unwrap();
    p
}
fn grave(g: &Game, id: u32) -> bool {
    matches!(
        g.current(object(g, id)).unwrap().zone,
        Zone::Graveyard { .. }
    )
}
fn stats(g: &Game, id: u32, power: i64, toughness: i64) {
    let c = g.derived_characteristics(object(g, id)).unwrap();
    assert_eq!(c.power.unwrap(), power.into());
    assert_eq!(c.toughness.unwrap(), toughness.into());
}
fn assert_grant(g: &Game, witness: &Witness, id: u32) {
    let (p, t) = witness.modifier.unwrap_or((0, 0));
    stats(g, id, 2 + p, 2 + t);
    assert_eq!(
        g.state.rules.effects.len(),
        1 + usize::from(witness.modifier.is_some())
    );
    if let Some((power, toughness)) = witness.modifier {
        assert!(matches!(&g.state.rules.effects[0].kind,
            ResolvedEffectKind::ModifyPowerToughness { target, power: p, toughness: t }
            if *target == object(g, id) && *p == power.into() && *t == toughness.into()));
    }
    let grant = g.state.rules.effects.last().unwrap();
    assert!(
        matches!(&grant.kind, ResolvedEffectKind::GrantKeywords { target, keywords }
        if *target == object(g, id) && keywords == witness.keywords)
    );
    assert_eq!(grant.origin.source_block, 0);
    assert_eq!(grant.origin.cursor, u32::from(witness.modifier.is_some()));
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

#[test]
fn temporary_combat_keyword_paid_vectors_modifiers_cleanup_and_forgery() {
    for witness in SOURCES {
        both(
            prepared(&[
                (witness.name, true),
                ("Grizzly Bears", false),
                (witness.land, false),
                (witness.land, false),
            ]),
            |g, q| {
                cast(g, q, 0, target(g, 1));
                assert_grant(g, witness, 1);
                assert!(grave(g, 0), "source spell departed normally");
                let before_queries = g.state.clone();
                let trace = g.trace.clone();
                for viewer in 0..4 {
                    assert!(
                        matches!(g.visible_effects(PlayerId(viewer)).unwrap().last().unwrap(),
                        VisibleEffect::GrantKeywords { target, keywords, .. }
                        if target.current.is_some() && keywords == witness.keywords)
                    );
                }
                assert_eq!(g.state, before_queries);
                assert_eq!(g.trace, trace);
                assert_eq!(
                    g.state.rules.objects.values().filter(|o| o.tapped).count(),
                    witness.cost
                );
                recover(g); // Reachable positive control before normalized forgeries.
                let good = g.snapshot().unwrap();
                Game::restore(g.prepared.as_ref().clone(), &good).unwrap();
                for variant in 0..9 {
                    let mut bad: Snapshot = strict_read(&good).unwrap();
                    let e = bad.state.rules.effects.last_mut().unwrap();
                    let ResolvedEffectKind::GrantKeywords { target, keywords } = &mut e.kind else {
                        unreachable!()
                    };
                    match variant {
                        0 => keywords.clear(),
                        1 => keywords.push(keywords[0]),
                        2 => *keywords = vec![KeywordAbility::Vigilance],
                        3 => {
                            *keywords = if keywords == &[KeywordAbility::Flying] {
                                vec![KeywordAbility::Lifelink]
                            } else {
                                vec![KeywordAbility::Flying]
                            }
                        }
                        4 => target.incarnation += 1,
                        5 => e.origin.source_block += 1,
                        6 => e.origin.source.incarnation += 1,
                        7 => e.origin.cursor += 1,
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
                assert!(g.current_keywords(object(g, 1)).is_empty());
                stats(g, 1, 2, 2);
                assert_eq!(g.current(object(g, 1)).unwrap().marked_damage, 0.into());
            },
        );
    }
}

#[test]
fn temporary_combat_keyword_target_departure_reentry_and_resolution_recheck() {
    for witness in SOURCES {
        for before_resolution in [false, true] {
            both(
                prepared(&[
                    (witness.name, true),
                    ("Grizzly Bears", false),
                    ("Unsummon", true),
                    (witness.land, false),
                    (witness.land, false),
                    ("Island", false),
                    ("Forest", false),
                    ("Forest", false),
                ]),
                |g, q| {
                    let old = object(g, 1);
                    if before_resolution {
                        announce(g, q, 0, target(g, 1));
                    } else {
                        cast(g, q, 0, target(g, 1));
                        assert_grant(g, witness, 1);
                    }
                    cast(g, q, 2, target(g, 1));
                    assert_eq!(
                        g.state.rules.effects.len(),
                        if before_resolution {
                            0
                        } else {
                            1 + usize::from(witness.modifier.is_some())
                        }
                    );
                    cast(g, q, 1, vec![]);
                    assert_ne!(object(g, 1), old);
                    stats(g, 1, 2, 2);
                    assert!(g.current_keywords(object(g, 1)).is_empty());
                    let before = g.state.clone();
                    let trace = g.trace.clone();
                    for viewer in 0..4 {
                        let visible = g.visible_effects(PlayerId(viewer)).unwrap();
                        if !before_resolution {
                            assert!(
                                matches!(visible.last().unwrap(), VisibleEffect::GrantKeywords { target, .. }
                            if target.current.is_none())
                            );
                        }
                    }
                    assert_eq!(g.state, before);
                    assert_eq!(g.trace, trace);
                },
            );
        }
    }
}

#[test]
fn temporary_combat_keyword_opponent_queries_invalid_responses_atomic() {
    for witness in SOURCES {
        let p = other_controller(
            prepared(&[
                (witness.name, true),
                ("Grizzly Bears", false),
                (witness.land, false),
                (witness.land, false),
            ]),
            1,
            1,
            1,
        );
        both(p, |g, q| {
            answer(g, q, Response::Cast { card: CardId(0) });
            let r = request(g, q);
            assert!(matches!(r, Request::Targets { .. }));
            let before = g.state.clone();
            let trace = g.trace.clone();
            for viewer in 1..4 {
                assert!(matches!(
                    g.player_decision_for(PlayerId(viewer), Budget::unlimited()),
                    QueryResult::Complete(QueryValue::Decision(None))
                ));
            }
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
                vec![],
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
            pay_and_resolve(g, q, vec![]);
            assert_grant(g, witness, 1);
        });
    }
}

#[test]
fn temporary_combat_keyword_suspended_compound_resolution_restore_and_replay() {
    for witness in SOURCES.iter().filter(|s| s.modifier.is_some()) {
        both(
            prepared(&[
                (witness.name, true),
                ("Grizzly Bears", false),
                (witness.land, false),
                (witness.land, false),
            ]),
            |g, q| {
                announce(g, q, 0, target(g, 1));
                let mut prefixes = 0;
                for _ in 0..2000 {
                    match g.advance(Budget::new(q)) {
                        Advance::Suspended { .. } => {
                            if g.state.rules.effects.len() == 1 {
                                let (p, t) = witness.modifier.unwrap();
                                stats(g, 1, 2 + p, 2 + t);
                                assert!(g.current_keywords(object(g, 1)).is_empty());
                                assert!(matches!(g.state.rules.frame, Frame::Resolve { .. }));
                                assert!(g.state.rules.pending.is_none());
                                if prefixes == 0 {
                                    recover(g);
                                }
                                prefixes += 1;
                            }
                        }
                        Advance::AwaitingDecision(r) => {
                            if g.state.rules.stack.is_empty() {
                                break;
                            }
                            g.respond(r.player(), r.decision_id(), pass(&r)).unwrap();
                        }
                        x => panic!("compound prefix {x:?}"),
                    }
                }
                if q == 1 {
                    assert!(prefixes > 0, "actual intermediate modifier prefix");
                }
                assert_grant(g, witness, 1);
            },
        );
    }
}

fn seek(g: &mut Game, q: u64, predicate: impl Fn(&Game, &Request) -> bool) -> Request {
    for _ in 0..300 {
        let r = request(g, q);
        if predicate(g, &r) {
            return r;
        }
        answer(g, q, pass(&r));
    }
    panic!("bounded combat/priority boundary");
}
fn declare(g: &mut Game, q: u64, attacker: u32, blockers: &[u32]) {
    let r = seek(g, q, |_, r| matches!(r, Request::Attackers { .. }));
    assert!(
        matches!(&r, Request::Attackers { candidates, .. } if candidates.contains(&object(g, attacker)))
    );
    answer(
        g,
        q,
        Response::Attackers {
            attacks: vec![(object(g, attacker), PlayerId(1))],
        },
    );
    seek(g, q, |_, r| matches!(r, Request::Blockers { .. }));
    answer(
        g,
        q,
        Response::Blockers {
            blocks: blockers
                .iter()
                .map(|b| (object(g, *b), object(g, attacker)))
                .collect(),
        },
    );
}
fn end_combat(g: &mut Game, q: u64) {
    seek(g, q, |g, _| g.turn().step == TurnStep::EndCombat);
}
fn rejected_activation(g: &mut Game, q: u64, id: u32) {
    let r = request(g, q);
    let before = g.snapshot().unwrap();
    assert!(
        g.respond(
            r.player(),
            r.decision_id(),
            Response::ActivateAbility {
                object: object(g, id),
                ability: 0
            }
        )
        .unwrap_err()
        .contains("continuously controlled")
    );
    assert_eq!(before, g.snapshot().unwrap());
}

#[test]
fn temporary_combat_keyword_haste_new_creature_attack_tap_authority_and_expiry() {
    for witness in SOURCES
        .iter()
        .filter(|w| w.keywords.contains(&KeywordAbility::Haste))
    {
        for attack in [false, true] {
            let mut rows = vec![
                (witness.name, true),
                (witness.name, true),
                (
                    if attack {
                        "Grizzly Bears"
                    } else {
                        "Prodigal Sorcerer"
                    },
                    true,
                ),
                ("Prodigal Sorcerer", true),
                ("Yoked Ox", false),
            ];
            // Real casts establish entry this turn; initial lands are explicit scenario setup.
            rows.extend(vec![("Island", false); 6]);
            rows.extend(vec![("Forest", false); 2]);
            rows.extend(vec![("Mountain", false); 4]);
            both(prepared(&rows), |g, q| {
                cast(g, q, 2, vec![]);
                cast(g, q, 3, vec![]);
                assert!(!g.current(object(g, 2)).unwrap().attack_ready);
                assert!(!g.current(object(g, 3)).unwrap().attack_ready);
                if !attack {
                    rejected_activation(g, q, 2);
                }
                assert!(!g.creatures(PlayerId(0), true).contains(&object(g, 2)));
                cast(g, q, 0, target(g, 2));
                cast(g, q, 1, target(g, 3));
                assert!(g.creatures(PlayerId(0), true).contains(&object(g, 2)));
                if attack {
                    declare(g, q, 2, &[]);
                    end_combat(g, q);
                    let bonus = witness.modifier.unwrap_or((0, 0)).0;
                    assert_eq!(g.state.rules.players[1].life, (38 - bonus).into());
                } else {
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
                            entities: target(g, 4),
                        },
                    );
                    while !g.state.rules.stack.is_empty() {
                        answer(g, q, Response::Pass);
                        request(g, q);
                    }
                    assert!(g.current(object(g, 2)).unwrap().tapped);
                    assert_eq!(g.current(object(g, 4)).unwrap().marked_damage, 1.into());
                }
                recover(g);
                through_cleanup(g, q);
                // The untouched new Sorcerer is untapped but not continuously controlled
                // since its controller's turn began; cleanup alone must revoke authority.
                until_player(g, q, PlayerId(0));
                assert!(!g.current(object(g, 3)).unwrap().tapped);
                assert!(!g.current(object(g, 3)).unwrap().attack_ready);
                assert!(!g.has_keyword(object(g, 3), KeywordAbility::Haste));
                rejected_activation(g, q, 3);
            });
        }
    }
}

#[test]
fn temporary_combat_keyword_first_strike_kills_before_ordinary_damage() {
    both(
        other_controller(
            prepared(&[
                ("Fervent Strike", true),
                ("Grizzly Bears", false),
                ("Grizzly Bears", false),
                ("Mountain", false),
            ]),
            2,
            1,
            1,
        ),
        |g, q| {
            cast(g, q, 0, target(g, 1));
            declare(g, q, 1, &[2]);
            seek(g, q, |g, r| {
                g.turn().step == TurnStep::FirstStrikeDamage
                    && matches!(r, Request::Priority { .. })
            });
            assert!(grave(g, 2));
            assert_eq!(g.current(object(g, 1)).unwrap().marked_damage, 0.into());
            assert!(
                g.state
                    .rules
                    .first_strike_participants
                    .contains(&object(g, 1))
            );
            recover(g);
            end_combat(g, q);
            assert_eq!(g.current(object(g, 1)).unwrap().zone, Zone::Battlefield);
            assert_eq!(g.current(object(g, 1)).unwrap().marked_damage, 0.into());
            assert_eq!(g.state.rules.players[1].life, 40.into());
        },
    );
}

#[test]
fn temporary_combat_keyword_late_first_strike_preserves_ordinary_eligibility() {
    both(
        other_controller(
            prepared(&[
                ("Fervent Strike", true),
                ("Giant Spider", false),
                ("Youthful Knight", false),
                ("Mountain", false),
            ]),
            2,
            1,
            1,
        ),
        |g, q| {
            declare(g, q, 1, &[2]);
            seek(g, q, |g, r| {
                g.turn().step == TurnStep::FirstStrikeDamage
                    && matches!(
                        r,
                        Request::Priority {
                            player: PlayerId(0),
                            ..
                        }
                    )
            });
            let attacker = object(g, 1);
            let blocker = object(g, 2);
            assert_eq!(g.current(attacker).unwrap().marked_damage, 2.into());
            assert_eq!(g.current(blocker).unwrap().marked_damage, 0.into());
            assert_eq!(
                g.state.rules.first_strike_participants,
                BTreeSet::from([blocker])
            );
            cast(g, q, 0, target(g, 1));
            assert_eq!(g.turn().step, TurnStep::FirstStrikeDamage);
            assert_eq!(
                g.state.rules.first_strike_participants,
                BTreeSet::from([blocker])
            );
            assert_eq!(
                g.current(blocker).unwrap().marked_damage,
                0.into(),
                "no invented extra first-strike damage"
            );
            recover(g);
            let mut forged: Snapshot = strict_read(&g.snapshot().unwrap()).unwrap();
            forged
                .state
                .rules
                .first_strike_participants
                .insert(attacker);
            forged.state_sha256 = digest(&forged.state).unwrap();
            assert!(
                Game::restore(g.prepared.as_ref().clone(), &canonical(&forged).unwrap()).is_err()
            );
            end_combat(g, q);
            assert!(
                grave(g, 2),
                "late first strike must preserve ordinary damage eligibility"
            );
            assert_eq!(
                g.current(attacker).unwrap().marked_damage,
                2.into(),
                "blocker cannot deal damage twice"
            );
            assert_eq!(g.state.rules.players[1].life, 40.into());
        },
    );
}

#[test]
fn temporary_combat_keyword_trample_exact_excess_dt_lifelink_and_duplicate_free_union() {
    for (source, collar, blocker, lethal, excess) in [
        (0, false, "Grizzly Bears", 2, 2),
        (2, false, "Grizzly Bears", 2, 2),
        (2, true, "Yoked Ox", 1, 3),
    ] {
        let witness = &SOURCES[source];
        let mut rows = vec![
            (witness.name, true),
            ("Grizzly Bears", false),
            (blocker, false),
            ("Basilisk Collar", false),
            ("Swiftfoot Boots", false),
        ];
        rows.extend(vec![(witness.land, false); 6]);
        both(other_controller(prepared(&rows), 2, 1, 1), |g, q| {
            if collar {
                equip(g, q, 3, 1);
                equip(g, q, 4, 1);
            }
            cast(g, q, 0, target(g, 1));
            if collar {
                // Complete paid grants compose with exact equipment sources.
                for k in [
                    KeywordAbility::FirstStrike,
                    KeywordAbility::Trample,
                    KeywordAbility::Deathtouch,
                    KeywordAbility::Lifelink,
                    KeywordAbility::Haste,
                    KeywordAbility::Hexproof,
                ] {
                    assert_eq!(
                        g.current_keywords(object(g, 1))
                            .iter()
                            .filter(|x| **x == k)
                            .count(),
                        1
                    );
                }
            }
            declare(g, q, 1, &[2]);
            let r = seek(g, q, |_, r| matches!(r, Request::Damage { .. }));
            let victim = object(g, 2);
            assert!(
                matches!(&r, Request::Damage { player_recipient: Some(PlayerId(1)), lethal_requirements, .. } if lethal_requirements == &vec![(victim, lethal.into())])
            );
            let before = g.snapshot().unwrap();
            assert!(
                g.respond(
                    r.player(),
                    r.decision_id(),
                    Response::TrampleDamage {
                        assignments: vec![(victim, (lethal - 1).into())],
                        player: PlayerId(1),
                        amount: (excess + 1).into()
                    }
                )
                .is_err()
            );
            assert_eq!(before, g.snapshot().unwrap());
            answer(
                g,
                q,
                Response::TrampleDamage {
                    assignments: vec![(victim, lethal.into())],
                    player: PlayerId(1),
                    amount: excess.into(),
                },
            );
            end_combat(g, q);
            assert!(grave(g, 2));
            assert_eq!(g.state.rules.players[1].life, (40 - excess).into());
            assert_eq!(
                g.state.rules.players[0].life,
                (if collar { 44 } else { 40 }).into()
            );
            assert_eq!(
                g.current(object(g, 1)).unwrap().marked_damage,
                (if source == 0 { 2 } else { 0 }).into()
            );
        });
    }
    both(
        prepared(&[
            ("Fervent Strike", true),
            ("Knight of Meadowgrain", false),
            ("Swiftfoot Boots", false),
            ("Mountain", false),
            ("Mountain", false),
        ]),
        |g, q| {
            equip(g, q, 2, 1);
            cast(g, q, 0, target(g, 1));
            for k in [
                KeywordAbility::FirstStrike,
                KeywordAbility::Haste,
                KeywordAbility::Lifelink,
                KeywordAbility::Hexproof,
            ] {
                assert_eq!(
                    g.current_keywords(object(g, 1))
                        .iter()
                        .filter(|x| **x == k)
                        .count(),
                    1
                );
            }
            through_cleanup(g, q);
            for k in [
                KeywordAbility::FirstStrike,
                KeywordAbility::Haste,
                KeywordAbility::Lifelink,
                KeywordAbility::Hexproof,
            ] {
                assert_eq!(
                    g.current_keywords(object(g, 1))
                        .iter()
                        .filter(|x| **x == k)
                        .count(),
                    1
                );
            }
        },
    );
}
