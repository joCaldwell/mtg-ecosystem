//! Synthetic AST probes over complete retained Healing Salve source. These
//! are not parser-produced card records or evidence of real-card misexecution.
use super::*;
use serde_json::{Value, json};

const SALVE: &str = include_str!("../../../tests/fixtures/healing-salve.json");

fn changed_bindings(option: Option<usize>, bindings: Value) -> String {
    tests::changed_source(SALVE, |record| {
        let result = &mut record["faces"][0]["result"];
        let sentence = &mut result["abilities"][0]["effects"][0];
        if let Some(option) = option {
            sentence["steps"][0]["options"][option]["effects"][0]["bindings"] = bindings;
        } else {
            sentence["bindings"] = bindings;
        }
        result["lines"][0]["ability"] = result["abilities"][0].clone();
    })
}

fn lowered(raw: &str) -> Box<Definition> {
    let result = compile(fixtures::MANIFEST, raw);
    let CompileResult::Lowered { definition } = result else {
        panic!("complete Healing Salve must lower: {result:?}")
    };
    definition.validate().unwrap();
    definition
}

fn assert_binding_rejected(raw: &str, reason: &str) {
    let result = compile(fixtures::MANIFEST, raw);
    assert!(
        matches!(&result, CompileResult::Unsupported { path, reason: actual }
            if path == "bindings" && actual == reason),
        "executable sentence binding must not be erased: {result:?}"
    );
}

// Sentence.bindings in oracle-parser/src/ast.ts defines variable "x", an
// Amount value, and optional AnnouncementTime asOf. Both fixed and count
// amounts below satisfy that contract; these are not malformed/null fields.
fn fixed_binding() -> Value {
    json!({"variable":"x","value":{"amount":"fixed","value":3}})
}

fn count_binding() -> Value {
    // This exact binding is supported for an ordinary X-damage instruction,
    // but neither Healing Salve option consumes X.
    json!({"variable":"x","value":{"amount":"count","of":{
        "types":["creature"],"zone":{"zone":"battlefield"}
    }}})
}

#[test]
fn retained_healing_salve_matches_complete_frozen_ir() {
    let definition = lowered(SALVE);
    let catalog = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-choices-ir.json"
    ))
    .unwrap();
    assert_eq!(
        catalog
            .definitions
            .iter()
            .find(|d| d.oracle_id == definition.oracle_id),
        Some(definition.as_ref())
    );
    assert_eq!(definition.spell.as_ref().unwrap().modes.len(), 2);
}

#[test]
fn rehashed_empty_outer_modal_bindings_preserve_complete_program() {
    let original = lowered(SALVE);
    let rewritten = lowered(&changed_bindings(None, json!([])));
    assert_eq!(rewritten.characteristics, original.characteristics);
    assert_eq!(rewritten.spell, original.spell);
    assert_eq!(rewritten.capabilities, original.capabilities);
}

#[test]
fn standalone_modal_rejects_outer_fixed_binding() {
    assert_binding_rejected(
        &changed_bindings(None, json!([fixed_binding()])),
        "bindings on standalone modal instructions are unsupported",
    );
}

#[test]
fn standalone_modal_rejects_outer_announcement_binding() {
    let mut binding = fixed_binding();
    binding["asOf"] = json!("casting");
    assert_binding_rejected(
        &changed_bindings(None, json!([binding])),
        "bindings on standalone modal instructions are unsupported",
    );
}

#[test]
fn standalone_modal_rejects_outer_unconsumed_count_binding() {
    assert_binding_rejected(
        &changed_bindings(None, json!([count_binding()])),
        "bindings on standalone modal instructions are unsupported",
    );
}

#[test]
fn modal_options_keep_existing_binding_validation() {
    for option in 0..2 {
        assert_binding_rejected(
            &changed_bindings(Some(option), json!([fixed_binding()])),
            "unsupported complete X binding",
        );
        assert_binding_rejected(
            &changed_bindings(Some(option), json!([count_binding()])),
            "unconsumed count binding",
        );
    }
}
