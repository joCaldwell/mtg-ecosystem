//! Closed ordinary scalar sequencing. CR 109.5, 305.2, 608.2b/c,
//! 701.17, 701.22, 701.25.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Operation {
    Scry,
    Surveil,
    Draw,
    Mill,
    TargetMill,
    Gain,
    Lose,
    Land,
    Destroy,
    Exile,
    Bounce,
    Discard,
}
impl Operation {
    fn is_library(self) -> bool {
        matches!(
            self,
            Self::Scry | Self::Surveil | Self::Mill | Self::TargetMill | Self::Land
        )
    }
}

#[derive(Clone)]
struct Clause {
    operation: Operation,
    count: i64,
    filter: Option<PermanentFilter>,
    archive_filter: Option<Value>,
}

fn fixed(n: i64) -> Value {
    json!({"amount":"fixed","value":n})
}

fn number(word: &str) -> Option<i64> {
    if matches!(word, "a" | "an") {
        return Some(1);
    }
    if let Ok(n) = word.parse::<u32>()
        && n > 0
        && n.to_string() == word
    {
        return Some(n.into());
    }
    (1..100).find(|&n| controller_library::matches_fixed_amount(word, n))
}

fn counted_noun(text: &str, singular: &str, plural: &str) -> Option<i64> {
    let (amount, noun) = text.split_once(' ')?;
    let n = number(amount)?;
    (noun == if n == 1 { singular } else { plural }).then_some(n)
}

fn permanent_filter(text: &str) -> Option<(PermanentFilter, Value)> {
    use CardKind::*;
    let (types, non_types, ast) = match text {
        "creature" => (vec![Creature], vec![], json!({"types":["creature"]})),
        "artifact" => (vec![Artifact], vec![], json!({"types":["artifact"]})),
        "land" => (vec![Land], vec![], json!({"types":["land"]})),
        "enchantment" => (vec![Enchantment], vec![], json!({"types":["enchantment"]})),
        "permanent" => (vec![], vec![], json!({"cls":"permanent"})),
        "nonland permanent" => (
            vec![],
            vec![Land],
            json!({"cls":"permanent","nonTypes":["land"]}),
        ),
        "nonblack creature" => {
            return Some((
                PermanentFilter {
                    types: vec![Creature],
                    non_types: vec![],
                    non_colors: vec![Color::B],
                    opponent: false,
                    tapped: false,
                    power: None,
                },
                json!({"types":["creature"],"nonColors":["black"]}),
            ));
        }
        "artifact or enchantment" => (
            vec![Artifact, Enchantment],
            vec![],
            json!({"allOf":[{"anyOf":[{"types":["artifact"]},{"types":["enchantment"]}]}]}),
        ),
        _ => return None,
    };
    Some((
        PermanentFilter {
            types,
            non_types,
            non_colors: vec![],
            opponent: false,
            tapped: false,
            power: None,
        },
        ast,
    ))
}

fn clause(text: &str) -> Option<Clause> {
    use Operation::*;
    let (operation, count) = if let Some(n) = text
        .strip_prefix("scry ")
        .or_else(|| text.strip_prefix("Scry "))
    {
        (Scry, number(n)?)
    } else if let Some(n) = text
        .strip_prefix("surveil ")
        .or_else(|| text.strip_prefix("Surveil "))
    {
        (Surveil, number(n)?)
    } else if let Some(n) = text
        .strip_prefix("Draw ")
        .or_else(|| text.strip_prefix("draw "))
        .or_else(|| text.strip_prefix("You draw "))
    {
        (Draw, counted_noun(n, "card", "cards")?)
    } else if let Some(n) = text.strip_prefix("Target player mills ") {
        (TargetMill, counted_noun(n, "card", "cards")?)
    } else if let Some(n) = text
        .strip_prefix("Mill ")
        .or_else(|| text.strip_prefix("mill "))
        .or_else(|| text.strip_prefix("You mill "))
    {
        (Mill, counted_noun(n, "card", "cards")?)
    } else if let Some(n) = text
        .strip_prefix("You gain ")
        .or_else(|| text.strip_prefix("gain "))
    {
        (Gain, number(n.strip_suffix(" life")?)?)
    } else if let Some(n) = text
        .strip_prefix("You lose ")
        .or_else(|| text.strip_prefix("lose "))
    {
        (Lose, number(n.strip_suffix(" life")?)?)
    } else if let Some(n) = text
        .strip_prefix("You may play ")
        .or_else(|| text.strip_prefix("you may play "))
    {
        (
            Land,
            counted_noun(
                n.strip_suffix(" this turn")?,
                "additional land",
                "additional lands",
            )?,
        )
    } else if let Some(n) = text
        .strip_prefix("Discard ")
        .or_else(|| text.strip_prefix("discard "))
        .or_else(|| text.strip_prefix("You discard "))
    {
        (Discard, counted_noun(n, "card", "cards")?)
    } else {
        let (op, target) = if let Some(t) = text.strip_prefix("Destroy target ") {
            (Destroy, t)
        } else if let Some(t) = text.strip_prefix("Exile target ") {
            (Exile, t)
        } else {
            (
                Bounce,
                text.strip_prefix("Return target ")?
                    .strip_suffix(" to its owner's hand")?,
            )
        };
        let (filter, archive_filter) = permanent_filter(target)?;
        return Some(Clause {
            operation: op,
            count: 0,
            filter: Some(filter),
            archive_filter: Some(archive_filter),
        });
    };
    Some(Clause {
        operation,
        count,
        filter: None,
        archive_filter: None,
    })
}

fn reminder(text: &str, clauses: &[Vec<Clause>]) -> bool {
    // CR 207.2a. Check only known explanatory forms against their actual
    // operation and amount. Unrecognized parentheticals are unconsumed source.
    clauses.iter().flatten().any(|c| {
        let (verb, tail) = match c.operation {
            Operation::Scry => ("scry", "on the bottom and the rest on top in any order."),
            Operation::Surveil => (
                "surveil",
                "into your graveyard and the rest on top of your library in any order.",
            ),
            _ => return false,
        };
        let Some(text) = text.strip_suffix(')') else {
            return false;
        };
        let text = if let Some(t) = text.strip_prefix("Look at the top ") {
            t
        } else if let Some(t) =
            text.strip_prefix(&format!("To {verb} {}, look at the top ", c.count))
        {
            t
        } else {
            return false;
        };
        if c.count == 1
            && ((c.operation == Operation::Scry
                && text == "card of your library. You may put that card on the bottom.")
                || (c.operation == Operation::Surveil
                    && matches!(
                        text,
                        "card of your library. You may put it into your graveyard."
                            | "card of your library. You may put that card into your graveyard."
                    )))
        {
            return true;
        }
        let Some((n, rest)) =
            text.split_once(" cards of your library, then put any number of them ")
        else {
            return false;
        };
        number(n) == Some(c.count) && rest == tail
    })
}

fn printed(text: &str) -> Option<Vec<Vec<Vec<Clause>>>> {
    let blocks = text
        .lines()
        .map(|line| {
            let (line, parenthetical) = line
                .split_once(" (")
                .map_or((line, None), |(line, r)| (line, Some(r)));
            let effects = line
                .strip_suffix('.')?
                .split(". ")
                .map(|sentence| {
                    sentence
                        .split(", ")
                        .map(|part| clause(part.strip_prefix("then ").unwrap_or(part)))
                        .collect::<Option<Vec<_>>>()
                })
                .collect::<Option<Vec<_>>>()?;
            if parenthetical.is_some_and(|r| !reminder(r, &effects)) {
                return None;
            }
            Some(effects)
        })
        .collect::<Option<Vec<_>>>()?;
    let mut ops = blocks
        .iter()
        .flatten()
        .flatten()
        .map(|c| (c.operation, c.count));
    let first = ops.next()?;
    let rest: Vec<_> = ops.collect();
    if rest.is_empty()
        || !(std::iter::once(&first)
            .chain(&rest)
            .any(|(op, _)| op.is_library())
            || first.0 == Operation::Discard && rest.iter().any(|(op, _)| *op == Operation::Draw))
    {
        return None;
    }
    // Existing source encodings and catalog bytes stay stable.
    if let [second] = rest.as_slice() {
        let old_look = |op| {
            matches!(
                op,
                Operation::Scry | Operation::Surveil | Operation::Mill | Operation::TargetMill
            )
        };
        if (old_look(first.0) && *second == (Operation::Draw, 1))
            || (first == (Operation::Draw, 1) && old_look(second.0))
            || (first.0 == Operation::Draw && second.0 == Operation::Land && blocks.len() == 1)
        {
            return None;
        }
    }
    Some(blocks)
}

fn selection(step: &Value, block: u32) -> Option<&str> {
    let id = step["selectionId"].as_str()?;
    let prefix = format!("line:{block}:");
    let index = id.strip_prefix(&prefix)?.parse::<u32>().ok()?;
    (id == format!("{prefix}{index}")).then_some(id)
}
