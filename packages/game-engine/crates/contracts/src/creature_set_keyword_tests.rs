//! Typed IR family boundaries; native execution is owned by the parent.
use super::*;

pub(super) fn grant(set: ObjectSet, keywords: &[KeywordAbility], modifier: bool) -> Definition {
    let mut d = temporary_creature_keyword_tests::grant(keywords, false);
    let p = d.spell.as_mut().unwrap();
    p.targets.clear();
    p.instructions = vec![Instruction::GrantKeywordsSet {
        set,
        modifier: modifier.then(|| CreatureStatModifier {
            power: Quantity::from(-2),
            toughness: Quantity::zero(),
        }),
        keywords: keywords.to_vec(),
        duration: Duration::UntilEndOfTurn,
        source_block: 0,
    }];
    d.capabilities = d.requirements();
    d
}
fn reject(mut d: Definition, change: impl FnOnce(&mut SpellProgram)) {
    change(d.spell.as_mut().unwrap());
    d.capabilities = d.requirements();
    assert!(d.validate().is_err());
}
#[test]
fn creature_set_keyword_exact_family_and_capability_closure() {
    for set in [
        ObjectSet::ControllerCreatures,
        ObjectSet::AttackingCreatures,
    ] {
        for keyword in [
            KeywordAbility::Flying,
            KeywordAbility::Deathtouch,
            KeywordAbility::Indestructible,
            KeywordAbility::Lifelink,
            KeywordAbility::Hexproof,
            KeywordAbility::Haste,
            KeywordAbility::Trample,
            KeywordAbility::FirstStrike,
        ] {
            for modifier in [false, true] {
                let d = grant(set.clone(), &[keyword], modifier);
                d.validate().unwrap();
                let p = d.spell.as_ref().unwrap();
                let desc = temporary_creature_set_keyword_program(p).unwrap();
                assert_eq!(*desc.set, set);
                assert_eq!(desc.keywords, [keyword]);
                assert_eq!(desc.modifier.is_some(), modifier);
                assert!(temporary_creature_keyword_program(p).is_none());
                assert!(compound_indestructible_parameters(p).is_none());
                assert_eq!(
                    d.capabilities,
                    BTreeSet::from([
                        Capability::NormalLayoutV1,
                        Capability::CastInstantV1,
                        Capability::BoardWipesV1,
                        Capability::TemporaryCreatureSetKeywordProgramV1,
                        temporary_keyword_primitive_capability(keyword).unwrap(),
                    ])
                );
                for cap in &d.capabilities {
                    let mut missing = d.clone();
                    missing.capabilities.remove(cap);
                    assert!(missing.validate().is_err());
                }
                for cap in [
                    Capability::TemporaryCreatureKeywordProgramV1,
                    Capability::TemporaryFlyingSpellV1,
                    Capability::TemporaryDeathtouchSpellV1,
                    Capability::TemporaryIndestructibleSpellV1,
                    Capability::CompoundIndestructibleSpellV1,
                    Capability::TemporaryPowerToughnessV1,
                    Capability::CreatureTargetV1,
                ] {
                    let mut spare = d.clone();
                    spare.capabilities.insert(cap);
                    assert!(spare.validate().is_err());
                }
            }
        }
    }
}
#[test]
fn creature_set_keyword_shapes_and_old_routes_reject() {
    let d = grant(
        ObjectSet::ControllerCreatures,
        &[KeywordAbility::Haste],
        true,
    );
    reject(d.clone(), |p| {
        p.instructions.pop();
    });
    reject(d.clone(), |p| {
        p.instructions.push(p.instructions[0].clone());
    });
    // CR 611.2c/613.6: a split pair is not one multi-layer effect, even when
    // both instructions have the same selector and source block.
    for set in [
        ObjectSet::ControllerCreatures,
        ObjectSet::AttackingCreatures,
    ] {
        let base = grant(set.clone(), &[KeywordAbility::Haste], true);
        for reverse in [false, true] {
            reject(base.clone(), |p| {
                if let Instruction::GrantKeywordsSet { modifier, .. } = &mut p.instructions[0] {
                    *modifier = None;
                }
                p.instructions.insert(
                    0,
                    Instruction::pump(
                        set.clone().kernel(kir::Context::Spell),
                        (-2).into(),
                        Quantity::zero(),
                        0,
                    ),
                );
                if reverse {
                    p.instructions.reverse();
                }
            });
        }
    }
    for block in [1, u32::MAX] {
        reject(d.clone(), |p| {
            if let Instruction::GrantKeywordsSet { source_block, .. } = &mut p.instructions[0] {
                *source_block = block;
            }
        });
    }
    for set in [
        ObjectSet::Creatures,
        ObjectSet::NonlandPermanents,
        ObjectSet::Enchantments,
        ObjectSet::OpponentCreatures,
        ObjectSet::FlyingCreatures,
        ObjectSet::NonflyingCreatures,
    ] {
        assert!(
            grant(set, &[KeywordAbility::Haste], false)
                .validate()
                .is_err()
        );
    }
    for keywords in [vec![], vec![KeywordAbility::Haste, KeywordAbility::Haste]] {
        assert!(
            grant(ObjectSet::ControllerCreatures, &keywords, false)
                .validate()
                .is_err()
        );
    }
    for i in [
        Instruction::pump(
            ObjectSet::ControllerCreatures.kernel(kir::Context::Spell),
            Quantity::from(1),
            Quantity::zero(),
            0,
        ),
        Instruction::act(
            Verb::Destroy {
                what: ObjectSet::ControllerCreatures.kernel(Context::Spell),
                regeneration_forbidden: false,
            },
            0,
        ),
        SetDamage::instruction(
            vec![ObjectSet::ControllerCreatures.kernel(Context::Spell)],
            false,
            kir::Amount::fixed(1),
            0,
        ),
    ] {
        reject(d.clone(), |p| p.instructions = vec![i]);
    }
    let targeted = temporary_creature_keyword_tests::grant(&[KeywordAbility::Haste], false);
    reject(d.clone(), |p| p.targets = targeted.spell.unwrap().targets);
    let catalog = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-effects-ir.json"
    ))
    .unwrap();
    for name in ["Samite Healer", "Blood Artist"] {
        for program in [d.spell.as_ref().unwrap().clone(), {
            let mut p = d.spell.as_ref().unwrap().clone();
            p.instructions = vec![Instruction::pump(
                ObjectSet::ControllerCreatures.kernel(kir::Context::Spell),
                Quantity::from(1),
                Quantity::zero(),
                0,
            )];
            p
        }] {
            let mut host = catalog
                .definitions
                .iter()
                .find(|d| d.name == name)
                .unwrap()
                .clone();
            if let Some(a) = host.activated.first_mut() {
                a.program = program;
            } else {
                host.triggered[0].program = program;
            }
            host.capabilities = host.requirements();
            assert!(host.validate().is_err(), "{name}");
        }
    }
    reject(d.clone(), |p| {
        let child = p.clone();
        p.instructions.clear();
        p.modes = vec![
            SpellMode {
                program: child.clone(),
            },
            SpellMode { program: child },
        ];
    });
    reject(d.clone(), |p| {
        p.additional_sacrifice = Some(SacrificeFilter {
            types: vec![CardKind::Artifact],
            subtypes: vec![],
        })
    });
    assert!(serde_json::from_value::<Instruction>(serde_json::json!({"op":"grant_keywords_set","set":"controller_creatures","keywords":["first_strike"],"duration":"until_end_of_turn","source_block":0})).is_err());
}

#[test]
fn creature_set_keyword_signed_parameters_mixed_vectors_and_pure_blocks() {
    let all = vec![
        KeywordAbility::Flying,
        KeywordAbility::Deathtouch,
        KeywordAbility::Indestructible,
        KeywordAbility::Lifelink,
        KeywordAbility::Hexproof,
        KeywordAbility::Haste,
        KeywordAbility::Trample,
        KeywordAbility::FirstStrike,
    ];
    let mut reverse = all.clone();
    reverse.reverse();
    for set in [
        ObjectSet::ControllerCreatures,
        ObjectSet::AttackingCreatures,
    ] {
        for keywords in [
            all.clone(),
            reverse.clone(),
            vec![
                KeywordAbility::FirstStrike,
                KeywordAbility::Trample,
                KeywordAbility::Lifelink,
            ],
        ] {
            for (power, toughness) in [
                ("0", "0"),
                ("-2", "3"),
                (
                    "99999999999999999999999999999999999999999",
                    "-99999999999999999999999999999999999999999",
                ),
            ] {
                let mut d = grant(set.clone(), &keywords, true);
                if let Instruction::GrantKeywordsSet {
                    modifier: Some(m), ..
                } = &mut d.spell.as_mut().unwrap().instructions[0]
                {
                    m.power = power.parse().unwrap();
                    m.toughness = toughness.parse().unwrap();
                }
                d.validate().unwrap();
                let p = d.spell.as_ref().unwrap();
                let desc = temporary_creature_set_keyword_program(p).unwrap();
                assert_eq!(desc.keywords, keywords);
                assert_eq!(desc.modifier.unwrap().0.to_string(), power);
                assert_eq!(desc.modifier.unwrap().1.to_string(), toughness);
                d.characteristics.kind = CardKind::Sorcery;
                d.spell.as_mut().unwrap().timing = SpellTiming::MainPhase;
                d.capabilities = d.requirements();
                d.validate().unwrap();
            }
        }
        let pure = grant(set, &[KeywordAbility::Haste], false);
        for block in [1, u32::MAX] {
            reject(pure.clone(), |p| {
                if let Instruction::GrantKeywordsSet { source_block, .. } = &mut p.instructions[0] {
                    *source_block = block;
                }
            });
        }
        for variant in 0..4 {
            let mut d = pure.clone();
            match variant {
                0 => d.keywords = vec![KeywordAbility::Flash],
                1 => d.static_abilities.push(StaticAbility {
                    functioning_zone: FunctioningZone::Battlefield,
                    effect: StaticEffect::CantBeCountered,
                }),
                2 => d.spell.as_mut().unwrap().timing = SpellTiming::MainPhase,
                _ => d.intrinsic_zone = Some(FunctioningZone::Battlefield),
            }
            d.capabilities = d.requirements();
            assert!(d.validate().is_err());
        }
    }
}

#[test]
fn creature_set_keyword_modifier_strict_io_and_first_strike_serialization() {
    use serde_json::json;
    let pure = grant(
        ObjectSet::ControllerCreatures,
        &[KeywordAbility::FirstStrike],
        false,
    );
    let compound = grant(
        ObjectSet::ControllerCreatures,
        &[KeywordAbility::FirstStrike],
        true,
    );
    let pure = serde_json::to_value(&pure.spell.unwrap().instructions[0]).unwrap();
    assert_eq!(pure["keywords"], json!(["first strike"]));
    assert!(pure.get("modifier").is_none());
    assert!(matches!(
        serde_json::from_value::<Instruction>(pure.clone()).unwrap(),
        Instruction::GrantKeywordsSet { modifier: None, .. }
    ));
    let compound = serde_json::to_value(&compound.spell.unwrap().instructions[0]).unwrap();
    assert_eq!(compound["modifier"], json!({"power":"-2","toughness":"0"}));
    for modifier in [
        json!({}),
        json!({"power":"1"}),
        json!({"toughness":"0"}),
        json!({"power":"1","toughness":"0","extra":true}),
        json!({"power":"-0","toughness":"0"}),
        json!({"power":1,"toughness":"0"}),
        json!({"power":{"amount":"x"},"toughness":"0"}),
    ] {
        let mut bad = compound.clone();
        bad["modifier"] = modifier;
        assert!(serde_json::from_value::<Instruction>(bad).is_err());
    }
    for (power, toughness) in [
        ("0", "0"),
        ("-2", "3"),
        (
            "9999999999999999999999999999999999999",
            "-9999999999999999999999999999999999999",
        ),
    ] {
        let mut value = compound.clone();
        value["modifier"] = json!({"power":power,"toughness":toughness});
        let decoded: Instruction = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(decoded).unwrap(), value);
    }
}

#[test]
fn creature_set_keyword_excludes_optional_cost_and_conditional_draw_families() {
    for set in [
        ObjectSet::ControllerCreatures,
        ObjectSet::AttackingCreatures,
        ObjectSet::ControllerPermanents,
    ] {
        for modifier in [false, true] {
            if set == ObjectSet::ControllerPermanents && modifier {
                continue;
            }
            let baseline = grant(set.clone(), &[KeywordAbility::Lifelink], modifier);
            baseline.validate().unwrap();
            assert!(temporary_set_keyword_program(baseline.spell.as_ref().unwrap()).is_some());
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
                assert!(temporary_set_keyword_program(program).is_none());
                invalid.capabilities = invalid.requirements();
                assert!(invalid.validate().is_err());
            }
        }
    }
}

#[test]
fn creature_set_keyword_shared_stat_type_preserves_wire_bytes() {
    let instruction = Instruction::GrantKeywordsSet {
        set: ObjectSet::ControllerCreatures,
        modifier: Some(CreatureStatModifier {
            power: "-2".parse().unwrap(),
            toughness: 3.into(),
        }),
        keywords: vec![KeywordAbility::Lifelink],
        duration: Duration::UntilEndOfTurn,
        source_block: 0,
    };
    assert_eq!(
        serde_json::to_string(&instruction).unwrap(),
        r#"{"op":"grant_keywords_set","set":"controller_creatures","modifier":{"power":"-2","toughness":"3"},"keywords":["lifelink"],"duration":"until_end_of_turn","source_block":0}"#
    );
}
