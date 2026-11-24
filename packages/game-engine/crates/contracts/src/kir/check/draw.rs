//! Draw cells (CR 121).
use super::*;

impl Walk<'_, '_> {
    /// CR 121.1-121.2, 608.2b: only the single-draw executor skips an
    /// illegal announced target and continues the remaining program.
    pub(super) fn audit_draw(
        &self,
        player: &PlayerExpr,
        amount: &Amount,
    ) -> Result<Cell, KirError> {
        let announced = Self::fixed_or_announced_x(amount, 0, "draw count")?;
        match player {
            PlayerExpr::Is {
                who: Ref::Controller { .. },
            } => Ok(if announced {
                Cell::DrawAnnouncedX
            } else if single_draw(player, amount) {
                Cell::DrawController
            } else {
                Cell::DrawControllerCounted
            }),
            PlayerExpr::EachPlayer => Ok(Cell::DrawEachPlayer),
            PlayerExpr::Is {
                who: Ref::EventPlayer { role },
            } if !announced && Self::recipient_role(*role) => Ok(Cell::DrawEventPlayer),
            PlayerExpr::Is {
                who: Ref::Target { .. },
            } if single_draw(player, amount)
                && self.player_target(player)? == Some(TargetDomain::AnyPlayer) =>
            {
                Ok(Cell::DrawTargetPlayer)
            }
            _ => unaudited("draw recipient or targeted count"),
        }
    }
}

/// CR 121.2c, 107.3: the group and announced-X draws name their own
/// surface. A fixed draw by one player is named by the complete program it
/// belongs to (`Definition::requirements`).
pub(super) fn capabilities(player: &PlayerExpr, amount: &Amount, out: &mut BTreeSet<Capability>) {
    if player.is_event_player() {
        out.insert(Capability::EventPlayerRecipientV1);
    }
    if *player == PlayerExpr::EachPlayer {
        out.insert(Capability::EachPlayerDrawV1);
    } else if amount.is_announced_x() {
        out.insert(Capability::AnnouncedXDrawV1);
    }
}
