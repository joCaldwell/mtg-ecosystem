//! Targeted combat restrictions through cleanup. CR 509.1b, 611.2c, 613.11 and 514.2.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub fn restriction(text: &str) -> Option<(CreatureRestriction, &'static str)> {
    Some(match text {
        "Target creature can't be blocked this turn." => {
            (CreatureRestriction::CantBeBlocked, "be-blocked")
        }
        "Target creature can't block this turn." => (CreatureRestriction::CantBlock, "block"),
        _ => return None,
    })
}

fn program(kind: CardKind, target: TargetSpec, instructions: Vec<Instruction>) -> SpellProgram {
    SpellProgram {
        functioning_zone: FunctioningZone::Stack,
        resolution: SpellResolution::Instructions,
        timing: if kind == CardKind::Sorcery {
            SpellTiming::MainPhase
        } else {
            SpellTiming::Instant
        },
        targets: vec![target],
        modes: vec![],
        optional_additional_costs: vec![],
        additional_sacrifice: None,
        instructions,
    }
}
