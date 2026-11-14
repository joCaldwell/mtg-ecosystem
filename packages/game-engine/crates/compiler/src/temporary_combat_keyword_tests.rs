//! Exact d705 archive witnesses; mutations below are synthetic compiler probes.
//! CR 205.3k, 601.2c, 608.2c, 611.2a/c, 613.1f/4c, 514.2,
//! 702.7, 702.10 and 702.19 (pinned 2026-09-25): one captured target,
//! ordered stat/keyword effects and end-of-turn duration use the existing family.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
//! Native tests are UNRUN by this child; parent owns combined qualification.
use super::*;
use serde_json::{Value, json};
const HEADER: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
struct Witness {
    raw: &'static str,
    name: &'static str,
    oracle: &'static str,
    record: &'static str,
    source: &'static str,
    ast: &'static str,
    binding: &'static str,
    index: u32,
    generic: i64,
    color: Color,
    modifier: Option<(i64, i64)>,
    keywords: &'static [KeywordAbility],
}
const WITNESSES: &[Witness] = &[
    Witness {
        raw: include_str!("../../../tests/fixtures/0b3bdbc6-cde7-4187-b13c-82fd02ae4037.json"),
        name: "Tread Upon",
        oracle: "0b3bdbc6-cde7-4187-b13c-82fd02ae4037",
        record: "977b9e20c96a8acef3abf32c8376e802fe0f04f9bfeb4480c972de6f30694051",
        source: "635e18a0ba7cf0c8df67b347000fe3489e82da0faeb4b0a02a3c84716986dc38",
        ast: "faed867c03543292e1b0f12482728662b090449b9790b6cc812e7de5337c23ad",
        binding: "c37c502243953ae2350acdaa48c6010a9bdf9bbcc04a2d7e2a04b9434f24c8a2",
        index: 1701,
        generic: 1,
        color: Color::G,
        modifier: Some((2, 2)),
        keywords: &[KeywordAbility::Trample],
    },
    Witness {
        raw: include_str!("../../../tests/fixtures/35efb1e6-6be2-4a38-969a-199524d48e02.json"),
        name: "Fervent Strike",
        oracle: "35efb1e6-6be2-4a38-969a-199524d48e02",
        record: "152136d12edd9e1755b7c2a3de0a51c4c4774007ed99574cc87d3a5480620b30",
        source: "30021aa68c324c271200418d2ca87cfc49e8b38494d193681f25b92195f445a5",
        ast: "819a5b6b5f4b73ffe1c83978b87e8c33fec0595eb48a267a3da828d17e1c4907",
        binding: "28f3ec2c3e5f15ccdae6e57546a321ec17597d3a3b7190e9cf1188e880100485",
        index: 8040,
        generic: 0,
        color: Color::R,
        modifier: Some((1, 0)),
        keywords: &[KeywordAbility::FirstStrike, KeywordAbility::Haste],
    },
    Witness {
        raw: include_str!("../../../tests/fixtures/6b37b0ef-7e89-447f-a189-b3ea7b42143f.json"),
        name: "Whirling Strike",
        oracle: "6b37b0ef-7e89-447f-a189-b3ea7b42143f",
        record: "d7d66df0fd77225ac4e0eab4facfb5138f9c7e5c0510aacb9ebbee4f45e81171",
        source: "0c4e79867223778aedc4319262c9739fe0bd3cb6054402a03ff7f0a428140bc2",
        ast: "e593d2bb736b06a5f06cd31b829b5132e28bf895c460b8b0ccff7b2eec0dd278",
        binding: "5f35879696015628d775f5298f3a9ba3bf6656c0489dd41500f7158b8436dd90",
        index: 16144,
        generic: 1,
        color: Color::R,
        modifier: Some((2, 0)),
        keywords: &[KeywordAbility::FirstStrike, KeywordAbility::Trample],
    },
    Witness {
        raw: include_str!("../../../tests/fixtures/85b0a2ea-dbdc-456e-9ce1-faa8c13f84d0.json"),
        name: "Uncanny Speed",
        oracle: "85b0a2ea-dbdc-456e-9ce1-faa8c13f84d0",
        record: "4cc4621ff2014af61f42932dd178cbbea3ace3dfaf38994574a852a4d4f8fabb",
        source: "e39be9ea89184351b52587870acdb44f8cfc4de29901e1d6eba6ed189352ea2d",
        ast: "89650c1bc5952b82f5681732b2485d15b41b52f84f9553bba94221dfb9c086cc",
        binding: "93c2e772151229f77fa2d7074c55bd2df44417542460a81430e775c40082e6e8",
        index: 20121,
        generic: 1,
        color: Color::R,
        modifier: Some((3, 0)),
        keywords: &[KeywordAbility::Haste],
    },
    Witness {
        raw: include_str!("../../../tests/fixtures/fa46ced5-b701-4aa3-b4a1-36fb15b80696.json"),
        name: "Unnatural Speed",
        oracle: "fa46ced5-b701-4aa3-b4a1-36fb15b80696",
        record: "7fc4ce8cf82a73c684e2c4c179eeed80d2e2dfd924d73a9843bb07afedaa8e42",
        source: "1b0d697cdd068e3e74c2d49803871c2df1eb8b17430626e8330fc835badaf0a3",
        ast: "8a328c18d1c07c035f4167ff2a7c71ac962192a17d7986cb3b7cb6e1db71ad6d",
        binding: "5c79d8940bbf375347edabf45ec104f30ef218c31f7f1aa6c1b0790d9487afeb",
        index: 37818,
        generic: 0,
        color: Color::R,
        modifier: None,
        keywords: &[KeywordAbility::Haste],
    },
];
fn lowered(raw: &str) -> Box<Definition> {
    let result = compile(HEADER, raw);
    let CompileResult::Lowered { definition } = result else {
        panic!("expected complete temporary combat keyword lowering: {result:?}");
    };
    definition.validate().unwrap();
    definition
}
// Keep the line mirror and all integrity hashes valid, isolating semantic guards.
fn changed_ability(raw: &str, change: impl FnOnce(&mut Value)) -> String {
    tests::changed_source(raw, |r| {
        let a = &mut r["faces"][0]["result"]["abilities"][0];
        change(a);
        r["faces"][0]["result"]["lines"][0]["ability"] = a.clone();
    })
}
fn assert_program(d: &Definition, modifier: Option<(i64, i64)>, keywords: &[KeywordAbility]) {
    assert_eq!(CURRENT_IR_VERSION, 1);
    let mut capabilities = BTreeSet::from([
        Capability::NormalLayoutV1,
        Capability::CastInstantV1,
        Capability::CreatureTargetV1,
        Capability::TemporaryCreatureKeywordProgramV1,
    ]);
    if modifier.is_some() {
        capabilities.insert(Capability::TemporaryPowerToughnessV1);
    }
    capabilities.extend(keywords.iter().map(|keyword| match keyword {
        KeywordAbility::Haste => Capability::HasteV1,
        KeywordAbility::Trample => Capability::TrampleV1,
        KeywordAbility::FirstStrike => Capability::FirstStrikeV1,
        _ => panic!("unexpected source keyword {keyword:?}"),
    }));
    assert_eq!(d.capabilities, capabilities);
    assert_eq!(d.capabilities, d.requirements());
    assert!(!d.x_mana_cost);
    assert!(d.keywords.is_empty() && d.intrinsic_zone.is_none());
    assert!(d.activated.is_empty() && d.triggered.is_empty() && d.static_abilities.is_empty());
    assert!(d.mana_abilities.is_empty());
    let p = d.spell.as_ref().unwrap();
    assert_eq!(p.timing, SpellTiming::Instant);
    assert_eq!(p.resolution, SpellResolution::Instructions);
    assert_eq!(p.functioning_zone, FunctioningZone::Stack);
    assert!(p.modes.is_empty() && p.additional_sacrifice.is_none());
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
    let instructions = vec![Instruction::GrantKeywords {
        modifier: modifier.map(|(power, toughness)| CreatureStatModifier {
            power: power.into(),
            toughness: toughness.into(),
        }),
        target_slot: 0,
        keywords: keywords.to_vec(),
        duration: Duration::UntilEndOfTurn,
        source_block: 0,
    }];
    assert_eq!(p.instructions, instructions);
    let descriptor = temporary_creature_keyword_program(p).unwrap();
    let quantities = modifier.map(|(p, t)| (Quantity::from(p), Quantity::from(t)));
    assert_eq!(
        descriptor.modifier,
        quantities.as_ref().map(|(p, t)| (p, t))
    );
    assert_eq!(descriptor.keywords, keywords);
}
#[test]
fn temporary_combat_keyword_complete_five_frozen_sources() {
    for w in WITNESSES {
        let r: Record = decode(w.raw, "record").unwrap();
        let raw: Value = serde_json::from_str(w.raw).unwrap();
        assert_eq!(sha256(w.raw.trim_end().as_bytes()), w.record);
        assert_eq!(sha256(r.source.get().as_bytes()), w.source);
        assert_eq!(sha256(r.binding.get().as_bytes()), w.binding);
        let result: ParseResult = decode(r.faces[0].result.get(), "result").unwrap();
        assert_eq!(
            sha256(format!("[{}]", result.abilities.get()).as_bytes()),
            w.ast
        );
        assert_eq!(raw["source"]["layout"], "normal");
        assert_eq!(raw["source"]["legalities"]["commander"], "legal");
        let target = json!({"ref":"target","filter":{"types":["creature"]},"count":{"amount":"fixed","value":1},"selectionId":"line:0:0"});
        let mut steps = vec![];
        if let Some((power, toughness)) = w.modifier {
            steps.push(json!({"effect":"pump","what":target,"power":{"sign":1,"amount":{"amount":"fixed","value":power}},"toughness":{"sign":1,"amount":{"amount":"fixed","value":toughness}},"duration":{"duration":"end-of-turn"}}));
        }
        let abilities: Vec<Value> = w
            .keywords
            .iter()
            .map(|keyword| json!({"gained":"keyword","keyword":{"keyword":keyword}}))
            .collect();
        steps.push(json!({"effect":"gain-abilities","what":target,"abilities":abilities,"duration":{"duration":"end-of-turn"}}));
        let ability = json!({"kind":"spell","effects":[{"steps":steps}]});
        assert_eq!(raw["faces"][0]["result"]["abilities"], json!([ability]));
        assert_eq!(
            raw["faces"][0]["result"]["lines"],
            json!([{"text":raw["source"]["oracle_text"],"ok":true,"ability":ability}])
        );
        let refs = raw["binding"]["references"].as_array().unwrap();
        assert_eq!(refs.len(), steps.len());
        for (i, reference) in refs.iter().enumerate() {
            assert_eq!(reference["expression"], target);
            assert_eq!(reference["binding"], "bound");
            assert_eq!(reference["target"], refs[0]["target"]);
            assert_eq!(
                reference["path"],
                format!("/faces/0/abilities/0/effects/0/steps/{i}/what")
            );
        }
        let registry: Value =
            serde_json::from_str(include_str!("../../../tests/fixtures/provenance.json")).unwrap();
        let accepted: Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/accepted-archives.json"
        ))
        .unwrap();
        let entry = registry["records"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["oracle_id"] == w.oracle)
            .unwrap();
        assert_eq!(entry["record_sha256"], w.record);
        assert_eq!(entry["source_sha256"], w.source);
        assert_eq!(entry["ast_sha256"], w.ast);
        assert_eq!(entry["archive_line"], w.index + 2);
        assert_eq!(entry["archive"], "archive-manifest-current.json");
        let archive = accepted
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["manifest_file"] == entry["archive"])
            .unwrap();
        assert!(
            archive["record_sha256"]
                .as_array()
                .unwrap()
                .contains(&json!(w.record))
        );
        assert_eq!(
            archive["file_sha256"],
            "d705222b280a71cb5a5362f657d530b22402ca51687bac3227c1d6db403d0282"
        );
        assert_eq!(
            registry["rules_sha256"],
            "8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca"
        );
        let d = lowered(w.raw);
        assert_eq!(d.oracle_id, w.oracle);
        assert_eq!(d.name, w.name);
        assert_eq!(d.characteristics.kind, CardKind::Instant);
        assert_eq!(
            d.characteristics.mana_cost,
            ManaCost {
                generic: w.generic.into(),
                colored: vec![w.color]
            }
        );
        assert_eq!(d.characteristics.colors, [w.color]);
        assert_eq!(d.characteristics.color_identity, [w.color]);
        assert_eq!(
            d.characteristics.subtypes,
            if w.modifier.is_none() {
                vec!["Arcane"]
            } else {
                vec![]
            }
        );
        assert_eq!(d.provenance.source_index, w.index);
        assert_eq!(d.provenance.source_sha256, w.source);
        assert_eq!(d.provenance.ast_sha256, w.ast);
        assert_eq!(d.provenance.binding_sha256, w.binding);
        assert_program(&d, w.modifier, w.keywords);
    }
}
#[test]
fn temporary_combat_keyword_parameters_identity_and_serialized_order() {
    for w in WITNESSES {
        let original = lowered(w.raw);
        let renamed = tests::changed_source(w.raw, |r| {
            r["source"]["name"] = "Independent combat family witness".into();
            r["faces"][0]["result"]["name"] = "Independent combat family witness".into();
            let old = r["oracleIdentity"].as_str().unwrap().to_owned();
            let new = "00000000-0000-4000-8000-000000000003";
            r["oracleIdentity"] = new.into();
            r["source"]["oracle_id"] = new.into();
            r["source"]["id"] = "00000000-0000-4000-8000-000000000004".into();
            let binding = serde_json::to_string(&r["binding"])
                .unwrap()
                .replace(&old, new);
            r["binding"] = serde_json::from_str(&binding).unwrap();
        });
        assert_eq!(lowered(&renamed).spell, original.spell);
        let reversed = changed_ability(w.raw, |a| {
            a["effects"][0]["steps"]
                .as_array_mut()
                .unwrap()
                .last_mut()
                .unwrap()["abilities"]
                .as_array_mut()
                .unwrap()
                .reverse();
        });
        let mut keywords = w.keywords.to_vec();
        keywords.reverse();
        assert_program(&lowered(&reversed), w.modifier, &keywords);
    }
    for (power, toughness) in [(-3_i64, 5_i64), (0, -2), (7, 0)] {
        let raw = changed_ability(WITNESSES[0].raw, |a| {
            for (field, value) in [("power", power), ("toughness", toughness)] {
                a["effects"][0]["steps"][0][field] = json!({"sign":if value < 0 {-1} else {1},"amount":{"amount":"fixed","value":value.abs()}});
            }
        });
        assert_program(
            &lowered(&raw),
            Some((power, toughness)),
            WITNESSES[0].keywords,
        );
    }
}
#[test]
fn temporary_combat_keyword_strict_gained_decoder() {
    let valid = json!([
        {"gained":"keyword","keyword":{"keyword":"haste"}},
        {"gained":"keyword","keyword":{"keyword":"first strike"}},
        {"gained":"keyword","keyword":{"keyword":"trample"}}
    ]);
    assert_eq!(
        lower_temporary_creature_keywords(valid.as_array().unwrap()).unwrap(),
        [
            KeywordAbility::Haste,
            KeywordAbility::FirstStrike,
            KeywordAbility::Trample
        ]
    );
    let malformed = [
        Value::Null,
        json!(false),
        json!(2),
        json!("haste"),
        json!([]),
        json!({}),
        json!({"gained":"keyword"}),
        json!({"gained":"keyword","keyword":null}),
        json!({"gained":"keyword","keyword":{"keyword":null}}),
        json!({"gained":"keyword","keyword":{"keyword":"first-strike"}}),
        json!({"gained":"keyword","keyword":{"keyword":"vigilance"}}),
        json!({"gained":"activated","keyword":{"keyword":"haste"}}),
        json!({"gained":"triggered","keyword":{"keyword":"haste"}}),
        json!({"gained":"keyword","keyword":{"keyword":"haste"},"extra":null}),
        json!({"gained":"keyword","keyword":{"keyword":"haste","cost":[]}}),
        json!({"gained":"keyword","keyword":{"keyword":"haste","filter":null}}),
    ];
    for bad in malformed {
        for pair in [
            [valid[0].clone(), bad.clone()],
            [bad.clone(), valid[0].clone()],
        ] {
            assert!(lower_temporary_creature_keywords(&pair).is_err(), "{bad}");
        }
    }
    assert!(lower_temporary_creature_keywords(&[]).is_err());
    for keyword in valid.as_array().unwrap() {
        assert!(lower_temporary_creature_keywords(&[keyword.clone(), keyword.clone()]).is_err());
    }
}
#[test]
fn temporary_combat_keyword_complete_sources_reject_unconsumed_shapes() {
    for w in WITNESSES {
        for variant in 0..30 {
            let raw = changed_ability(w.raw, |a| {
                let steps = &mut a["effects"][0]["steps"];
                let last = steps.as_array().unwrap().len() - 1;
                match variant {
                    0 => steps[last]["abilities"] = json!([]),
                    1 => steps[last]["abilities"] = Value::Null,
                    2 => steps[last]["abilities"][0]["keyword"]["keyword"] = "vigilance".into(),
                    3 => steps[last]["abilities"][0]["extra"] = Value::Null,
                    4 => steps[last]["abilities"][0]["keyword"]["extra"] = Value::Null,
                    5 => steps[last]["abilities"][0]["gained"] = "activated".into(),
                    6 => { let x = steps[last]["abilities"][0].clone(); steps[last]["abilities"].as_array_mut().unwrap().push(x); },
                    7 => steps[last]["duration"] = Value::Null,
                    8 => steps[last]["duration"]["duration"] = "forever".into(),
                    9 => steps[last]["duration"]["condition"] = json!({"condition":"you-control-creature"}),
                    10 => steps[last]["what"] = Value::Null,
                    11 => steps[last]["what"]["selectionId"] = "".into(),
                    12 => steps[last]["what"]["count"]["value"] = 2.into(),
                    13 => steps[last]["what"]["filter"]["control"] = "opponent".into(),
                    14 => steps[last]["what"]["filter"]["tapped"] = true.into(),
                    15 => steps[last]["what"]["filter"]["types"] = json!(["artifact"]),
                    16 => steps[last]["what"]["filter"]["subtypes"] = json!(["Goblin"]),
                    17 => steps[last]["condition"] = json!({"condition":"you-control-creature"}),
                    18 => steps[last]["extra"] = true.into(),
                    19 => { let x = steps[last].clone(); steps.as_array_mut().unwrap().push(x); },
                    20 => steps.as_array_mut().unwrap().push(json!({"effect":"draw","who":{"player":"you"},"count":{"amount":"fixed","value":1}})),
                    21 => a["effects"][0]["bindings"] = json!([{"variable":"unused","value":{"amount":"fixed","value":1}}]),
                    22 => { let child = a["effects"].clone(); a["effects"] = json!([{"steps":[{"effect":"modal","count":{"exactly":1},"options":[{"effects":child.clone()},{"effects":child}]}]}]); },
                    23 => { a["kind"] = "activated".into(); a["costs"] = json!([{"cost":"tap"}]); },
                    24 => { a["kind"] = "triggered".into(); a["trigger"] = json!({"trigger":"enters","what":{"ref":"this","noun":"creature"}}); },
                    25 => a["condition"] = json!({"condition":"you-control-creature"}),
                    26 => steps[last]["abilities"][0]["keyword"]["keyword"] = Value::Null,
                    27 => steps[last]["abilities"][0]["keyword"]["keyword"] = 2.into(),
                    28 => steps[last]["abilities"][0]["keyword"]["filter"] = Value::Null,
                    _ => steps[last]["what"]["filter"]["status"] = json!(["attacking"]),
                }
            });
            assert!(
                matches!(compile(HEADER, &raw), CompileResult::Unsupported { .. }),
                "{} variant {variant}",
                w.name
            );
        }
    }
}
#[test]
fn temporary_combat_keyword_compound_preserves_sentence_partition_and_rejects_semantic_changes() {
    for w in WITNESSES.iter().filter(|w| w.modifier.is_some()) {
        for variant in 0..10 {
            let raw = changed_ability(w.raw, |a| {
                let s = &mut a["effects"][0]["steps"];
                match variant {
                    0 => s.as_array_mut().unwrap().reverse(),
                    1 => s[1]["what"]["selectionId"] = "line:0:1".into(),
                    2 => s[0]["duration"]["duration"] = "forever".into(),
                    3 => s[0]["power"]["amount"] = json!({"amount":"variable","name":"x"}),
                    4 => {
                        s[0]["toughness"]["amount"] =
                            json!({"amount":"count","of":{"types":["creature"]}})
                    }
                    5 => s[0]["what"]["filter"]["control"] = "you".into(),
                    6 => s[1]["what"]["filter"]["control"] = "you".into(),
                    7 => {
                        let pump = s[0].clone();
                        s.as_array_mut().unwrap().insert(0, pump);
                    }
                    8 => {
                        let grant = s.as_array_mut().unwrap().pop().unwrap();
                        a["effects"]
                            .as_array_mut()
                            .unwrap()
                            .push(json!({"steps":[grant]}));
                    }
                    _ => {
                        let pump = s[0].clone();
                        let grant = s[1].clone();
                        a["effects"] = json!([{"steps":[pump,grant],"bindings":[{"variable":"unconsumed","value":{"amount":"fixed","value":1}}]}]);
                    }
                }
            });
            if variant == 8 {
                // Source blocks index abilities, not sentences (CR 608.2c).
                // Repartitioning the same ordered operations within one ability
                // preserves both instruction provenance and complete semantics.
                assert_program(&lowered(&raw), w.modifier, w.keywords);
                continue;
            }
            assert!(
                matches!(compile(HEADER, &raw), CompileResult::Unsupported { .. }),
                "{} variant {variant}",
                w.name
            );
        }
    }
}
#[test]
fn temporary_combat_keyword_extra_cost_and_unparsed_source_clause_reject() {
    for w in WITNESSES {
        let additional = tests::changed_source(w.raw, |r| {
            let text = "As an additional cost to cast this spell, sacrifice an artifact.";
            let cost = json!({"kind":"additional-cost","costs":[{"cost":"sacrifice","what":{"ref":"filter","filter":{"types":["artifact"]},"count":{"amount":"fixed","value":1}}}]});
            let original = r["source"]["oracle_text"].as_str().unwrap().to_owned();
            r["source"]["oracle_text"] = format!("{text}\n{original}").into();
            r["faces"][0]["result"]["abilities"]
                .as_array_mut()
                .unwrap()
                .insert(0, cost.clone());
            r["faces"][0]["result"]["lines"]
                .as_array_mut()
                .unwrap()
                .insert(0, json!({"text":text,"ok":true,"ability":cost}));
        });
        assert!(
            matches!(
                compile(HEADER, &additional),
                CompileResult::Unsupported { .. }
            ),
            "{} additional cost",
            w.name
        );
        let clause = tests::changed_source(w.raw, |r| {
            let original = r["source"]["oracle_text"].as_str().unwrap().to_owned();
            r["source"]["oracle_text"] = format!("{original}\nDraw a card.").into();
        });
        assert!(
            matches!(compile(HEADER, &clause), CompileResult::Invalid { path, .. } if path == "lines"),
            "{} unparsed clause",
            w.name
        );
    }
}
#[test]
fn temporary_combat_keyword_separate_source_blocks_reject() {
    for w in WITNESSES.iter().filter(|w| w.modifier.is_some()) {
        let raw = tests::changed_source(w.raw, |r| {
            let steps = &r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"];
            let abilities = json!([
                {"kind":"spell","effects":[{"steps":[steps[0].clone()]}]},
                {"kind":"spell","effects":[{"steps":[steps[1].clone()]}]}
            ]);
            r["faces"][0]["result"]["abilities"] = abilities.clone();
            // Synthetic source blocks, mirrored and rehashed, reach the complete
            // program guard rather than failing source/line integrity first.
            r["source"]["oracle_text"] = "Synthetic first block.\nSynthetic second block.".into();
            r["faces"][0]["result"]["lines"] = json!([
                {"text":"Synthetic first block.","ok":true,"ability":abilities[0]},
                {"text":"Synthetic second block.","ok":true,"ability":abilities[1]}
            ]);
        });
        assert!(
            matches!(compile(HEADER, &raw), CompileResult::Unsupported { .. }),
            "{} separate source blocks",
            w.name
        );
    }
}
#[test]
fn temporary_combat_keyword_source_hashes_and_line_mirror_remain_strict() {
    for w in WITNESSES {
        for variant in 0..4 {
            let normalized = tests::changed_source(w.raw, |_| {});
            let mut r: Value = serde_json::from_str(&normalized).unwrap();
            let expected_path = match variant {
                0 => {
                    r["sourceSha256"] = "0".repeat(64).into();
                    "record"
                }
                1 => {
                    r["bindingSha256"] = "0".repeat(64).into();
                    "record"
                }
                2 => {
                    r["astSha256"] = "0".repeat(64).into();
                    "astSha256"
                }
                _ => {
                    r["faces"][0]["result"]["lines"][0]["ability"]["effects"] = json!([]);
                    "lines"
                }
            };
            assert!(
                matches!(compile(HEADER, &canonical(&r).unwrap()), CompileResult::Invalid { path, .. } if path == expected_path),
                "{} integrity variant {variant}",
                w.name
            );
        }
    }
}
