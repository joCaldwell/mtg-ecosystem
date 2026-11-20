use super::*;
use crate::bindings::Bindings;
use serde_json::{Value, json};

pub(super) fn lower(effect: &SourceStaticEffect) -> Option<StaticAbility> {
    let SourceStaticEffect::Modify { what, modifiers } = effect else {
        return None;
    };
    let ast = controller_land_type_ast();
    let expected = &ast[0]["effect"];
    (*what == expected["what"] && expected["modifiers"].as_array() == Some(modifiers)).then_some(
        StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::GrantControllerLandsAllBasicTypes,
        },
    )
}

pub(super) fn bind(d: &Definition, raw: &Value, binding: &Value) -> Result<(), Failure> {
    if !d
        .capabilities
        .contains(&Capability::ControllerLandBasicTypesV1)
    {
        return Ok(());
    }
    let mut graph = Bindings::new(&d.oracle_id, raw);
    let ability = "/faces/0/abilities/0";
    let path = "/faces/0/abilities/0/effect/what";
    let selector = json!({"ref":"each","filter":{"types":["land"],"control":"you"}});
    graph.ability(ability, "static", "static-source-controller");
    let query=graph.node(path,"query","selection",json!({"scope":graph.id(ability,"ability"),"expression":selector,
        "cardinality":"many","timing":"continuous","selection":"evaluate-reference","rules":["608.2h"]}));
    graph.reference(
        &format!("{path}/filter/control"),
        &json!({"player":"you"}),
        &graph.id(ability, "you"),
        "109.5",
    );
    graph.reference(path, &selector, &query, "608.2h");
    graph.validate(
        binding,
        "requires the exact controlled land query and static controller",
    )
}

#[test]
fn controller_land_type_grant_requires_complete_source_and_continuous_binding() {
    let record = include_str!("../../../tests/fixtures/synthetic-controller-land-types.json");
    let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, record) else {
        panic!(
            "synthetic land grant failed: {:?}",
            compile(fixtures::MANIFEST, record)
        );
    };
    assert!(
        definition
            .capabilities
            .contains(&Capability::ControllerLandBasicTypesV1)
    );
    let mut stripped = *definition;
    stripped.static_abilities.clear();
    stripped.capabilities = stripped.requirements();
    assert!(stripped.validate().is_err());
    for n in 0..4 {
        let invalid = crate::tests::changed_source(record, |r| match n {
            0 => {
                r["binding"]["nodes"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .find(|v| v["node"] == "selection")
                    .unwrap()["timing"] = json!("resolution")
            }
            1 => r["binding"]["references"][0]["target"] = json!("another-controller"),
            2 => r["binding"]["diagnostics"] = json!([{"reason":"unresolved"}]),
            _ => {
                let text = r["source"]["oracle_text"]
                    .as_str()
                    .unwrap()
                    .replace("you control", "an opponent controls");
                r["source"]["oracle_text"] = json!(text);
                r["faces"][0]["result"]["lines"][0]["text"] = json!(text);
            }
        });
        assert!(
            !matches!(
                compile(fixtures::MANIFEST, &invalid),
                CompileResult::Lowered { .. }
            ),
            "mutation {n}"
        );
    }
}
