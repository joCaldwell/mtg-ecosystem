//! Complete frozen records, and coherently rehashed semantic/binding controls.
use super::*;
use serde_json::json;
const MANIFEST: &str = include_str!("../../../tests/fixtures/ramos/archive-manifest-current.json");
const NEXUS: &str =
    include_str!("../../../tests/fixtures/ramos/b1c346a4-fc9b-474a-9d51-cac168e363fa.json");
const SORCERER: &str =
    include_str!("../../../tests/fixtures/ramos/a450ef56-8511-43e0-bdca-0c25f54ba338.json");
#[test]
fn ramos_m4d_complete_frozen_history_sources() {
    for (raw, hash, eligibility) in [
        (
            NEXUS,
            "5d6da7b47b1501e7f4927e1f776875e87a03c5aefad9f6b44c938dcfb79ecf91",
            SpellCascadeEligibility::FirstCastEachTurn {},
        ),
        (
            SORCERER,
            "5f2d6518c7bfa05693bdf48d4911fc320e582252bf9da609fba699835c851728",
            SpellCascadeEligibility::FirstCastFromExileEachTurn {},
        ),
    ] {
        assert_eq!(sha256(raw.as_bytes()), hash);
        let result = compile(MANIFEST, raw);
        let CompileResult::Lowered { definition: d } = result else {
            panic!("{result:?}")
        };
        d.validate().unwrap();
        assert!(d.triggered.is_empty());
        assert_eq!(
            d.static_abilities,
            [StaticAbility {
                functioning_zone: FunctioningZone::Battlefield,
                effect: StaticEffect::GrantSpellCascade {
                    eligibility,
                    program: canonical_cascade_program(0)
                }
            }]
        );
        assert!(d.capabilities.contains(&Capability::CastHistoryV1));
        if raw == NEXUS {
            assert_eq!(d.characteristics.kind, CardKind::Enchantment);
            assert_eq!(
                d.characteristics.mana_cost,
                ManaCost {
                    generic: 0.into(),
                    colored: vec![Color::W, Color::U, Color::B, Color::R, Color::G]
                }
            );
        } else {
            assert_eq!(d.characteristics.kind, CardKind::Creature);
            assert_eq!(d.characteristics.subtypes, ["Orc", "Shaman", "Sorcerer"]);
            assert_eq!(d.characteristics.power, Some(4.into()));
            assert_eq!(d.characteristics.toughness, Some(3.into()));
            assert_eq!(
                d.characteristics.mana_cost,
                ManaCost {
                    generic: 3.into(),
                    colored: vec![Color::R]
                }
            );
        }
    }
}
#[test]
fn ramos_m4d_whole_scope_and_gained_ability_rejections() {
    for raw in [NEXUS, SORCERER] {
        for mutation in 0..18 {
            let changed = tests::changed_source(raw, |r| {
                for mirror in ["abilities", "lines"] {
                    let a = if mirror == "abilities" {
                        &mut r["faces"][0]["result"][mirror][0]
                    } else {
                        &mut r["faces"][0]["result"][mirror][0]["ability"]
                    };
                    let e = &mut a["effect"];
                    match mutation {
                        0 => e["scope"]["occurrence"]["ordinal"] = json!(2),
                        1 => e["scope"]["occurrence"]["ordinal"] = json!("1"),
                        2 => e["scope"]["occurrence"]["ordinal"] = json!(null),
                        3 => e["scope"]["occurrence"]["ordinal"] = json!(1.5),
                        4 => e["scope"]["occurrence"]["period"] = json!("game"),
                        5 => e["scope"]["caster"] = json!({"player":"opponent"}),
                        6 => e["scope"]["from"] = json!(null),
                        7 => e["scope"]["from"] = json!({"zone":"hand"}),
                        8 => e["scope"]["spells"]["filter"]["color"] = json!("red"),
                        9 => e["scope"]["action"] = json!("copy"),
                        10 => e["scope"]["duration"] = json!("turn"),
                        11 => e["abilities"][0]["keyword"]["keyword"] = json!("haste"),
                        12 => e["abilities"]
                            .as_array_mut()
                            .unwrap()
                            .push(json!({"gained":"keyword","keyword":{"keyword":"cascade"}})),
                        13 => e["abilities"][0]["keyword"]["cost"] = json!([]),
                        14 => {
                            e["scope"]["occurrence"]
                                .as_object_mut()
                                .unwrap()
                                .remove("ordinal");
                        }
                        15 => e["scope"]["occurrence"]["ordinal"] = json!(-1),
                        16 => e["scope"]["occurrence"]["extra"] = json!(true),
                        17 => e["extra"] = json!(true),
                        _ => unreachable!(),
                    }
                }
                // Keep executable query references consistent: rejection must consume scope.
                let spells =
                    r["faces"][0]["result"]["abilities"][0]["effect"]["scope"]["spells"].clone();
                for n in r["binding"]["nodes"].as_array_mut().unwrap() {
                    if n["node"] == "selection" {
                        n["expression"] = spells.clone();
                    }
                }
                for n in r["binding"]["references"].as_array_mut().unwrap() {
                    if n["path"].as_str().unwrap().ends_with("/spells") {
                        n["expression"] = spells.clone();
                    }
                }
            });
            assert!(
                !matches!(compile(MANIFEST, &changed), CompileResult::Lowered { .. }),
                "{mutation}"
            );
        }
    }
}
#[test]
fn ramos_m4d_binding_lies_and_duplicate_nodes_reject() {
    for raw in [NEXUS, SORCERER] {
        for mutation in 0..7 {
            let changed = tests::changed_source(raw, |r| {
                let nodes = r["binding"]["nodes"].as_array_mut().unwrap();
                match mutation {
                    0 => {
                        nodes.remove(0);
                    }
                    1 => {
                        nodes.remove(1);
                    }
                    2 => nodes[2]["source"] = json!("wrong"),
                    3 => nodes[3]["role"] = json!("trigger-controller"),
                    4 => nodes[4]["timing"] = json!("announcement"),
                    5 => nodes.push(nodes[4].clone()),
                    6 => r["binding"]["references"][0]["target"] = json!("wrong"),
                    _ => unreachable!(),
                }
                r["binding"]["fullyBound"] = json!(true);
            });
            assert!(
                !matches!(compile(MANIFEST, &changed), CompileResult::Lowered { .. }),
                "binding {mutation}"
            );
        }
    }
}
