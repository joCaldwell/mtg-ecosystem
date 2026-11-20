//! CR 118.12a and 608.2d: the targeted spell's controller chooses whether to pay at
//! resolution. CR 605.3a permits mana abilities during that payment. CR 701.6
//! counters the spell if its controller declines.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::bindings::Bindings;
use serde_json::{Value, json};

pub(super) const SELECTION: &str = "line:0:1";
const BASE: &str = "/faces/0/abilities/0";
const STEP: &str = "/faces/0/abilities/0/effects/0/steps/0";
const CONDITION: &str = "/faces/0/abilities/0/effects/0/condition";

pub(super) fn lower(
    source: &Value,
    result: &ParseResult,
    binding: &Value,
) -> Result<Option<Ability>, Failure> {
    let text = source["oracle_text"].as_str().unwrap_or_default();
    let Some(rest) = text.strip_prefix("Counter target spell unless ") else {
        return Ok(None);
    };
    let fail = || {
        unsupported(
            "counter_unless_generic",
            "requires the complete fixed-generic counter instant",
        )
    };
    let symbol = rest
        .strip_prefix("its controller pays {")
        .and_then(|s| s.strip_suffix("}."))
        .ok_or_else(fail)?;
    let generic: Quantity = symbol.parse().map_err(|_| fail())?;
    let target = json!({"ref":"target","filter":{"cls":"spell"},"count":{"amount":"fixed","value":1},"selectionId":SELECTION});
    let controller = json!({"player":"controller","of":{"ref":"it"}});
    let raw: Value = decode(result.abilities.get(), "abilities")?;
    if generic <= Quantity::zero()
        || source["type_line"] != "Instant"
        || source["keywords"] != json!([])
        || source["mana_cost"]
            .as_str()
            .is_none_or(|cost| cost.is_empty() || cost.contains("{X}"))
        || !matches!(result.lines.as_slice(), [line] if line.text == text)
        || raw
            != json!([{"kind":"spell","effects":[{
                "steps":[{"effect":"counter","what":target}],"conditionKind":"unless",
                "condition":{"condition":"pays","who":controller,"cost":[{"cost":"mana","symbols":[symbol]}]}
            }]}])
    {
        return Err(fail());
    }

    let mut graph = Bindings::new(source["oracle_id"].as_str().ok_or_else(fail)?, source);
    graph.ability(BASE, "spell", "spell-controller");
    let scope = graph.id(BASE, "ability");
    let announced = graph.target(&scope, &format!("{STEP}/what"), &target, SELECTION);
    graph.node(STEP, "event", "event", json!({
        "scope":scope,"operation":"counter","inputs":[announced],"availability":"when-performed",
        "conditional":true,"rules":["608.2c"]
    }));
    let who = format!("{CONDITION}/who");
    let payer = graph.node(
        &who,
        "query",
        "selection",
        json!({
            "scope":scope,"expression":controller,"cardinality":"one","timing":"resolution",
            "selection":"evaluate-reference","rules":["608.2h"]
        }),
    );
    graph.reference_with_rules(
        &format!("{who}/of"),
        &json!({"ref":"it"}),
        &announced,
        &["608.2k", "400.7j", "608.2h"],
    );
    graph.reference(&who, &controller, &payer, "608.2h");
    // The cost-local "you" is bound to this controller query, not the spell's caster.
    graph.reference(
        &format!("{CONDITION}/cost/0/payer"),
        &json!({"player":"you"}),
        &payer,
        "109.5",
    );
    graph.validate(
        binding,
        "requires the complete target, conditional counter and resolution payer graph",
    )?;
    Ok(Some(Ability::CounterUnlessGeneric { generic }))
}

#[cfg(test)]
mod tests;
