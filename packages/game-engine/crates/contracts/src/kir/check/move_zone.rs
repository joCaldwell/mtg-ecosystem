//! Zone-move cells (CR 400.3, 400.7).
use super::*;

impl Walk<'_, '_> {
    pub(super) fn audit_move(&self, what: &SetExpr, to: &Destination) -> Result<Cell, KirError> {
        match (what, to) {
            // CR 400.3, 400.7, 608.2b: one announced permanent to exile, its
            // owner's hand or the top of its owner's library.
            (
                SetExpr::One {
                    object: Ref::Target { slot },
                },
                to,
            ) => match (to, self.domain(*slot)?) {
                (Destination::Exile, TargetDomain::BattlefieldPermanent) => {
                    Ok(Cell::MoveTargetExile)
                }
                (
                    Destination::OwnerHand,
                    TargetDomain::BattlefieldPermanent
                    | TargetDomain::ControlledBattlefieldPermanent,
                ) => Ok(Cell::MovePermanentOwnerHand),
                (
                    Destination::OwnerHand,
                    TargetDomain::BattlefieldCreature | TargetDomain::CreatureOrPlaneswalker,
                ) => Ok(Cell::MoveTargetOwnerHand),
                (Destination::OwnerLibraryTop, _) if self.creature_target(*slot)? => {
                    Ok(Cell::MoveTargetOwnerLibraryTop)
                }
                _ => unaudited("move target destination"),
            },
            // CR 404.1, 400.3: exile every card in one announced player's
            // graveyard.
            (
                SetExpr::Matching {
                    zone:
                        ZoneExpr::Graveyard {
                            of:
                                PlayerExpr::Is {
                                    who: Ref::Target { slot },
                                },
                        },
                    filter,
                },
                Destination::Exile,
            ) if filter.0.is_empty() && self.domain(*slot)? == TargetDomain::AnyPlayer => {
                Ok(Cell::ExileTargetGraveyard)
            }
            // CR 400.3, 400.7: exile, or return to owners' hands, every
            // matching battlefield permanent; or exile every graveyard card.
            (
                SetExpr::Matching {
                    zone: ZoneExpr::Battlefield,
                    filter,
                },
                Destination::Exile | Destination::OwnerHand,
            ) => Self::audited_filter(&ZoneExpr::Battlefield, filter).map(|()| Cell::MoveSet),
            (
                SetExpr::Matching {
                    zone: ZoneExpr::Graveyards,
                    filter,
                },
                Destination::Exile,
            ) if filter.0.is_empty() => Ok(Cell::ExileGraveyards),
            _ => unaudited("verb"),
        }
    }
}

/// CR 404.1: exiling one announced player's graveyard; every other
/// board-wide movement shares the reviewed wipe surface. One announced
/// permanent: each destination names its reviewed surface.
pub(super) fn capabilities(what: &SetExpr, to: &Destination, out: &mut BTreeSet<Capability>) {
    match what {
        SetExpr::Matching {
            zone: ZoneExpr::Graveyard { .. },
            ..
        } => {
            out.insert(Capability::EntryGraveyardExileV1);
        }
        SetExpr::Matching { .. } => {
            out.insert(Capability::BoardWipesV1);
        }
        SetExpr::One { .. } => {
            out.insert(match to {
                Destination::OwnerHand => Capability::OwnerHandMoveV1,
                Destination::OwnerLibraryTop => Capability::OwnerLibraryTopMoveV1,
                _ => Capability::PermanentRemovalV1,
            });
        }
        SetExpr::Pool { .. } => {}
    }
}
