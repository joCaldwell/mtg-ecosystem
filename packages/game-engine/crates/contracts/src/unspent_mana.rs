//! CR 106.4, 500.5, 614.1a and 614.6: the controller's unspent
//! mana is retained in the replacement color instead of being lost.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(super) fn text() -> String {
    format!(
        "If you would lose unspent mana, that mana becomes {} instead.",
        "black"
    )
}

fn abilities() -> Value {
    json!([{
        "kind":"static","effect":{"static":"replacement","replacement":{
            "event":{"event":"lose-unspent-mana","who":{"player":"you"}},
            "occurrence":"each","replace":{"replacement":"keep-mana","as":{"symbol":"B"}}
        }}
    }])
}
