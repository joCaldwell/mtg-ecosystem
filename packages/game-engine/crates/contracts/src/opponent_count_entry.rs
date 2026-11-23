//! CR 109.5, 614.12, 800.4a and 806.1 (2026-09-25). Count the entering
//! controller's live opponents at the common prior rules state.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const INVALID: &str = "requires exact opponent-count entry and one tap-only dual mana ability";
const PREFIX: &str = "This land enters tapped unless you have ";
