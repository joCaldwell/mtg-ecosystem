//! Source closure for the reviewed two-block c38aef79 reparse.
//! CR 106.4, 500.5, 505.1, 608.2c-d and 701.20.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

fn printed_amount(word: &str) -> Option<Quantity> {
    let words = [
        "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
    ];
    let value = words
        .iter()
        .position(|w| *w == word)
        .map(|n| Quantity::from(n as i64 + 1))
        .or_else(|| word.parse().ok())?;
    (value > Quantity::zero()).then_some(value)
}
