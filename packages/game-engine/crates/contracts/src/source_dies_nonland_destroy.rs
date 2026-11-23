//! Complete source-dies destruction of nonland permanents (CR 603.10a,
//! 700.4, 701.8, 701.19c and 702.12b).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

/// CR 701.8, 701.19c: destroy every nonland permanent; none regenerates.
fn sweep(block: u32) -> Instruction {
    Instruction::act(
        Verb::Destroy {
            what: ObjectSet::NonlandPermanents.kernel(Context::Triggered),
            regeneration_forbidden: true,
        },
        block,
    )
}
