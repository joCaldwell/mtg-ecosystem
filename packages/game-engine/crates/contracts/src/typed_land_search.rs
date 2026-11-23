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

/// All found cards enter together, then the controller shuffles. Hidden quality
/// searches may find zero even when singular (CR 701.23b). `force_tapped=false`
/// preserves ordinary entry effects; it never forces an untapped permanent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TypedLandSearch {
    pub any_basic_land_types: Vec<BasicLandType>,
    pub maximum: Quantity,
    pub force_tapped: bool,
}
impl TypedLandSearch {
    pub fn validate(&self) -> Result<(), String> {
        if self.maximum <= Quantity::zero()
            || self.any_basic_land_types.is_empty()
            || !self.any_basic_land_types.windows(2).all(|w| w[0] < w[1])
        {
            return Err(
                "typed land search requires positive maximum and canonical nonempty types".into(),
            );
        }
        Ok(())
    }
    pub fn accepts(&self, c: &Characteristics) -> bool {
        c.has_type(CardKind::Land)
            && self
                .any_basic_land_types
                .iter()
                .any(|t| c.subtypes.iter().any(|s| s == t.subtype()))
    }
}
