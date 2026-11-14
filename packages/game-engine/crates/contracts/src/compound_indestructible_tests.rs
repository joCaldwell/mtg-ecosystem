//! Typed controls; complete archived source is tested independently by compiler.
use super::*;
pub(super) fn compound() -> Definition {
    let mut d = temporary_indestructible_tests::grant();
    d.spell.as_mut().unwrap().instructions.insert(
        0,
        Instruction::ModifyPowerToughness {
            target_slot: 0,
            power: 1.into(),
            toughness: 1.into(),
            duration: Duration::UntilEndOfTurn,
            source_block: 0,
        },
    );
    d.capabilities = d.requirements();
    d
}
#[test]
fn compound_indestructible_capabilities_are_distinct_from_pure_grants() {
    let d = compound();
    d.validate().unwrap();
    assert_eq!(CURRENT_IR_VERSION, 1);
    for cap in [
        Capability::CompoundIndestructibleSpellV1,
        Capability::TemporaryPowerToughnessV1,
        Capability::IndestructibleV1,
        Capability::CreatureTargetV1,
    ] {
        assert!(d.capabilities.contains(&cap));
        let mut b = d.clone();
        b.capabilities.remove(&cap);
        assert!(b.validate().is_err(), "missing {cap:?}");
    }
    for cap in [
        Capability::TemporaryIndestructibleSpellV1,
        Capability::TemporaryFlyingSpellV1,
    ] {
        assert!(!d.capabilities.contains(&cap));
        let mut b = d.clone();
        b.capabilities.insert(cap);
        assert!(b.validate().is_err(), "spare {cap:?}");
    }
    for mut pure in [
        temporary_indestructible_tests::grant(),
        temporary_flying_tests::grant(),
    ] {
        pure.validate().unwrap();
        assert!(
            !pure
                .capabilities
                .contains(&Capability::CompoundIndestructibleSpellV1)
        );
        pure.capabilities
            .insert(Capability::CompoundIndestructibleSpellV1);
        assert!(pure.validate().is_err());
    }
}
#[test]
fn compound_indestructible_rejects_structural_and_scalar_near_misses() {
    for variant in 0..21 {
        let mut b = compound();
        let p = b.spell.as_mut().unwrap();
        match variant {
            0 => p.instructions.reverse(),
            1 => p.instructions.push(p.instructions[1].clone()),
            2 => {
                p.instructions.push(p.instructions[0].clone());
            }
            3 => p.targets.push(p.targets[0].clone()),
            4 => p.targets[0].selection_id.clear(),
            5 => p.targets[0].domain = TargetDomain::ControlledBattlefieldCreature,
            6 => p.targets[0].domain = TargetDomain::BlockingBattlefieldCreature,
            7 => {
                p.targets[0].filter = Some(PermanentFilter {
                    types: vec![CardKind::Creature],
                    non_types: vec![],
                    non_colors: vec![],
                    opponent: false,
                    tapped: false,
                })
            }
            8 => p.functioning_zone = FunctioningZone::Battlefield,
            9 => b.characteristics.kind = CardKind::Creature,
            10 => {
                if let Instruction::ModifyPowerToughness { power, .. } = &mut p.instructions[0] {
                    *power = 2.into();
                }
            }
            11 => {
                if let Instruction::ModifyPowerToughness { toughness, .. } = &mut p.instructions[0]
                {
                    *toughness = (-1).into();
                }
            }
            12 => {
                if let Instruction::GrantKeywords { keywords, .. } = &mut p.instructions[1] {
                    keywords.push(KeywordAbility::Indestructible);
                }
            }
            13 => {
                if let Instruction::GrantKeywords { keywords, .. } = &mut p.instructions[1] {
                    *keywords = vec![KeywordAbility::Flying];
                }
            }
            14 => {
                if let Instruction::GrantKeywords { keywords, .. } = &mut p.instructions[1] {
                    keywords.push(KeywordAbility::Flying);
                }
            }
            15 => {
                if let Instruction::GrantKeywords { target_slot, .. } = &mut p.instructions[1] {
                    *target_slot = 1;
                }
            }
            16 => {
                if let Instruction::ModifyPowerToughness { target_slot, .. } =
                    &mut p.instructions[0]
                {
                    *target_slot = 1;
                }
            }
            17 => {
                if let Instruction::GrantKeywords { source_block, .. } = &mut p.instructions[1] {
                    *source_block = 1;
                }
            }
            18 => {
                let child = p.clone();
                p.targets.clear();
                p.instructions.clear();
                p.modes = vec![
                    SpellMode {
                        program: child.clone(),
                    },
                    SpellMode { program: child },
                ];
            }
            19 => p.instructions.push(Instruction::DrawCards {
                count: 1.into(),
                recipient: None,
                source_block: 0,
            }),
            _ => {
                p.targets.push(p.targets[0].clone());
                p.targets[1].slot = 1;
                p.targets[1].selection_id = "separate".into();
                if let Instruction::GrantKeywords { target_slot, .. } = &mut p.instructions[1] {
                    *target_slot = 1;
                }
            }
        }
        b.capabilities = b.requirements();
        assert!(b.validate().is_err(), "variant {variant}");
    }
}
#[test]
fn compound_indestructible_rejects_costs_activations_and_triggers() {
    let c = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap();
    let mut b = compound();
    b.spell.as_mut().unwrap().additional_sacrifice = c
        .definitions
        .iter()
        .find(|d| d.name == "Bone Splinters")
        .unwrap()
        .spell
        .as_ref()
        .unwrap()
        .additional_sacrifice
        .clone();
    b.capabilities = b.requirements();
    assert!(b.validate().is_err());
    let c = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-effects-ir.json"
    ))
    .unwrap();
    for name in ["Samite Healer", "Blood Artist"] {
        let mut b = c
            .definitions
            .iter()
            .find(|d| d.name == name)
            .unwrap()
            .clone();
        if let Some(a) = b.activated.first_mut() {
            a.program = compound().spell.unwrap();
        } else {
            b.triggered[0].program = compound().spell.unwrap();
        }
        b.capabilities = b.requirements();
        assert!(b.validate().is_err(), "{name}");
    }
}
