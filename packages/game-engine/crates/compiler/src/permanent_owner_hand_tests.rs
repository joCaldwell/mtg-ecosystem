//! Complete permanent-bounce sources and synthetic complete-consumption probes.
use super::*;
use serde_json::{Value, json};

const MANIFEST: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
const REGRESS: &str =
    include_str!("../../../tests/fixtures/2fa763ad-4d94-4c3a-a099-13ac22c09ce4.json");
const DISPERSE: &str =
    include_str!("../../../tests/fixtures/9d8fe3f8-5027-4f9e-999e-e277caa96478.json");

fn changed(text: &str, mutate: impl FnOnce(&mut Value)) -> String {
    tests::changed_source(REGRESS, |record| {
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
fn complete_regress_and_disperse_preserve_permanent_predicate_owner_destination_and_pins() {
    for (raw, name, raw_hash, index, generic, source_hash, ast_hash, binding_hash, non_types) in [
        (
            REGRESS,
            "Regress",
            "4f7c2a332010afa7de1728a26948f4297c9e5b285e4ce15557df5a09281807c1",
            7095,
            2,
            "eeb15b5a2bad7b0533627f6c44fd7e0ba608aa4f8474313e19fc0fcec46a290f",
            "1da067ce1046d7b3d008d02cdf3994a9a78a5f6bc55a2cc6559a8fe7c7e280c1",
            "9f515633ad2f44c0548dbc1fd2bd9101ccdcce0d84aa7eed5ff2f1a06aebabd5",
            vec![],
        ),
        (
            DISPERSE,
            "Disperse",
            "8ed164d05b7495bb4472f6e98ad1dd1429f84faedcecd1036b32e7ab6411ba62",
            23756,
            1,
            "4af37f5be33ff53776983580bca31202b9f4c32ea5a3c38b5d41f3dda3b8145f",
            "183f75204d24677bea7d64c7d450b5bbfe16e0df3c2d4c75b0b11aef65918725",
            "fa88842d9873a69da27f98b404d4e9fe233e887f16fd56164559d7ad7bf733bd",
            vec![CardKind::Land],
        ),
    ] {
        assert_eq!(sha256(raw.trim_end().as_bytes()), raw_hash);
        let record: Value = strict_read(raw).unwrap();
        let result = compile(MANIFEST, raw);
        let CompileResult::Lowered { definition: d } = result else {
            panic!("{name}: {result:?}")
        };
        assert_eq!(d.name, name);
        assert_eq!(d.oracle_id, record["oracleIdentity"].as_str().unwrap());
        assert_eq!(d.characteristics.kind, CardKind::Instant);
        assert_eq!(
            d.characteristics.mana_cost,
            ManaCost {
                generic: Quantity::from(generic),
                colored: vec![Color::U]
            }
        );
        assert_eq!(d.provenance.source_index, index);
        assert_eq!(d.provenance.source_sha256, source_hash);
        assert_eq!(d.provenance.ast_sha256, ast_hash);
        assert_eq!(d.provenance.binding_sha256, binding_hash);
        assert_eq!(
            strict_read::<Value>(&d.provenance.source_json).unwrap(),
            record["source"]
        );
        assert_eq!(
            strict_read::<Value>(&d.provenance.abilities_json).unwrap(),
            record["faces"][0]["result"]["abilities"]
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
                        non_types,
                        non_colors: vec![],
                        opponent: false,
                        tapped: false
                    })
                }],
                instructions: vec![Instruction::ReturnToOwnerHand {
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
                Capability::OwnerHandMoveV1,
                Capability::PermanentOwnerHandMoveV1
            ])
        );
        d.validate().unwrap();
    }
}

#[test]
fn permanent_bounce_rejects_unrepresented_selector_filter_and_destination_terms() {
    for (field, value) in [
        ("count", json!({"amount":"fixed","value":0})),
        ("count", json!({"amount":"fixed","value":2})),
        ("count", json!({"amount":"x"})),
        ("ref", json!("filter")),
        ("selectionId", json!("")),
        ("of", json!({"ref":"this","noun":"permanent"})),
        ("upTo", json!(true)),
    ] {
        let raw = changed("Synthetic permanent-bounce selector.", |r| {
            r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["what"][field] = value
        });
        let result = compile(MANIFEST, &raw);
        assert!(
            matches!(result, CompileResult::Unsupported { .. }),
            "{field}: {result:?}"
        );
    }
    for filter in [
        json!({"cls":"permanent","owner":"you"}),
        json!({"cls":"permanent","control":"you"}),
        json!({"cls":"permanent","control":"opponent"}),
        json!({"cls":"permanent","status":["tapped"]}),
        json!({"cls":"permanent","types":["artifact"]}),
        json!({"cls":"permanent","nonTypes":["artifact"]}),
        json!({"cls":"permanent","nonTypes":["land","creature"]}),
        json!({"cls":"permanent","nonColors":["black"]}),
        json!({"cls":"permanent","unknown":true}),
        Value::Null,
    ] {
        let raw = changed("Synthetic qualified permanent-bounce target.", |r| {
            r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["what"]["filter"] =
                filter
        });
        let result = compile(MANIFEST, &raw);
        assert!(
            matches!(result, CompileResult::Unsupported { .. }),
            "{result:?}"
        );
    }
    for to in [json!("controller"), json!("your"), json!("opponent")] {
        let raw = changed("Synthetic nonowner bounce destination.", |r| {
            r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["to"] = to
        });
        let result = compile(MANIFEST, &raw);
        assert!(
            matches!(result, CompileResult::Unsupported { .. }),
            "{result:?}"
        );
    }
    for optional in [true, false] {
        let raw = changed("Synthetic optional permanent-bounce instruction.", |r| {
            r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["optional"] =
                optional.into()
        });
        let result = compile(MANIFEST, &raw);
        assert!(
            matches!(result, CompileResult::Unsupported { .. }),
            "{result:?}"
        );
    }
}

#[test]
fn permanent_bounce_rejects_extra_effects_and_nonordinary_program_contexts() {
    for context in ["draw", "modal", "additional-cost", "activated", "triggered"] {
        let text = match context {
            "draw" => "Return target permanent to its owner's hand. Draw a card.",
            "modal" => "Choose one — return target permanent to its owner's hand; or draw a card.",
            "additional-cost" => {
                "As an additional cost to cast this spell, sacrifice a creature.\nReturn target permanent to its owner's hand."
            }
            "activated" => "{T}: Return target permanent to its owner's hand.",
            _ => "When this creature enters, return target permanent to its owner's hand.",
        };
        let raw = changed(text, |r| {
            let a = &mut r["faces"][0]["result"]["abilities"][0];
            match context {
                "draw" => {
                    a["effects"][0]["steps"].as_array_mut().unwrap().push(json!({"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1}}));
                }
                "modal" => {
                    let bounce = a["effects"].clone();
                    let draw = json!([{"steps":[{"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1}}]}]);
                    a["effects"] = json!([{"steps":[{"effect":"modal","count":{"exactly":1},"options":[{"effects":bounce},{"effects":draw}]}]}]);
                }
                "additional-cost" => {
                    r["faces"][0]["result"]["abilities"].as_array_mut().unwrap().insert(0, json!({"kind":"additional-cost","costs":[{"cost":"sacrifice","what":{"ref":"filter","filter":{"types":["creature"]},"count":{"amount":"fixed","value":1}}}]}));
                }
                _ => {
                    a["kind"] = context.into();
                    if context == "activated" {
                        a["costs"] = json!([{"cost":"tap-self"}]);
                    } else {
                        a["trigger"] =
                            json!({"trigger":"enters","what":{"ref":"this","noun":"creature"}});
                    }
                    r["source"]["type_line"] = "Creature — Human".into();
                    r["source"]["power"] = "1".into();
                    r["source"]["toughness"] = "1".into();
                }
            }
        });
        let result = compile(MANIFEST, &raw);
        assert!(
            matches!(result, CompileResult::Unsupported { .. }),
            "{context}: {result:?}"
        );
    }
}

#[test]
fn creature_owner_hand_and_owner_library_definitions_remain_exactly_unchanged() {
    let c = embedded_catalog("wipes").unwrap();
    for raw in [
        include_str!("../../../tests/fixtures/unsummon.json"),
        include_str!("../../../tests/fixtures/time-ebb.json"),
    ] {
        let result = compile(fixtures::MANIFEST, raw);
        let CompileResult::Lowered { definition } = result else {
            panic!("{result:?}")
        };
        assert_eq!(
            c.definitions
                .iter()
                .find(|d| d.oracle_id == definition.oracle_id),
            Some(definition.as_ref())
        );
        assert!(
            !definition
                .capabilities
                .contains(&Capability::PermanentOwnerHandMoveV1)
        );
    }
}
