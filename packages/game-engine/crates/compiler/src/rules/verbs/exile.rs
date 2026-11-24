//! "Exile" (CR 406.2, 400.3, 400.7).
use super::*;

/// Every member of an "all"/"each" set, or one announced permanent.
pub(super) fn lower(cx: &mut StepCx<'_>, what: &serde_json::Value) -> Result<Instruction, Failure> {
    uncounted(cx)?;
    let what = object(what)?;
    if is_set(&what) {
        return Ok(cx.act(Verb::Move {
            what: lower_board_set(&what, cx.context)?,
            to: Destination::Exile,
        }));
    }
    Ok(Instruction::move_target(
        bind_permanent_target(cx.program, cx.slots, &what)?,
        kir::Destination::Exile,
        cx.source_block,
    ))
}
