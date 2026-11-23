//! An entering land's own mandatory scry. CR 603.6a, 614.12 and 701.22
//! (2026-09-25). https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const ENTRY: &str = "This land enters tapped.";

const INVALID: &str = "requires a complete tapped dual land with one mandatory entry scry";

fn tapped() -> StaticAbility {
    StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::EntersTapped,
    }
}

pub(super) fn selector() -> Value {
    json!({"required":["characteristics","triggered"],"properties":{
        "characteristics":{"properties":{"kind":{"const":"land"}}},"triggered":{"contains":{
            "required":["condition","program"],"properties":{
                "condition":{"const":"self_enters_battlefield"},"program":{"not":{"$ref":"#/$defs/ScryFreeProgram"}}
            }
        }}
    }})
}
