//! Predefined artifact tokens, CR 111.10a–b,f.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PredefinedToken {
    Treasure,
    Food,
    Clue,
}
impl PredefinedToken {
    const ALL: [Self; 3] = [Self::Treasure, Self::Food, Self::Clue];
    pub fn name(self) -> &'static str {
        match self {
            Self::Treasure => "Treasure",
            Self::Food => "Food",
            Self::Clue => "Clue",
        }
    }
    fn reminder(self) -> &'static str {
        match self {
            Self::Treasure => {
                r#"It's an artifact with "{T}, Sacrifice this token: Add one mana of any color.""#
            }
            Self::Food => {
                r#"It's an artifact with "{2}, {T}, Sacrifice this token: You gain 3 life.""#
            }
            Self::Clue => r#"It's an artifact with "{2}, Sacrifice this token: Draw a card.""#,
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

pub(crate) fn reminder(text: &str, instructions: &[Instruction]) -> bool {
    instructions.iter().any(|i| {
        matches!(i,kernel!(CreateTokens { token: kir::TokenSpec::Predefined { kind }, .. })
        if text.strip_suffix(')')==Some(kind.reminder()))
    })
}
