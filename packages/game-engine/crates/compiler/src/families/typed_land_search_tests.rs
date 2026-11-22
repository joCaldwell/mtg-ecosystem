use super::*;
use crate::tests::changed_source;
const MANIFEST: &str =
    include_str!("../../../../tests/fixtures/ramos/archive-manifest-current.json");
const SOURCES: [(&str, &str, i64, bool); 3] = [
    (
        "Nature's Lore",
        include_str!("../../../../tests/fixtures/ramos/78826359-fe63-44ad-adc4-a17ffcd710e4.json"),
        1,
        false,
    ),
    (
        "Farseek",
        include_str!("../../../../tests/fixtures/ramos/495e52e6-4c2b-4574-9474-eadbdcc8b4ac.json"),
        1,
        true,
    ),
    (
        "Skyshroud Claim",
        include_str!("../../../../tests/fixtures/ramos/376c9d3f-21d3-4251-bb6a-026fa9e1b0e1.json"),
        2,
        false,
    ),
];
fn changed(raw: &str, f: impl FnOnce(&mut Value)) -> String {
    changed_source(raw, |r| {
        let result = &mut r["faces"][0]["result"];
        f(&mut result["abilities"][0]["effects"][0]["steps"]);
        result["lines"][0]["ability"] = result["abilities"][0].clone();
    })
}
#[test]
fn typed_land_search_complete_frozen_sources_and_generic_quantity() {
    let mut definitions = vec![];
    for (name, raw, maximum, force_tapped) in SOURCES {
        let CompileResult::Lowered { definition } = compile(MANIFEST, raw) else {
            panic!("{name}: {:?}", compile(MANIFEST, raw))
        };
        definition.validate().unwrap();
        let [
            Instruction::SearchTypedLands {
                search,
                source_block: 0,
            },
        ] = definition.spell.as_ref().unwrap().instructions.as_slice()
        else {
            panic!("typed search")
        };
        assert_eq!(search.maximum, maximum.into());
        assert_eq!(search.force_tapped, force_tapped);
        assert_eq!(
            search.any_basic_land_types,
            if name == "Farseek" {
                vec![
                    BasicLandType::Plains,
                    BasicLandType::Island,
                    BasicLandType::Swamp,
                    BasicLandType::Mountain,
                ]
            } else {
                vec![BasicLandType::Forest]
            }
        );
        assert!(
            definition
                .capabilities
                .contains(&Capability::TypedLandSearchV1)
        );
        assert!(
            !definition
                .capabilities
                .contains(&Capability::BasicLandSearchV1)
        );
        let renamed = changed_source(raw, |r| {
            r["source"]["name"] = "Generic typed search".into();
            r["faces"][0]["result"]["name"] = "Generic typed search".into();
        });
        assert!(matches!(
            compile(MANIFEST, &renamed),
            CompileResult::Lowered { .. }
        ));
        definitions.push(*definition);
    }
    // Synthetic AST quantities over a complete source, not additional paid sources.
    for maximum in [3, u32::MAX] {
        let raw = changed(SOURCES[2].1, |s| {
            s[0]["for"]["count"]["value"] = maximum.into()
        });
        let CompileResult::Lowered { definition } = compile(MANIFEST, &raw) else {
            panic!("generic count {maximum}")
        };
        assert!(
            matches!(&definition.spell.unwrap().instructions[0],Instruction::SearchTypedLands {search,..} if search.maximum == Quantity::from(i64::from(maximum)))
        );
    }
    if let Some(directory) = std::env::var_os("MTG_TYPED_LAND_SEARCH_EXPORT_DIR") {
        // Caller owns a fresh ignored evidence directory. Never replace evidence.
        use std::io::Write;
        definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
        let catalog = Catalog {
            schema_version: CURRENT_IR_VERSION,
            rules_edition: RULES_EDITION.into(),
            rules_sha256: RULES_SHA256.into(),
            definitions,
        };
        catalog.validate().unwrap();
        for (name, value) in [
            ("catalog.json", serde_json::to_value(&catalog).unwrap()),
            ("schema.json", catalog_schema()),
        ] {
            let path = std::path::Path::new(&directory).join(name);
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .unwrap();
            file.write_all(serde_json::to_string_pretty(&value).unwrap().as_bytes())
                .unwrap();
        }
    }
}
#[test]
fn typed_land_search_rehashed_near_misses_reject_complete_sequence() {
    for (_, raw, _, _) in SOURCES {
        for (pointer, value) in [
            ("/0/who/player", json!("opponent")),
            ("/0/who/selectionId", json!("qualified")),
            ("/0/who/of", json!({"ref":"it"})),
            ("/0/zones/of/0/owner", json!("opponent")),
            ("/0/zones/selection", json!("one")),
            ("/0/for/ref", json!("that")),
            ("/0/for/unknown", json!(true)),
            ("/0/for/upTo", json!(false)),
            ("/0/for/upTo", json!(null)),
            ("/0/for/count/value", json!(0)),
            ("/0/for/count/amount", json!("x")),
            ("/0/for/count/unknown", json!(true)),
            ("/0/for/filter/cls", json!("permanent")),
            ("/0/for/filter/types", json!(["land"])),
            ("/0/for/filter/subtypes", json!([])),
            ("/0/for/filter/supertypes", json!(["basic"])),
            ("/0/for/filter/unknown", json!(true)),
            ("/1/what/ref", json!("rest")),
            ("/1/what/selectionId", json!("foreign")),
            ("/1/to/zone", json!("hand")),
            ("/1/to/controller", json!("opponent")),
            ("/1/tapped", json!(false)),
            ("/1/order", json!("any")),
            ("/1/order", json!(null)),
            ("/2/optional", json!(true)),
            ("/2/optional", json!(false)),
            ("/2/optional", json!(null)),
            ("/2/who/player", json!("opponent")),
            ("/2/who/of", json!({"ref":"it"})),
            ("/2/who/selectionId", json!("qualified")),
            ("/2/library/owner/player", json!("opponent")),
            ("/2/library/unknown", json!(true)),
        ] {
            let bad = changed(raw, |s| {
                let (parent, key) = pointer.rsplit_once('/').unwrap();
                let at = s.pointer_mut(parent).unwrap();
                if let Ok(i) = key.parse::<usize>() {
                    at[i] = value;
                } else {
                    at[key] = value;
                }
            });
            assert!(
                !matches!(compile(MANIFEST, &bad), CompileResult::Lowered { .. }),
                "{pointer}: {}",
                bad
            );
        }
        let bad = changed_source(raw, |r| {
            r["source"]["mana_cost"] = "{X}{G}".into();
        });
        assert!(!matches!(
            compile(MANIFEST, &bad),
            CompileResult::Lowered { .. }
        ));
        for remove in 0..3 {
            let bad = changed(raw, |s| {
                s.as_array_mut().unwrap().remove(remove);
            });
            assert!(!matches!(
                compile(MANIFEST, &bad),
                CompileResult::Lowered { .. }
            ));
        }
        for step in [
            json!({"effect":"draw","who":{"player":"you"},"count":{"amount":"fixed","value":1}}),
            json!({"effect":"reveal","who":{"player":"you"},"what":{"ref":"them"}}),
        ] {
            let bad = changed(raw, |s| s.as_array_mut().unwrap().push(step));
            assert!(!matches!(
                compile(MANIFEST, &bad),
                CompileResult::Lowered { .. }
            ));
        }
        let bad = changed_source(raw, |r| {
            let a = r["faces"][0]["result"]["abilities"][0].clone();
            r["faces"][0]["result"]["abilities"]
                .as_array_mut()
                .unwrap()
                .push(a);
        });
        assert!(!matches!(
            compile(MANIFEST, &bad),
            CompileResult::Lowered { .. }
        ));
    }
    for filter in [
        json!({"cls":"card","subtypes":["Desert"]}),
        json!({"cls":"card","subtypes":["Forest","Island"]}),
        json!({"cls":"card","allOf":[{"anyOf":[]}]}),
        json!({"cls":"card","allOf":[{"anyOf":[{"subtypes":["Forest"]},{"subtypes":["Forest"]}]}]}),
        json!({"cls":"card","allOf":[{"anyOf":[{"subtypes":["Forest"],"unknown":true}]}]}),
    ] {
        let bad = changed(SOURCES[0].1, |s| s[0]["for"]["filter"] = filter);
        assert!(!matches!(
            compile(MANIFEST, &bad),
            CompileResult::Lowered { .. }
        ));
    }
}

#[test]
fn typed_land_search_rejects_explicit_null_actor_qualifiers() {
    for (_, raw, _, _) in SOURCES {
        for actor in [0, 2] {
            for qualifier in ["selectionId", "of"] {
                let bad = changed(raw, |steps| steps[actor]["who"][qualifier] = Value::Null);
                assert!(
                    matches!(compile(MANIFEST, &bad), CompileResult::Unsupported { .. }),
                    "actor {actor}/{qualifier} accepted"
                );
            }
        }
    }
}
