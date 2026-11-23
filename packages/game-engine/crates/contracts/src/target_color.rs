//! CR 105.4, 608.2d, 611.2c, 613.1e and 613.7 (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const PREFIX: &str = "{T}: Target ";
const SUFFIX: &str = " becomes the color of your choice until end of turn.";
