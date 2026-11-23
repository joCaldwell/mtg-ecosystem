//! Source-only blocker declaration restrictions, CR 509.1a–b and 509.1g.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const INVALID: &str = "requires exact printed source combat restrictions";
const RESTRICTIONS: [CreatureRestriction; 2] = [
    CreatureRestriction::CantBeBlocked,
    CreatureRestriction::CanBlockOnlyFlying,
];

pub fn static_ability(restriction: CreatureRestriction) -> StaticAbility {
    StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::ModifyCreatures {
            scope: CreatureScope::Source,
            power: Quantity::zero(),
            toughness: Quantity::zero(),
            keywords: vec![],
            restrictions: vec![restriction],
        },
    }
}

pub(super) fn restriction(effect: &StaticEffect) -> Option<CreatureRestriction> {
    match effect {
        StaticEffect::ModifyCreatures {
            scope: CreatureScope::Source,
            restrictions,
            ..
        } => match restrictions.as_slice() {
            [restriction] if RESTRICTIONS.contains(restriction) => Some(*restriction),
            _ => None,
        },
        _ => None,
    }
}
