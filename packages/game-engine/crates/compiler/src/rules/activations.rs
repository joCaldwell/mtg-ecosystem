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

pub(crate) fn lower_activation(
    costs: &[SourceCost],
    effects: &[Sentence],
    block: usize,
    kind: CardKind,
) -> Result<ActivatedAbility, Failure> {
    if let Some(ability) = land_sacrifice_draw::lower(costs, effects, block, kind)? {
        return Ok(ability);
    }
    if let Some(ability) = paid_counted_draw::lower(costs, effects, block, kind)? {
        return Ok(ability);
    }
    if let Some(ability) = activated_artifact_draw::lower_two_draws(costs, effects, block, kind)? {
        return Ok(ability);
    }
    if effects
        .iter()
        .any(|s| !s.bindings.is_empty() || s.condition.is_some() || s.condition_kind.is_some())
    {
        return Err(unsupported(
            "bindings",
            "bindings outside spell instructions",
        ));
    }
    let cost = match costs {
        [
            SourceCost::TapSelf {},
            SourceCost::PayLife { amount },
            SourceCost::Sacrifice { what },
        ] if kind == CardKind::Land && *what == serde_json::json!({"ref":"this","noun":"land"}) => {
            ActivationCost::TapPayLifeSacrificeSource {
                life: typed_fetchland::life(amount)?,
            }
        }
        [SourceCost::Sacrifice { what }] if what["ref"] == "filter" => {
            ActivationCost::SacrificePermanent {
                count: 1,
                filter: lower_sacrifice_filter(what)?,
                mana: None,
            }
        }
        [SourceCost::Mana { symbols }, SourceCost::Sacrifice { what }]
            if what["ref"] == "filter" =>
        {
            let mana = lower_permanent_costs(
                &[SourceCost::Mana {
                    symbols: symbols.clone(),
                }],
                kind,
            )?
            .mana;
            ActivationCost::SacrificePermanent {
                count: 1,
                filter: lower_sacrifice_filter(what)?,
                mana,
            }
        }
        [SourceCost::Sacrifice { what }]
            if *what == serde_json::json!({"ref":"this","noun":"creature"}) =>
        {
            ActivationCost::SacrificeSource
        }
        [SourceCost::TapSelf {}] => ActivationCost::TapSource,
        [SourceCost::Mana { symbols }] if !symbols.is_empty() => {
            // Each parser symbol is one complete Scryfall mana symbol. Reject
            // injected braces rather than interpreting extra unparsed symbols.
            if symbols
                .iter()
                .any(|symbol| symbol.is_empty() || symbol.contains(['{', '}']))
            {
                return Err(unsupported(
                    "abilities.costs",
                    "invalid complete mana-cost symbol",
                ));
            }
            let text = symbols
                .iter()
                .map(|symbol| format!("{{{symbol}}}"))
                .collect::<String>();
            ActivationCost::Mana {
                cost: mana_cost(&text)?,
            }
        }
        _ if kind == CardKind::Artifact
            || matches!(costs, [SourceCost::Mana { .. }, SourceCost::TapSelf {}] if kind == CardKind::Creature)
            || matches!(costs, [SourceCost::Mana { .. }, SourceCost::Sacrifice { what }] if kind == CardKind::Creature && *what == serde_json::json!({"ref":"this","noun":"creature"})) =>
        {
            ActivationCost::Composite {
                costs: lower_permanent_costs(costs, kind)?,
            }
        }
        _ => {
            return Err(unsupported(
                "abilities.costs",
                "unsupported complete activation cost or combined costs",
            ));
        }
    };
    if matches!(cost, ActivationCost::TapPayLifeSacrificeSource { .. }) {
        return typed_fetchland::lower(cost, effects, block);
    }
    // Only sacrificing this exact source establishes the first pronoun's
    // identity. Choosing another permanent does not make it the invocation.
    validate_damage_sources(
        effects,
        DamageSourceBinding::Invocation {
            first_it_is_self: matches!(cost, ActivationCost::SacrificeSource),
        },
    )?;
    let source_block = source_block(block)?;
    let mut program = SpellProgram {
        optional_additional_costs: vec![],
        additional_sacrifice: None,
        timing: SpellTiming::Instant,
        resolution: SpellResolution::Instructions,
        functioning_zone: FunctioningZone::Stack,
        targets: vec![],
        instructions: vec![],
        modes: vec![],
    };
    // CR 602.2b, 608.2h, 611.2c: reuse the whole-sentence set coalescer,
    // then require the complete pure controller-creature activation composition.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    if effects
        .iter()
        .flat_map(|s| &s.steps)
        .any(|step| matches!(step, Step::GainAbilities { .. }))
    {
        append_spell_effects(
            &mut program,
            effects,
            block,
            &mut BTreeMap::new(),
            PlayerBinding::Unbound,
            Context::Activated,
        )?;
        let ability = ActivatedAbility {
            functioning_zone: FunctioningZone::Battlefield,
            cost,
            program,
        };
        if kind != CardKind::Creature
            || sacrifice_source_creature_set_keyword_ability(&ability).is_none()
        {
            return Err(unsupported(
                "activation",
                "unsupported complete set keyword activation",
            ));
        }
        return Ok(ability);
    }
    if matches!(cost, ActivationCost::SacrificePermanent { .. })
        || (matches!(cost, ActivationCost::SacrificeSource)
            && effects.iter().flat_map(|s| &s.steps).any(|i| {
                matches!(
                    i,
                    Step::Damage {
                        source: DamageSource::It {},
                        ..
                    } | Step::Search { .. }
                )
            }))
    {
        append_spell_effects(
            &mut program,
            effects,
            block,
            &mut BTreeMap::new(),
            PlayerBinding::Unbound,
            Context::Activated,
        )?;
        return Ok(ActivatedAbility {
            functioning_zone: FunctioningZone::Battlefield,
            cost,
            program,
        });
    }
    if effects.iter().flat_map(|s| &s.steps).any(|step| {
        matches!(
            step,
            Step::Tap { .. } | Step::Counter { .. } | Step::Untap { .. }
        ) || matches!(step, Step::ReturnToHand { what, to, .. }
                if to == "owner" && what["filter"]["cls"] == "permanent")
            || (matches!(cost, ActivationCost::TapSource) && matches!(step, Step::Destroy { .. }))
    }) {
        append_spell_effects(
            &mut program,
            effects,
            block,
            &mut BTreeMap::new(),
            PlayerBinding::Unbound,
            Context::Activated,
        )?;
        return Ok(ActivatedAbility {
            functioning_zone: FunctioningZone::Battlefield,
            cost,
            program,
        });
    }
    if kind == CardKind::Creature && matches!(cost, ActivationCost::Composite { .. }) {
        append_spell_effects(
            &mut program,
            effects,
            block,
            &mut BTreeMap::new(),
            PlayerBinding::Unbound,
            Context::Activated,
        )?;
        if !matches!(
            program.instructions.as_slice(),
            [kernel!(Destroy {
                what: target_object!(_),
                regeneration_forbidden: _
            })]
        ) {
            return Err(unsupported(
                "activation",
                "unsupported targeted sacrifice program",
            ));
        }
        return Ok(ActivatedAbility {
            functioning_zone: FunctioningZone::Battlefield,
            cost,
            program,
        });
    }
    let mut slots = BTreeMap::new();
    for sentence in effects {
        if sentence.steps.is_empty() {
            return Err(invalid(
                "effects.steps",
                "empty activated instruction group",
            ));
        }
        for step in &sentence.steps {
            let you = PlayerSelector {
                player: "you".into(),
                selection_id: None,
                of: None,
            };
            let instruction = match step {
                Step::CreateReplacement { .. } => {
                    return Err(unsupported("replacement", "unsupported replacement"));
                }
                Step::Regenerate { what } => {
                    let reason = if *what == serde_json::json!({"ref":"this","noun":"creature"}) {
                        "self-regeneration requires a complete printed fixed-mana activation"
                    } else {
                        "regeneration requires this creature as the subject"
                    };
                    return Err(unsupported("effects.regenerate", reason));
                }
                Step::PreventDamage {
                    prevention,
                    duration,
                } => {
                    lower_prevention(&mut program, &mut slots, prevention, duration, source_block)?
                }
                Step::Damage {
                    optional: None,
                    source: DamageSource::This { noun },
                    amount,
                    to,
                } if noun == "creature" => {
                    let DamageDestination::AnyTarget { selection_id } = to else {
                        return Err(unsupported("damage", "unsupported damage destination"));
                    };
                    Instruction::damage_target(
                        bind_target(
                            &mut program,
                            &mut slots,
                            selection_id,
                            TargetDomain::AnyDamageTarget,
                        )?,
                        kir::Amount::Fixed {
                            value: fixed_positive_value(amount)?,
                        },
                        source_block,
                    )
                }
                Step::Draw {
                    who,
                    amount,
                    optional: true,
                } if *who == you && amount.fixed_one() => {
                    Instruction::OptionalDrawCards { source_block }
                }
                Step::Draw {
                    who,
                    amount,
                    optional: false,
                } if *who == you && amount.fixed_one() => Instruction::draw(
                    PlayerExpr::you(Context::Activated),
                    Quantity::from(1),
                    source_block,
                ),
                Step::Discard(DiscardStep::Chosen { who, what })
                    if *who == you
                        && what.reference == "filter"
                        && fixed_one(&what.count)
                        && what.filter
                            == ObjectFilter::Class(CardFilter { cls: "card".into() }) =>
                {
                    Instruction::discard(
                        kir::PlayerExpr::you(kir::Context::Activated),
                        kir::Amount::Fixed {
                            value: Quantity::from(1),
                        },
                        false,
                        source_block,
                    )
                }
                _ => {
                    return Err(unsupported(
                        "effects.steps",
                        "unsupported activated operator, controller or cardinality",
                    ));
                }
            };
            program.instructions.push(instruction);
        }
    }
    if program.instructions.is_empty() {
        return Err(unsupported("abilities.effects", "empty activated effects"));
    }
    // CR 602.1–2 / 608.2c: the cost is paid while activating; the ordered
    // program resolves on the stack independently of its sacrificed source.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    Ok(ActivatedAbility {
        functioning_zone: FunctioningZone::Battlefield,
        cost,
        program,
    })
}
