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
            what: destroy_set(&what, cx.context)?,
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

/// CR 120.2a, 400.7, 608.2h: the historical recipient is each candidate's
/// current controller, and the damage must be from this source incarnation.
fn destroy_set(what: &ast::ObjectRef, context: Context) -> Result<SetExpr, Failure> {
    let filter = match what {
        ast::ObjectRef::All(set) => &set.filter,
        ast::ObjectRef::Each(set) => &set.filter,
        _ => return lower_board_set(what, context),
    };
    if context != Context::Activated || filter.controller_matches.is_none() {
        return lower_board_set(what, context);
    }
    use serde_json::json;
    let source_history: ast::PlayerFilter = typed(
        &json!({"damageHistory":{
            "role":"recipient","window":{"during":"this-turn"},"damage":"combat",
            "source":{"ref":"this","noun":"creature"}
        }}),
        "controllerMatches",
    )?;
    if filter.controller_matches.as_deref() != Some(&source_history) {
        return lower_board_set(what, context);
    }
    if filter.zone.is_some() {
        return Err(operand("filter", "controller_matches:zone"));
    }
    let mut ordinary = what.clone();
    let filter = match &mut ordinary {
        ast::ObjectRef::All(set) => &mut set.filter,
        ast::ObjectRef::Each(set) => &mut set.filter,
        _ => unreachable!("checked set shape"),
    };
    filter.controller_matches = None;
    let SetExpr::Matching { zone, mut filter } = lower_board_set(&ordinary, context)? else {
        unreachable!("board set lowering always matches the battlefield");
    };
    filter.0.push(Atom::ControllerTookSourceCombatDamage {});
    Ok(SetExpr::Matching { zone, filter })
}
