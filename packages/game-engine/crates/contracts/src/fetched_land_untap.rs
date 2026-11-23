//! CR 400.7j, 608.2c and 701.26b (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const PREFIX: &str = " Then if you control ";
const SUFFIX: &str = " or more lands, untap that land.";
