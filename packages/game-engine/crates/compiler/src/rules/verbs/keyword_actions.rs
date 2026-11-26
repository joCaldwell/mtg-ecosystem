//! Resolving keyword actions (CR 701.14, 701.16, 701.34, 701.36).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

pub(super) fn lower(
    cx: &mut StepCx<'_>,
    action: &str,
    who: &serde_json::Value,
    times: Option<&serde_json::Value>,
) -> Result<Instruction, Failure> {
    if who != &serde_json::json!({ "player": "you" }) || times.is_some() {
        return Err(step("keyword-action", &format!("{action}:actor_or_count")));
    }
    let player = PlayerExpr::you(cx.context);
    Ok(cx.act(match action {
        "investigate" => Verb::CreateTokens {
            controller: player,
            count: Amount::fixed(1),
            token: kir::TokenSpec::Predefined {
                kind: PredefinedToken::Clue,
            },
            tapped: false,
        },
        "populate" => Verb::Populate { player },
        "proliferate" => Verb::Proliferate { player },
        _ => return Err(step("keyword-action", action)),
    }))
}

pub(super) fn fight(
    cx: &mut StepCx<'_>,
    a: &serde_json::Value,
    b: &serde_json::Value,
) -> Result<Instruction, Failure> {
    let a = object(a)?;
    let b = object(b)?;
    let first = if matches!(&a, ast::ObjectRef::It(_))
        || matches!(&a, ast::ObjectRef::That(that) if that.plural.is_none()
            && present_fields(&that.filter) == ["types"]
            && that.filter.types.as_deref().is_some_and(|types| types == ["creature"]))
    {
        let [sole] = cx.program.targets.as_slice() else {
            return Err(step("fight", "ambiguous first participant"));
        };
        let object = kir::Ref::Target { slot: sole.slot };
        let previous = cx
            .program
            .instructions
            .last()
            .filter(|i| i.source_block() == cx.source_block)
            .and_then(Instruction::verb);
        if !matches!(previous,
            Some(Verb::ModifyUntil { what: SetExpr::One { object: prior }, .. }
                | Verb::PutCounters { on: SetExpr::One { object: prior }, .. }) if *prior == object)
        {
            return Err(step(
                "fight",
                "first participant has no preceding creature effect",
            ));
        }
        object
    } else {
        fighter(cx, &a)?
    };
    let second = fighter(cx, &b)?;
    Ok(cx.act(Verb::Fight { first, second }))
}

fn fighter(cx: &mut StepCx<'_>, object: &ast::ObjectRef) -> Result<kir::Ref, Failure> {
    match object {
        ast::ObjectRef::Target(target) => {
            // CR 102.3: in these free-for-all profiles every other player is
            // an opponent. Both forms use the same live controller predicate.
            let mut target = target.clone();
            if target.filter.control == Some(ast::ObjectFilterControl::NotYou) {
                target.filter.control = Some(ast::ObjectFilterControl::Opponent);
            }
            Ok(kir::Ref::Target {
                slot: bind_creature_target(cx.program, cx.slots, &ast::ObjectRef::Target(target))?,
            })
        }
        ast::ObjectRef::SelfObject(_) if cx.context != Context::Spell => Ok(kir::Ref::Source),
        ast::ObjectRef::This(this) if this.noun == "creature" && cx.context != Context::Spell => {
            Ok(kir::Ref::Source)
        }
        _ => Err(step("fight", "participant")),
    }
}
