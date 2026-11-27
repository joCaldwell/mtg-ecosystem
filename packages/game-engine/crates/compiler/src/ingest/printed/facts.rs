use super::{
    Failure, invalid,
    nodes::BlockNodes,
    quotes::Quotes,
    text,
    tokens::{self, Kind, Line, Token},
};
use mtg_oracle_ast::{
    printed::*,
    vocab::{EXCEPTIONS, INTRODUCERS, MENTIONS, POLARITIES},
};
use regex::Regex;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    ops::Range,
    sync::LazyLock,
};

static STATION_CITATION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^It['’]s an? (?:artifact )?creature at (\d+)\+\.$").unwrap());

fn word_number(word: &str) -> Option<u32> {
    // The current number spellings in oracle-parser/src/vocab.ts. The
    // generated vocabulary exports the closed fact classes.
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
    let value = |word: &str| {
        UNITS
            .iter()
            .position(|n| *n == word)
            .map(|index| index as u32 + 1)
    };
    if word == "fifty" {
        return Some(50);
    }
    if let Some((tens, units)) = word.split_once('-') {
        let tens = [
            "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
        ]
        .iter()
        .position(|word| *word == tens)?;
        return Some((tens as u32 + 2) * 10 + value(units).filter(|&n| n < 10)?);
    }
    value(word)
}

fn number(token: &Token<'_>) -> Option<u32> {
    if token.kind == Kind::Number {
        token.raw.parse().ok()
    } else {
        word_number(token.word())
    }
}

pub(super) fn location(fact: &PrintedFact) -> Option<(&str, [u32; 3], &Nullable<String>)> {
    macro_rules! location {
        ($value:ident) => {
            Some((&$value.at, $value.tok.0, &$value.field))
        };
    }
    match fact {
        PrintedFact::Exception(v) => location!(v),
        PrintedFact::Introducer(v) => location!(v),
        PrintedFact::Mention(v) => location!(v),
        PrintedFact::Optional(v) => location!(v),
        PrintedFact::Polarity(v) => location!(v),
        PrintedFact::Punct(v) => location!(v),
        PrintedFact::Quantity(v) => location!(v),
        PrintedFact::Target(v) => location!(v),
        PrintedFact::ReminderCitation(_) => None,
    }
}

fn matches_surface(
    fact: &PrintedFact,
    phrase: &[Token<'_>],
    next: Option<&Token<'_>>,
    edits: &[Edit],
) -> bool {
    let token = &phrase[0];
    let word = token.word();
    let single = phrase.len() == 1;
    let pair = |first, second| phrase.len() == 2 && word == first && phrase[1].word() == second;
    let target = || single && matches!(word, "target" | "targets");
    match fact {
        PrintedFact::Target(_) => target() && !token.possessive(),
        PrintedFact::Introducer(v) => match v.form.as_str() {
            "a" => single && matches!(word, "a" | "an"),
            "target" => target() && !token.possessive(),
            "numeral" => single && number(token).is_some(),
            "up-to" => pair("up", "to"),
            form => single && form == word && INTRODUCERS.contains(&word),
        },
        PrintedFact::Mention(v) => {
            let form = v.form.as_str();
            v.possessive.is_some() == (single && token.possessive())
                && match form {
                    "target" => target(),
                    "this-way" => pair("this", "way"),
                    "that-many" => pair("that", "many"),
                    "the-chosen" => pair("the", "chosen"),
                    "the-rest" => pair("the", "rest"),
                    "source-name" | "gendered" => {
                        single
                            && edits.iter().any(|edit| {
                                let [start, end] = text::ranges(edit).1;
                                start < token.n[1]
                                    && token.n[0] < end
                                    && matches!(
                                        (form, edit),
                                        ("source-name", Edit::SelfName(_))
                                            | ("gendered", Edit::Pronoun(_))
                                    )
                            })
                    }
                    form => single && form == word && MENTIONS.contains(&word),
                }
        }
        PrintedFact::Quantity(v) => {
            use PrintedFactQuantityForm::*;
            single
                && match (&v.form, &v.value) {
                    (Digits, Nullable::Value(value)) => {
                        token.kind == Kind::Number && number(token) == Some(*value)
                    }
                    (Word, Nullable::Value(value)) => word_number(word) == Some(*value),
                    (Article, Nullable::Value(1)) => matches!(word, "a" | "an"),
                    (X, Nullable::Null) => word == "x",
                    _ => false,
                }
        }
        PrintedFact::Polarity(v) => match v.form.as_str() {
            "non-prefix" => {
                single
                    && word.starts_with("non")
                    && word.len() > 3
                    && word != "none"
                    && v.n
                        .as_ref()
                        .is_some_and(|range| range.0 == [token.n[0], token.n[0] + 3])
            }
            form => single && form == word && POLARITIES.contains(&word) && v.n.is_none(),
        },
        PrintedFact::Optional(v) => match v.form.as_str() {
            "up-to" => pair("up", "to"),
            form => {
                single
                    && word == "may"
                    && (form == "may-have") == next.is_some_and(|t| t.word() == "have")
            }
        },
        PrintedFact::Exception(v) => match v.form.as_str() {
            "other-than" => pair("other", "than"),
            "rather-than" => pair("rather", "than"),
            form => {
                single
                    && form == word
                    && EXCEPTIONS.contains(&word)
                    && (form != "other" || !next.is_some_and(|t| t.word() == "than"))
            }
        },
        PrintedFact::Punct(v) => {
            single
                && token.punct().is_some_and(|punct| {
                    let role = match punct {
                        "." => "sentence-end",
                        "," => "comma",
                        ":" => "colon",
                        "—" => "dash",
                        "•" => "bullet",
                        "\"" => "quote",
                        "'" => "inner-quote",
                        _ => "other",
                    };
                    v.role.as_str() == role
                })
        }
        PrintedFact::ReminderCitation(_) => false,
    }
}

fn matches_field(
    fact: &PrintedFact,
    nodes: &BlockNodes<'_>,
    at: &str,
    field: &str,
    word: &str,
) -> bool {
    let (Some(owner), Some(value)) = (nodes.value(at), nodes.value(field)) else {
        return false;
    };
    let property = field.strip_prefix(at);
    match fact {
        PrintedFact::Quantity(v) => matches!(v.value, Nullable::Value(number)
            if property == Some("/value") && owner["amount"] == "fixed" && *value == number),
        PrintedFact::Optional(v) => {
            *value == true
                && property
                    == Some(if v.form.as_str() == "up-to" {
                        "/upTo"
                    } else {
                        "/optional"
                    })
        }
        PrintedFact::Exception(v) => {
            matches!(v.form.as_str(), "another" | "other")
                && *value == true
                && matches!(property, Some("/other" | "/filter/other"))
        }
        PrintedFact::Polarity(v) => match v.form.as_str() {
            "only" => property == Some("/restriction") && *value == "only",
            "can't" => property == Some("/effect") && *value == "cant",
            "non-prefix" => {
                let mut matching = ["nonTypes", "nonSubtypes", "nonSupertypes"]
                    .into_iter()
                    .filter(|key| {
                        owner[*key].as_array().is_some_and(|traits| {
                            traits.iter().any(|value| {
                                value
                                    .as_str()
                                    .is_some_and(|value| value.to_lowercase() == word[3..])
                            })
                        })
                    });
                matching
                    .next()
                    .is_some_and(|key| property == Some(&format!("/{key}")))
                    && matching.next().is_none()
            }
            _ => false,
        },
        PrintedFact::Mention(v) => match v.form.as_str() {
            "this-way" => property == Some("/scope") && *value == "this-way",
            "you" | "your" => property == Some("/player") && *value == "you",
            _ => false,
        },
        _ => false,
    }
}

fn target_suffix(nodes: &BlockNodes<'_>, first: bool, line: &Line<'_>) -> Option<usize> {
    let ast = nodes.ast.filter(|_| first)?;
    let delimiter = if ast["effect"]["static"] == "station-band" {
        "|"
    } else if ast["kind"] == "sticker" {
        "—"
    } else {
        return None;
    };
    line.tokens
        .iter()
        .position(|token| token.punct() == Some(delimiter))
        .map(|index| index + 1)
}

pub(super) fn verify(
    facts: &[PrintedFact],
    nodes: &BlockNodes<'_>,
    lines: &[Line<'_>],
    block: Range<usize>,
    face: &PrintedFaceRead,
    original: &str,
    quotes: &Quotes,
) -> Result<(), Failure> {
    let mut targets = BTreeMap::<&str, Vec<([u32; 2], u32)>>::new();
    let mut target_tokens = BTreeSet::new();
    let mut moved = BTreeSet::new();
    for (index, fact) in facts.iter().enumerate() {
        let reject = |check, reason| invalid(check, format!("fact {index}: {reason}"));
        let Some((at, run, field)) = location(fact) else {
            let PrintedFact::ReminderCitation(cite) = fact else {
                unreachable!()
            };
            let [start, end] = cite.o.0;
            let reminder = face
                .removed_reminders
                .get(cite.reminder_index as usize)
                .ok_or_else(|| reject("P8", "station citation has no removed reminder"))?;
            if start < reminder.o.0[0] || end > reminder.o.0[1] {
                return Err(reject("P8", "station citation exceeds its reminder"));
            }
            if quotes.intersects_original(cite.o.0) {
                return Err(reject("P7", "station citation crosses into a quote region"));
            }
            let threshold = text::slice(original, cite.o.0)
                .and_then(|text| STATION_CITATION.captures(text))
                .and_then(|capture| capture[1].parse::<u64>().ok());
            if threshold.is_none() {
                return Err(reject("P8", "invalid station creature threshold text"));
            }
            if cite.field != "/effect/creature/basePT"
                || nodes.value(&cite.field) != Some(&Value::from("printed-box"))
                || nodes.value("/effect/static") != Some(&Value::from("station-band"))
                || nodes.value("/effect/threshold").and_then(Value::as_u64) != threshold
            {
                return Err(reject(
                    "P10",
                    "station citation disagrees with the striation AST",
                ));
            }
            continue;
        };
        let [line_index, start, end] = run.map(|v| v as usize);
        let line = lines
            .get(line_index)
            .filter(|_| block.contains(&line_index))
            .ok_or_else(|| reject("P7", "fact line lies outside its block"))?;
        let phrase = line
            .tokens
            .get(start..end)
            .filter(|tokens| !tokens.is_empty())
            .ok_or_else(|| reject("P7", "invalid fact token run"))?;
        if at != nodes.owner(run) {
            return Err(reject(
                "P7",
                "fact does not name the innermost containing node",
            ));
        }
        let region = quotes.region((line_index, start));
        if (start + 1..end).any(|index| quotes.region((line_index, index)) != region)
            || (nodes.ast.is_some() && !nodes.quotes.allows_fact(at, region))
        {
            return Err(reject(
                "P7",
                "fact crosses or escapes its owner's quote region",
            ));
        }
        if !matches_surface(fact, phrase, line.tokens.get(end), &face.edits) {
            return Err(reject(
                "P8",
                "fact form or value disagrees with its token spelling",
            ));
        }
        let token = &phrase[0];
        if let PrintedFact::Punct(value) = fact
            && (value.moved.is_some() != (token.grammar_index != start)
                || value.moved.is_some() && !moved.insert((line_index, start)))
        {
            return Err(reject(
                "P5",
                "punctuation move flag disagrees with quote order",
            ));
        }
        if let PrintedFact::Target(target) = fact {
            let Nullable::Value(field) = field else {
                return Err(reject("P9", "target selection field is absent"));
            };
            let path = field
                .strip_suffix("/selectionId")
                .ok_or_else(|| reject("P9", "wrong target selection field"))?;
            if nodes.scope(path) != Some(&target.scope)
                || nodes.value(field).and_then(Value::as_str) != Some(&target.selection_id)
                || !tokens::selection_matches(
                    line,
                    start,
                    &target.selection_id,
                    target_suffix(nodes, line_index == block.start, line),
                )
                || !target_tokens.insert((line_index, start))
            {
                return Err(reject(
                    "P9",
                    "target selection, scope or token occurrence disagrees",
                ));
            }
            targets
                .entry(&target.scope)
                .or_default()
                .push((text::hull(token.n, &face.edits), target.target_index));
        } else if let Nullable::Value(field) = field
            && !matches_field(fact, nodes, at, field, token.word())
        {
            return Err(reject(
                "P10",
                "fact disagrees with the linked legacy AST field",
            ));
        }
    }
    for (scope, selections) in &mut targets {
        selections.sort_unstable();
        if selections
            .iter()
            .enumerate()
            .any(|(index, (_, value))| index != *value as usize)
        {
            return Err(invalid(
                "P9",
                format!("target indices for {scope:?} differ from original order"),
            ));
        }
    }
    for line in block {
        for (index, token) in lines[line].tokens.iter().enumerate() {
            if token.grammar_index != index && !moved.contains(&(line, index)) {
                return Err(invalid("P5", "missing moved punctuation fact"));
            }
        }
    }
    Ok(())
}
