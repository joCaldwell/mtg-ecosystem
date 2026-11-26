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
    // The events whose first-step pronoun "it" is the source itself.
    let pattern = condition.pattern();
    let first_it_is_self = *pattern == EventPattern::self_dies()
        || matches!(pattern, EventPattern::Dies { object } if *object == Pred::source())
        || *pattern == EventPattern::self_enters()
        || matches!(pattern, EventPattern::Attacks { attacker } if *attacker == Pred::source())
        || matches!(pattern, EventPattern::DealsDamage { source, .. } if *source == Pred::source());
    validate_damage_sources(
        effects,
        DamageSourceBinding::Invocation { first_it_is_self },
    )
}

/// CR 700.4, 603.10a: a filtered creature death the reviewed row admits:
/// a creature you control, optionally another, nontoken or non-Spirit one.
fn creature_death(what: &TriggerObject) -> Option<(Pred, bool)> {
    match what {
        TriggerObject::Filter { filter: f, count }
            if fixed_one(count)
                && f.properties.is_none()
                && f.types == ["creature"]
                && f.control.as_deref() == Some("you")
                && f.non_subtypes.iter().all(|s| s == "Spirit")
                && f.non_subtypes.len() <= 1 =>
        {
            let mut atoms = vec![Atom::Type {
                kind: CardKind::Creature,
            }];
            atoms.extend(
                f.non_subtypes
                    .iter()
                    .map(|s| Atom::NotSubtype { subtype: s.clone() }),
            );
            atoms.push(Atom::ControlledBy {
                player: PlayerExpr::you(Context::Triggered),
            });
            if f.non_token {
                atoms.push(Atom::NotToken);
            }
            Some((Pred(atoms), f.other))
        }
        _ => None,
    }
}

pub(crate) fn cast_event(
    who: &PlayerSelector,
    what: &SourceTypeFilter,
    not_from: Option<&serde_json::Value>,
) -> Result<EventPattern, Failure> {
    if who.selection_id.is_some() || who.of.is_some() {
        return Err(unsupported("trigger.who", "unsupported caster qualifier"));
    }
    let caster = match who.player.as_str() {
        "you" => PlayerExpr::you(Context::Triggered),
        "a-player" => PlayerExpr::EachPlayer,
        "an-opponent" => PlayerExpr::EachOpponent,
        _ => return Err(unsupported("trigger.who", "unsupported caster relation")),
    };
    if not_from.is_some() {
        return Err(unsupported(
            "trigger.cast.origin",
            "unsupported cast-origin predicate",
        ));
    }
    Ok(EventPattern::Cast {
        caster,
        spell: lower_type_predicate(what, Some("spell"), true, 0)?,
    })
}

pub(crate) fn trigger_player_binding(condition: &TriggerCondition) -> PlayerBinding {
    if let Some(role) = kir::event_player_role(condition) {
        PlayerBinding::EventPlayer(role)
    } else if matches!(condition.pattern(), EventPattern::DealsDamage { .. }) {
        PlayerBinding::DamageRecipient
    } else if *condition.pattern() == EventPattern::opponent_creature_dies() {
        PlayerBinding::DeathController
    } else {
        PlayerBinding::Unbound
    }
}

pub(crate) fn lower_trigger(
    trigger: &SourceTrigger,
    effects: &[Sentence],
    intervening: Option<&SourceCondition>,
    block: usize,
) -> Result<TriggeredAbility, Failure> {
    if let Some(ability) = land_entry_gain_life::lower(trigger, effects, intervening, block)? {
        return Ok(ability);
    }
    // The archive reference-binding object is diagnostic/nonexecutable. This
    // Rust semantic binder consumes the complete typed AST instead: "other"
    // excludes this exact source incarnation (CR 201.5), and the exact disjoint
    // union [this creature dies OR another creature dies] is every creature
    // death, once per actual move. It does not bind an unused event pronoun.
    // ETB checks the resulting battlefield; dies checks the prior battlefield
    // (CR 603.6a/603.10/700.4). Runtime captures those event/source facts.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    let creature_dies = match trigger {
        SourceTrigger::Dies { what } => creature_death(what).map(|(mut pred, other)| {
            if other {
                pred.0.push(Atom::IsNot {
                    object: Ref::Source,
                });
            }
            pred
        }),
        SourceTrigger::AnyOf { events } if matches!(events.as_slice(), [SourceTrigger::Dies { what }, SourceTrigger::Dies { .. }] if is_source_creature(what)) =>
        {
            let SourceTrigger::Dies { what } = &events[1] else {
                unreachable!()
            };
            match creature_death(what) {
                // This creature or another creature you control: every
                // creature you control.
                Some((pred, true)) if pred.0.len() == 2 => Some(pred),
                Some(_) => return Err(unsupported("trigger", "non-disjoint dies union")),
                None => None,
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
        && !matches!(effects, [Sentence {bindings, steps, condition: None, condition_kind: None, ..}] if bindings.is_empty()
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
    let you = || PlayerExpr::you(Context::Triggered);
    let creature = || {
        Pred(vec![Atom::Type {
            kind: CardKind::Creature,
        }])
    };
    let pattern=match trigger {
        SourceTrigger::DealsDamage { what, combat: Some(true), to, grouping:None }
            if *to == serde_json::json!("any") => {
            EventPattern::DealsDamage { source: lower_damage_subject(what)?,
                combat: true, recipient: PlayerExpr::EachPlayer }
        },
        SourceTrigger::DealsDamage { what, combat: None, to, grouping:None }
            if *to == serde_json::json!({"player":"an-opponent"}) => {
            EventPattern::DealsDamage { source: lower_damage_subject(what)?,
                combat: false, recipient: PlayerExpr::EachOpponent }
        },
        SourceTrigger::Attacks { what } => {
            EventPattern::Attacks { attacker: lower_attack_subject(what)? }
        },
        SourceTrigger::Cast { who, what, not_from } => cast_event(who, what, not_from.as_ref())?,
        SourceTrigger::AttacksAndIsntBlocked { what } => {
            EventPattern::AttacksUnblocked { attacker: lower_attack_subject(what)? }
        },
        SourceTrigger::Draws { who } => {
            let player = match (who.player.as_str(), &who.selection_id, &who.of) {
                ("you", None, None) => you(),
                ("an-opponent", None, None) => PlayerExpr::EachOpponent,
                ("a-player", None, None) => PlayerExpr::EachPlayer,
                _ => return Err(unsupported("trigger.who", "unsupported drawing player")),
            };
            EventPattern::Draws { player }
        },
        SourceTrigger::Phase { phase, whose } => {
            let step = match phase.as_str() {
                "upkeep" => BeginningStep::Upkeep,
                "draw-step" => BeginningStep::Draw,
                "end-step" => BeginningStep::End,
                // CR 507.1, 511.1: the beginning and end of combat steps.
                "combat" => BeginningStep::BeginningOfCombat,
                "end-of-combat" => BeginningStep::EndOfCombat,
                // CR 505.1a: the main phase before combat, and the one after
                // the turn's only combat phase.
                "precombat-main" => BeginningStep::PrecombatMain,
                "postcombat-main" | "second-main" => BeginningStep::PostcombatMain,
                _ => return Err(unsupported("abilities.trigger.phase", "unsupported beginning step")),
            };
            let active = match whose.as_str() {
                Some("your") => you(),
                Some("each-player") => PlayerExpr::EachPlayer,
                Some("each-opponent") => PlayerExpr::EachOpponent,
                _ => return Err(unsupported("abilities.trigger.whose", "unsupported turn relation")),
            };
            EventPattern::StepBegins { step, active }
        },
        SourceTrigger::Dies { what: TriggerObject::Filter { filter, count } } if fixed_one(count) && filter.properties.is_none() && filter.types == ["creature"] && filter.control.as_deref() == Some("opponent") && !filter.other && !filter.non_token && filter.non_subtypes.is_empty() => EventPattern::opponent_creature_dies(),
        _ if creature_dies.is_some() => EventPattern::Dies { object: creature_dies.clone().unwrap_or_default() },
        SourceTrigger::Enters { what: TriggerObject::This { noun } } if noun == "enchantment" => EventPattern::self_enters(),
        SourceTrigger::Dies {what} if is_source_creature(what)=>EventPattern::self_dies(),
        SourceTrigger::Enters {what} if is_source_creature(what)=>EventPattern::self_enters(),
        SourceTrigger::Enters {what} if is_other_creature(what)=>EventPattern::other_creature_enters(),
        // CR 700.4: this creature or another creature dying is every
        // creature dying, once per actual move.
        SourceTrigger::AnyOf {events} if events.len()==2
            && events.iter().filter(|event|matches!(event,SourceTrigger::Dies {what} if is_source_creature(what))).count()==1
            && events.iter().filter(|event|matches!(event,SourceTrigger::Dies {what} if is_other_creature(what))).count()==1 =>EventPattern::Dies { object: creature() },
        _=>return Err(unsupported("abilities.trigger","unsupported complete event/source/other/alternative trigger binding")),
    };
    let mut condition = TriggerCondition::on(pattern);
    validate_trigger_damage_sources(&condition, effects)?;
    let pattern = condition.pattern().clone();
    let mut program = SpellProgram {
        optional_additional_costs: vec![],
        additional_sacrifice: None,
        timing: SpellTiming::Instant,
        resolution: SpellResolution::Instructions,
        functioning_zone: FunctioningZone::Stack,
        optional_target: false,
        targets: vec![],
        instructions: vec![],
    };
    if let Some(instruction) = source_dies_nonland_destroy::instruction(
        &condition,
        effects,
        intervening,
        source_block(block)?,
    ) {
        program.instructions.push(instruction);
    } else if pattern == EventPattern::self_enters()
        && matches!(intervening, Some(SourceCondition::WasCast { .. }))
        && let [
            Sentence {
                bindings,
                steps,
                condition: None,
                condition_kind: None,
                ..
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
    } else {
        append_spell_effects(
            &mut program,
            effects,
            block,
            &mut BTreeMap::new(),
            trigger_player_binding(&condition),
            Context::Triggered,
        )?;
    }
    let intervening_if = intervening_condition(Some(trigger), &pattern, intervening)?;
    condition = TriggerCondition::Event {
        pattern,
        intervening_if: intervening_if.map(|i| i.condition()),
    };
    Ok(TriggeredAbility {
        modal: None,
        functioning_zone: FunctioningZone::Battlefield,
        condition,
        program,
    })
}

/// CR 603.4: the reviewed intervening "if" clauses, each against the events
/// it was reviewed for. `trigger` is the narrow trigger when one decoded.
pub(crate) fn intervening_condition(
    trigger: Option<&SourceTrigger>,
    pattern: &EventPattern,
    intervening: Option<&SourceCondition>,
) -> Result<Option<kir::Intervening>, Failure> {
    Ok(match intervening {
        None => None,
        Some(SourceCondition::WasCast { what, by })
            if *pattern == EventPattern::self_enters()
                && *what == serde_json::json!({"ref":"it"})
                && by.player == "you"
                && by.of.is_none()
                && by.selection_id.is_none() =>
        {
            Some(kir::Intervening::SourceCastByController)
        }
        Some(SourceCondition::Control {
            who,
            what: TriggerObject::Filter { filter, count },
        }) if matches!(trigger, Some(SourceTrigger::Enters { what: TriggerObject::This { noun } }) if noun == "enchantment")
            && who.player == "you" =>
        {
            Some(kir::Intervening::ControllerHasCreatureWithPower {
                minimum_power: power_entry_draw::minimum_power(filter, count)?,
            })
        }
        Some(SourceCondition::CardsInHand { who, comparison })
            if (*pattern == EventPattern::self_dies()
                || matches!(pattern, EventPattern::StepBegins { .. }))
                && who.player == "you"
                && who.selection_id.is_none()
                && who.of.is_none()
                && comparison.op == "eq"
                && comparison.value.amount == "fixed"
                && comparison.value.value == 0 =>
        {
            Some(kir::Intervening::ControllerHandEmpty)
        }
        Some(SourceCondition::Status {
            what: TriggerObject::This { noun },
            status,
        }) if matches!(pattern, EventPattern::StepBegins { .. })
            && matches!(
                noun.as_str(),
                "artifact" | "creature" | "enchantment" | "permanent"
            ) =>
        {
            Some(kir::Intervening::SourceTapped {
                tapped: *status == TapStatus::Tapped,
            })
        }
        _ => {
            return Err(unsupported(
                "abilities.condition",
                "unsupported complete intervening-if condition",
            ));
        }
    })
}
