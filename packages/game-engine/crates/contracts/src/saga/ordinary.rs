use crate::{CardKind, TargetDomain, TargetSpec};
/// Exactly the filtered recipient of a source-validated damage chapter.
pub fn damage_target(target: &TargetSpec) -> bool {
    target.domain == TargetDomain::BattlefieldPermanent
        && target.filter.as_ref().is_some_and(|filter| {
            filter.types == [CardKind::Creature]
                && filter.opponent
                && filter.non_types.is_empty()
                && filter.non_colors.is_empty()
                && filter.power.is_none()
                && !filter.tapped
        })
}
