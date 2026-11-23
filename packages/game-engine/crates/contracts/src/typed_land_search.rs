//! Finite land-type OR search. CR 205.3i, 305.6, 701.23b (2026-09-25):
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum BasicLandType {
    Plains,
    Island,
    Swamp,
    Mountain,
    Forest,
}
impl BasicLandType {
    pub fn from_subtype(subtype: &str) -> Option<Self> {
        match subtype {
            "Plains" => Some(Self::Plains),
            "Island" => Some(Self::Island),
            "Swamp" => Some(Self::Swamp),
            "Mountain" => Some(Self::Mountain),
            "Forest" => Some(Self::Forest),
            _ => None,
        }
    }
    pub fn color(self) -> Color {
        match self {
            Self::Plains => Color::W,
            Self::Island => Color::U,
            Self::Swamp => Color::B,
            Self::Mountain => Color::R,
            Self::Forest => Color::G,
        }
    }
    pub fn subtype(self) -> &'static str {
        match self {
            Self::Plains => "Plains",
            Self::Island => "Island",
            Self::Swamp => "Swamp",
            Self::Mountain => "Mountain",
            Self::Forest => "Forest",
        }
    }
}
