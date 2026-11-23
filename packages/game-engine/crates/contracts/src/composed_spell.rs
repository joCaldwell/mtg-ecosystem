//! Bounded ordered ordinary spells. CR 109.5, 115, 601.2c, 608.2b/c,
//! 611.2c, 702.26. https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

// Slice 1 adapter: a source filter and its existing executable target domain
// describe the same current, announcement-time creature predicate.
#[derive(Clone, Copy)]
enum ObjectFilter {
    Creature,
    ControllerCreature,
}

impl ObjectFilter {
    fn read(raw: &Value) -> Option<Self> {
        if *raw == json!({"types":["creature"]}) {
            Some(Self::Creature)
        } else if *raw == json!({"types":["creature"],"control":"you"}) {
            Some(Self::ControllerCreature)
        } else {
            None
        }
    }

    fn domain(self) -> TargetDomain {
        match self {
            Self::Creature => TargetDomain::BattlefieldCreature,
            Self::ControllerCreature => TargetDomain::ControlledBattlefieldCreature,
        }
    }

    fn printed(self) -> &'static str {
        match self {
            Self::Creature => "target creature",
            Self::ControllerCreature => "target creature you control",
        }
    }
}

pub fn candidate(ast: &Value) -> bool {
    ast.as_array().is_some_and(|blocks| {
        blocks.iter().any(|block| {
            let starts_with_counter = block["effects"][0]["steps"][0]["effect"] == "put-counters";
            starts_with_counter
                && block["effects"].as_array().is_some_and(|sentences| {
                    sentences.iter().any(|sentence| {
                        sentence["steps"].as_array().is_some_and(|steps| {
                            steps.iter().any(|step| {
                                matches!(
                                    step["effect"].as_str(),
                                    Some("gain-abilities" | "phase-out")
                                )
                            })
                        })
                    })
                })
        })
    })
}

const COUNTER: &str = "put-counters";

fn fixed(value: &Value) -> Option<i64> {
    let n = value["value"].as_i64()?;
    (n >= 0 && *value == json!({"amount":"fixed","value":n})).then_some(n)
}

// CR 207.2a. Accept only reviewed explanatory sentences for present primitives.
