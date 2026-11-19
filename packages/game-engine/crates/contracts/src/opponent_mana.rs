//! CR 106.7 and 605.1a: one colored mana based on opponents' land output.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

pub fn opponent_land_mana_family(definition: &Definition) -> bool {
    let [ability] = definition.mana_abilities.as_slice() else {
        return false;
    };
    let c = &definition.characteristics;
    let source_shape = match c.kind {
        CardKind::Land => {
            definition.intrinsic_zone == Some(FunctioningZone::Battlefield)
                && definition.spell.is_none()
                && c.mana_cost.generic == Quantity::zero()
        }
        CardKind::Artifact => {
            definition.intrinsic_zone.is_none()
                && c.mana_cost.generic == Quantity::from(2)
                && definition.spell.as_ref().is_some_and(|spell| {
                    spell.resolution == SpellResolution::Permanent
                        && spell.timing == SpellTiming::MainPhase
                        && spell.functioning_zone == FunctioningZone::Stack
                        && spell.instructions.is_empty()
                        && spell.targets.is_empty()
                        && spell.modes.is_empty()
                        && spell.optional_additional_costs.is_empty()
                        && spell.additional_sacrifice.is_none()
                })
        }
        _ => false,
    };
    source_shape
        && c.additional_types.is_empty()
        && c.subtypes.is_empty()
        && !c.basic
        && !c.legendary
        && !c.snow
        && c.mana_cost.colored.is_empty()
        && c.colors.is_empty()
        && c.color_identity.is_empty()
        && c.power.is_none()
        && c.toughness.is_none()
        && definition.keywords.is_empty()
        && definition.activated.is_empty()
        && definition.triggered.is_empty()
        && definition.static_abilities.is_empty()
        && !definition.x_mana_cost
        && ability.source_block == 0
        && ability.costs.tap
        && ability.costs.mana.is_none()
        && !ability.costs.sacrifice
        && ability.costs.remove_source_plus_one_counters.is_none()
        && ability.restriction.is_none()
        && ability.self_damage.is_none()
        && ability.production == ManaProduction::OpponentLandColors
}

pub(super) fn extend_schema(schema: &mut serde_json::Value) {
    use serde_json::json;

    let production = json!({"const":ManaProduction::OpponentLandColors});
    let selected = json!({"required":["mana_abilities"],"properties":{
        "mana_abilities":{"contains":{"properties":{"production":production}}}
    }});
    let exact_capabilities = |kind_capability| {
        let capabilities = [
            Capability::NormalLayoutV1,
            Capability::PermanentManaV1,
            Capability::OpponentLandManaV1,
            kind_capability,
        ];
        json!({
            "items":{"enum":capabilities},
            "allOf":capabilities.map(|capability| json!({"contains":{"const":capability}}))
        })
    };
    schema["$defs"]["Definition"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "if":selected,
            "then":{
                "properties":{
                    "characteristics":{"properties":{
                        "additional_types":{"maxItems":0},"subtypes":{"maxItems":0},
                        "basic":{"const":false},"legendary":{"const":false},"snow":{"const":false},
                        "colors":{"maxItems":0},"color_identity":{"maxItems":0},
                        "power":{"type":"null"},"toughness":{"type":"null"}
                    }},
                    "keywords":{"maxItems":0},"activated":{"maxItems":0},
                    "triggered":{"maxItems":0},"static_abilities":{"maxItems":0},
                    "x_mana_cost":{"const":false},
                    "mana_abilities":{"minItems":1,"maxItems":1,"items":{"properties":{
                        "source_block":{"const":0},"restriction":{"type":"null"},
                        "self_damage":{"type":"null"},"production":production,
                        "costs":{"properties":{
                            "tap":{"const":true},"sacrifice":{"const":false},
                            "mana":{"type":"null"},"remove_source_plus_one_counters":{"type":"null"}
                        }}
                    }}}
                },
                "oneOf":[
                    {"required":["intrinsic_zone"],"properties":{
                        "characteristics":{"properties":{"kind":{"const":"land"},
                            "mana_cost":{"const":{"generic":"0","colored":[]}}
                        }},
                        "intrinsic_zone":{"const":"battlefield"},"spell":{"type":"null"},
                        "capabilities":exact_capabilities(Capability::LandPlayV1)
                    }},
                    {"required":["spell"],"properties":{
                        "characteristics":{"properties":{"kind":{"const":"artifact"},
                            "mana_cost":{"const":{"generic":"2","colored":[]}}
                        }},
                        "intrinsic_zone":{"type":"null"},
                        "spell":{"type":"object","properties":{
                            "timing":{"const":"main_phase"},"resolution":{"const":"permanent"},
                            "functioning_zone":{"const":"stack"},"instructions":{"maxItems":0},
                            "targets":{"maxItems":0},"modes":{"maxItems":0},
                            "additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0}
                        }},
                        "capabilities":exact_capabilities(Capability::CastArtifactV1)
                    }}
                ]
            },
            "else":{"properties":{
                "capabilities":{"not":{"contains":{"const":Capability::OpponentLandManaV1}}}
            }}
        }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let mut definition = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-mana-ir.json"
    ))
    .unwrap()
    .definitions
    .into_iter()
    .find(|definition| definition.name == "Command Tower")
    .unwrap();
    definition.source_labels = None;
    definition.mana_abilities[0].production = ManaProduction::OpponentLandColors;
    definition.capabilities = definition.requirements();
    definition
}
