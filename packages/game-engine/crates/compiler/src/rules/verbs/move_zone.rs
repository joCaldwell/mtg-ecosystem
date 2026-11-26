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
        if controller.is_none()
            && matches!(
                cx.program.targets[slot as usize].domain,
                TargetDomain::GraveyardCard { .. }
            )
        {
            return Err(step("move-zone", "battlefield_entry_controller"));
        }
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
    if who.is_some() || controller.is_some() {
        return Err(step("move-zone", "mover"));
    }
    let what = object(what)?;
    if matches!(&what, ast::ObjectRef::Target(t) if t.filter.zone.as_deref().is_some_and(|zone| zone.zone == ast::Zone::Graveyard))
    {
        // CR 400.3, 401.4: "your library" denotes the owner’s library
        // only when the target is restricted to your graveyard.
        let own_library =
            *to == serde_json::json!({"zone": "library", "owner": "your", "position": "top"});
        let destination = if own_library {
            Destination::OwnerLibraryTop
        } else {
            lower_destination(to)?
        };
        if destination != Destination::OwnerLibraryTop {
            return Err(step("move-zone", "graveyard_target_destination"));
        }
        let slot = bind_graveyard_target(cx.program, cx.slots, &what, own_library)?;
        return Ok(Instruction::move_target(slot, destination, cx.source_block));
    }
    let to = lower_destination(to)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn foreign_graveyard_entry_requires_an_explicit_controller() {
        for owner in ["your", "any", "an-opponent"] {
            let mut program = SpellProgram::instructions(SpellTiming::MainPhase);
            let mut slots = BTreeMap::new();
            let mut cx = StepCx {
                program: &mut program,
                slots: &mut slots,
                player_binding: PlayerBinding::Unbound,
                context: Context::Spell,
                source_block: 0,
                x: None,
                x_consumed: false,
            };
            let target = json!({
                "ref": "target",
                "filter": {"cls": "card", "types": ["creature"], "zone": {"zone": "graveyard", "owner": owner}},
                "count": {"amount": "fixed", "value": 1},
                "selectionId": "graveyard-card"
            });
            let result = lower(
                &mut cx,
                &target,
                &json!({"zone": "battlefield"}),
                None,
                None,
                None,
                None,
            );
            if owner == "your" {
                assert!(result.is_ok(), "{result:?}");
            } else {
                assert_eq!(
                    result.unwrap_err().reason,
                    "verb:move-zone:battlefield_entry_controller"
                );
            }
        }
    }
}
