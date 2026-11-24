//! "Draw" (CR 121).
use super::*;

/// CR 121.1-121.2c: draw one card at a time, in APNAP order for each player.
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
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
    let player = match typed::<ast::PlayerRef>(who, "player")? {
        ast::PlayerRef::EachPlayer(_) => PlayerExpr::EachPlayer,
        who => lower_player_ref(
            cx.program,
            cx.slots,
            who,
            cx.context,
            cx.player_binding.event_role(),
        )?,
    };
    let counted = player.is_you() || player == PlayerExpr::EachPlayer;
    match (amount, &player) {
        (DrawAmount::Fixed { value: 1..=2 }, PlayerExpr::Is { .. }) => {
            // CR 121.2: one ordinary instruction for each single draw.
            if matches!(amount, DrawAmount::Fixed { value: 2 }) {
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
        (DrawAmount::Fixed { value }, _) if counted && *value > 0 => Ok(cx.act(Verb::Draw {
            player,
            amount: Amount::fixed(i64::from(*value)),
        })),
        (DrawAmount::X {}, _) if counted => Ok(cx.act(Verb::Draw {
            player,
            amount: Amount::announced_x(),
        })),
        (DrawAmount::Fixed { .. } | DrawAmount::X {}, _) => Err(step("draw", "recipient_or_count")),
        (DrawAmount::ThatMuch {}, _) => Err(operand("amount", variant(amount))),
    }
}
