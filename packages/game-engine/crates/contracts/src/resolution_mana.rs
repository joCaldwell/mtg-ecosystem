//! Spell resolution produces unrestricted mana in the normal event batch.
//! CR 106.4, 601.2h, 605.5b, 608.2c and 608.2h.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ManaResolutionQuantity {
    Fixed {
        #[schemars(with = "String", regex(pattern = r"^[1-9][0-9]*$"))]
        amount: Quantity,
    },
    PerMatchingPermanent {
        #[schemars(with = "String", regex(pattern = r"^[1-9][0-9]*$"))]
        amount: Quantity,
        filter: PermanentFilter,
    },
}

pub fn candidate(text: &str) -> bool {
    text.lines().any(|line| line.starts_with("Add {"))
}
