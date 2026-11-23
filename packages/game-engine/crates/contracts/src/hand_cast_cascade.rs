//! CR 109.5, 514.2, 601.2i, 611.2f and 702.85a: later hand casts gain cascade.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const INVALID: &str = "hand-cast cascade requires the complete ordinary combat damage trigger";
