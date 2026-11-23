//! Equipment-owned triggered blocks. CR 109.5, 301.5, 603.3, 603.10a.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub fn count(text: &str) -> Option<u32> {
    if text == "a" || text == "an" {
        return Some(1);
    }
    text.parse::<u32>()
        .ok()
        .filter(|n| *n > 0 && n.to_string() == text)
        .or_else(|| (1..100).find(|&n| matches_fixed_amount(text, i64::from(n))))
}

pub fn draw_count(text: &str) -> Option<u32> {
    let (n, noun) = text.split_once(' ')?;
    let count = count(n)?;
    (noun == if count == 1 { "card" } else { "cards" }).then_some(count)
}
