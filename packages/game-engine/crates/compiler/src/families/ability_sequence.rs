use super::*;
use mtg_contracts::ability_sequence::BoundAbility;
use serde_json::{Value, json};

pub(crate) fn lower(
    block: usize,
    result: &ParseResult,
    source: &Value,
    binding: &Value,
    identity: &str,
    kind: CardKind,
) -> Result<Option<Ability>, Failure> {
    let line = &result.lines[block];
    if !mtg_contracts::ability_sequence::candidate(&line.text, kind) {
        return Ok(None);
    }
    let raw: Value = decode(line.ability.get(), "ability sequence")?;
    let fail = || {
        unsupported(
            "ability sequence",
            "requires every printed scalar step and complete cost",
        )
    };
    let a = mtg_contracts::ability_sequence::printed(&line.text, &raw, source_block(block)?, kind)
        .ok_or_else(fail)?;
    let base = format!("/faces/0/abilities/{block}");
    let mut graph = bindings::Bindings::new(identity, source);
    let (role, category) = match a {
        BoundAbility::Trigger(_) => ("trigger-controller", "triggered"),
        BoundAbility::Activation(_) => ("activator", "activated"),
    };
    graph.ability(&base, category, role);
    let object = graph.id("/source", "object");
    let scope = graph.id(&base, "ability");
    let you = graph.id(&base, "you");
    let player = json!({"player":"you"});
    if let Some(event) = raw["trigger"]["trigger"].as_str() {
        let path = format!("{base}/trigger");
        let what = &raw["trigger"]["what"];
        graph.reference(&format!("{path}/what"), what, &object, "201.5");
        graph.event(&base, &path, &format!("trigger:{event}"), &object, false);
        graph.result(
            &path,
            "objects",
            "same-objects",
            Some(&json!({"types":[what["noun"]]})),
        );
        if event == "dies" {
            graph.node(&path,"successor","event-result",json!({
                "event":graph.id(&path,"event"),"result":"objects","cardinality":"one",
                "availability":"if-produced","identity":"zone-successors","description":{"types":["creature"],"cls":"card"},
                "rules":["400.7e","603.6c"]
            }));
        }
    } else {
        for (i, cost) in raw["costs"].as_array().ok_or_else(fail)?.iter().enumerate() {
            let path = format!("{base}/costs/{i}");
            graph.reference(&format!("{path}/payer"), &player, &you, "109.5");
            match cost["cost"].as_str() {
                Some("mana") => (),
                Some("tap-self") => graph.reference(
                    &format!("{path}/source"),
                    &json!({"ref":"self"}),
                    &object,
                    "201.5",
                ),
                Some("sacrifice") => {
                    graph.reference(&format!("{path}/what"), &cost["what"], &object, "201.5");
                    graph.node(&path,"event","event",json!({"scope":scope,"operation":"cost:sacrifice","inputs":[],"availability":"when-performed","rules":["608.2c"]}));
                    graph.result(&path, "quantity", "measured-quantity", None);
                }
                _ => return Err(fail()),
            }
        }
    }
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
            let op = step["effect"].as_str().ok_or_else(fail)?;
            if op == "create-token" {
                equipped_trigger::bind_creation(&mut graph, &base, &path, &you, step);
            } else if matches!(op, "scry" | "surveil") {
                graph.node(&path,"event","event",json!({"scope":scope,"operation":op,"inputs":[],"availability":"when-performed","rules":["608.2c"]}));
            } else {
                graph.reference(&format!("{path}/who"), &player, &you, "109.5");
                graph.event(&base, &path, op, &you, false);
                if op == "draw" {
                    graph.result(
                        &path,
                        "objects",
                        "new-objects",
                        Some(&json!({"cls":"card"})),
                    );
                }
                graph.result(&path, "quantity", "measured-quantity", None);
            }
        }
    }
    graph.validate_block(binding, &base, "incomplete scalar ability binding graph")?;
    Ok(Some(match a {
        BoundAbility::Trigger(a) => Ability::BoundTrigger(a),
        BoundAbility::Activation(a) => Ability::BoundActivation(a),
    }))
}
