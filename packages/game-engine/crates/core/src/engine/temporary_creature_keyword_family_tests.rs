//! Private paid, complete-source family witnesses; no pool/policy admission.
//! CR 400.7, 608.2b/c/h, 611.2a/c, 613.1f/4c, 702.2/9/12/15, 704.5g/h, 514.2:
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
        name: "Grotesque Mutation",
        raw: include_str!("../../../../tests/fixtures/058a8475-649f-433a-b258-42bc03bf40ba.json"),
        land: "Swamp",
        cost: 2,
        modifier: Some((3, 1)),
        keywords: &[KeywordAbility::Lifelink],
    },
    Witness {
        name: "Battle-Rage Blessing",
        raw: include_str!("../../../../tests/fixtures/533d6b9f-a145-44c4-9d16-74fcd9c0ae2f.json"),
        land: "Swamp",
        cost: 2,
        modifier: None,
        keywords: &[KeywordAbility::Deathtouch, KeywordAbility::Indestructible],
    },
    Witness {
        name: "Mighty Leap",
        raw: include_str!("../../../../tests/fixtures/593caa15-e1d1-477f-bbb4-124d7a7b81f3.json"),
        land: "Plains",
        cost: 2,
        modifier: Some((2, 2)),
        keywords: &[KeywordAbility::Flying],
    },
    Witness {
        name: "Rush of Vitality",
        raw: include_str!("../../../../tests/fixtures/6e460d39-0f18-4a2d-9c8d-3eaba301d285.json"),
        land: "Swamp",
        cost: 2,
        modifier: Some((1, 0)),
        keywords: &[KeywordAbility::Lifelink, KeywordAbility::Indestructible],
    },
    Witness {
        name: "Coat with Venom",
        raw: include_str!("../../../../tests/fixtures/cbc808dd-8d50-4f42-b0f1-d4e45d0462a2.json"),
        land: "Swamp",
        cost: 1,
        modifier: Some((1, 2)),
        keywords: &[KeywordAbility::Deathtouch],
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
fn resolve(g: &mut Game, q: u64) {
    while !g.state.rules.stack.is_empty() {
        answer(g, q, Response::Pass);
        request(g, q);
    }
}
fn activate_damage(g: &mut Game, q: u64, source: u32, victim: u32) {
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
fn combat(g: &mut Game, q: u64, attacker: u32, blocker: Option<u32>) {
    for _ in 0..200 {
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
            attacks: vec![(object(g, attacker), PlayerId(1))],
        },
    );
    for _ in 0..200 {
        let r = request(g, q);
        if matches!(r, Request::Blockers { .. }) {
            break;
        }
        answer(g, q, pass(&r));
    }
    answer(
        g,
        q,
        Response::Blockers {
            blocks: blocker
                .map(|b| vec![(object(g, b), object(g, attacker))])
                .unwrap_or_default(),
        },
    );
    for _ in 0..200 {
        let r = request(g, q);
        if g.turn().step == TurnStep::EndCombat {
            return;
        }
        answer(g, q, pass(&r));
    }
    panic!("bounded real combat");
}

#[test]
fn temporary_creature_keyword_family_paid_vectors_modifiers_cleanup_and_forgery() {
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
                        2 => *keywords = vec![KeywordAbility::Haste],
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
fn temporary_creature_keyword_family_target_departure_reentry_and_resolution_recheck() {
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
fn temporary_creature_keyword_family_opponent_queries_invalid_responses_atomic() {
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
fn temporary_creature_keyword_family_real_combat_dt_protection_lifelink_and_flight() {
    // Different modifiers and vectors use the same paid grants and combat systems.
    for (source, attacker, blocker, gain, dies) in [
        (0, "Grizzly Bears", None, 5, false),
        (1, "Grizzly Bears", Some("Giant Spider"), 0, true),
        (1, "Typhoid Rats", Some("Youthful Knight"), 0, true),
        (2, "Grizzly Bears", None, 0, false),
        (3, "Grizzly Bears", Some("Yargle and Multani"), 3, false),
        (4, "Grizzly Bears", Some("Yoked Ox"), 0, true),
    ] {
        let witness = &SOURCES[source];
        let p = other_controller(
            prepared(&[
                (witness.name, true),
                (attacker, false),
                (blocker.unwrap_or("Yoked Ox"), false),
                (witness.land, false),
                (witness.land, false),
            ]),
            2,
            1,
            1,
        );
        both(p, |g, q| {
            cast(g, q, 0, target(g, 1));
            if source == 2 {
                // A ground creature cannot block the newly flying creature.
                for _ in 0..200 {
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
                for _ in 0..200 {
                    let r = request(g, q);
                    if matches!(r, Request::Blockers { .. }) {
                        break;
                    }
                    answer(g, q, pass(&r));
                }
                let r = request(g, q);
                let before = g.state.clone();
                assert!(
                    g.respond(
                        r.player(),
                        r.decision_id(),
                        Response::Blockers {
                            blocks: vec![(object(g, 2), object(g, 1))]
                        }
                    )
                    .is_err()
                );
                assert_eq!(g.state, before);
                answer(g, q, Response::Blockers { blocks: vec![] });
                for _ in 0..200 {
                    let r = request(g, q);
                    if g.turn().step == TurnStep::EndCombat {
                        break;
                    }
                    answer(g, q, pass(&r));
                }
                assert_eq!(g.state.rules.players[1].life, 36.into());
            } else {
                combat(g, q, 1, blocker.map(|_| 2));
            }
            assert!(!grave(g, 1), "{} attacker survived", witness.name);
            assert_eq!(grave(g, 2), dies);
            assert_eq!(g.state.rules.players[0].life, (40 + gain).into());
            if source == 1 && attacker == "Typhoid Rats" {
                assert_eq!(
                    g.current(object(g, 1)).unwrap().marked_damage,
                    2.into(),
                    "first-strike damage was survived before regular deathtouch damage"
                );
            }
            recover(g);
            through_cleanup(g, q);
            assert_eq!(g.current(object(g, 1)).unwrap().marked_damage, 0.into());
            assert!(g.state.rules.effects.is_empty());
        });
    }
}

#[test]
fn temporary_creature_keyword_family_paid_union_retains_intrinsic_and_equipped_keywords() {
    both(
        prepared(&[
            ("Rush of Vitality", true),
            ("Knight of Meadowgrain", false),
            ("Basilisk Collar", false),
            ("Swamp", false),
            ("Swamp", false),
            ("Swamp", false),
            ("Swamp", false),
        ]),
        |g, q| {
            equip(g, q, 2, 1);
            cast(g, q, 0, target(g, 1));
            let keywords = g.current_keywords(object(g, 1));
            for keyword in [
                KeywordAbility::Lifelink,
                KeywordAbility::Deathtouch,
                KeywordAbility::FirstStrike,
                KeywordAbility::Indestructible,
            ] {
                assert_eq!(keywords.iter().filter(|k| **k == keyword).count(), 1);
            }
            recover(g);
            through_cleanup(g, q);
            assert!(g.state.rules.effects.is_empty());
            let keywords = g.current_keywords(object(g, 1));
            assert!(!keywords.contains(&KeywordAbility::Indestructible));
            for keyword in [
                KeywordAbility::Lifelink,
                KeywordAbility::Deathtouch,
                KeywordAbility::FirstStrike,
            ] {
                assert_eq!(keywords.iter().filter(|k| **k == keyword).count(), 1);
            }
        },
    );
}

#[test]
fn temporary_creature_keyword_family_damage_current_departed_controller_prevented_and_restore() {
    for source in ["Prodigal Sorcerer", "Mogg Fanatic"] {
        for prevented in [false, true] {
            let p = other_controller(
                prepared(&[
                    ("Grotesque Mutation", true),
                    ("Coat with Venom", true),
                    (source, false),
                    ("Yoked Ox", false),
                    ("Samite Healer", false),
                    ("Swamp", false),
                    ("Swamp", false),
                    ("Swamp", false),
                ]),
                2,
                1,
                2,
            );
            both(p, |g, q| {
                cast(g, q, 0, target(g, 2));
                cast(g, q, 1, target(g, 2));
                assert_eq!(
                    g.state.rules.objects.values().filter(|o| o.tapped).count(),
                    3
                );
                assert!(g.has_keyword(object(g, 2), KeywordAbility::Lifelink));
                assert!(g.has_keyword(object(g, 2), KeywordAbility::Deathtouch));
                if prevented {
                    activate_damage(g, q, 4, 3); // Actual accepted Healer shield.
                    resolve(g, q);
                }
                until_player(g, q, PlayerId(1));
                activate_damage(g, q, 2, 3);
                assert_eq!(grave(g, 2), source == "Mogg Fanatic");
                let mut captured = false;
                for _ in 0..3000 {
                    match g.advance(Budget::new(q)) {
                        Advance::Suspended { .. } => {
                            if !captured
                                && let Frame::DamageEffects { transaction }
                                | Frame::DamageAllocation { transaction, .. } =
                                    &g.state.rules.frame
                            {
                                assert!(
                                    transaction.entries[0]
                                        .source
                                        .keywords
                                        .contains(&KeywordAbility::Lifelink)
                                );
                                assert!(
                                    transaction.entries[0]
                                        .source
                                        .keywords
                                        .contains(&KeywordAbility::Deathtouch)
                                );
                                captured = true;
                                recover(g);
                                let wire = g.snapshot().unwrap();
                                // Vocabulary alone cannot authenticate historical grants.
                                for variant in 0..4 {
                                    let mut bad: Snapshot = strict_read(&wire).unwrap();
                                    let (Frame::DamageEffects { transaction }
                                    | Frame::DamageAllocation { transaction, .. }) =
                                        &mut bad.state.rules.frame
                                    else {
                                        unreachable!()
                                    };
                                    let d = &mut transaction.entries[0];
                                    match variant {
                                        0 => d
                                            .source
                                            .keywords
                                            .retain(|k| *k != KeywordAbility::Lifelink),
                                        1 => d.source.keywords.push(KeywordAbility::Deathtouch),
                                        2 => d.source.controller = PlayerId(0),
                                        _ => d.source.object.incarnation += 1,
                                    }
                                    bad.state_sha256 = digest(&bad.state).unwrap();
                                    assert!(
                                        Game::restore(
                                            g.prepared.as_ref().clone(),
                                            &canonical(&bad).unwrap()
                                        )
                                        .is_err()
                                    );
                                }
                            }
                        }
                        Advance::AwaitingDecision(r) => {
                            if g.state.rules.stack.is_empty() {
                                break;
                            }
                            g.respond(r.player(), r.decision_id(), Response::Pass)
                                .unwrap();
                        }
                        x => panic!("bounded damage resolution {x:?}"),
                    }
                }
                assert!(g.state.rules.stack.is_empty());
                assert_eq!(grave(g, 3), !prevented, "actual deathtouch damage only");
                assert_eq!(
                    g.state.rules.players[0].life,
                    40.into(),
                    "grant caster gains nothing"
                );
                assert_eq!(
                    g.state.rules.players[1].life,
                    (if prevented { 40 } else { 41 }).into(),
                    "lifelink uses one actual damage and source controller"
                );
                assert_eq!(
                    g.state.rules.players[2].life,
                    40.into(),
                    "physical owner gains nothing"
                );
                if prevented {
                    assert_eq!(g.current(object(g, 3)).unwrap().marked_damage, 0.into());
                    assert!(!g.current(object(g, 3)).unwrap().deathtouch_damage);
                }
                if q == 1 {
                    assert!(captured, "actual suspended damage prefix captured");
                }
                if g.trace.mode == TraceMode::Detailed {
                    let commits: Vec<_> = g
                        .events()
                        .iter()
                        .filter_map(|e| match e {
                            Event::Committed {
                                actual: EventOutcome::DamageCommitted { actual, .. },
                                ..
                            } => Some(actual),
                            _ => None,
                        })
                        .collect();
                    assert_eq!(commits.len(), 1, "one actual damage transaction");
                    assert_eq!(commits[0].len(), usize::from(!prevented));
                    if !prevented {
                        assert_eq!(commits[0][0].amount, 1.into());
                        assert_eq!(commits[0][0].source.controller, PlayerId(1));
                    }
                    let gains: Vec<_> = g
                        .events()
                        .iter()
                        .filter_map(|e| match e {
                            Event::Committed {
                                actual: EventOutcome::DamageCommitted { life_gains, .. },
                                ..
                            } => Some(life_gains),
                            _ => None,
                        })
                        .flatten()
                        .collect();
                    assert_eq!(gains.len(), usize::from(!prevented));
                    if !prevented {
                        assert_eq!(gains[0].player, PlayerId(1));
                        assert_eq!(gains[0].amount, 1.into());
                    }
                }
            });
        }
    }
}

#[test]
fn temporary_creature_keyword_family_damage_time_grants_and_no_retroactive_damage() {
    for before_damage in [false, true] {
        both(
            prepared(&[
                ("Grotesque Mutation", true),
                ("Coat with Venom", true),
                ("Prodigal Sorcerer", false),
                ("Yoked Ox", false),
                ("Swamp", false),
                ("Swamp", false),
                ("Swamp", false),
            ]),
            |g, q| {
                activate_damage(g, q, 2, 3);
                if before_damage {
                    // Both paid instants resolve before the already-announced ping.
                    announce(g, q, 0, target(g, 2));
                    cast(g, q, 1, target(g, 2));
                    assert!(grave(g, 3));
                    assert_eq!(g.state.rules.players[0].life, 41.into());
                } else {
                    resolve(g, q);
                    assert_eq!(g.current(object(g, 3)).unwrap().marked_damage, 1.into());
                    cast(g, q, 0, target(g, 2));
                    cast(g, q, 1, target(g, 2));
                    assert!(!grave(g, 3));
                    assert!(!g.current(object(g, 3)).unwrap().deathtouch_damage);
                    assert_eq!(g.state.rules.players[0].life, 40.into());
                }
                through_cleanup(g, q);
                assert!(g.state.rules.effects.is_empty());
                if !before_damage {
                    assert_eq!(g.current(object(g, 3)).unwrap().marked_damage, 0.into());
                }
            },
        );
    }
}

#[test]
fn temporary_creature_keyword_family_suspended_compound_resolution_restore_and_replay() {
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

// CR 104.3a, 611.2c and 613.5–6: concession may interrupt a resolution,
// but departure observers must never consume only one layer of one effect.
#[test]
fn targeted_compound_atomic_departure_lki_keeps_all_characteristics() {
    let p = other_controller(
        prepared(&[
            ("Grotesque Mutation", true),
            ("Grizzly Bears", false),
            ("Swamp", false),
            ("Swamp", false),
        ]),
        1,
        0,
        2,
    );
    let mut g = Game::with_trace(p, TraceMode::Detailed);
    request(&mut g, 1);
    let recipient = object(&g, 1);
    let targets = target(&g, 1);
    announce(&mut g, 1, 0, targets);
    let mut reached = false;
    for _ in 0..2000 {
        let advance = g.advance(Budget::new(1));
        if !g.state.rules.effects.is_empty() {
            reached = true;
            break;
        }
        match advance {
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(r) => {
                g.respond(r.player(), r.decision_id(), pass(&r)).unwrap();
            }
            x => panic!("bounded paid compound creation {x:?}"),
        }
    }
    assert!(reached, "paid effect actually began");
    stats(&g, 1, 5, 3);
    recover(&g);
    g.concede(PlayerId(2)).unwrap();
    request(&mut g, 1);
    assert!(g.object_id(CardId(1)).is_none());
    let before = g
        .trace
        .records
        .iter()
        .filter_map(|r| match &r.record {
            EvidenceRecord::Batch { batch } => Some(batch.as_ref()),
            _ => None,
        })
        .flat_map(|batch| &batch.occurrences)
        .find(|occurrence| {
            matches!(
                &occurrence.proposed,
                ProposedEvent::LeaveGame { object, departing_owner }
                    if *object == recipient && *departing_owner == PlayerId(2)
            )
        })
        .unwrap()
        .before
        .objects
        .iter()
        .find(|o| o.object == recipient)
        .unwrap();
    assert_eq!(before.characteristics.power, Some(5.into()));
    assert_eq!(before.characteristics.toughness, Some(3.into()));
    assert!(
        before.keywords.contains(&KeywordAbility::Lifelink),
        "one continuous effect must be complete in actual owner-departure LKI"
    );
    recover(&g);
}
