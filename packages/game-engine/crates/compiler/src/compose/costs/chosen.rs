//! Fixed chosen-object terms (CR 302.6, 602.2b, 701.9, 701.13, 701.26).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use ChosenCostAction::{Discard, ExileFromGraveyard, Tap};
use ChosenCostFilter::{Any, ArtifactOrCreature, Subtype, Type};
use serde_json::{Value, json};

pub(crate) fn lower(cost: &SourceCost) -> Result<ChosenObjectCost, Failure> {
    let (action, what) = match cost {
        SourceCost::Discard { what, random: None } => (Discard, what),
        SourceCost::TapObjects { what } => (Tap, what),
        SourceCost::Exile { what, from: None } => (ExileFromGraveyard, what),
        _ => return Err(gap()),
    };
    if what.as_object().is_none_or(|o| o.len() != 3) || what["ref"] != "filter" {
        return Err(gap());
    }
    let count = what["count"]["value"]
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .filter(|n| *n > 0)
        .ok_or_else(gap)?;
    if what["count"] != json!({"amount":"fixed","value":count}) {
        return Err(gap());
    }
    let mut filter = what["filter"].as_object().ok_or_else(gap)?.clone();
    match action {
        Discard | ExileFromGraveyard => {
            if filter.remove("cls") != Some(json!("card"))
                || (action == ExileFromGraveyard
                    && filter.remove("zone") != Some(json!({"owner":"your","zone":"graveyard"})))
            {
                return Err(gap());
            }
        }
        Tap => {
            if filter.remove("control") != Some(json!("you"))
                || filter.remove("status") != Some(json!(["untapped"]))
            {
                return Err(gap());
            }
        }
    }
    let another = match filter.remove("other") {
        None => false,
        Some(Value::Bool(true)) if action != Discard => true,
        _ => return Err(gap()),
    };
    let keys: Vec<_> = filter.keys().map(String::as_str).collect();
    let filter = match keys.as_slice() {
        [] if action != Tap => Any,
        ["cls"] if action == Tap && filter["cls"] == "permanent" => Any,
        ["types"] => {
            let Some([kind]) = filter["types"].as_array().map(Vec::as_slice) else {
                return Err(gap());
            };
            let kind = serde_json::from_value(kind.clone()).map_err(|_| gap())?;
            if action == Tap && matches!(kind, CardKind::Instant | CardKind::Sorcery) {
                return Err(gap());
            }
            Type(kind)
        }
        ["subtypes"] if action != Discard => {
            let Some([subtype]) = filter["subtypes"].as_array().map(Vec::as_slice) else {
                return Err(gap());
            };
            Subtype(
                subtype
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .ok_or_else(gap)?
                    .to_owned(),
            )
        }
        ["allOf"]
            if action == Tap
                && [("artifact", "creature"), ("creature", "artifact")]
                    .into_iter()
                    .any(|(a, b)| {
                        filter["allOf"] == json!([{"anyOf":[{"types":[a]},{"types":[b]}]}])
                    }) =>
        {
            ArtifactOrCreature
        }
        _ => return Err(gap()),
    };
    Ok(ChosenObjectCost {
        minimum_power: None,
        action,
        filter,
        count,
        another,
        tap_source: false,
        mana: None,
    })
}

fn gap() -> Failure {
    unsupported("compose.cost", "unsupported complete chosen-object cost")
}

fn plural(noun: &str) -> String {
    match noun {
        "Elf" => "Elves".into(),
        "Dwarf" => "Dwarves".into(),
        "Wolf" => "Wolves".into(),
        "Merfolk" | "Mouse" => {
            if noun == "Mouse" {
                "Mice".into()
            } else {
                noun.into()
            }
        }
        _ if noun.ends_with(['s', 'x']) || noun.ends_with("ch") || noun.ends_with("sh") => {
            format!("{noun}es")
        }
        _ => format!("{noun}s"),
    }
}

pub(super) fn matches_printed(cost: &ChosenObjectCost, term: &str) -> bool {
    let (verb, suffix) = match cost.action {
        Discard => ("Discard ", ""),
        Tap => ("Tap ", " you control"),
        ExileFromGraveyard => ("Exile ", " from your graveyard"),
    };
    let Some(words) = term.strip_prefix(verb).and_then(|t| t.strip_suffix(suffix)) else {
        return false;
    };
    let (quantity, noun) = match words.split_once(' ') {
        Some(parts) => parts,
        None => return false,
    };
    let number = match quantity {
        "a" | "an" | "one" | "another" => 1,
        "two" => 2,
        "three" => 3,
        "four" => 4,
        "five" => 5,
        "six" => 6,
        "seven" => 7,
        "eight" => 8,
        "nine" => 9,
        "ten" => 10,
        digits => digits.parse().unwrap_or(0),
    };
    if number != cost.count {
        return false;
    }
    // "Another" may precede the noun or follow the number ("seven other cards").
    let (other, noun) = if let Some(rest) = noun.strip_prefix("other ") {
        (true, rest)
    } else {
        (quantity == "another", noun)
    };
    if other != cost.another {
        return false;
    }
    let compound = match cost.action {
        Tap => match noun.strip_prefix("untapped ") {
            Some(noun) => noun,
            None => return false,
        },
        _ => noun,
    };
    let plural_count = cost.count > 1;
    let name = |noun: &str| {
        if plural_count {
            plural(noun)
        } else {
            noun.to_owned()
        }
    };
    let quality = match &cost.filter {
        Any => {
            return compound
                == name(if cost.action == Tap {
                    "permanent"
                } else {
                    "card"
                });
        }
        Type(kind) => match kind {
            CardKind::Artifact => "artifact",
            CardKind::Creature => "creature",
            CardKind::Planeswalker => "planeswalker",
            CardKind::Enchantment => "enchantment",
            CardKind::Instant => "instant",
            CardKind::Land => "land",
            CardKind::Sorcery => "sorcery",
        },
        Subtype(subtype) => subtype,
        ArtifactOrCreature => {
            let (artifact, creature) = (name("artifact"), name("creature"));
            return [" or ", " and/or "].iter().any(|join| {
                compound == format!("{artifact}{join}{creature}")
                    || compound == format!("{creature}{join}{artifact}")
            });
        }
    };
    if cost.action == Tap {
        compound == name(quality)
    } else {
        compound == format!("{quality} {}", name("card"))
    }
}
