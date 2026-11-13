//! Synthetic AST probes derived from complete retained source records. These
//! preserve source text, synchronize line ASTs, and recompute archive hashes.
use super::*;
use serde_json::{Value, json};

const UNSUMMON: &str = include_str!("../../../tests/fixtures/unsummon.json");
const BOLT: &str = include_str!("../../../tests/fixtures/lightning-bolt.json");
const DOOM: &str = include_str!("../../../tests/fixtures/doom-blade.json");
const EBB: &str = include_str!("../../../tests/fixtures/time-ebb.json");
const ARSONIST: &str = include_str!("../../../tests/fixtures/goblin-arsonist.json");
const FLAGS: [(&str, &str); 6] = [
    (UNSUMMON, "optional"),
    (BOLT, "optional"),
    (DOOM, "optional"),
    (DOOM, "regenerationForbidden"),
    (EBB, "tapped"),
    (ARSONIST, "optional"),
];

fn changed(raw: &str, field: &str, value: Option<Value>) -> String {
    tests::changed_source(raw, |record| {
        let result = &mut record["faces"][0]["result"];
        let step = result["abilities"][0]["effects"][0]["steps"][0]
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
        panic!("{result:?}")
    };
    definition.validate().unwrap();
    definition
}

fn assert_bad_boolean(raw: &str) {
    let result = compile(fixtures::MANIFEST, raw);
    let CompileResult::Unsupported { path, reason } = result else {
        panic!("malformed boolean must reject at the typed AST: {result:?}")
    };
    assert_eq!(path, "abilities[0].effects[0].steps[0]");
    assert!(reason.ends_with("expected_a_boolean"), "{reason}");
}

#[test]
fn retained_boolean_sources_match_frozen_definitions() {
    for (raw, expected) in [
        (
            UNSUMMON,
            include_str!("../../../tests/fixtures/expected-ir.json"),
        ),
        (
            BOLT,
            include_str!("../../../tests/fixtures/expected-damage-life-ir.json"),
        ),
        (
            DOOM,
            include_str!("../../../tests/fixtures/expected-removal-ir.json"),
        ),
        (
            EBB,
            include_str!("../../../tests/fixtures/expected-library-ir.json"),
        ),
        (
            ARSONIST,
            include_str!("../../../tests/fixtures/expected-sacrifice-ir.json"),
        ),
    ] {
        let definition = lowered(raw);
        let catalog = Catalog::read(expected).unwrap();
        assert_eq!(
            catalog
                .definitions
                .iter()
                .find(|d| d.oracle_id == definition.oracle_id),
            Some(definition.as_ref())
        );
        // A no-op synthetic rewrite proves this mutation helper passes the
        // whole-source, hash and line-AST checks before any negative probe.
        let record: Value = serde_json::from_str(raw).unwrap();
        let value = record["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]
            .get("optional")
            .cloned();
        // Canonical rewriting changes provenance bytes/key order, so equality
        // with frozen definitions is checked only for the original above.
        lowered(&changed(raw, "optional", value));
    }
}

#[test]
fn absent_true_false_flags_keep_existing_decode_and_lowering_policy() {
    for (raw, field) in FLAGS {
        for value in [None, Some(false), Some(true)] {
            let mutant = changed(raw, field, value.map(Value::Bool));
            let record: Value = serde_json::from_str(&mutant).unwrap();
            let step: Step = serde_json::from_value(
                record["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0].clone(),
            )
            .unwrap();
            let decoded = match step {
                Step::ReturnToHand { optional, .. } | Step::Damage { optional, .. } => optional,
                Step::Destroy {
                    optional,
                    regeneration_forbidden,
                    ..
                } => {
                    if field == "optional" {
                        optional
                    } else {
                        regeneration_forbidden
                    }
                }
                Step::MoveZone { tapped, .. } => tapped,
                _ => unreachable!(),
            };
            assert_eq!(decoded, value);
            let result = compile(fixtures::MANIFEST, &mutant);
            // Owner-hand returns and library moves only support omission;
            // regenerationForbidden supports omission/true, never false.
            let rejected = (raw == UNSUMMON || raw == EBB) && value.is_some()
                || field == "regenerationForbidden" && value == Some(false);
            if rejected {
                let CompileResult::Unsupported { path, .. } = result else {
                    panic!("{result:?}")
                };
                assert_eq!(
                    path,
                    if field == "regenerationForbidden" {
                        "regeneration"
                    } else {
                        "effects.steps"
                    }
                );
            } else {
                lowered(&mutant);
            }
        }
    }
}

#[test]
fn nonboolean_step_flags_reject_at_the_typed_ast() {
    for (raw, field) in FLAGS {
        for value in [json!("false"), json!(0), json!([]), json!({})] {
            assert_bad_boolean(&changed(raw, field, Some(value)));
        }
    }
}

#[test]
fn null_step_flags_must_not_be_treated_as_absent() {
    for (raw, field) in FLAGS {
        assert_bad_boolean(&changed(raw, field, Some(Value::Null)));
    }
}
