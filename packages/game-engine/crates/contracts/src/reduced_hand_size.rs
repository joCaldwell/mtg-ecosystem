use super::*;
use serde_json::{Value, json};

pub(super) fn validate_definition(definition: &Definition) -> Result<(), String> {
    if !definition
        .static_abilities
        .iter()
        .any(|ability| matches!(ability.effect, StaticEffect::ReduceMaximumHandSize { .. }))
    {
        return Ok(());
    }

    let characteristics = &definition.characteristics;
    if !matches!(
        definition.static_abilities.as_slice(),
        [StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::ReduceMaximumHandSize { scope, amount },
        }] if amount > &Quantity::zero()
            && (definition.keywords.is_empty()
                || (*scope == HandSizeScope::Controller
                    && definition.keywords == [KeywordAbility::Flying]))
    ) || characteristics.kind != CardKind::Creature
        || !characteristics.additional_types.is_empty()
        || characteristics.basic
        || characteristics.legendary
        || characteristics.snow
        || characteristics.subtypes.iter().any(|subtype| {
            unlimited_hand_size::DISALLOWED_CREATURE_SUBTYPES.contains(&subtype.as_str())
        })
        || definition.x_mana_cost
        || !definition.mana_abilities.is_empty()
        || !definition.activated.is_empty()
        || !definition.triggered.is_empty()
        || definition
            .spell
            .as_ref()
            .is_some_and(|spell| spell.additional_sacrifice.is_some())
    {
        return Err("unsupported complete reduced hand size family".into());
    }
    // Ordinary creature validation also checks the complete cast program.
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    use Capability::*;
    let reduction = json!({"type":"object","required":["reduce_maximum_hand_size"]});
    let reduction_ability = json!({
        "required":["effect"],"properties":{"effect":reduction}
    });
    let controller_reduction_ability = json!({
        "properties":{"effect":{"properties":{"reduce_maximum_hand_size":{
            "properties":{"scope":{"const":HandSizeScope::Controller}}
        }}}}
    });
    let disallowed_subtypes: Vec<_> = UNSUPPORTED_RULES_SUBTYPES
        .iter()
        .chain(unlimited_hand_size::DISALLOWED_CREATURE_SUBTYPES)
        .collect();
    let base_capabilities = [
        NormalLayoutV1,
        BaseCreatureV1,
        CastCreatureV1,
        ReducedHandSizeV1,
    ];
    let capabilities = |allowed: &[Capability]| {
        json!({"minItems":allowed.len(),"maxItems":allowed.len(),
            "uniqueItems":true,"items":{"enum":allowed}})
    };
    let flying_capabilities = [base_capabilities.as_slice(), &[FlyingV1]].concat();
    schema["$defs"]["StaticAbility"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "if":reduction_ability,
            "then":{"properties":{"functioning_zone":{"const":FunctioningZone::Battlefield}}}
        }));
    schema["$defs"]["Definition"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "if":{"anyOf":[
                {"required":["static_abilities"],"properties":{
                    "static_abilities":{"contains":reduction_ability}
                }},
                {"required":["capabilities"],"properties":{
                    "capabilities":{"contains":{"const":ReducedHandSizeV1}}
                }}
            ]},
            "then":{"required":["spell","static_abilities"],"properties":{
                "static_abilities":{"minItems":1,"maxItems":1,"items":reduction_ability},
                "x_mana_cost":{"const":false},
                "characteristics":{"properties":{
                    "kind":{"const":CardKind::Creature},"additional_types":{"maxItems":0},
                    "basic":{"const":false},"legendary":{"const":false},"snow":{"const":false},
                    "subtypes":{"items":{"not":{"enum":disallowed_subtypes}}},
                    "power":{"type":"string"},"toughness":{"type":"string","pattern":"^[1-9][0-9]*$"},
                    "mana_cost":{"properties":{"generic":{"pattern":"^(0|[1-9][0-9]*)$"}}},
                    "colors":{"items":{"not":{"const":Color::C}}},
                    "color_identity":{"items":{"not":{"const":Color::C}}}
                }},
                "intrinsic_zone":{"type":"null"},
                "spell":{"type":"object","properties":{
                    "timing":{"const":SpellTiming::MainPhase},
                    "resolution":{"const":SpellResolution::Permanent},
                    "functioning_zone":{"const":FunctioningZone::Stack},
                    "additional_sacrifice":{"type":"null"},
                    "instructions":{"maxItems":0},"modes":{"maxItems":0},"targets":{"maxItems":0}
                }},
                "mana_abilities":{"maxItems":0},"activated":{"maxItems":0},"triggered":{"maxItems":0}
            },"oneOf":[
                {"properties":{
                    "keywords":{"maxItems":0},"capabilities":capabilities(&base_capabilities)
                }},
                {"required":["keywords"],"properties":{
                    "keywords":{"const":[KeywordAbility::Flying]},
                    "capabilities":capabilities(&flying_capabilities),
                    "static_abilities":{"items":controller_reduction_ability}
                }}
            ]}
        }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let mut creature = unlimited_hand_size::witness();
    creature.static_abilities[0].effect = StaticEffect::ReduceMaximumHandSize {
        scope: HandSizeScope::Opponents,
        amount: Quantity::from(2),
    };
    creature.capabilities = creature.requirements();
    creature
}

#[cfg(test)]
#[test]
fn reduced_hand_size() {
    let opponents = witness();
    assert!(opponents.validate().is_ok());
    let mut controller = opponents.clone();
    controller.static_abilities[0].effect = StaticEffect::ReduceMaximumHandSize {
        scope: HandSizeScope::Controller,
        amount: Quantity::from(1),
    };
    controller.keywords.push(KeywordAbility::Flying);
    controller.capabilities = controller.requirements();
    assert!(controller.validate().is_ok());
    let serialized = json!({"reduce_maximum_hand_size":{"scope":"controller","amount":"1"}});
    assert_eq!(json!(controller.static_abilities[0].effect), serialized);
    assert_eq!(
        serde_json::from_value::<StaticEffect>(serialized).unwrap(),
        controller.static_abilities[0].effect
    );

    let mut invalid = [
        opponents.clone(),
        opponents.clone(),
        opponents.clone(),
        opponents,
        controller.clone(),
        controller,
    ];
    invalid[0].keywords.push(KeywordAbility::Flying);
    invalid[1].static_abilities.push(StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::UnlimitedControllerHandSize,
    });
    invalid[2].static_abilities[0].effect = StaticEffect::ReduceMaximumHandSize {
        scope: HandSizeScope::Opponents,
        amount: Quantity::zero(),
    };
    invalid[3].static_abilities[0].functioning_zone = FunctioningZone::Stack;
    invalid[4]
        .characteristics
        .additional_types
        .push(CardKind::Artifact);
    invalid[5].spell = None;
    for definition in &mut invalid {
        definition.capabilities = definition.requirements();
        assert!(definition.validate().is_err());
    }
    let mut missing_capability = witness();
    missing_capability
        .capabilities
        .remove(&Capability::ReducedHandSizeV1);
    assert!(missing_capability.validate().is_err());
    let mut extra_capability = witness();
    extra_capability.static_abilities.clear();
    assert!(extra_capability.validate().is_err());
}
