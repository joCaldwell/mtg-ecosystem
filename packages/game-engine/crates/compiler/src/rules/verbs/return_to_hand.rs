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
    if to == "your" {
        let slot = bind_own_graveyard_target(cx.program, cx.slots, &object(what)?, false)?;
        let movement = Instruction::move_target(slot, Destination::OwnerHand, cx.source_block);
        return match optional {
            None => Ok(movement),
            // CR 603.5, 603.3d: announce the target when the ability is
            // stacked, then offer the controller the choice at resolution.
            Some(true) => {
                let Instruction::Kernel { node, source_block } = movement;
                Ok(Instruction::Kernel {
                    node: kir::Node::Optional {
                        chooser: PlayerExpr::you(cx.context),
                        node: Box::new(node),
                    },
                    source_block,
                })
            }
            Some(false) => Err(step("return-to-hand", "optional")),
        };
    }
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
    // CR 400.7: "this <permanent>" names the source's battlefield incarnation.
    if is_this_permanent(&what) && cx.context != Context::Spell {
        return Ok(cx.act(Verb::Move {
            what: SetExpr::source(),
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
