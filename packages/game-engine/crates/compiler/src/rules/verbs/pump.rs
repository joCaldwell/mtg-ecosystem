//! "Gets +N/+N until end of turn" (CR 611.2c, 613.4c).
use super::*;

/// A fixed signed P/T change until end of turn of a reviewed board set, the
/// exact source or one announced creature.
pub(super) fn lower(
    cx: &mut StepCx<'_>,
    what: &serde_json::Value,
    power: &SignedAmount,
    toughness: &SignedAmount,
    duration: &SourceDuration,
) -> Result<Instruction, Failure> {
    uncounted(cx)?;
    until_end_of_turn(duration)?;
    let what = object(what)?;
    let set = if is_set(&what) {
        lower_reviewed_set(&what, cx.context)?
    } else if is_this_creature(&what) {
        kir::SetExpr::source()
    } else {
        kir::SetExpr::target(bind_creature_target(cx.program, cx.slots, &what)?)
    };
    Ok(Instruction::pump(
        set,
        signed_fixed(power)?,
        signed_fixed(toughness)?,
        cx.source_block,
    ))
}
