//! Counter cells (CR 122).
use super::*;

impl Walk<'_, '_> {
    pub(super) fn audit_player_counters(
        &self,
        player: &PlayerExpr,
        amount: &Amount,
    ) -> Result<Cell, KirError> {
        if !player.is_you() && *player != PlayerExpr::EachOpponent {
            return unaudited("player counter recipient");
        }
        Self::fixed_at_least(amount, 1, "player counter amount")?;
        Ok(Cell::PlayerCounters)
    }

    /// CR 122.1, 122.6: a positive fixed number of +1/+1 counters on the
    /// exact source or one announced creature.
    pub(super) fn audit_counters(
        &self,
        on: &SetExpr,
        counter: &CounterKind,
        amount: &Amount,
    ) -> Result<Cell, KirError> {
        if *counter != CounterKind::PlusOnePlusOne {
            return unaudited("verb");
        }
        Self::fixed_at_least(amount, 1, "counter amount")?;
        // CR 122.6, 608.2h: a single simultaneous placement on the
        // creatures matching a public predicate when this step is performed.
        // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
        if let SetExpr::Matching {
            zone: ZoneExpr::Battlefield,
            filter,
        } = on
            && filter.0.contains(&Atom::Type {
                kind: CardKind::Creature,
            })
        {
            return Self::audited_filter(&ZoneExpr::Battlefield, filter)
                .map(|()| Cell::PlusOneCountersSet);
        }
        let SetExpr::One { object } = on else {
            return unaudited("counter recipient");
        };
        match object {
            Ref::Source => Ok(Cell::PlusOneCountersSource),
            Ref::EventObject { .. } if !self.plain_leaf => Ok(Cell::PlusOneCountersEventObject),
            Ref::Target { slot } => match self.domain(*slot)? {
                TargetDomain::ControlledBattlefieldCreature => Ok(Cell::PlusOneCountersTarget),
                _ if self.creature_target(*slot)? => Ok(Cell::PlusOneCountersTarget),
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
