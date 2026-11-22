//! Activated and mana abilities and their costs (CR 602, 605).
use super::*;

pub(crate) fn lower_permanent_costs(
    costs: &[SourceCost],
    kind: CardKind,
) -> Result<PermanentActivationCosts, Failure> {
    let mut result = PermanentActivationCosts {
        remove_source_plus_one_counters: None,
        tap: false,
        mana: None,
        sacrifice: false,
    };
    for cost in costs {
        match cost {
            SourceCost::RemoveCounters {
                counter,
                count,
                from,
            } if result.remove_source_plus_one_counters.is_none()
                && counter.r#type == "+1/+1"
                && count.amount == "fixed"
                && count.value > 0
                && kind == CardKind::Creature
                && *from == serde_json::json!({"ref":"this","noun":"creature"}) =>
            {
                result.remove_source_plus_one_counters =
                    Some(Quantity::from(i64::from(count.value)));
            }
            SourceCost::TapSelf {} if !result.tap => result.tap = true,
            SourceCost::Mana { symbols }
                if result.mana.is_none()
                    && !symbols.is_empty()
                    && symbols
                        .iter()
                        .all(|s| !s.is_empty() && !s.contains(['{', '}'])) =>
            {
                let m = mana_cost(
                    &symbols
                        .iter()
                        .map(|s| format!("{{{s}}}"))
                        .collect::<String>(),
                )?;
                if m.colored.contains(&Color::C) {
                    return Err(unsupported("cost", "colorless-specific costs excluded"));
                }
                result.mana = Some(m);
            }
            SourceCost::Sacrifice { what }
                if !result.sacrifice
                    && *what
                        == serde_json::json!({"ref":"this","noun":match kind {
                            CardKind::Artifact => "artifact",
                            CardKind::Enchantment => "enchantment",
                            CardKind::Land => "land",
                            _ => "creature",
                        }}) =>
            {
                result.sacrifice = true
            }
            _ => {
                return Err(unsupported(
                    "cost",
                    "unsupported complete permanent activation costs",
                ));
            }
        }
    }
    Ok(result)
}

pub(crate) fn lower_mana_ability(
    costs: &[SourceCost],
    effects: &[Sentence],
    restriction: Option<&SourceActivationRestriction>,
    block: usize,
    kind: CardKind,
) -> Result<Option<ManaAbility>, Failure> {
    if effects
        .iter()
        .any(|s| !s.bindings.is_empty() || s.condition.is_some() || s.condition_kind.is_some())
    {
        return Err(unsupported(
            "bindings",
            "bindings outside spell instructions",
        ));
    }
    if !effects
        .iter()
        .flat_map(|s| &s.steps)
        .any(|s| matches!(s, Step::AddMana { .. }))
    {
        return Ok(None);
    }
    let [Sentence { steps, .. }] = effects else {
        return Err(unsupported("mana", "extra mana effects"));
    };
    let [
        Step::AddMana {
            who,
            mana,
            spend_restriction: None,
        },
    ] = steps.as_slice()
    else {
        return Err(unsupported("mana", "extra mana steps"));
    };
    let you = PlayerSelector {
        player: "you".into(),
        selection_id: None,
        of: None,
    };
    if *who != you {
        return Err(unsupported("mana", "unsupported mana recipient"));
    }
    let production = match mana {
        SourceMana::Fixed { symbols } if !symbols.is_empty() => ManaProduction::Fixed {
            symbols: symbols.clone(),
        },
        SourceMana::AnyColor { amount } if fixed_one(amount) => ManaProduction::AnyColor,
        SourceMana::CommanderColor { amount, whose } if fixed_one(amount) && *whose == you => {
            ManaProduction::CommanderColor
        }
        _ => return Err(unsupported("mana", "unsupported complete mana production")),
    };
    let costs = lower_permanent_costs(costs, kind)?;
    if costs.remove_source_plus_one_counters.is_some() {
        if costs.tap
            || costs.mana.is_some()
            || costs.sacrifice
            || restriction.is_none()
            || !matches!(&production, ManaProduction::Fixed { symbols } if !symbols.is_empty())
        {
            return Err(unsupported(
                "mana",
                "unsupported counter-removal mana shape",
            ));
        }
    } else if !costs.tap || restriction.is_some() {
        return Err(unsupported("mana", "unsupported mana cost/restriction"));
    }
    Ok(Some(ManaAbility {
        self_damage: None,
        restriction: restriction.map(|_| ManaActivationRestriction::OnceEachTurn),
        costs,
        production,
        source_block: source_block(block)?,
    }))
}
