//! CR 106.1b, 109.5, 605.1a and 605.3b. Choose one listed color during
//! the permanent's mana ability. The tap is the sole activation cost.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

pub(super) fn present(definition: &Definition) -> bool {
    matches!(
        definition.mana_abilities.as_slice(),
        [ManaAbility {
            production: ManaProduction::Choice { .. },
            ..
        }]
    )
}

// CR 205.3i/305.6: these subtypes confer no intrinsic mana ability.
pub(super) fn descriptive_land_subtype(definition: &Definition) -> bool {
    definition.characteristics.kind == CardKind::Land
        && !definition.characteristics.basic
        && matches!(definition.characteristics.subtypes.as_slice(),
            [name] if matches!(name.as_str(), "Gate" | "Town"))
        && present(definition)
}
