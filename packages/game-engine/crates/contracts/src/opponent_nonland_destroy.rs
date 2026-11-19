//! Complete ordinary opponent nonland destruction.
use super::*;
use serde_json::{Value, json};

const ORACLE_TEXT: &str = "Destroy all nonland permanents your opponents control.";
const INVALID_FAMILY: &str = "opponent nonland destruction requires one complete ordinary sorcery";

fn contains_reserved_instruction(program: &SpellProgram) -> bool {
    let set = ObjectSet::OpponentNonlandPermanents;
    program
        .instructions
        .iter()
        .any(|instruction| match instruction {
            Instruction::DestroySet { .. } => true,
            Instruction::MoveSet { set: operand, .. }
            | Instruction::PumpSet { set: operand, .. }
            | Instruction::GrantKeywordsSet { set: operand, .. } => operand == &set,
            Instruction::DamageSet { sets, .. } => sets.contains(&set),
            _ => false,
        })
        || program
            .modes
            .iter()
            .any(|mode| contains_reserved_instruction(&mode.program))
}

pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    if d.activated
        .iter()
        .any(|a| contains_reserved_instruction(&a.program))
        || d.triggered
            .iter()
            .any(|a| contains_reserved_instruction(&a.program))
    {
        return Err(INVALID_FAMILY.into());
    }
    let reserved = d.spell.as_ref().is_some_and(contains_reserved_instruction);
    if !reserved && !d.provenance.source_json.contains(ORACLE_TEXT) {
        return Ok(());
    }

    let source: Value = strict_read(&d.provenance.source_json)?;
    if !reserved
        && !source["oracle_text"]
            .as_str()
            .is_some_and(|text| text.contains(ORACLE_TEXT))
    {
        return Ok(());
    }
    let p = d.spell.as_ref().ok_or(INVALID_FAMILY)?;
    let abilities: Value = strict_read(&d.provenance.abilities_json)?;
    if source["type_line"] != "Sorcery"
        || source["oracle_text"] != ORACLE_TEXT
        || source["layout"] != "normal"
        || abilities
            != json!([{"kind":"spell","effects":[{"steps":[{
                "effect":"destroy","what":{"ref":"all","filter":{
                    "nonTypes":["land"],"cls":"permanent","control":"opponent"
                }}
            }]}]}])
        || d.characteristics.kind != CardKind::Sorcery
        || d.characteristics.snow
        || !d.characteristics.subtypes.is_empty()
        || !d.keywords.is_empty()
        || !d.static_abilities.is_empty()
        || !d.activated.is_empty()
        || !d.triggered.is_empty()
        || !d.mana_abilities.is_empty()
        || d.x_mana_cost
        || p.functioning_zone != FunctioningZone::Stack
        || p.timing != SpellTiming::MainPhase
        || p.resolution != SpellResolution::Instructions
        || !p.targets.is_empty()
        || p.additional_sacrifice.is_some()
        || !p.optional_additional_costs.is_empty()
        || !p.modes.is_empty()
        || !matches!(
            p.instructions.as_slice(),
            [Instruction::DestroySet {
                set: ObjectSet::OpponentNonlandPermanents,
                source_block: 0,
            }]
        )
    {
        return Err(INVALID_FAMILY.into());
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    let set = ObjectSet::OpponentNonlandPermanents;
    let free = json!({"$ref":"#/$defs/OpponentNonlandDestroyFreeProgram"});
    schema["$defs"]["OpponentNonlandDestroyFreeProgram"] = json!({"properties":{
        "instructions":{"items":{"not":{"anyOf":[
            {"required":["op"],"properties":{"op":{"const":"destroy_set"}}},
            {"required":["set"],"properties":{"set":{"const":set}}},
            {"required":["sets"],"properties":{"sets":{"contains":{"const":set}}}}
        ]}}},
        "modes":{"items":{"properties":{"program":free}}}
    }});
    schema["$defs"]["Definition"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "properties":{
                "activated":{"items":{"properties":{"program":free}}},
                "triggered":{"items":{"properties":{"program":free}}}
            },
            "if":{"anyOf":[
                {"required":["spell"],"properties":{"spell":{"type":"object","not":free}}},
                {"required":["provenance"],"properties":{"provenance":{"properties":{
                    "source_json":{"pattern":r#""oracle_text"\s*:\s*"[^"]*Destroy all nonland permanents your opponents control\."#}
                }}}}
            ]},
            "then":{"required":["spell"],"properties":{
                "spell":{"type":"object","properties":{
                    "functioning_zone":{"const":"stack"},"timing":{"const":"main_phase"},
                    "resolution":{"const":"instructions"},"targets":{"maxItems":0},
                    "additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},
                    "modes":{"maxItems":0},
                    "instructions":{"const":[{"op":"destroy_set","set":set,"source_block":0}]}
                }},
                "characteristics":{"properties":{
                    "kind":{"const":"sorcery"},"additional_types":{"maxItems":0},
                    "subtypes":{"maxItems":0},"snow":{"const":false},"basic":{"const":false},
                    "legendary":{"const":false},"power":{"type":"null"},"toughness":{"type":"null"}
                }},
                "intrinsic_zone":{"type":"null"},"x_mana_cost":{"const":false},
                "activated":{"maxItems":0},"triggered":{"maxItems":0},"mana_abilities":{"maxItems":0},
                "keywords":{"maxItems":0},"static_abilities":{"maxItems":0},
                "capabilities":{"minItems":3,"maxItems":3,"uniqueItems":true,"items":{"enum":[
                    Capability::NormalLayoutV1,Capability::CastSorceryV1,Capability::BoardWipesV1
                ]}}
            }}
        }));
}
