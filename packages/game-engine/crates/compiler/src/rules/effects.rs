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
    if let Some((draw, count)) = hand_return::coalesce(effects)? {
        if block != 0
            || !program.instructions.is_empty()
            || !program.targets.is_empty()
            || !program.modes.is_empty()
            || !matches!(player_binding, PlayerBinding::Unbound)
        {
            return Err(unsupported(
                "hand return",
                "requires complete ordinary source",
            ));
        }
        program.instructions.extend([
            Instruction::draw(PlayerExpr::you(context), draw, 0),
            Instruction::PutHandOnLibrary {
                count,
                source_block: 0,
            },
        ]);
        return Ok(());
    }
    if let Some(amount) = library_order::coalesce(effects)? {
        if block != 0
            || !program.instructions.is_empty()
            || !program.targets.is_empty()
            || !program.modes.is_empty()
            || !matches!(player_binding, PlayerBinding::Unbound)
        {
            return Err(unsupported(
                "library order",
                "requires complete ordinary source",
            ));
        }
        program.instructions.extend([
            Instruction::OrderLibraryTop {
                amount,
                source_block: 0,
            },
            Instruction::OptionalShuffleLibrary { source_block: 0 },
        ]);
        return Ok(());
    }
    if let Some((amount, remainder)) = library_selection::coalesce(effects)? {
        if block != 0
            || !program.instructions.is_empty()
            || !program.targets.is_empty()
            || !program.modes.is_empty()
            || !matches!(player_binding, PlayerBinding::Unbound)
        {
            return Err(unsupported(
                "library selection",
                "requires standalone whole source",
            ));
        }
        program.instructions.push(Instruction::SelectLibraryCard {
            amount,
            remainder,
            source_block: 0,
        });
        return Ok(());
    }
    if let [
        Sentence {
            bindings,
            steps,
            condition: None,
            condition_kind: None,
        },
    ] = effects
        && let [Step::Modal { count, options }] = steps.as_slice()
    {
        // Outer sentence bindings define executable values; recursing only
        // into the options would bypass ordinary binding validation below.
        if !bindings.is_empty() {
            return Err(unsupported(
                "bindings",
                "bindings on standalone modal instructions are unsupported",
            ));
        }
        if let ModeCount::Minimum { minimum } = count {
            return Err(unsupported(
                "effects.modal",
                format!("atLeast {minimum} requires a complete supported standalone mode family"),
            ));
        }
        if !matches!(count, ModeCount::Exact { exactly: 1 })
            || options.len() < 2
            || !program.modes.is_empty()
            || !program.targets.is_empty()
            || !program.instructions.is_empty()
        {
            return Err(unsupported(
                "effects.modal",
                "only a complete standalone choose-one program is supported",
            ));
        }
        for option in options {
            let mut child = SpellProgram {
                optional_additional_costs: vec![],
                additional_sacrifice: None,
                timing: program.timing,
                resolution: program.resolution,
                functioning_zone: program.functioning_zone,
                targets: vec![],
                instructions: vec![],
                modes: vec![],
            };
            append_spell_effects(
                &mut child,
                &option.effects,
                block,
                &mut BTreeMap::new(),
                player_binding,
                context,
            )?;
            if !child.modes.is_empty() {
                return Err(unsupported("effects.modal", "nested modes unsupported"));
            }
            program.modes.push(SpellMode { program: child });
        }
        return Ok(());
    }
    if !program.modes.is_empty() {
        return Err(unsupported(
            "effects.modal",
            "mixed modal and ordinary root instructions",
        ));
    }
    if effects.is_empty() {
        return Err(unsupported("abilities.effects", "empty spell effects"));
    }
    let source_block = source_block(block)?;
    // CR 109.5, 601.2c, 608.2b/c, 611.2c: consume the complete ordinary
    // controlled-permanent grant, including its optional fixed controller tail.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    if let Some(Sentence { steps, .. }) = effects.first()
        && matches!(steps.first(), Some(Step::GainAbilities { what, .. })
            if (what["filter"]["cls"] == "permanent" || controller_library::artifact_creature_target(what)) && what["ref"] == "target")
    {
        if source_block != 0
            || player_binding != PlayerBinding::Unbound
            || program.functioning_zone != FunctioningZone::Stack
            || program.resolution != SpellResolution::Instructions
            || program.additional_sacrifice.is_some()
            || !program.targets.is_empty()
            || !program.instructions.is_empty()
            || !slots.is_empty()
            || !program.optional_additional_costs.is_empty()
            || effects.iter().any(|s| {
                !s.bindings.is_empty() || s.condition.is_some() || s.condition_kind.is_some()
            })
            || effects.len() > 2
        {
            return Err(unsupported(
                "effects",
                "unsupported complete permanent keyword context",
            ));
        }
        let [
            Step::GainAbilities {
                what,
                abilities,
                duration,
            },
        ] = steps.as_slice()
        else {
            return Err(unsupported(
                "effects",
                "permanent grant must occupy its whole sentence",
            ));
        };
        if duration.duration != "end-of-turn" {
            return Err(unsupported(
                "effects.duration",
                "permanent grant must end this turn",
            ));
        }
        let filtered = controller_library::artifact_creature_target(what);
        let target_slot = if filtered {
            controller_library::bind_grant_target(program, slots, what)?
        } else {
            bind_temporary_permanent_target(program, slots, what)?
        };
        let keywords = lower_temporary_creature_keywords(abilities)?;
        let tail = if effects.len() == 2 {
            if let [Step::Scry { amount }] = effects[1].steps.as_slice() {
                if !filtered {
                    return Err(unsupported(
                        "effects",
                        "scry tail requires artifact-or-creature family",
                    ));
                }
                Some(controller_library::instruction(amount, source_block)?)
            } else {
                if filtered {
                    return Err(unsupported(
                        "effects",
                        "artifact-or-creature grant only supports scry tail",
                    ));
                }
                let [
                    Step::GainLife {
                        who,
                        amount,
                        for_each: None,
                    },
                ] = effects[1].steps.as_slice()
                else {
                    return Err(unsupported(
                        "effects",
                        "only fixed controller life tail supported",
                    ));
                };
                Some(lower_fixed_controller_life(
                    who,
                    amount,
                    source_block,
                    context,
                )?)
            }
        } else {
            None
        };
        program.instructions.push(Instruction::GrantKeywords {
            target_slot,
            modifier: None,
            keywords,
            duration: Duration::UntilEndOfTurn,
            source_block,
        });
        program.instructions.extend(tail);
        return Ok(());
    }
    // CR 109.2/109.5, 608.2h, 611.2c: shared controller-set coalescing
    // consumes the complete sole pure-gain sentence. Definition/activation
    // closure separately fences its selector, source kind, cost and context.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    if source_block == 0
        && player_binding == PlayerBinding::Unbound
        && program.functioning_zone == FunctioningZone::Stack
        && program.resolution == SpellResolution::Instructions
        && program.additional_sacrifice.is_none()
        && program.optional_additional_costs.is_empty()
        && program.targets.is_empty()
        && program.instructions.is_empty()
        && let [
            Sentence {
                bindings,
                steps,
                condition: None,
                condition_kind: None,
            },
        ] = effects
        && bindings.is_empty()
        && let [
            Step::GainAbilities {
                what,
                abilities,
                duration,
            },
        ] = steps.as_slice()
        && matches!(what["ref"].as_str(), Some("all" | "each"))
        && duration.duration == "end-of-turn"
        && matches!(
            lower_object_set(what)?,
            ObjectSet::ControllerPermanents | ObjectSet::ControllerCreatures
        )
    {
        program.instructions.push(Instruction::GrantKeywordsSet {
            set: lower_object_set(what)?,
            modifier: None,
            keywords: lower_temporary_creature_keywords(abilities)?,
            duration: Duration::UntilEndOfTurn,
            source_block,
        });
        return Ok(());
    }
    // CR 608.2c/h, 611.2c, 613.5–6: consume the whole typed sentence as one
    // continuous effect, with one membership capture across all its layers.
    // Never join distinct sentences or scan arbitrary later instruction pairs.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    if source_block == 0
        && player_binding == PlayerBinding::Unbound
        && program.functioning_zone == FunctioningZone::Stack
        && program.resolution == SpellResolution::Instructions
        && program.additional_sacrifice.is_none()
        && program.targets.is_empty()
        && program.instructions.is_empty()
        && let [
            Sentence {
                condition: None,
                condition_kind: None,
                bindings,
                steps,
            },
        ] = effects
        && bindings.is_empty()
        && let [
            Step::Pump {
                what: pump_what,
                power,
                toughness,
                duration: pump_duration,
            },
            Step::GainAbilities {
                what,
                abilities,
                duration,
            },
        ] = steps.as_slice()
        && matches!(pump_what["ref"].as_str(), Some("all" | "each"))
        && matches!(what["ref"].as_str(), Some("all" | "each"))
        && pump_duration == duration
        && duration.duration == "end-of-turn"
    {
        let set = lower_object_set(what)?;
        if lower_object_set(pump_what)? != set
            || !matches!(
                set,
                ObjectSet::ControllerCreatures | ObjectSet::AttackingCreatures
            )
        {
            return Err(unsupported(
                "set",
                "compound set effect requires the same reviewed selector",
            ));
        }
        program.instructions.push(Instruction::GrantKeywordsSet {
            set,
            modifier: Some(CreatureStatModifier {
                power: signed_fixed(power)?,
                toughness: signed_fixed(toughness)?,
            }),
            keywords: lower_temporary_creature_keywords(abilities)?,
            duration: Duration::UntilEndOfTurn,
            source_block,
        });
        return Ok(());
    }
    // The targeted family likewise consumes the WHOLE sole typed sentence.
    // Binding both complete expressions preserves selection and domain checks.
    if source_block == 0
        && player_binding == PlayerBinding::Unbound
        && program.functioning_zone == FunctioningZone::Stack
        && program.resolution == SpellResolution::Instructions
        && program.additional_sacrifice.is_none()
        && program.targets.is_empty()
        && program.instructions.is_empty()
        && let [
            Sentence {
                condition: None,
                condition_kind: None,
                bindings,
                steps,
            },
        ] = effects
        && bindings.is_empty()
        && let [
            Step::Pump {
                what: pump_what,
                power,
                toughness,
                duration: pump_duration,
            },
            Step::GainAbilities {
                what,
                abilities,
                duration,
            },
        ] = steps.as_slice()
        && pump_duration == duration
        && duration.duration == "end-of-turn"
    {
        let target_slot = bind_temporary_creature_target(program, slots, pump_what)?;
        if bind_temporary_creature_target(program, slots, what)? != target_slot
            || target_slot != 0
            || program.targets.len() != 1
        {
            return Err(unsupported(
                "targets",
                "compound grant requires the same one target",
            ));
        }
        program.instructions.push(Instruction::GrantKeywords {
            target_slot,
            modifier: Some(CreatureStatModifier {
                power: signed_fixed(power)?,
                toughness: signed_fixed(toughness)?,
            }),
            keywords: lower_temporary_creature_keywords(abilities)?,
            duration: Duration::UntilEndOfTurn,
            source_block,
        });
        return Ok(());
    }
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
        if let Some(instruction) = typed_land_search::lower(&sentence.steps, source_block)? {
            program.instructions.push(instruction);
            continue;
        }
        // CR 701.20a-b, 701.23b, 701.24a/e (pinned 2026-09-25): consume
        // the entire reveal/identity partition, never squeeze it into all-entry search.
        // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
        if let [
            Step::Search { who, zones, find },
            Step::Reveal {
                who: revealer,
                what,
            },
            Step::Simultaneous { steps },
            Step::Shuffle {
                who: shuffler,
                library,
                optional: None,
            },
        ] = sentence.steps.as_slice()
        {
            let you = PlayerSelector {
                player: "you".into(),
                selection_id: None,
                of: None,
            };
            let steps: Vec<Step> = serde_json::from_value(serde_json::Value::Array(steps.clone()))
                .map_err(|_| {
                    unsupported(
                        "effects.simultaneous",
                        "requires complete supported movements",
                    )
                })?;
            let split = matches!(steps.as_slice(), [
                Step::MoveZone { what: first, to: battlefield, tapped: Some(true), order: None },
                Step::MoveZone { what: rest, to: hand, tapped: None, order: None }
            ] if *first == serde_json::json!({"ref":"from-pool","pool":{"ref":"them"},"count":{"amount":"fixed","value":1}})
                && *battlefield == serde_json::json!({"zone":"battlefield"})
                && *rest == serde_json::json!({"ref":"rest"})
                && *hand == serde_json::json!({"zone":"hand","owner":"your"}));
            if *who != you
                || revealer != who
                || shuffler != who
                || !split
                || *zones
                    != serde_json::json!({"selection":"all","of":[{"zone":"library","owner":"your"}]})
                || *find
                    != serde_json::json!({"ref":"filter","filter":{"supertypes":["basic"],"types":["land"],"cls":"card"},"count":{"amount":"fixed","value":2},"upTo":true})
                || *what != serde_json::json!({"ref":"that","filter":{"cls":"card"},"plural":true})
                || *library != serde_json::json!({"zone":"library","owner":{"player":"you"}})
            {
                return Err(unsupported(
                    "effects.search",
                    "unsupported complete search/reveal/split/shuffle sequence",
                ));
            }
            program
                .instructions
                .push(Instruction::SearchBasicLandsSplit { source_block });
            continue;
        }
        if let [
            Step::Search { who, zones, find },
            Step::MoveZone {
                what,
                to,
                tapped: Some(true),
                order: None,
            },
            Step::Shuffle {
                who: shuffler,
                library,
                optional: None,
            },
        ] = sentence.steps.as_slice()
        {
            let maximum = find["count"]["value"].as_u64().unwrap_or(0);
            let up_to = maximum == 2;
            let expected = if up_to {
                serde_json::json!({"ref":"filter","filter":{"supertypes":["basic"],"types":["land"],"cls":"card"},"count":{"amount":"fixed","value":2},"upTo":true})
            } else {
                serde_json::json!({"ref":"filter","filter":{"supertypes":["basic"],"types":["land"],"cls":"card"},"count":{"amount":"fixed","value":1}})
            };
            // This instruction searches and shuffles only its controller's
            // library. Consume the complete actor, including object qualifiers.
            let you = PlayerSelector {
                player: "you".into(),
                selection_id: None,
                of: None,
            };
            if *who != you
                || shuffler != who
                || *zones
                    != serde_json::json!({"selection":"all","of":[{"zone":"library","owner":"your"}]})
                || *find != expected
                || !(1..=2).contains(&maximum)
                || *what
                    != if up_to {
                        serde_json::json!({"ref":"them"})
                    } else {
                        serde_json::json!({"ref":"that","filter":{"cls":"card"}})
                    }
                || *to != serde_json::json!({"zone":"battlefield"})
                || *library != serde_json::json!({"zone":"library","owner":{"player":"you"}})
            {
                return Err(unsupported(
                    "effects.search",
                    "unsupported complete search/move/shuffle sequence",
                ));
            }
            program.instructions.push(Instruction::SearchBasicLands {
                maximum: maximum as u32,
                source_block,
            });
            continue;
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
            if let Some(i) = lower_wipes_step(
                step,
                counted,
                source_block,
                player_binding == PlayerBinding::DeathController,
                context,
            )? {
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
                Step::MoveZone {
                    what,
                    to,
                    tapped: None,
                    order: None,
                } if *to == serde_json::json!({"zone":"battlefield"}) => {
                    let id = graveyard_battlefield_return::creature_target(what)?;
                    Instruction::ReturnOwnGraveyardCreatureToBattlefield {
                        target_slot: bind_target(
                            program,
                            slots,
                            id,
                            TargetDomain::OwnGraveyardCreatureCard,
                        )?,
                        source_block,
                    }
                }
                Step::ReturnToHand { what, to, optional }
                    if to == "your"
                        && what["filter"]
                            == serde_json::json!({"cls":"card","zone":{"zone":"graveyard","owner":"your"}}) =>
                {
                    // Definition validation binds the complete raw selector, sentence and printed block.
                    let id = what["selectionId"].as_str().ok_or_else(|| {
                        unsupported("effects.return", "missing graveyard card target")
                    })?;
                    Instruction::ReturnOwnGraveyardCardToHand {
                        target_slot: bind_target(
                            program,
                            slots,
                            id,
                            TargetDomain::OwnGraveyardCard,
                        )?,
                        optional: optional.unwrap_or(false),
                        source_block,
                    }
                }
                Step::ReturnToHand { what, to, optional } if to == "your" => {
                    let id = graveyard_battlefield_return::creature_target(what)?;
                    Instruction::ReturnOwnGraveyardCreatureToHand {
                        target_slot: bind_target(
                            program,
                            slots,
                            id,
                            TargetDomain::OwnGraveyardCreatureCard,
                        )?,
                        optional: optional.unwrap_or(false),
                        source_block,
                    }
                }
                Step::CreateToken {
                    count,
                    token,
                    who: Some(who),
                } => Instruction::CreateTokensFor {
                    recipient: removal_recipient(program, who)?,
                    count: if count.amount == "fixed" && count.value > 0 {
                        count.value
                    } else {
                        return Err(unsupported("token", "unsupported token count"));
                    },
                    token: lower_creature_token(token)?,
                    source_block,
                },
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
                Step::PutCounters {
                    on: SourceCounterRecipient::Source(r),
                    counter,
                    count,
                } if r.reference == "this"
                    && r.noun == "creature"
                    && counter.r#type == "+1/+1"
                    && *count
                        == serde_json::json!({"amount":"product","factors":[
                            {"amount":"fixed","value":1},
                            {"amount":"distinct-count","characteristic":"color","among":{"ref":"that","filter":{"cls":"spell"}}}
                        ]}) =>
                {
                    Instruction::PutSourcePlusOneCountersForCastColors { source_block }
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
                Step::GainAbilities {
                    what,
                    abilities,
                    duration,
                } if duration.duration == "end-of-turn" => {
                    let keywords = lower_temporary_creature_keywords(abilities)?;
                    Instruction::GrantKeywords {
                        modifier: None,
                        target_slot: bind_temporary_creature_target(program, slots, what)?,
                        keywords,
                        duration: Duration::UntilEndOfTurn,
                        source_block,
                    }
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
                Step::PreventDamage {
                    prevention,
                    duration,
                } => lower_prevention(program, slots, prevention, duration, source_block)?,
                Step::Damage {
                    source,
                    amount,
                    to,
                    optional,
                } if !matches!(source, DamageSource::SelfReference {})
                    || optional.is_some()
                    || amount["amount"] != "fixed" =>
                {
                    lower_sacrifice_damage(
                        program,
                        slots,
                        source,
                        amount,
                        to,
                        *optional,
                        source_block,
                    )?
                }
                Step::Damage {
                    optional: None,
                    source: DamageSource::SelfReference {},
                    amount,
                    to: DamageDestination::Player { reference },
                } => {
                    let EffectRecipient::TargetController { target_slot } =
                        removal_recipient(program, reference)?
                    else {
                        return Err(unsupported("damage", "requires target controller"));
                    };
                    Instruction::DamageTargetController {
                        target_slot,
                        amount: fixed_positive_value(amount)?,
                        source_block,
                    }
                }
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
                Step::GainLife { who, amount, .. } | Step::LoseLife { who, amount }
                    if who.of.is_some() || amount["amount"] == "attribute" =>
                {
                    let recipient = removal_recipient(
                        program,
                        &serde_json::to_value(who).map_err(|_| invalid("life", "recipient"))?,
                    )?;
                    Instruction::ChangeLifeFromTarget {
                        recipient,
                        amount: removal_amount(program, amount)?,
                        gain: matches!(step, Step::GainLife { .. }),
                        source_block,
                    }
                }
                Step::Scry { amount } => controller_library::instruction(amount, source_block)?,
                Step::Surveil { amount } => {
                    controller_library::surveil_instruction(amount, source_block)?
                }
                Step::Mill { who, amount } => {
                    fixed_mill::instruction(program, slots, who, amount, context, source_block)?
                }
                Step::GainLife { who, amount, .. } => Instruction::act(
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
                    amount,
                    optional: true,
                } => {
                    if who.player != "you"
                        || who.selection_id.is_some()
                        || who.of.is_some()
                        || !amount.fixed_one()
                    {
                        return Err(unsupported(
                            "effects.draw",
                            "optional draw requires one card for the unqualified controller",
                        ));
                    }
                    Instruction::OptionalDrawCards { source_block }
                }
                Step::Draw {
                    who,
                    amount: DrawAmount::ThatMuch {},
                    optional: false,
                } if player_binding == PlayerBinding::DamageRecipient
                    && who.player == "you"
                    && who.selection_id.is_none()
                    && who.of.is_none() =>
                {
                    // CR 608.2c/608.2h: bind this quantity to the actual triggering
                    // damage event, not to current power, life change or another effect.
                    // The archive's reference graph remains diagnostic/nonexecutable.
                    Instruction::DrawDamageAmount { source_block }
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
                Step::Counter { what } if what.reference == "target" && fixed_one(&what.count) => {
                    let domain = match &what.filter {
                        CounterFilter::Spell { .. } => TargetDomain::StackSpell,
                        CounterFilter::Ability { any_of }
                            if any_of.len() == 2
                                && any_of
                                    .iter()
                                    .map(|filter| filter.ability_kind.as_str())
                                    .collect::<BTreeSet<_>>()
                                    == BTreeSet::from(["activated", "triggered"]) =>
                        {
                            TargetDomain::StackActivatedOrTriggeredAbility
                        }
                        _ => {
                            return Err(unsupported(
                                "targets",
                                "counter requires the complete spell or activated-or-triggered domain",
                            ));
                        }
                    };
                    let slot = bind_target(program, slots, &what.selection_id, domain)?;
                    if let CounterFilter::Spell {
                        all_of: Some(all_of),
                    } = &what.filter
                    {
                        let f = lower_permanent_filter(&serde_json::json!({"allOf":all_of}))?;
                        if f.types.is_empty()
                            || !f.non_types.is_empty()
                            || !f.non_colors.is_empty()
                            || f.opponent
                        {
                            return Err(unsupported("counter", "unsupported typed spell filter"));
                        }
                        if program.targets[slot as usize]
                            .filter
                            .as_ref()
                            .is_some_and(|old| old != &f)
                        {
                            return Err(unsupported("counter", "conflicting target filter"));
                        }
                        program.targets[slot as usize].filter = Some(f);
                    }
                    Instruction::CounterStackObject {
                        target_slot: slot,
                        source_block,
                    }
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
    death_controller: bool,
    context: Context,
) -> Result<Option<Instruction>, Failure> {
    use serde_json::json;
    Ok(Some(match step {
        Step::GainAbilities {
            what,
            abilities,
            duration,
        } if (what["ref"] == "all" || what["ref"] == "each")
            && duration.duration == "end-of-turn" =>
        {
            let set = lower_object_set(what)?;
            if set == ObjectSet::ControllerPermanents {
                return Err(unsupported(
                    "set",
                    "permanent keywords require a complete ordinary pure gain",
                ));
            }
            Instruction::GrantKeywordsSet {
                set,
                modifier: None,
                keywords: lower_temporary_creature_keywords(abilities)?,
                duration: Duration::UntilEndOfTurn,
                source_block,
            }
        }
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
            who,
            amount,
            for_each: Some(f),
        } if who.player == "you"
            && who.selection_id.is_none()
            && who.of.is_none()
            && *amount == json!({"amount":"fixed","value":1})
            && *f
                == json!({"types":["creature"],"relation":{"relation":"event-result","event":"destroy","scope":"this-way"}}) =>
        {
            Instruction::GainLifeDestroyed { source_block }
        }
        Step::GainLife {
            for_each: Some(_), ..
        } => return Err(unsupported("forEach", "unsupported event result")),
        Step::LoseLife { who, amount }
            if death_controller
                && who.player == "that-player"
                && who.selection_id.is_none()
                && who.of.is_none() =>
        {
            Instruction::LoseLifeDeathController {
                amount: fixed_positive_value(amount)?,
                source_block,
            }
        }
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
        Step::CreateReplacement {
            replacement,
            duration,
        } if duration.duration == "this-turn"
            && *replacement
                == json!({"event":{"event":"move-zone","what":{"ref":"filter","filter":{"types":["creature"],"damageHistory":{"role":"recipient","window":{"event":"this-way"}}},"count":{"amount":"fixed","value":1}},"from":{"zone":"battlefield"},"to":{"zone":"graveyard"}},"occurrence":"each","replace":{"replacement":"effects","effects":[{"steps":[{"effect":"exile","what":{"ref":"replacement-event-object","role":"affected"}}]}]}}) =>
        {
            Instruction::ExileDamagedThisTurn { source_block }
        }
        _ => return Ok(None),
    }))
}

pub(crate) fn lower_sacrifice_damage(
    p: &mut SpellProgram,
    slots: &mut BTreeMap<String, u32>,
    source: &DamageSource,
    amount: &serde_json::Value,
    to: &DamageDestination,
    optional: Option<bool>,
    source_block: u32,
) -> Result<Instruction, Failure> {
    if !(matches!(source, DamageSource::SelfReference {} | DamageSource::It {})
        || matches!(source, DamageSource::This { noun } if noun == "creature" || noun == "enchantment"))
    {
        return Err(unsupported("damage", "unsupported complete damage source"));
    }
    let amount = if *amount
        == serde_json::json!({"amount":"attribute","of":{"ref":"related","number":"singular","filter":{"types":["creature"]},"relation":{"relation":"event-result","event":"sacrifice","scope":"contextual"}},"attribute":"power"})
    {
        if !matches!(source, DamageSource::SelfReference {}) || p.additional_sacrifice.is_none() {
            return Err(unsupported("damage", "unbound sacrificed power"));
        }
        SacrificeDamageAmount::SacrificedPower
    } else {
        SacrificeDamageAmount::Fixed {
            value: fixed_positive_value(amount)?,
        }
    };
    let (target_slot, recipient) = match to {
        DamageDestination::AnyTarget { selection_id } => (
            Some(bind_target(
                p,
                slots,
                selection_id,
                TargetDomain::AnyDamageTarget,
            )?),
            None,
        ),
        DamageDestination::Player { reference }
            if *reference == serde_json::json!({"player":"you"}) =>
        {
            (None, Some(LifeRecipient::Controller))
        }
        DamageDestination::Player { reference }
            if *reference == serde_json::json!({"player":"each-opponent"}) =>
        {
            (None, Some(LifeRecipient::EachOpponent))
        }
        _ => {
            return Err(unsupported(
                "damage",
                "unsupported complete damage recipient",
            ));
        }
    };
    Ok(Instruction::SacrificeDamage {
        target_slot,
        recipient,
        amount,
        optional: optional.unwrap_or(false),
        source_block,
    })
}

pub(crate) fn lower_fixed_controller_life(
    who: &PlayerSelector,
    amount: &serde_json::Value,
    source_block: u32,
    context: Context,
) -> Result<Instruction, Failure> {
    if who.player != "you" || who.selection_id.is_some() || who.of.is_some() {
        return Err(unsupported(
            "life",
            "requires unqualified invocation controller",
        ));
    }
    let count: Count = serde_json::from_value(amount.clone())
        .map_err(|_| unsupported("life", "requires complete fixed nonnegative life amount"))?;
    if count.amount != "fixed" {
        return Err(unsupported("life", "requires fixed life amount"));
    }
    Ok(Instruction::act(
        Verb::GainLife {
            player: PlayerExpr::you(context),
            amount: Amount::Fixed {
                value: Quantity::from(i64::from(count.value)),
            },
        },
        source_block,
    ))
}
