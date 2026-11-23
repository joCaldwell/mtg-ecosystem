//! CR 109.5, 121.2, 601.2h and 602.2b: the activated ability uses the
//! composite source payment and draws only on resolution.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const INVALID: &str =
    "requires complete tapped dual land with mana, tap and source sacrifice to draw";

pub(super) fn present(d: &Definition) -> bool {
    d.characteristics.kind == CardKind::Land
        && d.activated.iter().any(|a| {
            matches!(&a.cost, ActivationCost::Composite { costs } if costs.tap && costs.sacrifice)
                && a.program
                    .instructions
                    .iter()
                    .any(Instruction::uncounted_draw)
        })
}

fn enters_tapped() -> StaticAbility {
    StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::EntersTapped,
    }
}
