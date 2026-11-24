//! Board-wide destruction and movement (CR 701.8, 400.3, 608.2h).
use super::super::*;
use mtg_contracts::kir::{Destination, Pred, ZoneExpr};

impl RulesTxn {
    /// CR 701.8, 400.3, 608.2h: read the set now, then destroy (`to` is
    /// `None`) or move every member in one simultaneous event.
    pub(in crate::engine) fn kernel_set_move(
        &mut self,
        frame: Resolving,
        zone: &ZoneExpr,
        filter: &Pred,
        to: Option<&Destination>,
        source_block: u32,
    ) -> Result<(), String> {
        let action = match to {
            None => wipes::SetMove::Destroy,
            Some(Destination::Exile) => wipes::SetMove::Exile,
            Some(Destination::OwnerHand) => wipes::SetMove::Hand,
            Some(_) => return Err("kernel set destination has no audited executor".into()),
        };
        let (objects, graveyards) = self.kernel_set(&frame, zone, filter)?;
        self.move_board_objects(frame, objects, action, source_block, graveyards)
    }
}
