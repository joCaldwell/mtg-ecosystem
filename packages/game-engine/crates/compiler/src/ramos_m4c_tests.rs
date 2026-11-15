//! UNQUALIFIED source checkpoint: complete frozen Imoti and explicit AST controls.
use super::*;
use serde_json::json;
const MANIFEST: &str = include_str!("../../../tests/fixtures/ramos/archive-manifest-current.json");
const IMOTI: &str =
    include_str!("../../../tests/fixtures/ramos/eca32dcd-6845-433e-a631-ed1f0ee78f25.json");
#[test]
fn ramos_m4c_complete_frozen_imoti_preserves_both_clauses_and_characteristics() {
    assert_eq!(
        sha256(IMOTI.as_bytes()),
        "0a5f152205e1f70b539aaf05fc9b019f4ea64354c53439d992477572bcde17d2"
    );
    let r: serde_json::Value = strict_read(IMOTI).unwrap();
    assert_eq!(r["sourceIndex"], 35755);
    let CompileResult::Lowered { definition: d } = compile(MANIFEST, IMOTI) else {
        panic!("complete Imoti")
    };
    d.validate().unwrap();
    assert_eq!(d.characteristics.kind, CardKind::Creature);
    assert!(d.characteristics.legendary);
    assert_eq!(d.characteristics.subtypes, ["Snake", "Druid"]);
    assert_eq!(d.characteristics.power, Some(3.into()));
    assert_eq!(d.characteristics.toughness, Some(1.into()));
    assert_eq!(
        d.characteristics.mana_cost,
        ManaCost {
            generic: 3.into(),
            colored: vec![Color::G, Color::U]
        }
    );
    assert_eq!(d.characteristics.colors, [Color::G, Color::U]);
    assert_eq!(d.characteristics.color_identity, [Color::G, Color::U]);
    assert_eq!(d.triggered.len(), 1);
    assert_eq!(d.triggered[0].condition, TriggerCondition::SelfSpellCast);
    assert_eq!(d.triggered[0].program, canonical_cascade_program(0));
    assert_eq!(
        d.static_abilities,
        [StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::GrantSpellCascade {
                mana_value_at_least: 6.into(),
                program: canonical_cascade_program(1)
            },
        }]
    );
    for cap in [
        Capability::StaticSpellCascadeGrantV1,
        Capability::CascadeV1,
        Capability::ResolutionCastV1,
        Capability::RandomBottomV1,
    ] {
        let mut bad = d.as_ref().clone();
        assert!(bad.capabilities.remove(&cap));
        assert!(bad.validate().is_err());
    }
}
#[test]
fn ramos_m4c_synthetic_ast_thresholds_are_exact_parameters() {
    let r: serde_json::Value = strict_read(IMOTI).unwrap();
    for value in [json!(0), json!(2), json!(9), json!(u64::MAX)] {
        let mut effect = r["faces"][0]["result"]["abilities"][1]["effect"].clone();
        effect["what"]["filter"]["properties"][0]["comparison"]["value"]["value"] = value.clone();
        let effect: SourceStaticEffect = serde_json::from_value(effect).unwrap();
        let a = spell_cascade_grants::lower(&effect, 17).unwrap().unwrap();
        assert_eq!(
            a.effect,
            StaticEffect::GrantSpellCascade {
                mana_value_at_least: value.to_string().parse().unwrap(),
                program: canonical_cascade_program(17),
            }
        );
    }
}
#[test]
fn ramos_m4c_complete_source_rejects_unconsumed_grant_operands() {
    for mutation in 0..18 {
        let raw = tests::changed_source(IMOTI, |r| {
            for section in ["abilities", "lines"] {
                let a = if section == "abilities" {
                    &mut r["faces"][0]["result"][section][1]
                } else {
                    &mut r["faces"][0]["result"][section][1]["ability"]
                };
                let e = &mut a["effect"];
                match mutation {
                    0 => {
                        e["what"]["filter"]["properties"][0]["comparison"]["value"]["value"] =
                            json!(-1)
                    }
                    1 => {
                        e["what"]["filter"]["properties"][0]["comparison"]["value"]["value"] =
                            json!(null)
                    }
                    2 => e["what"]["filter"]["properties"][0]["comparison"]["op"] = json!("gt"),
                    3 => {
                        e["what"]["filter"]["properties"][0]["comparison"]["value"]["amount"] =
                            json!("x")
                    }
                    4 => {
                        e["what"]["filter"]["properties"][0]["comparison"]["value"]["value"] =
                            json!("6")
                    }
                    5 => e["what"]["filter"]["properties"]
                        .as_array_mut()
                        .unwrap()
                        .push(json!({"property":"color","value":"green"})),
                    6 => e["what"]["filter"]["cast"]["by"]["player"] = json!("opponent"),
                    7 => e["what"]["filter"]["cast"]["from"] = json!({"zone":"hand"}),
                    8 => {
                        e["what"]["filter"]["cast"]["occurrence"] =
                            json!({"ordinal":1,"period":"turn"})
                    }
                    9 => e["what"]["filter"]["zone"] = json!("exile"),
                    10 => e["duration"] = json!("until-end-of-turn"),
                    11 => e["modifiers"][0]["abilities"][0]["keyword"]["keyword"] = json!("haste"),
                    12 => e["modifiers"][0]["abilities"][0]["keyword"]["cost"] = json!([]),
                    13 => e["modifiers"][0]["abilities"]
                        .as_array_mut()
                        .unwrap()
                        .push(json!({"gained":"keyword","keyword":{"keyword":"cascade"}})),
                    14 => e["modifiers"] = json!([]),
                    15 => e["what"]["ref"] = json!("target"),
                    16 => {
                        e["what"]["filter"]["properties"][0]["comparison"]["value"]["extra"] =
                            json!(true)
                    }
                    17 => a["extra_clause"] = json!({}),
                    _ => unreachable!(),
                }
            }
        });
        assert!(
            !matches!(compile(MANIFEST, &raw), CompileResult::Lowered { .. }),
            "operand {mutation}"
        );
    }
}
#[test]
fn ramos_m4c_rehashed_nested_binding_lies_do_not_follow_fully_bound_flag() {
    for suffix in [
        "#you",
        "/effect/what#query",
        "/effect/modifiers/0/abilities/0#recipient",
        "/effect/modifiers/0/abilities/0#ability",
        "/effect/modifiers/0/abilities/0/keyword#you",
    ] {
        let raw = tests::changed_source(IMOTI, |r| {
            let id = format!(
                "card:{}/faces/0/abilities/1{suffix}",
                r["oracleIdentity"].as_str().unwrap()
            );
            r["binding"]["nodes"]
                .as_array_mut()
                .unwrap()
                .retain(|n| n["id"] != id);
            r["binding"]["fullyBound"] = json!(true);
        });
        assert!(
            !matches!(compile(MANIFEST, &raw), CompileResult::Lowered { .. }),
            "binding {suffix}"
        );
    }
}
#[test]
fn ramos_m4c_history_and_unparsed_whole_sources_remain_unsupported() {
    for raw in [
        include_str!("../../../tests/fixtures/ramos/b1c346a4-fc9b-474a-9d51-cac168e363fa.json"),
        include_str!("../../../tests/fixtures/ramos/a450ef56-8511-43e0-bdca-0c25f54ba338.json"),
        include_str!("../../../tests/fixtures/ramos/9efe8aff-9a7d-4397-b5fd-c1a0fad7c15f.json"),
    ] {
        assert!(matches!(
            compile(MANIFEST, raw),
            CompileResult::Unsupported { .. }
        ));
    }
}
