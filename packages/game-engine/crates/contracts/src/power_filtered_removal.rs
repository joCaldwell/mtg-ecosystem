//! CR 601.2c, 608.2b. Restrict the new target predicate to its complete spell.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(super) fn deserialize_power<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<CreaturePowerFilter>, D::Error> {
    CreaturePowerFilter::deserialize(deserializer).map(Some)
}
