//! Zone changes (CR 400.3, 400.7). One evaluated cell admits every reviewed
//! combination of moved objects and destination; the evaluator performs it.
//! Public graveyard sets can go to exile or owners' hands. Battlefield entry
//! from a set admits the mover's graveyard only. Foreign graveyard entry
//! remains on the single-target path with its reviewed target constraints.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

const MOVED: Cell = Cell::Evaluated(EvaluatedVerb::Move);

impl Walk<'_, '_> {
    pub(super) fn audit_move(&self, what: &SetExpr, to: &Destination) -> Result<Cell, KirError> {
        match (what, to) {
            // CR 400.7, 608.2h: the source's current battlefield incarnation
            // to its owner's hand or library top; a new object is unaffected.
            (
                SetExpr::One {
                    object: Ref::Source,
                },
                Destination::OwnerHand | Destination::OwnerLibraryTop,
            ) if self.env.context != Context::Spell => Ok(MOVED),
            // CR 608.2n, 701.24c: the resolving spell shuffles itself into
            // its owner's library as it leaves the stack.
            (
                SetExpr::One {
                    object: Ref::Source,
                },
                Destination::OwnerLibraryShuffled,
            ) if self.env.context == Context::Spell => Ok(Cell::ShuffleResolvingSpell),
            // CR 400.3, 400.7, 608.2b: one announced object to the
            // destination its domain was reviewed for.
            (
                SetExpr::One {
                    object: Ref::Target { slot },
                },
                to,
            ) => self.audit_target_move(*slot, to),
            // CR 404.1, 400.3: a matching card set in a denoted public
            // graveyard goes to exile or the cards' owners' hands.
            (
                SetExpr::Matching {
                    zone: ZoneExpr::Graveyard { of },
                    filter,
                },
                Destination::Exile | Destination::OwnerHand,
            ) if of.is_you() || self.player_target(of)?.is_some() => {
                Self::library_card_filter(filter).map(|()| MOVED)
            }
            (
                SetExpr::Matching {
                    zone: ZoneExpr::Graveyard { of },
                    filter,
                },
                Destination::Battlefield { controller, .. },
            ) if of.is_you() && *controller == PlayerExpr::you(self.env.context) => {
                Self::library_card_filter(filter).map(|()| MOVED)
            }
            // CR 400.3, 400.7: exile or return every matching permanent.
            (
                SetExpr::Matching {
                    zone: ZoneExpr::Battlefield,
                    filter,
                },
                Destination::Exile | Destination::OwnerHand,
            ) => Self::audit_set_filter(&ZoneExpr::Battlefield, filter, true).map(|()| MOVED),
            (
                SetExpr::Matching {
                    zone: ZoneExpr::Graveyards,
                    filter,
                },
                Destination::Exile | Destination::OwnerHand,
            ) => Self::library_card_filter(filter).map(|()| MOVED),
            _ => unaudited("verb"),
        }
    }

    /// CR 400.3, 110.2a: the destinations reviewed for each announced
    /// object domain. A card from a graveyard enters under its mover's
    /// control; a foreign card does so only for a plain creature filter.
    fn audit_target_move(&self, slot: u32, to: &Destination) -> Result<Cell, KirError> {
        let you = PlayerExpr::you(self.env.context);
        let graveyard = matches!(
            self.domain(slot)?,
            TargetDomain::OwnGraveyardCard
                | TargetDomain::OwnGraveyardCreatureCard
                | TargetDomain::GraveyardCard { .. }
        );
        match (to, self.domain(slot)?) {
            // CR 401.4: a graveyard card can go to either end of its
            // owner's library.
            (
                Destination::OwnerHand
                | Destination::OwnerLibraryTop
                | Destination::OwnerLibraryBottom
                | Destination::Exile,
                _,
            ) if graveyard => Ok(MOVED),
            (
                Destination::Battlefield { controller, .. },
                TargetDomain::OwnGraveyardCreatureCard | TargetDomain::OwnGraveyardCard,
            ) if *controller == you => Ok(MOVED),
            (Destination::Battlefield { controller, .. }, TargetDomain::GraveyardCard { .. })
                if *controller == you
                    && self.env.targets[slot as usize]
                        .filter
                        .as_ref()
                        .is_some_and(|filter| {
                            filter.types == [CardKind::Creature] && filter.predicate.is_none()
                        }) =>
            {
                Ok(MOVED)
            }
            (Destination::Exile, TargetDomain::BattlefieldPermanent) => Ok(MOVED),
            // The reviewed higher-tier shapes keep their tier labels.
            (
                Destination::OwnerHand,
                TargetDomain::BattlefieldCreature | TargetDomain::CreatureOrPlaneswalker,
            ) => Ok(Cell::MoveTargetOwnerHand),
            (
                Destination::OwnerHand,
                TargetDomain::BattlefieldPermanent | TargetDomain::ControlledBattlefieldPermanent,
            ) => Ok(MOVED),
            (Destination::OwnerLibraryTop, _) if self.creature_target(slot)? => {
                Ok(Cell::MoveTargetOwnerLibraryTop)
            }
            (
                Destination::OwnerLibraryTop | Destination::OwnerLibraryBottom,
                TargetDomain::BattlefieldPermanent | TargetDomain::BattlefieldCreature,
            ) => Ok(MOVED),
            (Destination::OwnerLibraryShuffled, TargetDomain::BattlefieldPermanent)
                if self.env.context == Context::Spell && self.env.targets.len() == 1 =>
            {
                Ok(MOVED)
            }
            _ => unaudited("move target destination"),
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
                Destination::OwnerLibraryBottom => Capability::OwnerLibraryBottomMoveV1,
                Destination::OwnerLibraryShuffled => Capability::OwnerLibraryShuffleMoveV1,
                Destination::Battlefield { .. } => Capability::GraveyardRecursionV1,
                _ => Capability::PermanentRemovalV1,
            });
        }
        SetExpr::Pool { .. } | SetExpr::Targets { .. } | SetExpr::Result { .. } => {}
    }
}
