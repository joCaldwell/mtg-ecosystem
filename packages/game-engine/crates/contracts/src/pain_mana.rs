//! CR 106.1b, 109.5 and 605.1a: select one mana output and its disjoint
//! ability. The colored ability deals damage to the activator as an effect.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

pub fn disjoint_pain_mana_family(definition: &Definition) -> bool {
    let [safe, colored] = definition.mana_abilities.as_slice() else {
        return false;
    };
    let ManaProduction::Choice { colors } = &colored.production else {
        return false;
    };
    let characteristics = &definition.characteristics;
    let source_shape = match characteristics.kind {
        CardKind::Land => {
            definition.intrinsic_zone == Some(FunctioningZone::Battlefield)
                && definition.spell.is_none()
        }
        CardKind::Artifact => {
            definition.intrinsic_zone.is_none()
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
        && characteristics.additional_types.is_empty()
        && characteristics.subtypes.is_empty()
        && !characteristics.basic
        && !characteristics.snow
        && definition.keywords.is_empty()
        && definition.activated.is_empty()
        && definition.triggered.is_empty()
        && definition.static_abilities.is_empty()
        && !definition.x_mana_cost
        && safe.source_block == 0
        && colored.source_block == 1
        && [safe, colored].iter().all(|ability| {
            ability.costs.tap
                && ability.costs.mana.is_none()
                && !ability.costs.sacrifice
                && ability.costs.remove_source_plus_one_counters.is_none()
                && ability.restriction.is_none()
        })
        && matches!(&safe.production, ManaProduction::Fixed { symbols } if symbols.as_slice() == [Color::C])
        && safe.self_damage.is_none()
        && colored
            .self_damage
            .as_ref()
            .is_some_and(|n| n > &Quantity::zero())
        && (2..=5).contains(&colors.len())
        && colors.iter().all(|color| *color != Color::C)
        && colors.iter().collect::<BTreeSet<_>>().len() == colors.len()
}

pub(super) fn extend_schema(schema: &mut serde_json::Value) {
    use serde_json::json;

    let selected = json!({"required":["mana_abilities"],"properties":{
        "mana_abilities":{"anyOf":[{"minItems":2},{"contains":{
            "required":["self_damage"],"properties":{"self_damage":{"type":"string"}}
        }}]}
    }});
    let selected = json!({"allOf":[selected,{"not":hybrid_filter_mana::selector()}]});
    let tap_only = json!({"properties":{
        "tap":{"const":true},"mana":{"type":"null"},"sacrifice":{"const":false},
        "remove_source_plus_one_counters":{"type":"null"}
    }});
    let exact_capabilities = |kind_capability| {
        let capabilities = [
            Capability::NormalLayoutV1,
            Capability::PermanentManaV1,
            Capability::DisjointPainManaV1,
            Capability::NoncombatDamageV1,
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
                        "basic":{"const":false},"snow":{"const":false}
                    }},
                    "keywords":{"maxItems":0},"activated":{"maxItems":0},
                    "triggered":{"maxItems":0},"static_abilities":{"maxItems":0},
                    "x_mana_cost":{"const":false},
                    "mana_abilities":{"minItems":2,"items":false,"prefixItems":[
                        {"properties":{
                            "source_block":{"const":0},"costs":tap_only,"restriction":{"type":"null"},
                            "self_damage":{"type":"null"},
                            "production":{"const":{"mana":"fixed","symbols":["C"]}}
                        }},
                        {"required":["self_damage"],"properties":{
                            "source_block":{"const":1},"costs":tap_only,"restriction":{"type":"null"},
                            "self_damage":{"type":"string","pattern":"^[1-9][0-9]*$"},
                            "production":{"properties":{"mana":{"const":"choice"},"colors":{
                                "minItems":2,"maxItems":5,"uniqueItems":true,
                                "items":{"enum":["W","U","B","R","G"]}
                            }}}
                        }}
                    ]}
                },
                "oneOf":[
                    {"required":["intrinsic_zone"],"properties":{
                        "characteristics":{"properties":{"kind":{"const":"land"}}},
                        "intrinsic_zone":{"const":"battlefield"},"spell":{"type":"null"},
                        "capabilities":exact_capabilities(Capability::LandPlayV1)
                    }},
                    {"required":["spell"],"properties":{
                        "characteristics":{"properties":{"kind":{"const":"artifact"}}},
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
                "capabilities":{"not":{"contains":{"const":Capability::DisjointPainManaV1}}}
            }}
        }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    // The source-backed compiler witness is separate from this synthetic IR
    // entry in the capability registry.
    let mut definition = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-mana-ir.json"
    ))
    .unwrap()
    .definitions
    .into_iter()
    .find(|definition| definition.name == "Command Tower")
    .unwrap();
    definition.source_labels = None;
    let safe = &mut definition.mana_abilities[0];
    safe.production = ManaProduction::Fixed {
        symbols: vec![Color::C],
    };
    let colored = ManaAbility {
        self_damage: Some(2.into()),
        production: ManaProduction::Choice {
            colors: vec![Color::W, Color::U, Color::G],
        },
        source_block: 1,
        ..safe.clone()
    };
    definition.mana_abilities.push(colored);
    definition.capabilities = definition.requirements();
    definition
}
