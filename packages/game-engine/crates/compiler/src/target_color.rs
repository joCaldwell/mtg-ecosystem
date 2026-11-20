//! CR 105.4, 608.2d, 611.2c and 613.1e (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(super) fn lower(
    source: &Source,
    shape: &Value,
    result: &ParseResult,
    binding: &Value,
) -> Result<Option<ActivatedAbility>, Failure> {
    let Some((ability, ast)) = mtg_contracts::target_color::source(&source.oracle_text) else {
        return Ok(None);
    };
    if decode::<Value>(result.abilities.get(), "abilities")? != ast {
        return Err(unsupported(
            "abilities",
            "requires exact single-color target, duration and choice timing",
        ));
    }
    let mut graph = bindings::Bindings::new(&source.oracle_id, shape);
    let root = "/faces/0/abilities/0";
    let step = format!("{root}/effects/0/steps/0");
    let you = graph.id(root, "you");
    graph.ability(root, "activated", "activator");
    graph.reference(
        &format!("{root}/costs/0/source"),
        &json!({"ref":"self"}),
        &graph.id("/source", "object"),
        "201.5",
    );
    graph.reference(
        &format!("{root}/costs/0/payer"),
        &json!({"player":"you"}),
        &you,
        "109.5",
    );
    let scope = graph.id(root, "ability");
    graph.target(
        &scope,
        &format!("{step}/what"),
        &ast[0]["effects"][0]["steps"][0]["what"],
        "line:0:2",
    );
    graph.reference(
        &format!("{step}/modifiers/0/colors/who"),
        &json!({"player":"you"}),
        &you,
        "109.5",
    );
    graph.validate_with_features(binding, &[json!({
        "feature":"characteristic-layers","path":step,
        "message":"Characteristic changes need the engine's layer system, with the affected set and values fixed when the effect begins.",
        "rules":["611.2c","611.2d","613.6","604.3a"],
    })], "requires exact target, activation and color choice reference graph")?;
    Ok(Some(ability))
}

#[test]
fn target_color_rejects_shortened_duration_or_missing_activation() {
    let manifest = include_str!("../../../tests/fixtures/archive-manifest-current.json");
    for source in [
        include_str!("../../../tests/fixtures/9c9579c5-7edf-41ca-8595-713ca9245576.json"),
        include_str!("../../../tests/fixtures/8fc72080-8d4d-427d-ae30-065bc8ee9a28.json"),
    ] {
        let CompileResult::Lowered { definition } = compile(manifest, source) else {
            panic!("{:?}", compile(manifest, source));
        };
        let mut omitted = (*definition).clone();
        omitted.activated.clear();
        omitted.capabilities = omitted.requirements();
        assert!(omitted.validate().is_err());
        let bad = crate::tests::changed_source(source, |row| {
            for step in [
                "/faces/0/result/abilities/0/effects/0/steps/0",
                "/faces/0/result/lines/0/ability/effects/0/steps/0",
            ] {
                row.pointer_mut(step).unwrap()["duration"] = json!({"duration":"this-combat"});
            }
        });
        assert!(!matches!(
            compile(manifest, &bad),
            CompileResult::Lowered { .. }
        ));
    }
}
