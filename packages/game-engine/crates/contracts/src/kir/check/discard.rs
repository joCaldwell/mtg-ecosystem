//! Discard cells (CR 701.9).
use super::*;

impl Walk<'_, '_> {
    /// CR 701.9a-b: the controller or the damaged player discards a fixed
    /// count, one announced player discards a positive count (at random,
    /// also the announced X), and each player or each opponent discards a
    /// positive count.
    pub(super) fn audit_discard(
        &self,
        player: &PlayerExpr,
        amount: &Amount,
        random: bool,
    ) -> Result<Cell, KirError> {
        let announced = Self::fixed_or_announced_x(amount, 0, "discard count")?;
        match player {
            PlayerExpr::Is {
                who: Ref::Controller { .. },
            } if !random && !announced => Ok(Cell::DiscardController),
            PlayerExpr::Is {
                who:
                    Ref::EventPlayer {
                        role: EventPlayerRole::DamagedPlayer | EventPlayerRole::DefendingPlayer,
                    },
            } if !random && !announced => Ok(Cell::DiscardDamagedPlayer),
            PlayerExpr::Is {
                who: Ref::Target { .. },
            } if matches!(
                self.player_target(player)?,
                Some(TargetDomain::AnyPlayer | TargetDomain::OpponentPlayer)
            ) && ((!random && !announced)
                || (random && self.env.context == Context::Spell)) =>
            {
                if !announced {
                    Self::fixed_at_least(amount, 1, "discard count")?;
                }
                Ok(if random {
                    Cell::DiscardTargetRandom
                } else {
                    Cell::DiscardTarget
                })
            }
            PlayerExpr::EachPlayer | PlayerExpr::EachOpponent if !random && !announced => {
                Self::fixed_at_least(amount, 1, "discard count").map(|()| Cell::DiscardGroup)
            }
            _ => unaudited("discard recipient"),
        }
    }
}

/// CR 701.9: each recipient names its reviewed surface. The controller's
/// discard is named by its complete program (`Definition::requirements`).
pub(super) fn capabilities(player: &PlayerExpr, random: bool, out: &mut BTreeSet<Capability>) {
    match player {
        PlayerExpr::EachPlayer | PlayerExpr::EachOpponent => {
            out.insert(Capability::GroupHandDiscardV1);
        }
        _ if random => {
            out.insert(Capability::RandomPlayerDiscardV1);
        }
        PlayerExpr::Is {
            who: Ref::Target { .. },
        } => {
            out.insert(Capability::TargetPlayerDiscardV1);
        }
        PlayerExpr::Is {
            who: Ref::EventPlayer { .. },
        } => {
            out.insert(Capability::EventPlayerDiscardV1);
        }
        _ => {}
    }
}
