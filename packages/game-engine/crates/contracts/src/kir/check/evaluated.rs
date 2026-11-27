//! Check slots after the shared scope and sort walk.
use super::*;

impl Walk<'_, '_> {
    pub(super) fn audit_evaluated(&self, verb: &Verb) -> Result<Cell, KirError> {
        let (signature, player, amount) = verb.evaluated_operands().expect("evaluated verb");
        let [recipient, quantity] = signature.slots else {
            unreachable!("player and quantity signature");
        };
        for (slot, sort, class) in [
            (recipient, Sort::Player, OperandClass::player(player)),
            (quantity, Sort::Quantity, OperandClass::amount(amount)),
        ] {
            if slot.sort != sort || class.is_none_or(|class| !slot.accepts.contains(&class)) {
                return unaudited(format!("unsupported {} operand", slot.field));
            }
        }
        match amount {
            Amount::Fixed { value } if value.is_negative() => return kind("negative quantity"),
            Amount::Fixed { .. } | Amount::Var { .. } => {}
            Amount::Count { set } if self.audit_control_set(set).is_ok() => {}
            Amount::StatOf {
                object:
                    Ref::Source | Ref::Target { .. } | Ref::EventObject { .. } | Ref::BoundVar { .. },
                ..
            } => {}
            Amount::LifeOf { player }
                if !matches!(
                    player,
                    PlayerExpr::EachPlayer
                        | PlayerExpr::EachOpponent
                        | PlayerExpr::Is {
                            who: Ref::StepResult { .. }
                        }
                ) => {}
            amount => Self::audit_verb_amount(amount)?,
        }
        if let PlayerExpr::OwnerOf { object } | PlayerExpr::ControllerOf { object } = player {
            match object {
                Ref::Source | Ref::EventObject { .. } | Ref::BoundVar { .. } => {}
                Ref::Target { slot }
                    if !matches!(
                        self.domain(*slot)?,
                        TargetDomain::StackSpell
                            | TargetDomain::ControlledCreatureSpell
                            | TargetDomain::StackActivatedOrTriggeredAbility
                    ) => {}
                _ => return unaudited("player relation without object facts"),
            }
        }
        // Higher tiers keep their exact pre-pilot surface. This is a tier
        // restriction after admission, not a second admission table.
        if let Some(amount) = amount.as_fixed() {
            let target = self.player_target(player)? == Some(TargetDomain::AnyPlayer);
            match signature.verb {
                EvaluatedVerb::Draw if *amount == Quantity::from(1) => {
                    if player.is_you() {
                        return Ok(Cell::DrawController);
                    }
                    if target {
                        return Ok(Cell::DrawTargetPlayer);
                    }
                }
                EvaluatedVerb::GainLife | EvaluatedVerb::LoseLife if *amount > Quantity::zero() => {
                    if player.is_you() {
                        return Ok(Cell::LifeController);
                    }
                    if target {
                        return Ok(Cell::LifeTargetPlayer);
                    }
                }
                _ => {}
            }
        }
        Ok(Cell::Evaluated(signature.verb))
    }
}
