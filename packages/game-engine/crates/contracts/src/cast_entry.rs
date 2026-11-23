use super::*;
use serde_json::{Value, json};

// The IR condition stays independent of the effect. Source admission currently
// proves one complete self-entry operation and its printed condition.
fn contains_untap(p: &SpellProgram) -> bool {
    p.instructions.iter().any(|i| {
        matches!(
            i,
            kernel!(Untap {
                what: kir::SetExpr::Matching { .. }
            })
        )
    }) || p.modes.iter().any(|mode| contains_untap(&mode.program))
}
