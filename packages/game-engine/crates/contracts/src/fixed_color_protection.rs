//! Intrinsic creature protection from named colors, CR 702.16a–g.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

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

pub(super) fn keyword_for_color(name: &str) -> Option<KeywordAbility> {
    COLORS
        .iter()
        .find_map(|(_, color, keyword)| (*color == name).then_some(*keyword))
}

pub(super) fn extend_schema(schema: &mut Value) {
    let words = COLORS.map(|(_, _, keyword)| keyword);
    let fixed = json!({"contains":{"enum":words}});
    let cap = json!({"contains":{"const":Capability::FixedColorProtectionV1}});
    let attached = json!({"type":"object","required":["modify_creatures"],"properties":{"modify_creatures":{"required":["scope","keywords"],"properties":{
        "scope":{"enum":["equipped","enchanted"]},"keywords":fixed
    }}}});
    let has_grant = json!({"required":["static_abilities"],"properties":{"static_abilities":{"contains":{
        "required":["effect"],"properties":{"effect":attached}
    }}}});
    schema["$defs"]["Definition"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "if":{"required":["keywords"],"properties":{"keywords":fixed}},
            "then":{"properties":{
                "characteristics":{"properties":{"kind":{"const":"creature"}}},
                "capabilities":cap
            }},
            "else":{"if":has_grant,"then":{"properties":{"capabilities":cap}},"else":{"properties":{"capabilities":{"not":cap}}}}
        }));
    let intrinsic_only = json!({"items":{"not":{"enum":words}}});
    schema["$defs"]["CreatureToken"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({"properties":{"keywords":intrinsic_only}}));
    schema["$defs"]["StaticEffect"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({"properties":{"modify_creatures":{
            "if":{"properties":{"scope":{"enum":["equipped","enchanted"]}}},
            "else":{"properties":{"keywords":intrinsic_only}}
        }}}));
}
