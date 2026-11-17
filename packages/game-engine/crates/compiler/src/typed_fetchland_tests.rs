//! Exact frozen witnesses and synthetic rejection/parameter probes; no admission.
use super::*;
const MANIFEST: &str = include_str!("../../../tests/fixtures/ramos/archive-manifest-current.json");
const SOURCES: [(&str, &str, &str); 2] = [
    (include_str!("../../../tests/fixtures/ramos/ef86989d-ce80-4e55-aece-7d11710eeffa.json"), "e5f9a96d1ef5551541f9fc369f39c4b818cf8bd2a1ebbe512a581598acef3ebe", "b44b80867057843807ad9015744046cd983737a36d022293741e2da7ee847308"),
    (include_str!("../../../tests/fixtures/ramos/29737a60-3ebd-40d9-b935-c4f54b90d45d.json"), "bd3cc8af945cb2ad1a404787bf9ff76da9f51bd2d76a0675aab5594650747339", "f139d2389106e667593e55ca8c58722028c447a1d1e322f9a438a74357aedd36"),
];
fn lowered(raw: &str) -> Definition {
    match compile(MANIFEST, raw) {
        CompileResult::Lowered { definition } => *definition,
        other => panic!("{other:?}"),
    }
}
fn changed(raw: &str, change: impl FnOnce(&mut Value)) -> String {
    crate::tests::changed_source(raw, |r| {
        change(r);
        r["faces"][0]["result"]["lines"][0]["ability"] = r["faces"][0]["result"]["abilities"][0].clone();
    })
}
#[test]
fn typed_fetchland_complete_frozen_sources_and_exact_cost_export() {
    let mut definitions = vec![];
    for (i, (raw, file, ast)) in SOURCES.into_iter().enumerate() {
        assert_eq!(sha256(raw.as_bytes()), file);
        let r: Value = strict_read(raw).unwrap();
        assert_eq!(r["astSha256"], ast);
        let d = lowered(raw);
        d.validate().unwrap();
        assert_eq!(d.activated.len(), 1);
        assert_eq!(d.activated[0].cost, ActivationCost::TapPayLifeSacrificeSource { life: 1.into() });
        let [Instruction::SearchTypedLands { search, source_block: 0 }] = d.activated[0].program.instructions.as_slice() else { panic!("complete typed search"); };
        assert_eq!(search.maximum, 1.into());
        assert!(!search.force_tapped);
        assert_eq!(search.any_basic_land_types, if i == 0 { vec![BasicLandType::Island, BasicLandType::Swamp] } else { vec![BasicLandType::Plains, BasicLandType::Forest] });
        assert_eq!(d.capabilities, BTreeSet::from([
            Capability::NormalLayoutV1, Capability::LandPlayV1, Capability::ActivatedAbilityV1,
            Capability::PayLifeCostV1, Capability::TapSourceCostV1,
            Capability::SacrificeSourceCostV1, Capability::TypedLandSearchV1,
        ]));
        assert!(d.spell.is_none() && d.mana_abilities.is_empty());
        let renamed = changed(raw, |r| {
            r["source"]["name"] = json!("Generic typed search activation");
            r["faces"][0]["result"]["name"] = r["source"]["name"].clone();
            r["binding"]["nodes"][1]["name"] = r["source"]["name"].clone();
        });
        lowered(&renamed).validate().unwrap();
        definitions.push(d);
    }
    if let Some(directory) = std::env::var_os("MTG_TYPED_FETCHLAND_EXPORT_DIR") {
        use std::io::Write;
        definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
        let catalog = Catalog { schema_version: CURRENT_IR_VERSION, rules_edition: RULES_EDITION.into(), rules_sha256: RULES_SHA256.into(), definitions };
        catalog.validate().unwrap();
        for (name, value) in [("catalog.json", serde_json::to_value(catalog).unwrap()), ("schema.json", catalog_schema())] {
            let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(std::path::Path::new(&directory).join(name)).unwrap();
            file.write_all(serde_json::to_string_pretty(&value).unwrap().as_bytes()).unwrap();
        }
    }
}
#[test]
fn typed_fetchland_exact_u64_source_and_arbitrary_ir_quantity() {
    for value in [0, 1, 2, u64::from(u32::MAX) + 1, u64::MAX] {
        let raw = changed(SOURCES[0].0, |r| r["faces"][0]["result"]["abilities"][0]["costs"][1]["amount"]["value"] = json!(value));
        assert_eq!(lowered(&raw).activated[0].cost, ActivationCost::TapPayLifeSacrificeSource { life: value.to_string().parse().unwrap() });
    }
    for amount in [json!(null), json!({"amount":"fixed","value":null}), json!({"amount":"fixed","value":-1}), json!({"amount":"fixed","value":1.0}), json!({"amount":"fixed","value":"1"}), json!({"amount":"fixed","value":1e80}), json!({"amount":"fixed","value":1,"unknown":true}), json!({"amount":"x","value":1})] {
        let raw = changed(SOURCES[0].0, |r| r["faces"][0]["result"]["abilities"][0]["costs"][1]["amount"] = amount);
        assert!(!matches!(compile(MANIFEST, &raw), CompileResult::Lowered { .. }));
    }
    // Same complete finite family with different search bounds; not printed-one dispatch.
    for maximum in [3, u32::MAX] {
        let raw = changed(SOURCES[0].0, |r| {
            let steps = &mut r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"];
            steps[0]["for"]["count"]["value"] = json!(maximum);
            steps[0]["for"]["upTo"] = json!(true);
            steps[1]["what"] = json!({"ref":"them"});
            let find = steps[0]["for"].clone();
            for n in r["binding"]["nodes"].as_array_mut().unwrap() {
                if n["node"] == "selection" { n["expression"] = find.clone(); n["cardinality"] = json!("many"); }
                if n["result"] == "objects" { n["cardinality"] = json!("many"); }
            }
            for reference in r["binding"]["references"].as_array_mut().unwrap() {
                if reference["path"].as_str().unwrap().ends_with("/for") { reference["expression"] = find.clone(); }
                if reference["path"].as_str().unwrap().ends_with("/steps/1/what") { reference["expression"] = json!({"ref":"them"}); }
            }
        });
        let d = lowered(&raw);
        assert!(matches!(&d.activated[0].program.instructions[0], Instruction::SearchTypedLands { search, .. } if search.maximum == Quantity::from(i64::from(maximum))));
    }
    let mut d = lowered(SOURCES[0].0);
    d.activated[0].cost = ActivationCost::TapPayLifeSacrificeSource { life: "999999999999999999999999999999999999999".parse().unwrap() };
    d.validate().unwrap();
}
#[test]
fn typed_fetchland_complete_cost_effect_and_binding_near_misses_reject() {
    for (raw, _, _) in SOURCES {
        for change in 0..16 {
            let bad = changed(raw, |r| {
                let a = &mut r["faces"][0]["result"]["abilities"][0];
                match change {
                    0 => { a["costs"].as_array_mut().unwrap().remove(0); }
                    1 => { a["costs"].as_array_mut().unwrap().remove(1); }
                    2 => a["costs"][2]["what"]["noun"] = json!("creature"),
                    3 => a["costs"].as_array_mut().unwrap().swap(0, 1),
                    4 => a["costs"][0]["unknown"] = json!(true),
                    5 => a["restriction"] = json!(null),
                    6 => a["effects"][0]["condition"] = json!(null),
                    7 => a["effects"][0]["steps"][0]["who"]["of"] = json!(null),
                    8 => a["effects"][0]["steps"][1]["what"] = json!({"ref":"this","noun":"land"}),
                    9 => a["effects"][0]["steps"][1]["order"] = json!(null),
                    10 => a["effects"][0]["steps"][2]["optional"] = json!(null),
                    11 => { a["effects"][0]["steps"].as_array_mut().unwrap().remove(2); }
                    12 => a["effects"][0]["steps"][0]["for"]["filter"]["supertypes"] = json!(["basic"]),
                    13 => { let extra = a["effects"][0].clone(); a["effects"].as_array_mut().unwrap().push(extra); }
                    14 => r["binding"]["references"][2]["target"] = json!("foreign-payer"),
                    _ => { let extra = r["binding"]["references"][0].clone(); r["binding"]["references"].as_array_mut().unwrap().push(extra); }
                }
            });
            assert!(!matches!(compile(MANIFEST, &bad), CompileResult::Lowered { .. }), "mutation {change}");
        }
        // Each binding node and obligation is consumed, including cost payer.
        let value: Value = strict_read(raw).unwrap();
        for array in ["nodes", "references"] {
            for index in 0..value["binding"][array].as_array().unwrap().len() {
                let bad = changed(raw, |r| { r["binding"][array].as_array_mut().unwrap().remove(index); });
                assert!(!matches!(compile(MANIFEST, &bad), CompileResult::Lowered { .. }), "missing {array}/{index}");
            }
        }
    }
}
