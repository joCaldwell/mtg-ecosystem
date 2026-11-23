//! CR 603.2, 608.2g and 118.9. Source, selection and payment have distinct bindings.
use super::*;
use serde_json::{Value, json};

fn printed_trigger() -> String {
    let event = "Whenever this creature deals combat damage to a player";
    let offer = "you may cast a spell from your hand";
    format!("{event}, {offer} without paying its mana cost.")
}
