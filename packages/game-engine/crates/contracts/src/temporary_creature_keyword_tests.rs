//! Synthetic typed family controls. New-source native tests are UNRUN here;
//! the parent owns combined contracts/compiler/runtime/schema qualification.
use super::*;

pub(super) fn grant(keywords: &[KeywordAbility], modifier: bool) -> Definition {
    let mut d = if modifier {
        compound_indestructible_tests::compound()
    } else {
        temporary_indestructible_tests::grant()
    };
    let Instruction::GrantKeywords {
        keywords: granted, ..
    } = d.spell.as_mut().unwrap().instructions.last_mut().unwrap()
    else {
        unreachable!()
    };
    *granted = keywords.to_vec();
    d.capabilities = d.requirements();
    d
}

fn grant_domain(keywords: &[KeywordAbility], modifier: bool, domain: TargetDomain) -> Definition {
    let mut d = grant(keywords, modifier);
    d.spell.as_mut().unwrap().targets[0].domain = domain;
    d.capabilities = d.requirements();
    d
}

#[test]
fn temporary_creature_keyword_family_preserves_subset_fences() {
    for d in [
        temporary_flying_tests::grant(),
        temporary_deathtouch_tests::grant(),
        temporary_indestructible_tests::grant(),
        compound_indestructible_tests::compound(),
    ] {
        d.validate().unwrap();
        assert!(
            !d.requirements()
                .contains(&Capability::TemporaryCreatureKeywordProgramV1)
        );
        let mut spare = d;
        spare
            .capabilities
            .insert(Capability::TemporaryCreatureKeywordProgramV1);
        assert!(spare.validate().is_err());
    }
    for (keywords, modifier) in [
        (vec![KeywordAbility::Lifelink], false),
        (
            vec![KeywordAbility::Flying, KeywordAbility::Indestructible],
            false,
        ),
        (vec![KeywordAbility::Flying], true),
        (vec![KeywordAbility::Deathtouch], true),
        (vec![KeywordAbility::Lifelink], true),
    ] {
        let d = grant(&keywords, modifier);
        d.validate().unwrap();
        assert!(
            d.capabilities
                .contains(&Capability::TemporaryCreatureKeywordProgramV1)
        );
        let descriptor = temporary_creature_keyword_program(d.spell.as_ref().unwrap()).unwrap();
        assert_eq!(descriptor.keywords, keywords);
        assert_eq!(descriptor.modifier.is_some(), modifier);
    }
}

fn vectors(prefix: &mut Vec<KeywordAbility>, result: &mut Vec<Vec<KeywordAbility>>) {
    if !prefix.is_empty() {
        result.push(prefix.clone());
    }
    for keyword in [
        KeywordAbility::Flying,
        KeywordAbility::Deathtouch,
        KeywordAbility::Indestructible,
        KeywordAbility::Lifelink,
        KeywordAbility::Hexproof,
    ] {
        if !prefix.contains(&keyword) {
            prefix.push(keyword);
            vectors(prefix, result);
            prefix.pop();
        }
    }
}

#[test]
fn temporary_creature_keyword_family_all_ordered_vectors_have_exact_requirements() {
    let mut combinations = vec![];
    vectors(&mut vec![], &mut combinations);
    assert_eq!(combinations.len(), 325);
    for keywords in combinations {
        assert!(supported_temporary_keywords(&keywords));
        for (modifier, domain) in [false, true].into_iter().flat_map(|modifier| {
            [
                TargetDomain::BattlefieldCreature,
                TargetDomain::ControlledBattlefieldCreature,
            ]
            .into_iter()
            .map(move |domain| (modifier, domain))
        }) {
            let d = grant_domain(&keywords, modifier, domain);
            let mut expected = BTreeSet::from([
                Capability::NormalLayoutV1,
                Capability::CastInstantV1,
                if domain == TargetDomain::BattlefieldCreature {
                    Capability::CreatureTargetV1
                } else {
                    Capability::AttachmentsV1
                },
            ]);
            expected.insert(match (domain, modifier, keywords.as_slice()) {
                (TargetDomain::BattlefieldCreature, false, [KeywordAbility::Flying]) => {
                    Capability::TemporaryFlyingSpellV1
                }
                (TargetDomain::BattlefieldCreature, false, [KeywordAbility::Deathtouch]) => {
                    Capability::TemporaryDeathtouchSpellV1
                }
                (TargetDomain::BattlefieldCreature, false, [KeywordAbility::Indestructible]) => {
                    Capability::TemporaryIndestructibleSpellV1
                }
                (TargetDomain::BattlefieldCreature, true, [KeywordAbility::Indestructible]) => {
                    Capability::CompoundIndestructibleSpellV1
                }
                _ => Capability::TemporaryCreatureKeywordProgramV1,
            });
            if modifier {
                expected.insert(Capability::TemporaryPowerToughnessV1);
            }
            for keyword in &keywords {
                expected.insert(match keyword {
                    KeywordAbility::Flying => Capability::FlyingV1,
                    KeywordAbility::Deathtouch => Capability::DeathtouchV1,
                    KeywordAbility::Indestructible => Capability::IndestructibleV1,
                    KeywordAbility::Lifelink => Capability::LifelinkV1,
                    KeywordAbility::Hexproof => Capability::AttachmentsV1,
                    _ => unreachable!(),
                });
            }
            assert_eq!(d.requirements(), expected);
            d.validate().unwrap();
            let raw = canonical(&d).unwrap();
            let roundtrip: Definition = strict_read(&raw).unwrap();
            assert_eq!(roundtrip, d);
            roundtrip.validate().unwrap();
            for cap in &expected {
                let mut missing = d.clone();
                missing.capabilities.remove(cap);
                assert!(
                    missing.validate().is_err(),
                    "missing {cap:?}: {keywords:?}/{modifier}"
                );
            }
            for cap in [
                Capability::TemporaryFlyingSpellV1,
                Capability::TemporaryDeathtouchSpellV1,
                Capability::TemporaryIndestructibleSpellV1,
                Capability::CompoundIndestructibleSpellV1,
                Capability::TemporaryCreatureKeywordProgramV1,
                Capability::FlyingV1,
                Capability::DeathtouchV1,
                Capability::IndestructibleV1,
                Capability::LifelinkV1,
                Capability::TemporaryPowerToughnessV1,
                Capability::ReachV1,
                Capability::CreatureTargetV1,
                Capability::AttachmentsV1,
            ] {
                if !expected.contains(&cap) {
                    let mut spare = d.clone();
                    spare.capabilities.insert(cap);
                    assert!(
                        spare.validate().is_err(),
                        "spare {cap:?}: {keywords:?}/{modifier}"
                    );
                }
            }
        }
    }
}

#[test]
fn temporary_creature_keyword_family_preserves_exact_parameters_and_pure_provenance() {
    for (power, toughness) in [
        ("0", "0"),
        ("-2", "3"),
        (
            "99999999999999999999999999999999999999999",
            "-99999999999999999999999999999999999999999",
        ),
    ] {
        let mut d = grant(&[KeywordAbility::Flying, KeywordAbility::Lifelink], true);
        let p = d.spell.as_mut().unwrap();
        let Instruction::GrantKeywords {
            modifier:
                Some(CreatureStatModifier {
                    power: actual_p,
                    toughness: actual_t,
                }),
            ..
        } = &mut p.instructions[0]
        else {
            unreachable!()
        };
        *actual_p = power.parse().unwrap();
        *actual_t = toughness.parse().unwrap();
        let descriptor = temporary_creature_keyword_program(p).unwrap();
        let (actual_p, actual_t) = descriptor.modifier.unwrap();
        assert_eq!(actual_p.to_string(), power);
        assert_eq!(actual_t.to_string(), toughness);
        assert!(compound_indestructible_parameters(p).is_none());
        d.validate().unwrap();
        // The helper is program-only; ordinary instant/sorcery kind and timing
        // are checked by Definition, preserving the existing caller contract.
        d.characteristics.kind = CardKind::Sorcery;
        d.spell.as_mut().unwrap().timing = SpellTiming::MainPhase;
        d.capabilities = d.requirements();
        d.validate().unwrap();
    }
    for keyword in [
        KeywordAbility::Flying,
        KeywordAbility::Deathtouch,
        KeywordAbility::Indestructible,
        KeywordAbility::Lifelink,
        KeywordAbility::Hexproof,
    ] {
        for (block, domain) in [0, 7, u32::MAX].into_iter().flat_map(|block| {
            [
                TargetDomain::BattlefieldCreature,
                TargetDomain::ControlledBattlefieldCreature,
            ]
            .into_iter()
            .map(move |domain| (block, domain))
        }) {
            let mut d = grant_domain(&[keyword], false, domain);
            let Instruction::GrantKeywords { source_block, .. } =
                &mut d.spell.as_mut().unwrap().instructions[0]
            else {
                unreachable!()
            };
            *source_block = block;
            assert!(temporary_creature_keyword_program(d.spell.as_ref().unwrap()).is_some());
            d.validate().unwrap();
            let roundtrip: Definition = strict_read(&canonical(&d).unwrap()).unwrap();
            assert_eq!(roundtrip, d);
        }
    }
}

#[test]
fn temporary_creature_keyword_family_rejects_empty_duplicate_and_unreviewed_vocabulary() {
    assert!(!supported_temporary_keywords(&[]));
    for keywords in [
        vec![],
        vec![KeywordAbility::Flying, KeywordAbility::Flying],
        vec![
            KeywordAbility::Lifelink,
            KeywordAbility::Deathtouch,
            KeywordAbility::Lifelink,
        ],
        vec![KeywordAbility::Defender],
        vec![KeywordAbility::Defender],
        vec![KeywordAbility::DoubleStrike],
        vec![KeywordAbility::Menace],
        vec![KeywordAbility::Flash],
        vec![KeywordAbility::Shroud],
    ] {
        assert!(!supported_temporary_keywords(&keywords));
        for modifier in [false, true] {
            let d = grant(&keywords, modifier);
            assert!(temporary_creature_keyword_program(d.spell.as_ref().unwrap()).is_none());
            assert!(d.validate().is_err(), "{keywords:?}/{modifier}");
        }
    }
}

#[test]
fn temporary_creature_keyword_family_rejects_complete_program_near_misses() {
    for (modifier, domain) in [false, true].into_iter().flat_map(|modifier| {
        [
            TargetDomain::BattlefieldCreature,
            TargetDomain::ControlledBattlefieldCreature,
        ]
        .into_iter()
        .map(move |domain| (modifier, domain))
    }) {
        for variant in 0..17 {
            let mut d = grant_domain(
                &[KeywordAbility::Flying, KeywordAbility::Hexproof],
                modifier,
                domain,
            );
            let p = d.spell.as_mut().unwrap();
            match variant {
                0 => p.targets.clear(),
                1 => p.targets.push(p.targets[0].clone()),
                2 => p.targets[0].slot = 1,
                3 => p.targets[0].selection_id.clear(),
                4 => p.targets[0].domain = TargetDomain::AnyPlayer,
                5 => p.targets[0].domain = TargetDomain::BlockingBattlefieldCreature,
                6 => p.targets[0].domain = TargetDomain::BattlefieldPermanent,
                7 => {
                    p.targets[0].filter = Some(PermanentFilter {
                        types: vec![],
                        non_types: vec![],
                        non_colors: vec![],
                        opponent: false,
                        tapped: false,
                    })
                }
                8 => p.functioning_zone = FunctioningZone::Battlefield,
                9 => p.resolution = SpellResolution::Permanent,
                10 => {
                    p.additional_sacrifice = Some(SacrificeFilter {
                        types: vec![CardKind::Artifact],
                        subtypes: vec![],
                    })
                }
                11 => {
                    let child = p.clone();
                    p.modes.push(SpellMode { program: child });
                }
                12 => p.instructions.push(p.instructions.last().unwrap().clone()),
                13 => p.instructions.push(Instruction::DrawCards {
                    count: 1.into(),
                    recipient: None,
                    source_block: 0,
                }),
                14 => {
                    let Instruction::GrantKeywords { target_slot, .. } =
                        p.instructions.last_mut().unwrap()
                    else {
                        unreachable!()
                    };
                    *target_slot = 1;
                }
                15 => p.instructions.insert(
                    0,
                    Instruction::DrawCards {
                        count: 1.into(),
                        recipient: None,
                        source_block: 0,
                    },
                ),
                _ => p.instructions.clear(),
            }
            assert!(
                temporary_creature_keyword_program(p).is_none(),
                "{modifier}/{variant}"
            );
            d.capabilities = d.requirements();
            assert!(d.validate().is_err(), "{modifier}/{variant}");
        }
    }
    for (variant, domain) in (0..4).flat_map(|variant| {
        [
            TargetDomain::BattlefieldCreature,
            TargetDomain::ControlledBattlefieldCreature,
        ]
        .into_iter()
        .map(move |domain| (variant, domain))
    }) {
        let mut d = grant_domain(
            &[KeywordAbility::Deathtouch, KeywordAbility::Hexproof],
            true,
            domain,
        );
        let p = d.spell.as_mut().unwrap();
        match variant {
            0 => compound_indestructible_tests::split(p, true),
            1 => compound_indestructible_tests::split(p, false),
            2 => {
                let Instruction::GrantKeywords { source_block, .. } = &mut p.instructions[0] else {
                    unreachable!()
                };
                *source_block = 1;
            }
            _ => {
                let Instruction::GrantKeywords { target_slot, .. } = &mut p.instructions[0] else {
                    unreachable!()
                };
                *target_slot = 1;
            }
        }
        assert!(temporary_creature_keyword_program(p).is_none());
        d.capabilities = d.requirements();
        assert!(d.validate().is_err());
    }
}

#[test]
fn temporary_creature_keyword_family_rejects_definition_and_ability_contexts() {
    for (kind, domain) in [
        CardKind::Creature,
        CardKind::Artifact,
        CardKind::Enchantment,
        CardKind::Land,
    ]
    .into_iter()
    .flat_map(|kind| {
        [
            TargetDomain::BattlefieldCreature,
            TargetDomain::ControlledBattlefieldCreature,
        ]
        .into_iter()
        .map(move |domain| (kind, domain))
    }) {
        let mut d = grant_domain(&[KeywordAbility::Hexproof], false, domain);
        d.characteristics.kind = kind;
        assert!(temporary_creature_keyword_program(d.spell.as_ref().unwrap()).is_some());
        d.capabilities = d.requirements();
        assert!(d.validate().is_err());
    }
    let c = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-effects-ir.json"
    ))
    .unwrap();
    for name in ["Samite Healer", "Blood Artist"] {
        for (modifier, domain) in [false, true].into_iter().flat_map(|modifier| {
            [
                TargetDomain::BattlefieldCreature,
                TargetDomain::ControlledBattlefieldCreature,
            ]
            .into_iter()
            .map(move |domain| (modifier, domain))
        }) {
            let mut d = c
                .definitions
                .iter()
                .find(|d| d.name == name)
                .unwrap()
                .clone();
            let p = grant_domain(
                &[KeywordAbility::Deathtouch, KeywordAbility::Hexproof],
                modifier,
                domain,
            )
            .spell
            .unwrap();
            if let Some(a) = d.activated.first_mut() {
                a.program = p;
            } else {
                d.triggered[0].program = p;
            }
            d.capabilities = d.requirements();
            assert!(d.validate().is_err(), "{name}/{modifier}");
        }
    }
    let mut d = grant(&[KeywordAbility::Lifelink], false);
    d.provenance.source_sha256 = "0".repeat(64);
    assert!(d.validate().is_err());
}

#[test]
fn temporary_creature_keyword_family_rejects_unrepresentable_json_shapes() {
    for domain in [
        TargetDomain::BattlefieldCreature,
        TargetDomain::ControlledBattlefieldCreature,
    ] {
        let positive = serde_json::to_value(grant_domain(
            &[KeywordAbility::Flying, KeywordAbility::Hexproof],
            true,
            domain,
        ))
        .unwrap();
        for (pointer, invalid) in [
            (
                "/spell/instructions/0/keywords",
                serde_json::json!(["future-keyword"]),
            ),
            (
                "/spell/instructions/0/keywords",
                serde_json::json!([{"flying":null}]),
            ),
            (
                "/spell/instructions/0/duration",
                serde_json::json!("permanent"),
            ),
            (
                "/spell/instructions/0/modifier",
                serde_json::json!({"power":"1"}),
            ),
            ("/spell/targets/0/chooser", serde_json::json!("opponent")),
            ("/spell/targets/0/stage", serde_json::json!("resolution")),
            ("/spell/targets/0/count", serde_json::json!(2)),
            (
                "/spell/instructions/0/source_block",
                serde_json::json!(null),
            ),
            (
                "/spell/instructions/0/source_block",
                serde_json::json!(4294967296_u64),
            ),
            ("/spell/instructions/0/modifier/power", serde_json::json!(2)),
            (
                "/spell/instructions/0/modifier/power",
                serde_json::json!("+2"),
            ),
            (
                "/spell/instructions/0/modifier/toughness",
                serde_json::json!("-0"),
            ),
        ] {
            let mut bad = positive.clone();
            let (parent, key) = pointer.rsplit_once('/').unwrap();
            bad.pointer_mut(parent)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(key.into(), invalid);
            assert!(
                strict_read::<Definition>(&bad.to_string())
                    .and_then(|d| d.validate())
                    .is_err(),
                "{pointer}"
            );
        }
    }
}

#[test]
fn temporary_creature_keyword_family_schema_uses_unique_reviewed_vectors() {
    let schema = catalog_schema();
    assert_eq!(schema["$defs"]["TemporaryCreatureKeywords"]["minItems"], 1);
    assert_eq!(
        schema["$defs"]["TemporaryCreatureKeywords"]["uniqueItems"],
        true
    );
    assert_eq!(
        schema["$defs"]["TemporaryCreatureKeywords"]["items"]["enum"],
        serde_json::json!([
            "flying",
            "reach",
            "deathtouch",
            "indestructible",
            "lifelink",
            "hexproof",
            "haste",
            "trample",
            "first strike"
        ])
    );
    let instructions = &schema["$defs"]["TemporaryCreatureKeywordInstructions"];
    assert_eq!(instructions["minItems"], 1);
    assert_eq!(instructions["maxItems"], 1);
    assert_eq!(
        instructions["items"]["properties"]["modifier"],
        serde_json::json!({"anyOf":[{"type":"null"},{"$ref":"#/$defs/CreatureStatModifier"}]})
    );
    for field in ["power", "toughness"] {
        assert_eq!(
            schema["$defs"]["CreatureStatModifier"]["properties"][field],
            serde_json::json!({"$ref":"#/$defs/ExactQuantity"})
        );
    }
}

#[test]
fn temporary_creature_keyword_family_preserves_frozen_catalog_requirements_and_canonical_ir() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/catalogs");
    let mut paths = BTreeSet::new();
    for entry in std::fs::read_dir(&root).unwrap() {
        let manifest = entry.unwrap().path();
        if manifest
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            let manifest: serde_json::Value =
                serde_json::from_slice(&std::fs::read(manifest).unwrap()).unwrap();
            paths.insert(root.join(manifest["expected_ir"].as_str().unwrap()));
        }
    }
    assert!(!paths.is_empty());
    for path in paths {
        let bytes = std::fs::read_to_string(&path).unwrap();
        let frozen: serde_json::Value = serde_json::from_str(&bytes).unwrap();
        let catalog = Catalog::read(&bytes).unwrap();
        for d in &catalog.definitions {
            assert_eq!(d.requirements(), d.capabilities, "{}", d.name);
            assert!(
                !d.capabilities
                    .contains(&Capability::TemporaryCreatureKeywordProgramV1)
            );
        }
        assert_eq!(
            canonical(&catalog).unwrap(),
            canonical(&frozen).unwrap(),
            "{}",
            path.display()
        );
    }
}

#[test]
fn temporary_creature_keyword_family_keeps_controlled_pumps_and_policy_scoped() {
    for keyword in [KeywordAbility::Indestructible, KeywordAbility::Hexproof] {
        let mut d = grant_domain(
            &[keyword],
            true,
            TargetDomain::ControlledBattlefieldCreature,
        );
        let p = d.spell.as_mut().unwrap();
        assert!(temporary_creature_keyword_program(p).is_some());
        assert!(compound_indestructible_parameters(p).is_none());
        d.validate().unwrap();
        compound_indestructible_tests::split(d.spell.as_mut().unwrap(), false);
        d.spell.as_mut().unwrap().instructions.pop();
        d.capabilities = d.requirements();
        assert!(d.validate().is_err(), "bare controlled pump");
        d.spell
            .as_mut()
            .unwrap()
            .instructions
            .push(Instruction::DrawCards {
                count: 1.into(),
                recipient: None,
                source_block: 0,
            });
        d.capabilities = d.requirements();
        assert!(
            d.validate().is_err(),
            "controlled pump plus other operation"
        );
    }
    let mut intrinsic = grant(&[KeywordAbility::Hexproof], false);
    intrinsic.keywords.push(KeywordAbility::Hexproof);
    intrinsic.capabilities = intrinsic.requirements();
    assert!(
        intrinsic.validate().is_err(),
        "intrinsic Hexproof remains creature-only"
    );
}

#[test]
fn temporary_creature_keyword_atomic_modifier_json_and_pure_wire_parity() {
    for domain in [
        TargetDomain::BattlefieldCreature,
        TargetDomain::ControlledBattlefieldCreature,
    ] {
        let mut pure = grant_domain(&[KeywordAbility::Flying], false, domain);
        let Instruction::GrantKeywords { source_block, .. } =
            &mut pure.spell.as_mut().unwrap().instructions[0]
        else {
            unreachable!()
        };
        *source_block = 1;
        let bytes = canonical(&pure).unwrap();
        let mut raw = serde_json::to_value(&pure).unwrap();
        assert!(raw["spell"]["instructions"][0].get("modifier").is_none());
        raw["spell"]["instructions"][0]["modifier"] = serde_json::Value::Null;
        let explicit_none: Definition = strict_read(&raw.to_string()).unwrap();
        explicit_none.validate().unwrap();
        assert_eq!(explicit_none, pure);
        assert_eq!(canonical(&explicit_none).unwrap(), bytes);
        assert!(
            !pure
                .requirements()
                .contains(&Capability::TemporaryPowerToughnessV1)
        );
        let atomic = grant_domain(
            &[KeywordAbility::Flying, KeywordAbility::Lifelink],
            true,
            domain,
        );
        assert_eq!(atomic.spell.as_ref().unwrap().instructions.len(), 1);
        for modifier in [
            serde_json::json!({}),
            serde_json::json!({"power":"1"}),
            serde_json::json!({"toughness":"1"}),
            serde_json::json!({"power":"1","toughness":"1","extra":true}),
            serde_json::json!([]),
            serde_json::json!("1"),
            serde_json::json!(1),
            serde_json::json!(false),
        ] {
            let mut bad = serde_json::to_value(&atomic).unwrap();
            bad["spell"]["instructions"][0]["modifier"] = modifier;
            assert!(strict_read::<Definition>(&bad.to_string()).is_err());
        }
        // No old split ordering or source attribution can recreate an atomic effect.
        for reverse in [false, true] {
            for blocks in [[0, 0], [0, 1], [1, 0], [1, 1]] {
                let mut bad = atomic.clone();
                let p = bad.spell.as_mut().unwrap();
                compound_indestructible_tests::split(p, reverse);
                for (instruction, block) in p.instructions.iter_mut().zip(blocks) {
                    match instruction {
                        Instruction::ModifyPowerToughness { source_block, .. }
                        | Instruction::GrantKeywords { source_block, .. } => *source_block = block,
                        _ => unreachable!(),
                    }
                }
                assert!(temporary_creature_keyword_program(p).is_none());
                bad.capabilities = bad.requirements();
                assert!(bad.validate().is_err());
            }
        }
    }
}
