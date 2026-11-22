use super::*;
use serde_json::{Value, json};

pub(crate) fn lower(
    block: usize,
    result: &ParseResult,
    binding: &Value,
    source: &Value,
    identity: &str,
) -> Result<Option<Ability>, Failure> {
    let line = &result.lines[block];
    if !mtg_contracts::cast_subtype::candidate(&line.text) {
        return Ok(None);
    }
    let raw: Value = decode(line.ability.get(), "trigger.cast_subtype")?;
    let ability = mtg_contracts::cast_subtype::printed(&line.text, &raw, source_block(block)?)
        .ok_or_else(|| {
            unsupported(
                "trigger.cast_subtype",
                "requires the complete cast subtype and draw clause",
            )
        })?;
    let base = format!("/faces/0/abilities/{block}");
    let trigger = format!("{base}/trigger");
    let step = format!("{base}/effects/0/steps/0");
    let mut graph = bindings::Bindings::new(identity, source);
    graph.ability(&base, "triggered", "trigger-controller");
    let you = graph.id(&base, "you");
    let who = json!({"player":"you"});
    graph.reference(&format!("{trigger}/who"), &who, &you, "109.5");
    graph.event(&base, &trigger, "trigger:cast", &you, false);
    graph.result(
        &trigger,
        "objects",
        "same-objects",
        Some(&json!({"cls":"spell"})),
    );
    let effect = &raw["effects"][0]["steps"][0];
    if effect["effect"] == "create-token" {
        graph.reference(&format!("{step}/who"), &who, &you, "109.5");
        graph.event(&base, &step, "create-token", &you, false);
        graph.result(
            &step,
            "objects",
            "new-objects",
            Some(&json!({"cls":"token","types":["creature"]})),
        );
    } else {
        graph.reference(&format!("{step}/who"), &who, &you, "109.5");
        graph.event(&base, &step, "draw", &you, effect["optional"] == true);
        graph.result(
            &step,
            "objects",
            "new-objects",
            Some(&json!({"cls":"card"})),
        );
    }
    graph.result(&step, "quantity", "measured-quantity", None);
    graph.validate_block(
        binding,
        &base,
        "requires exact cast subtype and draw bindings",
    )?;
    Ok(Some(Ability::BoundTrigger(ability)))
}
