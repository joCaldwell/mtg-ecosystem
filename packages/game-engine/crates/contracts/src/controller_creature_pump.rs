//! CR 109.5, 608.2h, 611.2c and 613.4c: the spell's controller supplies the
//! fixed recipient set once, at resolution, for the rest of the turn.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const PREFIX: &str = "Creatures you control get ";

pub(super) fn program(p: &SpellProgram) -> bool {
    matches!(
        p.instructions.as_slice(),
        [i] if i.pumped().is_some_and(|(what, _, _, block)| block == 0
            && ObjectSet::ControllerCreatures.denotes(what))
    ) && p.targets.is_empty()
        && p.modes.is_empty()
        && p.additional_sacrifice.is_none()
        && p.optional_additional_costs.is_empty()
        && p.functioning_zone == FunctioningZone::Stack
        && p.resolution == SpellResolution::Instructions
}

fn uses(p: &SpellProgram) -> bool {
    p.instructions.iter().any(|i| {
        ObjectSet::pumped(i).is_some_and(|what| ObjectSet::ControllerCreatures.denotes(what))
    }) || p.modes.iter().any(|m| uses(&m.program))
}
