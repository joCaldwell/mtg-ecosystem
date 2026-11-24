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
pub(super) fn capabilities(player: &PlayerExpr, out: &mut BTreeSet<Capability>) {
    out.insert(match player {
        PlayerExpr::EachPlayer => Capability::BoardWipesV1,
        PlayerExpr::EachOpponent => Capability::SacrificeV1,
        PlayerExpr::Is {
            who: Ref::Target { .. },
        } => Capability::TargetPlayerSacrificeCreatureV1,
        _ => Capability::EventPlayerSacrificeV1,
    });
}
