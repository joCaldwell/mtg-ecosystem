//! "Put on top of its owner's library" (CR 400.3, 401.4).
use super::*;

/// One announced creature to the top of its owner's library.
pub(super) fn lower(
    cx: &mut StepCx<'_>,
    what: &serde_json::Value,
    to: &serde_json::Value,
    order: Option<&str>,
    tapped: Option<bool>,
    who: Option<&serde_json::Value>,
) -> Result<Instruction, Failure> {
    uncounted(cx)?;
    if tapped.is_some() || order.is_some() {
        return Err(step("move-zone", "entry_modifier"));
    }
    if who.is_some() {
        return Err(step("move-zone", "mover"));
    }
    let to = lower_destination(to)?;
    let what = object(what)?;
    Ok(Instruction::move_target(
        bind_creature_target(cx.program, cx.slots, &what)?,
        to,
        cx.source_block,
    ))
}
