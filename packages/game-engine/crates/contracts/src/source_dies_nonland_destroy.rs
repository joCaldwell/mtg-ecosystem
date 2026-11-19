//! Complete source-dies destruction of nonland permanents (CR 603.10a,
//! 700.4, 701.8, 701.19c and 702.12b).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const DESTROY_TEXT: &str = "destroy all nonland permanents. They can't be regenerated.";
const INVALID_FAMILY: &str =
    "nonland destruction with regeneration prohibited requires a complete source-dies trigger";

fn contains_destroy(p: &SpellProgram) -> bool {
    p.instructions.iter().any(|i| {
        matches!(i, Instruction::DestroySet { set, .. } if *set != ObjectSet::OpponentNonlandPermanents)
    }) || p.modes.iter().any(|mode| contains_destroy(&mode.program))
}

fn program_shape(p: &SpellProgram, timing: SpellTiming, resolution: SpellResolution) -> bool {
    p.functioning_zone == FunctioningZone::Stack
        && p.timing == timing
        && p.resolution == resolution
        && p.optional_additional_costs.is_empty()
        && p.additional_sacrifice.is_none()
        && p.targets.is_empty()
        && p.modes.is_empty()
}

pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    if d.spell.as_ref().is_some_and(contains_destroy)
        || d.activated.iter().any(|a| contains_destroy(&a.program))
    {
        return Err(INVALID_FAMILY.into());
    }
    let reserved = d.triggered.iter().any(|a| contains_destroy(&a.program));
    if !reserved && !d.provenance.source_json.contains(DESTROY_TEXT) {
        return Ok(());
    }
    let source: Value = strict_read(&d.provenance.source_json)?;
    if !reserved
        && !source["oracle_text"]
            .as_str()
            .is_some_and(|text| text.contains(DESTROY_TEXT))
    {
        return Ok(());
    }

    let [trigger] = d.triggered.as_slice() else {
        return Err(INVALID_FAMILY.into());
    };
    let spell = d.spell.as_ref().ok_or(INVALID_FAMILY)?;
    let abilities: Value = strict_read(&d.provenance.abilities_json)?;
    if source["layout"] != "normal"
        || source["name"] != d.name
        || source["oracle_text"] != format!("Trample\nWhen {} dies, {DESTROY_TEXT}", d.name)
        || abilities != json!([
            {"kind":"keywords","keywords":[{"keyword":"trample"}]},
            {"kind":"triggered","trigger":{"trigger":"dies","what":{"ref":"self"}},
             "effects":[{"steps":[{"effect":"destroy","what":{"ref":"all","filter":{
                "nonTypes":["land"],"cls":"permanent"
             }},"regenerationForbidden":true}]}]}
        ])
        || d.characteristics.kind != CardKind::Creature
        || !d.characteristics.additional_types.is_empty()
        || d.intrinsic_zone.is_some()
        || d.x_mana_cost
        || d.keywords != [KeywordAbility::Trample]
        || !d.static_abilities.is_empty()
        || !d.activated.is_empty()
        || !d.mana_abilities.is_empty()
        || !program_shape(spell, SpellTiming::MainPhase, SpellResolution::Permanent)
        || !spell.instructions.is_empty()
        || trigger.functioning_zone != FunctioningZone::Battlefield
        || trigger.condition != TriggerCondition::SelfDies
        || trigger.death_filter.is_some()
        || trigger.intervening_if.is_some()
        || !program_shape(&trigger.program, SpellTiming::Instant, SpellResolution::Instructions)
        || !matches!(
            trigger.program.instructions.as_slice(),
            [Instruction::DestroySet {
                set: ObjectSet::NonlandPermanents,
                regeneration_forbidden: true,
                source_block: 1,
            }]
        )
    {
        return Err(INVALID_FAMILY.into());
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    let nonopponent_destroy = json!({"required":["op","set"],"properties":{
        "op":{"const":"destroy_set"},
        "set":{"not":{"const":ObjectSet::OpponentNonlandPermanents}}
    }});
    let free = json!({"$ref":"#/$defs/SourceDiesNonlandDestroyFreeProgram"});
    schema["$defs"]["SourceDiesNonlandDestroyFreeProgram"] = json!({"properties":{
        "instructions":{"items":{"not":nonopponent_destroy}},
        "modes":{"items":{"properties":{"program":free}}}
    }});
    let program = |timing, resolution, instructions| json!({"type":"object","properties":{
        "functioning_zone":{"const":FunctioningZone::Stack},"timing":{"const":timing},
        "resolution":{"const":resolution},"targets":{"maxItems":0},"modes":{"maxItems":0},
        "additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},
        "instructions":{"const":instructions}
    }});
    let spell = program(SpellTiming::MainPhase, SpellResolution::Permanent, json!([]));
    let trigger = program(SpellTiming::Instant, SpellResolution::Instructions, json!([{
        "op":"destroy_set","set":ObjectSet::NonlandPermanents,"source_block":1,
        "regeneration_forbidden":true
    }]));
    schema["$defs"]["Definition"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "properties":{
                "spell":free,
                "activated":{"items":{"properties":{"program":free}}}
            },
            "if":{"anyOf":[
                {"required":["triggered"],"properties":{"triggered":{"contains":{
                    "required":["program"],"properties":{"program":{"not":free}}
                }}}},
                {"required":["provenance"],"properties":{"provenance":{"properties":{
                    "source_json":{"pattern":r#""oracle_text"\s*:\s*"[^"]*destroy all nonland permanents\. They can't be regenerated\."#}
                }}}}
            ]},
            "then":{"required":["spell","triggered"],"properties":{
                "spell":spell,
                "triggered":{"minItems":1,"maxItems":1,"items":{"properties":{
                    "functioning_zone":{"const":FunctioningZone::Battlefield},
                    "condition":{"const":TriggerCondition::SelfDies},
                    "death_filter":{"type":"null"},"intervening_if":{"type":"null"},
                    "program":trigger
                }}},
                "characteristics":{"properties":{
                    "kind":{"const":CardKind::Creature},"additional_types":{"maxItems":0}
                }},
                "intrinsic_zone":{"type":"null"},"x_mana_cost":{"const":false},
                "activated":{"maxItems":0},"mana_abilities":{"maxItems":0},
                "keywords":{"const":[KeywordAbility::Trample]},"static_abilities":{"maxItems":0},
                "capabilities":{"allOf":[
                    {"contains":{"const":Capability::BoardWipesV1}},
                    {"contains":{"const":Capability::SelfDiesV1}},
                    {"contains":{"const":Capability::TriggeredAbilityV1}}
                ]}
            }}
        }));
}
