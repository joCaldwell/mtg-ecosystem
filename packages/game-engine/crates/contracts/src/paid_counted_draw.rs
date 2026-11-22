//! Paid battlefield activations, including source tap or sacrifice, draw for the
//! ability's controller after payment (CR 113.7a/8, 121.2, 601.2h, 602.2b).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

fn draw_block(program: &SpellProgram) -> Option<u32> {
    let (count, block) = controller_counted_draw::standalone_controller_draw(program)?;
    (program.timing == SpellTiming::Instant && *count > Quantity::from(1) && block <= 2)
        .then_some(block)
}

pub(super) fn program_matches(program: &SpellProgram) -> bool {
    draw_block(program).is_some()
}

pub(super) fn ability_matches(ability: &ActivatedAbility) -> bool {
    ability.functioning_zone == FunctioningZone::Battlefield
        && program_matches(&ability.program)
        && match &ability.cost {
            ActivationCost::Mana { cost } => {
                !cost.generic.is_negative() && !cost.colored.contains(&Color::C)
            }
            ActivationCost::Composite { costs } => {
                (costs.tap || costs.sacrifice)
                    && costs.mana.is_some()
                    && validate_permanent_costs(costs).is_ok()
            }
            _ => false,
        }
}

pub(super) fn definition_matches(d: &Definition) -> bool {
    let [ability] = d.activated.as_slice() else {
        return false;
    };
    let Some(draw_block) = draw_block(&ability.program) else {
        return false;
    };
    let c = &d.characteristics;
    if !ability_matches(ability)
        || c.basic
        || c.legendary
        || c.snow
        || c.subtypes.iter().any(|subtype| {
            unsupported_rules_subtype(subtype) || matches!(subtype.as_str(), "Aura" | "Equipment")
        })
        || d.x_mana_cost
        || !d.keywords.is_empty()
        || !d.triggered.is_empty()
    {
        return false;
    }
    if c.kind == CardKind::Land {
        if !c.subtypes.is_empty()
            || !c.additional_types.is_empty()
            || d.intrinsic_zone != Some(FunctioningZone::Battlefield)
            || d.spell.is_some()
            || !matches!(
                d.static_abilities.as_slice(),
                [] | [StaticAbility {
                    functioning_zone: FunctioningZone::Battlefield,
                    effect: StaticEffect::EntersTapped,
                }]
            )
            || d.mana_abilities.len() != 1
        {
            return false;
        }
    } else if !d.static_abilities.is_empty()
        || d.intrinsic_zone.is_some()
        || !d.spell.as_ref().is_some_and(|p| {
            p.timing == SpellTiming::MainPhase
                && p.resolution == SpellResolution::Permanent
                && p.functioning_zone == FunctioningZone::Stack
                && p.additional_sacrifice.is_none()
                && p.optional_additional_costs.is_empty()
                && p.instructions.is_empty()
                && p.targets.is_empty()
                && p.modes.is_empty()
        })
        || match c.kind {
            CardKind::Creature => {
                !matches!(c.additional_types.as_slice(), [] | [CardKind::Enchantment])
            }
            CardKind::Artifact | CardKind::Enchantment => !c.additional_types.is_empty(),
            _ => true,
        }
    {
        return false;
    }
    match d.mana_abilities.as_slice() {
        [] => draw_block == 0,
        [mana] => {
            let last_block = if d.static_abilities.is_empty() { 1 } else { 2 };
            matches!(c.kind, CardKind::Artifact | CardKind::Land)
                && matches!(&ability.cost, ActivationCost::Composite { costs } if costs.tap)
                && mana.costs.tap
                && mana.costs.mana.is_none()
                && !mana.costs.sacrifice
                && matches!(&mana.production, ManaProduction::Fixed { symbols } if !symbols.is_empty())
                && draw_block <= last_block
                && mana.source_block <= last_block
                && draw_block != mana.source_block
        }
        _ => false,
    }
}

pub(super) fn extend_schema(s: &mut Value) {
    use Capability::*;
    let program = |block: Value| {
        json!({"properties":{
            "instructions":{"items":{"properties":{"source_block":block}}}
        }})
    };
    let draw = |block: Value| {
        json!({"properties":{
            "activated":{"items":{"properties":{"program":program(block)}}}
        }})
    };
    let mana = |block: Value| {
        json!({"properties":{
            "mana_abilities":{"items":{"properties":{"source_block":block}}}
        }})
    };
    let cost = json!({"type":"object","properties":{
        "generic":{"pattern":"^(0|[1-9][0-9]*)$"},"colored":{"items":{"enum":["W","U","B","R","G"]}}
    }});
    let defs = &mut s["$defs"];
    defs["PaidCountedDrawProgram"] = json!({"properties":{
        "additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},"targets":{"maxItems":0},"modes":{"maxItems":0},
        "functioning_zone":{"const":"stack"},"timing":{"const":"instant"},
        "resolution":{"const":"instructions"},
        "instructions":{"minItems":1,"maxItems":1,"items":instruction_schema::at_most(
            instruction_schema::counted_draw("^([2-9]|[1-9][0-9]+)$"), 2
        )}
    }});
    defs["PaidCountedDrawAbility"] = json!({"properties":{
        "functioning_zone":{"const":"battlefield"},
        "cost":{"type":"object","oneOf":[
            {"required":["mana"],"properties":{"mana":{"properties":{"cost":cost}}}},
            {"required":["composite"],"properties":{"composite":{"properties":{"costs":{
                "required":["mana"],"properties":{"mana":cost},
                "anyOf":[{"properties":{"tap":{"const":true}}},{"properties":{"sacrifice":{"const":true}}}]
            }}}}}
        ]},
        "program":{"$ref":"#/$defs/PaidCountedDrawProgram"}
    }});
    let has_mana =
        json!({"required":["mana_abilities"],"properties":{"mana_abilities":{"minItems":1}}});
    let has_static =
        json!({"required":["static_abilities"],"properties":{"static_abilities":{"minItems":1}}});
    let is_kind = |kind: CardKind| json!({"properties":{"characteristics":{"properties":{"kind":{"const":kind}}}}});
    let toggle_cap = |shape: Value, cap: Capability| {
        let contains = json!({"contains":{"const":cap}});
        json!({
            "if":shape,"then":{"properties":{"capabilities":contains}},
            "else":{"properties":{"capabilities":{"not":contains}}}
        })
    };
    let mut constraints = vec![
        json!({"if":has_mana,"then":{
            "properties":{"activated":{"items":{"properties":{"cost":{"required":["composite"],
                "properties":{"composite":{"properties":{"costs":{"properties":{"tap":{"const":true}}}}}}
            }}}}},
            "allOf":[
                {"if":{"not":has_static},"then":{"allOf":[draw(json!({"maximum":1})),mana(json!({"maximum":1}))]}},
                {"not":{"allOf":[draw(json!({"const":0})),mana(json!({"const":0}))]}},
                {"not":{"allOf":[draw(json!({"const":1})),mana(json!({"const":1}))]}},
                {"not":{"allOf":[draw(json!({"const":2})),mana(json!({"const":2}))]}}
            ]
        },"else":draw(json!({"const":0}))}),
        toggle_cap(has_mana, PermanentManaV1),
        toggle_cap(has_static, EntersTappedV1),
        toggle_cap(
            json!({"properties":{"activated":{"items":{"properties":{"cost":{"required":["composite"]}}}}}}),
            CompositeActivationCostV1,
        ),
        toggle_cap(
            json!({"properties":{"activated":{"items":{"properties":{"cost":{"required":["mana"]}}}}}}),
            ManaActivationCostV1,
        ),
        toggle_cap(is_kind(CardKind::Land), LandPlayV1),
        toggle_cap(is_kind(CardKind::Artifact), CastArtifactV1),
        toggle_cap(is_kind(CardKind::Creature), BaseCreatureV1),
        toggle_cap(is_kind(CardKind::Creature), CastCreatureV1),
        toggle_cap(
            json!({"anyOf":[
                is_kind(CardKind::Enchantment),
                {"properties":{"characteristics":{"required":["additional_types"],"properties":{"additional_types":{"const":[CardKind::Enchantment]}}}}}
            ]}),
            CastEnchantmentV1,
        ),
    ];
    constraints.push(json!({"if":is_kind(CardKind::Land),"then":{
        "required":["mana_abilities"],"properties":{
            "characteristics":{"properties":{"subtypes":{"maxItems":0},
                "mana_cost":{"properties":{"generic":{"const":"0"},"colored":{"maxItems":0}}}}},
            "intrinsic_zone":{"const":"battlefield"},"spell":{"type":"null"},
            "static_abilities":{"maxItems":1,"items":{"properties":{
                "functioning_zone":{"const":"battlefield"},"effect":{"const":StaticEffect::EntersTapped}
            }}},
            "mana_abilities":{"minItems":1}
        }
    },"else":{"properties":{
        "static_abilities":{"maxItems":0},"intrinsic_zone":{"type":"null"},
        "spell":{"type":"object","properties":{
            "functioning_zone":{"const":"stack"},"timing":{"const":"main_phase"},
            "resolution":{"const":"permanent"},"additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},
            "targets":{"maxItems":0},"instructions":{"maxItems":0},"modes":{"maxItems":0}
        }}
    }}}));
    constraints.push(json!({"if":is_kind(CardKind::Creature),"then":{
        "properties":{"characteristics":{"properties":{
            "additional_types":{"enum":[[],[CardKind::Enchantment]]},
            "power":{"type":"string"},"toughness":{"pattern":"^[1-9][0-9]*$","type":"string"}
        }}}
    },"else":{"properties":{"characteristics":{"properties":{
        "additional_types":{"maxItems":0},"power":{"type":"null"},"toughness":{"type":"null"}
    }}}}}));
    constraints.push(
        json!({"if":{"not":{"anyOf":[is_kind(CardKind::Artifact),is_kind(CardKind::Land)]}},
            "then":{"properties":{"mana_abilities":{"maxItems":0}}}
        }),
    );

    let excluded_subtypes: Vec<_> = UNSUPPORTED_RULES_SUBTYPES
        .iter()
        .copied()
        .chain(["Aura", "Equipment"])
        .collect();
    let required_caps = [NormalLayoutV1, ActivatedAbilityV1, ControllerCountedDrawV1]
        .map(|cap| json!({"contains":{"const":cap}}));
    defs["PaidCountedDrawDefinition"] = json!({
        "required":["activated","spell","intrinsic_zone","characteristics","capabilities"],
        "properties":{
            "characteristics":{"properties":{
                "kind":{"enum":[CardKind::Artifact,CardKind::Creature,CardKind::Enchantment,CardKind::Land]},
                "basic":{"const":false},"legendary":{"const":false},"snow":{"const":false},
                "subtypes":{"items":{"not":{"enum":excluded_subtypes}}},
                "mana_cost":{"properties":{"generic":{"pattern":"^(0|[1-9][0-9]*)$"}}},
                "colors":{"items":{"not":{"const":Color::C}}},
                "color_identity":{"items":{"not":{"const":Color::C}}}
            }},
            "activated":{"minItems":1,"maxItems":1,"items":{"$ref":"#/$defs/PaidCountedDrawAbility"}},
            "mana_abilities":{"maxItems":1,"items":{"properties":{
                "costs":{"properties":{"tap":{"const":true},"sacrifice":{"const":false},"mana":{"type":"null"}}},
                "production":{"properties":{"mana":{"const":"fixed"},"symbols":{"minItems":1}}},
                "source_block":{"maximum":2}
            }}},
            "keywords":{"maxItems":0},"triggered":{"maxItems":0},"x_mana_cost":{"const":false},
            "capabilities":{
                "items":{"enum":[NormalLayoutV1,ActivatedAbilityV1,ControllerCountedDrawV1,
                    PermanentManaV1,EntersTappedV1,CompositeActivationCostV1,ManaActivationCostV1,
                    LandPlayV1,CastArtifactV1,BaseCreatureV1,CastCreatureV1,CastEnchantmentV1]},
                "allOf":required_caps
            }
        },
        "allOf":constraints
    });
}

#[cfg(test)]
pub(super) fn witnesses() -> [Definition; 4] {
    let paid_mana = ManaCost {
        generic: 4.into(),
        colored: vec![Color::U],
    };
    let mut artifact = activated_artifact_draw::witness();
    let ability = &mut artifact.activated[0];
    ability.cost = ActivationCost::Composite {
        costs: PermanentActivationCosts {
            mana: Some(paid_mana.clone()),
            tap: true,
            sacrifice: true,
            remove_source_plus_one_counters: None,
        },
    };
    ability.program.instructions = vec![Instruction::draw(
        PlayerExpr::you(Context::Activated),
        3.into(),
        1,
    )];
    artifact.mana_abilities = vec![ManaAbility {
        restriction: None,
        self_damage: None,
        costs: PermanentActivationCosts {
            mana: None,
            tap: true,
            sacrifice: false,
            remove_source_plus_one_counters: None,
        },
        production: ManaProduction::Fixed {
            symbols: vec![Color::C, Color::C],
        },
        source_block: 0,
    }];
    artifact.capabilities = artifact.requirements();

    let mut land = artifact.clone();
    let c = &mut land.characteristics;
    c.kind = CardKind::Land;
    c.subtypes.clear();
    c.mana_cost = ManaCost {
        generic: 0.into(),
        colored: vec![],
    };
    land.intrinsic_zone = Some(FunctioningZone::Battlefield);
    land.spell = None;
    land.static_abilities = vec![StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::EntersTapped,
    }];
    land.activated[0].program.instructions = vec![Instruction::draw(
        PlayerExpr::you(Context::Activated),
        2.into(),
        2,
    )];
    land.mana_abilities[0].source_block = 1;
    land.capabilities = land.requirements();

    let mut creature = artifact.clone();
    let c = &mut creature.characteristics;
    c.kind = CardKind::Creature;
    c.additional_types = vec![CardKind::Enchantment];
    c.subtypes = vec!["Human".into(), "Wizard".into()];
    c.power = Some(1.into());
    c.toughness = Some(1.into());
    creature.mana_abilities.clear();
    creature.activated[0].cost = ActivationCost::Mana {
        cost: paid_mana.clone(),
    };
    creature.activated[0].program.instructions = vec![Instruction::draw(
        PlayerExpr::you(Context::Activated),
        "123456789012345678901234567890".parse().unwrap(),
        0,
    )];
    creature.capabilities = creature.requirements();

    let mut enchantment = creature.clone();
    let c = &mut enchantment.characteristics;
    c.kind = CardKind::Enchantment;
    c.additional_types.clear();
    c.subtypes.clear();
    c.power = None;
    c.toughness = None;
    enchantment.activated[0].cost = ActivationCost::Composite {
        costs: PermanentActivationCosts {
            tap: false,
            mana: Some(paid_mana),
            sacrifice: true,
            remove_source_plus_one_counters: None,
        },
    };
    enchantment.capabilities = enchantment.requirements();

    [artifact, land, creature, enchantment]
}

#[cfg(test)]
#[test]
fn paid_counted_draw() {
    let [artifact, land, creature, enchantment] = witnesses();
    for definition in [&artifact, &land, &creature, &enchantment] {
        definition.validate().unwrap();
        assert!(
            definition
                .capabilities
                .contains(&Capability::ControllerCountedDrawV1)
        );
        assert!(
            !definition
                .capabilities
                .contains(&Capability::ActivatedArtifactDrawV1)
        );
        assert_eq!(
            strict_read::<Definition>(&canonical(definition).unwrap()).unwrap(),
            *definition
        );
        for cap in &definition.capabilities {
            let mut missing = definition.clone();
            missing.capabilities.remove(cap);
            assert!(missing.validate().is_err(), "{cap:?}");
        }
    }

    type Mutation = (&'static str, fn(&mut Definition));
    let mutations: &[Mutation] = &[
        ("one-card draw", |d| {
            d.activated[0].program.instructions = vec![Instruction::draw(
                PlayerExpr::you(Context::Activated),
                1.into(),
                2,
            )];
        }),
        ("duplicate ordinal", |d| {
            d.mana_abilities[0].source_block = 2
        }),
        ("missing static leaves ordinal gap", |d| {
            d.static_abilities.clear()
        }),
        ("wrong static", |d| {
            d.static_abilities[0].effect = StaticEffect::UnlimitedControllerHandSize
        }),
        ("land without mana production", |d| d.mana_abilities.clear()),
        ("mana offers a choice", |d| {
            d.mana_abilities[0].production = ManaProduction::AnyColor
        }),
        ("mana activation pays mana", |d| {
            d.mana_abilities[0].costs.mana = Some(ManaCost {
                generic: 1.into(),
                colored: vec![],
            });
        }),
        ("source not reserved during payment", |d| {
            if let ActivationCost::Composite { costs } = &mut d.activated[0].cost {
                costs.tap = false;
            }
        }),
        ("source sacrifice without explicit mana", |d| {
            d.activated[0].cost = ActivationCost::SacrificeSource;
        }),
        ("composite without explicit mana", |d| {
            if let ActivationCost::Composite { costs } = &mut d.activated[0].cost {
                costs.mana = None;
            }
        }),
        ("additional activation", |d| {
            d.activated.push(d.activated[0].clone())
        }),
        ("additional instructions", |d| {
            let p = &mut d.activated[0].program;
            p.instructions.push(p.instructions[0].clone());
        }),
        ("land subtype", |d| {
            d.characteristics.subtypes.push("Island".into())
        }),
        ("snow", |d| d.characteristics.snow = true),
        ("legendary", |d| d.characteristics.legendary = true),
    ];
    for (label, mutate) in mutations {
        let mut invalid = land.clone();
        mutate(&mut invalid);
        invalid.capabilities = invalid.requirements();
        assert!(invalid.validate().is_err(), "{label}");
    }
    let mut invalid = creature;
    invalid.characteristics.additional_types = vec![CardKind::Artifact];
    invalid.capabilities = invalid.requirements();
    assert!(invalid.validate().is_err());
    let legacy = activated_artifact_draw::witness();
    legacy.validate().unwrap();
    assert!(!definition_matches(&legacy));
}
