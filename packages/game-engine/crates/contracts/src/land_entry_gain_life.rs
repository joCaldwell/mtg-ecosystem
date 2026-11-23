//! CR 109.5, 119.3, 603.6a and 608.2c: the exact entrant observes its own
//! entry, then the trigger's controller gains the fixed life amount.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const INVALID: &str = "requires complete mandatory controller life gain on land entry";

pub(super) fn present(definition: &Definition) -> bool {
    definition.characteristics.kind == CardKind::Land
        && definition.triggered.iter().any(|trigger| {
            trigger.condition == TriggerCondition::SelfEntersBattlefield
                && trigger
                    .program
                    .instructions
                    .iter()
                    .any(|instruction| matches!(instruction, kernel!(GainLife { .. })))
        })
}

fn enters_tapped() -> StaticAbility {
    StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::EntersTapped,
    }
}
