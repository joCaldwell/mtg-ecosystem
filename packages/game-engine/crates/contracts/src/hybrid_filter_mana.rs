//! A colored hybrid input and a single choice of an entire two-mana result.
//! CR 106.7, 107.4e, 601.2b/h, 602.2b, 605.3b and 733.1 (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

pub fn hybrid_filter_colors(definition: &Definition) -> Option<[Color; 2]> {
    match definition.mana_abilities.as_slice() {
        [
            _,
            ManaAbility {
                production: ManaProduction::HybridFilter { colors },
                ..
            },
        ] => Some(*colors),
        _ => None,
    }
}
