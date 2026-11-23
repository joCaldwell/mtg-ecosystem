//! Source creature pays fixed mana and taps to tap one targeted creature.
//! CR 302.6, 602.2b, 608.2b, 701.26a and 701.26b.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const INVALID: &str = "requires complete fixed-mana and source-tap activation to tap one creature";

/// Rule binding for the reviewed intrinsic keyword siblings.
pub fn keyword(k: KeywordAbility) -> Option<(&'static str, &'static str)> {
    match k {
        KeywordAbility::Defender => Some(("defender", "702.3")),
        KeywordAbility::Flying | KeywordAbility::Vigilance => fixed_activation_keyword(k),
        _ => None,
    }
}
