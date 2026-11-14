//! Paid complete-source set-grant witnesses; no pool/profile/consumer admission.
//! Archive d705222b280a71cb; CR20260925 109.5, 302.6, 400.7, 506.4,
//! 608.2c/h, 611.2a/c, 613.1f/4c/5/6, 702.7/10/12/15/19, 704.5g/h, 800.4:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
//! Native/schema execution is UNRUN by this child; parent owns qualification.
use super::equipped_indestructible_tests::{answer, both, object, recover, request};
use super::temporary_indestructible_tests::{pass, target, through_cleanup, until_player};
use super::*;

type Row = (&'static str, u8, u8, bool);
struct Witness {
    name: &'static str,
    raw: &'static str,
    lands: &'static [&'static str],
    modifier: Option<(i64, i64)>,
    keywords: &'static [KeywordAbility],
}
const SOURCES: &[Witness] = &[
    Witness {
        name: "Titanic Ultimatum",
        raw: include_str!("../../../../tests/fixtures/0cd28580-d718-4d0b-96e3-a836c2911c27.json"),
        lands: &[
            "Mountain", "Mountain", "Forest", "Forest", "Forest", "Plains", "Plains",
        ],
        modifier: Some((5, 5)),
        keywords: &[
            KeywordAbility::FirstStrike,
            KeywordAbility::Trample,
            KeywordAbility::Lifelink,
        ],
    },
    Witness {
        name: "Overrun",
        raw: include_str!("../../../../tests/fixtures/204f9afe-c20b-4933-b5cd-aa572784762a.json"),
        lands: &["Forest"; 5],
        modifier: Some((3, 3)),
        keywords: &[KeywordAbility::Trample],
    },
    Witness {
        name: "Stampede",
        raw: include_str!("../../../../tests/fixtures/49c997d5-7b41-4faf-b898-58db19055671.json"),
        lands: &["Forest"; 3],
        modifier: Some((1, 0)),
        keywords: &[KeywordAbility::Trample],
    },
    Witness {
        name: "Make a Stand",
        raw: include_str!("../../../../tests/fixtures/531f78d5-5004-4b02-99c7-b390cb342fd9.json"),
        lands: &["Plains"; 3],
        modifier: Some((1, 0)),
        keywords: &[KeywordAbility::Indestructible],
    },
    Witness {
        name: "Burst of Speed",
        raw: include_str!("../../../../tests/fixtures/91e33674-b13a-41de-8998-4b57016ae42f.json"),
        lands: &["Mountain"],
        modifier: None,
        keywords: &[KeywordAbility::Haste],
    },
];
const EXTRAS: &[(&str, &str)] = &[
    (
        "Control Magic",
        include_str!("../../../../tests/fixtures/cd0d7141-46d2-4aa3-bc77-6b3b4513803e.json"),
    ),
    (
        "Raise the Alarm",
        include_str!("../../../../tests/fixtures/raise-the-alarm.json"),
    ),
    (
        "Prodigal Sorcerer",
        include_str!("../../../../tests/fixtures/prodigal-sorcerer.json"),
    ),
];
fn prepared(rows: &[Row]) -> Prepared {
    let placeholders: Vec<_> = rows
        .iter()
        .map(|(name, _, _, hand)| {
            (
                if SOURCES.iter().any(|s| s.name == *name) || EXTRAS.iter().any(|(n, _)| n == name)
                {
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
    for raw in SOURCES
        .iter()
        .map(|s| s.raw)
        .chain(EXTRAS.iter().map(|(_, raw)| *raw))
    {
        let mtg_compiler::CompileResult::Lowered { definition } = mtg_compiler::compile(
            include_str!("../../../../tests/fixtures/archive-manifest-current.json"),
            raw,
        ) else {
            panic!("complete frozen source must lower")
        };
        if !c
            .definitions
            .iter()
            .any(|d| d.oracle_id == definition.oracle_id)
        {
            c.definitions.push(*definition);
        }
    }
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    for (i, (name, owner, controller, hand)) in rows.iter().enumerate() {
        let card = &mut p.config.cards[i];
        card.oracle_id = c
            .definitions
            .iter()
            .find(|d| d.name == *name)
            .unwrap()
            .oracle_id
            .clone();
        card.owner = PlayerId(*owner);
        card.controller = PlayerId(*controller);
        card.zone = if *hand {
            Zone::Hand { owner: card.owner }
        } else {
            Zone::Battlefield
        };
    }
    p.catalog_digest = digest(&c).unwrap();
    p.catalog = Arc::new(c);
    p.profile_digest = digest(&p.config).unwrap();
    p
}
fn lands(rows: &mut Vec<Row>, name: &'static str, seat: u8, count: usize) {
    rows.extend(vec![(name, seat, seat, false); count]);
}
fn witness_rows(w: &Witness, creatures: bool) -> Vec<Row> {
    let mut rows = vec![(w.name, 0, 0, true)];
    if creatures {
        rows.extend([
            ("Grizzly Bears", 2, 0, false),
            ("Grizzly Bears", 0, 1, false),
            ("Grizzly Bears", 0, 0, false),
        ]);
    }
    for land in w.lands {
        lands(&mut rows, land, 0, 1);
    }
    rows
}
fn stats(g: &Game, id: u32, p: i64, t: i64) {
    let c = g.derived_characteristics(object(g, id)).unwrap();
    assert_eq!(c.power, Some(p.into()));
    assert_eq!(c.toughness, Some(t.into()));
}
fn grave(g: &Game, id: u32) -> bool {
    matches!(
        g.current(object(g, id)).unwrap().zone,
        Zone::Graveyard { .. }
    )
}
fn seek(g: &mut Game, q: u64, predicate: impl Fn(&Game, &Request) -> bool) -> Request {
    for _ in 0..1000 {
        let r = request(g, q);
        if predicate(g, &r) {
            return r;
        }
        answer(g, q, pass(&r));
    }
    panic!("bounded rules boundary")
}
fn own_main(g: &mut Game, q: u64, seat: u8) {
    seek(g, q, |g, r| {
        g.active_player() == Some(PlayerId(seat))
            && g.turn().step == TurnStep::PrecombatMain
            && matches!(r,Request::Priority {player,..} if *player == PlayerId(seat))
    });
}
// Only named basic land witnesses select payment sources; spell mechanics never
// dispatch on a card name. Prefunding exact colored requirements prevents a
// generic land walk from starving a later colored symbol.
fn announce_paid(g: &mut Game, q: u64, id: u32, targets: Vec<TargetEntity>) {
    let cost = g
        .definition(CardId(id))
        .unwrap()
        .characteristics
        .mana_cost
        .clone();
    for color in &cost.colored {
        let name = match color {
            Color::W => "Plains",
            Color::U => "Island",
            Color::B => "Swamp",
            Color::R => "Mountain",
            Color::G => "Forest",
            _ => panic!("reviewed basic color"),
        };
        let r = request(g, q);
        let source = g
            .state
            .rules
            .objects
            .values()
            .find(|o| {
                o.controller == r.player()
                    && o.zone == Zone::Battlefield
                    && !o.tapped
                    && g.definition(o.id.card).is_ok_and(|d| d.name == name)
            })
            .unwrap()
            .id;
        answer(g, q, Response::ActivateMana { object: source });
    }
    // The reusable announcer exercises the normal casting mana window, exact
    // SpendMana and target lifecycle. For new set spells prove no target window.
    if targets.is_empty() {
        answer(g, q, Response::Cast { card: CardId(id) });
        let r = request(g, q);
        assert!(matches!(r, Request::Mana { .. }));
        let before = g.state.clone();
        let trace = g.trace.clone();
        for response in [
            Response::Targets { objects: vec![] },
            Response::EntityTargets { entities: vec![] },
        ] {
            assert!(g.respond(r.player(), r.decision_id(), response).is_err());
            assert_eq!(g.state, before);
            assert_eq!(g.trace, trace);
        }
        pay_announcement(g, q);
    } else {
        temporary_indestructible_tests::announce(g, q, id, targets);
    }
    assert!(g.state.rules.stack.last().unwrap().paid.is_some());
}
fn pay_announcement(g: &mut Game, q: u64) {
    loop {
        let r = request(g, q);
        let response = match &r {
            Request::Mana { cost, .. } => {
                if g.payment_pool(r.player(), cost).is_ok() {
                    Response::FinishMana { undo_mana: false }
                } else {
                    let source = g
                        .state
                        .rules
                        .objects
                        .values()
                        .find(|o| {
                            o.controller == r.player()
                                && o.zone == Zone::Battlefield
                                && !o.tapped
                                && g.definition(o.id.card)
                                    .is_ok_and(|d| d.characteristics.kind == CardKind::Land)
                        })
                        .unwrap()
                        .id;
                    Response::ActivateMana { object: source }
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
            x => panic!("nontargeted set announcement {x:?}"),
        };
        g.respond(r.player(), r.decision_id(), response).unwrap();
    }
}
fn resolve(g: &mut Game, q: u64, mut inspect: impl FnMut(&Game)) {
    for _ in 0..3000 {
        let a = g.advance(Budget::new(q));
        inspect(g);
        match a {
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(r) => {
                if g.state.rules.stack.is_empty() {
                    return;
                }
                g.respond(r.player(), r.decision_id(), pass(&r)).unwrap();
            }
            x => panic!("paid resolution {x:?}"),
        }
    }
    panic!("bounded resolution")
}
fn cast(g: &mut Game, q: u64, id: u32, t: Vec<TargetEntity>) {
    announce_paid(g, q, id, t);
    resolve(g, q, |_| {});
}
fn grant(g: &Game) -> (&ResolvedEffect, &[ObjectId], &[KeywordAbility]) {
    let e = g.state.rules.effects.last().unwrap();
    let ResolvedEffectKind::GrantKeywordsSet {
        targets, keywords, ..
    } = &e.kind
    else {
        panic!("set grant")
    };
    (e, targets, keywords)
}
fn assert_grant(g: &Game, w: &Witness, ids: &[u32]) {
    let expected: Vec<_> = ids.iter().map(|id| object(g, *id)).collect();
    let (e, targets, keywords) = grant(g);
    assert_eq!(targets, expected);
    assert_eq!(keywords, w.keywords);
    assert_eq!(e.origin.source_block, 0);
    assert_eq!(e.origin.cursor, 0);
    assert_eq!(g.state.rules.effects.len(), 1);
    let expected_modifier = w.modifier.map(|(power, toughness)| CreatureSetModifier {
        power: power.into(),
        toughness: toughness.into(),
    });
    assert!(
        matches!(&e.kind, ResolvedEffectKind::GrantKeywordsSet { modifier, .. }
        if modifier == &expected_modifier)
    );
    for id in ids {
        let (p, t) = w.modifier.unwrap_or((0, 0));
        stats(g, *id, 2 + p, 2 + t);
        for k in w.keywords {
            assert_eq!(
                g.current_keywords(object(g, *id))
                    .iter()
                    .filter(|x| *x == k)
                    .count(),
                1
            );
        }
    }
}
fn declare(g: &mut Game, q: u64, attacker: u32, blockers: &[u32]) {
    let r = seek(g, q, |_, r| matches!(r, Request::Attackers { .. }));
    assert!(
        matches!(&r,Request::Attackers {candidates,..} if candidates.contains(&object(g,attacker)))
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
fn finish_query(g: &Game, mut r: QueryResult) -> QueryValue {
    for _ in 0..10000 {
        r = match r {
            QueryResult::Complete(v) => return v,
            QueryResult::Incomplete { work, .. } => g.resume_work(work, Budget::new(1)),
            _ => panic!("query rejected or did not complete"),
        };
    }
    panic!("bounded query")
}

#[test]
fn creature_set_keyword_paid_five_sources_exact_costs_scope_order_and_cleanup() {
    for w in SOURCES {
        both(prepared(&witness_rows(w, true)), |g, q| {
            let ids = if w.name == "Stampede" {
                declare(g, q, 1, &[]);
                vec![1]
            } else {
                vec![1, 3]
            };
            until_player(g, q, PlayerId(0));
            let tapped = g.state.rules.objects.values().filter(|o| o.tapped).count();
            announce_paid(g, q, 0, vec![]);
            let paid = g.state.rules.stack.last().unwrap().paid.as_ref().unwrap();
            assert_eq!(
                paid,
                &g.definition(CardId(0)).unwrap().characteristics.mana_cost
            );
            assert_eq!(
                g.state.rules.objects.values().filter(|o| o.tapped).count() - tapped,
                w.lands.len()
            );
            resolve(g, q, |_| {});
            assert_grant(g, w, &ids);
            assert!(grave(g, 0));
            assert!(g.current_keywords(object(g, 2)).is_empty());
            stats(g, 2, 2, 2);
            if w.name == "Stampede" {
                assert!(g.current_keywords(object(g, 3)).is_empty());
                stats(g, 3, 2, 2);
            }
            recover(g);
            through_cleanup(g, q);
            assert!(g.state.rules.effects.is_empty());
            for id in [1, 2, 3] {
                stats(g, id, 2, 2);
                assert!(g.current_keywords(object(g, id)).is_empty());
            }
        });
    }
}

#[test]
fn creature_set_keyword_empty_sets_resolve_without_target_or_fizzle() {
    for w in SOURCES {
        both(prepared(&witness_rows(w, false)), |g, q| {
            announce_paid(g, q, 0, vec![]);
            let stack = g.state.rules.stack.last().unwrap().id;
            resolve(g, q, |_| {});
            assert_grant(g, w, &[]);
            assert!(grave(g, 0));
            if g.trace.mode == TraceMode::Detailed {
                assert!(g.events().iter().any(|e|matches!(e,Event::SpellCompleted {stack_id,resolved:true} if *stack_id==stack)));
            }
        });
    }
}

#[test]
fn creature_set_keyword_capture_excludes_later_creatures_tokens_and_reentry() {
    let mut rows = vec![
        ("Burst of Speed", 0, 0, true),
        ("Grizzly Bears", 0, 0, false),
        ("Grizzly Bears", 0, 0, true),
        ("Raise the Alarm", 0, 0, true),
        ("Unsummon", 0, 0, true),
    ];
    lands(&mut rows, "Forest", 0, 4);
    lands(&mut rows, "Plains", 0, 2);
    lands(&mut rows, "Island", 0, 1);
    lands(&mut rows, "Mountain", 0, 1);
    both(prepared(&rows), |g, q| {
        let old = object(g, 1);
        cast(g, q, 0, vec![]);
        assert_eq!(grant(g).1, &[old]);
        cast(g, q, 2, vec![]);
        cast(g, q, 3, vec![]);
        assert!(!g.has_keyword(object(g, 2), KeywordAbility::Haste));
        let tokens: Vec<_> = g
            .state
            .rules
            .objects
            .values()
            .filter(|o| g.state.rules.tokens.contains_key(&o.id.card))
            .map(|o| o.id)
            .collect();
        assert_eq!(tokens.len(), 2);
        for token in tokens {
            assert!(!g.has_keyword(token, KeywordAbility::Haste));
        }
        cast(g, q, 4, target(g, 1));
        cast(g, q, 1, vec![]);
        assert_ne!(object(g, 1), old);
        assert!(!g.has_keyword(object(g, 1), KeywordAbility::Haste));
        assert_eq!(grant(g).1, &[old]);
        for viewer in 0..4 {
            assert!(
                matches!(g.visible_effects(PlayerId(viewer)).unwrap().last().unwrap(),VisibleEffect::GrantKeywordsSet {targets,..} if targets.len()==1 && targets[0].current.is_none())
            );
        }
    });
}

#[test]
fn creature_set_keyword_authentic_control_changes_preserve_only_captured_incarnations() {
    let mut rows = vec![
        ("Make a Stand", 0, 0, true),
        ("Grizzly Bears", 2, 0, false),
        ("Grizzly Bears", 0, 0, false),
        ("Control Magic", 1, 1, true),
        ("Control Magic", 1, 1, true),
        ("Disperse", 1, 1, true),
        ("Disperse", 1, 1, true),
    ];
    lands(&mut rows, "Plains", 0, 3);
    lands(&mut rows, "Island", 1, 12);
    both(prepared(&rows), |g, q| {
        own_main(g, q, 1);
        cast(g, q, 3, target(g, 2));
        until_player(g, q, PlayerId(0));
        cast(g, q, 0, vec![]);
        assert_eq!(grant(g).1, &[object(g, 1)]);
        until_player(g, q, PlayerId(1));
        cast(g, q, 4, target(g, 1));
        assert_eq!(g.current(object(g, 1)).unwrap().controller, PlayerId(1));
        assert!(g.has_keyword(object(g, 1), KeywordAbility::Indestructible));
        stats(g, 1, 3, 2);
        cast(g, q, 5, target(g, 3));
        assert_eq!(g.current(object(g, 2)).unwrap().controller, PlayerId(0));
        assert!(!g.has_keyword(object(g, 2), KeywordAbility::Indestructible));
        stats(g, 2, 2, 2);
        cast(g, q, 6, target(g, 4));
        assert_eq!(g.current(object(g, 1)).unwrap().controller, PlayerId(0));
        assert!(g.has_keyword(object(g, 1), KeywordAbility::Indestructible));
        recover(g);
        through_cleanup(g, q);
        assert!(!g.has_keyword(object(g, 1), KeywordAbility::Indestructible));
    });
}

#[test]
fn creature_set_keyword_overrun_titanic_real_trample_first_strike_and_lifelink() {
    for w in &SOURCES[..2] {
        let mut rows = witness_rows(w, true);
        rows[2] = ("Grizzly Bears", 1, 1, false);
        both(prepared(&rows), |g, q| {
            cast(g, q, 0, vec![]);
            declare(g, q, 1, &[2]);
            let r = seek(g, q, |_, r| matches!(r, Request::Damage { .. }));
            let victim = object(g, 2);
            assert!(
                matches!(&r,Request::Damage {player_recipient:Some(PlayerId(1)),lethal_requirements,..} if lethal_requirements==&vec![(victim,2.into())])
            );
            let power = 2 + w.modifier.unwrap().0;
            let excess = power - 2;
            let before = g.snapshot().unwrap();
            assert!(
                g.respond(
                    r.player(),
                    r.decision_id(),
                    Response::TrampleDamage {
                        assignments: vec![(victim, 1.into())],
                        player: PlayerId(1),
                        amount: (power - 1).into()
                    }
                )
                .is_err()
            );
            assert_eq!(g.snapshot().unwrap(), before);
            answer(
                g,
                q,
                Response::TrampleDamage {
                    assignments: vec![(victim, 2.into())],
                    player: PlayerId(1),
                    amount: excess.into(),
                },
            );
            let mut captured_damage = false;
            for _ in 0..2000 {
                if let Frame::DamageEffects { transaction } = &g.state.rules.frame {
                    if transaction
                        .entries
                        .iter()
                        .any(|d| d.source.object == object(g, 1))
                    {
                        g.validate_damage_transaction(transaction).unwrap();
                        let source = &transaction
                            .entries
                            .iter()
                            .find(|d| d.source.object == object(g, 1))
                            .unwrap()
                            .source;
                        for keyword in w.keywords {
                            assert!(source.keywords.contains(keyword));
                        }
                        if !captured_damage {
                            recover(g);
                        }
                        captured_damage = true;
                    }
                }
                match g.advance(Budget::new(q)) {
                    Advance::Suspended { .. } => (),
                    Advance::AwaitingDecision(r) => {
                        if g.turn().step == TurnStep::EndCombat {
                            break;
                        }
                        g.respond(r.player(), r.decision_id(), pass(&r)).unwrap();
                    }
                    x => panic!("set-keyword combat damage {x:?}"),
                }
            }
            assert_eq!(g.turn().step, TurnStep::EndCombat);
            if q == 1 {
                assert!(captured_damage, "actual captured damage/LKI frame");
            }
            assert!(grave(g, 2));
            assert_eq!(g.state.rules.players[1].life, (40 - excess).into());
            assert_eq!(
                g.state.rules.players[0].life,
                (if w.name == "Titanic Ultimatum" {
                    40 + power
                } else {
                    40
                })
                .into()
            );
            assert_eq!(
                g.current(object(g, 1)).unwrap().marked_damage,
                (if w.name == "Titanic Ultimatum" { 0 } else { 2 }).into()
            );
        });
    }
}

#[test]
fn creature_set_keyword_burst_haste_actual_attack_and_tap_excludes_postgrant_entrant() {
    for attack in [false, true] {
        let mut rows = vec![
            ("Burst of Speed", 0, 0, true),
            ("Prodigal Sorcerer", 0, 0, true),
            ("Prodigal Sorcerer", 0, 0, true),
            ("Yoked Ox", 0, 0, false),
        ];
        lands(&mut rows, "Island", 0, 6);
        lands(&mut rows, "Mountain", 0, 1);
        both(prepared(&rows), |g, q| {
            cast(g, q, 1, vec![]);
            assert!(!g.current(object(g, 1)).unwrap().attack_ready);
            let r = request(g, q);
            let before = g.snapshot().unwrap();
            assert!(
                g.respond(
                    r.player(),
                    r.decision_id(),
                    Response::ActivateAbility {
                        object: object(g, 1),
                        ability: 0
                    }
                )
                .is_err()
            );
            assert_eq!(g.snapshot().unwrap(), before);
            cast(g, q, 0, vec![]);
            cast(g, q, 2, vec![]);
            assert!(g.creatures(PlayerId(0), true).contains(&object(g, 1)));
            assert!(!g.creatures(PlayerId(0), true).contains(&object(g, 2)));
            if attack {
                declare(g, q, 1, &[]);
                seek(g, q, |g, _| g.turn().step == TurnStep::EndCombat);
                assert_eq!(g.state.rules.players[1].life, 39.into());
            } else {
                answer(
                    g,
                    q,
                    Response::ActivateAbility {
                        object: object(g, 1),
                        ability: 0,
                    },
                );
                answer(
                    g,
                    q,
                    Response::EntityTargets {
                        entities: target(g, 3),
                    },
                );
                resolve(g, q, |_| {});
                assert!(g.current(object(g, 1)).unwrap().tapped);
                assert_eq!(g.current(object(g, 3)).unwrap().marked_damage, 1.into());
            }
            through_cleanup(g, q);
            assert!(!g.has_keyword(object(g, 1), KeywordAbility::Haste));
        });
    }
}

#[test]
fn creature_set_keyword_make_stand_mass_destruction_cleanup_then_fresh_destroy() {
    let mut rows = vec![
        ("Make a Stand", 0, 0, true),
        ("Grizzly Bears", 0, 0, false),
        ("Grizzly Bears", 2, 0, false),
        ("Grizzly Bears", 1, 1, false),
        ("Day of Judgment", 0, 0, true),
        ("Lightning Bolt", 0, 0, true),
        ("Doom Blade", 0, 0, true),
    ];
    lands(&mut rows, "Plains", 0, 7);
    lands(&mut rows, "Mountain", 0, 1);
    lands(&mut rows, "Swamp", 0, 2);
    both(prepared(&rows), |g, q| {
        cast(g, q, 0, vec![]);
        cast(g, q, 5, target(g, 1));
        assert_eq!(g.current(object(g, 1)).unwrap().marked_damage, 3.into());
        cast(g, q, 4, vec![]);
        assert!(grave(g, 3));
        for id in [1, 2] {
            assert_eq!(g.current(object(g, id)).unwrap().zone, Zone::Battlefield);
        }
        recover(g);
        through_cleanup(g, q);
        assert_eq!(g.current(object(g, 1)).unwrap().marked_damage, 0.into());
        assert!(g.state.rules.effects.is_empty());
        until_player(g, q, PlayerId(0));
        cast(g, q, 6, target(g, 1));
        assert!(grave(g, 1));
        assert_eq!(g.current(object(g, 2)).unwrap().zone, Zone::Battlefield);
    });
}

#[test]
fn creature_set_keyword_stampede_other_controller_attackers_frozen_after_combat() {
    let mut rows = vec![
        ("Stampede", 2, 2, true),
        ("Grizzly Bears", 3, 1, false),
        ("Grizzly Bears", 2, 2, false),
        ("Control Magic", 0, 0, true),
        ("Disperse", 0, 0, true),
    ];
    lands(&mut rows, "Forest", 2, 3);
    lands(&mut rows, "Island", 0, 6);
    both(prepared(&rows), |g, q| {
        cast(g, q, 3, target(g, 1));
        let turn = g.turn().number;
        seek(g, q, |g, r| {
            g.turn().number > turn
                && g.active_player() == Some(PlayerId(0))
                && g.turn().step == TurnStep::PrecombatMain
                && matches!(
                    r,
                    Request::Priority {
                        player: PlayerId(0),
                        ..
                    }
                )
        });
        declare(g, q, 1, &[]);
        until_player(g, q, PlayerId(2));
        cast(g, q, 0, vec![]);
        assert_grant(g, &SOURCES[2], &[1]);
        assert!(!g.has_keyword(object(g, 2), KeywordAbility::Trample));
        until_player(g, q, PlayerId(0));
        cast(g, q, 4, target(g, 3));
        assert_eq!(g.current(object(g, 1)).unwrap().controller, PlayerId(1));
        assert!(g.state.rules.combat.is_empty());
        assert!(g.has_keyword(object(g, 1), KeywordAbility::Trample));
        stats(g, 1, 3, 2);
        seek(g, q, |g, _| g.turn().step == TurnStep::PostcombatMain);
        assert!(g.has_keyword(object(g, 1), KeywordAbility::Trample));
        recover(g);
        through_cleanup(g, q);
        assert!(!g.has_keyword(object(g, 1), KeywordAbility::Trample));
    });
}

#[test]
fn creature_set_keyword_queries_atomic_target_rejection_forgery_and_complete_creation() {
    for w in SOURCES {
        both(prepared(&witness_rows(w, true)), |g, q| {
            let before = g.state.clone();
            let trace = g.trace.clone();
            let QueryResult::Incomplete { work: stale, .. } =
                g.observe(PlayerId(0), Budget::new(0))
            else {
                panic!("query work")
            };
            for viewer in 0..4 {
                let QueryValue::Observation(o) =
                    finish_query(g, g.observe(PlayerId(viewer), Budget::new(0)))
                else {
                    panic!("observation")
                };
                assert!(
                    !o.view
                        .objects
                        .iter()
                        .any(|o| matches!(o.zone, Zone::Library { .. }))
                );
            }
            assert_eq!(g.state, before);
            assert_eq!(g.trace, trace);
            announce_paid(g, q, 0, vec![]);
            assert!(matches!(
                g.resume_work(stale, Budget::unlimited()),
                QueryResult::Rejected(QueryRejection::StaleOrForeignWork)
            ));
            let r = request(g, q);
            let before = g.snapshot().unwrap();
            for response in [
                Response::Targets {
                    objects: vec![object(g, 1)],
                },
                Response::EntityTargets {
                    entities: target(g, 1),
                },
            ] {
                assert!(g.respond(r.player(), r.decision_id(), response).is_err());
                assert_eq!(g.snapshot().unwrap(), before);
            }
            let ids = if w.name == "Stampede" {
                vec![]
            } else {
                vec![1, 3]
            };
            let mut observed_complete = false;
            resolve(g, q, |g| {
                if g.state.rules.effects.is_empty() {
                    for id in [1, 3] {
                        stats(g, id, 2, 2);
                        assert!(!g.has_keyword(object(g, id), w.keywords[0]));
                    }
                } else {
                    // One effect makes all characteristic layers visible together.
                    // There is no resumable PT-only program prefix (CR 613.5-6).
                    assert_grant(g, w, &ids);
                    if !observed_complete {
                        recover(g);
                    }
                    observed_complete = true;
                }
            });
            assert!(observed_complete);
            assert_grant(g, w, &ids);
            let before = g.state.clone();
            let trace = g.trace.clone();
            for viewer in 0..4 {
                let effects = g.visible_effects(PlayerId(viewer)).unwrap();
                let VisibleEffect::GrantKeywordsSet {
                    handle,
                    origin,
                    targets,
                    keywords,
                    modifier,
                    expires_in_cleanup_of_turn,
                } = effects.last().unwrap()
                else {
                    panic!("visible set grant")
                };
                let (effect, captured, _) = grant(g);
                assert_eq!(
                    handle,
                    &g.effect_handle(PlayerId(viewer), &EffectKey::Resolved { id: effect.id })
                        .unwrap()
                );
                assert_eq!(origin.controller, effect.origin.controller);
                assert_eq!(
                    origin.oracle_id,
                    g.definition(effect.origin.source.card).unwrap().oracle_id
                );
                assert_eq!(
                    origin.source.announced,
                    effect.origin.source_handles[viewer as usize]
                );
                assert_eq!(
                    origin.source.current,
                    g.handle_for(PlayerId(viewer), effect.origin.source).ok()
                );
                assert_eq!(
                    *expires_in_cleanup_of_turn,
                    effect.expires_in_cleanup_of_turn
                );
                assert_eq!(targets.len(), ids.len());
                assert_eq!(keywords, w.keywords);
                assert_eq!(
                    *modifier,
                    w.modifier.map(|(power, toughness)| CreatureSetModifier {
                        power: power.into(),
                        toughness: toughness.into(),
                    })
                );
                for (i, recipient) in targets.iter().enumerate() {
                    assert_eq!(
                        recipient.announced,
                        effect.target_handles[i * 4 + viewer as usize]
                    );
                    assert_eq!(
                        recipient.current,
                        g.handle_for(PlayerId(viewer), captured[i]).ok()
                    );
                }
                finish_query(g, g.observe(PlayerId(viewer), Budget::new(0)));
            }
            assert_eq!(g.state, before);
            assert_eq!(g.trace, trace);
            recover(g);
            let good = g.snapshot().unwrap();
            // Coherently change the single capture and all four historical recipient
            // handles. This is structurally valid and needs authenticated input
            // replay, rather than local membership or state-hash checks, to fail.
            let forged_targets = vec![object(g, 2)];
            let forged_handles: Vec<_> = (0..4)
                .map(|v| g.handle_for(PlayerId(v), forged_targets[0]).unwrap())
                .collect();
            let mut coherent: Snapshot = strict_read(&good).unwrap();
            for effect in &mut coherent.state.rules.effects {
                match &mut effect.kind {
                    ResolvedEffectKind::GrantKeywordsSet { targets, .. } => {
                        *targets = forged_targets.clone()
                    }
                    _ => unreachable!(),
                }
                effect.target_handles = forged_handles.clone();
            }
            let mut structural = g.fork();
            structural.state = coherent.state.clone();
            structural.validate_effect_state().unwrap();
            coherent.state_sha256 = digest(&coherent.state).unwrap();
            assert!(
                Game::restore(g.prepared.as_ref().clone(), &canonical(&coherent).unwrap()).is_err(),
                "coherent forged captured set"
            );
            for variant in 0..15 {
                let mut bad: Snapshot = strict_read(&good).unwrap();
                let e = bad.state.rules.effects.last_mut().unwrap();
                let ResolvedEffectKind::GrantKeywordsSet {
                    targets,
                    keywords,
                    modifier,
                } = &mut e.kind
                else {
                    unreachable!()
                };
                match variant {
                    0 => targets.clear(),
                    1 => targets.push(object(g, 2)),
                    2 => targets.reverse(),
                    3 => {
                        if let Some(t) = targets.first_mut() {
                            t.incarnation += 1;
                        } else {
                            targets.push(object(g, 1));
                        }
                    }
                    4 => keywords.clear(),
                    5 => keywords.push(keywords[0]),
                    6 => *keywords = vec![KeywordAbility::Flying],
                    7 => e.origin.source_block += 1,
                    8 => e.origin.controller = PlayerId(1),
                    9 => e.expires_in_cleanup_of_turn += 1,
                    10 => e.id.0 += 1,
                    11 => e.origin.source.incarnation += 1,
                    12 => e.origin.cursor += 1,
                    13 => {
                        if let Some(m) = modifier {
                            m.power.add(&1.into());
                        } else {
                            *modifier = Some(CreatureSetModifier {
                                power: 1.into(),
                                toughness: 0.into(),
                            });
                        }
                    }
                    _ => {
                        *modifier = if modifier.is_some() {
                            None
                        } else {
                            Some(CreatureSetModifier {
                                power: 0.into(),
                                toughness: 0.into(),
                            })
                        };
                    }
                }
                if bad.state == g.state {
                    continue;
                } // Empty-set order/removal are identities.
                bad.state_sha256 = digest(&bad.state).unwrap();
                assert!(
                    Game::restore(g.prepared.as_ref().clone(), &canonical(&bad).unwrap()).is_err(),
                    "{} forgery {variant}",
                    w.name
                );
            }
        });
    }
}

#[test]
fn creature_set_keyword_source_owner_departure_before_or_after_atomic_creation() {
    for before_creation in [false, true] {
        let mut rows = vec![
            ("Make a Stand", 0, 0, true),
            ("Grizzly Bears", 2, 1, false),
            ("Control Magic", 0, 0, true),
        ];
        lands(&mut rows, "Plains", 0, 3);
        lands(&mut rows, "Island", 0, 4);
        both(prepared(&rows), |g, q| {
            cast(g, q, 2, target(g, 1));
            let captured = object(g, 1);
            announce_paid(g, q, 0, vec![]);
            if !before_creation {
                // Reach the identical complete-creation boundary in both budgets.
                let mut reached = false;
                for _ in 0..1000 {
                    let a = g.advance(Budget::new(1));
                    if !g.state.rules.effects.is_empty() {
                        assert_grant(g, &SOURCES[3], &[1]);
                        recover(g);
                        reached = true;
                        break;
                    }
                    stats(g, 1, 2, 2);
                    assert!(!g.has_keyword(captured, KeywordAbility::Indestructible));
                    match a {
                        Advance::Suspended { .. } => (),
                        Advance::AwaitingDecision(r) => {
                            g.respond(r.player(), r.decision_id(), pass(&r)).unwrap();
                        }
                        x => panic!("atomic creation boundary {x:?}"),
                    }
                }
                assert!(reached);
            }
            g.concede(PlayerId(0)).unwrap();
            assert!(matches!(
                g.observe(PlayerId(1), Budget::unlimited()),
                QueryResult::Rejected(_)
            ));
            request(g, q);
            assert!(!g.is_live(PlayerId(0)));
            assert!(g.object_id(CardId(0)).is_none());
            assert!(g.object_id(CardId(2)).is_none());
            assert_eq!(g.current(captured).unwrap().controller, PlayerId(1));
            if before_creation {
                // Departure removes the unstarted owned spell; neither component began.
                stats(g, 1, 2, 2);
                assert!(g.state.rules.effects.is_empty());
                assert!(!g.has_keyword(captured, KeywordAbility::Indestructible));
            } else {
                // CR 611.2c/613.6: the one effect keeps both components and its
                // original recipients after control returns to a surviving player.
                assert_grant(g, &SOURCES[3], &[1]);
                stats(g, 1, 3, 2);
                assert!(g.has_keyword(captured, KeywordAbility::Indestructible));
            }
            recover(g);
            let mut owner_left = g.fork();
            owner_left.concede(PlayerId(2)).unwrap();
            request(&mut owner_left, q);
            assert!(owner_left.object_id(CardId(1)).is_none());
            if !before_creation {
                for viewer in [1, 3] {
                    let effects = owner_left.visible_effects(PlayerId(viewer)).unwrap();
                    let VisibleEffect::GrantKeywordsSet { targets, .. } = effects.last().unwrap()
                    else {
                        panic!("departed recipient projection")
                    };
                    assert!(targets.iter().all(|t| t.current.is_none()));
                }
                if owner_left.trace.mode == TraceMode::Detailed {
                    let facts = owner_left
                        .trace
                        .records
                        .iter()
                        .filter_map(|r| match &r.record {
                            EvidenceRecord::Batch { batch } => Some(batch.as_ref()),
                            _ => None,
                        })
                        .flat_map(|b| &b.occurrences)
                        .find(|o| {
                            matches!(&o.proposed,
                        ProposedEvent::LeaveGame { object, departing_owner }
                        if *object == captured && *departing_owner == PlayerId(2))
                        })
                        .unwrap();
                    let before = facts
                        .before
                        .objects
                        .iter()
                        .find(|o| o.object == captured)
                        .unwrap();
                    assert_eq!(before.characteristics.power, Some(3.into()));
                    assert_eq!(before.characteristics.toughness, Some(2.into()));
                    assert!(before.keywords.contains(&KeywordAbility::Indestructible));
                    assert_eq!(before.controller, PlayerId(1));
                }
            }
            recover(&owner_left);
            through_cleanup(g, q);
            stats(g, 1, 2, 2);
            assert!(!g.has_keyword(captured, KeywordAbility::Indestructible));
        });
    }
}
