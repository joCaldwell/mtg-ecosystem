//! Nontargeted exile, CR 109.2, 400.7, 608.2h, 701.11, 903.9a.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

fn subset(p: &SpellProgram) -> bool {
    p.resolution == SpellResolution::OneOrMoreModes || p.modes.iter().any(|m| subset(&m.program))
}

/// CR 406.1: exile every member of one closed set.
fn exile(set: &ObjectSet) -> Instruction {
    Instruction::act(
        Verb::Move {
            what: set.kernel(Context::Spell),
            to: Destination::Exile,
        },
        0,
    )
}
