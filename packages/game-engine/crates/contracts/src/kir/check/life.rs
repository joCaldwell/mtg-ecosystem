//! Life cells (CR 119.3).
use super::*;

impl Walk<'_, '_> {
    /// CR 119.3: the controller, each opponent, one announced player or the
    /// trigger's event player gains or loses a fixed amount. A fixed zero
    /// (CR 119.8) is executed only as the reviewed tail of the
    /// permanent-grant family; program validation keeps every other life
    /// amount positive. CR 107.3: an announced X is the amount for the
    /// controller or one announced player only.
    pub(super) fn audit_life(
        &self,
        player: &PlayerExpr,
        amount: &Amount,
    ) -> Result<Cell, KirError> {
        let announced = Self::fixed_or_announced_x(amount, 0, "life amount")?;
        match player {
            PlayerExpr::Is {
                who: Ref::Controller { .. },
            } => Ok(Cell::LifeController),
            PlayerExpr::Is {
                who: Ref::EventPlayer { role },
            } if !announced && Self::recipient_role(*role) => Ok(Cell::LifeEventPlayer),
            PlayerExpr::EachOpponent if !announced => Ok(Cell::LifeEachOpponent),
            PlayerExpr::Is {
                who: Ref::Target { .. },
            } if self.player_target(player)? == Some(TargetDomain::AnyPlayer) => Ok(if announced {
                Cell::LifeTargetPlayerX
            } else {
                Cell::LifeTargetPlayer
            }),
            _ => unaudited("life recipient"),
        }
    }
}

pub(super) fn capabilities(gain: bool, player: &PlayerExpr, out: &mut BTreeSet<Capability>) {
    out.insert(if gain {
        Capability::LifeGainV1
    } else {
        Capability::LifeLossV1
    });
    if player.is_event_player() {
        out.insert(Capability::EventPlayerRecipientV1);
    }
    // The each-opponent life executor shares the reviewed group surface
    // that sacrifice admission gates.
    if *player == PlayerExpr::EachOpponent {
        out.insert(Capability::SacrificeV1);
    }
}
