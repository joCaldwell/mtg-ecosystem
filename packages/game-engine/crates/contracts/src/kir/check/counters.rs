//! Counter cells (CR 122).
use super::*;

impl Walk<'_, '_> {
    /// CR 122.1, 122.6: a positive fixed number of +1/+1 counters on the
    /// exact source or one announced creature.
    pub(super) fn audit_counters(
        &self,
        on: &SetExpr,
        counter: &CounterKind,
        amount: &Amount,
    ) -> Result<Cell, KirError> {
        let (SetExpr::One { object }, CounterKind::PlusOnePlusOne) = (on, counter) else {
            return unaudited("verb");
        };
        Self::fixed_at_least(amount, 1, "counter amount")?;
        match object {
            Ref::Source => Ok(Cell::PlusOneCountersSource),
            Ref::Target { slot } => match self.domain(*slot)? {
                TargetDomain::BattlefieldCreature | TargetDomain::ControlledBattlefieldCreature => {
                    Ok(Cell::PlusOneCountersTarget)
                }
                _ => unaudited("counter target domain"),
            },
            _ => unaudited("counter recipient"),
        }
    }
}

/// CR 122.1.
pub(super) fn capabilities(counter: &CounterKind, out: &mut BTreeSet<Capability>) {
    if *counter == CounterKind::PlusOnePlusOne {
        out.insert(Capability::PlusOneCountersV1);
    }
}
