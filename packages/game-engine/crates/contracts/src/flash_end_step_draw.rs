use super::*;
use serde_json::{Value, json};

// Exactly three source abilities are admitted. Their raw order is preserved by
// the compiler, independently of the printed-label indices.
const LAST_SOURCE_BLOCK: u32 = 2;

pub(super) fn trigger_program(program: &SpellProgram) -> bool {
    program.timing == SpellTiming::Instant
        && controller_counted_draw::standalone_fixed_controller_draw(program)
            .is_some_and(|(count, block)| *count > Quantity::zero() && block <= LAST_SOURCE_BLOCK)
}

pub(super) fn definition_matches(definition: &Definition) -> bool {
    let [ability] = definition.triggered.as_slice() else {
        return false;
    };
    let Some(spell) = &definition.spell else {
        return false;
    };
    let c = &definition.characteristics;
    c.kind == CardKind::Creature
        && c.additional_types.is_empty()
        && c.legendary
        && !c.basic
        && !c.snow
        && !c.subtypes.iter().any(|subtype| {
            unlimited_hand_size::DISALLOWED_CREATURE_SUBTYPES.contains(&subtype.as_str())
        })
        && !definition.x_mana_cost
        && definition.intrinsic_zone.is_none()
        && definition.keywords == [KeywordAbility::Flash]
        && definition.activated.is_empty()
        && definition.mana_abilities.is_empty()
        && matches!(
            definition.static_abilities.as_slice(),
            [StaticAbility {
                functioning_zone: FunctioningZone::Battlefield,
                effect: StaticEffect::ReduceMaximumHandSize {
                    scope: HandSizeScope::Opponents,
                    amount,
                },
            }] if *amount > Quantity::zero()
        )
        && ability.condition
            == (TriggerCondition::StepBegan {
                step: BeginningStep::End,
                turn_relation: TurnRelation::Controller,
            })
        && ability.functioning_zone == FunctioningZone::Battlefield
        && ability.death_filter.is_none()
        && ability.intervening_if.is_none()
        && trigger_program(&ability.program)
        && spell.timing == SpellTiming::Instant
        && spell.resolution == SpellResolution::Permanent
        && spell.functioning_zone == FunctioningZone::Stack
        && spell.additional_sacrifice.is_none()
        && spell.optional_additional_costs.is_empty()
        && spell.instructions.is_empty()
        && spell.targets.is_empty()
        && spell.modes.is_empty()
}

pub(super) fn extend_schema(schema: &mut Value) {
    use Capability::*;
    let capabilities = [
        NormalLayoutV1,
        BaseCreatureV1,
        CastCreatureV1,
        CommanderIdentityV1,
        FlashV1,
        ReducedHandSizeV1,
        TriggeredAbilityV1,
        StepBeginningTriggerV1,
    ];
    // CR 121.1-121.2: one card is the single draw, more is the counted draw.
    let single = json!({"required":["triggered"],"properties":{"triggered":{"items":{"properties":{
        "program":{"properties":{"instructions":{"items":instruction_schema::single_draw()}}}}}}}});
    let mut allowed = capabilities.to_vec();
    allowed.extend([ControllerCountedDrawV1, ControllerDrawV1]);
    let disallowed_subtypes: Vec<_> = UNSUPPORTED_RULES_SUBTYPES
        .iter()
        .chain(unlimited_hand_size::DISALLOWED_CREATURE_SUBTYPES)
        .collect();
    schema["$defs"]["FlashEndStepDrawProgram"] = json!({"properties":{
        "timing":{"const":SpellTiming::Instant},
        "resolution":{"const":SpellResolution::Instructions},
        "functioning_zone":{"const":FunctioningZone::Stack},
        "additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},
        "targets":{"maxItems":0},
        "modes":{"maxItems":0},
        "instructions":{"minItems":1,"maxItems":1,"items":instruction_schema::at_most(
            instruction_schema::fixed_controller_draw("^[1-9][0-9]*$"), LAST_SOURCE_BLOCK
        )}
    }});
    schema["$defs"]["FlashEndStepDrawDefinition"] = json!({
        "required":["characteristics","spell","keywords","static_abilities","triggered","capabilities"],
        "properties":{
            "characteristics":{"properties":{
                "kind":{"const":CardKind::Creature},
                "additional_types":{"maxItems":0},
                "legendary":{"const":true},
                "basic":{"const":false},
                "snow":{"const":false},
                "subtypes":{"items":{"not":{"enum":disallowed_subtypes}}},
                "power":{"type":"string"},
                "toughness":{"type":"string","pattern":"^[1-9][0-9]*$"},
                "mana_cost":{"properties":{"generic":{"pattern":"^(0|[1-9][0-9]*)$"}}},
                "colors":{"items":{"not":{"const":Color::C}}},
                "color_identity":{"items":{"not":{"const":Color::C}}}
            }},
            "x_mana_cost":{"const":false},
            "intrinsic_zone":{"type":"null"},
            "keywords":{"const":[KeywordAbility::Flash]},
            "activated":{"maxItems":0},
            "mana_abilities":{"maxItems":0},
            "static_abilities":{"minItems":1,"maxItems":1,"items":{
                "properties":{
                    "functioning_zone":{"const":FunctioningZone::Battlefield},
                    "effect":{"required":["reduce_maximum_hand_size"],"properties":{
                        "reduce_maximum_hand_size":{"properties":{
                            "scope":{"const":HandSizeScope::Opponents},
                            "amount":{"type":"string","pattern":"^[1-9][0-9]*$"}
                        }}
                    }}
                }
            }},
            "triggered":{"minItems":1,"maxItems":1,"items":{"properties":{
                "functioning_zone":{"const":FunctioningZone::Battlefield},
                "condition":{"const":TriggerCondition::StepBegan {
                    step: BeginningStep::End,
                    turn_relation: TurnRelation::Controller
                }},
                "death_filter":{"type":"null"},
                "intervening_if":{"type":"null"},
                "program":{"$ref":"#/$defs/FlashEndStepDrawProgram"}
            }}},
            "spell":{"type":"object","properties":{
                "timing":{"const":SpellTiming::Instant},
                "resolution":{"const":SpellResolution::Permanent},
                "functioning_zone":{"const":FunctioningZone::Stack},
                "additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},
                "targets":{"maxItems":0},"instructions":{"maxItems":0},"modes":{"maxItems":0}
            }},
            "capabilities":{
                "minItems":capabilities.len() + 1,"maxItems":capabilities.len() + 1,
                "uniqueItems":true,"items":{"enum":allowed},
                "allOf":capabilities.iter().map(|c| json!({"contains":{"const":c}})).collect::<Vec<_>>()
            }
        },
        "if":single,
        "then":{"properties":{"capabilities":{"contains":{"const":ControllerDrawV1}}}},
        "else":{"properties":{"capabilities":{"contains":{"const":ControllerCountedDrawV1}}}}
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flash_end_step_draw() {
        let mut definition = hand_size_adjustments::reduced_witness();
        definition.characteristics.legendary = true;
        definition.keywords = vec![KeywordAbility::Flash];
        definition.spell.as_mut().unwrap().timing = SpellTiming::Instant;
        let program = SpellProgram {
            resolution: SpellResolution::Instructions,
            instructions: vec![Instruction::draw(
                PlayerExpr::you(Context::Triggered),
                7.into(),
                1,
            )],
            ..definition.spell.clone().unwrap()
        };
        definition.triggered = vec![TriggeredAbility {
            functioning_zone: FunctioningZone::Battlefield,
            condition: TriggerCondition::StepBegan {
                step: BeginningStep::End,
                turn_relation: TurnRelation::Controller,
            },
            death_filter: None,
            intervening_if: None,
            program,
        }];
        definition.capabilities = definition.requirements();
        definition.validate().unwrap();
        assert!(
            definition
                .capabilities
                .contains(&Capability::ControllerCountedDrawV1)
        );
        assert!(
            !definition
                .capabilities
                .contains(&Capability::ControllerHandReturnV1)
        );

        let check_invalid = |mut invalid: Definition| {
            invalid.capabilities = invalid.requirements();
            assert!(invalid.validate().is_err());
        };
        let mut invalid = definition.clone();
        invalid.characteristics.legendary = false;
        check_invalid(invalid);
        let mut invalid = definition.clone();
        invalid.triggered[0].condition = TriggerCondition::StepBegan {
            step: BeginningStep::End,
            turn_relation: TurnRelation::AnyPlayer,
        };
        check_invalid(invalid);
        let mut invalid = definition.clone();
        invalid.static_abilities.clear();
        check_invalid(invalid);
        let mut invalid = definition.clone();
        invalid.static_abilities[0].effect = StaticEffect::ReduceMaximumHandSize {
            scope: HandSizeScope::Controller,
            amount: 2.into(),
        };
        check_invalid(invalid);
        // CR 121.1: "draw a card" is the single draw, named ControllerDrawV1.
        let mut single = definition.clone();
        single.triggered[0].program.instructions = vec![Instruction::draw(
            PlayerExpr::you(Context::Triggered),
            1.into(),
            1,
        )];
        single.capabilities = single.requirements();
        single.validate().unwrap();
        assert!(single.capabilities.contains(&Capability::ControllerDrawV1));
        assert!(
            !single
                .capabilities
                .contains(&Capability::ControllerCountedDrawV1)
        );
        let mut mislabelled = single.clone();
        mislabelled
            .capabilities
            .remove(&Capability::ControllerDrawV1);
        mislabelled
            .capabilities
            .insert(Capability::ControllerCountedDrawV1);
        assert!(mislabelled.validate().is_err());
        let schema = catalog_schema();
        let items =
            schema["$defs"]["FlashEndStepDrawProgram"]["properties"]["instructions"]["items"]
                .to_string();
        assert!(
            !items.contains(r#""not":{"const":"1"}"#),
            "the end-step draw admits one card"
        );
        for (count, source_block) in [(0, 1), (7, 3)] {
            let mut invalid = definition.clone();
            invalid.triggered[0].program.instructions = vec![Instruction::draw(
                PlayerExpr::you(Context::Triggered),
                count.into(),
                source_block,
            )];
            check_invalid(invalid);
        }
        let mut ordinary = controller_counted_draw::witness();
        ordinary.spell.as_mut().unwrap().instructions =
            definition.triggered[0].program.instructions.clone();
        check_invalid(ordinary);
    }
}
