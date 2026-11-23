//! Exact archived Ertai source and hash-consistent unsupported AST probes.
use super::*;
use serde_json::{Value, json};
const MANIFEST: &str = include_str!("../../../../tests/fixtures/archive-manifest-current.json");
const ERTAI: &str =
    include_str!("../../../../tests/fixtures/f2902dfd-888f-4b02-b8ad-1056f0059fc3.json");
fn changed(mutate: impl FnOnce(&mut Value)) -> String {
    tests::changed_source(ERTAI, |r| {
        mutate(&mut r["faces"][0]["result"]["abilities"][0]);
        let result = &mut r["faces"][0]["result"];
        result["lines"][0]["ability"] = result["abilities"][0].clone();
        let target = result["abilities"][0]["effects"][0]["steps"][0]["what"].clone();
        let path = "/faces/0/abilities/0/effects/0/steps/0/what";
        for n in r["binding"]["nodes"].as_array_mut().unwrap() {
            if n["node"] == "selection" && n["path"] == path {
                n["expression"] = target.clone();
            }
        }
        for n in r["binding"]["references"].as_array_mut().unwrap() {
            if n["path"] == path {
                n["expression"] = target.clone();
            }
        }
    })
}
#[test]
fn activated_counter_of_an_ability_is_an_ordinary_composed_activation() {
    // Self-consistent near misses are ordinary cards: the block pipeline
    // lowers them, marked composed, with exactly the changed selector.
    let raw = changed(|a| {
        a["effects"][0]["steps"][0]["what"]["filter"] = json!({"cls":"ability","anyOf":[{"abilityKind":"activated"},{"abilityKind":"triggered"}]})
    });
    let d = tests::composed_near_miss(crate::tests::synthetic_compile(MANIFEST, &raw));
    let [ability] = d.activated.as_slice() else {
        panic!("one activation")
    };
    assert_eq!(
        tests::sole_target(&ability.program),
        (
            TargetDomain::StackActivatedOrTriggeredAbility,
            vec![],
            vec![],
            vec![],
            false,
            false
        )
    );
    assert_eq!(tests::ops(&ability.program), ["counter_stack_object"]);
}
