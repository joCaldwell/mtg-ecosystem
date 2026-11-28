//! CR 614.15, 605.3b: an activated mana ability can replace its own output
//! after paying its cost. Lower the whole replacement as one production.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::{activation_restrictions, *};
use serde_json::{Value, json};

pub(super) fn lower(
    costs: &[SourceCost],
    effects: &[Value],
    text: &str,
    block: usize,
    player: &activation_restrictions::PlayerAt<'_>,
) -> Result<Option<ManaAbility>, Failure> {
    let reject = || unsupported("compose.mana-replacement", "conditional production shape");
    let [sentence] = effects else {
        return Ok(None);
    };
    let Some([step]) = sentence["steps"].as_array().map(Vec::as_slice) else {
        return Ok(None);
    };
    if step["effect"] != "self-replacement" || step["original"]["effect"] != "add-mana" {
        return Ok(None);
    }
    let Some([instead]) = step["instead"].as_array().map(Vec::as_slice) else {
        return Err(reject());
    };
    if sentence.as_object().map(serde_json::Map::len) != Some(1)
        || step.as_object().map(serde_json::Map::len) != Some(5)
        || step["targetPolicy"] != "retain-original"
        || !matches!(
            step["condition"]["condition"].as_str(),
            Some("control" | "control-count")
        )
        || !matches!(costs, [SourceCost::TapSelf {}])
    {
        return Err(reject());
    }
    let (condition, phrases) = activation_restrictions::condition(
        "/effects/0/steps/0/condition",
        &step["condition"],
        player,
    )?;
    let Ok(kir::ActivationCondition::Controlled {
        filter,
        cmp: kir::Cmp::AtLeast,
        value: minimum,
    }) = kir::ActivationCondition::of(&condition)
    else {
        return Err(reject());
    };
    let (base, first) = output(&step["original"])?;
    let (instead, second) = output(instead)?;
    if !phrases
        .iter()
        .any(|phrase| text == format!("{{T}}: Add {first}. If {phrase}, add {second} instead."))
    {
        return Err(reject());
    }
    Ok(Some(ManaAbility {
        restriction: None,
        condition: None,
        costs: PermanentActivationCosts {
            remove_source_counters: None,
            tap_other: None,
            tap: true,
            mana: None,
            sacrifice: false,
            pay_life: None,
        },
        hybrid_mana: vec![],
        production: ManaProduction::Conditional {
            filter,
            minimum,
            base: Box::new(base),
            instead: Box::new(instead),
        },
        spend_restriction: None,
        riders: vec![],
        source_block: source_block(block)?,
    }))
}

fn output(step: &Value) -> Result<(ManaProduction, String), Failure> {
    let reject = || unsupported("compose.mana-replacement", "conditional output shape");
    if step.as_object().map(serde_json::Map::len) != Some(3)
        || step["effect"] != "add-mana"
        || step["who"] != json!({ "player": "you" })
    {
        return Err(reject());
    }
    let you = PlayerSelector {
        player: "you".into(),
        selection_id: None,
        of: None,
    };
    let production = crate::rules::production(&typed(&step["mana"], "mana")?, &you)?;
    let text = match &production {
        ManaProduction::Fixed { symbols } => symbols.iter().map(|s| format!("{{{s:?}}}")).collect(),
        ManaProduction::AnyColor => "one mana of any color".into(),
        ManaProduction::Choose { amount, .. } => {
            format!(
                "{} mana of any one color",
                activation_restrictions::number(amount)?
            )
        }
        _ => return Err(reject()),
    };
    Ok((production, text))
}
