//! Targeted battlefield entry and owner-zone moves (CR 400.3, 401.4, 614.1c).
use super::*;

/// Move the announced card or the source to the printed destination.
pub(super) fn lower(
    cx: &mut StepCx<'_>,
    what: &serde_json::Value,
    to: &serde_json::Value,
    order: Option<&str>,
    tapped: Option<bool>,
    who: Option<&serde_json::Value>,
    controller: Option<&str>,
) -> Result<Instruction, Failure> {
    uncounted(cx)?;
    // CR 110.2a and 614.1c: a permanent card from your graveyard enters
    // under your control, tapped when the move specifies it.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    if *to == serde_json::json!({"zone": "battlefield"}) {
        if order.is_some()
            || who.is_some()
            || tapped == Some(false)
            || !matches!(controller, None | Some("you"))
        {
            return Err(step("move-zone", "battlefield_entry_modifier"));
        }
        let slot = bind_graveyard_target(cx.program, cx.slots, &object(what)?, false)?;
        let target = &cx.program.targets[slot as usize];
        let controller = if controller.is_some()
            || !matches!(target.domain, TargetDomain::GraveyardCard { .. })
        {
            PlayerExpr::you(cx.context)
        } else {
            PlayerExpr::OwnerOf {
                object: kir::Ref::Target { slot },
            }
        };
        return Ok(Instruction::move_target(
            slot,
            Destination::Battlefield {
                controller,
                tapped: tapped == Some(true),
            },
            cx.source_block,
        ));
    }
    if tapped.is_some() || order.is_some() {
        return Err(step("move-zone", "entry_modifier"));
    }
    if who.is_some() || controller.is_some() {
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
    let slot = if to == Destination::OwnerLibraryTop {
        bind_creature_target(cx.program, cx.slots, &what)?
    } else {
        bind_permanent_target(cx.program, cx.slots, &what)?
    };
    Ok(Instruction::move_target(slot, to, cx.source_block))
}
