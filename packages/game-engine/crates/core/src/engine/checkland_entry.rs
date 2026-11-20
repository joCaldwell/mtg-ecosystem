//! CR 109.5, 205.3i, 305.6 and 614.12 (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

// Membership uses derived subtypes. A land's produced mana and Basic
// supertype are irrelevant.
pub(super) fn record_land_types(
    land_types: &mut BTreeSet<(PlayerId, BasicLandType)>,
    controller: PlayerId,
    characteristics: &Characteristics,
) {
    land_types.extend(
        characteristics
            .subtypes
            .iter()
            .filter_map(|subtype| BasicLandType::from_subtype(subtype))
            .map(|subtype| (controller, subtype)),
    );
}

pub(super) fn controls_land_type(
    land_types: &BTreeSet<(PlayerId, BasicLandType)>,
    controller: PlayerId,
    subtypes: &[BasicLandType; 2],
) -> bool {
    subtypes
        .iter()
        .any(|subtype| land_types.contains(&(controller, *subtype)))
}
