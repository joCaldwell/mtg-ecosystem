//! Fixed controller-side additive stats at layer 7c (CR 205.3m, 208.3,
//! 208.3a, 611.3a/b and 613.4c).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const INVALID: &str = "requires a complete printed controller-side scoped creature stat modifier";
// The frozen, reviewed subset of CR 205.3m creature types and their printed
// plurals. A type may occur in either the explicit creature intersection or
// the standalone subtype selector.
const SUBTYPES: [(&str, &str); 19] = [
    ("Cat", "Cats"),
    ("Dinosaur", "Dinosaurs"),
    ("Dragon", "Dragons"),
    ("Eldrazi", "Eldrazi"),
    ("Elf", "Elves"),
    ("Kithkin", "Kithkin"),
    ("Knight", "Knights"),
    ("Kobold", "Kobolds"),
    ("Merfolk", "Merfolk"),
    ("Pirate", "Pirates"),
    ("Sliver", "Slivers"),
    ("Soldier", "Soldiers"),
    ("Spawn", "Spawn"),
    ("Spirit", "Spirits"),
    ("Squirrel", "Squirrels"),
    ("Treefolk", "Treefolk"),
    ("Vampire", "Vampires"),
    ("Warrior", "Warriors"),
    ("Zombie", "Zombies"),
];

fn stat(word: &str) -> Option<(i64, Value)> {
    let (multiplier, digits) = if let Some(digits) = word.strip_prefix('+') {
        (1, digits)
    } else {
        (-1, word.strip_prefix('-')?)
    };
    let value = digits.parse::<u32>().ok()?;
    (value.to_string() == digits).then(|| {
        (
            multiplier * i64::from(value),
            json!({"sign":multiplier,"amount":{"amount":"fixed","value":value}}),
        )
    })
}

/// Derive the executable modifier and exact archive AST from the whole printed
/// sentence. An omitted creature qualifier is retained in the expected AST.
pub fn printed(text: &str) -> Option<(StaticAbility, Value)> {
    let (subject, magnitude) = text.strip_suffix('.')?.split_once(" you control get ")?;
    let (power, toughness) = magnitude.split_once('/')?;
    let (power, printed_power) = stat(power)?;
    let (toughness, printed_toughness) = stat(toughness)?;
    if power == 0 && toughness == 0 {
        return None;
    }
    let (subject, other) = subject
        .strip_prefix("Other ")
        .map_or((subject, false), |rest| (rest, true));
    let mut ast_filter = json!({"control":"you"});
    if other {
        ast_filter["other"] = json!(true);
    }
    let filter = match subject {
        "creatures" if other => {
            ast_filter["types"] = json!(["creature"]);
            ControllerCreatureStatFilter::Any
        }
        // The equal positive anthem retains its longstanding wire shape.
        "Creatures" if !other && (power < 0 || toughness < 0) => {
            ast_filter["types"] = json!(["creature"]);
            ControllerCreatureStatFilter::Any
        }
        "artifact creatures" | "Artifact creatures" => {
            ast_filter["types"] = json!(["artifact", "creature"]);
            ControllerCreatureStatFilter::Artifact
        }
        "Creatures" => return None,
        _ => {
            let subtypes = if let Some(explicit) = subject.strip_suffix(" creatures") {
                let values: Vec<_> = explicit.split(' ').collect();
                if !(1..=2).contains(&values.len())
                    || values
                        .iter()
                        .any(|v| !SUBTYPES.iter().any(|(singular, _)| v == singular))
                    || values.iter().collect::<BTreeSet<_>>().len() != values.len()
                {
                    return None;
                }
                ast_filter["types"] = json!(["creature"]);
                values
            } else {
                // A subtype-only selector can include Kindreds in the full
                // rules. A noncreature permanent has no stats to change. If
                // it becomes a creature, the live subtype filter is checked
                // again by the same layer traversal (CR 208.3/208.3a).
                vec![SUBTYPES.iter().find(|(_, plural)| subject == *plural)?.0]
            };
            ast_filter["subtypes"] = json!(subtypes);
            ControllerCreatureStatFilter::Subtypes {
                subtypes: subtypes.into_iter().map(str::to_owned).collect(),
            }
        }
    };
    Some((
        StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::ControllerScopedCreatureStats {
                filter,
                other,
                power: power.into(),
                toughness: toughness.into(),
            },
        },
        json!({"kind":"static","effect":{"static":"modify","what":{"ref":"each","filter":ast_filter},
            "modifiers":[{"modifier":"pt","power":printed_power,"toughness":printed_toughness}]}}),
    ))
}

// This profile consumes every source block. Simple intrinsic keyword blocks
// retain their ordinary IR. No unknown or parameterized keyword is inferred.
