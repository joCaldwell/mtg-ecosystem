//! "Draw" (CR 121).
use super::*;

/// CR 121.1-121.2: one player draws one or two cards, one at a time.
pub(super) fn lower(
    cx: &mut StepCx<'_>,
    who: &PlayerSelector,
    amount: &DrawAmount,
    optional: bool,
) -> Result<Instruction, Failure> {
    uncounted(cx)?;
    if optional {
        return Err(step("draw", "optional"));
    }
    let DrawAmount::Fixed { value } = amount else {
        return Err(operand("amount", variant(amount)));
    };
    let player = lower_player(
        cx.program,
        cx.slots,
        who,
        cx.context,
        cx.player_binding.event_role(),
    )?;
    // CR 121.1: a group of opponents has no audited draw cell.
    if player == PlayerExpr::EachOpponent {
        return Err(unsupported("definition", "unsupported each-opponent draw"));
    }
    if !(1..=2).contains(value) {
        return Err(unsupported("effects.draw", "unsupported fixed draw count"));
    }
    // CR 121.2: multiple draws are performed one at a time.
    for _ in 1..*value {
        cx.program.instructions.push(Instruction::draw(
            player.clone(),
            Quantity::from(1),
            cx.source_block,
        ));
    }
    Ok(Instruction::draw(
        player,
        Quantity::from(1),
        cx.source_block,
    ))
}
