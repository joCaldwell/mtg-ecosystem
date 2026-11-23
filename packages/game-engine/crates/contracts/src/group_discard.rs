//! CR 101.4, 603.2 and 701.9. Mandatory group discard has no targets.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

impl<'de> Deserialize<'de> for GroupDiscardScope {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match String::deserialize(deserializer)?.as_str() {
            "each_player" => Ok(Self::EachPlayer),
            "each_opponent" => Ok(Self::EachOpponent),
            _ => Err(serde::de::Error::custom("unsupported group discard scope")),
        }
    }
}
