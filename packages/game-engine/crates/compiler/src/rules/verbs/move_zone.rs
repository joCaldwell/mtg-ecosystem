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
    // CR 110.2a and 614.1c: a creature card from your graveyard enters
    // under your control, tapped when the move specifies it.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    if *to == serde_json::json!({"zone": "battlefield"}) {
        if order.is_some() || who.is_some() || tapped == Some(false) {
            return Err(step("move-zone", "battlefield_entry_modifier"));
        }
        let slot = bind_own_graveyard_target(cx.program, cx.slots, &object(what)?, true)?;
        return Ok(Instruction::move_target(
            slot,
            Destination::Battlefield {
                controller: PlayerExpr::you(cx.context),
                tapped: tapped == Some(true),
            },
            cx.source_block,
        ));
    }
    if tapped.is_some() || order.is_some() {
        return Err(step("move-zone", "entry_modifier"));
    }
    if who.is_some() {
        return Err(step("move-zone", "mover"));
    }
    let to = lower_destination(to)?;
    let what = object(what)?;
    // CR 400.7: "this <permanent>" names the source's battlefield incarnation.
    if is_this_permanent(&what) && cx.context != Context::Spell {
        return Ok(cx.act(Verb::Move {
            what: SetExpr::source(),
            to,
        }));
    }
    Ok(Instruction::move_target(
        bind_creature_target(cx.program, cx.slots, &what)?,
        to,
        cx.source_block,
    ))
}
