//! Static block and attack restrictions from exact frozen R5 source.
//! Pinned CR 508.1b-d, 509.1b, 613.11, 702.111b:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::evasion::{accepts, attack, checked, game, lowered, offered, refuses, rejected, until};
use mtg_contracts::kir::{BlockRestriction, Scope, StaticEffect, StaticRule};
use mtg_core::testkit::*;
use std::collections::BTreeMap;

macro_rules! record {
    ($id:literal) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/phase1-r5/",
            $id,
            ".json"
        ))
    };
}

const SPITFIRE_HANDLER: &str = record!("01990628-12b4-49c9-af3d-03120891efc4");
const HUNTED_GHOUL: &str = record!("12827076-450d-42b1-98ca-9f9279a539fc");
const NORWOOD_RIDERS: &str = record!("1816b87c-73ba-4bfc-a51e-c982381c067d");
const VOW_OF_DUTY: &str = record!("1e7bddbd-e448-4aea-9d18-b44bd45837c5");
const FAMILIAR_GROUND: &str = record!("3b92c4dc-2605-42f4-a374-c3be4de3279e");
const JUGGERNAUT: &str = record!("4ac9116f-36bc-4d71-b696-d6ee064e1d58");
const ALPHA_AUTHORITY: &str = record!("538fa2e0-d225-4232-b243-38868b695e26");
const SILHANA_LEDGEWALKER: &str = record!("5b15dcac-38dc-4957-bc01-40153ea07559");
const REVERENCE: &str = record!("60963292-2dee-4b1c-9c1c-c35cf8ea3496");
const STEEL_LEAF_CHAMPION: &str = record!("666af637-9282-4121-962a-048e528f9221");
const BLAZING_ARCHON: &str = record!("82a70e4c-01ed-4280-80e4-86cb2bf6e63d");
const RAMPAGING_CERATOPS: &str = record!("91d01119-9b3e-4629-8cf0-bbca2c19a7cc");
const ELVEN_RIDERS: &str = record!("9a6ff511-3c8f-446e-8cad-b8997a753958");
const SPIRE_TRACER: &str = record!("9b38ab81-7ee7-4d89-878b-3507e44836ca");
const ORCISH_VETERAN: &str = record!("a3b66180-ee08-4bef-90aa-7859dc77cb10");
const BOWER_PASSAGE: &str = record!("f91ef713-5289-4901-9291-0a5afe6f997b");
const SACRED_KNIGHT: &str = record!("fe8f5cc0-5849-437f-8d1f-cb191f2fc638");

const SOURCES: &[&str] = &[
    HUNTED_GHOUL,
    NORWOOD_RIDERS,
    FAMILIAR_GROUND,
    JUGGERNAUT,
    SILHANA_LEDGEWALKER,
    REVERENCE,
    STEEL_LEAF_CHAMPION,
    BLAZING_ARCHON,
    RAMPAGING_CERATOPS,
    SPIRE_TRACER,
    ORCISH_VETERAN,
    BOWER_PASSAGE,
    SACRED_KNIGHT,
];

use Zone::Battlefield as B;

fn rule(d: &Definition) -> &StaticRule {
    d.static_abilities
        .iter()
        .find_map(|a| match &a.effect {
            StaticEffect::Rule { rule, .. } => Some(rule),
            _ => None,
        })
        .unwrap_or_else(|| panic!("{} has a rule static", d.name))
}

/// CR 509.1b: blocker filters lower with the affected attacker's scope;
/// ambiguous postmodified noun alternatives and relative powers reject.
#[test]
fn block_restrictions_lower_from_printed_source() {
    for (raw, only) in [
        (SILHANA_LEDGEWALKER, true),
        (SPIRE_TRACER, true),
        (STEEL_LEAF_CHAMPION, false),
        (SACRED_KNIGHT, false),
    ] {
        let d = lowered(raw);
        let rules: Vec<_> = d
            .static_abilities
            .iter()
            .filter_map(|a| match &a.effect {
                StaticEffect::Rule {
                    rule:
                        StaticRule::Block {
                            affects: Scope::Source,
                            restriction: BlockRestriction::Blockers { only: o, .. },
                        },
                    condition: None,
                } => Some(*o),
                _ => None,
            })
            .collect();
        assert_eq!(rules, vec![only], "{}", d.name);
    }
    assert!(matches!(
        rule(&lowered(NORWOOD_RIDERS)),
        StaticRule::Block {
            affects: Scope::Source,
            restriction: BlockRestriction::AtMost { count: 1 }
        }
    ));
    assert!(matches!(
        rule(&lowered(RAMPAGING_CERATOPS)),
        StaticRule::Block {
            affects: Scope::Source,
            restriction: BlockRestriction::AtLeast { count: 3 }
        }
    ));
    assert!(matches!(
        rule(&lowered(ALPHA_AUTHORITY)),
        StaticRule::Block {
            affects: Scope::Attachment { .. },
            restriction: BlockRestriction::AtMost { count: 1 }
        }
    ));
    assert!(matches!(
        rule(&lowered(VOW_OF_DUTY)),
        StaticRule::AttackRestriction {
            affects: Scope::Attachment { .. },
            ..
        }
    ));
    // "Walls and/or creatures with flying" may attach "with flying" to the
    // last noun only; "greater than this creature's power" is relative.
    assert!(rejected(ELVEN_RIDERS).contains("postmodified_alternatives"));
    assert!(rejected(SPITFIRE_HANDLER).contains("properties"));
}

/// CR 509.1b: "except by creatures with flying", "with flying or reach",
/// "by creatures with power 2 or less" and "by black and/or red
/// creatures" read the blocker's current keywords, power and colors.
#[test]
fn blocker_filters_read_current_blockers() {
    let mut g = game(
        SOURCES,
        &[
            ("Silhana Ledgewalker", 0, B),
            ("Steel Leaf Champion", 0, B),
            ("Sacred Knight", 0, B),
            ("Spire Tracer", 0, B),
            ("Storm Crow", 1, B),
            ("Grizzly Bears", 1, B),
            ("Royal Assassin", 1, B),
            ("Colossal Dreadmaw", 1, B),
        ],
    );
    let d = attack(&mut g, &[0, 1, 2, 3]);
    let offered = offered(&g, &d);
    assert_eq!(offered[&4], vec![0, 2, 3]);
    assert_eq!(offered[&5], vec![2]);
    assert_eq!(offered[&6], Vec::<u32>::new());
    assert_eq!(offered[&7], vec![1, 2]);
    refuses(&mut g, &d, &[(5, 0)]);
    refuses(&mut g, &d, &[(5, 1)]);
    refuses(&mut g, &d, &[(6, 2)]);
    accepts(&mut g, &d, &[(4, 0), (5, 2), (7, 1)]);
}

/// CR 509.1b: "can't block Humans", "can't block white creatures with
/// power 2 or greater" and "Creatures with flying can't block creatures
/// you control" restrict the blocker.
#[test]
fn blocking_filters_restrict_the_blocker() {
    let mut g = game(
        SOURCES,
        &[
            ("Royal Assassin", 0, B),
            ("Grizzly Bears", 0, B),
            ("Isamaru, Hound of Konda", 0, B),
            ("Soul's Attendant", 0, B),
            ("Hunted Ghoul", 1, B),
            ("Orcish Veteran", 1, B),
            ("Bower Passage", 0, B),
            ("Storm Crow", 1, B),
        ],
    );
    let d = attack(&mut g, &[0, 1, 2, 3]);
    let offered = offered(&g, &d);
    assert_eq!(offered[&4], vec![1, 2]);
    assert_eq!(offered[&5], vec![0, 1, 3]);
    assert_eq!(offered[&7], Vec::<u32>::new());
    refuses(&mut g, &d, &[(4, 0)]);
    refuses(&mut g, &d, &[(5, 2)]);
    refuses(&mut g, &d, &[(7, 1)]);
    accepts(&mut g, &d, &[(4, 2), (5, 3)]);
}

/// CR 509.1b, 702.111b: "more than one" and "three or more" bound each
/// nonempty block, across the whole declaration; both together leave no
/// legal block.
#[test]
fn blocker_counts_bound_the_whole_declaration() {
    let mut g = game(
        SOURCES,
        &[
            ("Norwood Riders", 0, B),
            ("Rampaging Ceratops", 0, B),
            ("Grizzly Bears", 1, B),
            ("Grizzly Bears", 1, B),
            ("Grizzly Bears", 1, B),
        ],
    );
    let d = attack(&mut g, &[0, 1]);
    let Shape::Pick { rules, .. } = &d.shape else {
        panic!("blocks are picks")
    };
    let index = |id| {
        d.index(&Candidate::Object {
            object: Driver::object(&g, id),
        })
        .unwrap()
    };
    assert_eq!(
        rules,
        &vec![
            JointRule::PickedAtLeast {
                candidates: vec![index(1)],
                count: 3
            },
            JointRule::PickedAtMost {
                candidates: vec![index(0)],
                count: 1
            },
        ]
    );
    refuses(&mut g, &d, &[(2, 0), (3, 0)]);
    refuses(&mut g, &d, &[(2, 1), (3, 1)]);
    accepts(&mut g, &d, &[(2, 1), (3, 1), (4, 1)]);

    let mut g = game(
        SOURCES,
        &[
            ("Rampaging Ceratops", 0, B),
            ("Grizzly Bears", 0, B),
            ("Familiar Ground", 0, B),
            ("Grizzly Bears", 1, B),
            ("Grizzly Bears", 1, B),
            ("Grizzly Bears", 1, B),
        ],
    );
    let d = attack(&mut g, &[0, 1]);
    refuses(&mut g, &d, &[(3, 0)]);
    refuses(&mut g, &d, &[(3, 0), (4, 0), (5, 0)]);
    refuses(&mut g, &d, &[(3, 1), (4, 1)]);
    accepts(&mut g, &d, &[(3, 1)]);
}

/// Each attacker's offered defenders and slot minimum, by board id.
fn attack_offer(g: &Game, d: &Decision<Rules>) -> BTreeMap<u32, (Vec<u8>, u32)> {
    let Shape::Pick { slots, .. } = &d.shape else {
        panic!("attacks are picks")
    };
    slots
        .iter()
        .map(|s| {
            let Candidate::Object { object } = &d.candidates[s.subject.unwrap() as usize] else {
                panic!("attackers are objects")
            };
            let id = (0..16).find(|i| Driver::object(g, *i) == *object).unwrap();
            let players = s
                .allowed
                .iter()
                .map(|a| match &d.candidates[*a as usize] {
                    Candidate::Player { player } => player.0,
                    other => panic!("defender {other:?}"),
                })
                .collect();
            (id, (players, s.min))
        })
        .collect()
}

/// CR 508.1b-d: "Creatures with power 2 or less can't attack you" and
/// "Creatures can't attack you" remove defenders; a creature that attacks
/// each combat if able must attack only when a defender remains. The
/// witness board seats four players.
#[test]
fn attack_restrictions_remove_defenders_before_requirements() {
    let mut g = game(
        SOURCES,
        &[
            ("Reverence", 1, B),
            ("Grizzly Bears", 0, B),
            ("Colossal Dreadmaw", 0, B),
            ("Juggernaut", 0, B),
        ],
    );
    let d = until(&mut g, &Reason::Attackers);
    let offer = attack_offer(&g, &d);
    assert_eq!(offer[&1], (vec![2, 3], 0));
    assert_eq!(offer[&2], (vec![1, 2, 3], 0));
    assert_eq!(offer[&3], (vec![1, 2, 3], 1));
    for forged in [vec![(1, 1), (3, 2)], vec![(2, 1)]] {
        let before = g.snapshot().unwrap();
        let answer = d.attack(
            forged
                .iter()
                .map(|(id, p)| (Driver::object(&g, *id), PlayerId(*p)))
                .collect(),
        );
        assert!(g.respond(d.player, d.id.clone(), answer).is_err());
        assert_eq!(g.snapshot().unwrap(), before);
    }
    let answer = d.attack(vec![
        (Driver::object(&g, 1), PlayerId(2)),
        (Driver::object(&g, 3), PlayerId(1)),
    ]);
    g.respond(d.player, d.id, answer).unwrap();
    checked(&g);

    let mut g = game(
        SOURCES,
        &[
            ("Blazing Archon", 1, B),
            ("Blazing Archon", 2, B),
            ("Blazing Archon", 3, B),
            ("Juggernaut", 0, B),
        ],
    );
    let d = until(&mut g, &Reason::Attackers);
    assert_eq!(attack_offer(&g, &d)[&3], (vec![], 0));
    g.respond(d.player, d.id.clone(), d.attack(vec![])).unwrap();
    checked(&g);
}
