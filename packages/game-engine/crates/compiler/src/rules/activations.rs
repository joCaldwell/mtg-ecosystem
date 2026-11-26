//! Activated and mana abilities and their costs (CR 602, 605).
//!
//! CR 605.1a: an activated ability without targets that could add mana as it
//! resolves is a mana ability. Its production, costs and the instructions it
//! performs after adding mana ("riders") lower here; riders are Kernel nodes
//! that `kir::mana_rider` admits. CR 106.1, 106.7, 118.3, 119.4.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

const ALL_COLORS: [Color; 5] = [Color::W, Color::U, Color::B, Color::R, Color::G];

fn fixed_amount(value: &serde_json::Value) -> Result<Quantity, Failure> {
    fixed_positive_amount(&typed(value, "amount")?)
}

pub(crate) fn lower_permanent_costs(
    costs: &[SourceCost],
    kind: CardKind,
) -> Result<PermanentActivationCosts, Failure> {
    let mut result = PermanentActivationCosts {
        remove_source_plus_one_counters: None,
        tap: false,
        mana: None,
        sacrifice: false,
        pay_life: None,
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
            // CR 118.3, 119.4: a fixed positive life payment.
            SourceCost::PayLife { amount } if result.pay_life.is_none() => {
                result.pay_life = Some(fixed_amount(amount)?);
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

/// CR 106.1a, 106.7: the complete production of the ability's one AddMana
/// step. A choice lists single, distinct symbols; a choice of multi-symbol
/// bundles (filter lands) or a "could produce" over anything but one land
/// controlled by "you" or "an opponent" rejects.
fn production(mana: &SourceMana, you: &PlayerSelector) -> Result<ManaProduction, Failure> {
    let unsupported_production = || unsupported("mana", "unsupported complete mana production");
    Ok(match mana {
        SourceMana::Fixed { symbols } if !symbols.is_empty() => ManaProduction::Fixed {
            symbols: symbols.clone(),
        },
        SourceMana::AnyColor { amount } if fixed_one(amount) => ManaProduction::AnyColor,
        SourceMana::CommanderColor { amount, whose } if fixed_one(amount) && whose == you => {
            ManaProduction::CommanderColor
        }
        SourceMana::Choice { options } => {
            let mut colors = vec![];
            for option in options {
                match option {
                    SourceMana::Fixed { symbols } => match symbols.as_slice() {
                        [color] if !colors.contains(color) => colors.push(*color),
                        [_] => return Err(unsupported("mana", "repeated mana choice")),
                        _ => return Err(unsupported("mana", "mana combination choice")),
                    },
                    _ => return Err(unsupported_production()),
                }
            }
            if colors.len() < 2 {
                return Err(unsupported_production());
            }
            ManaProduction::Choose {
                colors,
                amount: Quantity::from(1),
            }
        }
        SourceMana::AnyOneColor { amount } => ManaProduction::Choose {
            colors: ALL_COLORS.to_vec(),
            amount: fixed_amount(amount)?,
        },
        SourceMana::CouldProduce {
            amount,
            what,
            color_only,
        } if fixed_one(amount) => {
            let lands = |control: &str| {
                serde_json::json!({
                    "ref": "filter",
                    "filter": {"types": ["land"], "control": control},
                    "count": {"amount": "fixed", "value": 1}
                })
            };
            let controller = if *what == lands("you") {
                ManaLandController::You
            } else if *what == lands("opponent") {
                ManaLandController::Opponents
            } else {
                return Err(unsupported("mana", "could-produce source set"));
            };
            ManaProduction::LandsCouldProduce {
                controller,
                colors_only: *color_only,
            }
        }
        _ => return Err(unsupported_production()),
    })
}

/// CR 605.1a, 605.3b: an instruction after the AddMana step. The admitted
/// rows read only the ability's source and controller: "[this] deals N
/// damage to you" (CR 120.3a) and "you gain N life" (CR 119.3).
fn rider(
    step: &Step,
    you: &PlayerSelector,
    is_self: &dyn Fn(&str) -> bool,
) -> Result<kir::Node, Failure> {
    let controller = || kir::Ref::controller(Context::Activated);
    let node = match step {
        Step::Damage {
            source,
            amount,
            to: DamageDestination::Player { reference },
            optional: None,
        } if match source {
            DamageSource::This { noun } => is_self(noun),
            DamageSource::SelfReference {} => true,
            DamageSource::It {} => false,
        } && *reference == serde_json::json!({"player": "you"}) =>
        {
            kir::Node::Do(Verb::Damage {
                source: kir::Ref::Source,
                to: kir::DamageTo::Ref { who: controller() },
                amount: kir::Amount::Fixed {
                    value: fixed_amount(amount)?,
                },
            })
        }
        Step::GainLife {
            who,
            amount,
            for_each: None,
        } if who == you => kir::Node::Do(Verb::GainLife {
            player: PlayerExpr::you(Context::Activated),
            amount: kir::Amount::Fixed {
                value: fixed_amount(amount)?,
            },
        }),
        _ => return Err(unsupported("mana", "unsupported mana ability rider")),
    };
    kir::mana_rider(&node).map_err(|e| unsupported("mana", format!("mana ability rider: {e}")))?;
    Ok(node)
}

pub(crate) fn lower_mana_ability(
    costs: &[SourceCost],
    effects: &[Sentence],
    restriction: Option<&SourceActivationRestriction>,
    block: usize,
    kind: CardKind,
    is_self: &dyn Fn(&str) -> bool,
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
    let mut steps = effects.iter().flat_map(|s| &s.steps);
    if !effects
        .iter()
        .flat_map(|s| &s.steps)
        .any(|s| matches!(s, Step::AddMana { .. }))
    {
        return Ok(None);
    }
    // CR 608.2c: the mana is added first, then the riders in printed order.
    let Some(Step::AddMana {
        who,
        mana,
        spend_restriction: None,
    }) = steps.next()
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
    let production = production(mana, &you)?;
    let riders = steps
        .map(|step| {
            if matches!(step, Step::AddMana { .. }) {
                Err(unsupported("mana", "extra mana steps"))
            } else {
                rider(step, &you, is_self)
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    let costs = lower_permanent_costs(costs, kind)?;
    if costs.remove_source_plus_one_counters.is_some() {
        if costs.tap
            || costs.mana.is_some()
            || costs.sacrifice
            || costs.pay_life.is_some()
            || restriction.is_none()
            || !riders.is_empty()
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
    // A rider of a sacrificed source would read last-known information the
    // mana executor does not keep.
    if costs.sacrifice && !riders.is_empty() {
        return Err(unsupported("mana", "rider on a sacrificed mana source"));
    }
    Ok(Some(ManaAbility {
        restriction: restriction.map(|_| ManaActivationRestriction::OnceEachTurn),
        condition: None,
        costs,
        production,
        riders,
        source_block: source_block(block)?,
    }))
}
