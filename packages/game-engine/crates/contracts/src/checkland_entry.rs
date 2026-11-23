//! CR 109.5, 205.3i, 305.6 and 614.12 (2026-09-25): a self-entry
//! replacement checks the entering controller's current basic land types.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const INVALID: &str = "requires complete checkland entry and one tap-only dual mana ability";
