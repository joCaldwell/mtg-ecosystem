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

pub(super) fn validate_source(d: &Definition, fixed_activations: bool) -> Result<(), String> {
    let gated = d
        .triggered
        .iter()
        .any(|a| a.intervening_if == Some(InterveningCondition::SourceCastByController));
    if d.spell.as_ref().is_some_and(contains_untap)
        || d.activated.iter().any(|a| contains_untap(&a.program))
        || d.static_abilities.iter().any(|a| matches!(&a.effect, StaticEffect::GrantSpellCascade {program,..} if contains_untap(program)))
        || (!gated && d.triggered.iter().any(|a| !equipped_trigger::is_equipped(a) && contains_untap(&a.program))) {
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
    let block = u32::from(fixed_activations && !d.keywords.is_empty());
    let who = json!({"player":"you"});
    let draw = json!({"effect":"draw","who":who,"amount":{"amount":"fixed","value":1}});
    let (action, effect) = match trigger.program.instructions.as_slice() {
        [Instruction::OptionalDrawCards { source_block }] if *source_block == block => {
            let mut draw = draw;
            draw["optional"] = json!(true);
            ("you may draw a card.", draw)
        }
        [
            Instruction::DrawCards {
                recipient: None,
                count,
                source_block,
            },
        ] if *source_block == block && *count == Quantity::from(1) => ("draw a card.", draw),
        [Instruction::UntapControllerLands { source_block }] if *source_block == block => (
            "untap all lands you control.",
            json!({"effect":"untap","what":{"ref":"all","filter":{"types":["land"],"control":"you"}}}),
        ),
        _ => return Err(error.into()),
    };
    let abilities: Value = strict_read(&d.provenance.abilities_json)?;
    let line = text.lines().nth(block as usize);
    let entry_line = |name: &str| format!("When {name} enters, if you cast it, {action}");
    let printed = line == Some(entry_line(&d.name).as_str())
        || (fixed_activations
            && d.name
                .split_once(',')
                .is_some_and(|(short, _)| line == Some(entry_line(short).as_str())));
    if !gated
        || source["id"] != d.provenance.printing_id
        || source["name"] != d.name
        || source["oracle_id"] != d.oracle_id
        || !printed
        || text.lines().count() != block as usize + 1 + d.activated.len()
        || d.source_labels.as_ref().is_some_and(|labels| {
            if fixed_activations {
                return labels.blocks.get(block as usize)
                    != Some(&format!("When ~ enters, if you cast it, {action}"));
            }
            labels
                != &SourceLabels {
                    text: text.into(),
                    blocks: vec![format!("When ~ enters, if you cast it, {action}")],
                    triggered: vec![0],
                    ..Default::default()
                }
        })
        || (!fixed_activations && !d.keywords.is_empty())
        || (!fixed_activations && !d.activated.is_empty())
        || !d.mana_abilities.is_empty()
        || !d.static_abilities.is_empty()
        || d.intrinsic_zone.is_some()
        || !trigger.program.targets.is_empty()
        || trigger.death_filter.is_some()
        || abilities
            .as_array()
            .is_none_or(|a| a.len() != block as usize + 1 + d.activated.len())
        || abilities[block as usize]
            != json!({"kind":"triggered","trigger":{"trigger":"enters","what":{"ref":"self"}},
            "condition":{"condition":"was-cast","what":{"ref":"it"},"by":who},
            "effects":[{"steps":[effect]}]})
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
    let equipped =
        json!({"enum":[TriggerCondition::EquippedCombatDamage,TriggerCondition::EquippedDies]});
    let untap = json!({"required":["program"],"properties":{"program":{"not":free}}});
    let cast_untap = json!({"allOf":[untap,{"properties":{"condition":{"not":equipped}}}]});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "properties":{"spell":free,"activated":{"items":{"properties":{"program":free}}},
            "static_abilities":{"items":{"properties":{"effect":{"properties":{"grant_spell_cascade":{"properties":{"program":free}}}}}}},
            "triggered":{"items":{"if":cast_untap,"then":{"required":["intervening_if"],"properties":{
                "intervening_if":{"const":InterveningCondition::SourceCastByController}
            }}}}},
        "if":{"required":["triggered"],"properties":{"triggered":{"contains":untap}}},
        "then":{"properties":{"capabilities":{"contains":{"const":Capability::UntapControllerLandsV1}}},
            "if":{"properties":{"triggered":{"contains":cast_untap}}},
            "then":{"properties":{"source_labels":{"properties":{
                "blocks":{"contains":{"const":"When ~ enters, if you cast it, untap all lands you control."}},
                "triggered":{"enum":[[0],[1]]},"statics":{"maxItems":0},"mana":{"maxItems":0},"spell_modes":{"maxItems":0}
            }}}}},
        "else":{"properties":{"capabilities":{"not":{"contains":{"const":Capability::UntapControllerLandsV1}}}}}
    }));
    let block_shape = |block: u32| {
        json!({"properties":{
            "triggered":{"items":{"properties":{"program":{"properties":{"instructions":{"items":{"properties":{"source_block":{"const":block}}}}}}}}},
            "source_labels":{"properties":{"triggered":{"const":[block]}}}
        }})
    };
    let condition = json!({"required":["intervening_if"],"properties":{
        "intervening_if":{"const":InterveningCondition::SourceCastByController}
    }});
    let instruction_shapes = (0..=1)
        .flat_map(|source_block| {
            [
                vec![Instruction::DrawCards {
                    recipient: None,
                    count: 1.into(),
                    source_block,
                }],
                vec![Instruction::OptionalDrawCards { source_block }],
                vec![Instruction::UntapControllerLands { source_block }],
            ]
        })
        .collect::<Vec<_>>();
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"anyOf":[
            {"required":["triggered"],"properties":{"triggered":{"contains":condition}}},
            {"required":["provenance"],"properties":{"provenance":{"properties":{"source_json":{
                "pattern":r#""oracle_text"\s*:\s*"[^"]*, if you cast it,"#
            }}}}}
        ]},
        "then":{"required":["triggered"],"allOf":[{
            "if":{"required":["keywords"],"properties":{"keywords":{"minItems":1}}},
            "then":{"required":["activated"],"properties":{"activated":{"minItems":1}},"allOf":[block_shape(1)]},
            "else":block_shape(0)
        },{
            "if":{"not":{"required":["activated"],"properties":{"activated":{"minItems":1}}}},
            "then":{"properties":{"source_labels":{"properties":{"activated":{"maxItems":0},"blocks":{"maxItems":1}}}}}
        }],"properties":{
            "triggered":{"minItems":1,"maxItems":1,"items":{"required":["intervening_if"],"properties":{
                "intervening_if":{"const":InterveningCondition::SourceCastByController},
                "condition":{"const":TriggerCondition::SelfEntersBattlefield},
                "functioning_zone":{"const":FunctioningZone::Battlefield},
                "death_filter":{"type":"null"},"program":{"properties":{
                    "targets":{"maxItems":0},"modes":{"maxItems":0},"additional_sacrifice":{"type":"null"},
                    "optional_additional_costs":{"maxItems":0},"instructions":{"enum":instruction_shapes}
                }}
            }}},
            "keywords":{"uniqueItems":true,"items":{"enum":["flying","reach","trample","vigilance"]}},"activated":{"items":{"$ref":"#/$defs/FixedCreatureActivation"}},"mana_abilities":{"maxItems":0},
            "static_abilities":{"maxItems":0},"intrinsic_zone":{"type":"null"},
            "characteristics":{"properties":{"kind":{"const":CardKind::Creature}}},
            "capabilities":{"allOf":[{"contains":{"const":Capability::CastEntryConditionV1}},
                {"contains":{"const":Capability::InterveningIfV1}},{"contains":{"const":Capability::SelfEntersBattlefieldV1}}]}
        }},
        "else":{"properties":{"capabilities":{"not":{"contains":{"const":Capability::CastEntryConditionV1}}}}}
    }));
}
