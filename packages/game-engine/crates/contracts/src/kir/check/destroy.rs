//! Destroy cells (CR 701.8).
use super::*;

impl Walk<'_, '_> {
    pub(super) fn audit_destroy(
        &self,
        what: &SetExpr,
        regeneration_forbidden: bool,
    ) -> Result<Cell, KirError> {
        match what {
            // CR 701.8, 608.2b: destroy one announced permanent or creature.
            // CR 701.19c: forbidding regeneration is audited for a permanent.
            SetExpr::One {
                object: Ref::Target { slot },
            } => match self.domain(*slot)? {
                TargetDomain::BattlefieldPermanent => Ok(Cell::DestroyTargetPermanent),
                TargetDomain::BattlefieldCreature if !regeneration_forbidden => {
                    Ok(Cell::DestroyTargetCreature)
                }
                _ => unaudited("destroy target domain"),
            },
            SetExpr::Targets { slots } if !regeneration_forbidden => {
                self.battlefield_target_set(slots)?;
                Ok(Cell::DestroySet)
            }
            // CR 701.8, 608.2h: destroy every battlefield permanent that
            // satisfies the filter as the instruction is performed.
            // CR 701.19c: forbidding regeneration is audited for the nonland
            // permanent wave only.
            SetExpr::Matching {
                zone: ZoneExpr::Battlefield,
                filter,
            } => {
                let history = Atom::ControllerTookSourceCombatDamage {};
                let ordinary;
                let checked = if filter.0.contains(&history) {
                    if self.env.context != Context::Activated || regeneration_forbidden {
                        return unaudited(
                            "source combat-damage history outside activated destruction",
                        );
                    }
                    ordinary = Pred(
                        filter
                            .0
                            .iter()
                            .filter(|atom| **atom != history)
                            .cloned()
                            .collect(),
                    );
                    &ordinary
                } else {
                    filter
                };
                Self::audit_set_filter(&ZoneExpr::Battlefield, checked, true)?;
                if regeneration_forbidden && *filter != Pred::nonland_permanents() {
                    return unaudited("regeneration prohibition for this set");
                }
                Ok(if *filter == Pred::creatures() {
                    Cell::DestroyAllCreatures
                } else {
                    Cell::DestroySet
                })
            }
            _ => unaudited("verb"),
        }
    }
}

/// CR 701.8: destroying every creature names the reviewed creature
/// destruction surface; every other board-wide destruction shares the
/// reviewed wipe surface. A target destroy's surface depends on its target
/// domain and is named with the program (`Definition::requirements`).
pub(super) fn capabilities(what: &SetExpr, out: &mut BTreeSet<Capability>) {
    match what {
        SetExpr::Matching { zone, filter }
            if *zone == ZoneExpr::Battlefield && *filter == Pred::creatures() =>
        {
            out.insert(Capability::DestroyCreatureV1);
        }
        SetExpr::Matching { .. } | SetExpr::Targets { .. } => {
            out.insert(Capability::BoardWipesV1);
        }
        SetExpr::One { .. } | SetExpr::Pool { .. } | SetExpr::Result { .. } => {}
    }
}
