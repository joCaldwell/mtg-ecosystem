//! Resolving effect steps (CR 608.2): sentences of spell and ability programs.
use super::*;

pub(crate) fn append_spell_effects(
    program: &mut SpellProgram,
    effects: &[Sentence],
    block: usize,
    slots: &mut BTreeMap<String, u32>,
    player_binding: PlayerBinding,
    context: Context,
) -> Result<(), Failure> {
    if effects.is_empty() {
        return Err(unsupported("abilities.effects", "empty spell effects"));
    }
    let source_block = source_block(block)?;
    for sentence in effects {
        if sentence.condition.is_some() || sentence.condition_kind.is_some() {
            return Err(unsupported(
                "effects.condition",
                "unconsumed conditional instruction",
            ));
        }
        let counted = if sentence.bindings.is_empty() {
            false
        } else if sentence.bindings
            == [
                serde_json::json!({"variable":"x","value":{"amount":"count","of":{"types":["creature"],"zone":{"zone":"battlefield"}}}}),
            ]
        {
            true
        } else {
            return Err(unsupported("bindings", "unsupported complete X binding"));
        };
        if counted
            && !matches!(sentence.steps.as_slice(),[Step::Damage {amount,..}] if *amount==serde_json::json!({"amount":"x"}))
        {
            return Err(unsupported("bindings", "unconsumed count binding"));
        }
        if sentence.steps.is_empty() {
            return Err(invalid("effects.steps", "empty instruction group"));
        }
        for step in &sentence.steps {
            if player_binding == PlayerBinding::Unbound
                && matches!(step, Step::LoseLife { who, .. } if who.player == "that-player")
            {
                return Err(unsupported("trigger", "unbound death controller"));
            }
            if let Some(i) = lower_wipes_step(step, counted, source_block, context)? {
                program.instructions.push(i);
                continue;
            }
            if counted {
                return Err(unsupported("bindings", "unconsumed count binding"));
            }
            let instruction = match step {
                Step::CreateReplacement { .. } => {
                    return Err(unsupported("replacement", "unsupported replacement"));
                }
                Step::Exile { what } => Instruction::move_target(
                    bind_permanent_target(program, slots, what)?,
                    kir::Destination::Exile,
                    source_block,
                ),
                Step::Untap { what } => {
                    if what["filter"] != serde_json::json!({"cls":"permanent"})
                        && what["filter"] != serde_json::json!({"types":["creature"]})
                    {
                        return Err(unsupported(
                            "effects.untap",
                            "requires an ordinary permanent or activated creature target",
                        ));
                    }
                    Instruction::untap_target(
                        bind_permanent_target(program, slots, what)?,
                        source_block,
                    )
                }
                Step::Tap { what } => {
                    let target_slot = if what["filter"] == serde_json::json!({"types":["creature"]})
                    {
                        let target: Target =
                            serde_json::from_value(what.clone()).map_err(|_| {
                                unsupported(
                                    "effects.tap",
                                    "unsupported complete creature tap target",
                                )
                            })?;
                        bind_creature_target(program, slots, &target)?
                    } else {
                        // Definition validation fences this broader selector to
                        // the reviewed activated artifact tap program.
                        bind_permanent_target(program, slots, what)?
                    };
                    Instruction::tap_target(target_slot, source_block)
                }
                Step::Destroy {
                    what,
                    optional,
                    regeneration_forbidden,
                } => {
                    if what["ref"] == "all" && regeneration_forbidden.is_some() {
                        return Err(unsupported(
                            "regeneration",
                            "regeneration-forbidden mass destruction is outside this slice",
                        ));
                    }
                    if optional.is_none()
                        && regeneration_forbidden.is_none()
                        && *what == serde_json::json!({"ref":"all","filter":{"types":["creature"]}})
                    {
                        Instruction::destroy_all_creatures(source_block)
                    } else if optional.is_none()
                        && regeneration_forbidden.is_none()
                        && what["filter"] == serde_json::json!({"types":["creature"]})
                    {
                        let t: Target = serde_json::from_value(what.clone()).map_err(|_| {
                            unsupported("effects.destroy", "unsupported destroy target")
                        })?;
                        Instruction::destroy_target(
                            bind_creature_target(program, slots, &t)?,
                            false,
                            source_block,
                        )
                    } else {
                        if regeneration_forbidden.is_some() && *regeneration_forbidden != Some(true)
                        {
                            return Err(unsupported(
                                "regeneration",
                                "only cannot-be-regenerated destroy is supported",
                            ));
                        }
                        let target_slot = bind_permanent_target(program, slots, what)?;
                        match (
                            optional.unwrap_or(false),
                            regeneration_forbidden.unwrap_or(false),
                        ) {
                            (false, forbidden) => {
                                Instruction::destroy_target(target_slot, forbidden, source_block)
                            }
                            (true, false) => Instruction::optional_destroy_target(
                                context,
                                target_slot,
                                source_block,
                            ),
                            (true, true) => {
                                return Err(unsupported(
                                    "regeneration",
                                    "an optional destroy that forbids regeneration is outside this slice",
                                ));
                            }
                        }
                    }
                }
                Step::CreateToken {
                    count,
                    token,
                    who: None,
                } if count.amount == "fixed" && count.value > 0 => {
                    let token = lower_creature_token(token)?;
                    Instruction::create_tokens(
                        context,
                        count.value,
                        kir::TokenSpec::Creature { spec: token },
                        source_block,
                    )
                }
                Step::PutCounters { on, counter, count } if counter.r#type == "+1/+1" => {
                    let on = match on {
                        SourceCounterRecipient::Target(t) => {
                            kir::SetExpr::target(bind_creature_target(program, slots, t)?)
                        }
                        SourceCounterRecipient::Source(t)
                            if t.reference == "this" && t.noun == "creature" =>
                        {
                            kir::SetExpr::source()
                        }
                        _ => {
                            return Err(unsupported(
                                "effects.counters.on",
                                "unsupported counter recipient",
                            ));
                        }
                    };
                    Instruction::plus_one_counters(
                        on,
                        fixed_positive(&decode::<Count>(
                            &count.to_string(),
                            "effects.counters.count",
                        )?)?,
                        source_block,
                    )
                }
                Step::Pump {
                    what,
                    power,
                    toughness,
                    duration,
                } if *what == serde_json::json!({"ref":"this","noun":"creature"})
                    && duration.duration == "end-of-turn" =>
                {
                    Instruction::pump(
                        kir::SetExpr::source(),
                        signed_fixed(power)?,
                        signed_fixed(toughness)?,
                        source_block,
                    )
                }
                Step::Pump {
                    what,
                    power,
                    toughness,
                    duration,
                } if duration.duration == "end-of-turn" => Instruction::pump(
                    kir::SetExpr::target(bind_pump_target(program, slots, what)?),
                    signed_fixed(power)?,
                    signed_fixed(toughness)?,
                    source_block,
                ),
                Step::Damage {
                    optional: None,
                    source: DamageSource::SelfReference {},
                    amount,
                    to,
                } => {
                    let DamageDestination::AnyTarget { selection_id } = to else {
                        return Err(unsupported("damage", "unsupported damage destination"));
                    };
                    Instruction::damage_target(
                        bind_target(program, slots, selection_id, TargetDomain::AnyDamageTarget)?,
                        kir::Amount::Fixed {
                            value: fixed_positive_value(amount)?,
                        },
                        source_block,
                    )
                }
                Step::Mill { who, amount } => {
                    fixed_mill::instruction(program, slots, who, amount, context, source_block)?
                }
                Step::GainLife {
                    who,
                    amount,
                    for_each: None,
                } => Instruction::act(
                    Verb::GainLife {
                        player: life_player(
                            program,
                            slots,
                            who,
                            context,
                            player_binding.event_role(),
                        )?,
                        amount: Amount::Fixed {
                            value: fixed_positive(
                                &serde_json::from_value(amount.clone()).map_err(|_| {
                                    unsupported("amount", "unsupported fixed amount")
                                })?,
                            )?,
                        },
                    },
                    source_block,
                ),
                Step::LoseLife { who, amount } => Instruction::act(
                    Verb::LoseLife {
                        player: life_player(
                            program,
                            slots,
                            who,
                            context,
                            player_binding.event_role(),
                        )?,
                        amount: Amount::Fixed {
                            value: fixed_positive(
                                &serde_json::from_value(amount.clone()).map_err(|_| {
                                    unsupported("amount", "unsupported fixed amount")
                                })?,
                            )?,
                        },
                    },
                    source_block,
                ),
                Step::Discard(DiscardStep::Chosen { who, what })
                    if player_binding == PlayerBinding::DamageRecipient
                        && matches!(who.player.as_str(), "that-player" | "you")
                        && who.selection_id.is_none()
                        && who.of.is_none()
                        && what.reference == "filter"
                        && fixed_one(&what.count)
                        && what.filter
                            == ObjectFilter::Class(CardFilter { cls: "card".into() }) =>
                {
                    Instruction::discard(
                        if who.player == "that-player" {
                            PlayerExpr::event(kir::EventPlayerRole::DamagedPlayer)
                        } else {
                            PlayerExpr::you(context)
                        },
                        Amount::fixed(1),
                        false,
                        source_block,
                    )
                }
                Step::Draw {
                    who,
                    amount: DrawAmount::Fixed { value },
                    optional: false,
                } => {
                    let player =
                        life_player(program, slots, who, context, player_binding.event_role())?;
                    // CR 121.1: a group of opponents has no audited draw cell.
                    if player == PlayerExpr::EachOpponent {
                        return Err(unsupported("definition", "unsupported each-opponent draw"));
                    }
                    if !(1..=2).contains(value) {
                        return Err(unsupported("effects.draw", "unsupported fixed draw count"));
                    }
                    // CR 121.2: multiple draws are performed one at a time.
                    for _ in 1..*value {
                        program.instructions.push(Instruction::draw(
                            player.clone(),
                            Quantity::from(1),
                            source_block,
                        ));
                    }
                    Instruction::draw(player, Quantity::from(1), source_block)
                }
                Step::ReturnToHand {
                    what,
                    to,
                    optional: None,
                } if to == "owner" => {
                    let target_slot = if what["filter"]["cls"] == "permanent" {
                        bind_permanent_target(program, slots, what)?
                    } else {
                        let t: Target = serde_json::from_value(what.clone())
                            .map_err(|_| unsupported("effects.return", "unsupported target"))?;
                        bind_creature_target(program, slots, &t)?
                    };
                    Instruction::move_target(target_slot, kir::Destination::OwnerHand, source_block)
                }
                Step::MoveZone {
                    what,
                    to,
                    tapped: None,
                    order: None,
                } if *to
                    == serde_json::json!({"zone":"library","owner":"its-owner","position":"top"}) =>
                {
                    let t: Target = serde_json::from_value(what.clone())
                        .map_err(|_| unsupported("effects.move", "unsupported target"))?;
                    Instruction::move_target(
                        bind_creature_target(program, slots, &t)?,
                        kir::Destination::OwnerLibraryTop,
                        source_block,
                    )
                }
                Step::Sacrifice {
                    who,
                    what,
                    chosen_by,
                } if player_binding == PlayerBinding::DamageRecipient
                    && who.player == "that-player"
                    && who.selection_id.is_none()
                    && who.of.is_none()
                    && who == chosen_by
                    && what.reference == "filter"
                    && fixed_one(&what.count)
                    && what.filter
                        == ObjectFilter::Types(Filter {
                            types: vec!["creature".into()],
                        }) =>
                {
                    Instruction::sacrifice_creature(
                        PlayerExpr::event(kir::EventPlayerRole::DamagedPlayer),
                        source_block,
                    )
                }
                Step::Sacrifice {
                    who,
                    what,
                    chosen_by,
                } if who.player == "each-opponent"
                    && who.selection_id.is_none()
                    && who.of.is_none()
                    && who == chosen_by
                    && what.reference == "filter"
                    && fixed_one(&what.count)
                    && what.filter
                        == ObjectFilter::Types(Filter {
                            types: vec!["creature".into()],
                        }) =>
                {
                    Instruction::sacrifice_creature(PlayerExpr::EachOpponent, source_block)
                }
                Step::Sacrifice {
                    who,
                    what,
                    chosen_by,
                } if who.player == "target-opponent"
                    && who.of.is_none()
                    && who == chosen_by
                    && what.reference == "filter"
                    && fixed_one(&what.count)
                    && what.filter
                        == ObjectFilter::Types(Filter {
                            types: vec!["creature".into()],
                        }) =>
                {
                    // CR 601.2c / 608.2d / 701.21: opponent target is bound at
                    // announcement; that player selects a controlled creature
                    // only when the sacrifice instruction resolves.
                    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
                    let target_slot = bind_target(
                        program,
                        slots,
                        who.selection_id.as_deref().unwrap_or(""),
                        TargetDomain::OpponentPlayer,
                    )?;
                    Instruction::sacrifice_creature(PlayerExpr::target(target_slot), source_block)
                }
                _ => {
                    return Err(unsupported(
                        "effects.steps",
                        "unsupported complete spell operator, destination, chooser or cardinality",
                    ));
                }
            };
            program.instructions.push(instruction);
        }
    }
    Ok(())
}

pub(crate) fn lower_wipes_step(
    step: &Step,
    counted: bool,
    source_block: u32,
    context: Context,
) -> Result<Option<Instruction>, Failure> {
    use serde_json::json;
    Ok(Some(match step {
        Step::Exile { what } if what["ref"] == "all" || what["ref"] == "each" => Instruction::act(
            Verb::Move {
                what: lower_board_set(what, context)?,
                to: Destination::Exile,
            },
            source_block,
        ),
        Step::Destroy {
            what,
            optional: None,
            regeneration_forbidden: None,
        } if (what["ref"] == "all" || what["ref"] == "each")
            && *what != json!({"ref":"all","filter":{"types":["creature"]}}) =>
        {
            Instruction::act(
                Verb::Destroy {
                    what: lower_board_set(what, context)?,
                    regeneration_forbidden: false,
                },
                source_block,
            )
        }
        Step::ReturnToHand {
            what,
            to,
            optional: None,
        } if to == "owner" && (what["ref"] == "all" || what["ref"] == "each") => Instruction::act(
            Verb::Move {
                what: lower_board_set(what, context)?,
                to: Destination::OwnerHand,
            },
            source_block,
        ),
        Step::Pump {
            what,
            power,
            toughness,
            duration,
        } if (what["ref"] == "all" || what["ref"] == "each")
            && duration.duration == "end-of-turn" =>
        {
            Instruction::pump(
                lower_object_set(what)?.kernel(context),
                signed_fixed(power)?,
                signed_fixed(toughness)?,
                source_block,
            )
        }
        Step::Damage {
            source: DamageSource::SelfReference {},
            amount,
            to,
            optional: None,
        } if matches!(
            to,
            DamageDestination::Object { .. } | DamageDestination::Each { .. }
        ) =>
        {
            let refs = match to {
                DamageDestination::Object { reference } => vec![reference.clone()],
                DamageDestination::Each { refs } => refs.clone(),
                _ => unreachable!(),
            };
            let mut sets = vec![];
            let mut each_player = false;
            for r in refs {
                if r == json!({"player":"each-player"}) && !each_player {
                    each_player = true;
                } else {
                    sets.push(lower_object_set(&r)?.kernel(context));
                }
            }
            let amount = if *amount == json!({"amount":"x"}) {
                if counted {
                    // CR 608.2h: the number of creatures on the battlefield.
                    kir::Amount::Count {
                        set: SetExpr::battlefield(kir::Pred::creatures()),
                    }
                } else {
                    kir::Amount::announced_x()
                }
            } else {
                if counted {
                    return Err(unsupported("binding", "unused X"));
                }
                if *amount
                    == json!({"amount":"cast-mana-value-sum","who":{"player":"you"},
                    "during":"this-turn","excluding":{"ref":"this","noun":"spell"}})
                {
                    kir::Amount::OtherCastsManaValue {
                        caster: PlayerExpr::you(context),
                    }
                } else {
                    kir::Amount::Fixed {
                        value: fixed_positive_value(amount)?,
                    }
                }
            };
            SetDamage::instruction(sets, each_player, amount, source_block)
        }
        Step::GainLife {
            for_each: Some(_), ..
        } => return Err(unsupported("forEach", "unsupported event result")),
        Step::Sacrifice {
            who,
            what,
            chosen_by,
        } if who.player == "each-player"
            && who == chosen_by
            && who.of.is_none()
            && who.selection_id.is_none()
            && what.reference == "filter"
            && matches!(&what.filter,ObjectFilter::Types(f) if f.types==["creature"])
            && fixed_one(&what.count) =>
        {
            Instruction::sacrifice_creature(PlayerExpr::EachPlayer, source_block)
        }
        _ => return Ok(None),
    }))
}
