//! Cost table: each printed cost term lowers independently, then the complete
//! term set selects one runtime cost. A term set the runtime cannot pay as one
//! atomic cost is rejected; no term is dropped or reordered.
//!
//! CR 602.1a (cost before the colon), 118.1-118.3 (costs are paid in full),
//! 701.21a (sacrifice), 106.1/107.4 (mana symbols), 119.4 (paying life) -
//! pinned 2026-09-25.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

pub(crate) mod chosen;

#[derive(Default)]
struct Terms {
    tap: bool,
    mana: Option<ManaCost>,
    hybrid: Vec<ColorHybridSymbol>,
    x: bool,
    sacrifice_source: bool,
    sacrifice_other: Option<SacrificeOther>,
    chosen: Option<ChosenObjectCost>,
    life: Option<Quantity>,
    counters: Option<RemoveCounterCost>,
}

fn duplicate() -> Failure {
    unsupported("compose.cost", "repeated cost term")
}

/// "Sacrifice a/an T": one permanent of one permanent type, of one subtype,
/// or of any type in a printed union. The runtime reads the candidate's
/// derived types (CR 701.21a, 205.2a); `types` is a union and a subtype is a
/// conjunction, so a multi-type conjunction is rejected rather than widened.
struct SacrificeOther {
    count: u32,
    filter: SacrificeFilter,
    another: bool,
}

fn sacrifice_filter(what: &serde_json::Value) -> Result<SacrificeOther, Failure> {
    let unsupported_filter = || unsupported("cost", "unsupported complete sacrifice filter");
    let object = what.as_object().ok_or_else(unsupported_filter)?;
    // CR 701.21a: "Sacrifice N [filter]" chooses N distinct permanents.
    let count = match what["count"].as_object() {
        Some(count) if count.len() == 2 && count.get("amount") == Some(&"fixed".into()) => count
            .get("value")
            .and_then(serde_json::Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| *n > 0)
            .ok_or_else(unsupported_filter)?,
        _ => return Err(unsupported_filter()),
    };
    if object.len() != 3 || what["ref"] != "filter" {
        return Err(unsupported_filter());
    }
    // CR 109.1: "another" is any permanent other than the ability's source.
    let mut quality = what["filter"].clone();
    let another = quality
        .as_object_mut()
        .and_then(|o| o.remove("other"))
        .map(|other| {
            if other == true {
                Ok(true)
            } else {
                Err(unsupported_filter())
            }
        })
        .transpose()?
        .unwrap_or(false);
    sacrifice_quality(&quality).map(|filter| SacrificeOther {
        count,
        filter,
        another,
    })
}

/// The quality a sacrificed permanent must have.
pub(super) fn sacrifice_quality(filter: &serde_json::Value) -> Result<SacrificeFilter, Failure> {
    let unsupported_filter = || unsupported("cost", "unsupported complete sacrifice filter");
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
    let filter = filter.as_object().ok_or_else(unsupported_filter)?;
    if filter.is_empty() {
        return Err(unsupported_filter());
    }
    let mut result = SacrificeFilter::default();
    let token = |result: &mut SacrificeFilter, value: bool| match result.token.replace(value) {
        Some(_) => Err(unsupported_filter()),
        None => Ok(()),
    };
    let mut types = None;
    for (key, value) in filter {
        match key.as_str() {
            // CR 110.1: "a permanent" is any permanent its controller controls.
            "cls" if value == "permanent" => {}
            // CR 111.1: "a token" is a token permanent.
            "cls" if value == "token" => token(&mut result, true)?,
            "nonToken" if value == true => token(&mut result, false)?,
            "types" => {
                if types.replace(vec![kind(&one(value)?)?]).is_some() {
                    return Err(unsupported_filter());
                }
            }
            "allOf" => {
                // The parser spells a printed "A or B" as allOf[anyOf[...]].
                let union = one(value)?;
                let alternatives = union
                    .as_object()
                    .filter(|o| o.len() == 1)
                    .and_then(|o| o.get("anyOf"))
                    .and_then(|v| v.as_array())
                    .ok_or_else(unsupported_filter)?;
                let mut kinds = vec![];
                for alternative in alternatives {
                    let only = alternative
                        .as_object()
                        .filter(|o| o.len() == 1)
                        .and_then(|o| o.get("types"))
                        .ok_or_else(unsupported_filter)?;
                    let kind = kind(&one(only)?)?;
                    if kinds.contains(&kind) {
                        return Err(unsupported_filter());
                    }
                    kinds.push(kind);
                }
                if kinds.len() < 2 || types.replace(kinds).is_some() {
                    return Err(unsupported_filter());
                }
            }
            "subtypes" => {
                let subtype = one(value)?;
                let subtype = subtype.as_str().ok_or_else(unsupported_filter)?;
                if subtype.is_empty()
                    || !subtype.chars().all(|c| c.is_ascii_alphabetic() || c == '-')
                {
                    return Err(unsupported_filter());
                }
                result.subtypes = vec![subtype.to_owned()];
            }
            // CR 105.2: "a black creature" has that color.
            "colors" => {
                let colors: Vec<crate::ast::Color> =
                    serde_json::from_value(value.clone()).map_err(|_| unsupported_filter())?;
                for color in &colors {
                    let color = crate::operands::color(color);
                    if result.colors.contains(&color) {
                        return Err(unsupported_filter());
                    }
                    result.colors.push(color);
                }
                // No printed sacrifice cost conjoins colors.
                if result.colors.len() != 1 {
                    return Err(unsupported_filter());
                }
            }
            _ => return Err(unsupported_filter()),
        }
    }
    result.types = types.unwrap_or_default();
    // A subtype is a conjunction with at most the one type it narrows.
    if !result.subtypes.is_empty() && result.types.len() > 1 {
        return Err(unsupported_filter());
    }
    Ok(result)
}

fn term(card: &Card<'_>, terms: &mut Terms, cost: &SourceCost) -> Result<(), Failure> {
    match cost {
        SourceCost::Discard { .. } | SourceCost::TapObjects { .. } | SourceCost::Exile { .. } => {
            if terms.chosen.replace(chosen::lower(cost)?).is_some() {
                return Err(duplicate());
            }
        }
        SourceCost::TapSelf {} => {
            if std::mem::replace(&mut terms.tap, true) {
                return Err(duplicate());
            }
        }
        SourceCost::Mana { symbols } => {
            // Each parser symbol is one complete mana symbol; reject injected
            // braces instead of reinterpreting them.
            let mana = activation_mana(symbols, "compose.cost")?;
            if mana.base.colored.contains(&Color::C) {
                return Err(unsupported("compose.cost", "colorless-specific cost"));
            }
            if terms.mana.replace(mana.base).is_some() {
                return Err(duplicate());
            }
            terms.hybrid = mana.hybrid;
            terms.x = mana.x;
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
            let counters =
                crate::rules::lower_remove_counter_cost(cost, &|noun| card.is_self(noun))?;
            if terms.counters.replace(counters).is_some() {
                return Err(duplicate());
            }
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
            SourceCost::Exile { what, from } if what["ref"] == "this" => {
                *from == Some(serde_json::json!({"zone":"graveyard","owner":"your"}))
                    && term == "Exile this card from your graveyard"
            }
            SourceCost::Discard { .. }
            | SourceCost::TapObjects { .. }
            | SourceCost::Exile { .. } => chosen::matches_printed(&chosen::lower(cost)?, term),
        };
        if !same {
            return Err(mismatch());
        }
    }
    Ok(())
}

fn exile_gap() -> Failure {
    unsupported(
        "compose.cost",
        "activation exile cost outside the block tables",
    )
}

/// CR 113.6j, 602.2a-b, 701.13a: the cost defines which zone this
/// activation comes from. Only one fixed mana term and the exact source
/// card in its owner's graveyard are admitted; a hand exile cost rejects.
pub(super) fn exile_source(
    card: &Card<'_>,
    costs: &[SourceCost],
) -> Result<Option<(ActivationCost, FunctioningZone)>, Failure> {
    use serde_json::json;
    if !costs
        .iter()
        .any(|term| matches!(term, SourceCost::Exile { what, .. } if what["ref"] == "this"))
    {
        return Ok(None);
    }
    let [
        mana @ SourceCost::Mana { .. },
        SourceCost::Exile {
            what,
            from: Some(from),
        },
    ] = costs
    else {
        return Err(exile_gap());
    };
    if *what != json!({"ref":"this","noun":"card"}) {
        return Err(exile_gap());
    }
    if *from != json!({"zone":"graveyard","owner":"your"}) {
        return Err(exile_gap());
    }
    let ActivationCost::Mana { cost } = lower(card, std::slice::from_ref(mana))? else {
        return Err(exile_gap());
    };
    Ok(Some((
        ActivationCost::ExileSource { mana: cost },
        FunctioningZone::Graveyard,
    )))
}

/// A cost whose mana term announces nothing: a keyword or graveyard
/// activation has no announcement route for X or hybrid symbols.
pub(super) fn lower(card: &Card<'_>, costs: &[SourceCost]) -> Result<ActivationCost, Failure> {
    match announced(card, costs)? {
        (cost, hybrid, false) if hybrid.is_empty() => Ok(cost),
        _ => Err(unsupported(
            "source.mana_cost",
            "variable/hybrid/Phyrexian/special mana is not supported",
        )),
    }
}

/// CR 107.3a, 107.4e-f, 602.2b: the cost with its fixed mana, and the hybrid
/// symbols and leading X its activation announces.
pub(super) fn announced(
    card: &Card<'_>,
    costs: &[SourceCost],
) -> Result<(ActivationCost, Vec<ColorHybridSymbol>, bool), Failure> {
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
        hybrid,
        x,
        sacrifice_source,
        sacrifice_other,
        chosen,
        life,
        counters,
    } = terms;
    if let Some(mut cost) = chosen {
        if sacrifice_source || sacrifice_other.is_some() || life.is_some() || counters.is_some() {
            return Err(unsupported(
                "compose.cost",
                "unsupported chosen-object cost combination",
            ));
        }
        cost.tap_source = tap;
        cost.mana = mana;
        return Ok((ActivationCost::ChooseObjects(cost), hybrid, x));
    }
    if life.is_some() || counters.is_some() {
        if sacrifice_other.is_some() {
            return Err(unsupported(
                "compose.cost",
                "life or counter payment with another sacrificed permanent",
            ));
        }
        // CR 602.2b, 118.3: "{T}, Pay N life, Sacrifice [this]" keeps its
        // atomic ordered batch; every other source-bound term set is paid
        // by the composite cost after its mana (CR 601.2h, 119.4, 122.1).
        let cost = match (tap, mana, sacrifice_source, life, counters) {
            (true, None, true, Some(life), None) => {
                ActivationCost::TapPayLifeSacrificeSource { life }
            }
            (tap, mana, sacrifice, pay_life, counters) => ActivationCost::Composite {
                costs: PermanentActivationCosts {
                    remove_source_counters: counters,
                    tap,
                    mana,
                    sacrifice,
                    pay_life,
                },
            },
        };
        return Ok((cost, hybrid, x));
    }
    let cost = match (tap, mana, sacrifice_source, sacrifice_other) {
        // CR 602.2b: tapping the source and mana are paid with the chosen
        // sacrifice as one complete cost.
        (
            tap,
            mana,
            false,
            Some(SacrificeOther {
                count,
                filter,
                another,
            }),
        ) => ActivationCost::SacrificePermanent {
            count,
            filter,
            mana,
            tap,
            another,
        },
        (_, _, _, Some(_)) => {
            return Err(unsupported(
                "compose.cost",
                "sacrificing another permanent and the source",
            ));
        }
        (true, None, false, None) => ActivationCost::TapSource,
        (false, Some(cost), false, None) => ActivationCost::Mana { cost },
        (false, None, true, None) if card.kind == CardKind::Creature => {
            ActivationCost::SacrificeSource
        }
        (tap, mana, sacrifice, None) => ActivationCost::Composite {
            costs: PermanentActivationCosts {
                remove_source_counters: None,
                tap,
                mana,
                sacrifice,
                pay_life: None,
            },
        },
    };
    Ok((cost, hybrid, x))
}

/// CR 601.2b, 601.2f-h, 701.21a: "As an additional cost to cast this spell,
/// sacrifice a/an [quality]." The caster chooses the permanent after mana
/// and sacrifices it as part of the total cost; every other additional cost
/// has no runtime row and rejects.
pub(super) fn spell_additional(
    costs: &[serde_json::Value],
    line: &str,
    spell: &mut SpellProgram,
) -> Result<(), Failure> {
    let reject =
        |detail: &str| unsupported("additional-cost", format!("kind:additional-cost:{detail}"));
    let [cost] = costs else {
        return Err(reject("terms"));
    };
    let what = match serde_json::from_value::<SourceCost>(cost.clone()) {
        Ok(SourceCost::Sacrifice { what }) => what,
        _ => return Err(reject(cost["cost"].as_str().unwrap_or("cost"))),
    };
    if !line.starts_with("As an additional cost to cast this spell, sacrifice ")
        || spell.additional_sacrifice.is_some()
    {
        return Err(reject("printed"));
    }
    let SacrificeOther {
        count,
        filter,
        another,
    } = sacrifice_filter(&what).map_err(|_| reject("sacrifice-filter"))?;
    if count != 1 || another {
        return Err(reject("sacrifice_count"));
    }
    spell.additional_sacrifice = Some(filter);
    Ok(())
}
