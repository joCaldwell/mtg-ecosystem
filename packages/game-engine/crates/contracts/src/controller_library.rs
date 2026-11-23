//! Controller-only ordinary library choices. CR 109.5,608.2c,701.22/701.25.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt

// Match the archive producer's spelled-number vocabulary and canonical decimal text.
pub fn matches_fixed_amount(text: &str, amount: i64) -> bool {
    const UNITS: [&str; 20] = [
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "eight",
        "nine",
        "ten",
        "eleven",
        "twelve",
        "thirteen",
        "fourteen",
        "fifteen",
        "sixteen",
        "seventeen",
        "eighteen",
        "nineteen",
        "twenty",
    ];
    const TENS: [&str; 8] = [
        "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
    ];
    let decimal = amount.to_string();
    if text == decimal {
        return true;
    }
    match amount as usize {
        n @ 1..=20 => text == UNITS[n - 1],
        50 => text == "fifty",
        n @ 21..=99 if n % 10 != 0 => {
            text.split_once('-') == Some((TENS[n / 10 - 2], UNITS[n % 10 - 1]))
        }
        _ => false,
    }
}

// Source provenance and executable IR must each account for every fixed cast
// block. The source list also fixes its ordinal among printed triggered abilities.
