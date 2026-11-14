//! Frozen d705 family witnesses and explicitly synthetic AST boundary controls.
//! All new-source native tests are UNRUN until the parent combined pass.
use super::*;
use serde_json::{Value, json};
const HEADER: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
struct Witness {
    raw: &'static str,
    name: &'static str,
    record: &'static str,
    source: &'static str,
    ast: &'static str,
    binding: &'static str,
    index: u32,
    generic: u32,
    color: Color,
    modifier: Option<(i64, i64)>,
    keywords: &'static [KeywordAbility],
}
const WITNESSES: &[Witness] = &[
    Witness {
        raw: include_str!("../../../tests/fixtures/058a8475-649f-433a-b258-42bc03bf40ba.json"),
        name: "Grotesque Mutation",
        record: "206c2866ac9d7dfb3092fae5831b0f1e6c38a69d82892957aae6eb0ec64a2b45",
        source: "5af9e437f75ef59356a9e344d694e2e422721e3a27341867973c0162b9458e7d",
        ast: "65faf95763ea7aa8fbf16d3b49426f55bc70ccb55c1adc0207136dd9039d4f91",
        binding: "58eaee35a72045c8e493cef9279659109a22426156d3c57abe496da9d77b9140",
        index: 876,
        generic: 1,
        color: Color::B,
        modifier: Some((3, 1)),
        keywords: &[KeywordAbility::Lifelink],
    },
    Witness {
        raw: include_str!("../../../tests/fixtures/533d6b9f-a145-44c4-9d16-74fcd9c0ae2f.json"),
        name: "Battle-Rage Blessing",
        record: "44ba4053f0311b33ff93fba2445202e1d2385b33382fde8ef0ecbbf6ae56e53b",
        source: "ad0fec9b82e8bff9fe34e84277df6839a1fb41f03fb0ec36c16fe448a1d0b936",
        ast: "53e18c3df0275bb1c559a1405f33c66b3fd80c67a4d3b9812b9274d677db0058",
        binding: "f66a010e5a639903c6fc3451ac0b91cd42a18db1d37941765e64f380c0dcaed9",
        index: 12524,
        generic: 1,
        color: Color::B,
        modifier: None,
        keywords: &[KeywordAbility::Deathtouch, KeywordAbility::Indestructible],
    },
    Witness {
        raw: include_str!("../../../tests/fixtures/593caa15-e1d1-477f-bbb4-124d7a7b81f3.json"),
        name: "Mighty Leap",
        record: "d7846c9ef47c495b75fb5f505f8c7c84ef463043979f356000047a5c787463eb",
        source: "27a1a7ec9f64d483fa69bb916f7324f673405ff1d0764db3390d449d65ba4233",
        ast: "4fdf740f5c6a8d3e72f73506d74a7449790acc6b7dfe7c80551eb640a143c619",
        binding: "1ffc4471d8101b97ff9581dc68b92e06cb66575e82a138017c3564f34ca51eb0",
        index: 13412,
        generic: 1,
        color: Color::W,
        modifier: Some((2, 2)),
        keywords: &[KeywordAbility::Flying],
    },
    Witness {
        raw: include_str!("../../../tests/fixtures/6e460d39-0f18-4a2d-9c8d-3eaba301d285.json"),
        name: "Rush of Vitality",
        record: "07cd6659120c446f78414fac0051e074f1f6072583c0c9908d38fd738805c0c9",
        source: "16dd303a5ca8ef4d3f936ebc21cd3ebef184f1384b425cf905ccc990ef1426ef",
        ast: "6f66c24756481ddfd26bd4e30a46b80650d24112664c81740fefd1793f6f2dc3",
        binding: "d01a2a105f2eb728ce0c51c41939a58c5bf16e5d6f8c4e4e347fa7a8ae5c5573",
        index: 16585,
        generic: 1,
        color: Color::B,
        modifier: Some((1, 0)),
        keywords: &[KeywordAbility::Lifelink, KeywordAbility::Indestructible],
    },
    Witness {
        raw: include_str!("../../../tests/fixtures/cbc808dd-8d50-4f42-b0f1-d4e45d0462a2.json"),
        name: "Coat with Venom",
        record: "57b00e9c27a751b1292a5bde754321268586eb732bde256ba5ff29c4215a005b",
        source: "0e10a3e7bf85f939cfb9ca84c7f946191f6c3e110d5205a573e2219bbce5e41e",
        ast: "7b0f952bd09a9b463514920ab5783ed5d0f8545a610cc53b482a5116e4eb201b",
        binding: "b2491a6a32b10a089c0881f81d5d80fc8451c9aba142058556b67b0c938372b8",
        index: 30731,
        generic: 0,
        color: Color::B,
        modifier: Some((1, 2)),
        keywords: &[KeywordAbility::Deathtouch],
    },
];
fn lowered(raw: &str) -> Box<Definition> {
    let result = compile(HEADER, raw);
    let CompileResult::Lowered { definition } = result else {
        panic!("expected complete family lowering: {result:?}");
    };
    definition.validate().unwrap();
    definition
}
fn changed_ability(raw: &str, change: impl FnOnce(&mut Value)) -> String {
    tests::changed_source(raw, |r| {
        let a = &mut r["faces"][0]["result"]["abilities"][0];
        change(a);
        r["faces"][0]["result"]["lines"][0]["ability"] = a.clone();
    })
}
fn assert_program(d: &Definition, modifier: Option<(i64, i64)>, keywords: &[KeywordAbility]) {
    assert_eq!(CURRENT_IR_VERSION, 1);
    assert_eq!(d.capabilities, d.requirements());
    let p = d.spell.as_ref().unwrap();
    assert!(p.modes.is_empty());
    assert!(p.additional_sacrifice.is_none());
    assert_eq!(
        p.targets,
        [TargetSpec {
            slot: 0,
            selection_id: "line:0:0".into(),
            chooser: Chooser::Controller,
            stage: EvaluationStage::Announcement,
            domain: TargetDomain::BattlefieldCreature,
            filter: None,
        }]
    );
    let view = temporary_creature_keyword_program(p).expect("shared complete-program recognizer");
    assert_eq!(
        view.modifier,
        modifier
            .map(|(p, t)| (Quantity::from(p), Quantity::from(t)))
            .as_ref()
            .map(|(p, t)| (p, t))
    );
    assert_eq!(view.keywords, keywords);
    let mut expected = vec![];
    if let Some((power, toughness)) = modifier {
        expected.push(Instruction::ModifyPowerToughness {
            target_slot: 0,
            power: power.into(),
            toughness: toughness.into(),
            duration: Duration::UntilEndOfTurn,
            source_block: 0,
        });
    }
    expected.push(Instruction::GrantKeywords {
        target_slot: 0,
        keywords: keywords.to_vec(),
        duration: Duration::UntilEndOfTurn,
        source_block: 0,
    });
    assert_eq!(p.instructions, expected);
}
#[test]
fn temporary_creature_keyword_complete_five_frozen_sources() {
    for w in WITNESSES {
        let record: Value = serde_json::from_str(w.raw).unwrap();
        assert_eq!(sha256(w.raw.trim_end().as_bytes()), w.record);
        let d = lowered(w.raw);
        assert_eq!(d.name, w.name);
        assert_eq!(d.oracle_id, record["oracleIdentity"].as_str().unwrap());
        assert_eq!(d.characteristics.kind, CardKind::Instant);
        assert_eq!(
            d.characteristics.mana_cost,
            ManaCost {
                generic: i64::from(w.generic).into(),
                colored: vec![w.color]
            }
        );
        assert_eq!(d.characteristics.color_identity, [w.color]);
        assert_eq!(d.provenance.source_index, w.index);
        assert_eq!(d.provenance.source_sha256, w.source);
        assert_eq!(d.provenance.ast_sha256, w.ast);
        assert_eq!(d.provenance.binding_sha256, w.binding);
        assert_program(&d, w.modifier, w.keywords);
        assert!(
            d.capabilities
                .contains(&Capability::TemporaryCreatureKeywordProgramV1)
        );
        for old in [
            Capability::TemporaryFlyingSpellV1,
            Capability::TemporaryDeathtouchSpellV1,
            Capability::TemporaryIndestructibleSpellV1,
            Capability::CompoundIndestructibleSpellV1,
        ] {
            assert!(
                !d.capabilities.contains(&old),
                "{} retains only the general family requirement",
                w.name
            );
        }
        for (keyword, capability) in [
            (KeywordAbility::Flying, Capability::FlyingV1),
            (KeywordAbility::Deathtouch, Capability::DeathtouchV1),
            (KeywordAbility::Indestructible, Capability::IndestructibleV1),
            (KeywordAbility::Lifelink, Capability::LifelinkV1),
        ] {
            assert_eq!(
                d.capabilities.contains(&capability),
                w.keywords.contains(&keyword)
            );
        }
    }
}
#[test]
fn temporary_creature_keyword_parser_preserves_order_and_rejects_every_bad_item() {
    let valid = json!([
        {"gained":"keyword","keyword":{"keyword":"lifelink"}},
        {"gained":"keyword","keyword":{"keyword":"flying"}},
        {"gained":"keyword","keyword":{"keyword":"indestructible"}},
        {"gained":"keyword","keyword":{"keyword":"deathtouch"}}
    ]);
    assert_eq!(
        lower_temporary_creature_keywords(valid.as_array().unwrap()).unwrap(),
        [
            KeywordAbility::Lifelink,
            KeywordAbility::Flying,
            KeywordAbility::Indestructible,
            KeywordAbility::Deathtouch
        ]
    );
    let malformed = [
        Value::Null,
        json!(true),
        json!(1),
        json!("lifelink"),
        json!([]),
        json!({}),
        json!({"gained":"keyword"}),
        json!({"gained":"keyword","keyword":null}),
        json!({"gained":"keyword","keyword":{"keyword":null}}),
        json!({"gained":"keyword","keyword":{"keyword":["flying"]}}),
        json!({"gained":"keyword","keyword":{"keyword":"unknown"}}),
        json!({"gained":"activated","keyword":{"keyword":"lifelink"}}),
        json!({"gained":"keyword","keyword":{"keyword":"lifelink"},"extra":null}),
        json!({"gained":"keyword","keyword":{"keyword":"lifelink","extra":null}}),
        json!({"gained":"keyword","keyword":{"keyword":"lifelink","cost":[]}}),
        json!({"gained":"keyword","keyword":{"keyword":"lifelink","filter":{}}}),
        json!({"gained":"keyword","keyword":{"keyword":"reach"}}),
        json!({"gained":"keyword","keyword":{"keyword":"vigilance"}}),
        json!({"gained":"keyword","keyword":{"keyword":"shroud"}}),
    ];
    assert!(lower_temporary_creature_keywords(&[]).is_err());
    for bad in malformed {
        assert!(
            lower_temporary_creature_keywords(&[valid[0].clone(), bad.clone()]).is_err(),
            "{bad}"
        );
        assert!(
            lower_temporary_creature_keywords(&[bad.clone(), valid[0].clone()]).is_err(),
            "{bad}"
        );
    }
    assert!(lower_temporary_creature_keywords(&[valid[0].clone(), valid[0].clone()]).is_err());
    // Duplicate JSON attributes must fail before serde's internally tagged mirror.
    assert!(read_abilities(r#"[{"kind":"spell","effects":[{"steps":[{"effect":"gain-abilities","what":{},"abilities":[{"gained":"keyword","keyword":{"keyword":"flying","keyword":"lifelink"}}],"duration":{"duration":"end-of-turn"}}]}]}]"#).is_err());
}
#[test]
fn temporary_creature_keyword_synthetic_parameters_identity_order_and_sorcery() {
    for w in WITNESSES {
        let original = lowered(w.raw);
        let renamed = tests::changed_source(w.raw, |r| {
            r["source"]["name"] = "Independent family witness".into();
            r["faces"][0]["result"]["name"] = "Independent family witness".into();
            let old = r["oracleIdentity"].as_str().unwrap().to_owned();
            let new = "00000000-0000-4000-8000-000000000001";
            r["oracleIdentity"] = new.into();
            r["source"]["oracle_id"] = new.into();
            r["source"]["id"] = "00000000-0000-4000-8000-000000000002".into();
            // Rebind every diagnostic identity/reference consistently; target ASTs have no card identity.
            let binding = serde_json::to_string(&r["binding"])
                .unwrap()
                .replace(&old, new);
            r["binding"] = serde_json::from_str(&binding).unwrap();
        });
        let renamed = lowered(&renamed);
        assert_eq!(renamed.spell, original.spell);
        assert_eq!(renamed.capabilities, original.capabilities);
        let reversed = changed_ability(w.raw, |a| {
            a["effects"][0]["steps"]
                .as_array_mut()
                .unwrap()
                .last_mut()
                .unwrap()["abilities"]
                .as_array_mut()
                .unwrap()
                .reverse()
        });
        let mut keywords = w.keywords.to_vec();
        keywords.reverse();
        let reversed = lowered(&reversed);
        assert_program(&reversed, w.modifier, &keywords);
        assert_eq!(reversed.capabilities, original.capabilities);
    }
    // Signed quantities and printed values are parameters; no printed-number branch.
    for (power, toughness) in [(-3_i64, 5_i64), (0, -2), (7, 0)] {
        let raw = changed_ability(WITNESSES[0].raw, |a| {
            for (field, value) in [("power", power), ("toughness", toughness)] {
                a["effects"][0]["steps"][0][field] = json!({"sign":if value < 0 {-1} else {1},"amount":{"amount":"fixed","value":value.abs()}});
            }
        });
        assert_program(
            &lowered(&raw),
            Some((power, toughness)),
            &[KeywordAbility::Lifelink],
        );
    }
    let sorcery = tests::changed_source(WITNESSES[1].raw, |r| {
        r["source"]["type_line"] = "Sorcery".into()
    });
    let d = lowered(&sorcery);
    assert_eq!(d.characteristics.kind, CardKind::Sorcery);
    assert_eq!(d.spell.as_ref().unwrap().timing, SpellTiming::MainPhase);
    assert_program(&d, None, WITNESSES[1].keywords);
}

#[test]
fn temporary_creature_keyword_complete_sources_reject_unconsumed_shapes() {
    for w in WITNESSES {
        for variant in 0..32 {
            let raw = changed_ability(w.raw, |a| {
                let steps = &mut a["effects"][0]["steps"];
                let last = steps.as_array().unwrap().len() - 1;
                match variant {
                    0 => steps[last]["abilities"] = json!([]),
                    1 => steps[last]["abilities"] = Value::Null,
                    2 => steps[last]["abilities"][0] = Value::Null,
                    3 => steps[last]["abilities"][0]["keyword"] = Value::Null,
                    4 => steps[last]["abilities"][0]["keyword"]["keyword"] = "reach".into(),
                    5 => steps[last]["abilities"][0]["extra"] = Value::Null,
                    6 => steps[last]["abilities"][0]["keyword"]["extra"] = Value::Null,
                    7 => steps[last]["abilities"][0]["gained"] = "activated".into(),
                    8 => { let x = steps[last]["abilities"][0].clone(); steps[last]["abilities"].as_array_mut().unwrap().push(x); },
                    9 => steps[last]["duration"] = Value::Null,
                    10 => steps[last]["duration"]["duration"] = "forever".into(),
                    11 => steps[last]["duration"]["condition"] = json!({"condition":"you-control-creature"}),
                    12 => steps[last]["what"] = Value::Null,
                    13 => steps[last]["what"]["selectionId"] = "".into(),
                    14 => steps[last]["what"]["count"]["value"] = 2.into(),
                    15 => steps[last]["what"]["filter"]["control"] = "opponent".into(),
                    16 => steps[last]["what"]["filter"]["tapped"] = true.into(),
                    17 => steps[last]["what"]["filter"]["types"] = json!(["artifact"]),
                    18 => steps[last]["what"]["filter"]["subtypes"] = json!(["Goblin"]),
                    19 => steps[last]["condition"] = json!({"condition":"you-control-creature"}),
                    20 => steps[last]["extra"] = true.into(),
                    21 => { let x = steps[last].clone(); steps.as_array_mut().unwrap().push(x); },
                    22 => steps.as_array_mut().unwrap().push(json!({"effect":"draw","who":{"player":"you"},"count":{"amount":"fixed","value":1}})),
                    23 => a["effects"][0]["bindings"] = json!([{"variable":"unused","value":{"amount":"fixed","value":1}}]),
                    24 => { let child = a["effects"].clone(); a["effects"] = json!([{"steps":[{"effect":"modal","count":{"exactly":1},"options":[{"effects":child.clone()},{"effects":child}]}]}]); },
                    25 => { a["kind"] = "activated".into(); a["costs"] = json!([{"cost":"tap"}]); },
                    26 => { a["kind"] = "triggered".into(); a["trigger"] = json!({"trigger":"enters","what":{"ref":"self"}}); },
                    27 => a["condition"] = json!({"condition":"you-control-creature"}),
                    28 => steps[last]["abilities"][0]["keyword"]["keyword"] = Value::Null,
                    29 => steps[last]["abilities"][0]["keyword"]["keyword"] = 2.into(),
                    30 => steps[last]["abilities"][0]["keyword"]["keyword"] = "vigilance".into(),
                    _ => steps[last]["abilities"][0]["keyword"]["filter"] = Value::Null,
                }
            });
            assert!(
                !matches!(compile(HEADER, &raw), CompileResult::Lowered { .. }),
                "{} variant {variant}",
                w.name
            );
        }
        // A valid extra cost must still fail complete-program validation.
        let extra_cost = tests::changed_source(w.raw, |r| {
            let cost = json!({"kind":"additional-cost","costs":[{"cost":"sacrifice","what":{"ref":"filter","filter":{"types":["artifact"]},"count":{"amount":"fixed","value":1}}}]});
            let text = r["source"]["oracle_text"].as_str().unwrap().to_owned();
            r["source"]["oracle_text"] =
                format!("As an additional cost to cast this spell, sacrifice an artifact.\n{text}")
                    .into();
            r["faces"][0]["result"]["abilities"]
                .as_array_mut()
                .unwrap()
                .insert(0, cost.clone());
            r["faces"][0]["result"]["lines"].as_array_mut().unwrap().insert(0, json!({"text":"As an additional cost to cast this spell, sacrifice an artifact.","ok":true,"ability":cost}));
        });
        assert!(!matches!(
            compile(HEADER, &extra_cost),
            CompileResult::Lowered { .. }
        ));
    }
}

#[test]
fn temporary_creature_keyword_compound_rejects_order_occurrence_and_block_changes() {
    for w in WITNESSES.iter().filter(|w| w.modifier.is_some()) {
        for variant in 0..8 {
            let raw = changed_ability(w.raw, |a| {
                let s = &mut a["effects"][0]["steps"];
                match variant {
                    0 => s.as_array_mut().unwrap().reverse(),
                    1 => s[1]["what"]["selectionId"] = "line:0:1".into(),
                    2 => s[0]["what"]["selectionId"] = "".into(),
                    3 => s[0]["duration"]["duration"] = "forever".into(),
                    4 => s[0]["power"]["amount"] = json!({"amount":"variable","name":"x"}),
                    5 => {
                        s[0]["toughness"]["amount"] =
                            json!({"amount":"count","of":{"types":["creature"]}})
                    }
                    6 => {
                        let pump = s[0].clone();
                        s.as_array_mut().unwrap().push(pump);
                    }
                    _ => {
                        let grant = s[1].clone();
                        a["effects"] = json!([{"steps":[grant]}]);
                    }
                }
            });
            if variant == 7 {
                // Removing PT admits the pure grant through the same family or its
                // exact historical singleton capability, never the old compound cap.
                let d = lowered(&raw);
                assert_program(&d, None, w.keywords);
                assert!(
                    !d.capabilities
                        .contains(&Capability::CompoundIndestructibleSpellV1)
                );
            } else {
                assert!(
                    !matches!(compile(HEADER, &raw), CompileResult::Lowered { .. }),
                    "{} variant {variant}",
                    w.name
                );
            }
        }
        // Two operations in different top-level source blocks are not one family.
        let split = tests::changed_source(w.raw, |r| {
            let a = &r["faces"][0]["result"]["abilities"][0];
            let steps = &a["effects"][0]["steps"];
            let abilities = json!([
                {"kind":"spell","effects":[{"steps":[steps[0].clone()]}]},
                {"kind":"spell","effects":[{"steps":[steps[1].clone()]}]}
            ]);
            r["faces"][0]["result"]["abilities"] = abilities.clone();
            r["faces"][0]["result"]["lines"] = json!([
                {"text":"Synthetic first block.","ok":true,"ability":abilities[0].clone()},
                {"text":"Synthetic second block.","ok":true,"ability":abilities[1].clone()}
            ]);
            r["source"]["oracle_text"] = "Synthetic first block.\nSynthetic second block.".into();
        });
        assert!(
            !matches!(compile(HEADER, &split), CompileResult::Lowered { .. }),
            "{} split source blocks",
            w.name
        );
    }
}

#[test]
fn temporary_creature_keyword_hash_and_mirror_provenance_stays_strict() {
    for w in WITNESSES {
        for variant in 0..5 {
            let normalized = tests::changed_source(w.raw, |_| {});
            let mut r: Value = serde_json::from_str(&normalized).unwrap();
            match variant {
                0 => r["sourceSha256"] = "0".repeat(64).into(),
                1 => r["astSha256"] = "0".repeat(64).into(),
                2 => r["bindingSha256"] = "0".repeat(64).into(),
                3 => r["source"]["oracle_id"] = "other-source".into(),
                _ => {
                    r["faces"][0]["result"]["lines"][0]["ability"] =
                        json!({"kind":"spell","effects":[]})
                }
            }
            assert!(!matches!(
                compile(HEADER, &serde_json::to_string(&r).unwrap()),
                CompileResult::Lowered { .. }
            ));
        }
        // Rehash the inconsistent mirror so rejection reaches the mirror check.
        let mismatch = tests::changed_source(w.raw, |r| {
            r["faces"][0]["result"]["lines"][0]["ability"] = json!({"kind":"spell","effects":[]})
        });
        assert!(!matches!(
            compile(HEADER, &mismatch),
            CompileResult::Lowered { .. }
        ));
    }
}

#[test]
fn temporary_creature_keyword_all_reviewed_subsets_use_shared_program_and_exact_subset_caps() {
    let vocabulary = [
        KeywordAbility::Lifelink,
        KeywordAbility::Flying,
        KeywordAbility::Indestructible,
        KeywordAbility::Deathtouch,
    ];
    for mask in 1..16 {
        let keywords: Vec<_> = vocabulary
            .iter()
            .enumerate()
            .filter(|(i, _)| mask & (1 << i) != 0)
            .map(|(_, k)| *k)
            .collect();
        for modifier in [None, Some((3, 1))] {
            let w = if modifier.is_some() {
                &WITNESSES[0]
            } else {
                &WITNESSES[1]
            };
            let raw = changed_ability(w.raw, |a| {
                let steps = a["effects"][0]["steps"].as_array_mut().unwrap();
                steps.last_mut().unwrap()["abilities"] = Value::Array(
                    keywords
                        .iter()
                        .map(|k| json!({"gained":"keyword","keyword":{"keyword":k}}))
                        .collect(),
                );
            });
            let d = lowered(&raw);
            assert_program(&d, modifier, &keywords);
            let expected_cap = match (modifier, keywords.as_slice()) {
                (None, [KeywordAbility::Flying]) => Capability::TemporaryFlyingSpellV1,
                (None, [KeywordAbility::Deathtouch]) => Capability::TemporaryDeathtouchSpellV1,
                (None, [KeywordAbility::Indestructible]) => {
                    Capability::TemporaryIndestructibleSpellV1
                }
                (Some(_), [KeywordAbility::Indestructible]) => {
                    Capability::CompoundIndestructibleSpellV1
                }
                _ => Capability::TemporaryCreatureKeywordProgramV1,
            };
            for cap in [
                Capability::TemporaryFlyingSpellV1,
                Capability::TemporaryDeathtouchSpellV1,
                Capability::TemporaryIndestructibleSpellV1,
                Capability::CompoundIndestructibleSpellV1,
                Capability::TemporaryCreatureKeywordProgramV1,
            ] {
                assert_eq!(d.capabilities.contains(&cap), cap == expected_cap);
            }
        }
    }
    // These were genuine unsupported boundaries in the historical tests:
    // compound Flying and pure Flying/Deathtouch + Indestructible now qualify
    // through one general family capability. Duplicate/unreviewed controls stay red.
    for (raw, modifier, keywords) in [
        (
            include_str!("../../../tests/fixtures/ef29f979-1a6d-47bb-83f5-4679389471da.json"),
            Some((1, 1)),
            vec![KeywordAbility::Flying],
        ),
        (
            include_str!("../../../tests/fixtures/f7518456-45ed-41d4-bd3c-5aacea28eb35.json"),
            None,
            vec![KeywordAbility::Flying, KeywordAbility::Indestructible],
        ),
        (
            include_str!("../../../tests/fixtures/b75fb789-6a78-4d23-b872-7627b478fb6f.json"),
            None,
            vec![KeywordAbility::Deathtouch, KeywordAbility::Indestructible],
        ),
    ] {
        let raw = changed_ability(raw, |a| {
            a["effects"][0]["steps"]
                .as_array_mut()
                .unwrap()
                .last_mut()
                .unwrap()["abilities"] = json!(
                keywords
                    .iter()
                    .map(|k| json!({"gained":"keyword","keyword":{"keyword":k}}))
                    .collect::<Vec<_>>()
            )
        });
        let d = lowered(&raw);
        assert_program(&d, modifier, &keywords);
        assert!(
            d.capabilities
                .contains(&Capability::TemporaryCreatureKeywordProgramV1)
        );
    }
}

#[test]
fn temporary_creature_keyword_legacy_ir_caps_and_pure_source_block_provenance() {
    let existing = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-blocking-matches-ir.json"
    ))
    .unwrap();
    let controls = [
        (
            include_str!("../../../tests/fixtures/c462adf3-4ade-4b90-bdec-7247b080471f.json"),
            None,
            KeywordAbility::Indestructible,
            Capability::TemporaryIndestructibleSpellV1,
        ),
        (
            include_str!("../../../tests/fixtures/f7518456-45ed-41d4-bd3c-5aacea28eb35.json"),
            None,
            KeywordAbility::Flying,
            Capability::TemporaryFlyingSpellV1,
        ),
        (
            include_str!("../../../tests/fixtures/b75fb789-6a78-4d23-b872-7627b478fb6f.json"),
            None,
            KeywordAbility::Deathtouch,
            Capability::TemporaryDeathtouchSpellV1,
        ),
        (
            include_str!("../../../tests/fixtures/ef29f979-1a6d-47bb-83f5-4679389471da.json"),
            Some((1, 1)),
            KeywordAbility::Indestructible,
            Capability::CompoundIndestructibleSpellV1,
        ),
        (
            include_str!("../../../tests/fixtures/467e22c3-6107-40ff-afc7-5960710c970b.json"),
            Some((2, 2)),
            KeywordAbility::Indestructible,
            Capability::CompoundIndestructibleSpellV1,
        ),
        (
            include_str!("../../../tests/fixtures/795e0ffb-fcd9-4cb9-ad18-8a0217229bfe.json"),
            Some((2, 0)),
            KeywordAbility::Indestructible,
            Capability::CompoundIndestructibleSpellV1,
        ),
    ];
    for (raw, modifier, keyword, cap) in controls {
        let d = lowered(raw);
        assert_program(&d, modifier, &[keyword]);
        assert!(d.capabilities.contains(&cap));
        assert!(
            !d.capabilities
                .contains(&Capability::TemporaryCreatureKeywordProgramV1)
        );
        if let Some(expected) = existing
            .definitions
            .iter()
            .find(|old| old.oracle_id == d.oracle_id)
        {
            assert_eq!(canonical(&d).unwrap(), canonical(expected).unwrap());
        }
        // Typed IR provenance controls, not claims that complete-source
        // lowering emits arbitrary earlier blocks. Empty spell blocks and
        // nonspell blocks on instants/sorceries remain unsupported.
        let mut shifted = d.clone();
        for instruction in &mut shifted.spell.as_mut().unwrap().instructions {
            match instruction {
                Instruction::GrantKeywords { source_block, .. }
                | Instruction::ModifyPowerToughness { source_block, .. } => {
                    *source_block = 1;
                }
                _ => unreachable!(),
            }
        }
        assert_eq!(shifted.capabilities, d.capabilities);
        if modifier.is_none() {
            shifted.validate().unwrap();
            let descriptor =
                temporary_creature_keyword_program(shifted.spell.as_ref().unwrap()).unwrap();
            assert_eq!(descriptor.keywords, &[keyword]);
            assert!(descriptor.modifier.is_none());
        } else {
            assert!(shifted.validate().is_err());
        }
    }
    // The existing catalog::tests::all_committed_catalogs_are_fresh selector
    // separately checks every old catalog's complete IR bytes and digest.
}
