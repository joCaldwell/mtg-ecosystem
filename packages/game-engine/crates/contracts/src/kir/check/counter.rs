//! CR 701.6 and 701.19: counter an announced stack object or protect
//! a battlefield incarnation from destruction.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

impl Walk<'_, '_> {
    pub(super) fn audit_counter(&self, what: &Ref) -> Result<Cell, KirError> {
        if self.env.targets.len() == 1
            && *what == (Ref::Target { slot: 0 })
            && matches!(
                self.domain(0)?,
                TargetDomain::StackSpell
                    | TargetDomain::ControlledCreatureSpell
                    | TargetDomain::StackActivatedOrTriggeredAbility
            )
        {
            Ok(Cell::CounterStackTarget)
        } else {
            unaudited("counter target domain")
        }
    }

    pub(super) fn counter_payment(
        &self,
        payer: &PlayerExpr,
        payment: &Payment,
        node: &Node,
    ) -> bool {
        matches!(
            (payer, payment, node),
            (PlayerExpr::ControllerOf { object },
                Payment::Generic { amount: Amount::Fixed { value } },
                Node::Do(Verb::Counter { what }))
                if object == what && !value.is_negative()
        ) && (self.scan
            || matches!(
                self.domain(0),
                Ok(TargetDomain::StackSpell | TargetDomain::ControlledCreatureSpell)
            ))
    }

    pub(super) fn audit_regenerate(&self, what: &SetExpr) -> Result<Cell, KirError> {
        match what {
            SetExpr::One {
                object: Ref::Source,
            } if self.env.context != Context::Spell => Ok(Cell::RegenerateSource),
            SetExpr::One {
                object: Ref::Target { slot },
            } if matches!(
                self.domain(*slot)?,
                TargetDomain::BattlefieldCreature
                    | TargetDomain::BattlefieldPermanent
                    | TargetDomain::ControlledBattlefieldPermanent
                    | TargetDomain::ControlledBattlefieldCreature
            ) =>
            {
                Ok(Cell::RegenerateTarget)
            }
            _ => unaudited("regeneration recipient"),
        }
    }
}
