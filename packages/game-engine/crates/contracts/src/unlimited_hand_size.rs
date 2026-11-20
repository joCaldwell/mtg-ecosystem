use super::*;
use serde_json::{Value, json};

pub(super) const DISALLOWED_CREATURE_SUBTYPES: &[&str] = &[
    "Aura",
    "Book",
    "Equipment",
    "Plains",
    "Island",
    "Swamp",
    "Mountain",
    "Forest",
];

pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    if !d
        .static_abilities
        .iter()
        .any(|ability| ability.effect == StaticEffect::UnlimitedControllerHandSize)
    {
        return Ok(());
    }

    let c = &d.characteristics;
    let valid_kind = match (c.kind, d.mana_abilities.as_slice()) {
        (CardKind::Creature, []) => c
            .subtypes
            .iter()
            .all(|s| !DISALLOWED_CREATURE_SUBTYPES.contains(&s.as_str())),
        (CardKind::Artifact, [] | [_]) => c.subtypes.is_empty() || c.subtypes == ["Book"],
        (CardKind::Land, [_]) => c.subtypes.is_empty(),
        _ => false,
    };
    let valid_mana = d.mana_abilities.iter().all(|ability| {
        ability.costs.tap
            && ability.costs.mana.is_none()
            && !ability.costs.sacrifice
            && ability.costs.remove_source_plus_one_counters.is_none()
            && ability.restriction.is_none()
            && ability.self_damage.is_none()
            && match &ability.production {
                ManaProduction::Fixed { symbols } => symbols.as_slice() == [Color::C],
                ManaProduction::AnyColor => c.kind == CardKind::Artifact,
                ManaProduction::AnyOneColor { .. }
                | ManaProduction::CommanderColor
                | ManaProduction::OpponentLandColors
                | ManaProduction::Choice { .. }
                | ManaProduction::GreatestControlledPowerCombination
                | ManaProduction::EachPermanentColor { .. }
                | ManaProduction::HybridFilter { .. }
                | ManaProduction::FixedWithLifeGain { .. }
                | ManaProduction::RestrictedFixed { .. } => false,
            }
    });
    if !valid_kind
        || !valid_mana
        || !c.additional_types.is_empty()
        || c.basic
        || c.legendary
        || c.snow
        || d.x_mana_cost
        || !matches!(
            d.static_abilities.as_slice(),
            [StaticAbility {
                functioning_zone: FunctioningZone::Battlefield,
                effect: StaticEffect::UnlimitedControllerHandSize,
            }]
        )
        || !d.keywords.is_empty()
        || !d.activated.is_empty()
        || !d.triggered.is_empty()
        || d.spell.as_ref().is_some_and(|p| {
            p.additional_sacrifice.is_some() || !p.optional_additional_costs.is_empty()
        })
    {
        return Err("unsupported complete unlimited hand size family".into());
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    use Capability::*;
    let effect = StaticEffect::UnlimitedControllerHandSize;
    let static_ability = json!({"functioning_zone":FunctioningZone::Battlefield,"effect":effect});
    let fixed_mana = ManaProduction::Fixed {
        symbols: vec![Color::C],
    };
    let permanent_spell = json!({
        "type":"object","properties":{
            "timing":{"const":SpellTiming::MainPhase},
            "resolution":{"const":SpellResolution::Permanent},
            "functioning_zone":{"const":FunctioningZone::Stack},
            "additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},
            "instructions":{"maxItems":0},"modes":{"maxItems":0},"targets":{"maxItems":0}
        }
    });
    let creature_subtypes = UNSUPPORTED_RULES_SUBTYPES
        .iter()
        .chain(DISALLOWED_CREATURE_SUBTYPES)
        .collect::<Vec<_>>();
    let branches = [
        (
            CardKind::Creature,
            vec![BaseCreatureV1, CastCreatureV1],
            false,
            json!({"items":{"not":{"enum":creature_subtypes}}}),
        ),
        (
            CardKind::Artifact,
            vec![CastArtifactV1],
            false,
            json!({"enum":[[],["Book"]]}),
        ),
        (
            CardKind::Artifact,
            vec![CastArtifactV1, PermanentManaV1],
            true,
            json!({"enum":[[],["Book"]]}),
        ),
        (
            CardKind::Land,
            vec![LandPlayV1, PermanentManaV1],
            true,
            json!({"const":[]}),
        ),
    ]
    .map(|(kind, mut capabilities, has_mana, subtypes)| {
        capabilities.extend([NormalLayoutV1, UnlimitedHandSizeV1]);
        let required = if has_mana {
            vec!["spell", "mana_abilities"]
        } else {
            vec!["spell"]
        };
        let mut shape = json!({"required":required,"properties":{
            "characteristics":{"properties":{"kind":{"const":kind},"subtypes":subtypes}},
            "intrinsic_zone":{"type":"null"},"spell":permanent_spell,
            "mana_abilities":{"maxItems":0},
            "capabilities":{"minItems":capabilities.len(),"maxItems":capabilities.len(),
                "uniqueItems":true,"items":{"enum":capabilities}}
        }});
        if has_mana {
            let production = if kind == CardKind::Artifact {
                json!({"enum":[fixed_mana,ManaProduction::AnyColor]})
            } else {
                json!({"const":fixed_mana})
            };
            shape["properties"]["mana_abilities"] = json!({
                "minItems":1,"maxItems":1,"items":{"properties":{
                    "costs":{"properties":{
                        "tap":{"const":true},"mana":{"type":"null"},"sacrifice":{"const":false}
                    }},
                    "production":production
                }}
            });
        }
        let characteristics = &mut shape["properties"]["characteristics"]["properties"];
        if kind == CardKind::Creature {
            characteristics["power"] = json!({"type":"string"});
            characteristics["toughness"] = json!({"type":"string","pattern":"^[1-9][0-9]*$"});
        } else {
            characteristics["power"] = json!({"type":"null"});
            characteristics["toughness"] = json!({"type":"null"});
        }
        if kind == CardKind::Land {
            characteristics["mana_cost"] = json!({"const":{"generic":"0","colored":[]}});
            shape["properties"]["intrinsic_zone"] = json!({"const":FunctioningZone::Battlefield});
            shape["properties"]["spell"] = json!({"type":"null"});
        }
        shape
    });
    schema["$defs"]["StaticAbility"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "if":{"required":["effect"],"properties":{"effect":{"const":effect}}},
            "then":{"properties":{"functioning_zone":{"const":FunctioningZone::Battlefield}}}
        }));
    schema["$defs"]["Definition"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "if":{"anyOf":[
                {"required":["static_abilities"],"properties":{"static_abilities":{
                    "contains":{"required":["effect"],"properties":{"effect":{"const":effect}}}
                }}},
                {"required":["capabilities"],"properties":{"capabilities":{
                    "contains":{"const":UnlimitedHandSizeV1}
                }}}
            ]},
            "then":{"required":["static_abilities"],"properties":{
                "static_abilities":{"const":[static_ability]},
                "x_mana_cost":{"const":false},
                "characteristics":{"properties":{
                    "additional_types":{"maxItems":0},"basic":{"const":false},
                    "legendary":{"const":false},"snow":{"const":false},
                    "mana_cost":{"properties":{"generic":{"pattern":"^(0|[1-9][0-9]*)$"}}},
                    "colors":{"items":{"not":{"const":Color::C}}},
                    "color_identity":{"items":{"not":{"const":Color::C}}}
                }},
                "activated":{"maxItems":0},"triggered":{"maxItems":0},"keywords":{"maxItems":0}
            },"oneOf":branches}
        }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let catalog: Catalog =
        serde_json::from_str(include_str!("../../../tests/fixtures/expected-ir.json")).unwrap();
    let mut creature = catalog
        .definitions
        .into_iter()
        .find(|d| d.name == "Grizzly Bears")
        .unwrap();
    creature.source_labels = None;
    creature.static_abilities = vec![StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::UnlimitedControllerHandSize,
    }];
    creature.capabilities = creature.requirements();
    creature
}

#[cfg(test)]
#[test]
fn unlimited_hand_size() {
    let creature = witness();
    assert!(creature.validate().is_ok());
    assert!(
        creature
            .capabilities
            .contains(&Capability::UnlimitedHandSizeV1)
    );

    let mut artifact = creature.clone();
    artifact.characteristics.kind = CardKind::Artifact;
    artifact.characteristics.subtypes = vec!["Book".into()];
    artifact.characteristics.power = None;
    artifact.characteristics.toughness = None;
    artifact.capabilities = artifact.requirements();
    assert!(artifact.validate().is_ok());

    let mut mana = ManaAbility {
        costs: PermanentActivationCosts {
            tap: true,
            mana: None,
            sacrifice: false,
            remove_source_plus_one_counters: None,
        },
        restriction: None,
        self_damage: None,
        production: ManaProduction::AnyColor,
        source_block: 0,
    };
    artifact.mana_abilities.push(mana.clone());
    artifact.capabilities = artifact.requirements();
    assert!(artifact.validate().is_ok());

    let mut land = artifact.clone();
    land.characteristics.kind = CardKind::Land;
    land.characteristics.subtypes.clear();
    land.characteristics.mana_cost.generic = Quantity::zero();
    land.characteristics.mana_cost.colored.clear();
    land.spell = None;
    land.intrinsic_zone = Some(FunctioningZone::Battlefield);
    mana.production = ManaProduction::Fixed {
        symbols: vec![Color::C],
    };
    land.mana_abilities = vec![mana];
    land.capabilities = land.requirements();
    assert!(land.validate().is_ok());

    let mut invalid = [
        creature.clone(),
        creature.clone(),
        creature.clone(),
        creature.clone(),
        artifact.clone(),
        land.clone(),
        land.clone(),
        land,
    ];
    invalid[0]
        .capabilities
        .remove(&Capability::UnlimitedHandSizeV1);
    invalid[1].static_abilities[0].functioning_zone = FunctioningZone::Stack;
    invalid[2]
        .static_abilities
        .push(creature.static_abilities[0].clone());
    invalid[3]
        .characteristics
        .additional_types
        .push(CardKind::Artifact);
    invalid[4].mana_abilities[0].costs.sacrifice = true;
    invalid[5].mana_abilities[0].production = ManaProduction::AnyColor;
    invalid[6].characteristics.subtypes = vec!["Island".into()];
    invalid[7].mana_abilities.clear();
    invalid[7].capabilities = invalid[7].requirements();
    for d in invalid {
        assert!(d.validate().is_err());
    }
    let mut extra_capability = creature;
    extra_capability.static_abilities.clear();
    assert!(extra_capability.validate().is_err());
}
