//! A tap-only artifact mana ability with an immediate life-gain effect.
//! CR 109.5, 119.3, 119.7, 605.1a and 605.3b (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const PREFIX: &str = "{T}: Add ";

const INVALID: &str = "requires complete fixed tap mana followed by mandatory controller life gain";

pub(super) fn present(d: &Definition) -> bool {
    d.mana_abilities
        .iter()
        .any(|ability| matches!(ability.production, ManaProduction::FixedWithLifeGain { .. }))
}

fn permanent_spell() -> SpellProgram {
    SpellProgram {
        functioning_zone: FunctioningZone::Stack,
        timing: SpellTiming::MainPhase,
        resolution: SpellResolution::Permanent,
        targets: vec![],
        instructions: vec![],
        modes: vec![],
        optional_additional_costs: vec![],
        additional_sacrifice: None,
    }
}
