//! Closed fixed-mana nonmana creature activations. CR 109.5, 115.1c, 602.2b and 608.2b.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

fn positive(n: &str) -> Option<u32> {
    let value = n.parse::<u32>().ok()?;
    (value > 0 && value.to_string() == n).then_some(value)
}

pub(super) fn signed_stat(text: &str) -> Option<(Quantity, Value)> {
    let (sign, digits) = match text.as_bytes().first()? {
        b'+' => (1, &text[1..]),
        b'-' => (-1, &text[1..]),
        _ => return None,
    };
    let value: u32 = digits.parse().ok()?;
    (value.to_string() == digits).then(|| {
        (
            (i64::from(value) * sign).into(),
            json!({"sign":sign,"amount":{"amount":"fixed","value":value}}),
        )
    })
}

/// Intrinsic static keywords whose parser role and rules reference we can prove.
pub fn fixed_activation_keyword(keyword: KeywordAbility) -> Option<(&'static str, &'static str)> {
    Some(match keyword {
        KeywordAbility::Flying => ("flying", "702.9"),
        KeywordAbility::Reach => ("reach", "702.17"),
        KeywordAbility::Trample => ("trample", "702.19"),
        KeywordAbility::Vigilance => ("vigilance", "702.20"),
        _ => return None,
    })
}

pub(super) fn intrinsic_keyword(keyword: KeywordAbility) -> Option<(&'static str, &'static str)> {
    if let Some(bound) = fixed_activation_keyword(keyword) {
        return Some(bound);
    }
    Some(match keyword {
        KeywordAbility::Defender => ("defender", "702.3"),
        KeywordAbility::FirstStrike => ("first strike", "702.7"),
        KeywordAbility::DoubleStrike => ("double strike", "702.4"),
        KeywordAbility::Flash => ("flash", "702.8"),
        KeywordAbility::Haste => ("haste", "702.10"),
        KeywordAbility::Menace => ("menace", "702.111"),
        KeywordAbility::Lifelink => ("lifelink", "702.15"),
        KeywordAbility::Deathtouch => ("deathtouch", "702.2"),
        KeywordAbility::Hexproof => ("hexproof", "702.11"),
        KeywordAbility::Shroud => ("shroud", "702.18"),
        KeywordAbility::Indestructible => ("indestructible", "702.12"),
        _ => return None,
    })
}

pub(super) fn keyword_reminder(keyword: KeywordAbility) -> Option<&'static str> {
    Some(match keyword {
        KeywordAbility::Defender => "(This creature can't attack.)",
        KeywordAbility::FirstStrike => {
            "(This creature deals combat damage before creatures without first strike.)"
        }
        KeywordAbility::Flying => {
            "(This creature can't be blocked except by creatures with flying or reach.)"
        }
        KeywordAbility::Reach => "(This creature can block creatures with flying.)",
        KeywordAbility::Menace => {
            "(This creature can't be blocked except by two or more creatures.)"
        }
        KeywordAbility::Lifelink => {
            "(Damage dealt by this creature also causes you to gain that much life.)"
        }
        KeywordAbility::Flash => "(You may cast this spell any time you could cast an instant.)",
        KeywordAbility::Hexproof => {
            "(This creature can't be the target of spells or abilities your opponents control.)"
        }
        _ => return None,
    })
}
