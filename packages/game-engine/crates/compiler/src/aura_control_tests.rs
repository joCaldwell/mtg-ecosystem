//! Complete pinned d705 witnesses plus explicitly synthetic semantic probes.
//! CR 109.5, 303.4a/m, 611.3 and 613.1b, rules SHA-256
//! 8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const HEADER: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
const MIND: &str =
    include_str!("../../../tests/fixtures/5912546a-acc2-448c-b042-64bdac5ec129.json");
const SPIRIT: &str =
    include_str!("../../../tests/fixtures/15b618f0-281f-4745-8504-bd70e7b4a267.json");
const EMBRACE: &str =
    include_str!("../../../tests/fixtures/d5cb0370-a6f0-40c0-8b8f-553fca23778d.json");
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
    changed_from(MIND, change)
}

fn changed_from(raw: &str, change: impl FnOnce(&mut Value)) -> String {
    tests::changed_source(raw, |r| {
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
            3_i64,
            13385_u32,
        ),
        (
            MAGIC,
            "Control Magic",
            "cd0d7141-46d2-4aa3-bc77-6b3b4513803e",
            "80d4ccb9c5269361e67499ffc76d03442c2f75de2a6042f5b669e0e6f2b3220c",
            "227becbedfa4b8592d744bdbfad0ca8115cbdb8d57287dd9988e1d17f74c8d2c",
            2_i64,
            30912_u32,
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
                optional_additional_costs: vec![],
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
        // Pin the entire old executable serialization, including capability
        // order and omitted defaults. Provenance remains exact source bytes.
        let mut executable = serde_json::to_value(&d).unwrap();
        executable.as_object_mut().unwrap().remove("provenance");
        let expected = json!({
            "oracle_id":oracle,"name":name,"intrinsic_zone":null,
            "characteristics":{
                "kind":"enchantment","subtypes":["Aura"],"basic":false,"legendary":false,
                "mana_cost":{"generic":generic.to_string(),"colored":["U","U"]},
                "colors":["U"],"color_identity":["U"],"power":null,"toughness":null
            },
            "spell":{
                "timing":"main_phase","resolution":"permanent","functioning_zone":"stack",
                "targets":[{"slot":0,"selection_id":"enchanted-creature","chooser":"controller",
                    "stage":"announcement","domain":"battlefield_creature"}],"instructions":[]
            },
            "static_abilities":[{"functioning_zone":"battlefield","effect":"control_enchanted_creature"}],
            "capabilities":["cast_enchantment_v1","aura_control_v1","normal_layout_v1","creature_target_v1","attachments_v1"]
        });
        assert_eq!(
            canonical(&executable).unwrap(),
            canonical(&expected).unwrap()
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
fn aura_control_rejects_filtered_enchant_unrepresented_text_and_outside_compositions() {
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
            "Creatures you control get +1/+1.",
            json!({"kind":"static","effect":{"static":"modify","what":{"ref":"each","filter":{"types":["creature"],"control":"you"}},"modifiers":[{"modifier":"pt","power":{"sign":1,"amount":{"amount":"fixed","value":1}},"toughness":{"sign":1,"amount":{"amount":"fixed","value":1}}}]}}),
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

#[test]
fn aura_control_composed_frozen_sources_bind_complete_modifiers_without_new_capabilities() {
    let accepted = accepted_manifest(HEADER).unwrap();
    let plain = lowered(MIND);
    for (
        raw,
        name,
        oracle,
        record_hash,
        source_hash,
        ast_hash,
        binding_hash,
        index,
        colors,
        keyword,
        text,
    ) in [
        (
            SPIRIT,
            "Spirit Away",
            "15b618f0-281f-4745-8504-bd70e7b4a267",
            "bd72899f4c06c856f0f0e3e49682874fbdc01ae076683c50b806acc2f291a3e9",
            "be0f4ba1f07ffe5b0404b9b726fc1e9402c4c72001d64a35e1f1bbf2ace845f5",
            "96ca22959f69810fa7699aa29c18ea3829f6476cc5b069fcc46d7022aaf94d27",
            "6d232f31b42e5b060b4604fa395c8747278542eb359603f7cc10f86f27267c89",
            3304,
            vec![Color::U, Color::U],
            KeywordAbility::Flying,
            "Enchanted creature gets +2/+2 and has flying.",
        ),
        (
            EMBRACE,
            "Yavimaya's Embrace",
            "d5cb0370-a6f0-40c0-8b8f-553fca23778d",
            "f7b40c2bb18b54420297dd8cdbfb47012551c04372bf3157bf3956c84235172a",
            "76ae331c38d8f88c8b6c7275e7a1fd640a7cdcbf82319952bf0bd948d56568ea",
            "971e832ff370822c8c97c9e267f6eb0f6450a2e75f8fc77476d343389bf7bafd",
            "fb25a3e592a8da76369a6308424c34d10718086e5dd7af7c6b32b5492d3e27cc",
            32278,
            vec![Color::G, Color::U, Color::U],
            KeywordAbility::Trample,
            "Enchanted creature gets +2/+2 and has trample.",
        ),
    ] {
        assert_eq!(sha256(raw.trim_end().as_bytes()), record_hash);
        assert!(accepted.record_sha256.contains(record_hash));
        let record: Value = strict_read(raw).unwrap();
        assert_eq!(record["source"]["layout"], "normal");
        assert_eq!(record["source"]["type_line"], "Enchantment — Aura");
        assert_eq!(record["source"]["legalities"]["commander"], "legal");
        assert_eq!(
            record["source"]["oracle_text"],
            format!("Enchant creature\nYou control enchanted creature.\n{text}")
        );
        let d = lowered(raw);
        assert_eq!(d.name, name);
        assert_eq!(d.oracle_id, oracle);
        assert_eq!(d.provenance.source_index, index);
        assert_eq!(d.provenance.source_sha256, source_hash);
        assert_eq!(d.provenance.ast_sha256, ast_hash);
        assert_eq!(d.provenance.binding_sha256, binding_hash);
        assert_eq!(
            d.characteristics.mana_cost,
            ManaCost {
                generic: 5.into(),
                colored: colors
            }
        );
        assert_eq!(
            strict_read::<Value>(&d.provenance.abilities_json).unwrap(),
            record["faces"][0]["result"]["abilities"]
        );
        assert_eq!(d.capabilities, plain.capabilities);
        assert_eq!(d.spell, plain.spell);
        assert!(aura_control_source(&d));
        assert_eq!(
            d.static_abilities,
            vec![
                plain.static_abilities[0].clone(),
                StaticAbility {
                    functioning_zone: FunctioningZone::Battlefield,
                    effect: StaticEffect::ModifyCreatures {
                        scope: CreatureScope::Enchanted,
                        power: 2.into(),
                        toughness: 2.into(),
                        keywords: vec![keyword],
                        restrictions: vec![]
                    }
                },
            ]
        );
        let refs = record["binding"]["references"].as_array().unwrap();
        for block in [1, 2] {
            assert!(refs.iter().any(|r| r["path"]
                == format!("/faces/0/abilities/{block}/effect/what")
                && r["expression"] == json!({"ref":"enchanted","noun":"creature"})
                && r["binding"] == "bound"));
        }
    }
}

fn signed(value: i64) -> Value {
    json!({"sign":if value < 0 { -1 } else { 1 },"amount":{"amount":"fixed","value":value.abs()}})
}
fn modifier_parameters(power: i64, toughness: i64, keywords: &[&str]) -> Vec<Value> {
    let mut modifiers =
        vec![json!({"modifier":"pt","power":signed(power),"toughness":signed(toughness)})];
    if !keywords.is_empty() {
        modifiers.push(
            json!({"modifier":"abilities","abilities":keywords.iter().map(|k|
            json!({"gained":"keyword","keyword":{"keyword":k}})).collect::<Vec<_>>()}),
        );
    }
    modifiers
}

#[test]
fn aura_control_composes_parameterized_typed_modifiers_and_ignores_source_identity() {
    for (power, toughness, keywords) in [
        (7, 3, vec!["reach", "hexproof"]),
        (0, 4, vec![]),
        (0, 0, vec!["haste"]),
        (1, 0, vec!["first strike", "lifelink"]),
    ] {
        let raw = changed_from(SPIRIT, |r| {
            r["oracleIdentity"] = json!("synthetic-composed-oracle");
            r["source"]["oracle_id"] = json!("synthetic-composed-oracle");
            r["source"]["id"] = json!("synthetic-composed-printing");
            r["source"]["name"] = json!("Synthetic Composed Aura");
            r["source"]["mana_cost"] = json!("{9}{W}");
            r["source"]["colors"] = json!(["W"]);
            r["source"]["color_identity"] = json!(["W"]);
            r["faces"][0]["result"]["name"] = json!("Synthetic Composed Aura");
            r["binding"] = json!({});
            let grant = if keywords.is_empty() {
                String::new()
            } else {
                format!(" and has {}", keywords.join(" and "))
            };
            let text = format!("Enchanted creature gets +{power}/+{toughness}{grant}.");
            r["source"]["oracle_text"] = json!(format!(
                "Enchant creature\nYou control enchanted creature.\n{text}"
            ));
            r["faces"][0]["result"]["lines"][2]["text"] = json!(text);
            r["faces"][0]["result"]["abilities"][2]["effect"]["modifiers"] =
                json!(modifier_parameters(power, toughness, &keywords));
        });
        let d = lowered(&raw);
        assert!(aura_control_source(&d));
        assert_eq!(d.oracle_id, "synthetic-composed-oracle");
        let expected_keywords: Vec<KeywordAbility> = keywords
            .iter()
            .map(|k| serde_json::from_value(json!(k)).unwrap())
            .collect();
        assert_eq!(
            d.static_abilities[1].effect,
            StaticEffect::ModifyCreatures {
                scope: CreatureScope::Enchanted,
                power: power.into(),
                toughness: toughness.into(),
                keywords: expected_keywords,
                restrictions: vec![],
            }
        );
        assert_eq!(d.capabilities, lowered(MIND).capabilities);
    }
}

#[test]
fn aura_control_composition_rejects_whole_source_and_modifier_boundary_negatives() {
    for modifiers in [
        vec![],
        modifier_parameters(0, 0, &[]),
        modifier_parameters(-1, 2, &["flying"]),
        modifier_parameters(2, -1, &[]),
        modifier_parameters(0, 0, &["indestructible"]),
        modifier_parameters(2, 2, &["flash"]),
        modifier_parameters(2, 2, &["flying", "flying"]),
        vec![json!({"modifier":"abilities","abilities":[]})],
        vec![json!({"modifier":"abilities","abilities":[{"gained":"remove-all"}]})],
        vec![json!({"modifier":"set-pt","power":2,"toughness":2})],
        vec![json!({"modifier":"cant","action":"activate"})],
        vec![
            json!({"modifier":"pt","power":{"sign":1,"amount":{"amount":"variable","name":"x"}},"toughness":signed(2)}),
        ],
        vec![
            json!({"modifier":"pt","power":{"sign":0,"amount":{"amount":"fixed","value":2}},"toughness":signed(2)}),
        ],
        vec![
            modifier_parameters(2, 2, &[])[0].clone(),
            modifier_parameters(1, 1, &[])[0].clone(),
        ],
    ] {
        rejected(&changed_from(SPIRIT, |r| {
            r["faces"][0]["result"]["abilities"][2]["effect"]["modifiers"] = json!(modifiers)
        }));
    }
    for what in [
        json!({"ref":"equipped","noun":"creature"}),
        json!({"ref":"enchanted","noun":"permanent"}),
        json!({"ref":"this","noun":"creature"}),
        json!({"ref":"each","filter":{"types":["creature"],"control":"you"}}),
        json!({"ref":"enchanted","noun":"creature","selectionId":"other"}),
        json!({"ref":"enchanted","noun":"creature","filter":{"control":"you"}}),
    ] {
        rejected_complete_source(&changed_from(SPIRIT, |r| {
            r["faces"][0]["result"]["abilities"][2]["effect"]["what"] = what
        }));
    }
    for (field, value) in [
        ("duration", json!({"duration":"until-end-of-turn"})),
        ("condition", json!({"condition":"tapped"})),
        ("bindings", json!([])),
    ] {
        rejected(&changed_from(SPIRIT, |r| {
            r["faces"][0]["result"]["abilities"][2]["effect"][field] = value
        }));
    }
    for block in [1, 2] {
        rejected_complete_source(&changed_from(SPIRIT, |r| {
            let result = &mut r["faces"][0]["result"];
            let ability = result["abilities"][block].clone();
            let line = result["lines"][block].clone();
            result["abilities"].as_array_mut().unwrap().push(ability);
            result["lines"].as_array_mut().unwrap().push(line);
            r["source"]["oracle_text"] = json!(format!(
                "{}\n{}",
                r["source"]["oracle_text"].as_str().unwrap(),
                r["faces"][0]["result"]["lines"][block]["text"]
                    .as_str()
                    .unwrap()
            ));
        }));
    }
}
