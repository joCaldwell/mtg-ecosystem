//! Intrinsic creature protection from named colors, CR 702.16a–g.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

const COLORS: [(Color, &str, KeywordAbility); 5] = [
    (Color::W, "white", KeywordAbility::ProtectionFromWhite),
    (Color::U, "blue", KeywordAbility::ProtectionFromBlue),
    (Color::B, "black", KeywordAbility::ProtectionFromBlack),
    (Color::R, "red", KeywordAbility::ProtectionFromRed),
    (Color::G, "green", KeywordAbility::ProtectionFromGreen),
];

impl KeywordAbility {
    pub fn fixed_protection_color(self) -> Option<Color> {
        COLORS
            .iter()
            .find_map(|(color, _, keyword)| (*keyword == self).then_some(*color))
    }
}

pub fn color_name(color: Color) -> Option<&'static str> {
    COLORS
        .iter()
        .find_map(|(c, name, _)| (*c == color).then_some(*name))
}
