//! Source-relative scalar ability blocks. CR 109.5, 602.2, 603.3, 608.2c.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub fn subject(kind: CardKind) -> Option<&'static str> {
    Some(match kind {
        CardKind::Creature => "creature",
        CardKind::Artifact => "artifact",
        CardKind::Enchantment => "enchantment",
        _ => return None,
    })
}

pub fn candidate(line: &str, kind: CardKind) -> bool {
    let Some(noun) = subject(kind) else {
        return false;
    };
    let lower = line.to_ascii_lowercase();
    let look = lower.contains("scry ") || lower.contains("surveil ");
    let token = PredefinedToken::appears(line);
    if token
        && ((kind == CardKind::Creature && line.starts_with("When this creature dies, "))
            || (line.starts_with("{T}: Create ") && kind == CardKind::Creature))
    {
        return true;
    }

    ((look || token)
        && (line.starts_with(&format!("When this {noun} enters, "))
            || (kind == CardKind::Creature
                && line.starts_with("Whenever this creature attacks, "))))
        || (line.starts_with('{')
            && line.contains(&format!(", Sacrifice this {noun}: "))
            && (look || line.contains(" life and draw a card.")))
}

fn fixed(n: u32) -> Value {
    json!({"amount":"fixed","value":n})
}

fn cost(text: &str, noun: &str) -> Option<(ActivationCost, Value)> {
    if text == "{T}" {
        return Some((ActivationCost::TapSource, json!([{"cost":"tap-self"}])));
    }
    let (text, sacrifice) = text
        .strip_suffix(&format!(", Sacrifice this {noun}"))
        .map_or((text, false), |s| (s, true));
    let (mut text, tap) = text
        .strip_suffix(", {T}")
        .map_or((text, false), |s| (s, true));
    if !sacrifice {
        return None;
    }
    let mut mana = ManaCost {
        generic: Quantity::zero(),
        colored: vec![],
    };
    let mut symbols = Vec::new();
    while !text.is_empty() {
        let (symbol, rest) = text.strip_prefix('{')?.split_once('}')?;
        match symbol {
            "W" => mana.colored.push(Color::W),
            "U" => mana.colored.push(Color::U),
            "B" => mana.colored.push(Color::B),
            "R" => mana.colored.push(Color::R),
            "G" => mana.colored.push(Color::G),
            _ => {
                let n = symbol.parse::<u32>().ok()?;
                if n.to_string() != symbol || !symbols.is_empty() {
                    return None;
                }
                mana.generic = i64::from(n).into();
            }
        }
        symbols.push(symbol);
        text = rest;
    }
    if symbols.is_empty() {
        return None;
    }
    let mut ast = vec![json!({"cost":"mana","symbols":symbols})];
    if tap {
        ast.push(json!({"cost":"tap-self"}))
    }
    ast.push(json!({"cost":"sacrifice","what":{"ref":"this","noun":noun}}));
    Some((
        ActivationCost::Composite {
            costs: PermanentActivationCosts {
                mana: Some(mana),
                tap,
                sacrifice,
                remove_source_plus_one_counters: None,
            },
        },
        json!(ast),
    ))
}
