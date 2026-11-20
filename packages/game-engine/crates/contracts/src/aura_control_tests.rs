//! Synthetic typed IR controls; complete frozen source tests live in compiler.
use super::*;

pub(super) fn aura() -> Definition {
    let mut d = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-attachments-ir.json"
    ))
    .unwrap()
    .definitions
    .into_iter()
    .find(|d| d.name == "Pacifism")
    .unwrap();
    d.static_abilities = vec![StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::ControlEnchantedPermanent,
    }];
    d.capabilities = d.requirements();
    d
}

#[test]
fn aura_control_has_exact_nonrecursive_requirements_and_no_identity_dispatch() {
    let mut d = aura();
    assert_eq!(
        d.requirements(),
        BTreeSet::from([
            Capability::NormalLayoutV1,
            Capability::CastEnchantmentV1,
            Capability::CreatureTargetV1,
            Capability::AttachmentsV1,
            Capability::AuraControlV1,
        ])
    );
    for generic in [0_i64, 2, 3, 7] {
        d.characteristics.mana_cost.generic = generic.into();
        d.oracle_id = format!("synthetic-aura-{generic}");
        d.name = format!("Synthetic Aura {generic}");
        assert!(aura_control_source(&d));
        d.validate().unwrap();
    }
    d.capabilities.remove(&Capability::AuraControlV1);
    assert!(
        aura_control_source(&d),
        "shape must not recurse into capabilities"
    );
    assert!(d.validate().is_err(), "missing family capability");
    d.capabilities = d.requirements();
    d.capabilities.insert(Capability::FlashV1);
    assert!(d.validate().is_err(), "spare capability");
}

#[test]
fn aura_control_rejects_every_outside_shape_even_with_recomputed_capabilities() {
    type Mutation = fn(&mut Definition);
    let mutations: &[(&str, Mutation)] = &[
        ("wrong kind", |d| {
            d.characteristics.kind = CardKind::Artifact
        }),
        ("combined type", |d| {
            d.characteristics.additional_types.push(CardKind::Artifact)
        }),
        ("missing Aura", |d| d.characteristics.subtypes.clear()),
        ("extra subtype", |d| {
            d.characteristics.subtypes.push("Role".into())
        }),
        ("basic", |d| d.characteristics.basic = true),
        ("legendary", |d| d.characteristics.legendary = true),
        ("snow", |d| d.characteristics.snow = true),
        ("power", |d| d.characteristics.power = Some(1.into())),
        ("toughness", |d| {
            d.characteristics.toughness = Some(1.into())
        }),
        ("negative mana", |d| {
            d.characteristics.mana_cost.generic = (-1).into()
        }),
        ("variable cost", |d| d.x_mana_cost = true),
        ("intrinsic zone", |d| {
            d.intrinsic_zone = Some(FunctioningZone::Battlefield)
        }),
        ("keyword", |d| d.keywords.push(KeywordAbility::Flash)),
        ("no spell", |d| d.spell = None),
        ("instant timing", |d| {
            d.spell.as_mut().unwrap().timing = SpellTiming::Instant
        }),
        ("wrong resolution", |d| {
            d.spell.as_mut().unwrap().resolution = SpellResolution::Instructions
        }),
        ("wrong spell zone", |d| {
            d.spell.as_mut().unwrap().functioning_zone = FunctioningZone::Battlefield
        }),
        ("no target", |d| d.spell.as_mut().unwrap().targets.clear()),
        ("extra target", |d| {
            let p = d.spell.as_mut().unwrap();
            p.targets.push(p.targets[0].clone());
        }),
        ("target slot", |d| {
            d.spell.as_mut().unwrap().targets[0].slot = 1
        }),
        ("empty selection", |d| {
            d.spell.as_mut().unwrap().targets[0].selection_id.clear()
        }),
        ("controlled target", |d| {
            d.spell.as_mut().unwrap().targets[0].domain =
                TargetDomain::ControlledBattlefieldCreature
        }),
        ("permanent target", |d| {
            d.spell.as_mut().unwrap().targets[0].domain = TargetDomain::BattlefieldPermanent
        }),
        ("target filter", |d| {
            d.spell.as_mut().unwrap().targets[0].filter = Some(PermanentFilter {
                power: None,
                types: vec![CardKind::Creature],
                non_types: vec![],
                non_colors: vec![],
                opponent: false,
                tapped: false,
            });
        }),
        ("instruction", |d| {
            d.spell
                .as_mut()
                .unwrap()
                .instructions
                .push(Instruction::DrawCards {
                    count: 1.into(),
                    recipient: None,
                    source_block: 0,
                })
        }),
        ("mode", |d| {
            let p = d.spell.as_ref().unwrap().clone();
            d.spell
                .as_mut()
                .unwrap()
                .modes
                .push(SpellMode { program: p });
        }),
        ("additional cost", |d| {
            d.spell.as_mut().unwrap().additional_sacrifice = Some(SacrificeFilter {
                types: vec![CardKind::Creature],
                subtypes: vec![],
            })
        }),
        ("static zone", |d| {
            d.static_abilities[0].functioning_zone = FunctioningZone::Stack
        }),
        ("extra static", |d| {
            d.static_abilities.push(StaticAbility {
                functioning_zone: FunctioningZone::Battlefield,
                effect: StaticEffect::EntersTapped,
            })
        }),
        ("activation", |d| {
            d.activated.push(ActivatedAbility {
                functioning_zone: FunctioningZone::Battlefield,
                cost: ActivationCost::TapSource,
                program: d.spell.as_ref().unwrap().clone(),
            })
        }),
        ("mana ability", |d| {
            d.mana_abilities.push(ManaAbility {
                self_damage: None,
                restriction: None,
                costs: PermanentActivationCosts {
                    remove_source_plus_one_counters: None,
                    tap: true,
                    mana: None,
                    sacrifice: false,
                },
                production: ManaProduction::Fixed {
                    symbols: vec![Color::U],
                },
                source_block: 2,
            })
        }),
        ("trigger", |d| {
            let legacy = Catalog::read(include_str!(
                "../../../tests/fixtures/expected-attachments-ir.json"
            ))
            .unwrap();
            d.triggered = legacy
                .definitions
                .into_iter()
                .find(|d| d.name == "Rancor")
                .unwrap()
                .triggered;
        }),
    ];
    for (name, mutate) in mutations {
        let mut d = aura();
        mutate(&mut d);
        d.capabilities = d.requirements();
        assert!(!aura_control_source(&d), "{name}");
        assert!(d.validate().is_err(), "{name}");
    }
    let legacy = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-attachments-ir.json"
    ))
    .unwrap();
    for mut d in legacy.definitions {
        assert!(!aura_control_source(&d));
        assert!(!d.requirements().contains(&Capability::AuraControlV1));
        d.capabilities.insert(Capability::AuraControlV1);
        assert!(d.validate().is_err(), "spare capability on legacy source");
    }
}

#[test]
fn aura_control_schema_adds_narrow_shape_without_a_schema_bump() {
    assert_eq!(CURRENT_IR_VERSION, 1);
    let schema = catalog_schema();
    let shape = &schema["$defs"]["AuraControlDefinition"]["oneOf"][0]["properties"];
    assert_eq!(
        shape["static_abilities"]["contains"]["const"],
        serde_json::json!(
            {"functioning_zone":"battlefield","effect":"control_enchanted_permanent"}
        )
    );
    assert_eq!(shape["spell"]["properties"]["targets"]["minItems"], 1);
    assert_eq!(shape["spell"]["properties"]["targets"]["maxItems"], 1);
    assert_eq!(shape["static_abilities"]["maxItems"], 2);
    assert_eq!(shape["static_abilities"]["maxContains"], 1);
    assert_eq!(shape["capabilities"]["minItems"], 5);
    assert_eq!(shape["capabilities"]["maxItems"], 5);
}

fn composed(
    power: i64,
    toughness: i64,
    keywords: Vec<KeywordAbility>,
    restrictions: Vec<CreatureRestriction>,
) -> Definition {
    let mut d = aura();
    d.static_abilities.push(StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::ModifyCreatures {
            scope: CreatureScope::Enchanted,
            power: power.into(),
            toughness: toughness.into(),
            keywords,
            restrictions,
        },
    });
    d.capabilities = d.requirements();
    d
}

#[test]
fn aura_control_composes_existing_modifier_parameters_without_new_requirements() {
    let plain = aura();
    for (power, toughness, keywords, restrictions) in [
        (2, 2, vec![KeywordAbility::Flying], vec![]),
        (2, 2, vec![KeywordAbility::Trample], vec![]),
        (
            7,
            0,
            vec![KeywordAbility::Reach, KeywordAbility::Hexproof],
            vec![],
        ),
        (0, 3, vec![], vec![]),
        (0, 0, vec![KeywordAbility::Haste], vec![]),
        (0, 0, vec![], vec![CreatureRestriction::CantBeBlocked]),
    ] {
        let mut d = composed(power, toughness, keywords, restrictions);
        assert!(aura_control_source(&d));
        assert_eq!(d.capabilities, plain.capabilities);
        d.validate().unwrap();
        d.static_abilities.reverse();
        assert!(aura_control_source(&d));
        d.validate().unwrap();
    }
}

#[test]
fn aura_control_composition_keeps_existing_modifier_semantic_negatives() {
    for (power, toughness, keywords, restrictions) in [
        (-1, 2, vec![KeywordAbility::Flying], vec![]),
        (2, -1, vec![], vec![]),
        (0, 0, vec![], vec![]),
        (2, 2, vec![KeywordAbility::Flash], vec![]),
        (0, 0, vec![KeywordAbility::Indestructible], vec![]),
        (
            2,
            2,
            vec![KeywordAbility::Flying, KeywordAbility::Flying],
            vec![],
        ),
        (
            0,
            0,
            vec![],
            vec![
                CreatureRestriction::CantBlock,
                CreatureRestriction::CantBlock,
            ],
        ),
    ] {
        let d = composed(power, toughness, keywords, restrictions);
        assert!(!aura_control_source(&d));
        assert!(d.validate().is_err());
    }
    for scope in [
        CreatureScope::Equipped,
        CreatureScope::Controller,
        CreatureScope::Source,
        CreatureScope::AttackingController,
    ] {
        let mut d = composed(2, 2, vec![KeywordAbility::Flying], vec![]);
        if let StaticEffect::ModifyCreatures { scope: s, .. } = &mut d.static_abilities[1].effect {
            *s = scope;
        }
        d.capabilities = d.requirements();
        assert!(!aura_control_source(&d));
        assert!(d.validate().is_err());
    }
    for mutation in [0, 1, 2, 3] {
        let mut d = composed(2, 2, vec![KeywordAbility::Flying], vec![]);
        match mutation {
            0 => d.static_abilities.push(d.static_abilities[0].clone()),
            1 => d.static_abilities.push(d.static_abilities[1].clone()),
            2 => d.static_abilities[1].functioning_zone = FunctioningZone::Stack,
            _ => {
                d.static_abilities.remove(0);
            }
        };
        d.capabilities = d.requirements();
        assert!(!aura_control_source(&d));
        if mutation != 3 {
            assert!(d.validate().is_err());
        }
    }
}

// New generalized family controls are source-only preparation; native tests UNRUN.
pub(super) fn typed_aura(kind: CardKind) -> Definition {
    let mut d = aura();
    if kind != CardKind::Creature {
        let t = &mut d.spell.as_mut().unwrap().targets[0];
        t.domain = TargetDomain::BattlefieldPermanent;
        t.filter = Some(PermanentFilter {
            power: None,
            types: vec![kind],
            non_types: vec![],
            non_colors: vec![],
            opponent: false,
            tapped: false,
        });
    }
    d.capabilities = d.requirements();
    d
}

#[test]
fn aura_control_domain_is_exact_nonrecursive_and_capability_closed() {
    for kind in [CardKind::Creature, CardKind::Artifact, CardKind::Land] {
        let mut d = typed_aura(kind);
        assert_eq!(
            aura_control_domain(&d),
            Some(match kind {
                CardKind::Creature => AuraControlDomain::Creature,
                CardKind::Artifact => AuraControlDomain::Artifact,
                CardKind::Land => AuraControlDomain::Land,
                _ => unreachable!(),
            })
        );
        assert!(aura_control_source(&d));
        d.validate().unwrap();
        let target_cap = if kind == CardKind::Creature {
            Capability::CreatureTargetV1
        } else {
            Capability::PermanentRemovalV1
        };
        assert_eq!(
            d.requirements(),
            BTreeSet::from([
                Capability::NormalLayoutV1,
                Capability::CastEnchantmentV1,
                Capability::AttachmentsV1,
                Capability::AuraControlV1,
                target_cap,
            ])
        );
        for cap in d.requirements() {
            let mut missing = d.clone();
            missing.capabilities.remove(&cap);
            assert_eq!(
                aura_control_domain(&missing),
                Some(match kind {
                    CardKind::Creature => AuraControlDomain::Creature,
                    CardKind::Artifact => AuraControlDomain::Artifact,
                    CardKind::Land => AuraControlDomain::Land,
                    _ => unreachable!(),
                })
            );
            assert!(missing.validate().is_err(), "missing {cap:?} on {kind:?}");
        }
        d.capabilities.insert(if kind == CardKind::Creature {
            Capability::PermanentRemovalV1
        } else {
            Capability::CreatureTargetV1
        });
        assert!(d.validate().is_err());
    }
}

#[test]
fn aura_control_noncreature_domains_reject_all_filter_and_composition_extensions() {
    type Mutation = fn(&mut Definition);
    let mutations: &[(&str, Mutation)] = &[
        ("absent filter", |d| {
            d.spell.as_mut().unwrap().targets[0].filter = None
        }),
        ("creature domain with permanent filter", |d| {
            d.spell.as_mut().unwrap().targets[0].domain = TargetDomain::BattlefieldCreature
        }),
        ("union type", |d| {
            d.spell.as_mut().unwrap().targets[0]
                .filter
                .as_mut()
                .unwrap()
                .types
                .push(CardKind::Creature)
        }),
        ("duplicate type", |d| {
            let f = d.spell.as_mut().unwrap().targets[0]
                .filter
                .as_mut()
                .unwrap();
            f.types = vec![CardKind::Artifact, CardKind::Artifact];
        }),
        ("enchantment", |d| {
            d.spell.as_mut().unwrap().targets[0]
                .filter
                .as_mut()
                .unwrap()
                .types = vec![CardKind::Enchantment]
        }),
        ("creature permanent target", |d| {
            d.spell.as_mut().unwrap().targets[0]
                .filter
                .as_mut()
                .unwrap()
                .types = vec![CardKind::Creature]
        }),
        ("non type", |d| {
            d.spell.as_mut().unwrap().targets[0]
                .filter
                .as_mut()
                .unwrap()
                .non_types
                .push(CardKind::Creature)
        }),
        ("non color", |d| {
            d.spell.as_mut().unwrap().targets[0]
                .filter
                .as_mut()
                .unwrap()
                .non_colors
                .push(Color::B)
        }),
        ("opponent", |d| {
            d.spell.as_mut().unwrap().targets[0]
                .filter
                .as_mut()
                .unwrap()
                .opponent = true
        }),
        ("tapped", |d| {
            d.spell.as_mut().unwrap().targets[0]
                .filter
                .as_mut()
                .unwrap()
                .tapped = true
        }),
        ("additional source type", |d| {
            d.characteristics.additional_types.push(CardKind::Artifact)
        }),
        ("modifier", |d| {
            d.static_abilities.push(
                composed(2, 2, vec![KeywordAbility::Flying], vec![]).static_abilities[1].clone(),
            )
        }),
        ("prohibition", |d| {
            d.static_abilities.push(StaticAbility {
                functioning_zone: FunctioningZone::Battlefield,
                effect: StaticEffect::OpponentsCantCastDuringControllerTurn,
            })
        }),
        ("duplicate control", |d| {
            d.static_abilities.push(d.static_abilities[0].clone())
        }),
        ("target slot", |d| {
            d.spell.as_mut().unwrap().targets[0].slot = 1
        }),
        ("empty selection", |d| {
            d.spell.as_mut().unwrap().targets[0].selection_id.clear()
        }),
        ("stack static", |d| {
            d.static_abilities[0].functioning_zone = FunctioningZone::Stack
        }),
    ];
    for original in [
        typed_aura(CardKind::Artifact),
        typed_aura(CardKind::Land),
        general_aura(),
    ] {
        let kind = aura_control_domain(&original).unwrap();
        for (name, mutate) in mutations {
            let mut d = original.clone();
            mutate(&mut d);
            d.capabilities = d.requirements();
            assert_eq!(aura_control_domain(&d), None, "{kind:?}: {name}");
            assert!(!aura_control_source(&d));
            assert!(d.validate().is_err(), "{kind:?}: {name}");
        }
    }
}

#[test]
fn aura_control_new_unit_serialization_rejects_old_internal_ir() {
    assert_eq!(
        serde_json::to_value(StaticEffect::ControlEnchantedPermanent).unwrap(),
        serde_json::json!("control_enchanted_permanent")
    );
    assert!(
        serde_json::from_value::<StaticEffect>(serde_json::json!("control_enchanted_creature"))
            .is_err()
    );
    let schema = catalog_schema();
    let shapes = schema["$defs"]["AuraControlDefinition"]["oneOf"]
        .as_array()
        .unwrap();
    assert_eq!(shapes.len(), 4);
    for (i, types) in [vec![CardKind::Artifact], vec![CardKind::Land], vec![]]
        .into_iter()
        .enumerate()
    {
        let props = &shapes[i + 1]["properties"];
        assert_eq!(props["static_abilities"]["maxItems"], 1);
        assert_eq!(
            props["spell"]["properties"]["targets"]["items"]["properties"]["filter"]["properties"]
                ["types"]["const"],
            serde_json::json!(types)
        );
        assert!(
            props["capabilities"]["items"]["enum"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!(Capability::PermanentRemovalV1))
        );
        assert!(
            !props["capabilities"]["items"]["enum"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!(Capability::CreatureTargetV1))
        );
    }
}

#[test]
fn aura_control_typed_shapes_exclude_optional_cost_and_conditional_draw() {
    for baseline in [
        typed_aura(CardKind::Creature),
        typed_aura(CardKind::Artifact),
        typed_aura(CardKind::Land),
        general_aura(),
    ] {
        baseline.validate().unwrap();
        for variant in 0..3 {
            let mut invalid = baseline.clone();
            let identity = format!(
                "card:{}/faces/0/abilities/0/keywords/0#keyword",
                invalid.oracle_id
            );
            let program = invalid.spell.as_mut().unwrap();
            if variant != 1 {
                program
                    .optional_additional_costs
                    .push(OptionalAdditionalManaCost {
                        identity: identity.clone(),
                        source_block: 0,
                        mana: ManaCost {
                            generic: 2.into(),
                            colored: vec![Color::U],
                        },
                    });
            }
            if variant != 0 {
                program
                    .instructions
                    .push(Instruction::DrawCardsIfAdditionalCost {
                        cost_identity: identity,
                        count: 2.into(),
                        source_block: 1,
                    });
            }
            assert_eq!(aura_control_domain(&invalid), None);
            assert!(!aura_control_source(&invalid));
            invalid.capabilities = invalid.requirements();
            assert!(invalid.validate().is_err());
        }
    }
}

pub(super) fn general_aura() -> Definition {
    let mut d = typed_aura(CardKind::Artifact);
    d.spell.as_mut().unwrap().targets[0]
        .filter
        .as_mut()
        .unwrap()
        .types
        .clear();
    d
}

#[test]
fn aura_control_general_explicit_empty_filter_and_domain_membership() {
    let d = general_aura();
    assert_eq!(aura_control_domain(&d), Some(AuraControlDomain::Permanent));
    assert!(aura_control_source(&d));
    d.validate().unwrap();
    assert_eq!(d.capabilities, typed_aura(CardKind::Artifact).capabilities);
    for kind in [
        CardKind::Creature,
        CardKind::Artifact,
        CardKind::Land,
        CardKind::Enchantment,
        CardKind::Instant,
        CardKind::Sorcery,
    ] {
        let mut c = d.characteristics.clone();
        c.kind = kind;
        c.additional_types.clear();
        assert_eq!(
            AuraControlDomain::Permanent.accepts(&c),
            !matches!(kind, CardKind::Instant | CardKind::Sorcery)
        );
        for (domain, expected) in [
            (AuraControlDomain::Creature, CardKind::Creature),
            (AuraControlDomain::Artifact, CardKind::Artifact),
            (AuraControlDomain::Land, CardKind::Land),
        ] {
            assert_eq!(domain.accepts(&c), kind == expected);
        }
    }
    for kind in [CardKind::Creature, CardKind::Land] {
        let mut c = d.characteristics.clone();
        c.kind = kind;
        c.additional_types = vec![CardKind::Artifact];
        assert!(AuraControlDomain::Artifact.accepts(&c));
        assert!(AuraControlDomain::Permanent.accepts(&c));
    }
}

#[test]
fn aura_control_general_keeps_complete_program_capability_and_metadata_fences() {
    let original = general_aura();
    for (name, mutate) in [
        (
            "instruction",
            (|d: &mut Definition| {
                d.spell
                    .as_mut()
                    .unwrap()
                    .instructions
                    .push(Instruction::DrawCards {
                        count: 1.into(),
                        recipient: None,
                        source_block: 2,
                    })
            }) as fn(&mut Definition),
        ),
        ("mode", |d| {
            let p = d.spell.as_ref().unwrap().clone();
            d.spell
                .as_mut()
                .unwrap()
                .modes
                .push(SpellMode { program: p });
        }),
        ("cost", |d| {
            d.spell.as_mut().unwrap().additional_sacrifice = Some(SacrificeFilter {
                types: vec![CardKind::Artifact],
                subtypes: vec![],
            })
        }),
        ("timing", |d| {
            d.spell.as_mut().unwrap().timing = SpellTiming::Instant
        }),
        ("resolution", |d| {
            d.spell.as_mut().unwrap().resolution = SpellResolution::Instructions
        }),
        ("spell zone", |d| {
            d.spell.as_mut().unwrap().functioning_zone = FunctioningZone::Battlefield
        }),
        ("keyword", |d| d.keywords.push(KeywordAbility::Flying)),
        ("variable cost", |d| d.x_mana_cost = true),
        ("intrinsic zone", |d| {
            d.intrinsic_zone = Some(FunctioningZone::Battlefield)
        }),
        ("snow", |d| d.characteristics.snow = true),
        ("legendary", |d| d.characteristics.legendary = true),
        ("additional type", |d| {
            d.characteristics.additional_types.push(CardKind::Artifact)
        }),
    ] {
        let mut d = original.clone();
        mutate(&mut d);
        d.capabilities = d.requirements();
        assert_eq!(aura_control_domain(&d), None, "{name}");
        assert!(d.validate().is_err(), "{name}");
    }
    for cap in &original.capabilities {
        let mut d = original.clone();
        d.capabilities.remove(cap);
        assert_eq!(aura_control_domain(&d), Some(AuraControlDomain::Permanent));
        assert!(d.validate().is_err(), "missing {cap:?}");
    }
    let mut spare = original.clone();
    spare.capabilities.insert(Capability::CreatureTargetV1);
    assert!(spare.validate().is_err());
    let serialized = serde_json::to_value(&original).unwrap();
    for value in [serde_json::Value::Null, serde_json::json!({})] {
        let mut bad = serialized.clone();
        bad["spell"]["targets"][0]["filter"] = value;
        assert!(serde_json::from_value::<Definition>(bad).map_or(true, |d| d.validate().is_err()));
    }
    let mut missing = serialized.clone();
    missing["spell"]["targets"][0]
        .as_object_mut()
        .unwrap()
        .remove("filter");
    assert!(serde_json::from_value::<Definition>(missing).map_or(true, |d| d.validate().is_err()));
    for field in ["types", "non_types", "non_colors", "opponent"] {
        for null in [false, true] {
            let mut bad = serialized.clone();
            let obj = bad["spell"]["targets"][0]["filter"]
                .as_object_mut()
                .unwrap();
            if null {
                obj.insert(field.into(), serde_json::Value::Null);
            } else {
                obj.remove(field);
            }
            assert!(
                serde_json::from_value::<Definition>(bad).is_err(),
                "{field}: null={null}"
            );
        }
    }
    let mut metadata = original.clone();
    metadata.source_labels = None;
    metadata.validate().unwrap();
    assert_eq!(metadata.requirements(), original.requirements());
    assert_eq!(
        aura_control_domain(&metadata),
        Some(AuraControlDomain::Permanent)
    );
}
