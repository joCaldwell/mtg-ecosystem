//! Complete frozen controls and synthetic, rehashed AST operand mutations.
//! Mutants preserve Oracle source text; they are not claimed as frozen records.
use super::*;
use serde_json::{Value, json};

const RAISE: &str = include_str!("../../../tests/fixtures/raise-the-alarm.json");
const NECTAR: &str = include_str!("../../../tests/fixtures/sacred-nectar.json");
const SWAN: &str = include_str!("../../../tests/fixtures/swan-song.json");
const FUMIGATE: &str = include_str!("../../../tests/fixtures/fumigate.json");

fn changed(raw: &str, effect: usize, field: &str, value: Option<Value>) -> String {
    tests::changed_source(raw, |record| {
        let result = &mut record["faces"][0]["result"];
        let step = result["abilities"][0]["effects"][effect]["steps"][0]
            .as_object_mut()
            .unwrap();
        if let Some(value) = value {
            step.insert(field.into(), value);
        } else {
            step.remove(field);
        }
        result["lines"][0]["ability"] = result["abilities"][0].clone();
    })
}

fn lowered(raw: &str) -> Box<Definition> {
    let result = compile(fixtures::MANIFEST, raw);
    let CompileResult::Lowered { definition } = result else {
        panic!("complete control must lower: {result:?}")
    };
    definition.validate().unwrap();
    definition
}

fn assert_null_rejected(raw: &str, effect: usize, field: &str) {
    let mutant = changed(raw, effect, field, Some(Value::Null));
    let record: Value = strict_read(&mutant).unwrap();
    let original: Value = strict_read(raw).unwrap();
    assert_eq!(record["source"], original["source"]);
    let result = &record["faces"][0]["result"];
    assert_eq!(result["lines"][0]["ability"], result["abilities"][0]);
    assert_eq!(
        record["sourceSha256"],
        sha256(canonical(&record["source"]).unwrap().as_bytes())
    );
    assert_eq!(
        record["bindingSha256"],
        sha256(canonical(&record["binding"]).unwrap().as_bytes())
    );
    assert_eq!(
        record["astSha256"],
        sha256(canonical(&json!([result["abilities"]])).unwrap().as_bytes())
    );
    let compiled = compile(fixtures::MANIFEST, &mutant);
    assert!(
        matches!(&compiled, CompileResult::Unsupported { path, .. }
            if path == &format!("abilities[0].effects[{effect}].steps[0]")),
        "explicit {field}:null must reject at the typed AST, never select omission: {compiled:?}"
    );
    assert!(
        serde_json::from_value::<Step>(
            result["abilities"][0]["effects"][effect]["steps"][0].clone()
        )
        .is_err()
    );
}

#[test]
fn frozen_absent_and_present_operands_preserve_catalog_ir() {
    for (raw, catalog) in [
        (RAISE, "tokens"),
        (NECTAR, "damage-life"),
        (SWAN, "wipes"),
        (FUMIGATE, "wipes"),
    ] {
        let definition = lowered(raw);
        assert_eq!(
            embedded_catalog(catalog)
                .unwrap()
                .definitions
                .iter()
                .find(|d| d.oracle_id == definition.oracle_id),
            Some(definition.as_ref())
        );
    }
    for (raw, field) in [(RAISE, "who"), (NECTAR, "forEach")] {
        let record: Value = strict_read(raw).unwrap();
        assert!(
            record["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]
                .get(field)
                .is_none()
        );
        assert_eq!(
            lowered(&changed(raw, 0, field, None)).spell,
            lowered(raw).spell
        );
    }
    assert!(matches!(
        lowered(RAISE).spell.unwrap().instructions.as_slice(),
        [Instruction::CreateCreatureTokens { count: 2, .. }]
    ));
    assert!(matches!(
        lowered(NECTAR).spell.unwrap().instructions.as_slice(),
        [kernel!(GainLife { player: you!(), .. })]
    ));
    assert!(matches!(
        lowered(SWAN).spell.unwrap().instructions[1],
        Instruction::CreateTokensFor {
            recipient: EffectRecipient::TargetController { target_slot: 0 },
            ..
        }
    ));
    assert!(matches!(
        lowered(FUMIGATE).spell.unwrap().instructions[1],
        Instruction::GainLifeDestroyed { .. }
    ));
}

#[test]
fn valid_explicit_operands_keep_existing_semantics() {
    let explicit = lowered(&changed(RAISE, 0, "who", Some(json!({"player":"you"}))));
    assert!(matches!(
        explicit.spell.unwrap().instructions.as_slice(),
        [Instruction::CreateTokensFor {
            recipient: EffectRecipient::Controller,
            count: 2,
            ..
        }]
    ));
    for (raw, field) in [(SWAN, "who"), (FUMIGATE, "forEach")] {
        let record: Value = strict_read(raw).unwrap();
        let value =
            record["faces"][0]["result"]["abilities"][0]["effects"][1]["steps"][0][field].clone();
        assert_eq!(
            lowered(&changed(raw, 1, field, Some(value))).spell,
            lowered(raw).spell
        );
    }
}

#[test]
fn create_token_null_who_cannot_become_implicit_controller() {
    assert_null_rejected(RAISE, 0, "who");
}

#[test]
fn gain_life_null_for_each_cannot_become_fixed_life() {
    assert_null_rejected(NECTAR, 0, "forEach");
}

#[test]
fn explicit_token_recipient_null_cannot_discard_target_controller() {
    assert_null_rejected(SWAN, 1, "who");
}

#[test]
fn explicit_life_cardinality_null_cannot_discard_event_result() {
    assert_null_rejected(FUMIGATE, 1, "forEach");
}

#[test]
fn nonconforming_operands_keep_strict_semantic_rejection() {
    for value in [
        json!(false),
        json!(0),
        json!("you"),
        json!([]),
        json!({}),
        json!({"player":"each-player"}),
        json!({"player":"you","unknown":true}),
    ] {
        let result = compile(
            fixtures::MANIFEST,
            &changed(RAISE, 0, "who", Some(value.clone())),
        );
        assert!(
            matches!(&result, CompileResult::Unsupported { path, .. } if path == "recipient"),
            "who={value}: {result:?}"
        );
    }
    let record: Value = strict_read(FUMIGATE).unwrap();
    let valid =
        record["faces"][0]["result"]["abilities"][0]["effects"][1]["steps"][0]["forEach"].clone();
    let mut extra = valid.clone();
    extra["unknown"] = json!(true);
    let mut wrong_relation = valid;
    wrong_relation["relation"]["scope"] = json!("this-turn");
    for value in [
        json!(false),
        json!(0),
        json!("creature"),
        json!([]),
        json!({}),
        extra,
        wrong_relation,
    ] {
        let result = compile(
            fixtures::MANIFEST,
            &changed(FUMIGATE, 1, "forEach", Some(value.clone())),
        );
        assert!(
            matches!(&result, CompileResult::Unsupported { path, .. } if path == "forEach"),
            "forEach={value}: {result:?}"
        );
    }
    for raw in [RAISE, NECTAR] {
        let result = compile(
            fixtures::MANIFEST,
            &changed(raw, 0, "unknown", Some(json!(true))),
        );
        assert!(
            matches!(&result, CompileResult::Unsupported { path, .. } if path == "abilities[0].effects[0].steps[0]"),
            "{result:?}"
        );
    }
}
