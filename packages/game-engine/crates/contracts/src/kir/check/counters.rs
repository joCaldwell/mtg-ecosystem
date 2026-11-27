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
    /// exact source or one announced creature, or -1/-1 counters (CR
    /// 122.1a) on the same recipients, fixed or the announced X.
    pub(super) fn audit_counters(
        &self,
        on: &SetExpr,
        counter: &CounterKind,
        amount: &Amount,
    ) -> Result<Cell, KirError> {
        let plus_one = match counter {
            CounterKind::PlusOnePlusOne => {
                Self::fixed_at_least(amount, 1, "counter amount")?;
                true
            }
            CounterKind::MinusOneMinusOne => {
                Self::fixed_or_announced_x(amount, 1, "counter amount")?;
                false
            }
            // CR 122.1b: only keywords whose granted runtime is reviewed.
            CounterKind::Keyword { keyword }
                if crate::supported_temporary_keywords(std::slice::from_ref(keyword)) =>
            {
                Self::fixed_at_least(amount, 1, "counter amount")?;
                false
            }
            CounterKind::Keyword { .. } | CounterKind::Named { .. } => return unaudited("verb"),
        };
        let cell = |plus: Cell, kind: Cell| Ok(if plus_one { plus } else { kind });
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
            Self::audited_filter(&ZoneExpr::Battlefield, filter)?;
            return cell(Cell::PlusOneCountersSet, Cell::KindCountersSet);
        }
        let SetExpr::One { object } = on else {
            return unaudited("counter recipient");
        };
        match object {
            Ref::Source => cell(Cell::PlusOneCountersSource, Cell::KindCountersSource),
            Ref::EventObject { .. } if !self.plain_leaf => cell(
                Cell::PlusOneCountersEventObject,
                Cell::KindCountersEventObject,
            ),
            Ref::Target { slot } => match self.domain(*slot)? {
                TargetDomain::ControlledBattlefieldCreature => {
                    cell(Cell::PlusOneCountersTarget, Cell::KindCountersTarget)
                }
                _ if self.creature_target(*slot)? => {
                    cell(Cell::PlusOneCountersTarget, Cell::KindCountersTarget)
                }
                _ => unaudited("counter target domain"),
            },
            _ => unaudited("counter recipient"),
        }
    }
}

/// CR 122.1.
pub(super) fn capabilities(counter: &CounterKind, amount: &Amount, out: &mut BTreeSet<Capability>) {
    match counter {
        CounterKind::PlusOnePlusOne => {
            out.insert(Capability::PlusOneCountersV1);
        }
        CounterKind::MinusOneMinusOne => {
            out.insert(Capability::MinusOneCountersV1);
        }
        CounterKind::Keyword { keyword } => {
            out.insert(Capability::KeywordCountersV1);
            out.extend(crate::intrinsic_keyword_capability(*keyword));
        }
        CounterKind::Named { .. } => {}
    }
    if amount.is_announced_x() {
        out.insert(Capability::AnnouncedXEffectsV1);
    }
}
