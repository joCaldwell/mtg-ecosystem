//! Bind attached bearer, Equipment, controller and targets as separate operands.
use super::*;
use serde_json::{Value, json};

pub(super) fn lower(
    block: usize,
    result: &ParseResult,
    binding: &Value,
    identity: &str,
    source: &Value,
) -> Result<Option<TriggeredAbility>, Failure> {
    use mtg_contracts::equipped_trigger::{DAMAGE, DIES};
    let line = &result.lines[block];
    if !line.text.starts_with(DAMAGE) && !line.text.starts_with(DIES) {
        return Ok(None);
    }
    let fail = || {
        unsupported(
            "trigger.equipped",
            "requires complete supported equipped trigger operands",
        )
    };
    let raw: Value = decode(line.ability.get(), "trigger.equipped")?;
    let a = mtg_contracts::equipped_trigger::printed(&line.text, &raw, source_block(block)?)
        .ok_or_else(fail)?;
    let base = format!("/faces/0/abilities/{block}");
    let event = format!("{base}/trigger");
    let mut graph = bindings::Bindings::new(identity, source);
    graph.ability(&base, "triggered", "trigger-controller");
    let attached = graph.attachment(&base, &format!("{event}/what"), &raw["trigger"]["what"]);
    let operation = if a.condition == TriggerCondition::EquippedDies {
        "trigger:dies"
    } else {
        "trigger:deals-damage"
    };
    graph.event(&base, &event, operation, &attached, false);
    graph.result(
        &event,
        "objects",
        "same-objects",
        Some(&json!({"types":["creature"]})),
    );
    if a.condition == TriggerCondition::EquippedDies {
        graph.node(&event, "successor", "event-result", json!({
            "event":graph.id(&event,"event"),"result":"objects","cardinality":"one",
            "availability":"if-produced","identity":"zone-successors","description":{"types":["creature"],"cls":"card"},
            "rules":["400.7e","603.6c"]
        }));
    } else {
        graph.result(&event, "player", "same-objects", None);
        graph.result(&event, "quantity", "measured-quantity", None);
    }
    let scope = graph.id(&base, "ability");
    let you = graph.id(&base, "you");
    let object = graph.id("/source", "object");
    for (i, sentence) in raw["effects"]
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
            let path = format!("{base}/effects/{i}/steps/{j}");
            match step["effect"].as_str() {
                Some("create-token") => {
                    bind_creation(&mut graph, &base, &path, &you, step);
                    continue;
                }
                Some("draw") => {
                    graph.reference(&format!("{path}/who"), &step["who"], &you, "109.5");
                    graph.event(&base, &path, "draw", &you, step["optional"] == true);
                    graph.node(&path, "objects", "event-result", json!({
                        "event":graph.id(&path,"event"),"result":"objects","cardinality":
                            if step["amount"]["value"] == 1 { "one" } else { "many" },
                        "availability":"if-produced","identity":"new-objects","description":{"cls":"card"},"rules":["608.2h"]
                    }));
                }
                Some("gain-life") => {
                    graph.reference(&format!("{path}/who"), &step["who"], &you, "109.5");
                    graph.event(&base, &path, "gain-life", &you, false);
                }
                Some("discard") | Some("untap") => {
                    let discard = step["effect"] == "discard";
                    let what = format!("{path}/what");
                    let query=graph.node(&what,"query","selection",json!({
                        "scope":scope,"expression":step["what"],"cardinality":if discard{"one"} else {"many"},
                        "timing":"resolution","selection":"evaluate-reference","rules":["608.2h"]
                    }));
                    let mut inputs = vec![query.clone()];
                    if discard {
                        let player = graph.id(&event, "player");
                        graph.reference(&format!("{path}/who"), &step["who"], &player, "608.2k");
                        inputs.push(player);
                        graph.node(&path,"objects","event-result",json!({
                            "event":graph.id(&path,"event"),"result":"objects","cardinality":"one",
                            "availability":"if-produced","identity":"zone-successors","description":{"cls":"card"},"rules":["400.7j"]
                        }));
                    } else {
                        graph.reference(
                            &format!("{what}/filter/control"),
                            &json!({"player":"you"}),
                            &you,
                            "109.5",
                        );
                    }
                    graph.reference(&what, &step["what"], &query, "608.2h");
                    graph.node(
                        &path,
                        "event",
                        "event",
                        json!({
                            "scope":scope,"operation":step["effect"],"inputs":inputs,
                            "availability":"when-performed","rules":["608.2c"]
                        }),
                    );
                    continue;
                }
                Some("damage") => {
                    graph.reference(&format!("{path}/source"), &step["source"], &object, "201.5");
                    let id = step["to"]["selectionId"].as_str().ok_or_else(fail)?;
                    let target = graph.target(
                        &scope,
                        &format!("{path}/to"),
                        &json!({"ref":"any-target","selectionId":id}),
                        id,
                    );
                    graph.node(&path, "event", "event", json!({
                        "scope":scope,"operation":"damage","inputs":[object,target],"availability":"when-performed","rules":["608.2c"]
                    }));
                }
                _ => return Err(fail()),
            }
            graph.result(&path, "quantity", "measured-quantity", None);
        }
    }
    graph.validate_block(binding, &base, "incomplete equipped trigger binding graph")?;
    Ok(Some(a))
}

pub(super) fn bind_creation(
    graph: &mut bindings::Bindings<'_>,
    base: &str,
    path: &str,
    you: &str,
    step: &Value,
) {
    graph.reference(
        &format!("{path}/who"),
        &json!({"player":"you"}),
        you,
        "109.5",
    );
    graph.event(base, path, "create-token", you, false);
    graph.node(path,"objects","event-result",json!({
        "event":graph.id(path,"event"),"result":"objects","cardinality":if step["count"]["value"]==1{"one"}else{"many"},
        "availability":"if-produced","identity":"new-objects","description":{"cls":"token"},"rules":["608.2h"]
    }));
    graph.result(path, "quantity", "measured-quantity", None);
}
