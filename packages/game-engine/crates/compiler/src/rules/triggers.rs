//! Triggered abilities and their event subjects (CR 603).
use super::*;

// CR 120.2b/608.2h: the object specified by the instruction deals damage.
// An event/cost pronoun is not automatically this invocation's physical source.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
#[derive(Clone, Copy)]
pub(crate) enum DamageSourceBinding {
    Spell,
    Invocation { first_it_is_self: bool },
}

pub(crate) fn validate_damage_sources(
    effects: &[Sentence],
    binding: DamageSourceBinding,
) -> Result<(), Failure> {
    fn bound(effects: &[Sentence], binding: DamageSourceBinding, top_level: bool) -> bool {
        for (sentence_index, sentence) in effects.iter().enumerate() {
            for (step_index, step) in sentence.steps.iter().enumerate() {
                match step {
                    Step::Damage { source, .. } => match binding {
                        DamageSourceBinding::Spell
                            if !matches!(source, DamageSource::SelfReference {}) =>
                        {
                            return false;
                        }
                        DamageSourceBinding::Invocation { first_it_is_self }
                            if matches!(source, DamageSource::It {})
                                && (!first_it_is_self
                                    || !top_level
                                    || sentence_index != 0
                                    || step_index != 0
                                    || !sentence.bindings.is_empty()) =>
                        {
                            return false;
                        }
                        _ => {}
                    },
                    Step::Modal { options, .. }
                        if options
                            .iter()
                            .any(|option| !bound(&option.effects, binding, false)) =>
                    {
                        return false;
                    }
                    _ => {}
                }
            }
        }
        true
    }
    if !bound(effects, binding, true) {
        return Err(match binding {
            DamageSourceBinding::Spell => unsupported(
                "damage.source",
                "a spell must refer to its own spell source",
            ),
            DamageSourceBinding::Invocation { .. } => unsupported(
                "abilities.effects.source",
                "pronoun damage source requires a first-step proven invocation source",
            ),
        });
    }
    Ok(())
}

pub(crate) fn validate_trigger_damage_sources(
    condition: &TriggerCondition,
    effects: &[Sentence],
) -> Result<(), Failure> {
    let first_it_is_self = matches!(
        condition,
        TriggerCondition::SelfDies
            | TriggerCondition::SelfEntersBattlefield
            | TriggerCondition::CreatureAttacked {
                attacker: AttackSubject::Source {}
            }
            | TriggerCondition::DamageDealt {
                source: DamageTriggerSource::Source {},
                ..
            }
    );
    validate_damage_sources(
        effects,
        DamageSourceBinding::Invocation { first_it_is_self },
    )
}

pub(crate) fn lower_trigger(
    trigger: &SourceTrigger,
    effects: &[Sentence],
    intervening: Option<&SourceCondition>,
    block: usize,
) -> Result<TriggeredAbility, Failure> {
    if let Some(ability) =
        entry_graveyard_exile::lower_trigger(trigger, effects, intervening, block)
    {
        return Ok(ability);
    }
    if let Some(ability) = controlled_land_entry::lower(trigger, effects, intervening, block)? {
        return Ok(ability);
    }
    if land_entry_scry::matches(trigger, effects, intervening, block) {
        return Ok(land_entry_scry_trigger());
    }
    if let Some(ability) = land_entry_gain_life::lower(trigger, effects, intervening, block)? {
        return Ok(ability);
    }
    if let SourceTrigger::PutIntoGraveyard { what, from, to } = trigger {
        if matches!(what, TriggerObject::This { noun } if noun == "creature") {
            return successor_tuck::lower(effects, intervening, from, to, source_block(block)?);
        }
        // This shortcut emits no binding instructions; unconsumed sentence
        // bindings must not disappear when bypassing append_spell_effects.
        if !matches!(what, TriggerObject::This {noun} if noun == "aura")
            || from != "battlefield"
            || *to != serde_json::json!({"zone":"graveyard","owner":"any"})
            || intervening.is_some()
            || !matches!(effects, [Sentence {bindings, steps, condition: None, condition_kind: None}] if bindings.is_empty() && matches!(steps.as_slice(), [Step::ReturnToHand {what,to,optional:None}] if *what == serde_json::json!({"ref":"it"}) && to == "owner"))
        {
            return Err(unsupported("trigger", "unsupported Aura return"));
        }
        return Ok(TriggeredAbility {
            death_filter: None,
            functioning_zone: FunctioningZone::Battlefield,
            condition: TriggerCondition::SelfAuraToGraveyard,
            intervening_if: None,
            program: SpellProgram {
                optional_additional_costs: vec![],
                additional_sacrifice: None,
                timing: SpellTiming::Instant,
                resolution: SpellResolution::Instructions,
                functioning_zone: FunctioningZone::Stack,
                targets: vec![],
                instructions: vec![Instruction::ReturnAuraFromGraveyard {
                    source_block: block as u32,
                }],
                modes: vec![],
            },
        });
    }
    // The archive reference-binding object is diagnostic/nonexecutable. This
    // Rust semantic binder consumes the complete typed AST instead: "other"
    // excludes this exact source incarnation (CR 201.5), and the exact disjoint
    // union [this creature dies OR another creature dies] is every creature
    // death, once per actual move. It does not bind an unused event pronoun.
    // ETB checks the resulting battlefield; dies checks the prior battlefield
    // (CR 603.6a/603.10/700.4). Runtime captures those event/source facts.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    let filtered = |what: &TriggerObject| -> Option<DeathFilter> {
        match what {
            TriggerObject::Filter { filter: f, count }
                if fixed_one(count)
                    && f.properties.is_none()
                    && f.types == ["creature"]
                    && f.control.as_deref() == Some("you")
                    && f.non_subtypes.iter().all(|s| s == "Spirit")
                    && f.non_subtypes.len() <= 1 =>
            {
                Some(DeathFilter {
                    controlled: true,
                    other: f.other,
                    non_token: f.non_token,
                    non_subtypes: f.non_subtypes.clone(),
                })
            }
            _ => None,
        }
    };
    let death_filter = match trigger {
        SourceTrigger::Dies { what } => filtered(what),
        SourceTrigger::AnyOf { events } if matches!(events.as_slice(), [SourceTrigger::Dies { what }, SourceTrigger::Dies { .. }] if is_source_creature(what)) =>
        {
            let SourceTrigger::Dies { what } = &events[1] else {
                unreachable!()
            };
            if let Some(mut f) = filtered(what) {
                if !f.other || f.non_token || !f.non_subtypes.is_empty() {
                    return Err(unsupported("trigger", "non-disjoint dies union"));
                }
                f.other = false;
                Some(f)
            } else {
                None
            }
        }
        _ => None,
    };
    fn has_draw_quantity(effects: &[Sentence]) -> bool {
        effects
            .iter()
            .flat_map(|s| &s.steps)
            .any(|step| match step {
                Step::Draw {
                    amount: DrawAmount::ThatMuch {},
                    ..
                } => true,
                Step::Modal { options, .. } => {
                    options.iter().any(|o| has_draw_quantity(&o.effects))
                }
                _ => false,
            })
    }
    if has_draw_quantity(effects)
        && !matches!(effects, [Sentence {bindings, steps, condition: None, condition_kind: None}] if bindings.is_empty()
            && matches!(steps.as_slice(), [Step::Draw {amount: DrawAmount::ThatMuch {}, ..}]))
    {
        // Prior effects may produce another quantity. This finite binder only
        // proves the standalone trigger-quantity reference; it never guesses
        // the antecedent of "that much" in an arbitrary instruction sequence.
        return Err(unsupported(
            "effects.draw",
            "damage quantity requires a standalone draw sentence",
        ));
    }
    let condition=match trigger {
        SourceTrigger::DealsDamage { what:SourceDamageObject::Filter {filter,count},combat:Some(true),to,grouping:Some(grouping) }
            if grouping == "one-or-more" && *to == serde_json::json!("any")
                && fixed_one(count) && filter.types == ["creature"]
                && filter.control.as_deref() == Some("you") && filter.non_types.is_none()
                && filter.all_of.is_none() && filter.any_of.is_none()
                && filter.properties.as_deref() == Some(&[serde_json::json!({"property":"power","comparison":{"op":"gt","value":{"amount":"attribute","of":{"ref":"it"},"attribute":"base-power"}}})]) => {
            // CR 208.4b/603.2c/603.10: the property's "it" is EACH candidate
            // creature, not the observing source. Diagnostic references remain
            // unchanged even when their earlier binder chose another object.
            TriggerCondition::GroupedDamageDealt {
                source:DamageTriggerSource::CreatureFilter {controller:DamageSourceRelation::Controller,types:TypePredicate::Type {kind:CardKind::Creature}},
                kind:TriggerDamageKind::Combat,recipient:DamageTriggerRecipient::Player,source_power:DamageSourcePowerPredicate::GreaterThanBase,
            }
        },
        SourceTrigger::DealsDamage { what, combat: Some(true), to, grouping:None }
            if *to == serde_json::json!("any") => {
            TriggerCondition::DamageDealt { source: lower_damage_subject(what)?,
                kind: TriggerDamageKind::Combat, recipient: DamageTriggerRecipient::Player }
        },
        SourceTrigger::DealsDamage { what, combat: None, to, grouping:None }
            if *to == serde_json::json!({"player":"an-opponent"}) => {
            TriggerCondition::DamageDealt { source: lower_damage_subject(what)?,
                kind: TriggerDamageKind::Any, recipient: DamageTriggerRecipient::OpponentPlayer }
        },
        SourceTrigger::Attacks { what } => {
            let attacker = lower_attack_subject(what)?;
            TriggerCondition::CreatureAttacked { attacker }
        },
        SourceTrigger::Cast { who, what, not_from } => {
            if who.selection_id.is_some() || who.of.is_some() { return Err(unsupported("trigger.who", "unsupported caster qualifier")); }
            let caster = match who.player.as_str() {
                "you" => CasterRelation::Controller,
                "a-player" => CasterRelation::AnyPlayer,
                "an-opponent" => CasterRelation::Opponent,
                _ => return Err(unsupported("trigger.who", "unsupported caster relation")),
            };
            let spell = lower_type_predicate(what, Some("spell"), true, 0)?;
            match not_from {
                None => TriggerCondition::SpellCast { caster, spell },
                Some(zone) => cast_origin::lower_condition(caster, spell, zone)?,
            }
        },
        SourceTrigger::Phase { phase, whose } => {
            let step = match phase.as_str() {
                "upkeep" => BeginningStep::Upkeep,
                "draw-step" => BeginningStep::Draw,
                "end-step" => BeginningStep::End,
                _ => return Err(unsupported("abilities.trigger.phase", "unsupported beginning step")),
            };
            let turn_relation = match whose.as_str() {
                Some("your") => TurnRelation::Controller,
                Some("each-player") => TurnRelation::AnyPlayer,
                Some("each-opponent") => TurnRelation::Opponent,
                _ => return Err(unsupported("abilities.trigger.whose", "unsupported turn relation")),
            };
            TriggerCondition::StepBegan { step, turn_relation }
        },
        SourceTrigger::Dies { what: TriggerObject::Filter { filter, count } } if fixed_one(count) && filter.properties.is_none() && filter.types == ["creature"] && filter.control.as_deref() == Some("opponent") && !filter.other && !filter.non_token && filter.non_subtypes.is_empty() => TriggerCondition::OpponentCreatureDies,
        _ if death_filter.is_some() => TriggerCondition::AnyCreatureDies,
        SourceTrigger::Enters { what: TriggerObject::Filter { filter, count } } if filter.properties.is_some() => {
            TriggerCondition::ControlledCreatureEntersWithPower {
                minimum_power: power_entry_draw::minimum_power(filter, count, true)?,
                other: filter.other,
            }
        },
        SourceTrigger::Enters { what: TriggerObject::This { noun } } if noun == "enchantment" => TriggerCondition::SelfEntersBattlefield,
        SourceTrigger::Dies {what} if is_source_creature(what)=>TriggerCondition::SelfDies,
        SourceTrigger::Enters {what} if is_source_creature(what)=>TriggerCondition::SelfEntersBattlefield,
        SourceTrigger::Enters {what} if is_other_creature(what)=>TriggerCondition::OtherCreatureEntersBattlefield,
        SourceTrigger::AnyOf {events} if events.len()==2
            && events.iter().filter(|event|matches!(event,SourceTrigger::Dies {what} if is_source_creature(what))).count()==1
            && events.iter().filter(|event|matches!(event,SourceTrigger::Dies {what} if is_other_creature(what))).count()==1 =>TriggerCondition::AnyCreatureDies,
        _=>return Err(unsupported("abilities.trigger","unsupported complete event/source/other/alternative trigger binding")),
    };
    validate_trigger_damage_sources(&condition, effects)?;
    if matches!(condition, TriggerCondition::GroupedDamageDealt { .. })
        && (intervening.is_some()
            || !matches!(effects, [Sentence {bindings, steps, condition: None, condition_kind: None}] if bindings.is_empty()
            && matches!(steps.as_slice(), [Step::Draw {who,amount:DrawAmount::Fixed {value:1},optional:false}]
                if who.player == "you" && who.of.is_none() && who.selection_id.is_none())))
    {
        return Err(unsupported(
            "effects.grouped_damage",
            "grouped damage requires one standalone mandatory controller draw",
        ));
    }
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
    if let Some(instruction) = source_dies_nonland_destroy::instruction(
        &condition,
        effects,
        intervening,
        source_block(block)?,
    ) {
        program.instructions.push(instruction);
    } else if condition == TriggerCondition::SelfEntersBattlefield
        && matches!(intervening, Some(SourceCondition::WasCast { .. }))
        && let [
            Sentence {
                bindings,
                steps,
                condition: None,
                condition_kind: None,
            },
        ] = effects
        && bindings.is_empty()
        && let [Step::Untap { what }] = steps.as_slice()
        && *what == serde_json::json!({"ref":"all","filter":{"types":["land"],"control":"you"}})
    {
        program
            .instructions
            .push(Instruction::untap_lands_you_control(
                kir::Context::Triggered,
                source_block(block)?,
            ));
    } else if let Some(instruction) = controller_library::lower_cast_opponent_damage(
        &condition,
        effects,
        source_block(block)?,
        &mut program,
    )? {
        program.instructions.push(instruction);
    } else if let (
        SourceTrigger::Enters { what: subject },
        [
            Sentence {
                bindings,
                steps,
                condition: None,
                condition_kind: None,
            },
        ],
    ) = (trigger, effects)
        && is_source_creature(subject)
        && intervening.is_none()
        && bindings.is_empty()
        && let [Step::ReturnToHand { what, to, optional }] = steps.as_slice()
        && to == "owner"
    {
        let slot = bind_permanent_target(&mut program, &mut BTreeMap::new(), what)?;
        program
            .instructions
            .push(Instruction::ReturnEntryTargetCreature {
                target_slot: slot,
                optional: optional.unwrap_or(false),
                source_block: source_block(block)?,
            });
    } else {
        append_spell_effects(
            &mut program,
            effects,
            block,
            &mut BTreeMap::new(),
            if let Some(role) = kir::event_player_role(&condition) {
                PlayerBinding::EventPlayer(role)
            } else if matches!(condition, TriggerCondition::DamageDealt { .. }) {
                PlayerBinding::DamageRecipient
            } else if condition == TriggerCondition::OpponentCreatureDies {
                PlayerBinding::DeathController
            } else {
                PlayerBinding::Unbound
            },
            Context::Triggered,
        )?;
    }
    let intervening_if = match intervening {
        None => None,
        Some(SourceCondition::WasCast { what, by })
            if condition == TriggerCondition::SelfEntersBattlefield
                && *what == serde_json::json!({"ref":"it"})
                && by.player == "you"
                && by.of.is_none()
                && by.selection_id.is_none() =>
        {
            Some(InterveningCondition::SourceCastByController)
        }
        Some(SourceCondition::Control {
            who,
            what: TriggerObject::Filter { filter, count },
        }) if matches!(trigger, SourceTrigger::Enters { what: TriggerObject::This { noun } } if noun == "enchantment")
            && who.player == "you" =>
        {
            Some(InterveningCondition::ControllerHasCreatureWithPower {
                minimum_power: power_entry_draw::minimum_power(filter, count, false)?,
            })
        }
        Some(SourceCondition::CardsInHand { who, comparison })
            if (condition == TriggerCondition::SelfDies
                || matches!(condition, TriggerCondition::StepBegan { .. }))
                && who.player == "you"
                && who.selection_id.is_none()
                && who.of.is_none()
                && comparison.op == "eq"
                && comparison.value.amount == "fixed"
                && comparison.value.value == 0 =>
        {
            Some(InterveningCondition::ControllerHandEmpty)
        }
        Some(SourceCondition::Status {
            what: TriggerObject::This { noun },
            status,
        }) if matches!(condition, TriggerCondition::StepBegan { .. })
            && matches!(
                noun.as_str(),
                "artifact" | "creature" | "enchantment" | "permanent"
            ) =>
        {
            Some(InterveningCondition::SourceStatus { status: *status })
        }
        _ => {
            return Err(unsupported(
                "abilities.condition",
                "unsupported complete intervening-if condition",
            ));
        }
    };
    Ok(TriggeredAbility {
        death_filter,
        intervening_if,
        functioning_zone: FunctioningZone::Battlefield,
        condition,
        program,
    })
}
