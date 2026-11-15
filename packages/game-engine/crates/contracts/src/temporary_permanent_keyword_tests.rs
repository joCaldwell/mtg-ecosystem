//! Synthetic typed controls; every new native/schema execution is UNRUN here.
use super::*;
pub(super) fn grant(life: bool) -> Definition {
    let mut d = temporary_creature_keyword_tests::grant(
        &[KeywordAbility::Hexproof, KeywordAbility::Indestructible],
        false,
    );
    let p = d.spell.as_mut().unwrap();
    p.targets[0].domain = TargetDomain::ControlledBattlefieldPermanent;
    p.targets[0].filter = Some(PermanentFilter {
        types: vec![],
        non_types: vec![],
        non_colors: vec![],
        opponent: false,
        tapped: false,
    });
    if life {
        p.instructions.push(Instruction::GainLife {
            recipient: LifeRecipient::Controller,
            amount: Quantity::from(2),
            source_block: 0,
        });
    }
    d.capabilities = d.requirements();
    d
}
#[test]
fn temporary_permanent_keyword_contract_exact_union_and_capabilities() {
    for life in [false, true] {
        let d = grant(life);
        d.validate().unwrap();
        let p = d.spell.as_ref().unwrap();
        let a = temporary_targeted_keyword_program(p).unwrap();
        let b = temporary_permanent_keyword_program(p).unwrap();
        assert_eq!(a.keywords, b.keywords);
        assert!(a.modifier.is_none());
        assert_eq!(a.controller_life.cloned(), life.then(|| Quantity::from(2)));
        assert!(temporary_creature_keyword_program(p).is_none());
        assert!(temporary_set_keyword_program(p).is_none());
        assert!(compound_indestructible_parameters(p).is_none());
        let mut expected = BTreeSet::from([
            Capability::NormalLayoutV1,
            Capability::CastInstantV1,
            Capability::PermanentRemovalV1,
            Capability::TemporaryPermanentKeywordProgramV1,
            Capability::AttachmentsV1,
            Capability::IndestructibleV1,
        ]);
        if life {
            expected.insert(Capability::LifeGainV1);
        }
        assert_eq!(d.requirements(), expected);
        for cap in expected {
            let mut bad = d.clone();
            bad.capabilities.remove(&cap);
            assert!(bad.validate().is_err());
        }
        for cap in [
            Capability::CreatureTargetV1,
            Capability::TemporaryCreatureKeywordProgramV1,
            Capability::TemporaryPermanentSetKeywordProgramV1,
            Capability::TemporaryPowerToughnessV1,
        ] {
            let mut bad = d.clone();
            bad.capabilities.insert(cap);
            assert!(bad.validate().is_err());
        }
    }
}
#[test]
fn temporary_permanent_keyword_contract_vectors_signed_stats_and_exact_life() {
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
        for life in [false, true] {
            let mut d = grant(life);
            let p = d.spell.as_mut().unwrap();
            let Instruction::GrantKeywords { keywords, .. } = &mut p.instructions[0] else {
                unreachable!()
            };
            *keywords = vec![keyword];
            d.capabilities = d.requirements();
            d.validate().unwrap();
        }
    }
    for amount in ["0", "2", "99999999999999999999999999999999"] {
        let mut d = grant(true);
        let Instruction::GainLife { amount: a, .. } =
            &mut d.spell.as_mut().unwrap().instructions[1]
        else {
            unreachable!()
        };
        *a = amount.parse().unwrap();
        d.validate().unwrap();
    }
    for amount in [-1, -20] {
        let mut d = grant(true);
        let Instruction::GainLife { amount: a, .. } =
            &mut d.spell.as_mut().unwrap().instructions[1]
        else {
            unreachable!()
        };
        *a = Quantity::from(amount);
        assert!(d.validate().is_err());
    }
}
#[test]
fn temporary_permanent_keyword_contract_rejects_target_instruction_and_filter_mutants() {
    for case in 0..27 {
        let mut d = grant(true);
        let p = d.spell.as_mut().unwrap();
        match case {
            0 => p.targets[0].filter = None,
            1 => p.targets[0]
                .filter
                .as_mut()
                .unwrap()
                .types
                .push(CardKind::Creature),
            2 => p.targets[0]
                .filter
                .as_mut()
                .unwrap()
                .non_types
                .push(CardKind::Land),
            3 => p.targets[0]
                .filter
                .as_mut()
                .unwrap()
                .non_colors
                .push(Color::B),
            4 => p.targets[0].filter.as_mut().unwrap().opponent = true,
            5 => p.targets[0].filter.as_mut().unwrap().tapped = true,
            6 => p.targets[0].domain = TargetDomain::BattlefieldPermanent,
            7 => {
                p.targets[0].domain = TargetDomain::ControlledBattlefieldCreature;
                p.targets[0].filter = None;
            }
            8 => {
                p.targets[0].domain = TargetDomain::BattlefieldCreature;
                p.targets[0].filter = None;
            }
            9 => p.targets[0].slot = 1,
            10 => p.targets[0].selection_id.clear(),
            11 => p.targets.push(p.targets[0].clone()),
            12 => p.instructions.reverse(),
            13 => p.instructions.push(p.instructions[0].clone()),
            14 => {
                p.instructions.remove(0);
            }
            15 => {
                if let Instruction::GrantKeywords { modifier, .. } = &mut p.instructions[0] {
                    *modifier = Some(CreatureStatModifier {
                        power: Quantity::zero(),
                        toughness: Quantity::zero(),
                    });
                }
            }
            16 => {
                if let Instruction::GrantKeywords { source_block, .. } = &mut p.instructions[0] {
                    *source_block = 1;
                }
            }
            17 => {
                if let Instruction::GainLife { source_block, .. } = &mut p.instructions[1] {
                    *source_block = 1;
                }
            }
            18 => {
                if let Instruction::GainLife { recipient, .. } = &mut p.instructions[1] {
                    *recipient = LifeRecipient::EachOpponent;
                }
            }
            19 => p.functioning_zone = FunctioningZone::Battlefield,
            20 => p.resolution = SpellResolution::Permanent,
            21 => p.modes.push(SpellMode {
                program: grant(false).spell.unwrap(),
            }),
            22 => {
                p.additional_sacrifice = Some(SacrificeFilter {
                    types: vec![CardKind::Artifact],
                    subtypes: vec![],
                })
            }
            23 => {
                if let Instruction::GrantKeywords { keywords, .. } = &mut p.instructions[0] {
                    keywords.clear();
                }
            }
            24 => {
                if let Instruction::GrantKeywords { keywords, .. } = &mut p.instructions[0] {
                    keywords.push(keywords[0]);
                }
            }
            25 => {
                if let Instruction::GrantKeywords { keywords, .. } = &mut p.instructions[0] {
                    *keywords = vec![KeywordAbility::Shroud];
                }
            }
            _ => {
                if let Instruction::GrantKeywords { target_slot, .. } = &mut p.instructions[0] {
                    *target_slot = 1;
                }
            }
        }
        assert!(
            temporary_targeted_keyword_program(p).is_none(),
            "case {case}"
        );
        d.capabilities = d.requirements();
        assert!(d.validate().is_err(), "case {case}");
    }
}
#[test]
fn temporary_permanent_keyword_contract_definition_contexts_reject() {
    for case in 0..8 {
        let mut d = grant(false);
        match case {
            0 => d.characteristics.kind = CardKind::Creature,
            1 => d.x_mana_cost = true,
            2 => d.keywords.push(KeywordAbility::Flash),
            3 => {
                d.mana_abilities = Catalog::read(include_str!(
                    "../../../tests/fixtures/expected-effects-ir.json"
                ))
                .unwrap()
                .definitions
                .iter()
                .find(|d| !d.mana_abilities.is_empty())
                .unwrap()
                .mana_abilities
                .clone()
            }
            4 => d.intrinsic_zone = Some(FunctioningZone::Battlefield),
            5 => d.characteristics.additional_types.push(CardKind::Artifact),
            6 => d.characteristics.power = Some(Quantity::from(1)),
            _ => d.characteristics.legendary = true,
        }
        d.capabilities = d.requirements();
        assert!(d.validate().is_err(), "case {case}");
    }
}
#[test]
fn temporary_permanent_keyword_contract_creature_wire_and_wrappers_stay_narrow() {
    for controlled in [false, true] {
        for modifier in [false, true] {
            let mut d = temporary_creature_keyword_tests::grant(
                &[KeywordAbility::Indestructible],
                modifier,
            );
            if controlled {
                d.spell.as_mut().unwrap().targets[0].domain =
                    TargetDomain::ControlledBattlefieldCreature;
            }
            if !modifier {
                if let Instruction::GrantKeywords { source_block, .. } =
                    &mut d.spell.as_mut().unwrap().instructions[0]
                {
                    *source_block = 7;
                }
            }
            d.capabilities = d.requirements();
            d.validate().unwrap();
            let bytes = canonical(&d).unwrap();
            let p = d.spell.as_ref().unwrap();
            assert!(temporary_creature_keyword_program(p).is_some());
            assert!(
                temporary_targeted_keyword_program(p)
                    .unwrap()
                    .controller_life
                    .is_none()
            );
            assert!(temporary_permanent_keyword_program(p).is_none());
            assert_eq!(canonical(&d).unwrap(), bytes);
        }
    }
}

#[test]
fn temporary_permanent_keyword_zero_tail_exception_keeps_generic_life_strict() {
    let c = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-effects-ir.json"
    ))
    .unwrap();
    for name in ["Sacred Nectar", "Healing Hands", "Soul Feast"] {
        let original = c.definitions.iter().find(|d| d.name == name).unwrap();
        for index in 0..original.spell.as_ref().unwrap().instructions.len() {
            if !matches!(
                original.spell.as_ref().unwrap().instructions[index],
                Instruction::GainLife { .. } | Instruction::LoseLife { .. }
            ) {
                continue;
            }
            let mut d = original.clone();
            match &mut d.spell.as_mut().unwrap().instructions[index] {
                Instruction::GainLife { amount, .. } | Instruction::LoseLife { amount, .. } => {
                    *amount = Quantity::zero()
                }
                _ => unreachable!(),
            }
            d.capabilities = d.requirements();
            assert!(d.validate().is_err(), "{name}:{index}");
        }
    }
}
