//! Printed basic subtype landwalk on creatures, CR 702.14 and 305.6.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

impl KeywordAbility {
    pub fn basic_landwalk_type(self) -> Option<BasicLandType> {
        Some(match self {
            Self::Plainswalk => BasicLandType::Plains,
            Self::Islandwalk => BasicLandType::Island,
            Self::Swampwalk => BasicLandType::Swamp,
            Self::Mountainwalk => BasicLandType::Mountain,
            Self::Forestwalk => BasicLandType::Forest,
            _ => return None,
        })
    }
}
