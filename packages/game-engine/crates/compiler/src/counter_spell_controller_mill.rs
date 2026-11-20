//! CR 608.2b/c/h, 701.6, 701.17 (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::bindings::Bindings;
use serde_json::{Value, json};

pub(super) const TARGET_ID: &str = "line:0:1";

pub(super) fn lower(
    source: &Value,
    result: &ParseResult,
    binding: &Value,
) -> Result<Option<Quantity>, Failure> {
    let Some(printed) = source["oracle_text"]
        .as_str()
        .and_then(|text| text.strip_prefix("Counter target spell. Its controller mills "))
    else {
        return Ok(None);
    };
    let fail = || {
        unsupported(
            "counter_spell_controller_mill",
            "requires one complete ordinary instant with a fixed mill count",
        )
    };
    let raw: Value = decode(result.abilities.get(), "abilities")?;
    let count = raw[0]["effects"][1]["steps"][0]["amount"]["value"]
        .as_i64()
        .filter(|n| *n > 0)
        .ok_or_else(fail)?;
    let literal = printed
        .strip_suffix(if count == 1 { " card." } else { " cards." })
        .ok_or_else(fail)?;
    let what = json!({"ref":"target","filter":{"cls":"spell"},"count":{"amount":"fixed","value":1},"selectionId":TARGET_ID});
    let who = json!({"player":"controller","of":{"ref":"it"}});
    if !(matches_fixed_amount(literal, count) || count == 1 && literal == "a")
        || source["type_line"] != "Instant"
        || source["keywords"] != json!(["Mill"])
        || source["mana_cost"]
            .as_str()
            .is_none_or(|cost| cost.is_empty() || cost.contains("{X}"))
        || raw
            != json!([{"kind":"spell","effects":[
                {"steps":[{"effect":"counter","what":what}]},
                {"steps":[{"effect":"mill","who":who,"amount":{"amount":"fixed","value":count}}]}
            ]}])
    {
        return Err(fail());
    }

    let base = "/faces/0/abilities/0";
    let counter = "/faces/0/abilities/0/effects/0/steps/0";
    let mill = "/faces/0/abilities/0/effects/1/steps/0";
    let mut graph = Bindings::new(source["oracle_id"].as_str().ok_or_else(fail)?, source);
    graph.ability(base, "spell", "spell-controller");
    let scope = graph.id(base, "ability");
    let target = graph.target(&scope, &format!("{counter}/what"), &what, TARGET_ID);
    graph.event(base, counter, "counter", &target, false);
    let player = format!("{mill}/who");
    let query = graph.node(&player, "query", "selection", json!({"scope":scope,"expression":who,
        "cardinality":"one","timing":"resolution","selection":"evaluate-reference","rules":["608.2h"]}));
    graph.reference_rules(
        &format!("{player}/of"),
        &who["of"],
        &target,
        &["608.2k", "400.7j", "608.2h"],
    );
    graph.reference(&player, &who, &query, "608.2h");
    graph.event(base, mill, "mill", &query, false);
    graph.node(mill, "objects", "event-result", json!({"event":graph.id(mill,"event"),"result":"objects",
        "cardinality":"many","availability":"if-produced","identity":"new-objects","description":{"cls":"card"},"rules":["608.2h"]}));
    graph.result(mill, "quantity", "measured-quantity", None);
    graph.validate(
        binding,
        "requires both counter and controller references to the same target spell",
    )?;
    Ok(Some(count.into()))
}
