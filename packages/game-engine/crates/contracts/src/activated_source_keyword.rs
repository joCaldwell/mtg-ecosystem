//! CR 201.5, 400.7, 602.2, 611.2c: a fixed paid self grant on one incarnation.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const INVALID: &str = "requires complete printed mana-only self keyword grants";

pub fn keywords(a: &ActivatedAbility) -> Option<&[KeywordAbility]> {
    let p = &a.program;
    if a.functioning_zone != FunctioningZone::Battlefield
        || !matches!(&a.cost, ActivationCost::Mana { cost } if !cost.generic.is_negative())
        || p.functioning_zone != FunctioningZone::Stack
        || p.timing != SpellTiming::Instant
        || p.resolution != SpellResolution::Instructions
        || !p.targets.is_empty()
        || !p.modes.is_empty()
        || p.additional_sacrifice.is_some()
        || !p.optional_additional_costs.is_empty()
    {
        return None;
    }
    match p.instructions.as_slice() {
        [
            kernel!(ModifyUntil {
                what: source_object!(),
                change: kir::Change::AddKeywords { keywords },
                ..
            }),
        ] if supported_temporary_keywords(keywords) => Some(keywords),
        _ => None,
    }
}
