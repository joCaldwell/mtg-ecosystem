//! Life cells (CR 119.3).
use super::*;

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
