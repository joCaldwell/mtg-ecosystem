use super::*;
use serde_json::{Value, json};

pub(super) fn validate_definition(definition: &Definition) -> Result<(), String> {
    let adjustment = definition
        .static_abilities
        .iter()
        .find_map(|ability| match &ability.effect {
            StaticEffect::ReduceMaximumHandSize { scope, amount } => Some((
                amount,
                (*scope == HandSizeScope::Controller).then_some(KeywordAbility::Flying),
                "unsupported complete reduced hand size family",
            )),
            StaticEffect::IncreaseMaximumHandSize { amount } => Some((
                amount,
                Some(KeywordAbility::Defender),
                "unsupported complete increased hand size family",
            )),
            _ => None,
        });
    let Some((amount, optional_keyword, error)) = adjustment else {
        return Ok(());
    };
    if flash_end_step_draw::definition_matches(definition) {
        return Ok(());
    }
    let characteristics = &definition.characteristics;
    if !matches!(
        definition.static_abilities.as_slice(),
        [StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            ..
        }]
    ) || amount <= &Quantity::zero()
        || !(definition.keywords.is_empty()
            || optional_keyword.is_some_and(|keyword| definition.keywords == [keyword]))
        || characteristics.kind != CardKind::Creature
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
        return Err(error.into());
    }
    // Ordinary creature validation also checks the complete cast program.
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    use Capability::*;
    extend_adjustment_schema(
        schema,
        "reduce_maximum_hand_size",
        ReducedHandSizeV1,
        KeywordAbility::Flying,
        FlyingV1,
        json!({
            "properties":{"effect":{"properties":{"reduce_maximum_hand_size":{
                "properties":{"scope":{"const":HandSizeScope::Controller}}
            }}}}
        }),
    );
    extend_adjustment_schema(
        schema,
        "increase_maximum_hand_size",
        IncreasedHandSizeV1,
        KeywordAbility::Defender,
        DefenderV1,
        json!({}),
    );
}

fn extend_adjustment_schema(
    schema: &mut Value,
    effect_tag: &str,
    capability: Capability,
    keyword: KeywordAbility,
    keyword_capability: Capability,
    keyword_ability_constraint: Value,
) {
    use Capability::*;
    let adjustment_ability = json!({
        "required":["effect"],
        "properties":{"effect":{"type":"object","required":[effect_tag]}}
    });
    let disallowed_subtypes: Vec<_> = UNSUPPORTED_RULES_SUBTYPES
        .iter()
        .chain(unlimited_hand_size::DISALLOWED_CREATURE_SUBTYPES)
        .collect();
    let base_capabilities = [NormalLayoutV1, BaseCreatureV1, CastCreatureV1, capability];
    let capabilities = |allowed: &[Capability]| {
        json!({"minItems":allowed.len(),"maxItems":allowed.len(),
            "uniqueItems":true,"items":{"enum":allowed}})
    };
    let keyword_capabilities = [base_capabilities.as_slice(), &[keyword_capability]].concat();
    schema["$defs"]["StaticAbility"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "if":adjustment_ability,
            "then":{"properties":{"functioning_zone":{"const":FunctioningZone::Battlefield}}}
        }));
    let ordinary = json!({"required":["spell","static_abilities"],"properties":{
        "static_abilities":{"minItems":1,"maxItems":1,"items":adjustment_ability},
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
            "keywords":{"const":[keyword]},
            "capabilities":capabilities(&keyword_capabilities),
            "static_abilities":{"items":keyword_ability_constraint}
        }}
    ]});
    let family = if capability == ReducedHandSizeV1 {
        json!({"oneOf":[ordinary,{"$ref":"#/$defs/FlashEndStepDrawDefinition"}]})
    } else {
        ordinary
    };
    schema["$defs"]["Definition"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "if":{"anyOf":[
                {"required":["static_abilities"],"properties":{
                    "static_abilities":{"contains":adjustment_ability}
                }},
                {"required":["capabilities"],"properties":{
                    "capabilities":{"contains":{"const":capability}}
                }}
            ]},
            "then":family
        }));
}

#[cfg(test)]
pub(super) fn reduced_witness() -> Definition {
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
    let opponents = reduced_witness();
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
    let mut missing_capability = reduced_witness();
    missing_capability
        .capabilities
        .remove(&Capability::ReducedHandSizeV1);
    assert!(missing_capability.validate().is_err());
    let mut extra_capability = reduced_witness();
    extra_capability.static_abilities.clear();
    assert!(extra_capability.validate().is_err());
}

#[cfg(test)]
pub(super) fn increased_witness() -> Definition {
    let mut creature = unlimited_hand_size::witness();
    creature.static_abilities[0].effect = StaticEffect::IncreaseMaximumHandSize {
        amount: Quantity::from(1),
    };
    creature.keywords.push(KeywordAbility::Defender);
    creature.capabilities = creature.requirements();
    creature
}

#[cfg(test)]
#[test]
fn increased_hand_size() {
    let defender = increased_witness();
    assert!(defender.validate().is_ok());
    let mut bare = defender.clone();
    bare.keywords.clear();
    bare.capabilities = bare.requirements();
    assert!(bare.validate().is_ok());

    let serialized = json!({"increase_maximum_hand_size":{"amount":"1"}});
    assert_eq!(json!(defender.static_abilities[0].effect), serialized);
    assert_eq!(
        serde_json::from_value::<StaticEffect>(serialized).unwrap(),
        defender.static_abilities[0].effect
    );
    for invalid_effect in [
        json!({"increase_maximum_hand_size":{"amount":"01"}}),
        json!({"increase_maximum_hand_size":{"amount":"1","scope":"controller"}}),
    ] {
        assert!(serde_json::from_value::<StaticEffect>(invalid_effect).is_err());
    }

    let mut invalid = [
        bare.clone(),
        bare.clone(),
        bare.clone(),
        bare.clone(),
        bare.clone(),
        defender,
    ];
    invalid[0].static_abilities[0].effect = StaticEffect::IncreaseMaximumHandSize {
        amount: Quantity::zero(),
    };
    invalid[1].static_abilities[0].effect = StaticEffect::IncreaseMaximumHandSize {
        amount: Quantity::from(-1),
    };
    invalid[2].static_abilities[0].functioning_zone = FunctioningZone::Stack;
    invalid[3].characteristics.legendary = true;
    invalid[4].spell = None;
    invalid[5].keywords.push(KeywordAbility::Flying);
    for definition in &mut invalid {
        definition.capabilities = definition.requirements();
        assert!(definition.validate().is_err());
    }
    let mut missing_capability = bare.clone();
    missing_capability
        .capabilities
        .remove(&Capability::IncreasedHandSizeV1);
    assert!(missing_capability.validate().is_err());
    bare.static_abilities.clear();
    assert!(bare.validate().is_err());
}
