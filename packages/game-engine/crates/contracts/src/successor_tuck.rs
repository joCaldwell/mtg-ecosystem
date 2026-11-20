use super::*;
use std::num::NonZeroU32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SourceMoveOrigin {
    Battlefield,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TriggerSuccessorZone {
    TriggerControllerGraveyard,
    TriggerControllerLibrary,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum LibraryPosition {
    NthFromTop(NonZeroU32),
}

pub(super) fn contains_move(program: &SpellProgram) -> bool {
    program
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::MoveTriggeredSuccessor { .. }))
        || program.modes.iter().any(|m| contains_move(&m.program))
}

pub(super) fn validate(
    characteristics: &Characteristics,
    ability: &TriggeredAbility,
) -> Result<(), String> {
    let p = &ability.program;
    if !characteristics.has_type(CardKind::Creature)
        || ability.functioning_zone != FunctioningZone::Battlefield
        || ability.condition
            != (TriggerCondition::SelfMoved {
                from: SourceMoveOrigin::Battlefield,
                to: TriggerSuccessorZone::TriggerControllerGraveyard,
            })
        || ability.death_filter.is_some()
        || ability.intervening_if.is_some()
        || p.additional_sacrifice.is_some()
        || !p.optional_additional_costs.is_empty()
        || p.functioning_zone != FunctioningZone::Stack
        || p.timing != SpellTiming::Instant
        || p.resolution != SpellResolution::Instructions
        || !p.targets.is_empty()
        || !p.modes.is_empty()
        || !matches!(
            p.instructions.as_slice(),
            [Instruction::MoveTriggeredSuccessor {
                expected_zone: TriggerSuccessorZone::TriggerControllerGraveyard,
                destination: TriggerSuccessorZone::TriggerControllerLibrary,
                position: LibraryPosition::NthFromTop(_),
                ..
            }]
        )
    {
        return Err("successor tuck requires a mandatory self-move to the trigger controller's graveyard and one ordered move to their library".into());
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut serde_json::Value) {
    use serde_json::json;
    let moved = json!({"type":"object","required":["self_moved"]});
    let condition = TriggerCondition::SelfMoved {
        from: SourceMoveOrigin::Battlefield,
        to: TriggerSuccessorZone::TriggerControllerGraveyard,
    };
    let movement = json!({"type":"object","required":["op"],"properties":{
        "op":{"const":"move_triggered_successor"}
    }});
    let free = json!({"$ref":"#/$defs/SuccessorTuckFreeProgram"});
    schema["$defs"].as_object_mut().unwrap().insert(
        "SuccessorTuckFreeProgram".into(),
        json!({"properties":{
            "instructions":{"not":{"contains":movement}},
            "modes":{"items":{"properties":{"program":free}}}
        }}),
    );
    schema["$defs"]["TriggeredAbility"]
        .as_object_mut()
        .unwrap()
        .entry("allOf")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .unwrap()
        .push(json!({
            "if":{"properties":{"condition":moved},"required":["condition"]},
            "then":{"properties":{
                "condition":{"const":condition},
                "death_filter":{"type":"null"},
                "intervening_if":{"type":"null"},
                "functioning_zone":{"const":"battlefield"},
                "program":{"properties":{
                    "additional_sacrifice":{"type":"null"},
                    "optional_additional_costs":{"maxItems":0},
                    "timing":{"const":"instant"},
                    "resolution":{"const":"instructions"},
                    "functioning_zone":{"const":"stack"},
                    "targets":{"maxItems":0},
                    "modes":{"maxItems":0},
                    "instructions":{"minItems":1,"maxItems":1,"items":{"properties":{
                        "op":{"const":"move_triggered_successor"},
                        "expected_zone":{"const":TriggerSuccessorZone::TriggerControllerGraveyard},
                        "destination":{"const":TriggerSuccessorZone::TriggerControllerLibrary}
                    }}}
                }}
            }},
            "else":{"properties":{"program":free}}
        }));
    let cap = Capability::TriggeredSuccessorLibraryTuckV1;
    schema["$defs"]["Definition"]
        .as_object_mut()
        .unwrap()
        .entry("allOf")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .unwrap()
        .push(json!({
            "properties":{"spell":free,"activated":{"items":{"properties":{"program":free}}}},
            "if":{"required":["triggered"],"properties":{"triggered":{"contains":{
                "required":["condition"],"properties":{"condition":moved}
            }}}},
            "then":{"properties":{
                "characteristics":{"properties":{"kind":{"const":"creature"}}},
                "capabilities":{"contains":{"const":cap}}
            }},
            "else":{"properties":{"capabilities":{"not":{"contains":{"const":cap}}}}}
        }));
}

#[cfg(test)]
pub(super) fn witness(mut d: Definition) -> Definition {
    d.source_labels = None;
    d.triggered = vec![TriggeredAbility {
        death_filter: None,
        functioning_zone: FunctioningZone::Battlefield,
        condition: TriggerCondition::SelfMoved {
            from: SourceMoveOrigin::Battlefield,
            to: TriggerSuccessorZone::TriggerControllerGraveyard,
        },
        intervening_if: None,
        program: SpellProgram {
            optional_additional_costs: vec![],
            additional_sacrifice: None,
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            functioning_zone: FunctioningZone::Stack,
            targets: vec![],
            modes: vec![],
            instructions: vec![Instruction::MoveTriggeredSuccessor {
                expected_zone: TriggerSuccessorZone::TriggerControllerGraveyard,
                destination: TriggerSuccessorZone::TriggerControllerLibrary,
                position: LibraryPosition::NthFromTop(NonZeroU32::new(2).unwrap()),
                source_block: 0,
            }],
        },
    }];
    d.capabilities = d.requirements();
    d
}

#[test]
fn successor_tuck_positive_position_and_trigger_scope() {
    let catalog = Catalog::read(include_str!("../../../tests/fixtures/expected-ir.json")).unwrap();
    let creature = catalog
        .definitions
        .into_iter()
        .find(|d| d.name == "Grizzly Bears")
        .unwrap();
    let mut d = witness(creature);
    d.validate().unwrap();
    let op = &d.triggered[0].program.instructions[0];
    let mut op_json = serde_json::to_value(op).unwrap();
    op_json["position"]["nth_from_top"] = 0.into();
    assert!(strict_read::<Instruction>(&op_json.to_string()).is_err());

    d.triggered[0].condition = TriggerCondition::SelfDies;
    d.capabilities = d.requirements();
    assert!(d.validate().is_err());
}
