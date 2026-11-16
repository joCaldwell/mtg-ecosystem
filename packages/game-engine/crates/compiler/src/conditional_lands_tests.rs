//! Frozen whole-card witnesses; mutations below are synthetic parameter/fence probes.
use super::*;
const MANIFEST: &str = include_str!("../../../tests/fixtures/ramos/archive-manifest-current.json");
const CINDER: &str =
    include_str!("../../../tests/fixtures/ramos/dfac0258-e148-4d7d-8ded-fc2466d9caa6.json");
const PRAIRIE: &str = include_str!("../../../tests/fixtures/conditional-land/prairie-stream.json");
#[test]
fn conditional_lands_complete_frozen_sources_and_parameters() {
    let mut definitions = vec![];
    for (raw, hash, subtypes) in [
        (
            CINDER,
            "ec6d7141301bd3e491a5c87f36524a5240807eae6e9ab9e361993a7bbc289058",
            vec!["Mountain", "Forest"],
        ),
        (
            PRAIRIE,
            "5a6f4d91cc8a4021e03b03895e91fe91df8eb7a224e02e9c199ea07af4aae743",
            vec!["Plains", "Island"],
        ),
    ] {
        assert_eq!(sha256(raw.as_bytes()), hash);
        let CompileResult::Lowered { definition: d } = compile(MANIFEST, raw) else {
            panic!("{:?}", compile(MANIFEST, raw));
        };
        definitions.push(*d.clone());
        d.validate().unwrap();
        assert_eq!(d.characteristics.subtypes, subtypes);
        assert_eq!(
            d.static_abilities,
            [StaticAbility {
                functioning_zone: FunctioningZone::Battlefield,
                effect: StaticEffect::EntersTappedUnlessControllerBasicLands { minimum: 2.into() }
            }]
        );
        assert!(d.mana_abilities.is_empty());
        assert_eq!(
            d.capabilities,
            BTreeSet::from([
                Capability::NormalLayoutV1,
                Capability::LandPlayV1,
                Capability::TypedNonbasicIntrinsicManaV1,
                Capability::ConditionalBasicLandEntryV1
            ])
        );
        for n in [0u64, 1, 3, u64::MAX] {
            let changed = crate::tests::changed_source(raw, |r| {
                let result = &mut r["faces"][0]["result"];
                result["abilities"][0]["effect"]["condition"]["comparison"]["value"]["value"] =
                    json!(n);
                result["lines"][0]["ability"] = result["abilities"][0].clone();
            });
            let CompileResult::Lowered { definition: d } = compile(MANIFEST, &changed) else {
                panic!("parameter {n}: {:?}", compile(MANIFEST, &changed));
            };
            assert_eq!(
                d.static_abilities[0].effect,
                StaticEffect::EntersTappedUnlessControllerBasicLands {
                    minimum: n.to_string().parse().unwrap()
                }
            );
        }
        let renamed = crate::tests::changed_source(raw, |r| {
            r["source"]["name"] = json!("Generic conditional land");
            r["faces"][0]["result"]["name"] = json!("Generic conditional land");
            r["binding"]["nodes"][1]["name"] = json!("Generic conditional land");
        });
        assert!(matches!(
            compile(MANIFEST, &renamed),
            CompileResult::Lowered { .. }
        ));
    }

    if let Some(directory) = std::env::var_os("MTG_CONDITIONAL_LANDS_EXPORT_DIR") {
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
            ("catalog.json", serde_json::to_value(catalog).unwrap()),
            ("schema.json", catalog_schema()),
        ] {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(std::path::Path::new(&directory).join(name))
                .unwrap();
            file.write_all(serde_json::to_string_pretty(&value).unwrap().as_bytes())
                .unwrap();
        }
    }
}
#[test]
fn conditional_lands_whole_ast_predicate_binding_null_and_extra_rejections() {
    for (path, value) in [
        ("/conditionKind", json!("if")),
        ("/conditionKind", json!(null)),
        ("/condition", json!(null)),
        ("/condition/who", json!({"player":"opponent"})),
        ("/condition/who/extra", json!(true)),
        ("/condition/of", json!({"types":["land"]})),
        ("/condition/of/subtypes", json!(["Forest"])),
        ("/condition/of/status", json!(["tapped"])),
        ("/condition/comparison/op", json!("gt")),
        ("/condition/comparison/value/value", json!(-1)),
        ("/condition/comparison/value/value", json!(2.5)),
        ("/condition/comparison/value/value", json!("2")),
        ("/condition/comparison/value/extra", json!(true)),
        ("/what", json!({"ref":"each","noun":"land"})),
        ("/extra", json!(true)),
    ] {
        let changed = crate::tests::changed_source(CINDER, |r| {
            let a = &mut r["faces"][0]["result"]["abilities"][0]["effect"];
            let (parent, key) = path.rsplit_once('/').unwrap();
            a.pointer_mut(parent)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(key.into(), value.clone());
            r["faces"][0]["result"]["lines"][0]["ability"] =
                r["faces"][0]["result"]["abilities"][0].clone();
        });
        assert!(
            !matches!(compile(MANIFEST, &changed), CompileResult::Lowered { .. }),
            "{path}: {value}"
        );
    }
    for (path, value) in [
        ("/binding/nodes/0/identity", json!("foreign")),
        ("/binding/nodes/1/typeLine", json!("Land")),
        ("/binding/nodes/2/you", json!("foreign")),
        ("/binding/nodes/3/role", json!("owner")),
        ("/binding/nodes/4/role", json!("static-source-controller")),
        ("/binding/nodes/4/scope", json!("foreign")),
        ("/binding/references/1/target", json!("foreign")),
        ("/binding/references/0/binding", json!("unresolved")),
        ("/binding/fullyBound", json!(false)),
    ] {
        let changed =
            crate::tests::changed_source(CINDER, |r| *r.pointer_mut(path).unwrap() = value);
        assert!(
            !matches!(compile(MANIFEST, &changed), CompileResult::Lowered { .. }),
            "{path}"
        );
    }
    for raw in [
        include_str!("../../../tests/fixtures/ramos/16052b52-ade1-406f-a06b-ce7ea607fb63.json"),
        include_str!("../../../tests/fixtures/ramos/975ec9a3-6f20-4177-8211-82526e092538.json"),
    ] {
        assert!(matches!(
            compile(MANIFEST, raw),
            CompileResult::Unsupported { .. }
        ));
    }
}
