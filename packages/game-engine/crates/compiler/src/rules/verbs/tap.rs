//! "Tap" and "untap" (CR 701.26).
use super::*;

/// One announced creature, or one announced permanent; definition
/// validation fences the broader selector to the reviewed activated
/// artifact tap program.
pub(super) fn tap(cx: &mut StepCx<'_>, what: &serde_json::Value) -> Result<Instruction, Failure> {
    uncounted(cx)?;
    let what = object(what)?;
    if is_set(&what) {
        return Ok(cx.act(Verb::Tap {
            what: lower_board_set(&what, cx.context)?,
        }));
    }
    let slot = if is_creature_target(&what) {
        bind_creature_target(cx.program, cx.slots, &what)?
    } else {
        bind_permanent_target(cx.program, cx.slots, &what)?
    };
    Ok(Instruction::tap_target(slot, cx.source_block))
}

/// One announced ordinary permanent or creature.
pub(super) fn untap(cx: &mut StepCx<'_>, what: &serde_json::Value) -> Result<Instruction, Failure> {
    uncounted(cx)?;
    let what = object(what)?;
    if is_set(&what) {
        return Ok(cx.act(Verb::Untap {
            what: lower_board_set(&what, cx.context)?,
        }));
    }
    Ok(Instruction::untap_target(
        bind_permanent_target(cx.program, cx.slots, &what)?,
        cx.source_block,
    ))
}
