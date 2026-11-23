//! "Choose one. If you control a commander as you cast this spell, you may
//! choose both instead." The commander condition authorizes one additional
//! mode choice at announcement. Each printed mode creates its own continuous
//! effect (CR 601.2b, 608.2c, 611.2c, 700.2a/d, 702.16h, 903.3).
//!
//! The executable shape is structural: three modes, the first two each a
//! grant of keywords to the creatures you control until end of turn and the
//! third both of them in printed order. The printed text, AST, labels and
//! capabilities are derived from those keywords, never from a card.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

/// The Oracle wording, AST form and rule of a keyword a mode may grant.
fn keyword(k: KeywordAbility) -> Option<(&'static str, Value, &'static str)> {
    let plain = |name: &'static str, rule| Some((name, json!({"keyword": name}), rule));
    match k {
        KeywordAbility::Flying => plain("flying", "702.9"),
        KeywordAbility::Vigilance => plain("vigilance", "702.20"),
        KeywordAbility::DoubleStrike => plain("double strike", "702.4"),
        KeywordAbility::FirstStrike => plain("first strike", "702.7"),
        KeywordAbility::Lifelink => plain("lifelink", "702.15"),
        KeywordAbility::Indestructible => plain("indestructible", "702.12"),
        KeywordAbility::Trample => plain("trample", "702.19"),
        KeywordAbility::Haste => plain("haste", "702.10"),
        KeywordAbility::Deathtouch => plain("deathtouch", "702.2"),
        KeywordAbility::ProtectionFromEachColor => Some((
            "protection from each color",
            json!({"keyword":"protection","from":[{"scope":"all-colors"}]}),
            "702.16",
        )),
        _ => None,
    }
}

fn mode(instructions: Vec<Instruction>) -> SpellProgram {
    SpellProgram {
        timing: SpellTiming::Instant,
        functioning_zone: FunctioningZone::Stack,
        resolution: SpellResolution::Instructions,
        instructions,
        targets: vec![],
        modes: vec![],
        additional_sacrifice: None,
        optional_additional_costs: vec![],
    }
}

fn listed(keywords: &[KeywordAbility]) -> String {
    let names: Vec<_> = keywords
        .iter()
        .filter_map(|k| keyword(*k))
        .map(|k| k.0)
        .collect();
    match names.as_slice() {
        [one] => (*one).to_owned(),
        [a, b] => format!("{a} and {b}"),
        [init @ .., last] => format!("{}, and {last}", init.join(", ")),
        [] => String::new(),
    }
}
