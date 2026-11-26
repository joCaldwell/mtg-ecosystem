//! Sacrifice cells (CR 701.21).
use super::*;

impl Walk<'_, '_> {
    /// CR 701.21a: the damaged player, one announced opponent, each player
    /// or each opponent sacrifices one creature.
    pub(super) fn audit_sacrifice(
        &self,
        player: &PlayerExpr,
        count: &Amount,
        filter: &Pred,
    ) -> Result<Cell, KirError> {
        // CR 701.21a, 702.74a, 702.30a: "its controller sacrifices it" or
        // "sacrifice it": one exact source permanent, by its current
        // controller or by the ability's controller.
        if *filter == Pred::source() {
            return match player {
                _ if count.as_fixed() != Some(&Quantity::from(1)) => unaudited("verb"),
                PlayerExpr::ControllerOf {
                    object: Ref::Source,
                } => Ok(Cell::SacrificeSource),
                PlayerExpr::Is {
                    who: Ref::Controller { .. },
                } if self.env.context == Context::Triggered => Ok(Cell::SacrificeSource),
                _ => unaudited("sacrifice recipient"),
            };
        }
        if *filter != Pred::creatures() || count.as_fixed() != Some(&Quantity::from(1)) {
            return unaudited("verb");
        }
        match player {
            PlayerExpr::Is {
                who:
                    Ref::EventPlayer {
                        role: EventPlayerRole::DamagedPlayer,
                    },
            } => Ok(Cell::SacrificeDamagedPlayer),
            PlayerExpr::EachPlayer => Ok(Cell::SacrificeEachPlayer),
            PlayerExpr::EachOpponent => Ok(Cell::SacrificeEachOpponent),
            PlayerExpr::Is {
                who: Ref::Target { .. },
            } if self.player_target(player)? == Some(TargetDomain::OpponentPlayer) => {
                Ok(Cell::SacrificeTargetOpponent)
            }
            _ => unaudited("sacrifice recipient"),
        }
    }
}

/// CR 701.21a: "each player" shares the reviewed wipe surface and "each
/// opponent" the reviewed group sacrifice surface.
pub(super) fn capabilities(player: &PlayerExpr, filter: &Pred, out: &mut BTreeSet<Capability>) {
    if *filter == Pred::source() {
        out.insert(Capability::SacrificeSourceV1);
        return;
    }
    out.insert(match player {
        PlayerExpr::EachPlayer => Capability::BoardWipesV1,
        PlayerExpr::EachOpponent => Capability::SacrificeV1,
        PlayerExpr::Is {
            who: Ref::Target { .. },
        } => Capability::TargetPlayerSacrificeCreatureV1,
        _ => Capability::EventPlayerSacrificeV1,
    });
}
