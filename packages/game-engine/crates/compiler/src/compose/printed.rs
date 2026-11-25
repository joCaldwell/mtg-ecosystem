//! The printed clauses of a block against its AST, sentence by sentence.
//!
//! The resolution's span certificate assigns each printed byte range to the
//! AST fields that produced it. A choice, limit or condition is checked in
//! each certified segment against the AST of that segment's own fields, and
//! once more over the whole block, so a clause cannot be dropped, inverted or
//! moved to another sentence without the block rejecting:
//!
//! * "may" (CR 608.2d): an `optional` step or `optional-group`; a play or
//!   cast permission or additional land play (CR 305.2, 601.3); "you may
//!   choose the same mode more than once" and "you may choose both instead"
//!   of a modal step (CR 700.2d); "any player may activate" (CR 602.2).
//! * "if": a sentence condition of kind `if`, an intervening-if (CR 603.4)
//!   or restriction (CR 602.5b) condition, a conditional mode count, the
//!   conditional replacement of a countered spell (CR 701.6), a created
//!   replacement (CR 614.1) and an "if able" requirement (CR 508.1d,
//!   509.1c). In a static block a kindless condition is printed "if", "as
//!   long as" or "during" (CR 611.3a), so those phrases count together.
//! * "unless": a condition of kind `unless`; "if" and "unless" never stand
//!   in for each other.
//! * "up to" (CR 601.2c, 700.2): an `upTo` operand or a chosen count whose
//!   minimum is zero.
//! * CR 118.12: "If [a player] does" is `you-do`, "If [a player] doesn't" is
//!   `you-dont`.
//!
//! Reminder text and quoted abilities are not the block's own words. Card
//! names and literal operands are the parser's certified AST, not re-read
//! here.
//!
//! Pinned Comprehensive Rules 2026-09-25:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::Value;

fn disagrees() -> Failure {
    unsupported(
        "compose.effects",
        "printed choice, limit or condition disagrees with the AST",
    )
}

/// The block's own words, lowercased: no reminder text, no quoted ability.
fn rules_text(line: &str) -> String {
    let mut out = String::new();
    let (mut reminder, mut quoted) = (0usize, false);
    for c in line.chars() {
        match c {
            '(' if !quoted => reminder += 1,
            ')' if !quoted => reminder = reminder.saturating_sub(1),
            '"' if reminder == 0 => quoted = !quoted,
            '\u{2019}' if reminder == 0 && !quoted => out.push('\''),
            _ if reminder == 0 && !quoted => out.extend(c.to_lowercase()),
            _ => {}
        }
    }
    out
}

#[derive(Default, PartialEq, Debug)]
struct Counts {
    may: usize,
    condition: usize,
    unless: usize,
    limit: usize,
    duration: usize,
}

/// The printed words of `text` (already [`rules_text`]).
fn printed(text: &str, statics: bool) -> Counts {
    // Phrases inside one clause: "rounded up, to you" is not "up to".
    let phrases = |phrase: &[&str]| -> usize {
        text.split(|c: char| !(c.is_alphabetic() || c == ' '))
            .map(|clause| {
                let clause: Vec<&str> = clause.split_whitespace().collect();
                clause
                    .windows(phrase.len())
                    .filter(|w| *w == phrase)
                    .count()
            })
            .sum()
    };
    let words = |word: &str| phrases(&[word]);
    let static_conditions = if statics {
        phrases(&["as", "long", "as"]) + words("during")
    } else {
        0
    };
    Counts {
        may: words("may"),
        condition: words("if") + static_conditions,
        unless: words("unless"),
        limit: phrases(&["up", "to"]),
        duration: if statics {
            0
        } else {
            phrases(&["as", "long", "as"])
        },
    }
}

/// The carrier one AST member is: `key` holding `child` in an object whose
/// `conditionKind` is `kind`. Then every carrier inside `child`.
fn member(key: &str, child: &Value, kind: Option<&str>, found: &mut Counts) {
    match (key, child) {
        ("optional", Value::Bool(true)) => found.may += 1,
        ("effect" | "static", Value::String(tag)) => match tag.as_str() {
            "optional-group"
            | "grant-play-permission"
            | "additional-land-plays"
            | "entry-choice" => {
                found.may += 1;
            }
            "create-replacement" | "must-attack" | "must-block" => found.condition += 1,
            _ => {}
        },
        ("allowRepeat" | "permission", _) => found.may += 1,
        ("conditionalCount", _) => {
            found.may += 1;
            found.condition += 1;
        }
        ("conditionKind", Value::String(k)) => match k.as_str() {
            "if" => found.condition += 1,
            "unless" => found.unless += 1,
            _ => {}
        },
        ("condition", Value::Object(_)) if kind == Some("duration:as-long-as") => {}
        ("condition", Value::Object(_)) if kind.is_none() => found.condition += 1,
        ("duration", Value::String(duration)) if duration == "as-long-as" => found.duration += 1,
        ("declined", Value::Object(fields)) if fields.get("tapped") == Some(&Value::Bool(true)) => {
            found.condition += 1;
        }
        ("countered", _) => found.condition += 1,
        ("rule", Value::String(rule)) if rule.ends_with("-requirement") => found.condition += 1,
        ("upTo", Value::Bool(true)) => found.limit += 1,
        ("minimum" | "min", Value::Number(n)) if n.as_u64() == Some(0) => found.limit += 1,
        _ => {}
    }
    carriers(child, found);
}

/// The AST carriers of every printed word under `value`.
fn carriers(value: &Value, found: &mut Counts) {
    match value {
        Value::Object(fields) => {
            let kind = condition_kind(value);
            for (key, child) in fields {
                member(key, child, kind, found);
            }
        }
        Value::Array(items) => items.iter().for_each(|item| carriers(item, found)),
        _ => {}
    }
}

fn condition_kind(value: &Value) -> Option<&str> {
    if value["duration"] == "as-long-as" {
        Some("duration:as-long-as")
    } else {
        value["conditionKind"].as_str()
    }
}

/// The carriers of the block field at JSON pointer `field`, the field's own
/// member included (a certified `/condition` is itself an "if").
fn field_carriers(ability: &Value, field: &str, found: &mut Counts) -> Result<(), Failure> {
    let missing = || invalid("resolution.spans", "segment names a missing field");
    let node = ability.pointer(field).ok_or_else(missing)?;
    let (parent, key) = field.rsplit_once('/').ok_or_else(missing)?;
    let key = key.replace("~1", "/").replace("~0", "~");
    let kind = ability.pointer(parent).and_then(condition_kind);
    member(&key, node, kind, found);
    Ok(())
}

/// CR 118.12: an "if you do / don't" sentence prints the outcome its AST
/// condition reads: "If [a player] do/does," or "... don't/doesn't/do not/
/// does not,".
fn outcome_printed(text: &str, sentence: &Value) -> Result<(), Failure> {
    let wanted = match sentence["condition"]["condition"].as_str() {
        Some("you-do") => false,
        Some("you-dont") => true,
        _ => return Ok(()),
    };
    let clause = text
        .trim_start()
        .strip_prefix("if ")
        .and_then(|rest| rest.split_once(','))
        .map(|(clause, _)| clause.trim_end());
    let negative = |c: &str| {
        ["don't", "doesn't", "do not", "does not"]
            .iter()
            .any(|n| c.ends_with(n))
    };
    let positive = |c: &str| c.ends_with(" do") || c.ends_with(" does");
    match clause {
        Some(c) if wanted && negative(c) => Ok(()),
        Some(c) if !wanted && positive(c) => Ok(()),
        _ => Err(disagrees()),
    }
}

/// CR 614.1c: the tapped entrant is the outcome of declining the entry choice.
fn entry_decline_printed(text: &str, ability: &Value) -> Result<(), Failure> {
    if ability["effect"]["static"] != "entry-choice" {
        return Ok(());
    }
    let mut sentences = text.split_terminator('.').map(str::trim);
    let _ = sentences.next();
    if matches!(
        sentences.next(),
        Some("if you don't, it enters tapped" | "if you don't, this land enters tapped")
    ) && sentences.next().is_none()
    {
        Ok(())
    } else {
        Err(disagrees())
    }
}

/// Every printed choice, limit and condition is carried by the AST exactly
/// once, with its polarity, in the certified segment that prints it, and the
/// AST carries no unprinted one. `segments` are the block's certified spans
/// (fields, byte range) over `line`.
pub(super) fn choices_printed(
    line: &str,
    ability: &Value,
    segments: &[(Vec<String>, usize, usize)],
) -> Result<(), Failure> {
    let statics = ability["kind"] == "static";
    let mut found = Counts::default();
    carriers(ability, &mut found);
    let text = rules_text(line);
    entry_decline_printed(&text, ability)?;
    if printed(&text, statics) != found {
        return Err(disagrees());
    }
    for (index, (fields, ..)) in segments.iter().enumerate() {
        // A segment naming the whole block is the whole-block check above.
        if fields.iter().any(String::is_empty)
            || segments[..index].iter().any(|(earlier, ..)| earlier == fields)
        {
            continue;
        }
        // Segments certifying the same fields are one clause printed in
        // discontinuous pieces ("{T}: ... Activate only if ..."): their
        // words are counted together, never once per piece.
        let mut text = String::new();
        for (_, start, end) in segments.iter().filter(|(other, ..)| other == fields) {
            let piece = line
                .get(*start..*end)
                .ok_or_else(|| invalid("resolution.spans", "segment outside the block"))?;
            text.push_str(&rules_text(piece));
            text.push('|');
        }
        let mut found = Counts::default();
        for field in fields {
            field_carriers(ability, field, &mut found)?;
            if let Some(node) = ability.pointer(field) {
                outcome_printed(&text, node)?;
            }
        }
        if printed(&text, statics) != found {
            return Err(disagrees());
        }
    }
    Ok(())
}

/// CR 603.4: "When/Whenever/At [trigger event], if [condition], [effect]" is
/// an intervening-if clause, checked as the ability triggers and again on
/// resolution. It must be the block's own trigger condition; a sentence
/// condition would only be read on resolution (CR 608.2a). The clause is an
/// "if" printed immediately after the certified trigger header span,
/// however many commas that event contains. The header names `/trigger` and
/// may also certify the intervening condition, a trigger limit or other
/// header fields; a header that certifies `/condition` printed the clause
/// as the trigger's own condition.
pub(super) fn intervening_if(
    line: &str,
    ability: &Value,
    segments: &[(Vec<String>, usize, usize)],
) -> Result<(), Failure> {
    let (fields, _, end) = segments
        .iter()
        .find(|(fields, ..)| fields.iter().any(|f| f == "/trigger"))
        .ok_or_else(|| unsupported("resolution.spans", "no certified trigger event span"))?;
    if fields.iter().any(|f| f == "/condition") {
        return if ability.get("condition").is_some() {
            Ok(())
        } else {
            Err(invalid("resolution.spans", "segment names a missing field"))
        };
    }
    let after = line
        .get(*end..)
        .ok_or_else(|| invalid("resolution.spans", "segment outside the block"))?;
    let after = after.trim_start().trim_start_matches(',').trim_start();
    if after.to_lowercase().starts_with("if ") && ability.get("condition").is_none() {
        return Err(unsupported(
            "abilities.condition",
            "printed intervening if outside the trigger condition",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn span(line: &str, field: &str, text: &str) -> (Vec<String>, usize, usize) {
        let start = line.find(text).unwrap();
        (vec![field.to_owned()], start, start + text.len())
    }

    #[test]
    fn entry_choice_requires_its_printed_decline_clause() {
        let ability = json!({
            "kind": "static",
            "effect": {"static": "entry-choice", "declined": {"tapped": true}}
        });
        let prefix = "As this land enters, you may pay 2 life.";
        for (end, valid) in [
            (" If you don't, it enters tapped.", true),
            (" If you do, it enters tapped.", false),
            (" If you don't, it enters untapped.", false),
            ("", false),
        ] {
            let line = format!("{prefix}{end}");
            let spans = [(vec![String::new()], 0, line.len())];
            assert_eq!(choices_printed(&line, &ability, &spans).is_ok(), valid);
        }
    }

    #[test]
    fn an_intervening_if_follows_the_certified_event_span_past_its_commas() {
        let line = "Whenever a player casts a white, blue, or black spell, if you control an artifact, draw a card.";
        let segments = [
            span(
                line,
                "/trigger",
                "Whenever a player casts a white, blue, or black spell,",
            ),
            span(
                line,
                "/effects/0",
                "if you control an artifact, draw a card.",
            ),
        ];
        let sentence = json!({"kind": "triggered", "trigger": {"trigger": "cast"},
            "effects": [{"steps": [], "condition": {"condition": "control"},
                         "conditionKind": "if"}]});
        let failure = intervening_if(line, &sentence, &segments).unwrap_err();
        assert_eq!(failure.path, "abilities.condition");
        let mut trigger = sentence.clone();
        trigger["condition"] = json!({"condition": "control"});
        let segments = [
            span(
                line,
                "/trigger",
                "Whenever a player casts a white, blue, or black spell,",
            ),
            span(line, "/condition", "if you control an artifact,"),
            span(line, "/effects/0", "draw a card."),
        ];
        assert!(intervening_if(line, &trigger, &segments).is_ok());
        // A condition after the effect is a resolution condition (CR 608.2c).
        let line = "At the beginning of your upkeep, you lose 2 life if you don't control an Ogre.";
        let segments = [
            span(line, "/trigger", "At the beginning of your upkeep,"),
            span(
                line,
                "/effects/0",
                "you lose 2 life if you don't control an Ogre.",
            ),
        ];
        assert!(intervening_if(line, &sentence, &segments).is_ok());
    }

    #[test]
    fn a_keyword_block_is_checked_like_any_other() {
        let line = "Flying if you control an artifact";
        let ability = json!({"kind": "keywords", "keywords": [{"keyword": "flying"}]});
        let segments = [(vec!["/keywords".to_owned()], 0, line.len())];
        assert!(choices_printed(line, &ability, &segments).is_err());
        assert!(
            choices_printed("Flying", &ability, &[(vec!["/keywords".to_owned()], 0, 6)]).is_ok()
        );
    }

    #[test]
    fn a_joint_trigger_header_counts_and_certifies_its_condition() {
        let line = "When this creature enters, if you control an artifact, draw a card.";
        let header = "When this creature enters, if you control an artifact,";
        let segments = [
            (
                vec!["/trigger".to_owned(), "/condition".to_owned()],
                0,
                header.len(),
            ),
            span(line, "/effects/0", "draw a card."),
        ];
        let ability = json!({"kind": "triggered", "trigger": {"trigger": "enters"},
            "condition": {"condition": "control"}, "effects": [{"steps": []}]});
        assert!(intervening_if(line, &ability, &segments).is_ok());
        assert!(choices_printed(line, &ability, &segments).is_ok());
        let mut missing = ability.clone();
        missing.as_object_mut().unwrap().remove("condition");
        assert!(intervening_if(line, &missing, &segments).is_err());
    }

    #[test]
    fn a_paid_outcome_prints_its_polarity() {
        let you_do = json!({"condition": {"condition": "you-do"}, "conditionKind": "if"});
        let you_dont = json!({"condition": {"condition": "you-dont"}, "conditionKind": "if"});
        for (text, sentence, ok) in [
            ("if you do, you gain 1 life.", &you_do, true),
            ("if the player does, draw a card.", &you_do, true),
            ("if you do, you gain 1 life.", &you_dont, false),
            (
                "if they don't, you create a treasure token.",
                &you_dont,
                true,
            ),
            ("if that player doesn't, draw.", &you_do, false),
            ("if the player does not, draw.", &you_dont, true),
        ] {
            assert_eq!(outcome_printed(text, sentence).is_ok(), ok, "{text}");
        }
    }

    #[test]
    fn segments_scope_each_clause_to_its_own_sentence() {
        let line =
            "Return target creature to its owner's hand. If you control an artifact, draw a card.";
        let first = "Return target creature to its owner's hand.";
        let second = "If you control an artifact, draw a card.";
        let segments = [
            span(line, "/effects/0", first),
            span(line, "/effects/1", second),
        ];
        let condition = json!({"condition": "control"});
        let ability = |at: usize| {
            let mut effects = vec![json!({"steps": []}), json!({"steps": []})];
            effects[at]["condition"] = condition.clone();
            effects[at]["conditionKind"] = json!("if");
            json!({"kind": "spell", "effects": effects})
        };
        assert!(choices_printed(line, &ability(1), &segments).is_ok());
        // The same block totals, but the condition moved to the sentence
        // that does not print it.
        assert!(choices_printed(line, &ability(0), &segments).is_err());
    }
}
