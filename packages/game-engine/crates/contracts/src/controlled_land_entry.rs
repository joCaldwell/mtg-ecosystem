//! CR 109.5, 603.2c, 603.3a/b, 603.6a and 603.10 (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const PREFIX: &str = "Whenever a land you control enters, you gain ";
const INVALID: &str = "requires a complete controlled land entry trigger with fixed life gain and optional printed draw instruction";

pub(super) fn permanent_spell() -> SpellProgram {
    SpellProgram {
        functioning_zone: FunctioningZone::Stack,
        timing: SpellTiming::MainPhase,
        resolution: SpellResolution::Permanent,
        instructions: vec![],
        targets: vec![],
        modes: vec![],
        optional_additional_costs: vec![],
        additional_sacrifice: None,
    }
}
