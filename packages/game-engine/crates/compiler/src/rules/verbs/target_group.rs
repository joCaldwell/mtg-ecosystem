//! Counted target occurrences: "two target creatures", "up to three target
//! cards from your graveyard" (CR 115.1d, 115.3, 601.2c, 608.2b).
//!
//! One printed occurrence announces `count` distinct objects, or any number
//! up to it. It lowers to `count` slots with one domain and filter; slot `k`
//! is the archive selection identity with `/k` appended for `k > 0`, the
//! shape `SpellProgram::target_group` recognizes. The step is lowered once
//! per slot as if it named one target, so every resulting instruction is an
//! independent leaf whose whole action applies to its own object: a leaf
//! whose object was not chosen or is illegal at resolution does nothing,
//! and the spell is countered only when every chosen object is illegal.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::json;

/// The largest counted occurrence the reviewed group decision announces.
const MAX_GROUP: u32 = 7;

struct Group {
    count: u32,
    up_to: bool,
    selection: String,
}

impl Group {
    fn identity(&self, index: u32) -> String {
        if index == 0 {
            self.selection.clone()
        } else {
            format!("{}/{index}", self.selection)
        }
    }
    fn code(&self) -> &'static str {
        if self.up_to { "up_to" } else { "count" }
    }
}

enum Field<'a> {
    Json(&'a mut serde_json::Value),
    Typed(&'a mut ast::ObjectRef),
}

/// The one object operand of a step that may hold a counted occurrence.
/// Tap and untap keep their reviewed up-to batch node.
fn field(step: &mut Step) -> Option<Field<'_>> {
    Some(match step {
        Step::Destroy { what, .. }
        | Step::Pump { what, .. }
        | Step::GainAbilities { what, .. }
        | Step::Regenerate { what }
        | Step::Tap { what }
        | Step::Untap { what } => Field::Json(what),
        Step::Evaluated(effect) => match effect.as_mut() {
            ast::Effect::PutCounters(counters) => Field::Typed(&mut counters.on),
            ast::Effect::Exile(exile) => Field::Typed(&mut exile.what),
            ast::Effect::ReturnToHand(movement) => Field::Typed(&mut movement.what),
            ast::Effect::MoveZone(movement) => Field::Typed(&mut movement.what),
            ast::Effect::Damage(damage) => match damage.to.as_mut() {
                ast::DamageTarget::Object(to) => Field::Typed(&mut to.r#ref),
                _ => return None,
            },
            _ => return None,
        },
        Step::Cant { what, .. }
        | Step::Become { what, .. }
        | Step::GainControl { what, .. }
        | Step::ModifyCharacteristics { what, .. } => Field::Typed(what),
        _ => return None,
    })
}

fn group(field: &Field<'_>) -> Option<Group> {
    let (count, up_to, selection) = match field {
        Field::Json(value) => {
            if value["ref"] != "target" || value["count"]["amount"] != "fixed" {
                return None;
            }
            (
                value["count"]["value"].as_u64()?,
                value["upTo"] == true,
                value["selectionId"].as_str()?.to_owned(),
            )
        }
        Field::Typed(ast::ObjectRef::Target(target)) => {
            let ast::Amount::Fixed(count) = &*target.count else {
                return None;
            };
            (
                u64::try_from(count.value).ok()?,
                target.up_to == Some(true),
                target.selection_id.clone()?,
            )
        }
        Field::Typed(_) => return None,
    };
    let count = u32::try_from(count).ok().filter(|count| *count >= 2)?;
    Some(Group {
        count,
        up_to,
        selection,
    })
}

/// Rewrite the operand as the occurrence's `index`th single target.
fn member(field: Field<'_>, group: &Group, index: u32) -> Result<(), Failure> {
    let identity = group.identity(index);
    match field {
        Field::Json(value) => {
            let object = value
                .as_object_mut()
                .ok_or_else(|| operand("target", group.code()))?;
            object.remove("upTo");
            object.insert("count".into(), json!({"amount": "fixed", "value": 1}));
            object.insert("selectionId".into(), identity.into());
        }
        Field::Typed(ast::ObjectRef::Target(target)) => {
            target.up_to = None;
            *target.count = serde_json::from_value(json!({"amount": "fixed", "value": 1}))
                .map_err(|_| operand("target", group.code()))?;
            target.selection_id = Some(identity);
        }
        Field::Typed(_) => return Err(operand("target", group.code())),
    }
    Ok(())
}

/// The occurrence a step's operand announces, with the step rewritten once
/// per announced object. `None` leaves the step to its own lowerer.
fn members(step: &Step) -> Option<(Group, Vec<Step>)> {
    let mut probe = step.clone();
    let group = group(&field(&mut probe)?)?;
    if group.up_to && matches!(step, Step::Tap { .. } | Step::Untap { .. }) {
        return None;
    }
    let steps = (0..group.count)
        .map(|index| {
            let mut copy = step.clone();
            member(field(&mut copy)?, &group, index).ok()?;
            Some(copy)
        })
        .collect::<Option<Vec<_>>>()?;
    Some((group, steps))
}

/// CR 115.3, 601.2b-c: one selection uses the announced X or the eligible
/// object set. Only audited simultaneous battlefield actions are admitted.
fn variable_batch(cx: &mut StepCx<'_>, step: &Step) -> Result<Option<Vec<Instruction>>, Failure> {
    use kir::VariableTargetCount::*;
    if !matches!(
        step,
        Step::Tap { .. } | Step::Untap { .. } | Step::Destroy { .. }
    ) && !matches!(
        step.effect(),
        Some(ast::Effect::Exile(_) | ast::Effect::ReturnToHand(_))
    ) {
        return Ok(None);
    }
    let mut step = step.clone();
    let target = match field(&mut step) {
        Some(Field::Json(value)) => {
            if value["ref"] != "target" || value["count"]["amount"] == "fixed" {
                return Ok(None);
            }
            let ast::ObjectRef::Target(target) = object(value)? else {
                return Ok(None);
            };
            target
        }
        Some(Field::Typed(ast::ObjectRef::Target(target))) => target.clone(),
        _ => return Ok(None),
    };
    let variable = match (&*target.count, target.up_to) {
        (ast::Amount::X(_), None) => ExactlyX,
        (ast::Amount::X(_), Some(true)) => UpToX,
        (ast::Amount::Chosen(count), None) if count.minimum == 0 && count.maximum.is_none() => Any,
        _ => return Ok(None),
    };
    if !cx.program.targets.is_empty() {
        return Ok(None);
    }
    let Some(selection) = target.selection_id else {
        return Ok(None);
    };
    member(
        field(&mut step).ok_or_else(|| operand("target", "count"))?,
        &Group {
            count: 1,
            up_to: false,
            selection,
        },
        0,
    )?;
    let leaf = lower_step(cx, &step)?;
    let mut instructions = batch(cx, 1, vec![leaf]);
    let [Instruction::Kernel { node, .. }] = instructions.as_mut_slice() else {
        unreachable!("one batch instruction");
    };
    match node {
        kir::Node::TapTargets {
            variable: count, ..
        }
        | kir::Node::RemoveTargets {
            variable: count, ..
        } => *count = Some(variable),
        _ => return Err(operand("target", "count")),
    }
    Ok(Some(instructions))
}

/// Lower one step whose operand is a counted occurrence, or `None` when it
/// is not one. The group is the program's only target occurrence; a later
/// step may name the same occurrence again (one selection identity).
pub(super) fn lower(cx: &mut StepCx<'_>, step: &Step) -> Result<Option<Vec<Instruction>>, Failure> {
    let Some((group, steps)) = members(step) else {
        return variable_batch(cx, step);
    };
    // A spell deals its damage to each member; another source's damage
    // events would be observable one at a time (CR 120.3).
    if step.damage().is_some() && cx.context != Context::Spell {
        return Err(operand("target", format!("{}:damage_source", group.code())));
    }
    // CR 608.2f, 603.2c: counters put on each of several targets are one
    // action processed simultaneously, one event with several occurrences.
    // Per-member leaves would place them one event at a time, and no set
    // names several announced targets.
    if matches!(step.effect(), Some(ast::Effect::PutCounters(_))) {
        return Err(operand("target", format!("{}:counters", group.code())));
    }
    if group.count > MAX_GROUP {
        return Err(operand(
            "target",
            format!("{}:{}", group.code(), group.count),
        ));
    }
    let identities: Vec<String> = (0..group.count).map(|i| group.identity(i)).collect();
    let rebound = cx
        .program
        .targets
        .iter()
        .map(|t| &t.selection_id)
        .eq(identities.iter());
    if !(cx.program.targets.is_empty() && !cx.program.optional_target
        || rebound && cx.program.optional_target == group.up_to)
    {
        return Err(operand(
            "target",
            format!("{}:shared_announcement", group.code()),
        ));
    }
    cx.program.optional_target = false;
    let mut out = vec![];
    for (index, member) in steps.iter().enumerate() {
        let instruction = lower_step(cx, member)?;
        // Each member reads exactly its own slot as one independent leaf.
        let Instruction::Kernel { node, .. } = &instruction;
        if node.group_leaf_slot() != Some(index as u32)
            || cx.program.targets.len() <= index
            || cx.program.targets[index].selection_id != identities[index]
        {
            return Err(operand("target", format!("{}:leaf", group.code())));
        }
        out.push(instruction);
    }
    if cx.program.targets.len() != group.count as usize || cx.program.target_group().is_none() {
        return Err(operand("target", format!("{}:shape", group.code())));
    }
    cx.program.optional_target = group.up_to;
    Ok(Some(batch(cx, group.count, out)))
}

/// CR 603.6a, 603.10a, 608.2b: battlefield removal and tap status change
/// every legal member in one simultaneous event, so the per-member leaves of
/// one step become the group's batch node. Other leaves stay per member.
fn batch(cx: &StepCx<'_>, count: u32, leaves: Vec<Instruction>) -> Vec<Instruction> {
    let battlefield = matches!(
        cx.program.targets[0].domain,
        TargetDomain::BattlefieldCreature
            | TargetDomain::BattlefieldPermanent
            | TargetDomain::ControlledBattlefieldPermanent
    );
    let node = leaves.iter().try_fold(None, |kind, instruction| {
        let Instruction::Kernel { node, .. } = instruction;
        let this = match node.verb()? {
            Verb::Tap { .. } => kir::Node::TapTargets {
                count,
                variable: None,
                tap: true,
            },
            Verb::Untap { .. } => kir::Node::TapTargets {
                count,
                variable: None,
                tap: false,
            },
            Verb::Destroy {
                regeneration_forbidden: false,
                ..
            } => kir::Node::RemoveTargets {
                count,
                variable: None,
                removal: kir::GroupRemoval::Destroy,
            },
            Verb::Move {
                to: kir::Destination::Exile,
                ..
            } => kir::Node::RemoveTargets {
                count,
                variable: None,
                removal: kir::GroupRemoval::Exile,
            },
            Verb::Move {
                to: kir::Destination::OwnerHand,
                ..
            } => kir::Node::RemoveTargets {
                count,
                variable: None,
                removal: kir::GroupRemoval::OwnerHand,
            },
            _ => return None,
        };
        match kind {
            None => Some(Some(this)),
            Some(kind) if kind == this => Some(Some(kind)),
            Some(_) => None,
        }
    });
    match node {
        Some(Some(node)) if battlefield => vec![Instruction::Kernel {
            node,
            source_block: cx.source_block,
        }],
        _ => leaves,
    }
}
