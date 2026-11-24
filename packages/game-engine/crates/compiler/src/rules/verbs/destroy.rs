//! "Destroy" (CR 701.8, 701.19c).
use super::*;

pub(super) fn lower(
    cx: &mut StepCx<'_>,
    what: &serde_json::Value,
    optional: Option<bool>,
    regeneration_forbidden: Option<bool>,
) -> Result<Instruction, Failure> {
    uncounted(cx)?;
    let what = object(what)?;
    // CR 608.2h: every member of an "all"/"each" set.
    if is_set(&what) {
        if optional.is_some() || regeneration_forbidden.is_some() {
            return Err(step("destroy", "qualified set"));
        }
        return Ok(cx.act(Verb::Destroy {
            what: lower_board_set(&what, cx.context)?,
            regeneration_forbidden: false,
        }));
    }
    // CR 608.2b: one announced creature, destroyed as a creature.
    if optional.is_none() && regeneration_forbidden.is_none() && is_creature_target(&what) {
        return Ok(Instruction::destroy_target(
            bind_creature_target(cx.program, cx.slots, &what)?,
            false,
            cx.source_block,
        ));
    }
    if regeneration_forbidden == Some(false) {
        return Err(step("destroy", "regeneration_permitted"));
    }
    let slot = bind_permanent_target(cx.program, cx.slots, &what)?;
    match (
        optional.unwrap_or(false),
        regeneration_forbidden.unwrap_or(false),
    ) {
        (false, forbidden) => Ok(Instruction::destroy_target(
            slot,
            forbidden,
            cx.source_block,
        )),
        // CR 608.2d: "you may destroy target permanent".
        (true, false) => Ok(Instruction::optional_destroy_target(
            cx.context,
            slot,
            cx.source_block,
        )),
        (true, true) => Err(step("destroy", "optional_regeneration_forbidden")),
    }
}
