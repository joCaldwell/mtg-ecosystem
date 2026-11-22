use super::*;
use serde_json::{Value, json};

pub(super) fn validate(ability: &TriggeredAbility) -> Result<(), String> {
    let TriggerCondition::SpellCastWithOrigin { caster, spell, .. } = &ability.condition else {
        return Err("expected a cast-origin trigger".into());
    };
    spell.validate()?;
    let p = &ability.program;
    if *caster != CasterRelation::Controller
        || !p.targets.is_empty()
        || !matches!(p.instructions.as_slice(),
            [kernel!(Draw { player: you!(), amount: fixed!(count) })] if *count == Quantity::from(1))
    {
        return Err("cast-origin triggers require a battlefield controller cast and one mandatory controller draw".into());
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    let condition = json!({"type":"object","required":["spell_cast_with_origin"]});
    let trigger = json!({"required":["condition"],"properties":{"condition":condition}});
    schema["$defs"]["TriggeredAbility"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "if":trigger,
            "then":{"properties":{
                "condition":{"properties":{"spell_cast_with_origin":{"properties":{
                    "caster":{"const":CasterRelation::Controller}
                }}}},
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
                    "instructions":{"minItems":1,"maxItems":1,"items":instruction_schema::single_draw()}
                }}
            }}
        }));
    let cap = Capability::CastOriginTriggerV1;
    schema["$defs"]["Definition"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "if":{"required":["triggered"],"properties":{"triggered":{"contains":trigger}}},
            "then":{"properties":{"capabilities":{"allOf":[
                {"contains":{"const":cap}},
                {"contains":{"const":Capability::CastTriggerV1}},
                {"contains":{"const":Capability::ControllerDrawV1}},
                {"contains":{"const":Capability::TriggeredAbilityV1}}
            ]}}},
            "else":{"properties":{"capabilities":{"not":{"contains":{"const":cap}}}}}
        }));
}
