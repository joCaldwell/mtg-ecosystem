//! Bind the live controller, exact source exclusion and continuous recipient
//! query of one printed stat modifier (CR 109.5, 201.5, 611.3a/b, 613.4c).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(super) fn lower(
    effect: &SourceStaticEffect,
    block: usize,
    result: &ParseResult,
    source: &Value,
    binding: &Value,
    identity: &str,
    kind: CardKind,
) -> Result<Option<StaticAbility>, Failure> {
    if kind != CardKind::Creature || !matches!(effect, SourceStaticEffect::Modify { .. }) {
        return Ok(None);
    }
    let line = &result.lines[block];
    let Some((ability, ast)) = mtg_contracts::scoped_creature_stats::printed(&line.text) else {
        return Ok(None);
    };
    if block != 0
        || result.lines.len() != 1
        || decode::<Value>(line.ability.get(), "lines.ability")? != ast
    {
        return Err(unsupported(
            "static.scope",
            "requires complete printed scoped creature stats",
        ));
    }
    let root = "/faces/0/abilities/0";
    let path = format!("{root}/effect/what");
    let expression = &ast["effect"]["what"];
    let mut graph = bindings::Bindings::new(identity, source);
    graph.ability(root, "static", "static-source-controller");
    let query = graph.node(
        &path,
        "query",
        "selection",
        json!({
            "scope":graph.id(root,"ability"),"expression":expression,"cardinality":"many",
            "timing":"continuous","selection":"evaluate-reference","rules":["608.2h"]
        }),
    );
    if expression["filter"]["other"] == true {
        graph.reference(
            &format!("{path}/filter/other"),
            &json!({"ref":"self"}),
            &graph.id("/source", "object"),
            "201.5",
        );
    }
    graph.reference(
        &format!("{path}/filter/control"),
        &json!({"player":"you"}),
        &graph.id(root, "you"),
        "109.5",
    );
    graph.reference(&path, expression, &query, "608.2h");
    graph.validate(
        binding,
        "requires the exact live controller and recipient filter",
    )?;
    Ok(Some(ability))
}
