//! Exact Pressure Point source and explicitly synthetic scope-consumption probes.
use super::*;
use serde_json::{Value, json};

const MANIFEST: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
const PRESSURE: &str =
    include_str!("../../../tests/fixtures/d4bbdd0c-e578-49ed-b2f7-b1decb04ee87.json");

fn changed(text: &str, mutate: impl FnOnce(&mut Value)) -> String {
    tests::changed_source(PRESSURE, |record| {
        record["source"]["oracle_text"] = text.into();
        mutate(record);
        let source = &record["source"];
        let lines = source_lines(
            source["oracle_text"].as_str().unwrap(),
            source["name"].as_str().unwrap(),
            source["type_line"].as_str().unwrap(),
        )
        .unwrap();
        let result = &mut record["faces"][0]["result"];
        assert_eq!(lines.len(), result["abilities"].as_array().unwrap().len());
        result["lines"] = Value::Array(
            lines
                .into_iter()
                .enumerate()
                .map(|(i, text)| json!({"text":text,"ok":true,"ability":result["abilities"][i]}))
                .collect(),
        );
        let target = result["abilities"][0]["effects"][0]["steps"][0]["what"].clone();
        let path = "/faces/0/abilities/0/effects/0/steps/0/what";
        for node in record["binding"]["nodes"].as_array_mut().unwrap() {
            if node["node"] == "selection" && node["path"] == path {
                node["expression"] = target.clone();
            }
        }
        for reference in record["binding"]["references"].as_array_mut().unwrap() {
            if reference["path"] == path {
                reference["expression"] = target.clone();
            }
        }
    })
}

#[test]
fn complete_pressure_point_preserves_tap_then_draw_and_full_provenance() {
    assert_eq!(
        sha256(PRESSURE.trim_end().as_bytes()),
        "996b7de09db304f21077c066d3cc692bb6c41b5ed38dce6657d1aad07b605f4c"
    );
    let raw: Value = strict_read(PRESSURE).unwrap();
    assert_eq!(
        raw["source"]["oracle_text"],
        "Tap target creature.\nDraw a card."
    );
    assert_eq!(raw["binding"]["fullyBound"], true);
    let result = compile(MANIFEST, PRESSURE);
    let CompileResult::Lowered { definition: d } = result else {
        panic!("{result:?}")
    };
    assert_eq!(d.name, "Pressure Point");
    assert_eq!(d.oracle_id, "d4bbdd0c-e578-49ed-b2f7-b1decb04ee87");
    assert_eq!(d.characteristics.kind, CardKind::Instant);
    assert_eq!(
        d.characteristics.mana_cost,
        ManaCost {
            generic: 1.into(),
            colored: vec![Color::W]
        }
    );
    assert_eq!(d.provenance.source_index, 32116);
    assert_eq!(
        d.provenance.source_sha256,
        "afceea85ed79f36c110705dd1cc1308da45ed2750187b6f713ef9d7cf2b97679"
    );
    assert_eq!(
        d.provenance.ast_sha256,
        "de96d9dc6d2d250c91ab9a6e14346be96eb80553acdf214f773041bd78b43470"
    );
    assert_eq!(
        d.provenance.binding_sha256,
        "31f397f66f4bde7fe7da480f40ab4ed4eed49764331fc16bc52a4d5a95473046"
    );
    assert_eq!(
        strict_read::<Value>(&d.provenance.source_json).unwrap(),
        raw["source"]
    );
    assert_eq!(
        strict_read::<Value>(&d.provenance.abilities_json).unwrap(),
        raw["faces"][0]["result"]["abilities"]
    );
    assert!(d.activated.is_empty() && d.triggered.is_empty() && d.static_abilities.is_empty());
    assert_eq!(
        d.spell,
        Some(SpellProgram {
            additional_sacrifice: None,
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            functioning_zone: FunctioningZone::Stack,
            modes: vec![],
            targets: vec![TargetSpec {
                slot: 0,
                selection_id: "line:0:1".into(),
                chooser: Chooser::Controller,
                stage: EvaluationStage::Announcement,
                domain: TargetDomain::BattlefieldCreature,
                filter: None
            }],
            instructions: vec![
                Instruction::TapPermanent {
                    target_slot: 0,
                    source_block: 0
                },
                Instruction::DrawCards {
                    count: 1.into(),
                    recipient: None,
                    source_block: 1
                }
            ],
        })
    );
    assert_eq!(
        d.capabilities,
        BTreeSet::from([
            Capability::NormalLayoutV1,
            Capability::CastInstantV1,
            Capability::CreatureTargetV1,
            Capability::TapPermanentV1,
            Capability::ControllerDrawV1
        ])
    );
    d.validate().unwrap();
}

#[test]
fn tap_source_consumes_complete_target_and_rejects_optional_or_qualified_steps() {
    for (field, value) in [
        ("count", json!({"amount":"fixed","value":0})),
        ("count", json!({"amount":"fixed","value":2})),
        ("count", json!({"amount":"x"})),
        ("ref", json!("all")),
        ("selectionId", json!("")),
        ("of", json!({"ref":"this","noun":"creature"})),
        ("upTo", json!(true)),
    ] {
        let raw = changed("Synthetic tap target.\nDraw a card.", |r| {
            r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["what"][field] = value
        });
        let result = compile(MANIFEST, &raw);
        assert!(
            matches!(result, CompileResult::Unsupported { .. }),
            "{field}: {result:?}"
        );
    }
    for filter in [
        Value::Null,
        json!({}),
        json!({"cls":"permanent"}),
        json!({"types":["artifact","creature"]}),
        json!({"types":["creature"],"control":"you"}),
        json!({"types":["creature"],"control":"opponent"}),
        json!({"types":["creature"],"owner":"you"}),
        json!({"types":["creature"],"status":["untapped"]}),
        json!({"types":["creature"],"status":["tapped"]}),
        json!({"types":["creature"],"subtypes":["Human"]}),
    ] {
        let raw = changed("Synthetic qualified tap target.\nDraw a card.", |r| {
            r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["what"]["filter"] =
                filter
        });
        let result = compile(MANIFEST, &raw);
        assert!(
            matches!(result, CompileResult::Unsupported { .. }),
            "{result:?}"
        );
    }
    for (field, value) in [
        ("optional", json!(true)),
        ("optional", json!(false)),
        ("optional", Value::Null),
        ("who", json!({"player":"you"})),
    ] {
        let raw = changed("Synthetic qualified tap step.\nDraw a card.", |r| {
            r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0][field] = value
        });
        assert!(
            !matches!(compile(MANIFEST, &raw), CompileResult::Lowered { .. }),
            "{field}"
        );
    }
}

#[test]
fn tap_source_preserves_standalone_support_and_rejects_unreviewed_composition() {
    let standalone = changed("Tap target creature.", |r| {
        r["faces"][0]["result"]["abilities"]
            .as_array_mut()
            .unwrap()
            .pop();
    });
    let result = compile(MANIFEST, &standalone);
    let CompileResult::Lowered { definition } = result else {
        panic!("{result:?}")
    };
    assert!(matches!(
        definition.spell.unwrap().instructions.as_slice(),
        [Instruction::TapPermanent {
            target_slot: 0,
            source_block: 0
        }]
    ));
    for case in [
        "draw-two",
        "optional",
        "other-player",
        "extra-step",
        "reversed",
    ] {
        let text = if case == "reversed" {
            "Draw a card.\nTap target creature."
        } else {
            "Synthetic tap creature.\nSynthetic draw clause."
        };
        let raw = changed(text, |r| {
            let a = r["faces"][0]["result"]["abilities"].as_array_mut().unwrap();
            match case {
                "draw-two" => a[1]["effects"][0]["steps"][0]["amount"]["value"] = 2.into(),
                "optional" => a[1]["effects"][0]["steps"][0]["optional"] = true.into(),
                "other-player" => {
                    a[1]["effects"][0]["steps"][0]["who"] = json!({"player":"each-opponent"})
                }
                "extra-step" => {
                    a[1]["effects"][0]["steps"].as_array_mut().unwrap().push(json!({"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1}}));
                }
                "reversed" => a.swap(0, 1),
                _ => unreachable!(),
            }
        });
        let result = compile(MANIFEST, &raw);
        assert!(
            matches!(result, CompileResult::Unsupported { .. }),
            "{case}: {result:?}"
        );
    }
}

#[test]
fn tap_source_rejects_modal_activated_and_triggered_contexts_as_whole_cards() {
    for context in ["modal", "activated", "triggered"] {
        let text = match context {
            "modal" => "Choose one — tap target creature; or draw a card.",
            "activated" => "{T}: Tap target creature.",
            _ => "When this creature enters, tap target creature.",
        };
        let raw = changed(text, |r| {
            let mut ability = r["faces"][0]["result"]["abilities"][0].clone();
            if context == "modal" {
                let tap = ability["effects"].clone();
                let draw = r["faces"][0]["result"]["abilities"][1]["effects"].clone();
                ability["effects"] = json!([{"steps":[{"effect":"modal","count":{"exactly":1},"options":[{"effects":tap},{"effects":draw}]}]}]);
            } else {
                r["source"]["type_line"] = "Creature — Human".into();
                r["source"]["power"] = "1".into();
                r["source"]["toughness"] = "1".into();
                ability["kind"] = context.into();
                if context == "activated" {
                    ability["costs"] = json!([{"cost":"tap-self"}]);
                } else {
                    ability["trigger"] =
                        json!({"trigger":"enters","what":{"ref":"this","noun":"creature"}});
                }
            }
            r["faces"][0]["result"]["abilities"] = json!([ability]);
        });
        let result = compile(MANIFEST, &raw);
        assert!(
            matches!(result, CompileResult::Unsupported { .. }),
            "{context}: {result:?}"
        );
    }
}
