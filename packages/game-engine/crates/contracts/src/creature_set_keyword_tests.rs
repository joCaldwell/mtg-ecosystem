//! Typed IR family boundaries; native execution is owned by the parent.
use super::*;

pub(super) fn grant(set: ObjectSet, keywords: &[KeywordAbility], modifier: bool) -> Definition {
    let mut d = temporary_creature_keyword_tests::grant(keywords, false);
    let p = d.spell.as_mut().unwrap();
    p.targets.clear();
    p.instructions = vec![Instruction::GrantKeywordsSet {
        set: set.clone(),
        keywords: keywords.to_vec(),
        duration: Duration::UntilEndOfTurn,
        source_block: 0,
    }];
    if modifier {
        p.instructions.insert(
            0,
            Instruction::PumpSet {
                set,
                power: Quantity::from(-2),
                toughness: Quantity::zero(),
                source_block: 0,
            },
        );
    }
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
    reject(d.clone(), |p| p.instructions.reverse());
    reject(d.clone(), |p| {
        p.instructions.pop();
    });
    reject(d.clone(), |p| {
        let i = p.instructions[1].clone();
        p.instructions.push(i);
    });
    reject(d.clone(), |p| {
        if let Instruction::PumpSet { set, .. } = &mut p.instructions[0] {
            *set = ObjectSet::AttackingCreatures;
        }
    });
    for block in [1, u32::MAX] {
        reject(d.clone(), |p| {
            if let Instruction::GrantKeywordsSet { source_block, .. } = &mut p.instructions[1] {
                *source_block = block;
            }
        });
        reject(d.clone(), |p| {
            if let Instruction::PumpSet { source_block, .. } = &mut p.instructions[0] {
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
    for keywords in [
        vec![],
        vec![KeywordAbility::Haste, KeywordAbility::Haste],
        vec![KeywordAbility::Reach],
    ] {
        assert!(
            grant(ObjectSet::ControllerCreatures, &keywords, false)
                .validate()
                .is_err()
        );
    }
    for i in [
        Instruction::MoveSet {
            set: ObjectSet::ControllerCreatures,
            action: SetMove::Destroy,
            source_block: 0,
        },
        Instruction::DamageSet {
            sets: vec![ObjectSet::ControllerCreatures],
            each_player: false,
            amount: WipeAmount::Fixed {
                value: Quantity::from(1),
            },
            source_block: 0,
        },
    ] {
        reject(d.clone(), |p| p.instructions = vec![i]);
    }
    let targeted = temporary_creature_keyword_tests::grant(&[KeywordAbility::Haste], false);
    reject(d.clone(), |p| p.targets = targeted.spell.unwrap().targets);
    let mut nested = d.clone();
    let program = nested.spell.take().unwrap();
    // Reuse a valid activated context so the family-specific guard is exercised.
    let mut host = temporary_creature_keyword_tests::grant(&[KeywordAbility::Haste], false);
    host.spell = Some(program.clone());
    host.characteristics.kind = CardKind::Creature;
    host.capabilities = host.requirements();
    assert!(host.validate().is_err());
    assert!(serde_json::from_value::<Instruction>(serde_json::json!({"op":"grant_keywords_set","set":"controller_creatures","keywords":["first_strike"],"duration":"until_end_of_turn","source_block":0})).is_err());
}
