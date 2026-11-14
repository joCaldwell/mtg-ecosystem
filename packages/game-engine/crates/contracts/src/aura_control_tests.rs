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
        effect: StaticEffect::ControlEnchantedCreature,
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
                costs: PermanentActivationCosts {
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
    let shape = &schema["$defs"]["AuraControlDefinition"]["properties"];
    assert_eq!(
        shape["static_abilities"]["contains"]["const"],
        serde_json::json!(
            {"functioning_zone":"battlefield","effect":"control_enchanted_creature"}
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
