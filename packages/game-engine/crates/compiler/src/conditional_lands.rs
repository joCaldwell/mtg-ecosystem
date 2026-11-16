//! Complete self-entry family, frozen CR 109.5, 205.4c, 305.6, 614.12:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(super) fn lower(effect: &SourceStaticEffect) -> Result<Option<StaticAbility>, Failure> {
    let SourceStaticEffect::EntersTapped { what, condition_kind, condition } = effect else {
        return Ok(None);
    };
    if condition_kind.is_none() && condition.is_none() { return Ok(None); }
    let bad = || unsupported("static.entry", "unsupported complete conditional land entry");
    let condition = condition.as_ref().ok_or_else(bad)?;
    let operand = &condition["comparison"]["value"]["value"];
    let minimum: Quantity = match operand {
        Value::Number(n) => n.to_string().parse().ok(),
        _ => None,
    }.filter(|q: &Quantity| !q.is_negative()).ok_or_else(bad)?;
    if *what != json!({"ref":"this","noun":"land"})
        || condition_kind.as_ref() != Some(&json!("unless"))
        || *condition != json!({"condition":"control-count","who":{"player":"you"},
            "of":{"supertypes":["basic"],"types":["land"]},
            "comparison":{"op":"ge","value":{"amount":"fixed","value":operand}}})
    { return Err(bad()); }
    Ok(Some(StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::EntersTappedUnlessControllerBasicLands { minimum },
    }))
}

pub(super) fn bind(effect: &SourceStaticEffect, binding: &Value, identity: &str, block: u32, card: &Source) -> Result<(), Failure> {
    if lower(effect)?.is_none() { return Ok(()); }
    let bad = || unsupported("binding.entry", "missing complete source/entering-controller binding closure");
    let path = format!("/faces/0/abilities/{block}");
    let root = format!("card:{identity}");
    let source = format!("{root}/source#object");
    let face = format!("{root}/faces/0#face");
    let ability = format!("{root}{path}#ability");
    let you = format!("{root}{path}#you");
    let entering = format!("{root}{path}/effect#entering-controller");
    let nodes = binding["nodes"].as_array().ok_or_else(bad)?;
    let required = [
        json!({"id":source,"path":"/source","node":"source","identity":identity}),
        json!({"id":face,"path":"/faces/0","node":"face","source":source,"sourceFaceIndex":0,"name":card.name,"layout":card.layout,"typeLine":card.type_line,"manaCost":card.mana_cost}),
        json!({"id":ability,"path":path,"node":"ability","source":source,"face":face,"abilityKind":"static","you":you}),
        json!({"id":you,"path":path,"node":"runtime-player","scope":ability,"role":"static-source-controller"}),
        json!({"id":entering,"path":format!("{path}/effect"),"node":"runtime-player","scope":ability,"role":"entering-controller"}),
    ];
    if binding["identity"] != identity || binding["fullyBound"] != true
        || binding["referencesBound"] != true || binding["diagnostics"] != json!([])
        || required.iter().any(|expected| {
            let matches: Vec<_> = nodes.iter().filter(|n| n["id"] == expected["id"]).collect();
            matches.len() != 1 || { let mut actual = matches[0].clone(); actual.as_object_mut().unwrap().remove("rules"); actual != *expected }
        }) { return Err(bad()); }
    let references = binding["references"].as_array().ok_or_else(bad)?;
    for (suffix, expression, target) in [
        ("/effect/what", json!({"ref":"this","noun":"land"}), source),
        ("/effect/condition/who", json!({"player":"you"}), entering),
    ] {
        let matches: Vec<_> = references.iter().filter(|r| r["path"] == format!("{path}{suffix}")).collect();
        if matches.len() != 1 || matches[0]["binding"] != "bound"
            || matches[0]["expression"] != expression || matches[0]["target"] != target { return Err(bad()); }
    }
    Ok(())
}

#[cfg(test)]
#[path = "conditional_lands_tests.rs"]
mod tests;
