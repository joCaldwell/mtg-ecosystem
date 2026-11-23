//! The Demonstrate keyword (CR 702.144a, 707.10) and the reveal-and-cast
//! effect (CR 400.7j, 608.2c-g, 609.3, 701.20, 701.24), each lowered from its
//! own printed block. A definition is admitted as the reviewed composition:
//! a sorcery whose blocks are exactly those two, in that order.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

/// CR 702.144a: the keyword line with its reminder text.
pub const DEMONSTRATE: &str = "Demonstrate (When you cast this spell, you may copy it. If you do, choose an opponent to also copy it.)";
/// CR 701.20, 701.24, 608.2c-g: the reveal-and-cast effect line.
pub const REVEAL_AND_CAST: &str = concat!(
    "Shuffle your library, then reveal cards from the top of it until you reveal a nonland card. ",
    "Exile that card and put the rest on the bottom of your library in a random order. ",
    "You may cast the exiled card without paying its mana cost."
);
/// The printed blocks of the reviewed composition, in order.
fn text() -> String {
    format!("{DEMONSTRATE}\n{REVEAL_AND_CAST}")
}

/// The typed AST of the Demonstrate keyword block.
pub fn demonstrate_block() -> Value {
    json!({"kind":"keywords","keywords":[{"keyword":"demonstrate"}]})
}

/// The typed AST of the reveal-and-cast spell block.
pub fn reveal_and_cast_block() -> Value {
    abilities()[1].clone()
}

/// Which reviewed block a printed AST block is, if either.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Block {
    Demonstrate,
    RevealAndCast,
}

/// Classifies one printed block as one of the two reviewed blocks.
pub fn block(ast: &Value) -> Option<Block> {
    if *ast == demonstrate_block() {
        Some(Block::Demonstrate)
    } else if *ast == reveal_and_cast_block() {
        Some(Block::RevealAndCast)
    } else {
        None
    }
}
const INVALID: &str = "requires the complete targetless demonstrate and reveal-and-cast sorcery";

fn abilities() -> Value {
    // The keyword block, then the spell block.
    let you = json!({"player":"you"});
    let library = json!({"zone":"library","owner":"your"});
    let card = json!({"cls":"card"});
    json!([
        {"kind":"keywords","keywords":[{"keyword":"demonstrate"}]},
        {"kind":"spell","effects":[
            {"steps":[
                {"effect":"shuffle","who":you,"library":library},
                {"effect":"scan-library","who":you,"operation":"reveal","library":library,
                    "until":{"count":{"amount":"fixed","value":1},"matching":{"nonTypes":["land"],"cls":"card"}}}
            ]},
            {"steps":[
                {"effect":"exile","what":{"ref":"that","filter":card}},
                {"effect":"move-zone","what":{"ref":"rest"},
                    "to":{"zone":"library","owner":"your","position":"bottom"},"order":"random"}
            ]},
            {"steps":[{"effect":"play-card","instruction":{
                "who":you,"action":"cast","what":{"ref":"related","number":"singular","filter":card,
                    "relation":{"relation":"event-result","event":"exile","scope":"contextual"}},
                "alternativeCost":{"cost":"without-mana-cost"}},"optional":true}]}
        ]}
    ])
}

fn ast_marker(node: &Value) -> bool {
    match node {
        Value::Array(items) => items.iter().any(ast_marker),
        Value::Object(fields) => {
            node["keyword"] == "demonstrate"
                || node["effect"] == "scan-library"
                || fields.values().any(ast_marker)
        }
        _ => false,
    }
}
