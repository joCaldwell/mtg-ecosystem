use super::*;
use serde_json::{Value, json};

// The IR condition stays independent of the effect. Source admission currently
// proves a single controller draw (mandatory or optional), with no other line.
pub(super) fn validate_source(d: &Definition) -> Result<(), String> {
    let gated = d
        .triggered
        .iter()
        .any(|a| a.intervening_if == Some(InterveningCondition::SourceCastByController));
    let raw = &d.provenance.source_json;
    if !gated && !raw.contains("if you cast it,") && !raw.contains(r"\u") {
        return Ok(());
    }
    let source: Value = strict_read(raw)?;
    let text = source["oracle_text"].as_str().unwrap_or("");
    if !gated && !text.contains(", if you cast it,") {
        return Ok(());
    }
    let error = "cast-entry source requires its matching historical condition and complete draw";
    let [trigger] = d.triggered.as_slice() else {
        return Err(error.into());
    };
    let optional = match trigger.program.instructions.as_slice() {
        [Instruction::OptionalDrawCards { source_block: 0 }] => true,
        [
            Instruction::DrawCards {
                recipient: None,
                count,
                source_block: 0,
            },
        ] if *count == Quantity::from(1) => false,
        _ => return Err(error.into()),
    };
    let who = json!({"player":"you"});
    let mut draw = json!({"effect":"draw","who":who,"amount":{"amount":"fixed","value":1}});
    if optional {
        draw["optional"] = json!(true);
    }
    let abilities: Value = strict_read(&d.provenance.abilities_json)?;
    let action = if optional {
        "you may draw a card."
    } else {
        "draw a card."
    };
    if !gated
        || source["name"] != d.name
        || source["oracle_id"] != d.oracle_id
        || text != format!("When {} enters, if you cast it, {action}", d.name)
        || !d.keywords.is_empty()
        || !d.activated.is_empty()
        || !d.mana_abilities.is_empty()
        || !d.static_abilities.is_empty()
        || d.intrinsic_zone.is_some()
        || !trigger.program.targets.is_empty()
        || trigger.death_filter.is_some()
        || abilities
            != json!([{"kind":"triggered","trigger":{"trigger":"enters","what":{"ref":"self"}},
            "condition":{"condition":"was-cast","what":{"ref":"it"},"by":who},
            "effects":[{"steps":[draw]}]}])
    {
        return Err(error.into());
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    let condition = json!({"required":["intervening_if"],"properties":{
        "intervening_if":{"const":InterveningCondition::SourceCastByController}
    }});
    let instruction_shapes = json!([
        [Instruction::DrawCards {
            recipient: None,
            count: 1.into(),
            source_block: 0
        }],
        [Instruction::OptionalDrawCards { source_block: 0 }]
    ]);
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"anyOf":[
            {"required":["triggered"],"properties":{"triggered":{"contains":condition}}},
            {"required":["provenance"],"properties":{"provenance":{"properties":{"source_json":{
                "pattern":r#""oracle_text"\s*:\s*"[^"]*, if you cast it,"#
            }}}}}
        ]},
        "then":{"required":["triggered"],"properties":{
            "triggered":{"minItems":1,"maxItems":1,"items":{"required":["intervening_if"],"properties":{
                "intervening_if":{"const":InterveningCondition::SourceCastByController},
                "condition":{"const":TriggerCondition::SelfEntersBattlefield},
                "functioning_zone":{"const":FunctioningZone::Battlefield},
                "death_filter":{"type":"null"},"program":{"properties":{
                    "targets":{"maxItems":0},"modes":{"maxItems":0},"additional_sacrifice":{"type":"null"},
                    "optional_additional_costs":{"maxItems":0},"instructions":{"enum":instruction_shapes}
                }}
            }}},
            "keywords":{"maxItems":0},"activated":{"maxItems":0},"mana_abilities":{"maxItems":0},
            "static_abilities":{"maxItems":0},"intrinsic_zone":{"type":"null"},
            "characteristics":{"properties":{"kind":{"const":CardKind::Creature}}},
            "capabilities":{"allOf":[{"contains":{"const":Capability::CastEntryConditionV1}},
                {"contains":{"const":Capability::InterveningIfV1}},{"contains":{"const":Capability::SelfEntersBattlefieldV1}}]}
        }},
        "else":{"properties":{"capabilities":{"not":{"contains":{"const":Capability::CastEntryConditionV1}}}}}
    }));
}
