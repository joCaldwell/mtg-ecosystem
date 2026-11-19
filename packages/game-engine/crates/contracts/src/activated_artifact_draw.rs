//! Battlefield activations pay their fixed mana and source tap costs at
//! announcement (CR 113.6, 602.2b, 601.2h) and draw for the ability's
//! controller on resolution (CR 113.8, 121.2, 608.2c).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

const CAPABILITIES: [Capability; 6] = [
    Capability::NormalLayoutV1,
    Capability::CastArtifactV1,
    Capability::ActivatedAbilityV1,
    Capability::CompositeActivationCostV1,
    Capability::ControllerDrawV1,
    Capability::ActivatedArtifactDrawV1,
];

/// The exact paid source-tap draw-one ability. Definition validation also
/// checks the complete source and its cast program.
pub fn activated_artifact_draw_ability(ability: &ActivatedAbility) -> bool {
    let p = &ability.program;
    ability.functioning_zone == FunctioningZone::Battlefield
        && matches!(&ability.cost, ActivationCost::Composite { costs }
            if costs.tap && !costs.sacrifice && costs.mana.is_some()
                && validate_permanent_costs(costs).is_ok())
        && p.functioning_zone == FunctioningZone::Stack
        && p.timing == SpellTiming::Instant
        && p.resolution == SpellResolution::Instructions
        && p.additional_sacrifice.is_none()
        && p.targets.is_empty()
        && p.modes.is_empty()
        && matches!(p.instructions.as_slice(),
            [Instruction::DrawCards { count, recipient: None, source_block: 0 }]
                if *count == Quantity::from(1))
}

pub(super) fn validate_source(d: &Definition) -> Result<(), String> {
    let c = &d.characteristics;
    if c.kind != CardKind::Artifact
        || !c.additional_types.is_empty()
        || c.subtypes
            .iter()
            .any(|s| matches!(s.as_str(), "Equipment" | "Aura"))
        || c.legendary
        || d.activated.len() != 1
        || !d.keywords.is_empty()
        || !d.static_abilities.is_empty()
        || !d.triggered.is_empty()
        || !d.mana_abilities.is_empty()
        || d.spell
            .as_ref()
            .is_some_and(|p| p.additional_sacrifice.is_some())
    {
        return Err("paid artifact draw requires a complete ordinary artifact source".into());
    }
    // The ordinary artifact definition checks the remaining characteristics
    // and requires the main-phase permanent cast program.
    Ok(())
}

pub(super) fn extend_schema(s: &mut serde_json::Value) {
    use serde_json::json;

    let defs = &mut s["$defs"];
    defs["ActivatedArtifactDrawAbility"] = json!({
        "required":["functioning_zone","cost","program"],
        "properties":{
            "functioning_zone":{"const":"battlefield"},
            "cost":{"required":["composite"],"properties":{"composite":{
                "required":["costs"],"properties":{"costs":{
                    "required":["tap","sacrifice","mana"],"properties":{
                        "tap":{"const":true},"sacrifice":{"const":false},
                        "mana":{"type":"object","properties":{
                            "generic":{"pattern":"^(0|[1-9][0-9]*)$"},
                            "colored":{"items":{"enum":["W","U","B","R","G"]}}
                        }}
                    }
                }}
            }}},
            "program":{"properties":{
                "additional_sacrifice":{"type":"null"},"targets":{"maxItems":0},"modes":{"maxItems":0},
                "functioning_zone":{"const":"stack"},"timing":{"const":"instant"},
                "resolution":{"const":"instructions"},
                "instructions":{"minItems":1,"maxItems":1,"items":{
                    "required":["op","count","source_block"],"properties":{
                        "op":{"const":"draw_cards"},"count":{"const":"1"},
                        "recipient":{"type":"null"},"source_block":{"const":0}
                    }
                }}
            }}
        }
    });
    defs["ArtifactDrawFreeProgram"] = json!({"properties":{
        "instructions":{"items":{"not":{"required":["op"],"properties":{
            "op":{"enum":["draw_cards","optional_draw_cards"]}
        }}}},
        "modes":{"items":{"properties":{"program":{"$ref":"#/$defs/ArtifactDrawFreeProgram"}}}}
    }});
    let composite_draw = json!({
        "required":["cost","program"],
        "properties":{
            "cost":{"type":"object","required":["composite"],"properties":{"composite":{
                "required":["costs"],"properties":{"costs":{
                    "required":["sacrifice"],"properties":{"sacrifice":{"const":false}}
                }}
            }}},
            "program":{"not":{"$ref":"#/$defs/ArtifactDrawFreeProgram"}}
        }
    });
    defs["ActivatedAbility"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "if":composite_draw,
            "then":{"$ref":"#/$defs/ActivatedArtifactDrawAbility"}
        }));

    let excluded_subtypes = UNSUPPORTED_RULES_SUBTYPES
        .iter()
        .copied()
        .chain(["Equipment", "Aura"])
        .collect::<Vec<_>>();
    let required_caps = CAPABILITIES.map(|cap| json!({"contains":{"const":cap}}));
    defs["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"required":["activated"],"properties":{"activated":{"contains":composite_draw}}},
        "then":{
            "required":["spell"],
            "properties":{
                "characteristics":{"properties":{
                    "kind":{"const":"artifact"},"additional_types":{"maxItems":0},
                    "subtypes":{"items":{"not":{"enum":excluded_subtypes}}},
                    "basic":{"const":false},"legendary":{"const":false},"snow":{"const":false},
                    "power":{"type":"null"},"toughness":{"type":"null"}
                }},
                "spell":{"type":"object","properties":{
                    "additional_sacrifice":{"type":"null"},"targets":{"maxItems":0},
                    "modes":{"maxItems":0},"instructions":{"maxItems":0},
                    "functioning_zone":{"const":"stack"},"timing":{"const":"main_phase"},
                    "resolution":{"const":"permanent"}
                }},
                "activated":{"minItems":1,"maxItems":1,"items":{"$ref":"#/$defs/ActivatedArtifactDrawAbility"}},
                "mana_abilities":{"maxItems":0},"triggered":{"maxItems":0},
                "static_abilities":{"maxItems":0},"keywords":{"maxItems":0},
                "x_mana_cost":{"const":false},"intrinsic_zone":{"type":"null"},
                "capabilities":{"items":{"enum":CAPABILITIES},"allOf":required_caps}
            }
        },
        "else":{"properties":{"capabilities":{"not":{"contains":{"const":Capability::ActivatedArtifactDrawV1}}}}}
    }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let mut d = activated_permanent_tap_tests::activated_tap();
    let p = &mut d.activated[0].program;
    p.targets.clear();
    p.instructions = vec![Instruction::DrawCards {
        count: 1.into(),
        recipient: None,
        source_block: 0,
    }];
    d.capabilities = d.requirements();
    d
}

#[cfg(test)]
#[test]
fn paid_artifact_draw_requires_exact_ability_source_and_capabilities() {
    let mut d = witness();
    d.characteristics.subtypes.push("Clue".into());
    let ActivationCost::Composite { costs } = &mut d.activated[0].cost else {
        panic!("expected composite activation");
    };
    costs.mana.as_mut().unwrap().colored.push(Color::U);
    d.validate().unwrap();
    assert!(activated_artifact_draw_ability(&d.activated[0]));
    assert_eq!(d.capabilities, BTreeSet::from(CAPABILITIES));
    assert_eq!(
        strict_read::<Definition>(&canonical(&d).unwrap()).unwrap(),
        d
    );

    for cap in CAPABILITIES {
        let mut missing = d.clone();
        missing.capabilities.remove(&cap);
        assert!(missing.validate().is_err(), "{cap:?}");
    }

    type Mutation = (&'static str, fn(&mut Definition));
    let mutations: &[Mutation] = &[
        ("different source kind", |d| {
            d.characteristics.kind = CardKind::Creature
        }),
        ("additional type", |d| {
            d.characteristics
                .additional_types
                .push(CardKind::Enchantment)
        }),
        ("Equipment", |d| {
            d.characteristics.subtypes = vec!["Equipment".into()]
        }),
        ("another activation", |d| {
            d.activated.push(d.activated[0].clone())
        }),
        ("keyword", |d| d.keywords.push(KeywordAbility::Flash)),
        ("nonordinary cast", |d| {
            d.spell.as_mut().unwrap().timing = SpellTiming::Instant
        }),
        ("wrong timing", |d| {
            d.activated[0].program.timing = SpellTiming::MainPhase
        }),
        ("without fixed mana", |d| {
            if let ActivationCost::Composite { costs } = &mut d.activated[0].cost {
                costs.mana = None;
            }
        }),
        ("wrong draw instruction", |d| {
            d.activated[0].program.instructions = vec![Instruction::DrawCards {
                count: 2.into(),
                recipient: Some(LifeRecipient::Controller),
                source_block: 1,
            }];
        }),
    ];
    for (label, mutate) in mutations {
        let mut invalid = d.clone();
        mutate(&mut invalid);
        invalid.capabilities = invalid.requirements();
        assert!(invalid.validate().is_err(), "{label}");
    }
    let mut legacy = d;
    let ActivationCost::Composite { costs } = &mut legacy.activated[0].cost else {
        panic!("expected composite activation");
    };
    costs.sacrifice = true;
    legacy.capabilities = legacy.requirements();
    assert!(
        !legacy
            .capabilities
            .contains(&Capability::ActivatedArtifactDrawV1)
    );
    legacy.validate().unwrap();
}
