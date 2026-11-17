//! Pure sacrifice-source / controlled-creature keyword composition. No source
//! identity, quantity or individual keyword selects an activation mechanism.
use super::*;

/// CR 602.2b, 608.2h, 611.2c: pay the source, then capture one complete set at
/// resolution through the same program used by ordinary temporary set spells.
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
pub fn sacrifice_source_creature_set_keyword_ability(
    a: &ActivatedAbility,
) -> Option<TemporarySetKeywordProgram<'_>> {
    if a.functioning_zone != FunctioningZone::Battlefield
        || a.cost != ActivationCost::SacrificeSource
        || a.program.timing != SpellTiming::Instant
    {
        return None;
    }
    let p = temporary_creature_set_keyword_program(&a.program)?;
    (*p.set == ObjectSet::ControllerCreatures && p.modifier.is_none()).then_some(p)
}

pub(super) fn definition(d: &Definition) -> bool {
    d.characteristics.kind == CardKind::Creature
        && d.characteristics.additional_types.is_empty()
        && !d.characteristics.legendary
        && !d.characteristics.basic
        && d.spell.is_none()
        && d.intrinsic_zone.is_none()
        && !d.x_mana_cost
        && d.keywords.is_empty()
        && d.static_abilities.is_empty()
        && d.mana_abilities.is_empty()
        && d.triggered.is_empty()
        && matches!(d.activated.as_slice(), [a]
            if sacrifice_source_creature_set_keyword_ability(a).is_some())
}

pub(super) fn schema(s: &mut serde_json::Value) {
    use serde_json::json;
    let op = json!({"required":["op"],"properties":{"op":{"const":"grant_keywords_set"}}});
    s["$defs"]["HasActivatedSetKeyword"] = json!({"required":["activated"],"properties":{
        "activated":{"contains":{"required":["program"],"properties":{"program":{
            "not":{"$ref":"#/$defs/KeywordGrantFreeProgram"}
        }}}}
    }});
    s["$defs"]["SacrificeSourceCreatureSetKeywordAbility"] = json!({"required":["cost","program"],"properties":{
        "functioning_zone":{"const":"battlefield"},"cost":{"const":"sacrifice_source"},
        "program":{"properties":{
            "optional_additional_costs":{"maxItems":0},"additional_sacrifice":{"type":"null"},
            "modes":{"maxItems":0},"targets":{"maxItems":0},"timing":{"const":"instant"},
            "functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"},
            "instructions":{"minItems":1,"maxItems":1,"items":{"allOf":[op,{"properties":{
                "set":{"const":ObjectSet::ControllerCreatures},"modifier":{"type":"null"},
                "keywords":{"$ref":"#/$defs/TemporaryCreatureKeywords"},
                "duration":{"const":"until_end_of_turn"},"source_block":{"const":0}
            }}]}}
        }}
    }});
    let mut caps = vec![
        Capability::NormalLayoutV1,
        Capability::BaseCreatureV1,
        Capability::CastCreatureV1,
        Capability::ActivatedAbilityV1,
        Capability::SacrificeSourceCostV1,
        Capability::BoardWipesV1,
        Capability::TemporaryCreatureSetKeywordProgramV1,
    ];
    let exact: Vec<_> = caps
        .iter()
        .map(|cap| json!({"contains":{"const":cap}}))
        .collect();
    let mut primitive_rules = Vec::new();
    // Derive the shared vocabulary so a reviewed common-keyword extension does
    // not need a second activation whitelist or a new per-source capability.
    for value in s["$defs"]["KeywordAbility"]["enum"].as_array().unwrap() {
        let keyword: KeywordAbility = serde_json::from_value(value.clone()).unwrap();
        if !supported_temporary_keywords(&[keyword]) {
            continue;
        }
        let cap = temporary_keyword_primitive_capability(keyword).unwrap();
        caps.push(cap);
        let present = json!({"properties":{"activated":{"contains":{"properties":{"program":{"properties":{
            "instructions":{"contains":{"properties":{"keywords":{"contains":{"const":keyword}}}}}
        }}}}}}});
        primitive_rules.push(json!({"if":present,
            "then":{"properties":{"capabilities":{"contains":{"const":cap}}}},
            "else":{"properties":{"capabilities":{"not":{"contains":{"const":cap}}}}}
        }));
    }
    s["$defs"]["SacrificeSourceCreatureSetKeywordDefinition"] = json!({"allOf":[
        {"properties":{
            "characteristics":{"properties":{"kind":{"const":"creature"},"additional_types":{"maxItems":0},
                "legendary":{"const":false},"basic":{"const":false}}},
            "spell":{"type":"null"},"intrinsic_zone":{"type":"null"},"x_mana_cost":{"const":false},
            "keywords":{"maxItems":0},"static_abilities":{"maxItems":0},"mana_abilities":{"maxItems":0},"triggered":{"maxItems":0},
            "activated":{"minItems":1,"maxItems":1,"items":{"$ref":"#/$defs/SacrificeSourceCreatureSetKeywordAbility"}},
            "capabilities":{"items":{"enum":caps},"allOf":exact}
        }},
        {"allOf":primitive_rules}
    ]});
}
