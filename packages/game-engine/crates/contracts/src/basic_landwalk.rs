//! Printed basic subtype landwalk on creatures, CR 702.14 and 305.6.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

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

pub(super) fn without_reminder(line: &str) -> Option<String> {
    let mut depth = 0_usize;
    let mut text = String::new();
    for ch in line.chars() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.checked_sub(1)?,
            _ if depth == 0 => text.push(ch),
            _ => {}
        }
    }
    (depth == 0).then_some(text)
}
