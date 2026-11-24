//! Cost table: each printed cost term lowers independently, then the complete
//! term set selects one runtime cost. A term set the runtime cannot pay as one
//! atomic cost is rejected; no term is dropped or reordered.
//!
//! CR 602.1a (cost before the colon), 118.1-118.3 (costs are paid in full),
//! 701.21a (sacrifice), 106.1/107.4 (mana symbols), 119.4 (paying life) -
//! pinned 2026-09-25.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

#[derive(Default)]
struct Terms {
    tap: bool,
    mana: Option<ManaCost>,
    sacrifice_source: bool,
    sacrifice_other: Option<SacrificeFilter>,
    life: Option<Quantity>,
}

fn duplicate() -> Failure {
    unsupported("compose.cost", "repeated cost term")
}

/// "Sacrifice a/an T": one permanent of one permanent type, of one subtype,
/// or of any type in a printed union. The runtime reads the candidate's
/// derived types (CR 701.21a, 205.2a); `types` is a union and a subtype is a
/// conjunction, so a multi-type conjunction is rejected rather than widened.
fn sacrifice_filter(what: &serde_json::Value) -> Result<SacrificeFilter, Failure> {
    let unsupported_filter = || unsupported("cost", "unsupported complete sacrifice filter");
    let object = what.as_object().ok_or_else(unsupported_filter)?;
    if object.len() != 3
        || what["ref"] != "filter"
        || what["count"] != serde_json::json!({"amount":"fixed","value":1})
    {
        return Err(unsupported_filter());
    }
    let kind = |value: &serde_json::Value| -> Result<CardKind, Failure> {
        match serde_json::from_value(value.clone()) {
            Ok(
                kind @ (CardKind::Creature
                | CardKind::Artifact
                | CardKind::Enchantment
                | CardKind::Land),
            ) => Ok(kind),
            _ => Err(unsupported_filter()),
        }
    };
    let one = |value: &serde_json::Value| -> Result<serde_json::Value, Failure> {
        match value.as_array().map(Vec::as_slice) {
            Some([only]) => Ok(only.clone()),
            _ => Err(unsupported_filter()),
        }
    };
    let filter = what["filter"].as_object().ok_or_else(unsupported_filter)?;
    let keys: Vec<&str> = filter.keys().map(String::as_str).collect();
    match keys.as_slice() {
        ["types"] => Ok(SacrificeFilter {
            types: vec![kind(&one(&filter["types"])?)?],
            subtypes: vec![],
        }),
        ["subtypes"] => {
            let subtype = one(&filter["subtypes"])?;
            let subtype = subtype.as_str().ok_or_else(unsupported_filter)?;
            if subtype.is_empty() || !subtype.chars().all(|c| c.is_ascii_alphabetic() || c == '-') {
                return Err(unsupported_filter());
            }
            Ok(SacrificeFilter {
                types: vec![],
                subtypes: vec![subtype.to_owned()],
            })
        }
        ["allOf"] => {
            // The parser spells a printed "A or B" as allOf[anyOf[...]].
            let union = one(&filter["allOf"])?;
            let alternatives = union
                .as_object()
                .filter(|o| o.len() == 1)
                .and_then(|o| o.get("anyOf"))
                .and_then(|v| v.as_array())
                .ok_or_else(unsupported_filter)?;
            let mut types = vec![];
            for alternative in alternatives {
                let only = alternative
                    .as_object()
                    .filter(|o| o.len() == 1)
                    .and_then(|o| o.get("types"))
                    .ok_or_else(unsupported_filter)?;
                let kind = kind(&one(only)?)?;
                if types.contains(&kind) {
                    return Err(unsupported_filter());
                }
                types.push(kind);
            }
            if types.len() < 2 {
                return Err(unsupported_filter());
            }
            Ok(SacrificeFilter {
                types,
                subtypes: vec![],
            })
        }
        _ => Err(unsupported_filter()),
    }
}

fn term(card: &Card<'_>, terms: &mut Terms, cost: &SourceCost) -> Result<(), Failure> {
    match cost {
        SourceCost::TapSelf {} => {
            if std::mem::replace(&mut terms.tap, true) {
                return Err(duplicate());
            }
        }
        SourceCost::Mana { symbols } => {
            // Each parser symbol is one complete mana symbol; reject injected
            // braces instead of reinterpreting them.
            if symbols.is_empty()
                || symbols
                    .iter()
                    .any(|s| s.is_empty() || s.contains(['{', '}']))
            {
                return Err(unsupported("compose.cost", "invalid mana-cost symbol"));
            }
            let mana = mana_cost(
                &symbols
                    .iter()
                    .map(|s| format!("{{{s}}}"))
                    .collect::<String>(),
            )?;
            if mana.colored.contains(&Color::C) {
                return Err(unsupported("compose.cost", "colorless-specific cost"));
            }
            if terms.mana.replace(mana).is_some() {
                return Err(duplicate());
            }
        }
        SourceCost::Sacrifice { what } => {
            let this = what.as_object().is_some_and(|o| o.len() == 2)
                && what["ref"] == "this"
                && what["noun"].as_str().is_some_and(|noun| card.is_self(noun));
            if this {
                if std::mem::replace(&mut terms.sacrifice_source, true) {
                    return Err(duplicate());
                }
            } else if terms
                .sacrifice_other
                .replace(sacrifice_filter(what)?)
                .is_some()
            {
                return Err(duplicate());
            }
        }
        // CR 119.4: a fixed positive life payment.
        SourceCost::PayLife { amount } => {
            let life = fixed_positive_amount(&typed(amount, "amount")?)?;
            if terms.life.replace(life).is_some() {
                return Err(duplicate());
            }
        }
        SourceCost::RemoveCounters { .. } => {
            return Err(unsupported(
                "compose.cost",
                "counter costs outside the block tables",
            ));
        }
    }
    Ok(())
}

/// CR 602.1a: an activated ability is printed "[cost]: [effect]", its cost a
/// comma-separated list. Each AST term must be the printed term at the same
/// position: `{T}` for tapping, the same mana symbols for mana, and a
/// "Sacrifice" phrase for a sacrifice. Reference binding does not cover tap
/// and mana terms, so this keeps a changed cost from lowering.
pub(super) fn printed(costs: &[SourceCost], line: &str) -> Result<(), Failure> {
    let mismatch = || unsupported("compose.cost", "printed cost differs from the AST");
    let (text, _) = line.split_once(": ").ok_or_else(mismatch)?;
    // An ability word and its dash precede the cost (CR 207.2c).
    let text = text
        .rsplit_once(" \u{2014} ")
        .map_or(text, |(_, rest)| rest);
    let terms: Vec<&str> = text.split(", ").collect();
    if terms.len() != costs.len() {
        return Err(mismatch());
    }
    for (term, cost) in terms.into_iter().zip(costs) {
        let same = match cost {
            SourceCost::TapSelf {} => term == "{T}",
            SourceCost::Mana { symbols } => {
                term == symbols
                    .iter()
                    .map(|s| format!("{{{s}}}"))
                    .collect::<String>()
            }
            SourceCost::Sacrifice { .. } => term.starts_with("Sacrifice "),
            SourceCost::PayLife { .. } => term.starts_with("Pay "),
            SourceCost::RemoveCounters { .. } => term.starts_with("Remove "),
        };
        if !same {
            return Err(mismatch());
        }
    }
    Ok(())
}

pub(super) fn lower(card: &Card<'_>, costs: &[SourceCost]) -> Result<ActivationCost, Failure> {
    if costs.is_empty() {
        return Err(unsupported("compose.cost", "missing activation cost"));
    }
    let mut terms = Terms::default();
    for cost in costs {
        term(card, &mut terms, cost)?;
    }
    let Terms {
        tap,
        mana,
        sacrifice_source,
        sacrifice_other,
        life,
    } = terms;
    if let Some(life) = life {
        // CR 602.2b, 118.3: "{T}, Pay N life, Sacrifice [this]" is the one
        // life-paying term set the runtime pays as an atomic ordered batch.
        return match (tap, mana, sacrifice_source, sacrifice_other) {
            (true, None, true, None) => Ok(ActivationCost::TapPayLifeSacrificeSource { life }),
            _ => Err(unsupported(
                "compose.cost",
                "life payment combines only with tapping and sacrificing the source",
            )),
        };
    }
    Ok(match (tap, mana, sacrifice_source, sacrifice_other) {
        (false, mana, false, Some(filter)) => ActivationCost::SacrificePermanent {
            count: 1,
            filter,
            mana,
        },
        (_, _, _, Some(_)) => {
            return Err(unsupported(
                "compose.cost",
                "sacrificing another permanent combines only with mana",
            ));
        }
        (true, None, false, None) => ActivationCost::TapSource,
        (false, Some(cost), false, None) => ActivationCost::Mana { cost },
        (false, None, true, None) if card.kind == CardKind::Creature => {
            ActivationCost::SacrificeSource
        }
        (tap, mana, sacrifice, None) => ActivationCost::Composite {
            costs: PermanentActivationCosts {
                remove_source_plus_one_counters: None,
                tap,
                mana,
                sacrifice,
                pay_life: None,
            },
        },
    })
}
