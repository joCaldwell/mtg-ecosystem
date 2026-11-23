//! CR 109.5, 118.12a, 603.3a, 603.5 and 608.2d (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const PREFIX: &str =
    "Whenever an opponent casts a spell, you may draw a card unless that player pays ";
const INVALID: &str = "requires the complete opponent-cast optional draw and fixed caster payment";

fn abilities(generic: &Quantity) -> Value {
    json!([{"kind":"triggered","trigger":{"trigger":"cast","who":{"player":"an-opponent"},"what":{"cls":"spell"}},
        "effects":[{"steps":[{"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1},"optional":true}],
            "conditionKind":"unless","condition":{"condition":"pays","who":{"player":"that-player"},
                "cost":[{"cost":"mana","symbols":[generic.to_string()]}]}}]}])
}
