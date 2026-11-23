//! Paid cast observers with subtype alternatives. CR 205.3, 601.2i and 603.3a.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(crate) fn subtype_spelling(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.as_bytes()[0].is_ascii_uppercase()
        && s.bytes()
            .all(|b| b.is_ascii_alphabetic() || matches!(b, b'-' | b' '))
}

fn body(line: &str) -> Option<(&str, &str)> {
    let rest = line
        .strip_prefix("Whenever you cast an ")
        .or_else(|| line.strip_prefix("Whenever you cast a "))?;
    rest.split_once(" spell, ")
}

fn filter(text: &str) -> Option<(Value, TypePredicate)> {
    let names = if let Some((start, end)) = text.rsplit_once(", or ") {
        start.split(", ").chain([end]).collect::<Vec<_>>()
    } else if let Some((a, b)) = text.split_once(" or ") {
        vec![a, b]
    } else {
        vec![text]
    };
    if names.len() > 16
        || names
            .iter()
            .enumerate()
            .any(|(i, s)| !subtype_spelling(s) || names[..i].contains(s))
    {
        return None;
    }
    let subtype = |name: &str| TypePredicate::Subtype {
        subtype: name.into(),
    };
    if let [name] = names.as_slice() {
        return Some((json!({"subtypes":[name],"cls":"spell"}), subtype(name)));
    }
    let terms = names.iter().map(|s| subtype(s)).collect();
    let any_of = names
        .iter()
        .map(|name| json!({"subtypes":[name]}))
        .collect::<Vec<_>>();
    Some((
        json!({"allOf":[{"anyOf":any_of}],"cls":"spell"}),
        TypePredicate::AnyOf { terms },
    ))
}

pub(super) fn restricted(p: &TypePredicate) -> bool {
    match p {
        TypePredicate::Subtype { .. } => true,
        TypePredicate::All { terms } | TypePredicate::AnyOf { terms } => {
            terms.iter().any(restricted)
        }
        _ => false,
    }
}
