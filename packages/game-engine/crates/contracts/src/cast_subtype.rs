//! Paid cast observers with subtype alternatives. CR 205.3, 601.2i and 603.3a.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

pub(crate) fn subtype_spelling(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.as_bytes()[0].is_ascii_uppercase()
        && s.bytes()
            .all(|b| b.is_ascii_alphabetic() || matches!(b, b'-' | b' '))
}

pub(super) fn restricted(p: &kir::Pred) -> bool {
    p.0.iter().any(|atom| match atom {
        kir::Atom::Subtype { .. } => true,
        kir::Atom::AnyOf { alternatives } => alternatives.iter().any(restricted),
        _ => false,
    })
}
