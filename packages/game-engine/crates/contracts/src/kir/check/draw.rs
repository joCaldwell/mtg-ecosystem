//! Draw cells (CR 121).
use super::*;

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
