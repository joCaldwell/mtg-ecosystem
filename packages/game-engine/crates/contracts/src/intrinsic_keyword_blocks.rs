//! Source-relative intrinsic keywords. CR 113.3d, 702.14, and 702.16.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const INVALID: &str = "requires complete printed intrinsic keyword blocks";

#[derive(Debug, PartialEq, Eq)]
pub struct KeywordBlock {
    pub index: usize,
    pub occurrences: Vec<Vec<KeywordAbility>>,
}

fn is_special(k: KeywordAbility) -> bool {
    k.basic_landwalk_type().is_some() || k.fixed_protection_color().is_some()
}

fn simple(word: &str) -> Option<KeywordAbility> {
    serde_json::from_value(json!(word)).ok()
}

fn printed_special(word: &str) -> bool {
    let word = word.trim_start().to_ascii_lowercase();
    if let Some(rest) = word.strip_prefix("protection from ") {
        return rest
            .split([' ', '('])
            .next()
            .and_then(fixed_color_protection::keyword_for_color)
            .is_some();
    }
    word.split([' ', '('])
        .next()
        .and_then(simple)
        .is_some_and(|k| k.basic_landwalk_type().is_some())
}

fn atom(word: &str) -> Option<(Value, Vec<KeywordAbility>)> {
    if let Some(colors) = word.strip_prefix("protection from ") {
        let names = colors.split(" and from ").collect::<Vec<_>>();
        let keywords = names
            .iter()
            .map(|name| fixed_color_protection::keyword_for_color(name))
            .collect::<Option<Vec<_>>>()?;
        let from = names
            .iter()
            .map(|name| json!({"scope":"color","color":name}))
            .collect::<Vec<_>>();
        return Some((json!({"keyword":"protection","from":from}), keywords));
    }
    let keyword = simple(word)?;
    if matches!(
        keyword,
        KeywordAbility::ProtectionFromEachColor | KeywordAbility::ProtectionFromEverything
    ) || keyword.fixed_protection_color().is_some()
    {
        return None;
    }
    let ast = if let Some(land_type) = keyword.basic_landwalk_type() {
        json!({"keyword":"landwalk","filter":{"types":["land"],"subtypes":[land_type.subtype()]}})
    } else {
        json!({"keyword":word})
    };
    Some((ast, vec![keyword]))
}

pub fn source(text: &str, raw: &str) -> Result<Vec<KeywordBlock>, String> {
    let has_printed = text.split(['\n', ',', ';']).any(printed_special);
    if !has_printed
        && !["landwalk", "protection", r"\u"]
            .iter()
            .any(|s| raw.contains(s))
    {
        return Ok(vec![]);
    }
    let ast: Vec<Value> = strict_read(raw)?;
    let has_ast = ast.iter().any(|block| {
        block["kind"] == "keywords"
            && block["keywords"].as_array().is_some_and(|words| {
                words.iter().any(|w| {
                    w["keyword"] == "landwalk"
                        || (w["keyword"] == "protection"
                            && w["from"].as_array().is_some_and(|from| {
                                from.iter().any(|scope| scope["scope"] == "color")
                            }))
                })
            })
    });
    if !has_ast && !has_printed {
        return Ok(vec![]);
    }
    if ast.len() != text.lines().count() {
        return Err(INVALID.into());
    }
    let mut seen = Vec::new();
    let mut blocks = Vec::new();
    for (index, (line, raw)) in text.lines().zip(ast).enumerate() {
        if raw["kind"] != "keywords" && !line.split([',', ';']).any(printed_special) {
            continue;
        }
        let printed = basic_landwalk::without_reminder(line).ok_or(INVALID)?;
        let (atoms, occurrences): (Vec<_>, Vec<_>) = printed
            .split([',', ';'])
            .map(|part| atom(&part.trim().to_ascii_lowercase()))
            .collect::<Option<Vec<_>>>()
            .ok_or(INVALID)?
            .into_iter()
            .unzip();
        if raw != json!({"kind":"keywords","keywords":atoms}) {
            return Err(INVALID.into());
        }
        for keyword in occurrences.iter().flatten() {
            if seen.contains(keyword) {
                return Err(INVALID.into());
            }
            seen.push(*keyword);
        }
        blocks.push(KeywordBlock { index, occurrences });
    }
    if !seen.into_iter().any(is_special) {
        return Err(INVALID.into());
    }
    Ok(blocks)
}

pub(super) fn composition(d: &Definition) -> bool {
    !d.activated.is_empty() && d.keywords.iter().copied().any(is_special)
}
