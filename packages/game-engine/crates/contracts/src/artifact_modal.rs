//! Fixed artifact-set payment, CR 601.2b/c/h, 602.2b, 700.2, 701.21a.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

fn positive(s: &str) -> Option<u32> {
    let n = s.parse::<u32>().ok()?;
    (n > 0 && n.to_string() == s).then_some(n)
}

fn count(s: &str) -> Option<u32> {
    positive(s).or_else(|| {
        [
            "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
        ]
        .iter()
        .position(|x| *x == s)
        .map(|i| i as u32 + 2)
    })
}

fn program() -> SpellProgram {
    SpellProgram {
        timing: SpellTiming::Instant,
        resolution: SpellResolution::Instructions,
        functioning_zone: FunctioningZone::Stack,
        targets: vec![],
        instructions: vec![],
        modes: vec![],
        additional_sacrifice: None,
        optional_additional_costs: vec![],
    }
}

fn signed(s: &str) -> Option<(i64, Value)> {
    let (sign, n) = if let Some(n) = s.strip_prefix('+') {
        (1, n)
    } else {
        (-1, s.strip_prefix('-')?)
    };
    let n = positive(n)?;
    Some((
        sign * i64::from(n),
        json!({"sign":sign,"amount":{"amount":"fixed","value":n}}),
    ))
}

// Keep exhaustive. Introducing planeswalker characteristics requires explicitly
// classifying them here before a catalog can use player-or-planeswalker damage.
