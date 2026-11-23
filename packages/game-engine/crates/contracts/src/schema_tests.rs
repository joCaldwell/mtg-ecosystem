//! The published Catalog JSON Schema (`catalog_schema`) agrees with strict
//! decoding: KIR nodes are accepted, retired instruction tags and every
//! malformed shape below are rejected, and only IR schema 1 is admitted.
//! Ported from the retired Node contract tests.
use super::*;
use serde_json::{Value, json};

fn validator() -> jsonschema::Validator {
    jsonschema::draft202012::options()
        .build(&catalog_schema())
        .unwrap()
}
fn fixture(raw: &str) -> Value {
    serde_json::from_str(raw).unwrap()
}
fn base() -> Value {
    fixture(include_str!("../../../tests/fixtures/expected-ir.json"))
}
fn spell_definition(catalog: &mut Value, kind: Option<&str>) -> usize {
    catalog["definitions"]
        .as_array()
        .unwrap()
        .iter()
        .position(|d| {
            !d["spell"].is_null() && kind.is_none_or(|kind| d["characteristics"]["kind"] == kind)
        })
        .unwrap()
}

#[test]
fn instruction_schema_accepts_kir_nodes_and_rejects_retired_instruction_tags() {
    let validate = validator();
    let mut catalog = fixture(include_str!(
        "../../../tests/fixtures/expected-action-ir.json"
    ));
    let index = catalog["definitions"]
        .as_array()
        .unwrap()
        .iter()
        .position(|d| d["name"] == "Cruel Edict")
        .unwrap();
    let instructions = &mut catalog["definitions"][index]["spell"]["instructions"];
    let leaf = instructions[0].clone();
    let mut sequence = leaf.clone();
    sequence["node"] = json!({"seq": [leaf["node"], leaf["node"]]});
    *instructions = json!([sequence]);
    assert!(validate.is_valid(&catalog));
    catalog["definitions"][index]["spell"]["instructions"] =
        json!([{"op": "scry", "count": "1", "source_block": 0}]);
    assert!(!validate.is_valid(&catalog));
}

#[test]
fn card_ir_accepts_only_schema_one() {
    let validate = validator();
    let catalog = base();
    assert_eq!(catalog["schema_version"], CURRENT_IR_VERSION);
    assert!(validate.is_valid(&catalog));
    for version in [json!(0), json!(2), json!(33), json!("1"), Value::Null] {
        let mut bad = catalog.clone();
        bad["schema_version"] = version;
        assert!(!validate.is_valid(&bad));
    }
    let mut missing = catalog;
    missing.as_object_mut().unwrap().remove("schema_version");
    assert!(!validate.is_valid(&missing));
}

#[test]
fn schema_rejects_unknown_and_malformed_semantic_fields() {
    let validate = validator();
    let catalog = base();
    let mutations: [fn(&mut Value); 9] = [
        |c| c["unknownSemanticField"] = json!(true),
        |c| {
            let i = spell_definition(c, None);
            c["definitions"][i]["spell"]
                .as_object_mut()
                .unwrap()
                .remove("timing");
        },
        |c| {
            let i = spell_definition(c, None);
            c["definitions"][i]["spell"]
                .as_object_mut()
                .unwrap()
                .remove("resolution");
        },
        |c| {
            let i = spell_definition(c, None);
            c["definitions"][i]["spell"]["timing"] = json!("anytime");
        },
        |c| {
            let i = spell_definition(c, None);
            c["definitions"][i]["spell"]["resolution"] = json!("execute_text");
        },
        |c| c["definitions"][0]["characteristics"]["power"] = json!(2),
        |c| c["definitions"][0]["characteristics"]["power"] = json!("02"),
        |c| {
            let i = spell_definition(c, Some("instant"));
            c["definitions"][i]["spell"]["instructions"][0]["op"] = json!("execute_text");
        },
        |c| {
            let i = spell_definition(c, Some("instant"));
            c["definitions"][i]["spell"]["instructions"][0]["extra"] = json!(true);
        },
    ];
    for mutate in mutations {
        let mut bad = catalog.clone();
        mutate(&mut bad);
        assert!(!validate.is_valid(&bad), "{bad}");
    }
}

#[test]
fn schema_rejects_object_shaped_keywords() {
    let validate = validator();
    let catalog = fixture(include_str!(
        "../../../tests/fixtures/expected-match-ir.json"
    ));
    assert!(validate.is_valid(&catalog));
    let wall = catalog["definitions"]
        .as_array()
        .unwrap()
        .iter()
        .position(|d| d["name"] == "Wall of Wood")
        .unwrap();
    for keywords in [
        json!([{"keyword": "defender"}]),
        json!([{"defender": null}]),
    ] {
        let mut bad = catalog.clone();
        bad["definitions"][wall]["keywords"] = keywords;
        assert!(!validate.is_valid(&bad));
    }
}
