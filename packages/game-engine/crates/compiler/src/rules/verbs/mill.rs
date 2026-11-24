//! "Mill" (CR 701.17).
use super::*;

/// CR 701.17a: "you" or one announced player mills a fixed count.
pub(super) fn lower(
    cx: &mut StepCx<'_>,
    who: &serde_json::Value,
    amount: &serde_json::Value,
) -> Result<Instruction, Failure> {
    uncounted(cx)?;
    let player = match typed::<ast::PlayerRef>(who, "player")? {
        ast::PlayerRef::You(_) => PlayerExpr::you(cx.context),
        ast::PlayerRef::TargetPlayer(target) => match *target {
            ast::PlayerRefTargetPlayer {
                count: None,
                up_to: None,
                random: None,
                selection_id: Some(id),
            } if !id.is_empty() => PlayerExpr::target(bind_target(
                cx.program,
                cx.slots,
                &id,
                TargetDomain::AnyPlayer,
            )?),
            _ => return Err(operand("player", "target_player:qualified")),
        },
        other => return Err(operand("player", variant(&other))),
    };
    let count = match typed::<ast::Amount>(amount, "amount")? {
        ast::Amount::Fixed(n) if u32::try_from(n.value).is_ok() => Quantity::from(n.value),
        other => return Err(operand("amount", variant(&other))),
    };
    Ok(Instruction::mill(
        player,
        Amount::Fixed { value: count },
        cx.source_block,
    ))
}
