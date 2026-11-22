//! CR 601.2b–d: select the mode, then announce the target count, distinct objects
//! and positive division before costs. The archive R4 resolution certifies every
//! reference, step and printed token; this complete printed clause and exact AST
//! supply the lowering.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::resolution::Resolution;
use serde_json::{Value, json};

const TEXT: &str = "Choose one —\n• Exile target creature with power 3 or greater.\n• You draw two cards and you lose 2 life.\n• Distribute two +1/+1 counters among one or two target creatures.";
const EXILE: &str = "line:1:2";
const AMONG: &str = "line:3:13";

fn abilities() -> Value {
    let fixed = |value| json!({"amount":"fixed","value":value});
    let exile = json!({"effect":"exile","what":{"ref":"target","filter":{"types":["creature"],
        "properties":[{"property":"power","comparison":{"op":"ge","value":fixed(3)}}]},
        "count":fixed(1),"selectionId":EXILE}});
    let controller = |effect| json!({"effect":effect,"who":{"player":"you"},"amount":fixed(2)});
    let division = json!({"effect":"distribute-counters","counter":{"type":"+1/+1"},"count":fixed(2),
        "among":{"ref":"target","filter":{"types":["creature"]},"count":{"amount":"chosen","minimum":1,"maximum":fixed(2)},"selectionId":AMONG}});
    let option = |steps: Vec<Value>| json!({"effects":[{"steps":steps}]});
    json!([{"kind":"spell","effects":[{"steps":[{"effect":"modal","count":{"exactly":1},"options":[
        option(vec![exile]),option(vec![controller("draw"),controller("lose-life")]),option(vec![division])
    ]}]}]}])
}

/// Every reference of the block: each mode's own announced target (CR 601.2c,
/// slots in printed order) and the spell's controller for "you" (CR 109.5).
fn refs() -> Value {
    let step = |option: u8, index: u8, operand: &str| {
        format!("/effects/0/steps/0/options/{option}/effects/0/steps/{index}/{operand}")
    };
    let you = json!({"ref":"controller","role":"spell-controller"});
    json!([
        {"path":step(0, 0, "what"),"ref":{"ref":"target","slot":0}},
        {"path":step(1, 0, "who"),"ref":you},
        {"path":step(1, 1, "who"),"ref":you},
        {"path":step(2, 0, "among"),"ref":{"ref":"target","slot":1}},
    ])
}

pub(super) fn validate(
    source: &Value,
    result: &ParseResult,
    record: &Record,
) -> Result<bool, Failure> {
    if source["oracle_text"] != TEXT {
        return Ok(false);
    }
    let raw: Value = decode(result.abilities.get(), "abilities")?;
    if source["type_line"] != "Instant"
        || source["keywords"] != json!([])
        || source["mana_cost"]
            .as_str()
            .is_none_or(|cost| cost.is_empty() || cost.contains("{X}"))
        || !matches!(result.lines.as_slice(), [line] if line.text == TEXT)
        || raw != abilities()
    {
        return Err(unsupported(
            "divided_plus_one_counters",
            "requires the complete choose-one instant and its exact three options",
        ));
    }
    // The resolution, not the diagnostic graph: complete, covering the AST,
    // tiling the printed block, and binding exactly these references.
    let resolution = Resolution::read(
        record.resolution.as_deref(),
        record.resolution_sha256.as_deref(),
    )?;
    resolution.complete()?;
    resolution.covers(&raw)?;
    resolution.ordinary_antecedents(&raw)?;
    resolution.spans(0, TEXT, &raw[0])?;
    let card: Value = decode(
        record.resolution.as_deref().map_or("null", RawValue::get),
        "resolution",
    )?;
    if card["faces"][0]["blocks"][0]["refs"] != refs() {
        return Err(unsupported(
            "resolution",
            "requires each mode's own announced target and the spell controller",
        ));
    }
    Ok(true)
}

#[cfg(test)]
mod tests;
