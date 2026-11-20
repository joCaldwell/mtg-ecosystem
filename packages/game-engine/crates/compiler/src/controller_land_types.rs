use super::*;
use crate::bindings::Bindings;
use serde_json::{Value, json};

pub(super) fn lower(effect: &SourceStaticEffect) -> Option<StaticAbility> {
    let SourceStaticEffect::Modify { what, modifiers } = effect else {
        return None;
    };
    for (ast, grant) in [
        (
            controller_land_type_ast(),
            StaticEffect::GrantControllerLandsAllBasicTypes,
        ),
        (
            controller_nonland_colors_ast(),
            StaticEffect::ControllerNonlandsAllColors,
        ),
    ] {
        let expected = &ast[0]["effect"];
        if *what == expected["what"] && expected["modifiers"].as_array() == Some(modifiers) {
            return Some(StaticAbility {
                functioning_zone: FunctioningZone::Battlefield,
                effect: grant,
            });
        }
    }
    None
}

pub(super) fn bind(d: &Definition, raw: &Value, binding: &Value) -> Result<(), Failure> {
    let colors = d
        .capabilities
        .contains(&Capability::ControllerNonlandColorsV1);
    if !colors
        && !d
            .capabilities
            .contains(&Capability::ControllerLandBasicTypesV1)
    {
        return Ok(());
    }
    let mut graph = Bindings::new(&d.oracle_id, raw);
    let ability = "/faces/0/abilities/0";
    let ast = if colors {
        controller_nonland_colors_ast()
    } else {
        controller_land_type_ast()
    };
    let selector = ast[0]["effect"]["what"].clone();
    bind_grant(&mut graph, ability, &selector);
    graph.validate(
        binding,
        "requires the exact continuous query and static controller",
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

#[test]
fn controller_nonland_colors_require_the_complete_continuous_selection() {
    let record = include_str!("../../../tests/fixtures/synthetic-nonland-colors.json");
    for mutation in 0..3 {
        let changed = crate::tests::changed_source(record, |r| match mutation {
            0 => r["binding"]["references"][0]["target"] = json!("different-controller"),
            1 => {
                r["binding"]["nodes"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .find(|node| node["node"] == "selection")
                    .unwrap()["expression"]["filter"]["other"] = json!(true)
            }
            _ => {
                let line = "Other nonland permanents you control are all colors.";
                r["source"]["oracle_text"] = json!(line);
                r["faces"][0]["result"]["lines"][0]["text"] = json!(line);
            }
        });
        assert!(!matches!(
            compile(fixtures::MANIFEST, &changed),
            CompileResult::Lowered { .. }
        ));
    }
}

pub(super) fn bind_grant(graph: &mut Bindings, ability: &str, selector: &Value) {
    let path = format!("{ability}/effect/what");
    graph.ability(ability, "static", "static-source-controller");
    let query=graph.node(&path,"query","selection",json!({"scope":graph.id(ability,"ability"),"expression":selector,
        "cardinality":"many","timing":"continuous","selection":"evaluate-reference","rules":["608.2h"]}));
    graph.reference(
        &format!("{path}/filter/control"),
        &json!({"player":"you"}),
        &graph.id(ability, "you"),
        "109.5",
    );
    graph.reference(&path, selector, &query, "608.2h");
}

#[test]
fn opening_hybrid_three_static_blocks_bind_each_controller_and_exact_source() {
    let record =
        include_str!("../../../tests/fixtures/ramos/6b2e6a51-b9e6-4848-b7d7-98f02f2b897f.json");
    let manifest = include_str!("../../../tests/fixtures/archive-manifest-current.json");
    let result = compile(manifest, record);
    let CompileResult::Lowered { definition } = result else {
        panic!("{result:?}")
    };
    assert_eq!(definition.static_abilities.len(), 3);
    assert_eq!(definition.characteristics.printed_mana_value(), 4.into());
    for n in 0..3 {
        let mut stripped = (*definition).clone();
        stripped.static_abilities.remove(n);
        stripped.capabilities = stripped.requirements();
        assert!(stripped.validate().is_err());
    }
    for n in 0..4 {
        let altered = crate::tests::changed_source(record, |r| match n {
            0 => {
                r["binding"]["references"][4]["target"] =
                    r["binding"]["references"][2]["target"].clone()
            }
            1 => {
                r["binding"]["references"][0]["target"] =
                    r["binding"]["references"][1]["target"].clone()
            }
            2 => r["faces"][0]["result"]["abilities"]
                .as_array_mut()
                .unwrap()
                .swap(1, 2),
            _ => r["source"]["mana_cost"] = json!("{W/P}"),
        });
        assert!(!matches!(
            compile(manifest, &altered),
            CompileResult::Lowered { .. }
        ));
    }
}
