//! CR 602.1b, 602.5, 602.5b-d, 307.5 and 508.1 (2026-09-25). The printed
//! restriction is a permission gate, separate from the cost and effect.
//! "Activate only if [condition]" reads the current game state as the
//! activation begins (CR 602.5); it lowers to one audited
//! [`kir::ActivationCondition`], and the printed sentence must be exactly
//! the one the typed AST renders. Every other restriction rejects.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use kir::{Atom, Cmp, Cond};
use serde_json::{Map, Value, json};

fn reject() -> Failure {
    unsupported("restriction", "unsupported activation restriction")
}

/// Binds a printed player path ("/restriction/condition/who") to a kernel
/// player through the block's resolution.
pub(super) type PlayerAt<'a> = dyn Fn(&str) -> Result<Option<PlayerExpr>, Failure> + 'a;

#[derive(Default)]
struct Terms {
    timing: Option<SpellTiming>,
    window: Option<ActivationWindow>,
    once_each_turn: bool,
    condition: Option<Cond>,
}

pub(super) fn lower(
    source: Option<Value>,
    line: &str,
    player: &PlayerAt<'_>,
) -> Result<(SpellTiming, Option<ActivationRestriction>), Failure> {
    let Some(source) = source else {
        return if line.contains(". Activate only ") || line.starts_with("Activate only ") {
            Err(reject())
        } else {
            Ok((SpellTiming::Instant, None))
        };
    };
    let parts: Vec<(String, &Value)> = if source.get("restriction") == Some(&json!("all-of")) {
        let terms = source
            .get("restrictions")
            .and_then(Value::as_array)
            .filter(|terms| terms.len() >= 2 && source.as_object().is_some_and(|o| o.len() == 2))
            .ok_or_else(reject)?;
        terms
            .iter()
            .enumerate()
            .map(|(i, t)| (format!("/restriction/restrictions/{i}"), t))
            .collect()
    } else {
        vec![("/restriction".to_owned(), &source)]
    };
    let mut terms = Terms::default();
    // Every printed alternative of each term, in printed order.
    let mut phrases: Vec<Vec<String>> = vec![];
    for (path, term) in &parts {
        phrases.push(term_phrases(path, term, &mut terms, player)?);
    }
    let line = line.replace('\u{2019}', "'");
    let printed = combinations(&phrases)
        .into_iter()
        .any(|joined| line.ends_with(&format!("Activate only {joined}.")));
    if !printed {
        return Err(reject());
    }
    let timing = terms.timing.unwrap_or(SpellTiming::Instant);
    let restriction = (terms.window.is_some() || terms.once_each_turn || terms.condition.is_some())
        .then_some(ActivationRestriction {
            window: terms.window,
            once_each_turn: terms.once_each_turn,
            condition: terms.condition,
        });
    Ok((timing, restriction))
}

/// The phrases of all terms joined with " and only ", in printed order.
fn combinations(phrases: &[Vec<String>]) -> Vec<String> {
    phrases.iter().fold(vec![String::new()], |done, options| {
        done.iter()
            .flat_map(|head| {
                options.iter().map(move |o| {
                    if head.is_empty() {
                        o.clone()
                    } else {
                        format!("{head} and only {o}")
                    }
                })
            })
            .collect()
    })
}

fn term_phrases(
    path: &str,
    term: &Value,
    terms: &mut Terms,
    player: &PlayerAt<'_>,
) -> Result<Vec<String>, Failure> {
    if *term == json!({"restriction": "once-each-turn"}) && !terms.once_each_turn {
        terms.once_each_turn = true;
        return Ok(vec!["once each turn".into()]);
    }
    if *term == json!({"restriction": "sorcery"}) && terms.timing.is_none() && terms.window.is_none()
    {
        terms.timing = Some(SpellTiming::MainPhase);
        return Ok(vec!["as a sorcery".into()]);
    }
    let windows = [
        (
            json!({"restriction":"timing","turn":"your"}),
            ActivationWindow::YourTurn,
            "during your turn",
        ),
        (
            json!({"restriction":"timing","turn":"your","phase":"upkeep"}),
            ActivationWindow::YourUpkeep,
            "during your upkeep",
        ),
        (
            json!({"restriction":"timing","turn":"your","before":"declare-attackers"}),
            ActivationWindow::YourTurnBeforeAttackers,
            "during your turn, before attackers are declared",
        ),
        (
            json!({"restriction":"timing","phase":"combat"}),
            ActivationWindow::Combat,
            "during combat",
        ),
        (
            json!({"restriction":"timing","phase":"end-of-combat"}),
            ActivationWindow::EndOfCombat,
            "during the end of combat step",
        ),
        (
            json!({"restriction":"condition","condition":{"condition":"not-your-turn"}}),
            ActivationWindow::NotYourTurn,
            "if it's not your turn",
        ),
    ];
    if let Some((_, window, phrase)) = windows.iter().find(|(shape, ..)| term == shape) {
        if terms.window.is_some() || terms.timing.is_some() {
            return Err(reject());
        }
        terms.window = Some(*window);
        return Ok(vec![(*phrase).into()]);
    }
    let object = term.as_object().ok_or_else(reject)?;
    if term["restriction"] == "condition" && object.len() == 2 && terms.condition.is_none() {
        let (condition, phrases) = condition(&format!("{path}/condition"), &term["condition"], player)?;
        terms.condition = Some(condition);
        return Ok(phrases.into_iter().map(|p| format!("if {p}")).collect());
    }
    Err(reject())
}

/// One printed condition: its kernel form and every printed phrase of it.
fn condition(
    path: &str,
    value: &Value,
    player: &PlayerAt<'_>,
) -> Result<(Cond, Vec<String>), Failure> {
    let fields = value.as_object().map(Map::len).unwrap_or(0);
    let you = || PlayerExpr::you(kir::Context::Activated);
    let activator = |who: &str| -> Result<(), Failure> {
        (player(&format!("{path}/{who}"))? == Some(you()))
            .then_some(())
            .ok_or_else(reject)
    };
    let (cond, phrases) = match value["condition"].as_str() {
        // "if you control a [filter]": at least one such permanent.
        Some("control") if fields == 3 => {
            activator("who")?;
            let what = &value["what"];
            if what.as_object().map(Map::len) != Some(3)
                || what["ref"] != "filter"
                || what["count"] != json!({"amount": "fixed", "value": 1})
            {
                return Err(reject());
            }
            let filter = &what["filter"];
            let phrases = vec![format!("you control {}", singular(filter, false)?)];
            (controlled(filter, Cmp::AtLeast, 1.into())?, phrases)
        }
        // "if you control N or more [filter]" / "no [filter]".
        Some("control-count") if fields == 4 => {
            activator("who")?;
            let (cmp, n) = control::comparison(&value["comparison"])?;
            let noun = plural(&value["of"], false)?;
            let phrases = match (cmp, number(&n)?) {
                (Cmp::Exactly, "zero") => vec![format!("you control no {noun}")],
                (Cmp::AtLeast, word) if n > 1.into() => vec![
                    format!("you control {word} or more {noun}"),
                    format!("you control at least {word} {noun}"),
                ],
                _ => return Err(reject()),
            };
            (controlled(&value["of"], cmp, n)?, phrases)
        }
        // "if there are N or more [type] cards in your graveyard".
        Some("count") if fields == 3 => {
            let of = value["of"].as_object().ok_or_else(reject)?;
            if of.get("zone") != Some(&json!({"zone": "graveyard", "owner": "your"}))
                || of.get("cls") != Some(&json!("card"))
            {
                return Err(reject());
            }
            let mut filter = of.clone();
            filter.remove("zone");
            filter.remove("cls");
            let filter = Value::Object(filter);
            let (cmp, n) = control::comparison(&value["comparison"])?;
            let phrases = match (cmp, number(&n)?) {
                (Cmp::AtLeast, "one") => vec![format!(
                    "there is {} in your graveyard",
                    singular(&filter, true)?
                )],
                (Cmp::AtLeast, word) if n > 1.into() => {
                    let noun = plural(&filter, true)?;
                    vec![
                        format!("there are {word} or more {noun} in your graveyard"),
                        format!("there are at least {word} {noun} in your graveyard"),
                    ]
                }
                _ => return Err(reject()),
            };
            let pred = predicate(&filter)?;
            let set = kir::SetExpr::Matching {
                zone: kir::ZoneExpr::Graveyard { of: you() },
                filter: pred,
            };
            (count(set, cmp, n), phrases)
        }
        // "if you have no cards in hand" / "exactly seven cards in hand".
        Some("cards-in-hand") if fields == 3 => {
            activator("who")?;
            let (cmp, n) = control::comparison(&value["comparison"])?;
            let word = number(&n)?;
            let phrases = match (cmp, word) {
                (Cmp::Exactly, "zero") => vec!["you have no cards in hand".into()],
                (Cmp::Exactly, word) => vec![format!("you have exactly {word} cards in hand")],
                (Cmp::AtLeast, word) if n > 1.into() => vec![
                    format!("you have {word} or more cards in hand"),
                    format!("you have {word} or more cards in your hand"),
                ],
                (Cmp::AtMost, word) => vec![format!("you have {word} or fewer cards in hand")],
                _ => return Err(reject()),
            };
            let set = kir::SetExpr::Matching {
                zone: kir::ZoneExpr::Hand { of: you() },
                filter: kir::Pred::default(),
            };
            (count(set, cmp, n), phrases)
        }
        // CR 506.4: "if this creature is attacking".
        Some("status") if fields == 3 => {
            let (atom, word) = source_status(value)?;
            let filter = kir::Pred(vec![atom]);
            (source(filter), vec![format!("this creature is {word}")])
        }
        // CR 208.1: "if this creature's power is N or greater".
        Some("compare") if fields == 3 => {
            let measured = &value["value"];
            let stat = measured["attribute"].as_str();
            if measured.as_object().map(Map::len) != Some(3)
                || measured["amount"] != "attribute"
                || measured["of"] != json!({"noun": "creature", "ref": "this"})
            {
                return Err(reject());
            }
            let (cmp, n) = control::comparison(&value["comparison"])?;
            let bound = (0..=20).find(|&i| n == Quantity::from(i)).ok_or_else(reject)?;
            if cmp != Cmp::AtLeast {
                return Err(reject());
            }
            let value = kir::Amount::fixed(n);
            let (atom, stat) = match stat {
                Some("power") => (Atom::Power { cmp, value }, "power"),
                Some("toughness") => (Atom::Toughness { cmp, value }, "toughness"),
                _ => return Err(reject()),
            };
            let phrase = format!("this creature's {stat} is {bound} or greater");
            (source(kir::Pred(vec![atom])), vec![phrase])
        }
        // "if this creature is attacking or blocking": one fact of the source.
        Some("any-of")
            if fields == 2
                && value["conditions"]
                    .as_array()
                    .is_some_and(|parts| parts.iter().all(|p| p["condition"] == "status")) =>
        {
            let parts = value["conditions"].as_array().ok_or_else(reject)?;
            let statuses = parts
                .iter()
                .map(source_status)
                .collect::<Result<Vec<_>, _>>()?;
            let [(first, a), (second, b)] = statuses.as_slice() else {
                return Err(reject());
            };
            let filter = kir::Pred(vec![Atom::AnyOf {
                alternatives: vec![kir::Pred(vec![first.clone()]), kir::Pred(vec![second.clone()])],
            }]);
            (source(filter), vec![format!("this creature is {a} or {b}")])
        }
        // "A or B": at least one holds.
        Some("any-of") if fields == 2 => {
            let parts = value["conditions"]
                .as_array()
                .filter(|parts| parts.len() >= 2)
                .ok_or_else(reject)?;
            let mut conds = vec![];
            let mut phrases: Vec<Vec<String>> = vec![];
            for (i, part) in parts.iter().enumerate() {
                let (c, p) = condition(&format!("{path}/conditions/{i}"), part, player)?;
                conds.push(Cond::Not {
                    condition: Box::new(c),
                });
                phrases.push(p);
            }
            let joined = phrases.iter().fold(vec![String::new()], |done, options| {
                done.iter()
                    .flat_map(|head| {
                        options.iter().map(move |o| {
                            if head.is_empty() {
                                o.clone()
                            } else {
                                format!("{head} or {o}")
                            }
                        })
                    })
                    .collect()
            });
            let cond = Cond::Not {
                condition: Box::new(Cond::All { conditions: conds }),
            };
            (cond, joined)
        }
        _ => return Err(reject()),
    };
    kir::ActivationCondition::of(&cond).map_err(|_| reject())?;
    Ok((cond, phrases))
}

fn source(filter: kir::Pred) -> Cond {
    Cond::Satisfies {
        object: kir::Ref::Source,
        filter,
    }
}

/// CR 506.4: the source's printed combat status.
fn source_status(value: &Value) -> Result<(Atom, &'static str), Failure> {
    if value.as_object().map(Map::len) != Some(3)
        || value["what"] != json!({"noun": "creature", "ref": "this"})
    {
        return Err(reject());
    }
    match value["status"].as_str() {
        Some("attacking") => Ok((Atom::Attacking, "attacking")),
        Some("blocking") => Ok((Atom::Blocking, "blocking")),
        _ => Err(reject()),
    }
}

fn count(set: kir::SetExpr, cmp: Cmp, n: Quantity) -> Cond {
    Cond::Compare {
        left: kir::Amount::Count { set },
        cmp,
        right: kir::Amount::fixed(n),
    }
}

/// The permanents the activator controls matching a printed board filter.
fn controlled(filter: &Value, cmp: Cmp, n: Quantity) -> Result<Cond, Failure> {
    if filter.get("control").is_some() || filter.get("zone").is_some() {
        return Err(reject());
    }
    let mut pred = predicate(filter)?;
    pred.0.push(Atom::ControlledBy {
        player: PlayerExpr::you(kir::Context::Activated),
    });
    Ok(count(kir::SetExpr::battlefield(pred), cmp, n))
}

/// A printed filter's predicate; "a Plains or a Swamp" is one
/// [`Atom::AnyOf`] over the alternatives' own predicates.
fn predicate(filter: &Value) -> Result<kir::Pred, Failure> {
    if let Some(alternatives) = filter.get("anyOf") {
        let alternatives = alternatives
            .as_array()
            .filter(|a| a.len() >= 2 && filter.as_object().is_some_and(|o| o.len() == 1))
            .ok_or_else(reject)?
            .iter()
            .map(predicate)
            .collect::<Result<_, _>>()?;
        return Ok(kir::Pred(vec![Atom::AnyOf { alternatives }]));
    }
    let typed: ast::ObjectFilter = typed(filter, "filter")?;
    lower_filter(&typed, kir::Context::Activated)
}

fn number(n: &Quantity) -> Result<&'static str, Failure> {
    const WORDS: [&str; 13] = [
        "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
        "eleven", "twelve",
    ];
    (0..WORDS.len())
        .find(|&i| *n == Quantity::from(i as i64))
        .map(|i| WORDS[i])
        .ok_or_else(reject)
}

fn color_word(c: &Value) -> Result<&'static str, Failure> {
    Ok(match c.as_str() {
        Some("white") => "white",
        Some("blue") => "blue",
        Some("black") => "black",
        Some("red") => "red",
        Some("green") => "green",
        _ => return Err(reject()),
    })
}

/// The printed noun of a board or graveyard card filter, split into its
/// leading modifiers, head noun and trailing qualifiers. Only the listed
/// filter terms render; any other term rejects.
fn noun(filter: &Value, card: bool) -> Result<(Vec<String>, String, String), Failure> {
    let object = filter.as_object().ok_or_else(reject)?;
    const TERMS: [&str; 7] = [
        "supertypes",
        "colors",
        "subtypes",
        "types",
        "cls",
        "withAbility",
        "properties",
    ];
    if object.keys().any(|k| !TERMS.contains(&k.as_str())) {
        return Err(reject());
    }
    let strings = |key: &str| -> Result<Vec<String>, Failure> {
        match object.get(key) {
            None => Ok(vec![]),
            Some(Value::Array(items)) if !items.is_empty() => items
                .iter()
                .map(|v| v.as_str().map(str::to_owned).ok_or_else(reject))
                .collect(),
            Some(_) => Err(reject()),
        }
    };
    let mut lead = vec![];
    for s in strings("supertypes")? {
        if s != "legendary" && s != "basic" {
            return Err(reject());
        }
        lead.push(s);
    }
    if let Some(Value::Array(colors)) = object.get("colors") {
        for c in colors {
            lead.push(color_word(c)?.to_owned());
        }
    } else if object.contains_key("colors") {
        return Err(reject());
    }
    let subtypes = strings("subtypes")?;
    let types = strings("types")?;
    let cls = object.get("cls").map(|c| c.as_str().ok_or_else(reject)).transpose()?;
    if !matches!(cls, None | Some("permanent")) || card && cls.is_some() {
        return Err(reject());
    }
    let mut words: Vec<String> = subtypes;
    words.extend(types.iter().cloned());
    if card {
        words.push("card".into());
    } else if cls == Some("permanent") {
        if !types.is_empty() {
            return Err(reject());
        }
        words.push("permanent".into());
    }
    let head = words.pop().ok_or_else(reject)?;
    lead.extend(words);
    let mut tail = String::new();
    for ability in strings("withAbility")? {
        if ability != "flying" || !tail.is_empty() {
            return Err(reject());
        }
        tail.push_str(" with flying");
    }
    if let Some(properties) = object.get("properties") {
        let [property] = properties.as_array().map(Vec::as_slice).ok_or_else(reject)? else {
            return Err(reject());
        };
        let (cmp, n) = control::comparison(&property["comparison"])?;
        let stat = match property["property"].as_str() {
            Some(stat @ ("power" | "toughness")) => stat,
            _ => return Err(reject()),
        };
        if cmp != Cmp::AtLeast
            || property.as_object().map(Map::len) != Some(2)
            || !tail.is_empty()
        {
            return Err(reject());
        }
        let bound = (0..=20)
            .find(|&i| n == Quantity::from(i))
            .ok_or_else(reject)?;
        tail = format!(" with {stat} {bound} or greater");
    }
    Ok((lead, head, tail))
}

fn article(word: &str) -> &'static str {
    match word.chars().next().map(|c| c.to_ascii_lowercase()) {
        Some('a' | 'e' | 'i' | 'o' | 'u') => "an",
        _ => "a",
    }
}

/// "a Swamp", "an artifact", "a creature with flying", "a Plains or an
/// Island".
fn singular(filter: &Value, card: bool) -> Result<String, Failure> {
    if let Some(alternatives) = filter.get("anyOf") {
        let alternatives = alternatives.as_array().ok_or_else(reject)?;
        if filter.as_object().map(Map::len) != Some(1) || alternatives.len() < 2 {
            return Err(reject());
        }
        return Ok(alternatives
            .iter()
            .map(|a| singular(a, card))
            .collect::<Result<Vec<_>, _>>()?
            .join(" or "));
    }
    let (lead, head, tail) = noun(filter, card)?;
    let words: Vec<_> = lead.iter().map(String::as_str).chain([head.as_str()]).collect();
    let phrase = words.join(" ");
    Ok(format!("{} {phrase}{tail}", article(&phrase)))
}

/// "lands", "red permanents", "creature cards". Only a card type, "card" or
/// "permanent" pluralizes regularly; a subtype head rejects.
fn plural(filter: &Value, card: bool) -> Result<String, Failure> {
    let (lead, head, tail) = noun(filter, card)?;
    const REGULAR: [&str; 7] = [
        "artifact",
        "creature",
        "enchantment",
        "land",
        "planeswalker",
        "permanent",
        "card",
    ];
    if !REGULAR.contains(&head.as_str()) {
        return Err(reject());
    }
    let words: Vec<_> = lead.iter().map(String::as_str).collect();
    let lead = if words.is_empty() {
        String::new()
    } else {
        format!("{} ", words.join(" "))
    };
    Ok(format!("{lead}{head}s{tail}"))
}
