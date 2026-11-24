//! Tap and untap cells (CR 701.26).
use super::*;

impl Walk<'_, '_> {
    /// CR 701.26a, 608.2b: tap one announced permanent.
    pub(super) fn audit_tap(&self, what: &SetExpr) -> Result<Cell, KirError> {
        match what {
            SetExpr::One {
                object: Ref::Target { slot },
            } => match self.domain(*slot)? {
                TargetDomain::BattlefieldPermanent | TargetDomain::BattlefieldCreature => {
                    Ok(Cell::TapTarget)
                }
                _ => unaudited("tap target domain"),
            },
            _ => unaudited("verb"),
        }
    }

    /// CR 701.26b, 608.2b: untap one announced permanent; CR 608.2h: untap
    /// every land you control.
    pub(super) fn audit_untap(&self, what: &SetExpr) -> Result<Cell, KirError> {
        match what {
            SetExpr::One {
                object: Ref::Target { slot },
            } => match self.domain(*slot)? {
                TargetDomain::BattlefieldPermanent => Ok(Cell::UntapTarget),
                _ => unaudited("untap target domain"),
            },
            what if *what == SetExpr::lands_you_control(self.env.context) => {
                Ok(Cell::UntapControllerLands)
            }
            _ => unaudited("verb"),
        }
    }
}

pub(super) fn tap_capabilities(what: &SetExpr, out: &mut BTreeSet<Capability>) {
    if let SetExpr::One { .. } = what {
        out.insert(Capability::TapPermanentV1);
    }
}

pub(super) fn untap_capabilities(what: &SetExpr, out: &mut BTreeSet<Capability>) {
    match what {
        SetExpr::One { .. } => {
            out.insert(Capability::UntapPermanentV1);
        }
        SetExpr::Matching { .. } => {
            out.insert(Capability::UntapControllerLandsV1);
        }
        SetExpr::Pool { .. } => {}
    }
}
