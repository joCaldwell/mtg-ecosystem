//! Exact binding of one live static modifier block alongside independently
//! supported siblings.
use super::*;
use serde_json::{Value, json};

pub(super) fn lower(
    block: usize,
    result: &ParseResult,
    source: &Value,
    binding: &Value,
    identity: &str,
) -> Result<Option<StaticAbility>, Failure> {
    let line = &result.lines[block];
    let Some((ability, ast)) = mtg_contracts::scoped_static::printed(&line.text) else {
        return Ok(None);
    };
    let fail = || {
        unsupported(
            "static.scope",
            "requires the complete subtype modifier and live binding scope",
        )
    };
    if decode::<Value>(line.ability.get(), "lines.ability")? != ast {
        return Err(fail());
    }
    let base = format!("/faces/0/abilities/{block}");
    let path = format!("{base}/effect/what");
    let expression = &ast["effect"]["what"];
    let mut graph = bindings::Bindings::new(identity, source);
    graph.ability(&base, "static", "static-source-controller");
    let scope = graph.id(&base, "ability");
    let query = graph.node(
        &path,
        "query",
        "selection",
        json!({
            "scope":scope,"expression":expression,"cardinality":"many","timing":"continuous",
            "selection":"evaluate-reference","rules":["608.2h"]
        }),
    );
    graph.reference(
        &format!("{path}/filter/control"),
        &json!({"player":"you"}),
        &graph.id(&base, "you"),
        "109.5",
    );
    if expression["filter"]["other"] == true {
        graph.reference(
            &format!("{path}/filter/other"),
            &json!({"ref":"self"}),
            &graph.id("/source", "object"),
            "201.5",
        );
    }
    graph.reference(&path, expression, &query, "608.2h");
    let StaticEffect::ModifyCreatures { keywords, .. } = &ability.effect else {
        return Err(fail());
    };
    for (index, keyword) in keywords.iter().enumerate() {
        let rule = mtg_contracts::scoped_static::keyword_rule(*keyword).ok_or_else(fail)?;
        let at = format!("{base}/effect/modifiers/0/abilities/{index}");
        let recipient = graph.node(
            &at,
            "recipient",
            "recipient",
            json!({"scope":scope,"of":query,"each":true,"rules":["113.10","201.5a"]}),
        );
        let granted = graph.node(
            &at,
            "ability",
            "ability",
            json!({
                "source":recipient,"face":graph.id("/faces/0","face"),"abilityKind":"keywords",
                "you":graph.id(&at,"you"),"rules":["113","113.10"]
            }),
        );
        graph.node(&at, "you", "runtime-player", json!({
            "scope":granted,"role":"static-source-controller","rules":["109.5","301.5d","303.4e"]
        }));
        let kp = format!("{at}/keyword");
        let you = graph.node(
            &kp,
            "you",
            "runtime-player",
            json!({
                "scope":granted,"role":"static-source-controller","rules":["109.5",rule]
            }),
        );
        graph.node(
            &kp,
            "keyword",
            "keyword-ability",
            json!({
                "scope":granted,"keyword":keyword,"abilities":["static"],"you":you,"rules":[rule]
            }),
        );
    }
    graph.validate_block(
        binding,
        &base,
        "requires the exact live subtype modifier graph",
    )?;
    Ok(Some(ability))
}
