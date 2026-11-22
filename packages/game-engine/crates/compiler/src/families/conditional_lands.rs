//! Complete self-entry family, frozen CR 109.5, 205.4c, 305.6, 614.12:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

// Deliberately separate from SourceCost: entry replacement is no casting cost.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "cost", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum SourceEntryCost {
    PayLife { amount: Value },
    Reveal { what: Value },
}

pub(crate) fn lower(effect: &SourceStaticEffect) -> Result<Option<StaticAbility>, Failure> {
    if let Some(reveal) = reveal_land_entry::lower(effect)? {
        return Ok(Some(reveal));
    }
    if let SourceStaticEffect::EntryChoice {
        what,
        who,
        costs,
        declined,
    } = effect
    {
        let bad = || {
            unsupported(
                "static.entry",
                "unsupported complete optional-life land entry",
            )
        };
        let [SourceEntryCost::PayLife { amount }] = costs.as_slice() else {
            return Err(bad());
        };
        // serde_json's exact native integer range is the SOURCE decoder limit.
        // Never lower a float (including rounded huge JSON numbers) to Quantity.
        // IR quantities remain arbitrary precision, independently of this limit.
        let value = amount["value"].as_u64().ok_or_else(bad)?;
        if *what != json!({"ref":"this","noun":"land"})
            || *who != json!({"player":"you"})
            || *amount != json!({"amount":"fixed","value":value})
            || *declined != json!({"tapped":true})
        {
            return Err(bad());
        }
        return Ok(Some(StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::EntersTappedUnlessPayLife {
                amount: value.to_string().parse().map_err(|_| bad())?,
            },
        }));
    }
    let SourceStaticEffect::EntersTapped {
        what,
        condition_kind,
        condition,
    } = effect
    else {
        return Ok(None);
    };
    if condition_kind.is_none() && condition.is_none() {
        return Ok(None);
    }
    if let Some(entry) = checkland_entry::lower(effect)? {
        return Ok(Some(entry));
    }
    if let Some(entry) = other_land_entry::lower(effect)? {
        return Ok(Some(entry));
    }
    if let Some(entry) = opponent_count_entry::lower(effect)? {
        return Ok(Some(entry));
    }
    let bad = || {
        unsupported(
            "static.entry",
            "unsupported complete conditional land entry",
        )
    };
    let condition = condition.as_ref().ok_or_else(bad)?;
    let operand = &condition["comparison"]["value"]["value"];
    let minimum: Quantity = match operand {
        Value::Number(n) => n.to_string().parse().ok(),
        _ => None,
    }
    .filter(|q: &Quantity| !q.is_negative())
    .ok_or_else(bad)?;
    if *what != json!({"ref":"this","noun":"land"})
        || condition_kind.as_ref() != Some(&json!("unless"))
        || *condition
            != json!({"condition":"control-count","who":{"player":"you"},
            "of":{"supertypes":["basic"],"types":["land"]},
            "comparison":{"op":"ge","value":{"amount":"fixed","value":operand}}})
    {
        return Err(bad());
    }
    Ok(Some(StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::EntersTappedUnlessControllerBasicLands { minimum },
    }))
}

pub(crate) fn bind(
    effect: &SourceStaticEffect,
    binding: &Value,
    identity: &str,
    block: u32,
    card: &Source,
) -> Result<(), Failure> {
    if !legacy_binding(effect)? {
        return Ok(());
    }
    let bad = || {
        unsupported(
            "binding.entry",
            "missing complete source/entering-controller binding closure",
        )
    };
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
    if binding["identity"] != identity
        || binding["fullyBound"] != true
        || binding["referencesBound"] != true
        || binding["diagnostics"] != json!([])
        || required.iter().any(|expected| {
            let matches: Vec<_> = nodes.iter().filter(|n| n["id"] == expected["id"]).collect();
            matches.len() != 1 || {
                let mut actual = matches[0].clone();
                actual.as_object_mut().unwrap().remove("rules");
                actual != *expected
            }
        })
    {
        return Err(bad());
    }
    let references = binding["references"].as_array().ok_or_else(bad)?;
    let optional = matches!(effect, SourceStaticEffect::EntryChoice { .. });
    let mut obligations = vec![
        ("/effect/what", json!({"ref":"this","noun":"land"}), source),
        (
            if optional {
                "/effect/who"
            } else {
                "/effect/condition/who"
            },
            json!({"player":"you"}),
            entering.clone(),
        ),
    ];
    if optional {
        obligations.push(("/effect/costs/0/payer", json!({"player":"you"}), entering));
    }
    for (suffix, expression, target) in obligations {
        let matches: Vec<_> = references
            .iter()
            .filter(|r| r["path"] == format!("{path}{suffix}"))
            .collect();
        if matches.len() != 1
            || matches[0]["binding"] != "bound"
            || matches[0]["expression"] != expression
            || matches[0]["target"] != target
        {
            return Err(bad());
        }
    }
    Ok(())
}

fn legacy_binding(effect: &SourceStaticEffect) -> Result<bool, Failure> {
    Ok(lower(effect)?.is_some_and(|entry| {
        !matches!(
            entry.effect,
            StaticEffect::EntersTappedUnlessControllerOtherLands { .. }
                | StaticEffect::EntersTappedUnlessOpponentCount { .. }
                | StaticEffect::EntersTappedUnlessRevealLandTypes { .. }
                | StaticEffect::EntersTappedUnlessControllerLandTypes { .. }
        )
    }))
}

// Whole-source reference authority: no unconsumed or contradictory obligations.
pub(crate) fn validate_binding_closure(
    abilities: &[Ability],
    binding: &Value,
    identity: &str,
) -> Result<(), Failure> {
    let mut conditional = false;
    let mut optional = false;
    for a in abilities {
        if let Ability::Static { effect, .. } = a {
            conditional |= legacy_binding(effect)?;
            optional |= matches!(effect, SourceStaticEffect::EntryChoice { .. });
        }
    }
    if !conditional {
        return Ok(());
    }
    let bad = || {
        unsupported(
            "binding.entry",
            "incomplete conditional-entry binding obligation set",
        )
    };
    // This family has exactly one replacement on the whole pure source. Do
    // not imply ordering support by accepting another optional/intrinsic block.
    if optional && abilities.len() != 1 {
        return Err(bad());
    }
    let mut expected = Vec::new();
    for (block, a) in abilities.iter().enumerate() {
        let Ability::Static { effect, .. } = a else {
            return Err(bad());
        };
        let what = match effect {
            SourceStaticEffect::EntersTapped { what, .. }
            | SourceStaticEffect::EntryChoice { what, .. } => what,
            _ => return Err(bad()),
        };
        let path = format!("/faces/0/abilities/{block}/effect");
        expected.push(json!({"path":format!("{path}/what"),"expression":what,"binding":"bound","target":format!("card:{identity}/source#object")}));
        if legacy_binding(effect)? {
            let who = if optional { "who" } else { "condition/who" };
            expected.push(json!({"path":format!("{path}/{who}"),"expression":{"player":"you"},"binding":"bound","target":format!("card:{identity}{path}#entering-controller")}));
            if optional {
                expected.push(json!({"path":format!("{path}/costs/0/payer"),"expression":{"player":"you"},"binding":"bound","target":format!("card:{identity}{path}#entering-controller")}));
            }
        }
    }
    // bind() already validates every required node exactly (ignoring only
    // diagnostic rules). Reject unconsumed/duplicate/foreign nodes as well.
    if optional
        && binding["nodes"]
            .as_array()
            .is_none_or(|n| n.len() != 2 + 3 * abilities.len())
    {
        return Err(bad());
    }
    let references = binding["references"].as_array().ok_or_else(bad)?;
    if references.len() != expected.len()
        || binding["counts"]
            != json!({"obligations":expected.len(),"bound":expected.len(),"unresolved":0})
    {
        return Err(bad());
    }
    for wanted in &expected {
        let matching: Vec<_> = references
            .iter()
            .filter(|r| r["path"] == wanted["path"])
            .collect();
        if matching.len() != 1 {
            return Err(bad());
        }
        let mut actual = matching[0].clone();
        let Some(object) = actual.as_object_mut() else {
            return Err(bad());
        };
        object.remove("rules");
        if actual != *wanted {
            return Err(bad());
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "conditional_lands_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "optional_life_lands_tests.rs"]
mod optional_life_tests;
