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
        assert!(g.respond(d.player, d.id, answer).is_err());
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
    g.respond(d.player, d.id, d.attack(vec![])).unwrap();
    checked(&g);
}

const MOBILE_FORT: &str = record!("0547e426-e23e-4c60-ba6b-cfd4b5ff8485");
const BRISTLEPACK_SENTRY: &str = record!("05852d99-8f30-4318-913c-d0b1eaf8c222");
const HECKLING_FIENDS: &str = record!("4b8fb8dc-f1c5-4994-8a58-4849e7e85539");
const ROLLING_STONES: &str = record!("7ce533d0-8b6e-46d8-b21e-a2932bdffb67");
const BOILING_BLOOD: &str = record!("cd55a003-da0d-4522-b08a-d0731b1050c6");
const RETURNED_PHALANX: &str = record!("f8d64a8e-1bff-4960-bc36-1fbb4fc18c8f");

const TURN_SOURCES: &[&str] = &[
    MOBILE_FORT,
    HECKLING_FIENDS,
    ROLLING_STONES,
    RETURNED_PHALANX,
];

/// Activate the board object's first ability, paying and resolving it.
fn activate(g: &mut Game, id: u32, targets: Vec<TargetEntity>) {
    let object = Driver::object(g, id);
    Driver::action(g, Action::ActivateAbility { object, ability: 0 });
    mtg_witness::equipped::pay_and_resolve(g, u64::MAX, targets);
    checked(g);
}

/// CR 609.4, 702.3b: "can attack as though it didn't have defender",
/// statically for Walls and until end of turn for an activated source.
#[test]
fn defender_pretense_offers_the_creature_as_an_attacker() {
    use mtg_contracts::kir::CombatRequirement as R;
    assert!(matches!(
        rule(&lowered(ROLLING_STONES)),
        StaticRule::Combat {
            requirement: R::AttackAsThoughNoDefender,
            ..
        }
    ));
    // "As long as you control a creature with power 4 or greater" is not an
    // admitted static condition.
    assert_eq!(rejected(BRISTLEPACK_SENTRY), "cell:static:rule-condition");
    let mut g = game(
        TURN_SOURCES,
        &[
            ("Rolling Stones", 0, B),
            ("Mobile Fort", 0, B),
            ("Returned Phalanx", 0, B),
        ],
    );
    let d = until(&mut g, &Reason::Attackers);
    let offer = attack_offer(&g, &d);
    assert!(offer.contains_key(&1));
    assert!(!offer.contains_key(&2));

    let mut g = game(
        TURN_SOURCES,
        &[
            ("Returned Phalanx", 0, B),
            ("Island", 0, B),
            ("Island", 0, B),
        ],
    );
    let phalanx = Driver::object(&g, 0);
    assert!(!g.creatures(PlayerId(0), true).contains(&phalanx));
    activate(&mut g, 0, vec![]);
    let d = until(&mut g, &Reason::Attackers);
    assert_eq!(attack_offer(&g, &d)[&0], (vec![1, 2, 3], 0));
    g.respond(d.player, d.id, d.attack(vec![(phalanx, PlayerId(1))]))
        .unwrap();
    checked(&g);
}

/// CR 508.1d, 611.2c: "Target creature attacks this turn if able" makes
/// the announced creature's slot required for this turn only.
#[test]
fn attacks_this_turn_if_able_requires_the_target() {
    assert!(lowered(BOILING_BLOOD).validate().is_ok());
    let mut g = game(
        TURN_SOURCES,
        &[
            ("Heckling Fiends", 0, B),
            ("Grizzly Bears", 0, B),
            ("Mountain", 0, B),
            ("Mountain", 0, B),
            ("Mountain", 0, B),
        ],
    );
    let bears = Driver::object(&g, 1);
    activate(&mut g, 0, vec![TargetEntity::Object { object: bears }]);
    let d = until(&mut g, &Reason::Attackers);
    let offer = attack_offer(&g, &d);
    assert_eq!(offer[&1], (vec![1, 2, 3], 1));
    assert_eq!(offer[&0].1, 0);
    let before = g.snapshot().unwrap();
    assert!(g.respond(d.player, d.id, d.attack(vec![])).is_err());
    assert_eq!(g.snapshot().unwrap(), before);
    g.respond(d.player, d.id, d.attack(vec![(bears, PlayerId(2))]))
        .unwrap();
    checked(&g);
}

const DORAN: &str = record!("a46d96d9-8e59-4777-8208-7730b9e33240");

#[test]
fn combat_damage_assignment_uses_toughness_for_attackers_and_blockers() {
    let mut g = game(
        &[DORAN],
        &[
            ("Doran, the Siege Tower", 0, B),
            ("Storm Crow", 1, B),
            ("Grizzly Bears", 1, B),
        ],
    );
    let d = attack(&mut g, &[0]);
    accepts(&mut g, &d, &[(1, 0), (2, 0)]);
    let source = Driver::object(&g, 0);
    let d = until(
        &mut g,
        &Reason::CombatDamage {
            source,
            power: 0.into(),
        },
    );
    let Shape::Distribute { total, .. } = &d.shape else {
        panic!("two blockers require a damage distribution");
    };
    assert_eq!(*total, 5.into());
    checked(&g);
    let crow = Driver::object(&g, 1);
    let bears = Driver::object(&g, 2);
    let before = g.snapshot().unwrap();
    assert!(
        g.respond(
            d.player,
            d.id,
            d.damage_to(vec![(crow, 0.into()), (bears, 0.into())])
        )
        .is_err()
    );
    assert_eq!(g.snapshot().unwrap(), before);
    g.respond(
        d.player,
        d.id,
        d.damage_to(vec![(crow, 2.into()), (bears, 3.into())]),
    )
    .unwrap();
    let _ = until(&mut g, &Reason::Priority);
    let doran = g
        .state
        .rules
        .objects
        .values()
        .find(|o| o.id == source)
        .unwrap();
    assert_eq!(doran.marked_damage, 4.into());
    checked(&g);
}

#[test]
fn toughness_assignment_lowers_exact_frozen_set_scopes() {
    use mtg_contracts::kir::CombatRequirement::AssignCombatDamageWithToughness;
    for raw in [
        DORAN,
        record!("4d6d3e6d-f958-4c17-b117-960fb8033601"),
        record!("5f657279-cc0f-4431-b517-47bd1c7109b6"),
        record!("fa2d8440-a44a-44fe-8624-0e8b4be53f44"),
    ] {
        let definition = lowered(raw);
        assert!(definition.static_abilities.iter().any(|ability| matches!(
            &ability.effect,
            StaticEffect::Rule {
                rule: StaticRule::Combat {
                    requirement: AssignCombatDamageWithToughness,
                    ..
                },
                condition: None,
            }
        )));
    }
}

#[test]
fn toughness_assignment_survives_pending_damage_replacement() {
    let mut g = game(
        &[
            DORAN,
            record!("3d960d33-623a-4415-ae00-f8cffbc15f5a"),
            record!("e5fa232a-c8f2-4532-88bf-60d1223181b0"),
        ],
        &[
            ("Doran, the Siege Tower", 0, B),
            ("Dictate of the Twin Gods", 0, B),
            ("Urza's Armor", 1, B),
        ],
    );
    let d = attack(&mut g, &[0]);
    accepts(&mut g, &d, &[]);
    for _ in 0..12 {
        let d = Driver::request(&mut g, u64::MAX);
        if matches!(d.reason, Reason::DamageEffect { .. }) {
            let snapshot = g.snapshot().unwrap();
            assert!(snapshot.contains("toughness_sources"));
            let mut restored = Game::restore(g.prepared.as_ref().clone(), &snapshot).unwrap();
            let choice = d.select(d.candidates[0].clone());
            g.respond(d.player, d.id, choice.clone()).unwrap();
            restored.respond(d.player, d.id, choice).unwrap();
            let _ = until(&mut g, &Reason::Priority);
            let _ = until(&mut restored, &Reason::Priority);
            assert_eq!(restored.snapshot().unwrap(), g.snapshot().unwrap());
            return;
        }
        assert_eq!(d.reason, Reason::Priority);
        g.respond(d.player, d.id, mtg_witness::temporary::pass(&d))
            .unwrap();
    }
    panic!("never reached damage replacement");
}
