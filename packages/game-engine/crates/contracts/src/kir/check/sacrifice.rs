//! Sacrifice capabilities (CR 701.21).
use super::*;

/// CR 701.21a: "each player" shares the reviewed wipe surface and "each
/// opponent" the reviewed group sacrifice surface. A designated permanent
/// ("sacrifice it") uses the exact-object surface.
pub(super) fn capabilities(player: &PlayerExpr, filter: &Pred, out: &mut BTreeSet<Capability>) {
    if matches!(filter.0.as_slice(), [Atom::Is { .. }]) {
        out.insert(Capability::SacrificeSourceV1);
        return;
    }
    out.insert(match player {
        PlayerExpr::EachPlayer => Capability::BoardWipesV1,
        PlayerExpr::EachOpponent
        | PlayerExpr::Is {
            who: Ref::Controller { .. },
        } => Capability::SacrificeV1,
        PlayerExpr::Is {
            who: Ref::Target { .. },
        } => Capability::TargetPlayerSacrificeCreatureV1,
        _ => Capability::EventPlayerSacrificeV1,
    });
}
