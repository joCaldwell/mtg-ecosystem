//! CR 400.7j, 608.2c and 701.26b (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const PREFIX: &str = " Then if you control ";
const SUFFIX: &str = " or more lands, untap that land.";

pub fn fetched_land_untap_clause(text: &str, minimum: std::num::NonZeroU32) -> Option<Value> {
    let printed_amount = text.strip_prefix(PREFIX)?.strip_suffix(SUFFIX)?;
    if !matches_fixed_amount(printed_amount, i64::from(minimum.get())) {
        return None;
    }
    Some(json!({
        "steps": [{"effect":"untap","what":fetched_land_reference()}],
        "condition":{"condition":"control-count","who":{"player":"you"},
            "of":{"types":["land"]},"comparison":{"op":"ge",
                "value":{"amount":"fixed","value":minimum.get()}}},
        "conditionKind":"if"
    }))
}

pub fn fetched_land_reference() -> Value {
    json!({"ref":"that","filter":{"types":["land"]}})
}
