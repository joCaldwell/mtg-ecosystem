//! CR 611.2a,c, 508.1c, 509.1b: temporary combat restrictions.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use kir::{CombatProhibitionScope, CombatRequirement};

pub(super) fn lower(
    cx: &mut StepCx<'_>,
    what: &ast::ObjectRef,
    action: &ast::CantAction,
    duration: &SourceDuration,
) -> Result<Instruction, Failure> {
    uncounted(cx)?;
    use ast::CantAction as A;
    let restriction = match action {
        A::Attack => CombatRequirement::CantAttack,
        A::Block => CombatRequirement::CantBlock,
        A::AttackOrBlock => CombatRequirement::CantAttackOrBlock,
        A::BeBlocked => CombatRequirement::CantBeBlocked,
        other => return Err(step("cant", &variant(other))),
    };
    match typed::<ast::Duration>(duration, "duration")? {
        ast::Duration::EndOfTurn(_) | ast::Duration::ThisTurn(_) => {}
        other => return Err(operand("duration", variant(&other))),
    }
    let target = |slot| CombatProhibitionScope::Target {
        target: kir::Ref::Target { slot },
    };
    let affects = match what {
        ast::ObjectRef::Target(_) => target(bind_creature_target(cx.program, cx.slots, what)?),
        ast::ObjectRef::Each(_) | ast::ObjectRef::All(_) => {
            let set = lower_board_set(what, cx.context)?;
            if set == ObjectSet::Creatures.kernel(cx.context) {
                CombatProhibitionScope::Creatures {}
            } else if set == ObjectSet::NonflyingCreatures.kernel(cx.context) {
                CombatProhibitionScope::CreaturesWithoutFlying {}
            } else {
                return Err(operand("cant_recipient", "battlefield_filter"));
            }
        }
        other => return Err(operand("cant_recipient", variant(other))),
    };
    Ok(cx.act(Verb::RestrictCombat {
        affects,
        restriction,
        until: kir::Until::EndOfTurn,
    }))
}
