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
    if is_this_permanent(&what) && cx.context != Context::Spell {
        return Ok(cx.act(Verb::Tap {
            what: SetExpr::source(),
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
    if is_this_permanent(&what) && cx.context != Context::Spell {
        return Ok(cx.act(Verb::Untap {
            what: SetExpr::source(),
        }));
    }
    Ok(Instruction::untap_target(
        match continuous::followup_target(cx, &what) {
            Some(slot) => slot,
            None => bind_permanent_target(cx.program, cx.slots, &what)?,
        },
        cx.source_block,
    ))
}
