//! Predefined artifact tokens, CR 111.10a–c,f.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PredefinedToken {
    Treasure,
    Gold,
    Food,
    Clue,
}
impl PredefinedToken {
    const ALL: [Self; 4] = [Self::Treasure, Self::Gold, Self::Food, Self::Clue];
    pub fn name(self) -> &'static str {
        match self {
            Self::Treasure => "Treasure",
            Self::Gold => "Gold",
            Self::Food => "Food",
            Self::Clue => "Clue",
        }
    }

    pub fn appears(text: &str) -> bool {
        Self::ALL
            .into_iter()
            .any(|k| text.contains(&format!(" {} token", k.name())))
    }
}

pub fn printed(
    clause: &str,
    context: kir::Context,
    source_block: u32,
) -> Option<(Value, Instruction)> {
    let clause = clause
        .strip_prefix("Create ")
        .or_else(|| clause.strip_prefix("create "))?;
    let (count, object) = clause.split_once(' ')?;
    let count = equipped_trigger::count(count)?;
    let noun = object.strip_suffix(if count == 1 { " token" } else { " tokens" })?;
    let kind = PredefinedToken::ALL
        .into_iter()
        .find(|k| k.name() == noun)?;
    Some((
        json!({"effect":"create-token","count":{"amount":"fixed","value":count},"token":{"predefined":kind.name()}}),
        Instruction::create_tokens(
            context,
            count,
            kir::TokenSpec::Predefined { kind },
            source_block,
        ),
    ))
}
