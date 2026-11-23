//! UNQUALIFIED source checkpoint: complete frozen Imoti and explicit AST controls.
use super::*;
use serde_json::json;
const MANIFEST: &str =
    include_str!("../../../../tests/fixtures/ramos/archive-manifest-current.json");
const IMOTI: &str =
    include_str!("../../../../tests/fixtures/ramos/eca32dcd-6845-433e-a631-ed1f0ee78f25.json");
#[test]
fn ramos_m4c_synthetic_ast_thresholds_are_exact_parameters() {
    let r: serde_json::Value = strict_read(IMOTI).unwrap();
    for value in [json!(0), json!(2), json!(9), json!(u64::MAX)] {
        let mut effect = r["faces"][0]["result"]["abilities"][1]["effect"].clone();
        effect["what"]["filter"]["properties"][0]["comparison"]["value"]["value"] = value.clone();
        let effect: SourceStaticEffect = serde_json::from_value(effect).unwrap();
        let a = spell_cascade_grants::lower(&effect, 17).unwrap().unwrap();
        assert_eq!(
            a.effect,
            StaticEffect::GrantSpellCascade {
                eligibility: SpellCascadeEligibility::ManaValueAtLeast {
                    minimum: value.to_string().parse().unwrap()
                },
                program: canonical_cascade_program(17),
            }
        );
    }
}
#[test]
fn ramos_m4c_history_and_unparsed_whole_sources_remain_unsupported() {
    for raw in [include_str!(
        "../../../../tests/fixtures/ramos/9efe8aff-9a7d-4397-b5fd-c1a0fad7c15f.json"
    )] {
        assert!(matches!(
            compile(MANIFEST, raw),
            CompileResult::Unsupported { .. }
        ));
    }
}
