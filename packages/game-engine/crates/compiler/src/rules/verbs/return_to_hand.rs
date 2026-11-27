//! "Return to its owner's hand" (CR 400.3, 400.7).
use super::*;

/// Every member of an "all"/"each" set, or one announced permanent or
/// creature, to its owner's hand; or a card from your graveyard to your
/// hand.
pub(super) fn lower(
    cx: &mut StepCx<'_>,
    movement: &ast::EffectReturnToHand,
) -> Result<Instruction, Failure> {
    let ast::EffectReturnToHand {
        optional,
        decided_by,
        starting_with,
        for_each,
        for_each_beyond,
        for_each_vote,
        duration,
        what,
        to,
        from,
        who,
    } = movement;
    if starting_with.is_some()
        || for_each.is_some()
        || for_each_beyond.is_some()
        || for_each_vote.is_some()
        || duration.is_some()
    {
        return Err(step("return-to-hand", "qualified"));
    }
    if who.is_some() || decided_by.is_some() {
        return Err(step("return-to-hand", "mover"));
    }
    if from.is_some() {
        return Err(step("return-to-hand", "from"));
    }
    let optional = optional.is_some().then_some(true);
    let to = to.as_str();
    uncounted(cx)?;
    if to == "your" {
        let what = what.as_ref().clone();
        if let Some(choice) = super::move_zone::chosen(cx, &what, Destination::OwnerHand)? {
            return match (optional, choice) {
                (None, choice) => Ok(choice),
                (
                    Some(true),
                    Instruction::Kernel {
                        node: kir::Node::Do(Verb::Search { player, mut search }),
                        source_block,
                    },
                ) => {
                    search.optional = true;
                    Ok(Instruction::act(
                        Verb::Search { player, search },
                        source_block,
                    ))
                }
                _ => Err(step("return-to-hand", "optional")),
            };
        }
        let slot = bind_graveyard_target(cx.program, cx.slots, &what, true)?;
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
    let what = what.as_ref().clone();
    // CR 109.5, 115.1d, 400.3, 608.2c-d: choose the permanent when returning it.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    if let ast::ObjectRef::Filter(selection) = &what
        && matches!(&*selection.count, ast::Amount::Fixed(count) if count.value == 1)
        && selection.up_to.is_none()
        && selection.group_constraints.is_none()
        && selection.random.is_none()
        && selection.filter.control == Some(ast::ObjectFilterControl::You)
    {
        return Ok(Instruction::Kernel {
            node: kir::Node::ChooseObject {
                from: SetExpr::battlefield(lower_filter(&selection.filter, cx.context)?),
                action: kir::ObjectChoiceAction::ReturnToOwnerHand,
            },
            source_block: cx.source_block,
        });
    }
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
    let slot = if matches!(&what, ast::ObjectRef::Target(t) if t.filter.zone.is_some()) {
        bind_graveyard_target(cx.program, cx.slots, &what, false)?
    } else if is_creature_target(&what) {
        bind_creature_target(cx.program, cx.slots, &what)?
    } else {
        bind_permanent_target(cx.program, cx.slots, &what)?
    };
    Ok(Instruction::move_target(
        slot,
        kir::Destination::OwnerHand,
        cx.source_block,
    ))
}
