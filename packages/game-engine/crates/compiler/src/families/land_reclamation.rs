//! Whole three-block source with distinct continuous permissions and Landfall.
//! CR 109.5, 305.2, 611.3 and 603.6a (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(crate) fn bound(
    source: &Source,
    shape: &Value,
    result: &ParseResult,
    binding: &Value,
) -> Result<bool, Failure> {
    let Some(ast) = land_reclamation_source(&source.oracle_text) else {
        return Ok(false);
    };
    if !type_line::exactly(&source.type_line, &[CardKind::Creature], false)
        || decode::<Value>(result.abilities.get(), "abilities")? != ast
    {
        return Err(unsupported(
            "land_reclamation",
            "requires the complete printed land permission and trigger source",
        ));
    }
    let mut graph = bindings::Bindings::new(&source.oracle_id, shape);
    let allowance = "/faces/0/abilities/0";
    let permission = "/faces/0/abilities/1";
    let block = "/faces/0/abilities/2";
    let you = json!({"player":"you"});
    for root in [allowance, permission] {
        graph.ability(root, "static", "static-source-controller");
    }
    graph.reference(
        &format!("{allowance}/effect/allowance/who"),
        &you,
        &graph.id(allowance, "you"),
        "109.5",
    );
    let what = format!("{permission}/effect/permission/what");
    let expr = json!({"ref":"each","filter":{"types":["land"]}});
    let query = graph.node(&what, "query", "selection", json!({
        "scope":graph.id(permission,"ability"),"expression":expr,"cardinality":"many","timing":"continuous",
        "selection":"evaluate-reference","rules":["608.2h"]
    }));
    graph.reference(&what, &expr, &query, "608.2h");
    for field in ["who", "from/owner"] {
        graph.reference(
            &format!("{permission}/effect/permission/{field}"),
            &you,
            &graph.id(permission, "you"),
            "109.5",
        );
    }
    graph.ability(block, "triggered", "trigger-controller");
    let trigger = format!("{block}/trigger");
    let subject = format!("{trigger}/what");
    let expr = controlled_land_query();
    let query = graph.node(&subject, "query", "selection", json!({
        "scope":graph.id(block,"ability"),"expression":expr,"cardinality":"one","timing":"trigger-match",
        "selection":"evaluate-reference","rules":["608.2h"]
    }));
    let controller = graph.id(block, "you");
    graph.reference(
        &format!("{subject}/filter/control"),
        &you,
        &controller,
        "109.5",
    );
    graph.reference(&subject, &expr, &query, "608.2h");
    graph.event(block, &trigger, "trigger:enters", &query, false);
    graph.result(&trigger, "objects", "same-objects", Some(&expr["filter"]));
    let step = format!("{block}/effects/0/steps/0");
    graph.reference(&format!("{step}/who"), &you, &controller, "109.5");
    graph.event(block, &step, "mill", &controller, false);
    graph.result(
        &step,
        "objects",
        "new-objects",
        Some(&json!({"cls":"card"})),
    );
    graph.result(&step, "quantity", "measured-quantity", None);
    graph.validate(
        binding,
        "requires both continuous land permissions and controlled-entry mill",
    )?;
    Ok(true)
}

#[test]
fn land_reclamation_requires_all_printed_blocks_and_permission_ownership() {
    let source =
        include_str!("../../../../tests/fixtures/109cdefd-e8cc-4ac7-b6ba-2cfdef8d780f.json");
    let manifest = include_str!("../../../../tests/fixtures/archive-manifest-current.json");
    let result = compile(manifest, source);
    let CompileResult::Lowered { definition } = result else {
        panic!("{result:?}")
    };
    assert_eq!(definition.static_abilities, land_reclamation_statics());
    assert_eq!(definition.triggered, [land_reclamation_trigger()]);
    let altered = crate::tests::changed_source(source, |row| {
        let refs = row["binding"]["references"].as_array_mut().unwrap();
        refs.iter_mut()
            .find(|value| value["path"].as_str().unwrap().ends_with("/from/owner"))
            .unwrap()["target"] = json!("foreign-graveyard");
    });
    assert!(
        matches!(compile(manifest, &altered), CompileResult::Unsupported { path, .. } if path == "binding")
    );
}

#[test]
fn land_reclamation_rejects_unmatched_static_operands() {
    for operand in [
        SourceStaticEffect::AdditionalLandPlays {
            _allowance: json!({}),
        },
        SourceStaticEffect::PlayPermission {
            _permission: json!({}),
        },
    ] {
        assert!(matches!(lower_static(&operand, 0), Err(Failure { .. })));
    }
}
