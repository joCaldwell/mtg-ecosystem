//! CR 401.2, 401.4, 608.2c–d, 701.20a/e: inspect the controller's
//! fixed library prefix privately, take a bounded subset into hand, and
//! bottom the complement in a chosen or random order. A printed reveal
//! publishes only the selected cards. The selection and the remainder
//! are bound by their local producers, before any card changes zone.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use kir::{Amount, LookSelection, PlayerExpr, Verb};
use serde_json::{Value, json};

fn reject(detail: &str) -> Failure {
    unsupported("effects.steps", format!("verb:look-at:{detail}"))
}

pub(crate) fn candidate(effects: &[Value]) -> bool {
    matches!(effects, [first, second, ..]
        if first["steps"][0]["effect"] == "look-at"
            && second["steps"][0]["effect"] == "optional-group")
}

fn steps(sentence: &Value) -> Result<&[Value], Failure> {
    let Some(steps) = sentence.get("steps").and_then(Value::as_array) else {
        return Err(reject("sentence"));
    };
    if sentence.as_object().is_none_or(|fields| fields.len() != 1) {
        return Err(reject("qualified_sentence"));
    }
    Ok(steps)
}

fn count(value: &Value) -> Result<u32, Failure> {
    let Some(n) = value["value"].as_u64().and_then(|n| u32::try_from(n).ok()) else {
        return Err(reject("fixed_positive_count"));
    };
    if n == 0 || *value != json!({"amount":"fixed","value":n}) {
        return Err(reject("fixed_positive_count"));
    }
    Ok(n)
}

fn is_hand(action: &Value, what: &Value) -> bool {
    *action
        == json!({
            "effect":"move-zone",
            "what":what,
            "to":{"zone":"hand","owner":"your"},
        })
}

fn is_bottom(action: &Value, order: Option<&str>) -> bool {
    let mut expected = json!({
        "effect":"move-zone",
        "what":{"ref":"rest"},
        "to":{"zone":"library","owner":"your","position":"bottom"},
    });
    if let Some(order) = order {
        expected["order"] = json!(order);
    }
    *action == expected
}

fn filtered_reveal(reveal: &Value, movement: &Value) -> Result<LookSelection, Failure> {
    let what = &reveal["what"];
    if *reveal != json!({"effect":"reveal","who":{"player":"you"},"what":what}) {
        return Err(reject("reveal"));
    }
    let maximum = count(&what["count"])?;
    let filter = &what["filter"];
    let mut expected = json!({"ref":"filter","filter":filter,"count":what["count"]});
    if what["upTo"] == true {
        expected["upTo"] = json!(true);
    } else if maximum != 1 {
        // “May reveal N” can decline or pick N, not every quantity in between.
        return Err(reject("optional_exact_multiple"));
    }
    if *what != expected {
        return Err(reject("reveal_pool"));
    }
    let mut filter: ast::ObjectFilter = typed(filter, "looked card filter")?;
    if !matches!(
        filter.from_among.take().as_deref(),
        Some(ast::ObjectRef::Them(_))
    ) {
        return Err(reject("reveal_pool"));
    }
    let permanent = filter.permanent.take().is_some();
    let mut filter = lower_card_filter(&filter)?;
    if permanent {
        filter.0.push(kir::Atom::AnyType {
            kinds: CardKind::CARD_TYPES
                .into_iter()
                .filter(|kind| !matches!(kind, CardKind::Instant | CardKind::Sorcery))
                .collect(),
        });
    }
    let referent = &movement["what"];
    if !is_hand(movement, referent)
        || !(maximum == 1
            && matches!(referent["ref"].as_str(), Some("it" | "that"))
            && (*referent == json!({"ref":"it"})
                || *referent == json!({"ref":"that","filter":{"cls":"card"}}))
            || maximum > 1 && *referent == json!({"ref":"them"}))
    {
        return Err(reject("selected_object"));
    }
    Ok(LookSelection {
        filter,
        maximum,
        optional: true,
        reveal: true,
        random_bottom: false,
    })
}

/// CR 401.2, 701.20e: both printed partitions inspect the controller's
/// own fixed prefix.
pub(crate) fn prefix(who: &Value, what: &Value) -> Result<u32, Failure> {
    let top = &what["top"];
    if *who != json!({"player":"you"})
        || *what != json!({"top":top,"of":{"zone":"library","owner":"your"}})
    {
        return Err(reject("look_prefix"));
    }
    count(top)
}

fn complement(
    amount: u32,
    mut selection: LookSelection,
    remainder: &Value,
) -> Result<LookSelection, Failure> {
    if is_bottom(remainder, Some("random")) {
        selection.random_bottom = true;
    } else if !(is_bottom(remainder, Some("any"))
        || (!selection.optional
            && amount.saturating_sub(selection.maximum) <= 1
            && is_bottom(remainder, None)))
    {
        return Err(reject("remainder"));
    }
    Ok(selection)
}

/// "Put N of them into your hand and the rest on the bottom." The pool and
/// its complement are that one preceding look, proved at consecutive sites.
pub(crate) fn mandatory(amount: u32, members: &[Value]) -> Result<LookSelection, Failure> {
    let [move_to_hand, rest] = members else {
        return Err(reject("selection_group"));
    };
    let what = &move_to_hand["what"];
    let maximum = count(&what["count"])?;
    if !is_hand(move_to_hand, what)
        || *what
            != json!({
                "ref":"from-pool","pool":{"ref":"them"},"count":what["count"],
            })
    {
        return Err(reject("selection_pool"));
    }
    complement(
        amount,
        LookSelection {
            maximum,
            ..LookSelection::default()
        },
        rest,
    )
}

/// Claim the whole optional reveal block only after its exact producer,
/// chooser, consumers and destinations have been proved.
pub(crate) fn certify(effects: &[Value]) -> Result<(u32, LookSelection), Failure> {
    let [first, second, rest] = effects else {
        return Err(reject("selection_sequence"));
    };
    let ([look], [group], [rest]) = (steps(first)?, steps(second)?, steps(rest)?) else {
        return Err(reject("selection_sequence"));
    };
    let (who, what) = (&look["who"], &look["what"]);
    if *look != json!({"effect":"look-at","who":who,"what":what}) {
        return Err(reject("look_prefix"));
    }
    let amount = prefix(who, what)?;
    let members = group["steps"]
        .as_array()
        .ok_or_else(|| reject("selection_group"))?;
    let [reveal, movement] = members.as_slice() else {
        return Err(reject("selection_group"));
    };
    if *group != json!({"effect":"optional-group","who":{"player":"you"},"steps":members}) {
        return Err(reject("selection_group"));
    }
    Ok((
        amount,
        complement(amount, filtered_reveal(reveal, movement)?, rest)?,
    ))
}

pub(crate) fn verb(context: Context, amount: u32, selection: LookSelection) -> Verb {
    Verb::LookChooseHand {
        player: PlayerExpr::you(context),
        amount: Amount::fixed(i64::from(amount)),
        selection,
    }
}

pub(super) fn lower(
    effects: &[Value],
    program: &mut SpellProgram,
    context: Context,
    block: u32,
) -> Result<(), Failure> {
    let (amount, selection) = certify(effects)?;
    // The exact group above has a single local selection producer. Report
    // its pronoun binding for the frozen reference comparison.
    if selection.reveal {
        let word = match effects[1]
            .pointer("/steps/0/steps/1/what/ref")
            .and_then(Value::as_str)
        {
            Some("it") => Some("it"),
            Some("that") => Some("that"),
            _ => None,
        };
        if let Some(word) = word {
            crate::reference_trace::record(|| crate::reference_trace::Bound {
                block,
                word,
                resolved: crate::reference_trace::Resolved::StepResult {
                    step: "/effects/1/steps/0/steps/0/what".into(),
                    role: "selection",
                },
            });
        }
    }
    program
        .instructions
        .push(Instruction::act(verb(context, amount, selection), block));
    Ok(())
}
