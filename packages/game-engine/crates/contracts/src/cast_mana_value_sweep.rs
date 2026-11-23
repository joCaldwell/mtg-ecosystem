//! CR 202.3, 601.2i, 608.2h and 702.85c. Source-bound mana value of other casts.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const INVALID: &str =
    "requires a complete ordinary double cascade sorcery and other-casts damage sweep";

fn abilities() -> Value {
    json!([
        {"kind":"keywords","keywords":[{"keyword":"cascade"},{"keyword":"cascade"}]},
        {"kind":"spell","effects":[{"steps":[{"effect":"damage","source":{"ref":"self"},
            "amount":{"amount":"cast-mana-value-sum","who":{"player":"you"},"during":"this-turn",
                "excluding":{"ref":"this","noun":"spell"}},
            "to":{"to":"object","ref":{"ref":"each","filter":{"types":["creature"],"control":"opponent"}}}
        }]}]}
    ])
}

fn instruction() -> Instruction {
    SetDamage::instruction(
        vec![ObjectSet::OpponentCreatures.kernel(Context::Spell)],
        false,
        kir::Amount::OtherCastsManaValue {
            caster: PlayerExpr::you(Context::Spell),
        },
        1,
    )
}
