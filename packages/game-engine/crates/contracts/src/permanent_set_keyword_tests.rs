//! Synthetic typed IR controls, not printed-card/admission claims. Native UNRUN.
use super::*;
use serde_json::json;

fn grant(keywords: &[KeywordAbility]) -> Definition {
    creature_set_keyword_tests::grant(ObjectSet::ControllerPermanents, keywords, false)
}
fn reject(mut d: Definition, change: impl FnOnce(&mut SpellProgram)) {
    change(d.spell.as_mut().unwrap());
    d.capabilities = d.requirements();
    assert!(d.validate().is_err());
}
#[test]
fn permanent_set_keyword_shared_descriptor_and_exact_capability_closure() {
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
    let mut vectors: Vec<_> = all.iter().map(|k| vec![*k]).collect();
    vectors.push(vec![
        KeywordAbility::Hexproof,
        KeywordAbility::Indestructible,
    ]);
    vectors.push(all.clone());
    vectors.push(all.into_iter().rev().collect());
    for keywords in vectors {
        for sorcery in [false, true] {
            let mut d = grant(&keywords);
            if sorcery {
                d.characteristics.kind = CardKind::Sorcery;
                d.spell.as_mut().unwrap().timing = SpellTiming::MainPhase;
                d.capabilities = d.requirements();
            }
            d.validate().unwrap();
            let p = d.spell.as_ref().unwrap();
            let desc: TemporarySetKeywordProgram<'_> = temporary_set_keyword_program(p).unwrap();
            assert_eq!(*desc.set, ObjectSet::ControllerPermanents);
            assert_eq!(desc.keywords, keywords);
            assert!(desc.modifier.is_none());
            assert_eq!(
                temporary_permanent_set_keyword_program(p).unwrap().keywords,
                keywords
            );
            assert!(temporary_creature_set_keyword_program(p).is_none());
            assert!(temporary_creature_keyword_program(p).is_none());
            assert!(compound_indestructible_parameters(p).is_none());
            let mut expected = BTreeSet::from([
                Capability::NormalLayoutV1,
                if sorcery {
                    Capability::CastSorceryV1
                } else {
                    Capability::CastInstantV1
                },
                Capability::BoardWipesV1,
                Capability::TemporaryPermanentSetKeywordProgramV1,
            ]);
            expected.extend(
                keywords
                    .iter()
                    .map(|k| temporary_keyword_primitive_capability(*k).unwrap()),
            );
            assert_eq!(d.capabilities, expected);
            for cap in &expected {
                let mut missing = d.clone();
                missing.capabilities.remove(cap);
                assert!(missing.validate().is_err());
            }
            for cap in [
                Capability::TemporaryCreatureSetKeywordProgramV1,
                Capability::TemporaryCreatureKeywordProgramV1,
                Capability::TemporaryFlyingSpellV1,
                Capability::TemporaryDeathtouchSpellV1,
                Capability::TemporaryIndestructibleSpellV1,
                Capability::CompoundIndestructibleSpellV1,
                Capability::CreatureTargetV1,
                Capability::TemporaryPowerToughnessV1,
                Capability::ReachV1,
                Capability::FlashV1,
                Capability::AttachmentsV1,
                Capability::IndestructibleV1,
            ] {
                if expected.contains(&cap) {
                    continue;
                }
                let mut spare = d.clone();
                spare.capabilities.insert(cap);
                assert!(spare.validate().is_err());
            }
        }
    }
    // Narrow wrappers stay disjoint; no creature modifier semantics widen.
    for set in [
        ObjectSet::ControllerCreatures,
        ObjectSet::AttackingCreatures,
    ] {
        for modifier in [false, true] {
            let d =
                creature_set_keyword_tests::grant(set.clone(), &[KeywordAbility::Haste], modifier);
            d.validate().unwrap();
            let p = d.spell.as_ref().unwrap();
            assert!(temporary_creature_set_keyword_program(p).is_some());
            assert!(temporary_permanent_set_keyword_program(p).is_none());
            assert!(
                !d.capabilities
                    .contains(&Capability::TemporaryPermanentSetKeywordProgramV1)
            );
        }
    }
}
#[test]
fn permanent_set_keyword_all_stat_modifiers_and_malformed_vectors_reject() {
    let d = grant(&[KeywordAbility::Hexproof, KeywordAbility::Indestructible]);
    for (power, toughness) in [
        ("0", "0"),
        ("-2", "3"),
        ("1", "-1"),
        (
            "999999999999999999999999999999999999",
            "-999999999999999999999999999999999999",
        ),
    ] {
        let mut bad = d.clone();
        if let Instruction::GrantKeywordsSet { modifier, .. } =
            &mut bad.spell.as_mut().unwrap().instructions[0]
        {
            *modifier = Some(CreatureStatModifier {
                power: power.parse().unwrap(),
                toughness: toughness.parse().unwrap(),
            });
        }
        let p = bad.spell.as_ref().unwrap();
        assert!(temporary_set_keyword_program(p).is_none());
        assert!(temporary_permanent_set_keyword_program(p).is_none());
        bad.capabilities = bad.requirements();
        assert!(bad.validate().is_err());
    }
    for keywords in [
        vec![],
        vec![KeywordAbility::Hexproof, KeywordAbility::Hexproof],
        vec![KeywordAbility::Shroud],
        vec![KeywordAbility::Flash],
    ] {
        assert!(grant(&keywords).validate().is_err());
    }
    let encoded = serde_json::to_value(&d.spell.as_ref().unwrap().instructions[0]).unwrap();
    assert_eq!(encoded["set"], "controller_permanents");
    assert_eq!(encoded["keywords"], json!(["hexproof", "indestructible"]));
    assert!(encoded.get("modifier").is_none());
    let mut explicit_none = encoded.clone();
    explicit_none["modifier"] = json!(null);
    assert_eq!(
        serde_json::from_value::<Instruction>(explicit_none).unwrap(),
        d.spell.as_ref().unwrap().instructions[0]
    );
    for (key, value) in [
        ("set", json!({"cls":"permanent","control":"you"})),
        ("set", json!("permanents")),
        ("keywords", json!(["first_strike"])),
        ("target_slot", json!(0)),
        ("modifier", json!({"power":"0"})),
    ] {
        let mut bad = encoded.clone();
        bad[key] = value;
        assert!(serde_json::from_value::<Instruction>(bad).is_err());
    }
}
#[test]
fn permanent_set_keyword_every_other_opcode_and_context_rejects() {
    let d = grant(&[KeywordAbility::Hexproof, KeywordAbility::Indestructible]);
    for instruction in [
        Instruction::MoveSet {
            set: ObjectSet::ControllerPermanents,
            action: SetMove::Destroy,
            source_block: 0,
        },
        Instruction::MoveSet {
            set: ObjectSet::ControllerPermanents,
            action: SetMove::Exile,
            source_block: 0,
        },
        Instruction::MoveSet {
            set: ObjectSet::ControllerPermanents,
            action: SetMove::Hand,
            source_block: 0,
        },
        Instruction::DamageSet {
            sets: vec![ObjectSet::ControllerPermanents],
            each_player: false,
            amount: WipeAmount::Fixed { value: 1.into() },
            source_block: 0,
        },
        Instruction::PumpSet {
            set: ObjectSet::ControllerPermanents,
            power: 0.into(),
            toughness: 0.into(),
            source_block: 0,
        },
        d.spell.as_ref().unwrap().instructions[0].clone(),
    ] {
        // The grant is the sole positive top-level shape, but all other opcodes
        // reject there even if capabilities are recomputed from the mutation.
        if !matches!(instruction, Instruction::GrantKeywordsSet { .. }) {
            reject(d.clone(), |p| p.instructions = vec![instruction.clone()]);
        }
        for host in ["Samite Healer", "Blood Artist"] {
            let catalog = Catalog::read(include_str!(
                "../../../tests/fixtures/expected-effects-ir.json"
            ))
            .unwrap();
            let mut context = catalog
                .definitions
                .iter()
                .find(|d| d.name == host)
                .unwrap()
                .clone();
            let mut program = d.spell.as_ref().unwrap().clone();
            program.instructions = vec![instruction.clone()];
            if let Some(a) = context.activated.first_mut() {
                a.program = program;
            } else {
                context.triggered[0].program = program;
            }
            context.capabilities = context.requirements();
            assert!(context.validate().is_err());
        }
        reject(d.clone(), |p| {
            let mut child = p.clone();
            child.instructions = vec![instruction.clone()];
            p.instructions.clear();
            p.modes = vec![
                SpellMode {
                    program: child.clone(),
                },
                SpellMode { program: child },
            ];
        });
    }
    for variant in 0..9 {
        reject(d.clone(), |p| match variant {
            0 => {
                p.targets = temporary_creature_keyword_tests::grant(&[KeywordAbility::Haste], false)
                    .spell
                    .unwrap()
                    .targets
            }
            1 => {
                p.additional_sacrifice = Some(SacrificeFilter {
                    types: vec![CardKind::Artifact],
                    subtypes: vec![],
                })
            }
            2 => p.functioning_zone = FunctioningZone::Battlefield,
            3 => p.resolution = SpellResolution::Permanent,
            4 => p.instructions.push(p.instructions[0].clone()),
            5 => p.instructions.clear(),
            6 => p.timing = SpellTiming::MainPhase,
            _ => {
                if let Instruction::GrantKeywordsSet { source_block, .. } = &mut p.instructions[0] {
                    *source_block = if variant == 7 { 1 } else { u32::MAX };
                }
            }
        });
    }
    for variant in 0..5 {
        let mut bad = d.clone();
        match variant {
            0 => bad.keywords = vec![KeywordAbility::Flash],
            1 => bad.static_abilities.push(StaticAbility {
                functioning_zone: FunctioningZone::Battlefield,
                effect: StaticEffect::CantBeCountered,
            }),
            2 => bad.intrinsic_zone = Some(FunctioningZone::Battlefield),
            3 => bad.characteristics.kind = CardKind::Creature,
            _ => bad.x_mana_cost = true,
        }
        bad.capabilities = bad.requirements();
        assert!(bad.validate().is_err());
    }
}
