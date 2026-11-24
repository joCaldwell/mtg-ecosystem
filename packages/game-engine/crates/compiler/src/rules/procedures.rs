//! Shared lowering for audited kernel effects and their trigger operands.
use super::*;
use serde_json::{Value, json};

pub(crate) mod controller_library {
    use super::*;
    pub(crate) fn fixed_count(amount: &serde_json::Value) -> Result<Quantity, Failure> {
        let count: Count = serde_json::from_value(amount.clone()).map_err(|_| {
            unsupported(
                "library count",
                "requires complete fixed nonnegative amount",
            )
        })?;
        if count.amount != "fixed" {
            return Err(unsupported(
                "library count",
                "requires fixed nonnegative amount",
            ));
        }
        Ok(Quantity::from(i64::from(count.value)))
    }

    // Match both directions: an edited AST must not erase a printed lookup,
    // and an injected lookup cannot borrow a different printed effect.
    pub(crate) fn lower_cast_opponent_damage(
        condition: &TriggerCondition,
        effects: &[Sentence],
        block: u32,
        program: &mut SpellProgram,
    ) -> Result<Option<Instruction>, Failure> {
        // Trigger lowering has already bound the damage source to the invocation.
        let TriggerCondition::SpellCast {
            caster: CasterRelation::Controller,
            spell: TypePredicate::SpellColor { .. },
        } = condition
        else {
            return Ok(None);
        };
        let [
            Sentence {
                bindings,
                steps,
                condition: None,
                condition_kind: None,
            },
        ] = effects
        else {
            return Ok(None);
        };
        let [
            Step::Damage {
                optional: None,
                amount,
                to: DamageDestination::Player { reference },
                ..
            },
        ] = steps.as_slice()
        else {
            return Ok(None);
        };
        if !bindings.is_empty() || reference["player"] != "target-opponent" {
            return Ok(None);
        }
        let selection = reference["selectionId"]
            .as_str()
            .ok_or_else(|| unsupported("damage.target", "missing bound opponent occurrence"))?;
        if selection.is_empty()
            || *reference != json!({"player":"target-opponent", "selectionId":selection})
        {
            return Err(unsupported(
                "damage.target",
                "unexpected opponent selector fields",
            ));
        }
        Ok(Some(Instruction::damage_target(
            bind_target(
                program,
                &mut BTreeMap::new(),
                selection,
                TargetDomain::OpponentPlayer,
            )?,
            kir::Amount::Fixed {
                value: fixed_positive_value(amount)?,
            },
            block,
        )))
    }
}
pub(crate) mod fixed_mill {
    use super::*;
    pub(crate) fn target_selection(who: &serde_json::Value) -> Option<&str> {
        let object = who.as_object()?;
        if object.len() != 2 || who["player"] != "target-player" {
            return None;
        }
        who["selectionId"].as_str().filter(|s| !s.is_empty())
    }

    pub(crate) fn instruction(
        program: &mut SpellProgram,
        slots: &mut BTreeMap<String, u32>,
        who: &serde_json::Value,
        amount: &serde_json::Value,
        context: Context,
        source_block: u32,
    ) -> Result<Instruction, Failure> {
        let player = if *who == serde_json::json!({"player":"you"}) {
            PlayerExpr::you(context)
        } else {
            let id = target_selection(who).ok_or_else(|| {
                unsupported(
                    "mill.who",
                    "requires exact controller or target-player actor",
                )
            })?;
            PlayerExpr::target(bind_target(program, slots, id, TargetDomain::AnyPlayer)?)
        };
        Ok(Instruction::mill(
            player,
            Amount::Fixed {
                value: controller_library::fixed_count(amount)?,
            },
            source_block,
        ))
    }
}
pub(crate) mod source_dies_nonland_destroy {
    use super::*;
    fn selection() -> Value {
        json!({"ref":"all","filter":{"nonTypes":["land"],"cls":"permanent"}})
    }

    pub(crate) fn instruction(
        condition: &TriggerCondition,
        effects: &[Sentence],
        intervening: Option<&SourceCondition>,
        block: u32,
    ) -> Option<Instruction> {
        let [
            Sentence {
                condition: None,
                condition_kind: None,
                bindings,
                steps,
            },
        ] = effects
        else {
            return None;
        };
        if *condition == TriggerCondition::SelfDies
            && intervening.is_none()
            && bindings.is_empty()
            && matches!(steps.as_slice(),
            [Step::Destroy { what, optional: None, regeneration_forbidden: Some(true) }]
                if *what == selection())
        {
            Some(sweep(block))
        } else {
            None
        }
    }

    /// CR 701.8, 701.19c: destroy every nonland permanent; none regenerates.
    fn sweep(block: u32) -> Instruction {
        Instruction::act(
            Verb::Destroy {
                what: ObjectSet::NonlandPermanents.kernel(Context::Triggered),
                regeneration_forbidden: true,
            },
            block,
        )
    }
}
pub(crate) mod land_entry_gain_life {
    use super::*;
    pub(crate) fn lower(
        trigger: &SourceTrigger,
        effects: &[Sentence],
        intervening: Option<&SourceCondition>,
        block: usize,
    ) -> Result<Option<TriggeredAbility>, Failure> {
        if !matches!(trigger, SourceTrigger::Enters {what: TriggerObject::This {noun}} if noun == "land")
        {
            return Ok(None);
        }
        let [
            Sentence {
                bindings,
                steps,
                condition: None,
                condition_kind: None,
            },
        ] = effects
        else {
            return Ok(None);
        };
        let [
            Step::GainLife {
                who,
                amount,
                for_each: None,
            },
        ] = steps.as_slice()
        else {
            return Ok(None);
        };
        if intervening.is_some()
            || !bindings.is_empty()
            || who.player != "you"
            || who.of.is_some()
            || who.selection_id.is_some()
        {
            return Err(unsupported(
                "abilities.trigger",
                "entry life gain must be mandatory for the trigger controller",
            ));
        }
        let amount = fixed_positive(
            &serde_json::from_value(amount.clone())
                .map_err(|_| unsupported("amount", "entry life gain requires a fixed amount"))?,
        )?;
        Ok(Some(TriggeredAbility {
            functioning_zone: FunctioningZone::Battlefield,
            condition: TriggerCondition::SelfEntersBattlefield,
            intervening_if: None,
            death_filter: None,
            program: SpellProgram {
                functioning_zone: FunctioningZone::Stack,
                timing: SpellTiming::Instant,
                resolution: SpellResolution::Instructions,
                optional_additional_costs: vec![],
                additional_sacrifice: None,
                targets: vec![],
                instructions: vec![Instruction::act(
                    Verb::GainLife {
                        player: PlayerExpr::you(Context::Triggered),
                        amount: Amount::Fixed { value: amount },
                    },
                    source_block(block)?,
                )],
            },
        }))
    }
}
pub(crate) mod power_entry_draw {
    use super::*;
    fn shape_error() -> Failure {
        unsupported(
            "abilities.power-entry-draw",
            "requires complete power-filtered entry draw source",
        )
    }

    pub(crate) fn minimum_power(
        filter: &TriggerFilter,
        count: &Count,
    ) -> Result<Quantity, Failure> {
        let Some([property]) = &filter.properties else {
            return Err(shape_error());
        };
        if !fixed_one(count)
            || filter.types != ["creature"]
            || filter.control.is_some()
            || filter.other
            || filter.non_token
            || !filter.non_subtypes.is_empty()
            || property.property != "power"
            || property.comparison.op != "ge"
            || property.comparison.value.amount != "fixed"
        {
            return Err(shape_error());
        }
        Ok(i64::from(property.comparison.value.value).into())
    }
}
