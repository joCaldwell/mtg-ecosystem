//! CR 109.5, 201.5 and 614.12 (2026-09-25). The entering controller's
//! common-prior lands determine whether this source enters tapped.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const INVALID: &str = "requires exact other-land count entry and one tap-only dual mana ability";
const PREFIX: &str = "This land enters tapped unless you control ";
