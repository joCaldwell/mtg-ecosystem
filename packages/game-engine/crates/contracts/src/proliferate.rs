//! CR 603.6a and 701.34a (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const CLAUSE: &str = "Whenever a land you control enters, proliferate.";

// One targeted, fixed Layer 7c modifier followed by the nontargeted keyword
// action (CR 608.2b/c and 701.34a). State-based actions wait until the
// complete spell has finished resolving (CR 704.4).

pub(super) fn selector() -> Value {
    json!({"properties":{"provenance":{"properties":{"source_json":{
        "pattern":r#""oracle_text"\s*:\s*"(?:[^"\\]* — )?Whenever a land you control enters, proliferate\.(?: \(Choose any number of permanents and/or players, then give each another counter of each kind already there\.\))?""#
    }}}}})
}
