//! CR 109.5, 118.12a, 603.3a, 603.5 and 608.2d. The cast event supplies
//! the payer, and the trigger controller decides whether to draw after nonpayment.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::bindings::Bindings;
use serde_json::{Value, json};

const BASE: &str = "/faces/0/abilities/0";
const CAST: &str = "/faces/0/abilities/0/trigger";
const DRAW: &str = "/faces/0/abilities/0/effects/0/steps/0";
const PAYER: &str = "/faces/0/abilities/0/effects/0/condition/cost/0/payer";
const CONDITION: &str = "/faces/0/abilities/0/effects/0/condition";

pub(crate) fn lower(
    source: &Value,
    parsed: &ParseResult,
    binding: &Value,
) -> Result<Option<Ability>, Failure> {
    let text = source["oracle_text"].as_str().unwrap_or_default();
    let fail = || {
        unsupported(
            "trigger.cast.payment",
            "requires a complete opponent cast, conditional controller draw and event-caster payment",
        )
    };
    if !text.contains("draw a card unless") {
        // Also fence a rewritten sentence/AST that leaves the archived payer behind.
        if binding["references"].as_array().is_some_and(|refs| {
            refs.iter().any(|r| {
                r["path"] == PAYER
                    && r["target"]
                        .as_str()
                        .is_some_and(|id| id.ends_with("/faces/0/abilities/0/trigger#player"))
            })
        }) {
            return Err(fail());
        }
        return Ok(None);
    }
    let abilities: Value = decode(parsed.abilities.get(), "abilities")?;
    let symbol = abilities
        .pointer("/0/effects/0/condition/cost/0/symbols/0")
        .and_then(Value::as_str)
        .ok_or_else(fail)?;
    let generic: Quantity = symbol.parse().map_err(|_| fail())?;
    if generic <= Quantity::zero()
        || source["type_line"] != "Enchantment"
        || !matches!(parsed.lines.as_slice(), [line] if line.text == text)
    {
        return Err(fail());
    }

    // Definition::validate checks the original AST, including the exact symbol,
    // independently against the retained printed text and the resulting program.
    let mut graph = Bindings::new(source["oracle_id"].as_str().ok_or_else(fail)?, source);
    graph.ability(BASE, "triggered", "trigger-controller");
    let scope = graph.id(BASE, "ability");
    let controller = graph.id(BASE, "you");
    let opponent = json!({"player":"an-opponent"});
    let who = format!("{CAST}/who");
    let query = graph.node(
        &who,
        "query",
        "selection",
        json!({
            "scope":scope,"expression":opponent,"cardinality":"one","timing":"trigger-match",
            "selection":"evaluate-reference","rules":["608.2h"]
        }),
    );
    graph.reference(&who, &opponent, &query, "608.2h");
    graph.event(BASE, CAST, "trigger:cast", &query, false);
    graph.result(
        CAST,
        "objects",
        "same-objects",
        Some(&json!({"cls":"spell"})),
    );
    graph.result(CAST, "player", "same-objects", None);
    let caster = graph.id(CAST, "player");
    let you = json!({"player":"you"});
    graph.reference(&format!("{DRAW}/who"), &you, &controller, "109.5");
    graph.node(
        DRAW,
        "event",
        "event",
        json!({
            "scope":scope,"operation":"draw","inputs":[controller],"availability":"when-performed",
            "optional":true,"decider":controller,"conditional":true,"rules":["608.2c"]
        }),
    );
    graph.result(DRAW, "objects", "new-objects", Some(&json!({"cls":"card"})));
    graph.result(DRAW, "quantity", "measured-quantity", None);
    graph.reference(
        &format!("{CONDITION}/who"),
        &json!({"player":"that-player"}),
        &caster,
        "608.2k",
    );
    graph.reference(PAYER, &you, &caster, "109.5");
    graph.validate_ordered(
        binding,
        "requires the exact cast, draw and inherited payer bindings",
    )?;
    Ok(Some(Ability::CastPayerDraw { generic }))
}

#[cfg(test)]
mod tests;
