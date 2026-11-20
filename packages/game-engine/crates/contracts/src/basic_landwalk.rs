//! Printed basic subtype landwalk on creatures, CR 702.14 and 305.6.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const WORDS: [&str; 5] = [
    "plainswalk",
    "islandwalk",
    "swampwalk",
    "mountainwalk",
    "forestwalk",
];
const INVALID: &str = "requires complete printed basic-subtype landwalk and intrinsic keywords";

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

fn without_reminder(line: &str) -> Option<String> {
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

/// Consume complete printed keyword lines and their exact archived AST blocks.
pub fn source_keywords(
    source_text: &str,
    raw_ast: &str,
) -> Result<Option<Vec<Vec<KeywordAbility>>>, String> {
    let printed_landwalk = source_text
        .split(['\n', ',', ';'])
        .map(str::trim_start)
        .filter_map(|word| word.split([' ', '(']).next())
        .any(|word| WORDS.iter().any(|w| word.eq_ignore_ascii_case(w)));
    if !printed_landwalk && !raw_ast.contains("landwalk") && !raw_ast.contains(r"\u") {
        return Ok(None);
    }
    let ast: Vec<Value> = strict_read(raw_ast)?;
    if !printed_landwalk
        && !ast.iter().any(|block| {
            block["kind"] == "keywords"
                && block["keywords"]
                    .as_array()
                    .is_some_and(|words| words.iter().any(|word| word["keyword"] == "landwalk"))
        })
    {
        return Ok(None);
    }
    let error = || INVALID.to_string();
    if ast.len() != source_text.lines().count() {
        return Err(error());
    }
    let mut seen = Vec::new();
    let blocks = source_text
        .lines()
        .zip(&ast)
        .map(|(line, block)| {
            let printed = without_reminder(line).ok_or_else(error)?;
            let mut words = Vec::new();
            let mut expected = Vec::new();
            for word in printed.split([',', ';']) {
                let word = word.trim().to_ascii_lowercase();
                let keyword: KeywordAbility =
                    serde_json::from_value(json!(word)).map_err(|_| error())?;
                if matches!(
                    keyword,
                    KeywordAbility::ProtectionFromEachColor | KeywordAbility::ProtectionFromEverything
                ) || seen.contains(&keyword)
                {
                    return Err(error());
                }
                seen.push(keyword);
                words.push(keyword);
                expected.push(if let Some(land_type) = keyword.basic_landwalk_type() {
                    json!({"keyword":"landwalk","filter":{"types":["land"],"subtypes":[land_type.subtype()]}})
                } else {
                    json!({"keyword":word})
                });
            }
            if *block != json!({"kind":"keywords","keywords":expected}) {
                return Err(error());
            }
            Ok(words)
        })
        .collect::<Result<Vec<_>, String>>()?;
    if !seen.iter().any(|k| k.basic_landwalk_type().is_some()) {
        return Err(error());
    }
    Ok(Some(blocks))
}

pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    let raw = &d.provenance.source_json;
    let ast = &d.provenance.abilities_json;
    let intrinsic = d.keywords.iter().any(|k| k.basic_landwalk_type().is_some());
    if !intrinsic
        && !raw.contains("walk")
        && !raw.contains(r"\u")
        && !ast.contains("landwalk")
        && !ast.contains(r"\u")
    {
        return Ok(());
    }
    let source: Value = strict_read(raw)?;
    let text = source["oracle_text"].as_str().unwrap_or_default();
    let printed = source_keywords(text, ast)?;
    match printed {
        None if !intrinsic => Ok(()),
        Some(blocks)
            if d.keywords
                .iter()
                .copied()
                .eq(blocks.iter().flatten().copied())
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
    let landwalk = json!({"contains":{"enum":WORDS}});
    let cap = json!({"contains":{"const":Capability::BasicLandwalkV1}});
    schema["$defs"]["Definition"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "if":{"required":["keywords"],"properties":{"keywords":landwalk}},
            "then":{"properties":{
                "characteristics":{"properties":{"kind":{"const":"creature"}}},
                "activated":{"maxItems":0},"triggered":{"maxItems":0},
                "mana_abilities":{"maxItems":0},"static_abilities":{"maxItems":0},
                "capabilities":cap
            }},
            "else":{"properties":{"capabilities":{"not":cap}}}
        }));
    let no_landwalk = json!({"items":{"not":{"enum":WORDS}}});
    schema["$defs"]["CreatureToken"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({"properties":{"keywords":no_landwalk}}));
    schema["$defs"]["StaticEffect"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({"properties":{"modify_creatures":{"properties":{"keywords":no_landwalk}}}}));
}
