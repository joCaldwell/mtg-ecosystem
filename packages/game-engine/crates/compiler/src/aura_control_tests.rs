//! Complete pinned d705 witnesses plus explicitly synthetic semantic probes.
//! CR 109.5, 303.4a/m, 611.3 and 613.1b, rules SHA-256
//! 8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const HEADER: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
const MIND: &str =
    include_str!("../../../tests/fixtures/5912546a-acc2-448c-b042-64bdac5ec129.json");
const MAGIC: &str =
    include_str!("../../../tests/fixtures/cd0d7141-46d2-4aa3-bc77-6b3b4513803e.json");

fn lowered(raw: &str) -> Box<Definition> {
    let result = compile(HEADER, raw);
    let CompileResult::Lowered { definition } = result else {
        panic!("expected complete Aura control source: {result:?}");
    };
    definition.validate().unwrap();
    definition
}

fn changed(change: impl FnOnce(&mut Value)) -> String {
    tests::changed_source(MIND, |r| {
        change(r);
        let result = &mut r["faces"][0]["result"];
        for i in 0..result["abilities"].as_array().unwrap().len() {
            result["lines"][i]["ability"] = result["abilities"][i].clone();
        }
    })
}

fn rejected(raw: &str) {
    let result = compile(HEADER, raw);
    assert!(
        matches!(result, CompileResult::Unsupported { .. }),
        "{result:?}"
    );
}

fn rejected_complete_source(raw: &str) {
    let result = compile(HEADER, raw);
    assert!(
        matches!(&result, CompileResult::Unsupported { path, reason }
        if path == "static.control" && reason == "unsupported complete Aura control source"),
        "complete-source rejection must reach the family boundary: {result:?}"
    );
}

#[test]
fn aura_control_complete_frozen_witnesses_have_pinned_source_and_same_reusable_shape() {
    let accepted = accepted_manifest(HEADER).unwrap();
    assert_eq!(
        accepted.file_sha256.as_deref(),
        Some("d705222b280a71cb5a5362f657d530b22402ca51687bac3227c1d6db403d0282")
    );
    let mut definitions = vec![];
    for (raw, name, oracle, record_hash, source_hash, generic, archive_line) in [
        (
            MIND,
            "Mind Control",
            "5912546a-acc2-448c-b042-64bdac5ec129",
            "708e8919bb3f44c792b79faf93099984b6161f87c3318ae1933e7ff1166569d0",
            "822114230f5a5a7abec8f00f67a1c11eb3b99697f34d4c141fd0470513029d31",
            3,
            13385,
        ),
        (
            MAGIC,
            "Control Magic",
            "cd0d7141-46d2-4aa3-bc77-6b3b4513803e",
            "80d4ccb9c5269361e67499ffc76d03442c2f75de2a6042f5b669e0e6f2b3220c",
            "227becbedfa4b8592d744bdbfad0ca8115cbdb8d57287dd9988e1d17f74c8d2c",
            2,
            30912,
        ),
    ] {
        assert_eq!(sha256(raw.trim_end().as_bytes()), record_hash);
        assert!(accepted.record_sha256.contains(record_hash));
        let record: Value = strict_read(raw).unwrap();
        assert_eq!(
            record["source"]["oracle_text"],
            "Enchant creature\nYou control enchanted creature."
        );
        assert_eq!(record["source"]["type_line"], "Enchantment — Aura");
        let d = lowered(raw);
        assert_eq!(d.name, name);
        assert_eq!(d.oracle_id, oracle);
        assert_eq!(d.provenance.source_sha256, source_hash);
        assert_eq!(d.provenance.source_index, archive_line - 2);
        assert_eq!(
            d.provenance.ast_sha256,
            "a7f1c9bd213578c9b979e6d5c24c81ada402dc3218da7005a12d5f894655ddab"
        );
        assert_eq!(
            d.characteristics.mana_cost,
            ManaCost {
                generic: generic.into(),
                colored: vec![Color::U, Color::U]
            }
        );
        assert!(aura_control_source(&d));
        assert_eq!(
            d.capabilities,
            BTreeSet::from([
                Capability::NormalLayoutV1,
                Capability::CastEnchantmentV1,
                Capability::AttachmentsV1,
                Capability::CreatureTargetV1,
                Capability::AuraControlV1,
            ])
        );
        assert_eq!(
            d.static_abilities,
            vec![StaticAbility {
                functioning_zone: FunctioningZone::Battlefield,
                effect: StaticEffect::ControlEnchantedCreature,
            }]
        );
        assert_eq!(
            d.spell,
            Some(SpellProgram {
                timing: SpellTiming::MainPhase,
                resolution: SpellResolution::Permanent,
                functioning_zone: FunctioningZone::Stack,
                additional_sacrifice: None,
                targets: vec![TargetSpec {
                    slot: 0,
                    selection_id: "enchanted-creature".into(),
                    chooser: Chooser::Controller,
                    stage: EvaluationStage::Announcement,
                    domain: TargetDomain::BattlefieldCreature,
                    filter: None
                }],
                instructions: vec![],
                modes: vec![],
            })
        );
        definitions.push(d);
    }
    assert_eq!(definitions[0].spell, definitions[1].spell);
    assert_eq!(
        definitions[0].static_abilities,
        definitions[1].static_abilities
    );
}

#[test]
fn aura_control_renamed_reidentified_typed_source_does_not_dispatch_by_identity() {
    let raw = changed(|r| {
        r["oracleIdentity"] = json!("synthetic-control-oracle");
        r["source"]["oracle_id"] = json!("synthetic-control-oracle");
        r["source"]["id"] = json!("synthetic-control-printing");
        r["source"]["name"] = json!("Synthetic Control Aura");
        r["faces"][0]["result"]["name"] = json!("Synthetic Control Aura");
        // There are no self-name operands here. Literal enchanted/you operands
        // do not derive authority from diagnostic binding references.
        r["binding"] = json!({});
    });
    let d = lowered(&raw);
    assert_eq!(d.oracle_id, "synthetic-control-oracle");
    assert_eq!(d.name, "Synthetic Control Aura");
    assert!(aura_control_source(&d));
    assert_eq!(d.static_abilities, lowered(MIND).static_abilities);
}

#[test]
fn aura_control_rejects_recipient_object_qualifier_and_binding_semantics() {
    for who in [
        json!({"player":"opponent"}),
        json!({"player":"owner"}),
        json!({"player":"you","selectionId":"chosen"}),
        json!({"player":"you","of":{"ref":"enchanted","noun":"creature"}}),
        json!({"player":"you","of":null}),
    ] {
        rejected(&changed(|r| {
            r["faces"][0]["result"]["abilities"][1]["effect"]["who"] = who
        }));
    }
    for what in [
        json!({"ref":"equipped","noun":"creature"}),
        json!({"ref":"enchanted","noun":"permanent"}),
        json!({"ref":"this","noun":"creature"}),
        json!({"ref":"enchanted","noun":"creature","filter":{"types":["creature"],"control":"you"}}),
        json!({"ref":"enchanted","noun":"creature","selectionId":"other"}),
    ] {
        rejected(&changed(|r| {
            r["faces"][0]["result"]["abilities"][1]["effect"]["what"] = what
        }));
    }
    for (key, value) in [
        (
            "bindings",
            json!([{"variable":"x","value":{"amount":"fixed","value":1}}]),
        ),
        ("bindings", json!([])),
        ("duration", json!({"duration":"until-end-of-turn"})),
        (
            "condition",
            json!({"condition":"status","what":{"ref":"enchanted","noun":"creature"},"status":"tapped"}),
        ),
    ] {
        rejected(&changed(|r| {
            r["faces"][0]["result"]["abilities"][1]["effect"][key] = value
        }));
    }
    rejected(&changed(|r| {
        r["faces"][0]["result"]["abilities"][1]["bindings"] = json!([])
    }));
}

#[test]
fn aura_control_rejects_filtered_enchant_additional_text_and_whole_source_compositions() {
    for filter in [
        json!({"types":["creature"],"control":"you"}),
        json!({"types":["permanent"]}),
        json!({"types":["creature"],"properties":["tapped"]}),
    ] {
        rejected_complete_source(&changed(|r| {
            r["faces"][0]["result"]["abilities"][0]["keywords"][0]["filter"] = filter
        }));
    }
    rejected_complete_source(&changed(|r| {
        let text = "You control enchanted creature. It has flying.";
        r["source"]["oracle_text"] = json!(format!("Enchant creature\n{text}"));
        r["faces"][0]["result"]["lines"][1]["text"] = json!(text);
    }));
    for (text, ability) in [
        (
            "Enchanted creature has flying.",
            json!({"kind":"static","effect":{"static":"modify","what":{"ref":"enchanted","noun":"creature"},"modifiers":[{"modifier":"keyword","keyword":"flying"}]}}),
        ),
        (
            "{1}: Draw a card.",
            json!({"kind":"activated","costs":[{"cost":"mana","symbols":["1"]}],"effects":[{"steps":[{"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1}}]}]}),
        ),
        (
            "Choose one —\n• Draw a card.",
            json!({"kind":"spell","effects":[{"steps":[{"effect":"modal","count":{"exactly":1},"options":[{"effects":[{"steps":[{"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1}}]}]}]}]}]}),
        ),
    ] {
        // Valid mirrors must reach semantic admission rather than failing on
        // a malformed synthetic AST field before the complete-family guard.
        serde_json::from_value::<Ability>(ability.clone()).unwrap();
        rejected_complete_source(&changed(|r| {
            r["source"]["oracle_text"] = json!(format!(
                "Enchant creature\nYou control enchanted creature.\n{text}"
            ));
            let result = &mut r["faces"][0]["result"];
            result["abilities"]
                .as_array_mut()
                .unwrap()
                .push(ability.clone());
            result["lines"]
                .as_array_mut()
                .unwrap()
                .push(json!({"text":text,"ok":true,"ability":ability}));
        }));
    }
}
