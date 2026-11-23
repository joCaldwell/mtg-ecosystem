//! CR 702.29a, 602.2a-b, 701.9a (2026-09-25): fixed mana, discard
//! this hand source, draw one. No cycling observers/modifiers are admitted.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

pub fn hand_discard_draw_activation(a: &ActivatedAbility) -> bool {
    let p = &a.program;
    a.functioning_zone == FunctioningZone::Hand
        && matches!(&a.cost, ActivationCost::DiscardSource { mana } if !mana.generic.is_negative())
        && p.functioning_zone == FunctioningZone::Stack
        && p.timing == SpellTiming::Instant
        && p.resolution == SpellResolution::Instructions
        && p.optional_additional_costs.is_empty()
        && p.additional_sacrifice.is_none()
        && p.targets.is_empty()
        && p.modes.is_empty()
        && matches!(p.instructions.as_slice(), [kernel!(Draw { player: you!(), amount: fixed!(count) })] if *count == Quantity::from(1))
}

// Complete source composition currently qualified for lowering. Neither the
// subtype count nor the mana quantity selects a mechanic or a capacity.
