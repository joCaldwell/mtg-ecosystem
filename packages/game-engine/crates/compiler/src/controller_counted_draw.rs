//! Whole ordinary fixed controller draws; legacy one/two-draw IR stays stable.
use super::*;
use serde_json::{Value, json};
pub(super) fn coalesce(effects: &[Sentence]) -> Result<Option<Quantity>, Failure> {
    let [Sentence { bindings, steps, condition: None, condition_kind: None }] = effects else {
        return Ok(None);
    };
    let [
        Step::Draw {
            who,
            amount: DrawAmount::Fixed { value },
            optional: false,
        },
    ] = steps.as_slice()
    else {
        return Ok(None);
    };
    // Preserve existing source output/admission for legacy one/two-card draws.
    if (1..=2).contains(value) {
        return Ok(None);
    }
    if !bindings.is_empty() || who.player != "you" || who.selection_id.is_some() || who.of.is_some()
    {
        return Err(unsupported(
            "counted draw",
            "requires an unqualified controller",
        ));
    }
    Ok(Some((*value as i64).into()))
}
pub(super) fn validate_source(binding: &Value, identity: &str, raw: &str) -> Result<(), Failure> {
    let fail = || {
        unsupported(
            "binding",
            "requires complete standalone counted controller draw",
        )
    };
    let mut abilities: Value = decode(raw, "abilities")?;
    if abilities[0]["effects"][0].get("bindings").is_some() {
        if abilities[0]["effects"][0]["bindings"] != json!([]) {
            return Err(fail());
        }
        abilities[0]["effects"][0]
            .as_object_mut()
            .ok_or_else(fail)?
            .remove("bindings");
    }
    if abilities[0]["effects"][0]["steps"][0]
        .get("optional")
        .is_some()
    {
        if abilities[0]["effects"][0]["steps"][0]["optional"] != false {
            return Err(fail());
        }
        abilities[0]["effects"][0]["steps"][0]
            .as_object_mut()
            .ok_or_else(fail)?
            .remove("optional");
    }
    let amount = abilities[0]["effects"][0]["steps"][0]["amount"].clone();
    if abilities
        != json!([{"kind":"spell","effects":[{"steps":[{"effect":"draw","who":{"player":"you"},"amount":amount}]}]}])
    {
        return Err(fail());
    }
    let base = "/faces/0/abilities/0";
    let root = format!("card:{identity}{base}");
    let you = format!("{root}#you");
    let refs = binding["references"].as_array().ok_or_else(fail)?;
    let nodes = binding["nodes"].as_array().ok_or_else(fail)?;
    if binding["identity"] != identity
        || binding["fullyBound"] != true
        || binding["referencesBound"] != true
        || binding["diagnostics"] != json!([])
        || binding["counts"] != json!({"obligations":1,"bound":1,"unresolved":0})
        || refs.len() != 1
        || nodes.len() != 7
        || refs[0]["path"] != format!("{base}/effects/0/steps/0/who")
        || refs[0]["expression"] != json!({"player":"you"})
        || refs[0]["binding"] != "bound"
        || refs[0]["target"] != you
        || !nodes.iter().any(|n| {
            n["id"] == you
                && n["node"] == "runtime-player"
                && n["scope"] == format!("{root}#ability")
                && n["role"] == "spell-controller"
        })
        || !nodes.iter().any(|n| {
            n["id"] == format!("{root}/effects/0/steps/0#event")
                && n["node"] == "event"
                && n["operation"] == "draw"
        })
    {
        return Err(fail());
    }
    Ok(())
}
