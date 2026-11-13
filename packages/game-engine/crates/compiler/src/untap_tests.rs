//! Exact retained source and explicitly synthetic complete-consumption probes.
use super::*;
use serde_json::{Value, json};

const MANIFEST: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
const BURST: &str =
    include_str!("../../../tests/fixtures/b795ecfd-31ac-4a2a-9679-97b088932d93.json");

fn changed(text: &str, mutate: impl FnOnce(&mut Value)) -> String {
    tests::changed_source(BURST, |record| {
        record["source"]["oracle_text"] = text.into();
        let result = &mut record["faces"][0]["result"];
        mutate(&mut result["abilities"][0]);
        result["lines"][0]["text"] = text.into();
        result["lines"][0]["ability"] = result["abilities"][0].clone();
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
fn complete_burst_source_preserves_provenance_and_unqualified_permanent_target() {
    assert_eq!(
        sha256(BURST.trim_end().as_bytes()),
        "03bc2da03720bce11afddda29142f16ccd89f0bfdc16057571dc6f0a183947af"
    );
    let raw: Value = strict_read(BURST).unwrap();
    assert_eq!(raw["source"]["oracle_text"], "Untap target permanent.");
    assert_eq!(raw["binding"]["fullyBound"], true);
    let result = compile(MANIFEST, BURST);
    let CompileResult::Lowered { definition: d } = result else {
        panic!("{result:?}")
    };
    assert_eq!(d.name, "Burst of Energy");
    assert_eq!(d.oracle_id, "b795ecfd-31ac-4a2a-9679-97b088932d93");
    assert_eq!(d.characteristics.kind, CardKind::Instant);
    assert_eq!(
        d.characteristics.mana_cost,
        ManaCost {
            generic: 0.into(),
            colored: vec![Color::W]
        }
    );
    assert_eq!(d.provenance.source_index, 27635);
    assert_eq!(
        d.provenance.source_sha256,
        "bdfb17b6ba8a1595f3cf1d090b12b39f51dcea32679b9c1eaede113dde07738c"
    );
    assert_eq!(
        d.provenance.ast_sha256,
        "fd765a1886019f035fd4f37fcd6c858835404beefd806049e6d8769cb0f7d65d"
    );
    assert_eq!(
        d.provenance.binding_sha256,
        "064fcc24986de7e617c268bae003f3d7b81bfa2dc19c491c14a08519702969bf"
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
                domain: TargetDomain::BattlefieldPermanent,
                filter: Some(PermanentFilter {
                    types: vec![],
                    non_types: vec![],
                    non_colors: vec![],
                    opponent: false,
                    tapped: false
                }),
            }],
            instructions: vec![Instruction::UntapPermanent {
                target_slot: 0,
                source_block: 0
            }],
        })
    );
    assert_eq!(
        d.capabilities,
        BTreeSet::from([
            Capability::NormalLayoutV1,
            Capability::CastInstantV1,
            Capability::PermanentRemovalV1,
            Capability::UntapPermanentV1
        ])
    );
    d.validate().unwrap();
}

#[test]
fn untap_source_rejects_all_unrepresented_target_and_step_fields() {
    for (field, value) in [
        ("count", json!({"amount":"fixed","value":0})),
        ("count", json!({"amount":"fixed","value":2})),
        ("count", json!({"amount":"X"})),
        ("ref", json!("all")),
        ("selectionId", json!("")),
        ("selectionId", Value::Null),
        ("of", json!({"ref":"this","noun":"creature"})),
    ] {
        let raw = changed("Synthetic unsupported untap target.", |a| {
            a["effects"][0]["steps"][0]["what"][field] = value
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
        json!({"types":["creature"]}),
        json!({"cls":"permanent","control":"you"}),
        json!({"cls":"permanent","control":"opponent"}),
        json!({"cls":"permanent","owner":"you"}),
        json!({"cls":"permanent","status":["tapped"]}),
        json!({"cls":"permanent","status":["untapped"]}),
        json!({"cls":"permanent","zone":{"zone":"graveyard"}}),
        json!({"cls":"permanent","other":true}),
    ] {
        let raw = changed("Synthetic qualified untap target.", |a| {
            a["effects"][0]["steps"][0]["what"]["filter"] = filter
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
        ("duration", json!({"duration":"until-end-of-turn"})),
    ] {
        let raw = changed("Synthetic qualified untap effect.", |a| {
            a["effects"][0]["steps"][0][field] = value
        });
        assert!(
            !matches!(compile(MANIFEST, &raw), CompileResult::Lowered { .. }),
            "{field}"
        );
    }
}

#[test]
fn untap_rejects_extra_effects_and_modal_composition_as_whole_sources() {
    let raw = changed("Untap target permanent. Draw a card.", |a| {
        a["effects"][0]["steps"].as_array_mut().unwrap().push(
            json!({"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1}}),
        );
    });
    let result = compile(MANIFEST, &raw);
    assert!(
        matches!(result, CompileResult::Unsupported { ref path, .. } if path == "definition"),
        "{result:?}"
    );
    let raw = changed(
        "Choose one — Untap target permanent; or draw a card.",
        |a| {
            let untap = a["effects"].clone();
            a["effects"] = json!([{"steps":[{"effect":"modal","count":{"exactly":1},"options":[{"effects":untap},{"effects":[{"steps":[{"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1}}]}]}]}]}]);
        },
    );
    let result = compile(MANIFEST, &raw);
    assert!(
        matches!(result, CompileResult::Unsupported { ref path, .. } if path == "definition"),
        "{result:?}"
    );
}

#[test]
fn untap_source_does_not_acquire_activated_or_triggered_support() {
    for activated in [true, false] {
        let text = if activated {
            "{T}: Untap target permanent."
        } else {
            "When this creature enters, untap target permanent."
        };
        let raw = tests::changed_source(BURST, |record| {
            record["source"]["type_line"] = "Creature — Human".into();
            record["source"]["power"] = "1".into();
            record["source"]["toughness"] = "1".into();
            record["source"]["oracle_text"] = text.into();
            let result = &mut record["faces"][0]["result"];
            let ability = &mut result["abilities"][0];
            ability["kind"] = if activated { "activated" } else { "triggered" }.into();
            if activated {
                ability["costs"] = json!([{"cost":"tap-self"}]);
            } else {
                ability["trigger"] =
                    json!({"trigger":"enters","what":{"ref":"this","noun":"creature"}});
            }
            result["lines"][0]["text"] = text.into();
            result["lines"][0]["ability"] = result["abilities"][0].clone();
            // Retained binding data is diagnostic. Keep its known scope/face
            // descriptions aligned; the Rust compiler owns support decisions.
            for node in record["binding"]["nodes"].as_array_mut().unwrap() {
                match node["node"].as_str() {
                    Some("face") => node["typeLine"] = "Creature — Human".into(),
                    Some("ability") => {
                        node["abilityKind"] =
                            if activated { "activated" } else { "triggered" }.into()
                    }
                    Some("runtime-player") => node["role"] = "ability-controller".into(),
                    _ => (),
                }
            }
        });
        let result = compile(MANIFEST, &raw);
        assert!(
            matches!(result, CompileResult::Unsupported { .. }),
            "{text}: {result:?}"
        );
    }
}
