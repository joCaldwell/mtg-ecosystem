//! Controller-only ordinary library choices. CR 109.5,608.2c,701.22/701.25.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

pub(crate) fn has_look(p: &SpellProgram) -> bool {
    p.instructions
        .iter()
        .any(|i| matches!(i, Instruction::Scry { .. } | Instruction::Surveil { .. }))
        || p.modes.iter().any(|mode| has_look(&mode.program))
}

/// Complete nontargeted ordinary scry, alone or with one mandatory card draw.
/// Either ordering and original source-block provenance are meaningful.
pub fn controller_scry_program(p: &SpellProgram) -> bool {
    controller_look_program(p, false)
}

/// Complete ordinary controller Surveil, alone or with one mandatory draw.
pub fn controller_surveil_program(p: &SpellProgram) -> bool {
    controller_look_program(p, true)
}

fn controller_look_program(p: &SpellProgram, surveil: bool) -> bool {
    if p.additional_sacrifice.is_some()
        || !p.optional_additional_costs.is_empty()
        || !p.modes.is_empty()
        || !p.targets.is_empty()
        || p.functioning_zone != FunctioningZone::Stack
        || p.resolution != SpellResolution::Instructions
    {
        return false;
    }
    let scry = |i: &Instruction| match i {
        Instruction::Scry { amount, .. } if !surveil => !amount.is_negative(),
        Instruction::Surveil { amount, .. } if surveil => !amount.is_negative(),
        _ => false,
    };
    let draw = |i: &Instruction| matches!(i, Instruction::DrawCards { recipient: None, count, .. } if *count == Quantity::from(1));
    match p.instructions.as_slice() {
        [i] => scry(i),
        [a, b] => (scry(a) && draw(b)) || (draw(a) && scry(b)),
        _ => false,
    }
}

fn cast_scry_conditions() -> impl Iterator<Item = TriggerCondition> {
    [Color::W, Color::U, Color::B, Color::R, Color::G]
        .into_iter()
        .map(|color| TriggerCondition::SpellCast {
            caster: CasterRelation::Controller,
            spell: TypePredicate::SpellColor { color },
        })
}

fn cast_scry_program(a: &TriggeredAbility) -> bool {
    a.functioning_zone == FunctioningZone::Battlefield
        && a.program.timing == SpellTiming::Instant
        && a.intervening_if.is_none()
        && a.death_filter.is_none()
        && cast_scry_conditions().any(|condition| a.condition == condition)
        && matches!(
            a.program.instructions.as_slice(),
            [Instruction::Scry { .. }]
        )
        && controller_scry_program(&a.program)
}

pub(crate) fn validate_definition(d: &Definition) -> Result<(), String> {
    if d.activated.iter().any(|a| has_look(&a.program))
        || d.triggered.iter().any(|a| {
            has_look(&a.program)
                && (d.characteristics.kind != CardKind::Creature
                    || !d.characteristics.additional_types.is_empty()
                    || !cast_scry_program(a))
        })
    {
        return Err("library look requires an ordinary spell or a supported cast trigger".into());
    }
    if !d.spell.as_ref().is_some_and(has_look) {
        return Ok(());
    }
    let c = &d.characteristics;
    if !matches!(c.kind, CardKind::Instant | CardKind::Sorcery)
        || !c.additional_types.is_empty()
        || c.basic
        || c.legendary
        || c.power.is_some()
        || c.toughness.is_some()
        || d.intrinsic_zone.is_some()
        || d.x_mana_cost
        || !d.keywords.is_empty()
        || !d.static_abilities.is_empty()
        || !d.activated.is_empty()
        || !d.triggered.is_empty()
        || !d.mana_abilities.is_empty()
    {
        return Err("library look requires a complete ordinary spell definition".into());
    }
    Ok(())
}

pub(crate) fn extend_schema(schema: &mut serde_json::Value) {
    use serde_json::json;
    let scry = json!({"required":["op","amount","source_block"],"properties":{"op":{"const":"scry"},"amount":{"type":"string","pattern":"^(0|[1-9][0-9]*)$"}}});
    let draw = json!({"properties":{"op":{"const":"draw_cards"},"count":{"const":"1"},"recipient":{"type":"null"}}});
    let root = json!({"required":["targets","instructions"],"properties":{
        "optional_additional_costs":{"maxItems":0},"additional_sacrifice":{"type":"null"},"modes":{"maxItems":0},
        "functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"}
    }});
    schema["$defs"]["ControllerScryProgram"] = json!({"allOf":[root.clone(),
        {"properties":{"targets":{"maxItems":0},"instructions":{"oneOf":[
            {"minItems":1,"maxItems":1,"items":scry},
            {"minItems":2,"maxItems":2,"prefixItems":[scry,draw],"items":false},
            {"minItems":2,"maxItems":2,"prefixItems":[draw,scry],"items":false}
        ]}}}
    ]});
    let grant = json!({"properties":{"op":{"const":"grant_keywords"},"target_slot":{"const":0},
        "modifier":{"type":"null"},"keywords":{"$ref":"#/$defs/TemporaryCreatureKeywords"},
        "duration":{"const":"until_end_of_turn"},"source_block":{"const":0}}});
    let scry_tail = json!({"allOf":[scry,{"properties":{"source_block":{"const":0}}}]});
    schema["$defs"]["ArtifactCreatureKeywordProgram"] = json!({"allOf":[root,
        {"properties":{
            "targets":{"minItems":1,"maxItems":1,"items":{"required":["filter"],"properties":{
                "slot":{"const":0},"selection_id":{"minLength":1},"chooser":{"const":"controller"},"stage":{"const":"announcement"},
                "domain":{"const":"battlefield_permanent"},"filter":{"type":"object","required":["types","non_types","non_colors","opponent"],"properties":{
                    "types":{"const":[CardKind::Creature,CardKind::Artifact]},"non_types":{"maxItems":0},"non_colors":{"maxItems":0},"opponent":{"const":false},"tapped":{"const":false}
                }}
            }}},
            "instructions":{"minItems":1,"maxItems":2,"prefixItems":[grant,scry_tail],"items":false}
        }}
    ]});
    let controlled = schema["$defs"]["TemporaryPermanentKeywordProgram"].clone();
    schema["$defs"]["ControlledPermanentKeywordProgram"] = controlled;
    schema["$defs"]["TemporaryPermanentKeywordProgram"] = json!({"oneOf":[
        {"$ref":"#/$defs/ControlledPermanentKeywordProgram"},
        {"$ref":"#/$defs/ArtifactCreatureKeywordProgram"}
    ]});
    schema["$defs"]["ScryFreeProgram"] = json!({"properties":{
        "instructions":{"items":{"not":{"required":["op"],"properties":{"op":{"const":"scry"}}}}},
        "modes":{"items":{"properties":{"program":{"$ref":"#/$defs/ScryFreeProgram"}}}}
    }});
    schema["$defs"]["SpellProgram"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"not":{"$ref":"#/$defs/ScryFreeProgram"}},
        "then":{"anyOf":[{"$ref":"#/$defs/ControllerScryProgram"},{"$ref":"#/$defs/ArtifactCreatureKeywordProgram"}]}
    }));
    let has_scry = json!({"required":["spell"],"properties":{"spell":{"type":"object","not":{"$ref":"#/$defs/ScryFreeProgram"}}}});
    let draw_in_program = json!({"properties":{"spell":{"properties":{"instructions":{"contains":{"required":["op"],"properties":{"op":{"const":"draw_cards"}}}}}}}});
    let def = schema["$defs"]["Definition"]["allOf"]
        .as_array_mut()
        .unwrap();
    let cast_scry = json!({
        "required":["program"],
        "properties":{"program":{"not":{"$ref":"#/$defs/ScryFreeProgram"}}}
    });
    let has_cast_scry = json!({
        "required":["triggered"],"properties":{"triggered":{"contains":cast_scry}}
    });
    def.push(json!({"properties":{
        "activated":{"items":{"properties":{"program":{"$ref":"#/$defs/ScryFreeProgram"}}}},
        "triggered":{"items":{"if":cast_scry,"then":{"properties":{
            "condition":{"enum":cast_scry_conditions().collect::<Vec<_>>()},
            "functioning_zone":{"const":"battlefield"},"intervening_if":{"type":"null"},
            "death_filter":{"type":"null"},"program":{"allOf":[
                {"$ref":"#/$defs/ControllerScryProgram"},
                {"properties":{"timing":{"const":"instant"},"instructions":{"minItems":1,"maxItems":1}}}
            ]}
        }}}}
    }}));
    def.push(json!({"if":has_cast_scry,"then":{"properties":{
        "characteristics":{"properties":{"kind":{"const":"creature"},"additional_types":{"maxItems":0}}},
        "capabilities":{"contains":{"const":Capability::ControllerScryV1}}
    }}}));
    def.push(json!({"if":has_scry,"then":{"properties":{
        "keywords":{"maxItems":0},"static_abilities":{"maxItems":0},"activated":{"maxItems":0},"triggered":{"maxItems":0},"mana_abilities":{"maxItems":0},"x_mana_cost":{"const":false},"intrinsic_zone":{"type":"null"},
        "characteristics":{"properties":{"kind":{"enum":["instant","sorcery"]},"additional_types":{"maxItems":0},"basic":{"const":false},"legendary":{"const":false},"power":{"type":"null"},"toughness":{"type":"null"}}},
        "capabilities":{"allOf":[{"contains":{"const":Capability::NormalLayoutV1}},{"contains":{"const":Capability::ControllerScryV1}}]}
    }},"else":{"if":{"not":has_cast_scry},"then":{"properties":{"capabilities":{"not":{"contains":{"const":Capability::ControllerScryV1}}}}}}}));
    def.push(json!({"if":has_scry,"then":{"allOf":[
        {"if":{"properties":{"characteristics":{"properties":{"kind":{"const":"instant"}}}}},
         "then":{"properties":{"spell":{"properties":{"timing":{"const":"instant"}}},"capabilities":{"contains":{"const":Capability::CastInstantV1},"not":{"contains":{"const":Capability::CastSorceryV1}}}}},
         "else":{"properties":{"spell":{"properties":{"timing":{"const":"main_phase"}}},"capabilities":{"contains":{"const":Capability::CastSorceryV1},"not":{"contains":{"const":Capability::CastInstantV1}}}}}},
        {"if":{"properties":{"spell":{"$ref":"#/$defs/ControllerScryProgram"}}},"then":{"allOf":[
            {"properties":{"capabilities":{"items":{"enum":[Capability::NormalLayoutV1,Capability::CastInstantV1,Capability::CastSorceryV1,Capability::ControllerScryV1,Capability::ControllerDrawV1]}}}},
            {"if":draw_in_program,"then":{"properties":{"capabilities":{"contains":{"const":Capability::ControllerDrawV1}}}},"else":{"properties":{"capabilities":{"not":{"contains":{"const":Capability::ControllerDrawV1}}}}}}
        ]}}
    ]}}));
    extend_surveil_schema(schema);
}

fn extend_surveil_schema(schema: &mut serde_json::Value) {
    use serde_json::json;
    // Same closed composition grammar, with a distinct opcode/capability.
    let encoded = serde_json::to_string(&schema["$defs"]["ControllerScryProgram"]).unwrap();
    schema["$defs"]["ControllerSurveilProgram"] =
        serde_json::from_str(&encoded.replace("\"scry\"", "\"surveil\"")).unwrap();
    schema["$defs"]["SurveilFreeProgram"] = json!({"properties":{
        "instructions":{"items":{"not":{"required":["op"],"properties":{"op":{"const":"surveil"}}}}},
        "modes":{"items":{"properties":{"program":{"$ref":"#/$defs/SurveilFreeProgram"}}}}
    }});
    let contains = json!({"not":{"$ref":"#/$defs/SurveilFreeProgram"}});
    schema["$defs"]["SpellProgram"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({"if":contains,"then":{"$ref":"#/$defs/ControllerSurveilProgram"}}));
    let has = json!({"required":["spell"],"properties":{"spell":{"type":"object","not":{"$ref":"#/$defs/SurveilFreeProgram"}}}});
    let def = schema["$defs"]["Definition"]["allOf"]
        .as_array_mut()
        .unwrap();
    def.push(json!({"properties":{
        "activated":{"items":{"properties":{"program":{"$ref":"#/$defs/SurveilFreeProgram"}}}},
        "triggered":{"items":{"properties":{"program":{"$ref":"#/$defs/SurveilFreeProgram"}}}}
    }}));
    def.push(json!({"if":has,"then":{"properties":{
        "spell":{"$ref":"#/$defs/ControllerSurveilProgram"},
        "keywords":{"maxItems":0},"static_abilities":{"maxItems":0},"activated":{"maxItems":0},"triggered":{"maxItems":0},"mana_abilities":{"maxItems":0},"x_mana_cost":{"const":false},"intrinsic_zone":{"type":"null"},
        "characteristics":{"properties":{"kind":{"enum":["instant","sorcery"]},"additional_types":{"maxItems":0},"basic":{"const":false},"legendary":{"const":false},"power":{"type":"null"},"toughness":{"type":"null"}}},
        "capabilities":{"items":{"enum":[Capability::NormalLayoutV1,Capability::CastInstantV1,Capability::CastSorceryV1,Capability::ControllerSurveilV1,Capability::ControllerDrawV1]},"allOf":[{"contains":{"const":Capability::NormalLayoutV1}},{"contains":{"const":Capability::ControllerSurveilV1}}]}
    }},"else":{"properties":{"capabilities":{"not":{"contains":{"const":Capability::ControllerSurveilV1}}}}}}));
    def.push(json!({"if":has,"then":{"allOf":[
        {"if":{"properties":{"characteristics":{"properties":{"kind":{"const":"instant"}}}}},
         "then":{"properties":{"spell":{"properties":{"timing":{"const":"instant"}}},"capabilities":{"contains":{"const":Capability::CastInstantV1},"not":{"contains":{"const":Capability::CastSorceryV1}}}}},
         "else":{"properties":{"spell":{"properties":{"timing":{"const":"main_phase"}}},"capabilities":{"contains":{"const":Capability::CastSorceryV1},"not":{"contains":{"const":Capability::CastInstantV1}}}}}},
        {"if":{"properties":{"spell":{"properties":{"instructions":{"contains":{"required":["op"],"properties":{"op":{"const":"draw_cards"}}}}}}}},
         "then":{"properties":{"capabilities":{"contains":{"const":Capability::ControllerDrawV1}}}},
         "else":{"properties":{"capabilities":{"not":{"contains":{"const":Capability::ControllerDrawV1}}}}}}
    ]}}));
}

#[cfg(test)]
pub(crate) fn witness() -> Definition {
    let mut d = temporary_permanent_keyword_tests::grant(false);
    let p = d.spell.as_mut().unwrap();
    p.targets.clear();
    p.instructions = vec![Instruction::Scry {
        amount: Quantity::from(2),
        source_block: 0,
    }];
    d.capabilities = d.requirements();
    d
}

#[cfg(test)]
pub(crate) fn surveil_witness() -> Definition {
    let mut d = witness();
    d.spell.as_mut().unwrap().instructions = vec![Instruction::Surveil {
        amount: 2.into(),
        source_block: 0,
    }];
    d.capabilities = d.requirements();
    d
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn controller_scry_contract_exact_counts_orders_and_capabilities() {
        for amount in ["0", "1", "2", "9999999999999999999999999999999999"] {
            for order in 0..3 {
                let mut d = witness();
                let scry = Instruction::Scry {
                    amount: amount.parse().unwrap(),
                    source_block: 0,
                };
                let draw = Instruction::DrawCards {
                    recipient: None,
                    count: 1.into(),
                    source_block: 1,
                };
                d.spell.as_mut().unwrap().instructions = match order {
                    0 => vec![scry],
                    1 => vec![scry, draw],
                    _ => vec![draw, scry],
                };
                d.capabilities = d.requirements();
                let mut caps = BTreeSet::from([
                    Capability::NormalLayoutV1,
                    Capability::CastInstantV1,
                    Capability::ControllerScryV1,
                ]);
                if order > 0 {
                    caps.insert(Capability::ControllerDrawV1);
                }
                assert_eq!(d.capabilities, caps);
                assert!(controller_scry_program(d.spell.as_ref().unwrap()));
                d.validate().unwrap();
                for cap in &caps {
                    let mut missing = d.clone();
                    missing.capabilities.remove(cap);
                    assert!(missing.validate().is_err());
                }
                let mut spare = d;
                spare.capabilities.insert(Capability::LifeGainV1);
                assert!(spare.validate().is_err());
            }
        }
    }

    #[test]
    fn controller_scry_contract_rejects_contexts_and_mixed_programs() {
        for case in 0..13 {
            let mut d = witness();
            let p = d.spell.as_mut().unwrap();
            match case {
                0 => {
                    p.instructions = vec![Instruction::Scry {
                        amount: (-1).into(),
                        source_block: 0,
                    }]
                }
                1 => p.instructions.push(Instruction::GainLife {
                    recipient: LifeRecipient::Controller,
                    amount: 2.into(),
                    source_block: 0,
                }),
                2 => p.instructions.push(p.instructions[0].clone()),
                3 => d.characteristics.kind = CardKind::Land,
                4 => d.characteristics.basic = true,
                5 => d.keywords.push(KeywordAbility::Flash),
                6 => d.intrinsic_zone = Some(FunctioningZone::Battlefield),
                7 => d.static_abilities.push(StaticAbility {
                    functioning_zone: FunctioningZone::Battlefield,
                    effect: StaticEffect::CantBeCountered,
                }),
                8 => d.x_mana_cost = true,
                9 => {
                    p.additional_sacrifice = Some(SacrificeFilter {
                        types: vec![CardKind::Creature],
                        subtypes: vec![],
                    })
                }
                10 => {
                    let child = p.clone();
                    p.instructions.clear();
                    p.modes = vec![
                        SpellMode {
                            program: child.clone(),
                        },
                        SpellMode { program: child },
                    ];
                }
                11 => {
                    p.targets = temporary_permanent_keyword_tests::grant(false)
                        .spell
                        .unwrap()
                        .targets
                }
                _ => p.instructions.push(Instruction::DrawCards {
                    recipient: None,
                    count: 2.into(),
                    source_block: 0,
                }),
            }
            d.capabilities = d.requirements();
            assert!(d.validate().is_err(), "case {case}");
        }
    }

    #[test]
    fn controller_scry_contract_filtered_grant_keeps_other_families_narrow() {
        let mut d = temporary_permanent_keyword_tests::grant(false);
        let p = d.spell.as_mut().unwrap();
        p.targets[0].domain = TargetDomain::BattlefieldPermanent;
        p.targets[0].filter.as_mut().unwrap().types = vec![CardKind::Creature, CardKind::Artifact];
        p.instructions.push(Instruction::Scry {
            amount: 1.into(),
            source_block: 0,
        });
        d.capabilities = d.requirements();
        d.validate().unwrap();
        let p = d.spell.as_ref().unwrap();
        assert!(!controller_scry_program(p));
        assert!(!controller_surveil_program(p));
        let descriptor = temporary_permanent_keyword_program(p).unwrap();
        assert_eq!(descriptor.controller_scry, Some(&Quantity::from(1)));
        assert!(descriptor.controller_life.is_none());
        assert!(temporary_creature_keyword_program(p).is_none());
        for case in 0..4 {
            let mut invalid = d.clone();
            let p = invalid.spell.as_mut().unwrap();
            match case {
                0 => p.targets[0].filter.as_mut().unwrap().types = vec![CardKind::Land],
                1 => {
                    p.instructions[1] = Instruction::GainLife {
                        recipient: LifeRecipient::Controller,
                        amount: 2.into(),
                        source_block: 0,
                    }
                }
                2 => {
                    p.instructions[1] = Instruction::Scry {
                        amount: 1.into(),
                        source_block: 1,
                    }
                }
                _ => p.targets[0].domain = TargetDomain::ControlledBattlefieldPermanent,
            }
            invalid.capabilities = invalid.requirements();
            assert!(invalid.validate().is_err());
        }
    }

    #[test]
    fn controller_scry_contract_strict_wire_and_nonnegative_validation() {
        let instruction = Instruction::Scry {
            amount: 2.into(),
            source_block: 4,
        };
        let wire = json!({"op":"scry","amount":"2","source_block":4});
        assert_eq!(serde_json::to_value(&instruction).unwrap(), wire);
        assert_eq!(
            serde_json::from_value::<Instruction>(wire.clone()).unwrap(),
            instruction
        );
        for case in 0..7 {
            let mut changed = wire.clone();
            match case {
                0 => changed["amount"] = json!(2),
                1 => changed["amount"] = serde_json::Value::Null,
                2 => changed["amount"] = json!("01"),
                3 => changed["recipient"] = serde_json::Value::Null,
                4 => changed["source_block"] = json!(-1),
                5 => {
                    changed.as_object_mut().unwrap().remove("amount");
                }
                _ => changed["amount"] = json!({"amount":"x"}),
            }
            assert!(
                serde_json::from_value::<Instruction>(changed).is_err(),
                "case {case}"
            );
        }
    }
}

#[cfg(test)]
mod combined_cost_fences {
    use super::*;
    #[test]
    fn scry_combined_optional_cost_and_conditional_draw_fences() {
        for variant in 0..3 {
            let targeted = variant == 1;
            let mut d = if targeted {
                temporary_permanent_keyword_tests::grant(false)
            } else if variant == 2 {
                surveil_witness()
            } else {
                witness()
            };
            if targeted {
                let p = d.spell.as_mut().unwrap();
                p.targets[0].domain = TargetDomain::BattlefieldPermanent;
                p.targets[0].filter.as_mut().unwrap().types =
                    vec![CardKind::Creature, CardKind::Artifact];
                p.instructions.push(Instruction::Scry {
                    amount: 1.into(),
                    source_block: 0,
                });
            }
            d.capabilities = d.requirements();
            d.validate().unwrap();
            let identity = format!(
                "card:{}/faces/0/abilities/0/keywords/0#keyword",
                d.oracle_id
            );
            let p = d.spell.as_mut().unwrap();
            p.optional_additional_costs
                .push(OptionalAdditionalManaCost {
                    identity: identity.clone(),
                    source_block: 0,
                    mana: ManaCost {
                        generic: 1.into(),
                        colored: vec![],
                    },
                });
            assert!(!controller_scry_program(p));
            assert!(!controller_surveil_program(p));
            assert!(temporary_targeted_keyword_program(p).is_none());
            d.capabilities = d.requirements();
            assert!(d.validate().is_err());
            let p = d.spell.as_mut().unwrap();
            p.optional_additional_costs.clear();
            p.instructions.push(Instruction::DrawCardsIfAdditionalCost {
                cost_identity: identity,
                count: 1.into(),
                source_block: 0,
            });
            assert!(!controller_scry_program(p));
            assert!(!controller_surveil_program(p));
            assert!(temporary_targeted_keyword_program(p).is_none());
            d.capabilities = d.requirements();
            assert!(d.validate().is_err());
        }
    }
}

#[cfg(test)]
mod surveil_tests {
    use super::*;
    #[test]
    fn controller_surveil_contract_counts_orders_and_strict_family() {
        for n in ["0", "2", "9999999999999999999999999999"] {
            for order in 0..3 {
                let mut d = surveil_witness();
                let look = Instruction::Surveil {
                    amount: n.parse().unwrap(),
                    source_block: 0,
                };
                let draw = Instruction::DrawCards {
                    recipient: None,
                    count: 1.into(),
                    source_block: 1,
                };
                d.spell.as_mut().unwrap().instructions = match order {
                    0 => vec![look],
                    1 => vec![look, draw],
                    _ => vec![draw, look],
                };
                d.capabilities = d.requirements();
                d.validate().unwrap();
                assert!(controller_surveil_program(d.spell.as_ref().unwrap()));
                assert!(!controller_scry_program(d.spell.as_ref().unwrap()));
                for cap in d.capabilities.clone() {
                    let mut missing = d.clone();
                    missing.capabilities.remove(&cap);
                    assert!(missing.validate().is_err());
                }
                let mut spare = d.clone();
                spare.capabilities.insert(Capability::ControllerScryV1);
                assert!(spare.validate().is_err());
                let mut mixed = d;
                mixed
                    .spell
                    .as_mut()
                    .unwrap()
                    .instructions
                    .push(Instruction::Scry {
                        amount: 1.into(),
                        source_block: 0,
                    });
                mixed.capabilities = mixed.requirements();
                assert!(mixed.validate().is_err());
            }
        }
    }
}
