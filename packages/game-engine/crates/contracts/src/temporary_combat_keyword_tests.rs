//! Bounded synthetic IR controls for the shared family vocabulary extension.
//! Complete archived witnesses belong to compiler tests. Native tests are UNRUN
//! by this child; the parent owns combined qualification under the shared lock.
use super::*;

const DOMAINS: [TargetDomain; 2] = [
    TargetDomain::BattlefieldCreature,
    TargetDomain::ControlledBattlefieldCreature,
];
const REVIEWED: [KeywordAbility; 8] = [
    KeywordAbility::Flying,
    KeywordAbility::Deathtouch,
    KeywordAbility::Indestructible,
    KeywordAbility::Lifelink,
    KeywordAbility::Hexproof,
    KeywordAbility::Haste,
    KeywordAbility::Trample,
    KeywordAbility::FirstStrike,
];

fn grant(keywords: &[KeywordAbility], modifier: bool, domain: TargetDomain) -> Definition {
    let mut d = temporary_creature_keyword_tests::grant(keywords, modifier);
    d.spell.as_mut().unwrap().targets[0].domain = domain;
    d.capabilities = d.requirements();
    d
}

fn representatives() -> Vec<Vec<KeywordAbility>> {
    use KeywordAbility::*;
    // Preserve the old five-keyword exhaustive controls; sample new ordering
    // and cross-vocabulary composition without generating eight-keyword permutations.
    vec![
        vec![Haste],
        vec![Trample],
        vec![FirstStrike],
        vec![FirstStrike, Haste],
        vec![Haste, FirstStrike],
        vec![FirstStrike, Trample],
        vec![Trample, FirstStrike],
        vec![Haste, Trample],
        vec![Trample, Haste],
        vec![Haste, Flying],
        vec![Deathtouch, Trample],
        vec![Indestructible, FirstStrike],
        vec![Lifelink, Haste],
        vec![Hexproof, Trample],
        REVIEWED.to_vec(),
        REVIEWED.into_iter().rev().collect(),
    ]
}

fn primitive(keyword: KeywordAbility) -> Capability {
    match keyword {
        KeywordAbility::Flying => Capability::FlyingV1,
        KeywordAbility::Deathtouch => Capability::DeathtouchV1,
        KeywordAbility::Indestructible => Capability::IndestructibleV1,
        KeywordAbility::Lifelink => Capability::LifelinkV1,
        KeywordAbility::Hexproof => Capability::AttachmentsV1,
        KeywordAbility::Haste => Capability::HasteV1,
        KeywordAbility::Trample => Capability::TrampleV1,
        KeywordAbility::FirstStrike => Capability::FirstStrikeV1,
        _ => unreachable!(),
    }
}

#[test]
fn temporary_combat_keyword_representatives_have_exact_capabilities_and_roundtrip() {
    assert_eq!(CURRENT_IR_VERSION, 1);
    assert_eq!(representatives().len(), 16);
    for keywords in representatives() {
        for domain in DOMAINS {
            for modifier in [false, true] {
                let d = grant(&keywords, modifier, domain);
                let mut expected = BTreeSet::from([
                    Capability::NormalLayoutV1,
                    Capability::CastInstantV1,
                    Capability::TemporaryCreatureKeywordProgramV1,
                    if domain == TargetDomain::BattlefieldCreature {
                        Capability::CreatureTargetV1
                    } else {
                        Capability::AttachmentsV1
                    },
                ]);
                expected.extend(keywords.iter().copied().map(primitive));
                if modifier {
                    expected.insert(Capability::TemporaryPowerToughnessV1);
                }
                assert_eq!(
                    d.requirements(),
                    expected,
                    "{keywords:?}/{domain:?}/{modifier}"
                );
                d.validate().unwrap();
                let descriptor =
                    temporary_creature_keyword_program(d.spell.as_ref().unwrap()).unwrap();
                assert_eq!(descriptor.keywords, keywords);
                assert_eq!(descriptor.modifier.is_some(), modifier);
                assert!(compound_indestructible_parameters(d.spell.as_ref().unwrap()).is_none());
                let raw = canonical(&d).unwrap();
                let roundtrip: Definition = strict_read(&raw).unwrap();
                assert_eq!(roundtrip, d);
                roundtrip.validate().unwrap();
                for capability in &expected {
                    let mut bad = d.clone();
                    bad.capabilities.remove(capability);
                    assert!(bad.validate().is_err(), "missing {capability:?}");
                }
                for capability in [
                    Capability::TemporaryFlyingSpellV1,
                    Capability::TemporaryDeathtouchSpellV1,
                    Capability::TemporaryIndestructibleSpellV1,
                    Capability::CompoundIndestructibleSpellV1,
                    Capability::HasteV1,
                    Capability::TrampleV1,
                    Capability::FirstStrikeV1,
                    Capability::DoubleStrikeV1,
                    Capability::VigilanceV1,
                    Capability::ReachV1,
                    Capability::AttachmentsV1,
                    Capability::CreatureTargetV1,
                    Capability::TemporaryPowerToughnessV1,
                ] {
                    if !expected.contains(&capability) {
                        let mut bad = d.clone();
                        bad.capabilities.insert(capability);
                        assert!(bad.validate().is_err(), "spare {capability:?}");
                    }
                }
            }
        }
    }
}

#[test]
fn temporary_combat_keyword_vectors_reject_duplicates_and_unreviewed_members() {
    let mut invalid = vec![vec![]];
    for keyword in REVIEWED {
        invalid.push(vec![keyword, keyword]);
        invalid.push(vec![keyword, KeywordAbility::Defender]);
    }
    for keyword in [
        KeywordAbility::Defender,
        KeywordAbility::Defender,
        KeywordAbility::Defender,
        KeywordAbility::Menace,
        KeywordAbility::Flash,
        KeywordAbility::Shroud,
    ] {
        invalid.push(vec![keyword]);
        invalid.push(vec![
            KeywordAbility::Haste,
            keyword,
            KeywordAbility::Trample,
        ]);
    }
    for keywords in invalid {
        assert!(!supported_temporary_keywords(&keywords));
        for domain in DOMAINS {
            for modifier in [false, true] {
                let d = grant(&keywords, modifier, domain);
                assert!(temporary_creature_keyword_program(d.spell.as_ref().unwrap()).is_none());
                assert!(d.validate().is_err(), "{keywords:?}/{domain:?}/{modifier}");
            }
        }
    }
    for mut keywords in representatives() {
        keywords.push(keywords[0]);
        assert!(!supported_temporary_keywords(&keywords));
    }
}

#[test]
fn temporary_combat_keyword_parameters_and_pure_provenance_stay_exact() {
    for domain in DOMAINS {
        for keyword in [
            KeywordAbility::Haste,
            KeywordAbility::Trample,
            KeywordAbility::FirstStrike,
        ] {
            for block in [0, 7, u32::MAX] {
                let mut d = grant(&[keyword], false, domain);
                let Instruction::GrantKeywords { source_block, .. } =
                    &mut d.spell.as_mut().unwrap().instructions[0]
                else {
                    unreachable!()
                };
                *source_block = block;
                assert!(temporary_creature_keyword_program(d.spell.as_ref().unwrap()).is_some());
                d.validate().unwrap();
            }
        }
        for (power, toughness) in [
            ("1", "0"),
            ("2", "2"),
            ("2", "0"),
            ("3", "0"),
            ("0", "-2"),
            (
                "99999999999999999999999999999999999999999",
                "-99999999999999999999999999999999999999999",
            ),
        ] {
            let mut d = grant(
                &[KeywordAbility::FirstStrike, KeywordAbility::Trample],
                true,
                domain,
            );
            let Instruction::GrantKeywords {
                modifier:
                    Some(CreatureStatModifier {
                        power: p,
                        toughness: t,
                    }),
                ..
            } = &mut d.spell.as_mut().unwrap().instructions[0]
            else {
                unreachable!()
            };
            *p = power.parse().unwrap();
            *t = toughness.parse().unwrap();
            let descriptor = temporary_creature_keyword_program(d.spell.as_ref().unwrap()).unwrap();
            let (p, t) = descriptor.modifier.unwrap();
            assert_eq!(p.to_string(), power);
            assert_eq!(t.to_string(), toughness);
            d.validate().unwrap();
            d.characteristics.kind = CardKind::Sorcery;
            d.spell.as_mut().unwrap().timing = SpellTiming::MainPhase;
            d.capabilities = d.requirements();
            d.validate().unwrap();
        }
    }
}

#[test]
fn temporary_combat_keyword_json_near_misses_stay_rejected() {
    for domain in DOMAINS {
        for modifier in [false, true] {
            let positive = serde_json::to_value(grant(
                &[KeywordAbility::FirstStrike, KeywordAbility::Haste],
                modifier,
                domain,
            ))
            .unwrap();
            let grant_index = 0;
            let grant_path = format!("/spell/instructions/{grant_index}");
            let mut cases = vec![
                (format!("{grant_path}/keywords"), serde_json::json!([])),
                (
                    format!("{grant_path}/keywords"),
                    serde_json::json!(["first strike", "haste", "first strike"]),
                ),
                (
                    format!("{grant_path}/keywords"),
                    serde_json::json!(["haste", "defender"]),
                ),
                (
                    format!("{grant_path}/keywords"),
                    serde_json::json!(["haste", "future-keyword"]),
                ),
                (
                    format!("{grant_path}/keywords"),
                    serde_json::json!([{"haste":null}]),
                ),
                (format!("{grant_path}/keywords"), serde_json::json!("haste")),
                (format!("{grant_path}/keywords"), serde_json::json!(null)),
                (
                    format!("{grant_path}/duration"),
                    serde_json::json!("permanent"),
                ),
                (format!("{grant_path}/target_slot"), serde_json::json!(1)),
                (
                    format!("{grant_path}/source_block"),
                    serde_json::json!(null),
                ),
                ("/spell/targets/0/slot".into(), serde_json::json!(1)),
                (
                    "/spell/targets/0/selection_id".into(),
                    serde_json::json!(""),
                ),
                (
                    "/spell/targets/0/domain".into(),
                    serde_json::json!("blocking_battlefield_creature"),
                ),
                (
                    "/spell/targets/0/domain".into(),
                    serde_json::json!("battlefield_permanent"),
                ),
                (
                    "/spell/targets/0/chooser".into(),
                    serde_json::json!("opponent"),
                ),
                (
                    "/spell/targets/0/stage".into(),
                    serde_json::json!("resolution"),
                ),
                ("/spell/targets/0/count".into(), serde_json::json!(2)),
                (
                    "/spell/targets/0/filter".into(),
                    serde_json::json!({"types":[],"non_types":[],"non_colors":[],"opponent":false,"tapped":false}),
                ),
                (
                    "/spell/functioning_zone".into(),
                    serde_json::json!("battlefield"),
                ),
                ("/spell/resolution".into(), serde_json::json!("permanent")),
                (
                    "/spell/additional_sacrifice".into(),
                    serde_json::json!({"types":["artifact"],"subtypes":[]}),
                ),
            ];
            if modifier {
                cases.extend([
                    (format!("{grant_path}/source_block"), serde_json::json!(1)),
                    (
                        "/spell/instructions/0/source_block".into(),
                        serde_json::json!(1),
                    ),
                    (
                        "/spell/instructions/0/target_slot".into(),
                        serde_json::json!(1),
                    ),
                    (
                        "/spell/instructions/0/duration".into(),
                        serde_json::json!("permanent"),
                    ),
                    (
                        "/spell/instructions/0/modifier/power".into(),
                        serde_json::json!(2),
                    ),
                    (
                        "/spell/instructions/0/modifier/power".into(),
                        serde_json::json!({"amount":"source-power"}),
                    ),
                    (
                        "/spell/instructions/0/modifier/toughness".into(),
                        serde_json::json!("-0"),
                    ),
                ]);
            }
            for (pointer, invalid) in cases {
                let mut bad = positive.clone();
                let (parent, key) = pointer.rsplit_once('/').unwrap();
                bad.pointer_mut(parent)
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .insert(key.into(), invalid);
                assert!(
                    strict_read::<Definition>(&bad.to_string())
                        .and_then(|mut d| {
                            // Isolate semantic rejection from changed requirements.
                            d.capabilities = d.requirements();
                            d.validate()
                        })
                        .is_err(),
                    "{pointer}/{domain:?}/{modifier}"
                );
            }
            for variant in 0..5 {
                let mut d = grant(
                    &[KeywordAbility::Trample, KeywordAbility::Haste],
                    modifier,
                    domain,
                );
                let p = d.spell.as_mut().unwrap();
                match variant {
                    0 => p.targets.clear(),
                    1 => p.targets.push(p.targets[0].clone()),
                    2 => p.instructions.push(Instruction::DrawCards {
                        count: 1.into(),
                        recipient: None,
                        source_block: 0,
                    }),
                    3 => {
                        let child = p.clone();
                        p.modes.push(SpellMode { program: child });
                    }
                    _ => {
                        if modifier {
                            compound_indestructible_tests::split(p, true);
                        } else {
                            p.instructions.push(p.instructions[0].clone());
                        }
                    }
                }
                assert!(temporary_creature_keyword_program(p).is_none());
                d.capabilities = d.requirements();
                assert!(d.validate().is_err(), "{variant}/{domain:?}/{modifier}");
            }
        }
    }
}

#[test]
fn temporary_combat_keyword_contexts_reject_with_recomputed_requirements() {
    let catalog = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-effects-ir.json"
    ))
    .unwrap();
    for domain in DOMAINS {
        for modifier in [false, true] {
            let ordinary = grant(
                &[KeywordAbility::FirstStrike, KeywordAbility::Trample],
                modifier,
                domain,
            );
            ordinary.validate().unwrap();
            for kind in [
                CardKind::Creature,
                CardKind::Artifact,
                CardKind::Enchantment,
                CardKind::Land,
            ] {
                let mut d = ordinary.clone();
                d.characteristics.kind = kind;
                d.capabilities = d.requirements();
                assert_eq!(
                    d.validate().unwrap_err(),
                    "temporary indestructible requires an ordinary single-target spell",
                    "{kind:?}/{domain:?}/{modifier}"
                );
            }
            for name in ["Samite Healer", "Blood Artist"] {
                let mut d = catalog
                    .definitions
                    .iter()
                    .find(|d| d.name == name)
                    .unwrap()
                    .clone();
                d.validate().unwrap();
                let program = ordinary.spell.clone().unwrap();
                if let Some(ability) = d.activated.first_mut() {
                    ability.program = program;
                } else {
                    d.triggered[0].program = program;
                }
                d.capabilities = d.requirements();
                assert_eq!(
                    d.validate().unwrap_err(),
                    "temporary indestructible requires an ordinary single-target spell",
                    "{name}/{domain:?}/{modifier}"
                );
            }
        }
    }
}

#[test]
fn temporary_combat_keyword_schema_preserves_legacy_subset_vocabulary() {
    let schema = catalog_schema();
    assert_eq!(
        schema["$defs"]["TemporaryCreatureKeywords"]["items"]["enum"],
        serde_json::json!([
            "flying",
            "double strike",
            "vigilance",
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
    assert_eq!(
        schema["$defs"]["PureKeywordGrantInstructions"]["items"]["properties"]["keywords"]["enum"],
        serde_json::json!([["indestructible"], ["flying"], ["deathtouch"]])
    );
    assert_eq!(
        schema["$defs"]["CompoundIndestructibleInstructions"]["items"]["properties"]["keywords"]["const"],
        serde_json::json!(["indestructible"])
    );
}
