//! CR 109.5, 205.3i, 614.12a and 701.20a–c (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const INVALID: &str =
    "requires exact optional two-type hand reveal entry and one tap-only dual mana ability";
