//! CR 305.1–3, 400.7j, 406.3, 601.2, 608.2c and 611.2a.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PlayWindow {
    ThisTurn,
    ThroughNextControllerTurn,
}

pub fn candidate(text: &str) -> bool {
    text.starts_with("Exile the top ") && text.contains("you may play those cards.")
}
