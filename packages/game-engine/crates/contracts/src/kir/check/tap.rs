//! Tap and untap cells (CR 701.26).
use super::*;

impl Walk<'_, '_> {
    pub(super) fn battlefield_target_set(&self, slots: &[u32]) -> Result<(), KirError> {
        for slot in slots {
            if !matches!(
                self.domain(*slot)?,
                TargetDomain::BattlefieldPermanent
                    | TargetDomain::ControlledBattlefieldPermanent
                    | TargetDomain::BattlefieldCreature
            ) {
                return unaudited("simultaneous battlefield target domain");
            }
        }
        Ok(())
    }

    /// CR 701.26a, 608.2b: tap one announced permanent.
    pub(super) fn audit_tap(&self, what: &SetExpr) -> Result<Cell, KirError> {
        match what {
            SetExpr::One {
                object: Ref::Source,
            } => Ok(Cell::TapStatusSource),
            SetExpr::One {
                object: Ref::Target { slot },
            } => match self.domain(*slot)? {
                TargetDomain::BattlefieldPermanent
                | TargetDomain::ControlledBattlefieldPermanent
                | TargetDomain::BattlefieldCreature => Ok(Cell::TapTarget),
                _ => unaudited("tap target domain"),
            },
            SetExpr::Targets { slots } => {
                self.battlefield_target_set(slots)?;
                Ok(Cell::TapSet)
            }
            SetExpr::Matching {
                zone: ZoneExpr::Battlefield,
                filter,
            } if matches!(filter.0.as_slice(), [Atom::Type { kind: CardKind::Creature },
                    Atom::ControlledBy { player: PlayerExpr::Is { who: Ref::Target { slot } } }]
                    if self.domain(*slot)? == TargetDomain::AnyPlayer) =>
            {
                Ok(Cell::TapTargetPlayersCreatures)
            }
            SetExpr::Matching {
                zone: ZoneExpr::Battlefield,
                filter,
            } => Self::audited_filter(&ZoneExpr::Battlefield, filter).map(|()| Cell::TapSet),
            _ => unaudited("verb"),
        }
    }

    /// CR 701.26b, 608.2b: untap one announced permanent; CR 608.2h: untap
    /// every land you control.
    pub(super) fn audit_untap(&self, what: &SetExpr) -> Result<Cell, KirError> {
        match what {
            SetExpr::One {
                object: Ref::Source,
            } => Ok(Cell::TapStatusSource),
            SetExpr::One {
                object: Ref::Target { slot },
            } => match self.domain(*slot)? {
                TargetDomain::BattlefieldPermanent
                | TargetDomain::ControlledBattlefieldPermanent
                | TargetDomain::BattlefieldCreature => Ok(Cell::UntapTarget),
                _ => unaudited("untap target domain"),
            },
            SetExpr::Targets { slots } => {
                self.battlefield_target_set(slots)?;
                Ok(Cell::UntapSet)
            }
            what if *what == SetExpr::lands_you_control(self.env.context) => {
                Ok(Cell::UntapControllerLands)
            }
            SetExpr::Matching {
                zone: ZoneExpr::Battlefield,
                filter,
            } => Self::audited_filter(&ZoneExpr::Battlefield, filter).map(|()| Cell::UntapSet),
            _ => unaudited("verb"),
        }
    }
}

pub(super) fn tap_capabilities(what: &SetExpr, out: &mut BTreeSet<Capability>) {
    if matches!(
        what,
        SetExpr::One { .. } | SetExpr::Matching { .. } | SetExpr::Targets { .. }
    ) {
        out.insert(Capability::TapPermanentV1);
    }
}

pub(super) fn untap_capabilities(what: &SetExpr, out: &mut BTreeSet<Capability>) {
    match what {
        SetExpr::One { .. } => {
            out.insert(Capability::UntapPermanentV1);
        }
        SetExpr::Matching { .. }
            if [Context::Spell, Context::Activated, Context::Triggered]
                .into_iter()
                .any(|context| *what == SetExpr::lands_you_control(context)) =>
        {
            out.insert(Capability::UntapControllerLandsV1);
        }
        SetExpr::Matching { .. } | SetExpr::Targets { .. } => {
            out.insert(Capability::UntapPermanentV1);
        }
        SetExpr::Pool { .. }
        | SetExpr::Result { .. }
        | SetExpr::LiveResult { .. }
        | SetExpr::Revealed { .. } => {}
    }
}
