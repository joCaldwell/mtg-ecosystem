//! Board-wide destruction (CR 701.8, 608.2h).
use super::super::*;
use mtg_contracts::kir::{Pred, ZoneExpr};

impl RulesTxn {
    /// CR 701.8, 608.2h: read the set now, then destroy every member in one
    /// simultaneous event.
    pub(in crate::engine) fn kernel_destroy_set(
        &mut self,
        frame: Resolving,
        zone: &ZoneExpr,
        filter: &Pred,
        source_block: u32,
    ) -> Result<(), String> {
        let (objects, graveyards) = self.kernel_set(&frame, zone, filter)?;
        self.move_board_objects(
            frame,
            objects,
            wipes::SetMove::Destroy,
            source_block,
            graveyards,
        )
    }
}
