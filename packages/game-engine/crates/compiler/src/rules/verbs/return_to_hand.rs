//! "Return to its owner's hand" (CR 400.3, 400.7).
use super::*;

/// Every member of an "all"/"each" set, or one announced permanent or
/// creature, to its owner's hand.
pub(super) fn lower(
    cx: &mut StepCx<'_>,
    what: &serde_json::Value,
    to: &str,
    optional: Option<bool>,
) -> Result<Instruction, Failure> {
    uncounted(cx)?;
    if optional.is_some() {
        return Err(step("return-to-hand", "optional"));
    }
    if to != "owner" {
        return Err(step("return-to-hand", "destination"));
    }
    let what = object(what)?;
    if is_set(&what) {
        return Ok(cx.act(Verb::Move {
            what: lower_board_set(&what, cx.context)?,
            to: Destination::OwnerHand,
        }));
    }
    let slot = if is_permanent_target(&what) {
        bind_permanent_target(cx.program, cx.slots, &what)?
    } else {
        bind_creature_target(cx.program, cx.slots, &what)?
    };
    Ok(Instruction::move_target(
        slot,
        kir::Destination::OwnerHand,
        cx.source_block,
    ))
}
