use super::*;
use serde_json::{Value, json};

pub(super) fn validate_definition(definition: &Definition) -> Result<bool, String> {
    let has_mana = definition.mana_abilities.iter().any(|ability| {
        matches!(ability.production, ManaProduction::EachPermanentColor { .. })
    });
    let has_stats = definition.static_abilities.iter().any(|ability| {
        matches!(
            ability.effect,
            StaticEffect::ModifySourceByDistinctPermanentColors { .. }
        )
    });
    if !has_mana && !has_stats {
        return Ok(false);
    }
    let bad = || "unsupported complete distinct permanent colors family".to_owned();
    let [ManaAbility {
        costs,
        production: ManaProduction::EachPermanentColor { scope: mana_scope },
        restriction: None,
        self_damage: None,
        source_block: 2,
    }] = definition.mana_abilities.as_slice()
    else {
        return Err(bad());
    };
    let [StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect:
            StaticEffect::ModifySourceByDistinctPermanentColors {
                scope,
                power_per_color,
                toughness_per_color,
            },
    }] = definition.static_abilities.as_slice()
    else {
        return Err(bad());
    };
    let c = &definition.characteristics;
    if scope != mana_scope
        || power_per_color <= &Quantity::zero()
        || toughness_per_color <= &Quantity::zero()
        || !costs.tap
        || costs.mana.is_some()
        || costs.sacrifice
        || costs.remove_source_plus_one_counters.is_some()
        || c.kind != CardKind::Creature
        || !c.additional_types.is_empty()
        || c.legendary
        || c.snow
        || c.toughness.as_ref().is_none_or(Quantity::is_negative)
        || definition.x_mana_cost
        || definition.keywords != [KeywordAbility::Vigilance]
        || !definition.activated.is_empty()
        || !definition.triggered.is_empty()
        || !definition.spell.as_ref().is_some_and(|spell| {
            spell.additional_sacrifice.is_none() && spell.optional_additional_costs.is_empty()
        })
        || definition.source_labels.as_ref().is_some_and(|labels| {
            labels.blocks.len() != 3 || labels.mana != [2] || labels.statics != [1]
        })
    {
        return Err(bad());
    }
    Ok(true)
}

pub(super) fn extend_schema(schema: &mut Value) {
    let production = json!({"const": ManaProduction::EachPermanentColor {
        scope: PermanentColorScope::Controller,
    }});
    let stats = json!({
        "type":"object","required":["modify_source_by_distinct_permanent_colors"]
    });
    let capabilities = [
        Capability::NormalLayoutV1,
        Capability::BaseCreatureV1,
        Capability::CastCreatureV1,
        Capability::PermanentManaV1,
        Capability::VigilanceV1,
        Capability::DistinctPermanentColorsV1,
    ];
    let positive = json!({"pattern":"^[1-9][0-9]*$"});
    schema["$defs"]["Definition"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "if":{"anyOf":[
                {"required":["mana_abilities"],"properties":{
                    "mana_abilities":{"contains":{"properties":{"production":production}}}
                }},
                {"required":["static_abilities"],"properties":{
                    "static_abilities":{"contains":{"properties":{"effect":stats}}}
                }}
            ]},
            "then":{
                "required":["mana_abilities","static_abilities","keywords"],
                "properties":{
                    "characteristics":{"properties":{
                        "kind":{"const":"creature"},"additional_types":{"maxItems":0},
                        "basic":{"const":false},"legendary":{"const":false},"snow":{"const":false},
                        "power":{"type":"string"},
                        "toughness":{"type":"string","pattern":"^(0|[1-9][0-9]*)$"}
                    }},
                    "keywords":{"const":[KeywordAbility::Vigilance]},
                    "activated":{"maxItems":0},"triggered":{"maxItems":0},
                    "x_mana_cost":{"const":false},"intrinsic_zone":{"type":"null"},
                    "spell":{"type":"object","properties":{
                        "timing":{"const":"main_phase"},"resolution":{"const":"permanent"},
                        "functioning_zone":{"const":"stack"},"additional_sacrifice":{"type":"null"},
                        "optional_additional_costs":{"maxItems":0},"instructions":{"maxItems":0},
                        "modes":{"maxItems":0},"targets":{"maxItems":0}
                    }},
                    "mana_abilities":{"minItems":1,"maxItems":1,"items":{"properties":{
                        "production":production,"source_block":{"const":2},
                        "restriction":{"type":"null"},"self_damage":{"type":"null"},
                        "costs":{"properties":{
                            "tap":{"const":true},"sacrifice":{"const":false},
                            "mana":{"type":"null"},"remove_source_plus_one_counters":{"type":"null"}
                        }}
                    }}},
                    "static_abilities":{"minItems":1,"maxItems":1,"items":{"properties":{
                        "functioning_zone":{"const":"battlefield"},
                        "effect":{"allOf":[stats,{"properties":{
                            "modify_source_by_distinct_permanent_colors":{"properties":{
                                "scope":{"const":PermanentColorScope::Controller},
                                "power_per_color":positive,"toughness_per_color":positive
                            }}
                        }}]}
                    }}},
                    "source_labels":{"properties":{
                        "blocks":{"minItems":3,"maxItems":3},
                        "mana":{"const":[2]},"statics":{"const":[1]}
                    }},
                    "capabilities":{
                        "items":{"enum":capabilities},
                        "allOf":capabilities.map(|capability| json!({"contains":{"const":capability}}))
                    }
                }
            },
            "else":{"properties":{
                "capabilities":{"not":{"contains":{"const":Capability::DistinctPermanentColorsV1}}}
            }}
        }));
}
