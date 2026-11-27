//! Damage capabilities (CR 120).
use super::*;

/// CR 120.2b: directed noncombat damage; an announced X names its surface
/// too. Board-wide damage shares the reviewed wipe surface, and a
/// cast-history amount names the history surface.
pub(super) fn capabilities(to: &DamageTo, amount: &Amount, out: &mut BTreeSet<Capability>) {
    match to {
        DamageTo::Ref { .. } => {
            out.insert(Capability::NoncombatDamageV1);
            if amount.is_announced_x() {
                out.insert(Capability::AnnouncedXEffectsV1);
            }
        }
        _ => {
            out.insert(Capability::BoardWipesV1);
            if matches!(amount, Amount::OtherCastsManaValue { .. }) {
                out.extend([Capability::CastManaValueSweepV1, Capability::CastHistoryV1]);
            }
        }
    }
}
