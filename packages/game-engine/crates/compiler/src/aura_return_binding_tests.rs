//! Synthetic AST probes over complete retained Rancor source, not parser-produced
//! card records or evidence of real-card misexecution.
use super::*;
use serde_json::{Value, json};

const RANCOR: &str = include_str!("../../../tests/fixtures/rancor.json");

fn changed_bindings(bindings: Value) -> String {
    tests::changed_source(RANCOR, |record| {
        let result = &mut record["faces"][0]["result"];
        result["abilities"][2]["effects"][0]["bindings"] = bindings;
        result["lines"][2]["ability"] = result["abilities"][2].clone();
    })
}

fn lowered(raw: &str) -> Box<Definition> {
    let result = compile(fixtures::MANIFEST, raw);
    let CompileResult::Lowered { definition } = result else {
        panic!("complete Rancor must lower: {result:?}")
    };
    definition.validate().unwrap();
    definition
}

fn assert_binding_rejected(binding: Value) {
    let result = compile(fixtures::MANIFEST, &changed_bindings(json!([binding])));
    assert!(
        matches!(&result, CompileResult::Unsupported { path, reason }
            if path == "trigger" && reason == "unsupported Aura return"),
        "Aura return must not erase an executable sentence binding: {result:?}"
    );
}

// Sentence.bindings in oracle-parser/src/ast.ts admits Amount and optional
// AnnouncementTime. These valid shapes reach the semantic shortcut guard.
fn fixed_binding() -> Value {
    json!({"variable":"x","value":{"amount":"fixed","value":3}})
}

#[test]
fn retained_rancor_matches_complete_frozen_ir() {
    let definition = lowered(RANCOR);
    let catalog = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-attachments-ir.json"
    ))
    .unwrap();
    assert_eq!(
        catalog
            .definitions
            .iter()
            .find(|d| d.oracle_id == definition.oracle_id),
        Some(definition.as_ref())
    );
    assert_eq!(definition.triggered.len(), 1);
    assert_eq!(
        definition.triggered[0].condition,
        TriggerCondition::SelfAuraToGraveyard
    );
    assert!(matches!(
        definition.triggered[0].program.instructions.as_slice(),
        [Instruction::ReturnAuraFromGraveyard { source_block: 2 }]
    ));
    println!(
        "retained Rancor full IR digest: {}",
        digest(&definition).unwrap()
    );
}

#[test]
fn explicit_empty_aura_return_bindings_preserve_complete_ir() {
    // Rehash both inputs identically, then normalize only the changed AST
    // provenance. Compare every executable field and all remaining provenance.
    let original = lowered(&tests::changed_source(RANCOR, |_| {}));
    let raw = changed_bindings(json!([]));
    let record: Value = serde_json::from_str(&raw).unwrap();
    let mut rewritten = lowered(&raw);
    assert_eq!(
        rewritten.provenance.ast_sha256,
        record["astSha256"].as_str().unwrap()
    );
    rewritten.provenance.ast_sha256 = original.provenance.ast_sha256.clone();
    rewritten.provenance.abilities_json = original.provenance.abilities_json.clone();
    assert_eq!(rewritten, original);
}

#[test]
fn aura_return_rejects_fixed_binding() {
    assert_binding_rejected(fixed_binding());
}

#[test]
fn aura_return_rejects_count_binding() {
    assert_binding_rejected(json!({"variable":"x","value":{"amount":"count","of":{
        "types":["creature"],"zone":{"zone":"battlefield"}
    }}}));
}

#[test]
fn aura_return_rejects_announcement_binding() {
    let mut binding = fixed_binding();
    binding["asOf"] = json!("casting");
    assert_binding_rejected(binding);
}
