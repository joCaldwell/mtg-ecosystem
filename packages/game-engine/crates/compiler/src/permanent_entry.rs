use super::*;
use serde_json::{Value, json};

pub(super) fn lower(
    n: usize,
    result: &ParseResult,
    binding: &Value,
    card: &Value,
    identity: &str,
    noun: &str,
) -> Result<Option<Ability>, Failure> {
    let line = &result.lines[n];
    if !mtg_contracts::permanent_entry::candidate(&line.text, noun) {
        return Ok(None);
    }
    let fail = || {
        unsupported(
            "trigger.permanent_entry",
            "requires all exact printed entry clauses and targets",
        )
    };
    let ast: Value = decode(line.ability.get(), "trigger.permanent_entry")?;
    let Some(ability) =
        mtg_contracts::permanent_entry::printed(&line.text, noun, &ast, source_block(n)?)
    else {
        return Ok(None);
    };
    let root = format!("/faces/0/abilities/{n}");
    let event = format!("{root}/trigger");
    let mut graph = bindings::Bindings::new(identity, card);
    graph.ability(&root, "triggered", "trigger-controller");
    let scope = graph.id(&root, "ability");
    let object = graph.id("/source", "object");
    let entry = graph.id(&event, "objects");
    let you = graph.id(&root, "you");
    graph.reference(
        &format!("{event}/what"),
        &ast["trigger"]["what"],
        &object,
        "201.5",
    );
    graph.event(&root, &event, "trigger:enters", &object, false);
    let types = json!({"types":[noun]});
    graph.result(
        &event,
        "objects",
        "same-objects",
        matches!(noun, "artifact" | "creature").then_some(&types),
    );
    for (i, sentence) in ast["effects"]
        .as_array()
        .ok_or_else(fail)?
        .iter()
        .enumerate()
    {
        for (j, step) in sentence["steps"]
            .as_array()
            .ok_or_else(fail)?
            .iter()
            .enumerate()
        {
            let path = format!("{root}/effects/{i}/steps/{j}");
            let op = step["effect"].as_str().ok_or_else(fail)?;
            match op {
                "draw" | "gain-life" | "lose-life" => {
                    graph.reference(&format!("{path}/who"), &step["who"], &you, "109.5");
                    graph.event(&root, &path, op, &you, false);
                    if op == "draw" {
                        graph.node(&path, "objects", "event-result", json!({
                            "event":graph.id(&path,"event"),"result":"objects",
                            "cardinality":if step["amount"]["value"]==1 {"one"} else {"many"},
                            "availability":"if-produced","identity":"new-objects","description":{"cls":"card"},"rules":["608.2h"]
                        }));
                    }
                    graph.result(&path, "quantity", "measured-quantity", None);
                }
                "damage" => {
                    graph.reference_rules(
                        &format!("{path}/source"),
                        &step["source"],
                        &entry,
                        &["608.2k", "400.7j", "608.2h"],
                    );
                    let id = step["to"]["selectionId"].as_str().ok_or_else(fail)?;
                    let target = graph.target(
                        &scope,
                        &format!("{path}/to"),
                        &json!({"ref":"any-target","selectionId":id}),
                        id,
                    );
                    graph.node(&path, "event", "event", json!({"scope":scope,"operation":op,"inputs":[entry,target],"availability":"when-performed","rules":["608.2c"]}));
                    graph.result(&path, "quantity", "measured-quantity", None);
                }
                "sacrifice" => {
                    let mut operands = Vec::new();
                    for (key, cardinality) in
                        [("who", "many"), ("what", "one"), ("chosenBy", "many")]
                    {
                        let field = format!("{path}/{key}");
                        let expr = &step[key];
                        let query=graph.node(&field,"query","selection",json!({
                            "scope":scope,"expression":expr,"cardinality":cardinality,
                            "timing":"resolution","selection":"evaluate-reference","rules":["608.2h"]
                        }));
                        graph.reference(&field, expr, &query, "608.2h");
                        operands.push(query);
                    }
                    graph.node(
                        &path,
                        "event",
                        "event",
                        json!({"scope":scope,"operation":op,"inputs":[operands[1],operands[0]],
                        "availability":"when-performed","rules":["608.2c"]}),
                    );
                }
                "destroy" => {
                    let id = step["what"]["selectionId"].as_str().ok_or_else(fail)?;
                    let target = graph.target(&scope, &format!("{path}/what"), &step["what"], id);
                    graph.event(&root, &path, op, &target, false);
                }
                _ => return Err(fail()),
            }
        }
    }
    graph.validate_block(binding, &root, "incomplete permanent entry graph")?;
    Ok(Some(Ability::BoundTrigger(ability)))
}
