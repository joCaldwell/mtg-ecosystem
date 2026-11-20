//! Intrinsic creature protection from named colors, CR 702.16a–g.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const INVALID: &str = "requires exact printed fixed-color protection and intrinsic keywords";
const PREFIX: &str = "protection from ";
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

/// Preserve the raw block and occurrence boundaries for binding. A printed
/// "from [A] and from [B]" occurrence represents two independent abilities.
pub fn source_keywords(
    text: &str,
    raw_ast: &str,
) -> Result<Option<Vec<Vec<Vec<KeywordAbility>>>>, String> {
    if !raw_ast.contains("protection")
        && !text.contains("rotection from ")
        && !raw_ast.contains(r"\u")
    {
        return Ok(None);
    }
    let ast: Vec<Value> = strict_read(raw_ast)?;
    let printed = text.split(['\n', ',', ';']).any(|part| {
        let lower = part.trim_start().to_ascii_lowercase();
        lower.strip_prefix(PREFIX).is_some_and(|rest| {
            rest.split([' ', '('])
                .next()
                .and_then(keyword_for_color)
                .is_some()
        })
    });
    if !printed
        && !ast.iter().any(|block| {
            block["kind"] == "keywords"
                && block["keywords"].as_array().is_some_and(|words| {
                    words.iter().any(|word| {
                        word["keyword"] == "protection"
                            && word["from"].as_array().is_some_and(|scopes| {
                                scopes.iter().any(|from| from["scope"] == "color")
                            })
                    })
                })
        })
    {
        return Ok(None);
    }
    let error = || INVALID.to_string();
    if ast.len() != text.lines().count() {
        return Err(error());
    }
    let mut seen = Vec::new();
    let mut blocks = Vec::new();
    for (block, raw) in text.lines().zip(ast) {
        let printed = basic_landwalk::without_reminder(block).ok_or_else(error)?;
        let mut words = Vec::new();
        let mut expected = Vec::new();
        for word in printed.split([',', ';']) {
            let word = word.trim().to_ascii_lowercase();
            let keywords = if let Some(names) = word.strip_prefix(PREFIX) {
                let names: Vec<_> = names.split(" and from ").collect();
                let keywords = names
                    .iter()
                    .map(|name| keyword_for_color(name))
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(error)?;
                let from: Vec<_> = names
                    .iter()
                    .map(|name| json!({"scope":"color","color":name}))
                    .collect();
                expected.push(json!({"keyword":"protection","from":from}));
                keywords
            } else {
                let keyword: KeywordAbility =
                    serde_json::from_value(json!(word)).map_err(|_| error())?;
                if keyword.fixed_protection_color().is_some()
                    || keyword.basic_landwalk_type().is_some()
                    || matches!(
                        keyword,
                        KeywordAbility::ProtectionFromEachColor
                            | KeywordAbility::ProtectionFromEverything
                    )
                {
                    return Err(error());
                }
                expected.push(json!({"keyword":word}));
                vec![keyword]
            };
            for keyword in &keywords {
                if seen.contains(keyword) {
                    return Err(error());
                }
                seen.push(*keyword);
            }
            words.push(keywords);
        }
        if raw != json!({"kind":"keywords","keywords":expected}) {
            return Err(error());
        }
        blocks.push(words);
    }
    if !seen.iter().any(|k| k.fixed_protection_color().is_some()) {
        return Err(error());
    }
    Ok(Some(blocks))
}

pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    let ast = &d.provenance.abilities_json;
    let raw = &d.provenance.source_json;
    let intrinsic = d
        .keywords
        .iter()
        .any(|k| k.fixed_protection_color().is_some());
    if !intrinsic
        && !raw.contains("rotection from ")
        && !ast.contains("protection")
        && !raw.contains(r"\u")
        && !ast.contains(r"\u")
    {
        return Ok(());
    }
    let source: Value = strict_read(raw)?;
    let text = source["oracle_text"].as_str().unwrap_or_default();
    match source_keywords(text, ast)? {
        None if !intrinsic => Ok(()),
        Some(blocks)
            if d.keywords
                .iter()
                .copied()
                .eq(blocks.iter().flatten().flatten().copied())
                && d.characteristics.kind == CardKind::Creature
                && d.activated.is_empty()
                && d.triggered.is_empty()
                && d.mana_abilities.is_empty()
                && d.static_abilities.is_empty()
                && source["layout"] == "normal"
                && source["oracle_id"] == d.oracle_id
                && source["name"] == d.name
                && source["id"] == d.provenance.printing_id =>
        {
            Ok(())
        }
        _ => Err(INVALID.into()),
    }
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
                "activated":{"maxItems":0},"triggered":{"maxItems":0},
                "mana_abilities":{"maxItems":0},"static_abilities":{"maxItems":0},
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
