//! CR 103.6a, 113.6, 400.7. The owner may place this exact kept hand card
//! before normal play, with the static ability functioning only from hand.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub fn opening_placement_ability() -> StaticAbility {
    StaticAbility {
        functioning_zone: FunctioningZone::Hand,
        effect: StaticEffect::MayBeginOnBattlefield,
    }
}
