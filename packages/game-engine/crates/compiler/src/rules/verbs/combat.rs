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

/// CR 508.1d, 609.4, 611.2c: "attacks this turn if able" and "can attack
/// this turn as though it didn't have defender" on one announced creature
/// or the ability's own source.
pub(super) fn lower_rule(
    cx: &mut StepCx<'_>,
    what: &ast::ObjectRef,
    rule: &ast::CombatRule,
    duration: &SourceDuration,
) -> Result<Instruction, Failure> {
    uncounted(cx)?;
    use ast::CombatRule as C;
    let reject = || step("combat-rule", &variant(rule));
    let restriction = match rule {
        C::AttackRequirement(x)
            if x.frequency == ast::CombatRuleAttackRequirementFrequency::EachCombat
                && x.defender.is_none()
                && x.when_attacking.is_none()
                && x.condition.is_none()
                && x.condition_kind.is_none() =>
        {
            CombatRequirement::AttackEachCombat
        }
        C::AsThough(x)
            if x.condition.is_none()
                && x.condition_kind.is_none()
                && x.defenders.is_none()
                && x.attackers.is_none()
                && crate::rules::lacks_defender_to_attack(x) =>
        {
            CombatRequirement::AttackAsThoughNoDefender
        }
        _ => return Err(reject()),
    };
    match typed::<ast::Duration>(duration, "duration")? {
        ast::Duration::EndOfTurn(_) | ast::Duration::ThisTurn(_) => {}
        other => return Err(operand("duration", variant(&other))),
    }
    let target = match what {
        ast::ObjectRef::Target(_) => kir::Ref::Target {
            slot: bind_creature_target(cx.program, cx.slots, what)?,
        },
        ast::ObjectRef::This(x) if x.noun == "creature" && cx.context != Context::Spell => {
            kir::Ref::Source
        }
        other => return Err(operand("combat_rule_recipient", variant(other))),
    };
    Ok(cx.act(Verb::RestrictCombat {
        affects: CombatProhibitionScope::Target { target },
        restriction,
        until: kir::Until::EndOfTurn,
    }))
}
