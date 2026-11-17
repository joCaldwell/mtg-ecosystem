//! Exact frozen complete sources, plus synthetic rejection/parameter controls.
use super::*;
use serde_json::{Value, json};
const HEADER: &str = include_str!("../../../tests/fixtures/ramos/archive-manifest-current.json");
const SOURCES: [&str; 3] = [
    include_str!("../../../tests/fixtures/ramos/6bae00e8-06cf-4ac4-a1cc-757e454109fe.json"),
    include_str!("../../../tests/fixtures/ramos/3123ec89-8e95-4761-ba17-747ec667509f.json"),
    include_str!("../../../tests/fixtures/ramos/f7e7b78c-c769-4720-8585-1874773eb342.json"),
];
fn lower(raw: &str) -> Box<Definition> {
    match compile(HEADER, raw) {
        CompileResult::Lowered { definition } => definition,
        other => panic!("{other:?}"),
    }
}
#[test]
fn cycling_complete_source_three_frozen_tapped_intrinsic_and_hand_programs() {
    for (raw, colors) in SOURCES.into_iter().zip([
        vec![Color::G, Color::U, Color::R],
        vec![Color::G, Color::W, Color::U],
        vec![Color::B, Color::R, Color::G],
    ]) {
        let d = lower(raw);
        d.validate().unwrap();
        assert!(!d.characteristics.basic);
        assert_eq!(d.intrinsic_zone, Some(FunctioningZone::Battlefield));
        assert!(d.spell.is_none() && d.mana_abilities.is_empty());
        assert_eq!(
            d.characteristics
                .subtypes
                .iter()
                .map(|s| basic_land_color(s).unwrap())
                .collect::<Vec<_>>(),
            colors
        );
        assert_eq!(
            d.static_abilities,
            vec![StaticAbility {
                functioning_zone: FunctioningZone::Battlefield,
                effect: StaticEffect::EntersTapped
            }]
        );
        assert_eq!(d.activated.len(), 1);
        assert!(hand_discard_draw_activation(&d.activated[0]));
        assert_eq!(
            d.activated[0].cost,
            ActivationCost::DiscardSource {
                mana: ManaCost {
                    generic: 3.into(),
                    colored: vec![]
                }
            }
        );
        assert_eq!(
            d.activated[0].program.instructions,
            vec![Instruction::DrawCards {
                recipient: None,
                count: 1.into(),
                source_block: 1
            }]
        );
        assert_eq!(d.source_labels.as_ref().unwrap().activated, vec![1]);
        assert_eq!(d.source_labels.as_ref().unwrap().statics, vec![0]);
        let original: Value = serde_json::from_str(raw).unwrap();
        assert_eq!(d.provenance.source_sha256, original["sourceSha256"]);
        assert_eq!(d.provenance.ast_sha256, original["astSha256"]);
        assert_eq!(d.provenance.binding_sha256, original["bindingSha256"]);
    }
}
#[test]
fn cycling_complete_source_rejects_partial_null_unknown_variable_and_binding_obligations() {
    for case in 0..23 {
        let changed = tests::changed_source(SOURCES[0], |r| {
            match case {
                0 => r["faces"][0]["result"]["abilities"][1]["keywords"][0]["cost"] = json!([]),
                1 => r["faces"][0]["result"]["abilities"][1]["keywords"][0]["cost"] = Value::Null,
                2 => {
                    r["faces"][0]["result"]["abilities"][1]["keywords"][0]["cost"][0]["symbols"] =
                        json!(["X"])
                }
                3 => {
                    r["faces"][0]["result"]["abilities"][1]["keywords"][0]["cost"][0]["symbols"] =
                        json!(["03"])
                }
                4 => {
                    r["faces"][0]["result"]["abilities"][1]["keywords"][0]["cost"][0]["symbols"] =
                        json!(["-1"])
                }
                5 => {
                    r["faces"][0]["result"]["abilities"][1]["keywords"][0]["cost"][0]["symbols"] =
                        json!([3])
                }
                6 => {
                    r["faces"][0]["result"]["abilities"][1]["keywords"][0]["discard"] = json!(true)
                }
                7 => r["faces"][0]["result"]["abilities"][1]["abilityWord"] = Value::Null,
                8 => r["faces"][0]["result"]["abilities"][0]["effect"]["condition"] = Value::Null,
                9 => {
                    r["faces"][0]["result"]["abilities"]
                        .as_array_mut()
                        .unwrap()
                        .remove(0);
                }
                10 => {
                    let copy = r["binding"]["references"][1].clone();
                    r["binding"]["references"]
                        .as_array_mut()
                        .unwrap()
                        .push(copy);
                }
                11 => {
                    r["binding"]["references"].as_array_mut().unwrap().pop();
                }
                12 => {
                    r["binding"]["references"][1]["target"] = r["binding"]["nodes"][6]["id"].clone()
                }
                13 => r["binding"]["nodes"][7]["role"] = json!("source-controller"),
                14 => r["binding"]["nodes"][8]["abilities"] = json!(["static"]),
                15 => {
                    let copy = r["binding"]["nodes"][7].clone();
                    r["binding"]["nodes"].as_array_mut().unwrap().push(copy);
                }
                16 => r["binding"]["references"][0]["expression"]["noun"] = json!("card"),
                17 => r["binding"]["counts"]["obligations"] = json!(3),
                18 => r["binding"]["nodes"][8]["unused"] = Value::Null,
                19 => {
                    r["faces"][0]["result"]["abilities"][1]["keywords"][0]["cost"]
                        .as_array_mut()
                        .unwrap()
                        .push(json!({"cost":"tap-self"}));
                }
                20 => {
                    r["source"]["oracle_text"] = json!(
                        "This land enters tapped unless you control two basic lands.\nCycling {3}"
                    );
                    r["faces"][0]["result"]["lines"][0]["text"] =
                        json!("This land enters tapped unless you control two basic lands.");
                }
                21 => r["binding"]["engineFeatures"] = json!(["unconsumed"]),
                _ => {
                    r["faces"][0]["result"]["abilities"][1]["keywords"][0]["cost"][0]["payer"] =
                        json!({"player":"opponent"})
                }
            }
            let copies = r["faces"][0]["result"]["abilities"]
                .as_array()
                .unwrap()
                .clone();
            for (i, a) in copies.into_iter().enumerate() {
                r["faces"][0]["result"]["lines"][i]["ability"] = a;
            }
        });
        assert!(
            !matches!(compile(HEADER, &changed), CompileResult::Lowered { .. }),
            "{case}"
        );
    }
}
#[test]
fn cycling_complete_source_generic_fixed_mana_is_not_printed_cost_dispatch() {
    // Private parameter controls, not frozen records or catalog admission.
    for symbols in [
        vec!["0"],
        vec!["17", "U", "C"],
        vec!["99999999999999999999999999999999999999999"],
    ] {
        let changed = tests::changed_source(SOURCES[0], |r| {
            let text = symbols
                .iter()
                .map(|s| format!("{{{s}}}"))
                .collect::<String>();
            r["source"]["oracle_text"] = json!(format!("This land enters tapped.\nCycling {text}"));
            r["faces"][0]["result"]["lines"][1]["text"] = json!(format!("Cycling {text}"));
            r["faces"][0]["result"]["abilities"][1]["keywords"][0]["cost"][0]["symbols"] =
                json!(symbols);
            r["faces"][0]["result"]["lines"][1]["ability"] =
                r["faces"][0]["result"]["abilities"][1].clone();
        });
        let d = lower(&changed);
        let text = symbols
            .iter()
            .map(|s| format!("{{{s}}}"))
            .collect::<String>();
        assert_eq!(
            d.activated[0].cost,
            ActivationCost::DiscardSource {
                mana: mana_cost(&text).unwrap()
            }
        );
    }
}
