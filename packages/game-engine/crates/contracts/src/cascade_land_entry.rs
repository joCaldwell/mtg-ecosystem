//! Complete ordinary creature source for the battlefield cascade land option.
use super::*;
use serde_json::{Value, json};

pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    if !d
        .static_abilities
        .iter()
        .any(|a| a.effect == StaticEffect::CascadeLandEntry)
    {
        return Ok(());
    }
    let c = &d.characteristics;
    // Common creature validation checks the ordinary spell's timing, zone,
    // resolution, and empty program. Only this static may modify the source.
    if c.kind != CardKind::Creature
        || !c.additional_types.is_empty()
        || c.snow
        || !d.keywords.is_empty()
        || !d.activated.is_empty()
        || !d.triggered.is_empty()
        || !d.mana_abilities.is_empty()
        || !matches!(
            d.static_abilities.as_slice(),
            [StaticAbility {
                effect: StaticEffect::CascadeLandEntry,
                functioning_zone: FunctioningZone::Battlefield,
            }]
        )
        || d.spell.as_ref().is_none_or(|p| {
            p.additional_sacrifice.is_some() || !p.optional_additional_costs.is_empty()
        })
    {
        return Err(
            "cascade land entry requires a sole battlefield static on an ordinary creature".into(),
        );
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    use Capability::*;
    let effect = json!({"required":["effect"],"properties":{
        "effect":{"const":StaticEffect::CascadeLandEntry}
    }});
    let battlefield =
        json!({"properties":{"functioning_zone":{"const":FunctioningZone::Battlefield}}});
    schema["$defs"]["StaticAbility"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "if":effect,"then":battlefield
        }));
    let caps = [
        NormalLayoutV1,
        BaseCreatureV1,
        CastCreatureV1,
        CascadeLandEntryV1,
    ];
    let required_caps: Vec<_> = caps
        .iter()
        .map(|cap| json!({"contains":{"const":cap}}))
        .collect();
    let allowed_caps: Vec<_> = caps.into_iter().chain([CommanderIdentityV1]).collect();
    let disallowed_subtypes: Vec<_> = UNSUPPORTED_RULES_SUBTYPES
        .iter()
        .copied()
        .chain(["Aura", "Equipment"])
        .collect();
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"required":["static_abilities"],"properties":{"static_abilities":{"contains":effect}}},
        "then":{"required":["spell"],"properties":{
            "characteristics":{"required":["power","toughness"],"properties":{
                "kind":{"const":CardKind::Creature},"additional_types":{"maxItems":0},
                "basic":{"const":false},"snow":{"const":false},
                "subtypes":{"items":{"not":{"enum":disallowed_subtypes}}},
                "power":{"type":"string"},"toughness":{"pattern":"^[1-9][0-9]*$","type":"string"},
                "mana_cost":{"properties":{"generic":{"pattern":"^(0|[1-9][0-9]*)$"}}},
                "colors":{"items":{"not":{"const":Color::C}}},"color_identity":{"items":{"not":{"const":Color::C}}}
            }},
            "x_mana_cost":{"const":false},"intrinsic_zone":{"type":"null"},
            "keywords":{"maxItems":0},"activated":{"maxItems":0},
            "mana_abilities":{"maxItems":0},"triggered":{"maxItems":0},
            "static_abilities":{"maxItems":1},
            "spell":{"type":"object","properties":{
                "timing":{"const":SpellTiming::MainPhase},"resolution":{"const":SpellResolution::Permanent},
                "functioning_zone":{"const":FunctioningZone::Stack},
                "instructions":{"maxItems":0},"targets":{"maxItems":0},"modes":{"maxItems":0},
                "additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0}
            }},
            "capabilities":{"uniqueItems":true,"items":{"enum":allowed_caps},"allOf":required_caps}
        }},
        "else":{"properties":{"capabilities":{"not":{"contains":{"const":CascadeLandEntryV1}}}}}
    }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let mut d = Catalog::read(include_str!("../../../tests/fixtures/expected-ir.json"))
        .unwrap()
        .definitions
        .into_iter()
        .find(|d| {
            d.capabilities
                == BTreeSet::from([
                    Capability::NormalLayoutV1,
                    Capability::BaseCreatureV1,
                    Capability::CastCreatureV1,
                ])
        })
        .expect("ordinary creature");
    d.static_abilities.push(StaticAbility {
        effect: StaticEffect::CascadeLandEntry,
        functioning_zone: FunctioningZone::Battlefield,
    });
    d.capabilities = d.requirements();
    d
}

#[test]
fn cascade_land_entry() {
    use Capability::*;
    let mut d = witness();
    let ordinary = BTreeSet::from([
        NormalLayoutV1,
        BaseCreatureV1,
        CastCreatureV1,
        CascadeLandEntryV1,
    ]);
    assert_eq!(d.capabilities, ordinary);
    d.validate().unwrap();
    d.characteristics.legendary = true;
    d.capabilities = d.requirements();
    assert_eq!(
        d.capabilities,
        ordinary
            .into_iter()
            .chain([CommanderIdentityV1])
            .collect::<BTreeSet<_>>()
    );
    let decoded: Definition = strict_read(&canonical(&d).unwrap()).unwrap();
    assert_eq!(d, decoded);
    decoded.validate().unwrap();
    assert_eq!(json!(d.static_abilities[0].effect), "cascade_land_entry");
    assert!(
        serde_json::from_value::<StaticEffect>(json!({"cascade_land_entry":{"tapped":false}}))
            .is_err()
    );

    for capability in d.capabilities.clone() {
        let mut bad = d.clone();
        bad.capabilities.remove(&capability);
        assert!(bad.validate().is_err());
    }
    for capability in [CascadeV1, CastTriggerV1] {
        let mut bad = d.clone();
        bad.capabilities.insert(capability);
        assert!(bad.validate().is_err());
    }
    let mutations: &[fn(&mut Definition)] =
        &[
            |d| d.static_abilities[0].functioning_zone = FunctioningZone::Stack,
            |d| d.static_abilities.push(d.static_abilities[0].clone()),
            |d| d.characteristics.kind = CardKind::Artifact,
            |d| d.characteristics.additional_types.push(CardKind::Artifact),
            |d| d.characteristics.snow = true,
            |d| d.keywords.push(KeywordAbility::Flying),
            |d| d.spell = None,
            |d| {
                d.spell.as_mut().unwrap().optional_additional_costs.push(
                    OptionalAdditionalManaCost {
                        identity: "additional".into(),
                        source_block: 0,
                        mana: ManaCost {
                            generic: 1.into(),
                            colored: vec![],
                        },
                    },
                )
            },
            |d| {
                d.spell
                    .as_mut()
                    .unwrap()
                    .instructions
                    .push(Instruction::draw(
                        PlayerExpr::you(Context::Spell),
                        1.into(),
                        0,
                    ))
            },
        ];
    for mutate in mutations {
        let mut bad = d.clone();
        mutate(&mut bad);
        bad.capabilities = bad.requirements();
        assert!(bad.validate().is_err());
    }
}
