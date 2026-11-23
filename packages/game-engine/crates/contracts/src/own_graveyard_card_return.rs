//! CR 109.5, 400.3, 601.2c, 603.3d, 603.5 and 608.2b–d.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const INVALID: &str = "requires the complete own-graveyard card return spell or self-entry trigger";

fn selection(entry: bool) -> &'static str {
    if entry { "line:0:8" } else { "line:0:1" }
}

pub(super) fn abilities(entry: bool) -> Value {
    let target = json!({"ref":"target","filter":{"cls":"card","zone":{"zone":"graveyard","owner":"your"}},
        "count":{"amount":"fixed","value":1},"selectionId":selection(entry)});
    let mut step = json!({"effect":"return-to-hand","what":target,"to":"your"});
    if entry {
        step["optional"] = json!(true);
        json!([{"kind":"triggered","trigger":{"trigger":"enters","what":{"ref":"this","noun":"creature"}},
            "effects":[{"steps":[step]}]}])
    } else {
        json!([{"kind":"spell","effects":[{"steps":[step]}]}])
    }
}
