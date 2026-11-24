//! Mill cells (CR 701.17).
use super::*;

impl Walk<'_, '_> {
    /// CR 701.17a: the controller or one announced player mills a fixed
    /// count; one announced player mills the announced X.
    pub(super) fn audit_mill(
        &self,
        player: &PlayerExpr,
        amount: &Amount,
    ) -> Result<Cell, KirError> {
        let announced = Self::fixed_or_announced_x(amount, 0, "mill count")?;
        match player {
            PlayerExpr::Is {
                who: Ref::Controller { .. },
            } if !announced => Ok(Cell::MillController),
            PlayerExpr::Is {
                who: Ref::Target { .. },
            } if self.player_target(player)? == Some(TargetDomain::AnyPlayer) => Ok(if announced {
                Cell::MillTargetPlayerX
            } else {
                Cell::MillTargetPlayer
            }),
            _ => unaudited("mill recipient"),
        }
    }
}

/// CR 701.17a, 107.3.
pub(super) fn capabilities(amount: &Amount, out: &mut BTreeSet<Capability>) {
    out.insert(if amount.is_announced_x() {
        Capability::AnnouncedXEffectsV1
    } else {
        Capability::FixedMillProgramV1
    });
}
