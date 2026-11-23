//! CR 109.5, 201.5, 614.8, 701.19a: one paid ability establishes a
//! destruction replacement for its own incarnation.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const INVALID: &str = "requires complete fixed-mana regeneration of this creature";

/// Independently supported intrinsic keyword and its archive binding rule.
pub fn keyword(k: KeywordAbility) -> Option<(&'static str, &'static str)> {
    fixed_creature_activation::intrinsic_keyword(k)
}

/// Prove each printed keyword block before the final regeneration activation.
pub fn source_keywords(text: &str, raw_ast: &str) -> Result<Vec<Vec<KeywordAbility>>, String> {
    let ast: Vec<Value> = strict_read(raw_ast)?;
    let lines: Vec<_> = text.lines().collect();
    if lines.is_empty() || lines.len() != ast.len() || lines.join("\n") != text {
        return Err(INVALID.into());
    }
    let mut seen = Vec::new();
    lines[..lines.len() - 1]
        .iter()
        .zip(&ast)
        .map(|(line, actual)| {
            let (words, reminder) = line
                .split_once(" (")
                .map_or((*line, None), |(words, remainder)| {
                    (words, Some(format!("({remainder}")))
                });
            let mut keywords = Vec::new();
            let mut expected = Vec::new();
            for word in words.split(", ") {
                let parsed: KeywordAbility =
                    serde_json::from_value(json!(word.to_ascii_lowercase()))
                        .map_err(|_| INVALID)?;
                let (canonical_word, _) = keyword(parsed).ok_or(INVALID)?;
                if seen.contains(&parsed) {
                    return Err(INVALID.into());
                }
                seen.push(parsed);
                keywords.push(parsed);
                expected.push(json!({"keyword":canonical_word}));
            }
            let mut printed = expected
                .iter()
                .map(|k| k["keyword"].as_str().expect("known keyword"))
                .collect::<Vec<_>>()
                .join(", ");
            printed[..1].make_ascii_uppercase();
            let reminder_valid = match (keywords.as_slice(), reminder.as_deref()) {
                (_, None) => true,
                ([k], Some(text)) => fixed_creature_activation::keyword_reminder(*k) == Some(text),
                _ => false,
            };
            if words != printed
                || !reminder_valid
                || *actual != json!({"kind":"keywords","keywords":expected})
            {
                return Err(INVALID.into());
            }
            Ok(keywords)
        })
        .collect()
}
