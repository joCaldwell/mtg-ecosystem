//! CR 106.1, 107.3c, 302.6, 605.1a, 605.3b and 608.2h (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(super) fn present(d: &Definition) -> bool {
    d.mana_abilities
        .iter()
        .any(|a| matches!(a.production, ManaProduction::AnyOneColor { .. }))
}
