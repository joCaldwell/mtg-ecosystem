//! Battlefield activations pay their fixed mana and source tap costs at
//! announcement (CR 113.6, 602.2b, 601.2h) and draw for the ability's
//! controller on resolution, with a discard in the draw/discard programs
//! (CR 113.8, 121.2, 701.9, 608.2c).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

/// Paid source-tap draw-one, or one or two draws followed by one discard.
/// Definition validation also checks the complete source and its cast program.
pub fn activated_artifact_draw_ability(ability: &ActivatedAbility) -> bool {
    let p = &ability.program;
    ability.functioning_zone == FunctioningZone::Battlefield
        && matches!(&ability.cost, ActivationCost::Composite { costs }
            if costs.tap && !costs.sacrifice && costs.mana.is_some()
                && validate_permanent_costs(costs).is_ok())
        && p.functioning_zone == FunctioningZone::Stack
        && p.timing == SpellTiming::Instant
        && p.resolution == SpellResolution::Instructions
        && p.additional_sacrifice.is_none()
        && p.optional_additional_costs.is_empty()
        && p.targets.is_empty()
        && match p.instructions.split_last() {
            Some((last, [])) => draw_one(last),
            Some((
                kernel!(Discard { player: you!(), amount: fixed!(count), random: false } @ 0),
                draws @ ([_] | [_, _]),
            )) => *count == Quantity::from(1) && draws.iter().all(draw_one),
            _ => false,
        }
}

fn draw_one(instruction: &Instruction) -> bool {
    matches!(
        instruction,
        kernel!(Draw { player: you!(), amount: fixed!(count) } @ 0)
            if *count == Quantity::from(1)
    )
}
