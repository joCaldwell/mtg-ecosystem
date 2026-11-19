use super::*;
use serde_json::{Value, json};

// The IR condition stays independent of the effect. Source admission currently
// proves one complete self-entry operation and its printed condition.
fn contains_untap(p: &SpellProgram) -> bool {
    p.instructions
        .iter()
        .any(|i| matches!(i, Instruction::UntapControllerLands { .. }))
        || p.modes.iter().any(|mode| contains_untap(&mode.program))
}

pub(super) fn validate_source(d: &Definition) -> Result<(), String> {
    let gated = d
        .triggered
        .iter()
        .any(|a| a.intervening_if == Some(InterveningCondition::SourceCastByController));
    if d.spell.as_ref().is_some_and(contains_untap)
        || d.activated.iter().any(|a| contains_untap(&a.program))
        || d.static_abilities.iter().any(|a| matches!(&a.effect, StaticEffect::GrantSpellCascade {program,..} if contains_untap(program)))
        || (!gated && d.triggered.iter().any(|a| contains_untap(&a.program))) {
        return Err("land untap requires the complete cast-entry program".into());
    }
    let raw = &d.provenance.source_json;
    if !gated && !raw.contains("if you cast it,") && !raw.contains(r"\u") {
        return Ok(());
    }
    let source: Value = strict_read(raw)?;
    let text = source["oracle_text"].as_str().unwrap_or("");
    if !gated && !text.contains(", if you cast it,") {
        return Ok(());
    }
    let error = "cast-entry source requires its matching historical condition and complete effect";
    let [trigger] = d.triggered.as_slice() else {
        return Err(error.into());
    };
    let who = json!({"player":"you"});
    let draw = json!({"effect":"draw","who":who,"amount":{"amount":"fixed","value":1}});
    let (action, effect) = match trigger.program.instructions.as_slice() {
        [Instruction::OptionalDrawCards { source_block: 0 }] => {
            let mut draw = draw;
            draw["optional"] = json!(true);
            ("you may draw a card.", draw)
        }
        [
            Instruction::DrawCards {
                recipient: None,
                count,
                source_block: 0,
            },
        ] if *count == Quantity::from(1) => ("draw a card.", draw),
        [Instruction::UntapControllerLands { source_block: 0 }] => (
            "untap all lands you control.",
            json!({"effect":"untap","what":{"ref":"all","filter":{"types":["land"],"control":"you"}}}),
        ),
        _ => return Err(error.into()),
    };
    let abilities: Value = strict_read(&d.provenance.abilities_json)?;
    if !gated
        || source["name"] != d.name
        || source["oracle_id"] != d.oracle_id
        || text != format!("When {} enters, if you cast it, {action}", d.name)
        || d.source_labels.as_ref().is_some_and(|labels| {
            labels
                != &SourceLabels {
                    text: text.into(),
                    blocks: vec![format!("When ~ enters, if you cast it, {action}")],
                    triggered: vec![0],
                    ..Default::default()
                }
        })
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
            "effects":[{"steps":[effect]}]}])
    {
        return Err(error.into());
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    let free = json!({"$ref":"#/$defs/ControllerLandUntapFreeProgram"});
    schema["$defs"]["ControllerLandUntapFreeProgram"] = json!({"properties":{
        "instructions":{"items":{"not":{"properties":{"op":{"const":"untap_controller_lands"}}}}},
        "modes":{"items":{"properties":{"program":free}}}
    }});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "properties":{"spell":free,"activated":{"items":{"properties":{"program":free}}},
            "static_abilities":{"items":{"properties":{"effect":{"properties":{"grant_spell_cascade":{"properties":{"program":free}}}}}}}},
        "if":{"required":["triggered"],"properties":{"triggered":{"contains":{"properties":{"program":{"not":free}}}}}},
        "then":{"properties":{"triggered":{"items":{"required":["intervening_if"],"properties":{
            "intervening_if":{"const":InterveningCondition::SourceCastByController}
        }}}, "capabilities":{"contains":{"const":Capability::UntapControllerLandsV1}},
        "source_labels":{"properties":{"blocks":{"const":[format!("When ~ enters, if you cast it, {}", "untap all lands you control.")]},
            "triggered":{"const":[0]},"activated":{"maxItems":0},"statics":{"maxItems":0},"mana":{"maxItems":0},"spell_modes":{"maxItems":0}}}}},
        "else":{"properties":{"capabilities":{"not":{"contains":{"const":Capability::UntapControllerLandsV1}}}}}
    }));
    let condition = json!({"required":["intervening_if"],"properties":{
        "intervening_if":{"const":InterveningCondition::SourceCastByController}
    }});
    let instruction_shapes = json!([
        [Instruction::DrawCards {
            recipient: None,
            count: 1.into(),
            source_block: 0
        }],
        [Instruction::OptionalDrawCards { source_block: 0 }],
        [Instruction::UntapControllerLands { source_block: 0 }]
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
