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
        && draw_program(&p.instructions)
}

fn draw_program(instructions: &[Instruction]) -> bool {
    match instructions.split_last() {
        Some((last, [])) => draw_count(last) == Some(1),
        Some((
            kernel!(Discard { player: you!(), amount: Some(fixed!(count)), random: false } @ 0),
            draws,
        )) if *count == Quantity::from(1) => match draws {
            // A two-card draw used to lower to two separate draw instructions.
            [draw] => matches!(draw_count(draw), Some(1..=2)),
            [first, second] => draw_count(first) == Some(1) && draw_count(second) == Some(1),
            _ => false,
        },
        _ => false,
    }
}

fn draw_count(instruction: &Instruction) -> Option<u32> {
    match instruction {
        kernel!(Draw { player: you!(), amount: fixed!(count) } @ 0) => count.to_u32(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kir::{Amount, Context, PlayerExpr, Verb};

    #[test]
    fn fixed_two_draws_then_discard_keep_the_artifact_draw_surface() {
        let player = PlayerExpr::you(Context::Activated);
        let draw = |count| {
            Instruction::act(
                Verb::Draw {
                    player: player.clone(),
                    amount: Amount::fixed(count),
                },
                0,
            )
        };
        let discard = Instruction::act(
            Verb::Discard {
                player: player.clone(),
                amount: Some(Amount::fixed(1)),
                random: false,
            },
            0,
        );
        assert!(draw_program(&[draw(1), draw(1), discard.clone()]));
        assert!(draw_program(&[draw(2), discard.clone()]));
        assert!(!draw_program(&[draw(2)]));
        assert!(!draw_program(&[draw(3), discard]));
    }
}
