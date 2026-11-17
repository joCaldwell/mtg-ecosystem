//! Complete frozen d705 witnesses and explicitly synthetic compiler AST controls.
//! All tests added for this increment are native UNRUN; the parent owns the
//! single combined contracts/compiler/runtime/schema pass.
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
        raw: include_str!("../../../tests/fixtures/1cb419cc-d8e4-4676-987f-089cebccc374.json"),
        name: "Masterful Flourish",
        oracle: "1cb419cc-d8e4-4676-987f-089cebccc374",
        record: "d44e04907bfdf1276997613ec69a3363e8efe1a012787e79470ae363c7538fda",
        source: "b2a7f5d9ca372de26bad356a0b3c88a3afa978c73126fc5191b27a69288c87e8",
        ast: "2f3db121fcddc97024ad40bf918bdea8865ff51425fda25bff15befc334ba5f9",
        binding: "1f186d5a35b3d68e44339d06ded73c7dcd7c136e5346bfa46b268e740e4a46d7",
        index: 4402,
        generic: 0,
        color: Color::B,
        modifier: Some((1, 0)),
        keywords: &[KeywordAbility::Indestructible],
    },
    Witness {
        raw: include_str!("../../../tests/fixtures/2f990b54-fbf3-4949-85bb-9ba39710e72a.json"),
        name: "Dive Down",
        oracle: "2f990b54-fbf3-4949-85bb-9ba39710e72a",
        record: "4a62ccafe5062257a1e5d39ad9332f9bb613635a3a70df491da976f9acb08889",
        source: "66efad9b7c029950623859e12783e29b7e6b94f86f7b5a7c9938950b9f6c6f71",
        ast: "b09b388d5e7c939a2fd0ef5975251605a9df1f30345b7721e4c2ef2f65904491",
        binding: "18617143b24f4d395804829d5340465c93ffc4d32c48f9c2eb22531cc4a7ac9d",
        index: 7084,
        generic: 0,
        color: Color::U,
        modifier: Some((0, 3)),
        keywords: &[KeywordAbility::Hexproof],
    },
    Witness {
        raw: include_str!("../../../tests/fixtures/5a851367-1c4a-4cc9-a9c3-3d2775986b4c.json"),
        name: "Blossoming Defense",
        oracle: "5a851367-1c4a-4cc9-a9c3-3d2775986b4c",
        record: "e7617a29006c2bc42fd5680f91f77edd7df5078413725638bf56c3fd6cb1ff29",
        source: "74235313d6f066ee9d8c21afd7e071f829081a5718861be591e19819710521cb",
        ast: "60a7bf2acce3ca646171fb88dbeca8e8deef97025f67cf34aaa8fbd0e3936d30",
        binding: "9f604a053dbd312f11e0732cc950e28d474515d3dbba59cce705f87ceaabd55b",
        index: 13608,
        generic: 0,
        color: Color::G,
        modifier: Some((2, 2)),
        keywords: &[KeywordAbility::Hexproof],
    },
    Witness {
        raw: include_str!("../../../tests/fixtures/694cb93a-bcc5-44b0-a76c-19ae4a4e5e0f.json"),
        name: "Ranger's Guile",
        oracle: "694cb93a-bcc5-44b0-a76c-19ae4a4e5e0f",
        record: "b711b87b0636d600e660a6f36896081b95f0b5280a4a7ceb2d106826c6d6c689",
        source: "e8aab543913efc2d3c2bb6e6f717c75e73f2d681df2b5b8ebb7bd69b290c38b9",
        ast: "f25e6117ace811b6812fea4a6f4a898727de70f04c55b20d784d41db67e98b9d",
        binding: "ba3975eb17d94ced131ed52acab2b95ef1bf06086d34f51871f6ed758bd5844d",
        index: 15844,
        generic: 0,
        color: Color::G,
        modifier: Some((1, 1)),
        keywords: &[KeywordAbility::Hexproof],
    },
    Witness {
        raw: include_str!("../../../tests/fixtures/c4229cc8-11e5-4221-b00d-86969701e21b.json"),
        name: "Alesha's Legacy",
        oracle: "c4229cc8-11e5-4221-b00d-86969701e21b",
        record: "3df7f4d578b2111ff4d4de1aa2a9a06c444bddbc3061dd5a345f2d1cd069f9c8",
        source: "6ba5aff403f137c39c1ec31c82272d7ed0ff24b45938003b0af80d794c8fd990",
        ast: "62abd8b42240efb1f0ababf36c8584557e869a3c3a1b766d0466fde8ff043b7b",
        binding: "a155f0f8a87d58807c0f477c3e04c3174b9cd978618e1d7b7a91c871c96de2b5",
        index: 29565,
        generic: 1,
        color: Color::B,
        modifier: None,
        keywords: &[KeywordAbility::Deathtouch, KeywordAbility::Indestructible],
    },
];
fn lowered(raw: &str) -> Box<Definition> {
    let result = compile(HEADER, raw);
    let CompileResult::Lowered { definition } = result else {
        panic!("expected complete controlled family lowering: {result:?}");
    };
    definition.validate().unwrap();
    definition
}
// Synthetic AST controls rehash the source/AST and mirror the aggregate line.
// They are compiler probes, not assertions that the source parser emits them.
fn changed_ability(raw: &str, change: impl FnOnce(&mut Value)) -> String {
    tests::changed_source(raw, |r| {
        let a = &mut r["faces"][0]["result"]["abilities"][0];
        change(a);
        r["faces"][0]["result"]["lines"][0]["ability"] = a.clone();
    })
}
fn assert_program(
    d: &Definition,
    domain: TargetDomain,
    modifier: Option<(i64, i64)>,
    keywords: &[KeywordAbility],
) {
    assert_eq!(CURRENT_IR_VERSION, 1);
    assert_eq!(d.capabilities, d.requirements());
    assert!(!d.x_mana_cost);
    assert!(d.keywords.is_empty());
    assert!(d.activated.is_empty() && d.triggered.is_empty() && d.static_abilities.is_empty());
    assert!(d.mana_abilities.is_empty() && d.intrinsic_zone.is_none());
    let p = d.spell.as_ref().unwrap();
    assert_eq!(p.timing, SpellTiming::Instant);
    assert_eq!(p.resolution, SpellResolution::Instructions);
    assert_eq!(p.functioning_zone, FunctioningZone::Stack);
    assert!(p.additional_sacrifice.is_none() && p.modes.is_empty());
    assert_eq!(
        p.targets,
        [TargetSpec {
            slot: 0,
            selection_id: "line:0:0".into(),
            chooser: Chooser::Controller,
            stage: EvaluationStage::Announcement,
            domain,
            filter: None,
        }]
    );
    let descriptor = temporary_creature_keyword_program(p).unwrap();
    let quantities =
        modifier.map(|(power, toughness)| (Quantity::from(power), Quantity::from(toughness)));
    assert_eq!(
        descriptor.modifier,
        quantities
            .as_ref()
            .map(|(power, toughness)| (power, toughness))
    );
    assert_eq!(descriptor.keywords, keywords);
    let expected = vec![Instruction::GrantKeywords {
        modifier: modifier.map(|(power, toughness)| CreatureStatModifier {
            power: power.into(),
            toughness: toughness.into(),
        }),
        target_slot: 0,
        keywords: keywords.to_vec(),
        duration: Duration::UntilEndOfTurn,
        source_block: 0,
    }];
    assert_eq!(p.instructions, expected);
    let mut expected_caps = BTreeSet::from([
        Capability::NormalLayoutV1,
        Capability::CastInstantV1,
        Capability::TemporaryCreatureKeywordProgramV1,
    ]);
    if domain == TargetDomain::BattlefieldCreature {
        expected_caps.insert(Capability::CreatureTargetV1);
    } else {
        expected_caps.insert(Capability::AttachmentsV1);
    }
    if modifier.is_some() {
        expected_caps.insert(Capability::TemporaryPowerToughnessV1);
    }
    for keyword in keywords {
        expected_caps.insert(match keyword {
            KeywordAbility::Hexproof => Capability::AttachmentsV1,
            KeywordAbility::Flying => Capability::FlyingV1,
            KeywordAbility::Deathtouch => Capability::DeathtouchV1,
            KeywordAbility::Indestructible => Capability::IndestructibleV1,
            KeywordAbility::Lifelink => Capability::LifelinkV1,
            _ => unreachable!(),
        });
    }
    assert_eq!(d.capabilities, expected_caps);
}

#[test]
fn controlled_temporary_keyword_complete_five_frozen_sources() {
    let registry: Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/provenance.json")).unwrap();
    let archives: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/accepted-archives.json"
    ))
    .unwrap();
    let accepted = archives
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["manifest_file"] == "archive-manifest-current.json")
        .unwrap();
    for w in WITNESSES {
        let r: Value = serde_json::from_str(w.raw).unwrap();
        assert_eq!(sha256(w.raw.trim_end().as_bytes()), w.record);
        assert_eq!(r["source"]["layout"], "normal");
        assert_eq!(r["source"]["legalities"]["commander"], "legal");
        assert_eq!(r["binding"]["fullyBound"], true);
        assert_eq!(r["binding"]["referencesBound"], true);
        assert_eq!(r["binding"]["diagnostics"], json!([]));
        assert!(
            accepted["record_sha256"]
                .as_array()
                .unwrap()
                .contains(&json!(w.record))
        );
        let provenance = registry["records"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["oracle_id"] == w.oracle)
            .unwrap();
        assert_eq!(provenance["record_sha256"], w.record);
        assert_eq!(provenance["source_sha256"], w.source);
        assert_eq!(provenance["ast_sha256"], w.ast);
        assert_eq!(provenance["archive_line"], w.index + 2);
        let d = lowered(w.raw);
        assert_eq!(d.name, w.name);
        assert_eq!(d.oracle_id, w.oracle);
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
            d.provenance.printing_id,
            r["source"]["id"].as_str().unwrap()
        );
        assert_eq!(d.provenance.source_index, w.index);
        assert_eq!(d.provenance.source_sha256, w.source);
        assert_eq!(d.provenance.ast_sha256, w.ast);
        assert_eq!(d.provenance.binding_sha256, w.binding);
        assert_eq!(
            d.provenance.archive_manifest_sha256,
            accepted["manifest_sha256"].as_str().unwrap()
        );
        assert_eq!(
            d.provenance.parser_sha256,
            accepted["parser_source_sha256"].as_str().unwrap()
        );
        assert_eq!(
            serde_json::from_str::<Value>(&d.provenance.source_json).unwrap(),
            r["source"]
        );
        assert_eq!(
            serde_json::from_str::<Value>(&d.provenance.abilities_json).unwrap(),
            r["faces"][0]["result"]["abilities"]
        );
        assert_program(
            &d,
            TargetDomain::ControlledBattlefieldCreature,
            w.modifier,
            w.keywords,
        );
        // CR109.5: every control qualifier binds to this spell's controller.
        let refs = r["binding"]["references"].as_array().unwrap();
        let control_refs: Vec<_> = refs
            .iter()
            .filter(|r| r["path"].as_str().unwrap().ends_with("/filter/control"))
            .collect();
        let steps = r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"]
            .as_array()
            .unwrap();
        assert_eq!(control_refs.len(), steps.len());
        for reference in control_refs {
            assert_eq!(reference["expression"], json!({"player":"you"}));
            assert_eq!(reference["binding"], "bound");
            assert_eq!(
                reference["target"],
                format!("card:{}/faces/0/abilities/0#you", w.oracle)
            );
        }
    }
}

#[test]
fn controlled_temporary_keyword_plain_hexproof_and_controlled_legacy_subsets() {
    // Synthetic compiler AST positives. The old four-keyword plain matrix and
    // canonical catalog tests remain the authority for unchanged subset IR.
    for keyword in [
        KeywordAbility::Flying,
        KeywordAbility::Deathtouch,
        KeywordAbility::Indestructible,
        KeywordAbility::Lifelink,
    ] {
        for modifier in [None, Some((1, 0))] {
            let witness = if modifier.is_some() {
                &WITNESSES[0]
            } else {
                &WITNESSES[4]
            };
            let raw = changed_ability(witness.raw, |a| {
                a["effects"][0]["steps"]
                    .as_array_mut()
                    .unwrap()
                    .last_mut()
                    .unwrap()["abilities"] =
                    json!([{"gained":"keyword","keyword":{"keyword":keyword}}]);
            });
            assert_program(
                &lowered(&raw),
                TargetDomain::ControlledBattlefieldCreature,
                modifier,
                &[keyword],
            );
        }
    }
    for witness in [&WITNESSES[1], &WITNESSES[4]] {
        let raw = changed_ability(witness.raw, |a| {
            for step in a["effects"][0]["steps"].as_array_mut().unwrap() {
                step["what"]["filter"]
                    .as_object_mut()
                    .unwrap()
                    .remove("control");
            }
            a["effects"][0]["steps"]
                .as_array_mut()
                .unwrap()
                .last_mut()
                .unwrap()["abilities"] =
                json!([{"gained":"keyword","keyword":{"keyword":"hexproof"}}]);
        });
        assert_program(
            &lowered(&raw),
            TargetDomain::BattlefieldCreature,
            witness.modifier,
            &[KeywordAbility::Hexproof],
        );
    }
    // New vocabulary composes with the old vocabulary and retains order. This
    // is one compiler vector, not a duplicate of the existing schema matrix.
    let keywords = [
        KeywordAbility::Hexproof,
        KeywordAbility::Flying,
        KeywordAbility::Lifelink,
        KeywordAbility::Indestructible,
        KeywordAbility::Deathtouch,
    ];
    let abilities: Vec<_> = keywords
        .iter()
        .map(|k| json!({"gained":"keyword","keyword":{"keyword":k}}))
        .collect();
    assert_eq!(
        lower_temporary_creature_keywords(&abilities).unwrap(),
        keywords
    );
    let raw = changed_ability(WITNESSES[0].raw, |a| {
        a["effects"][0]["steps"][1]["abilities"] = json!(abilities)
    });
    assert_program(
        &lowered(&raw),
        TargetDomain::ControlledBattlefieldCreature,
        Some((1, 0)),
        &keywords,
    );
}

#[test]
fn controlled_temporary_keyword_rejects_every_unconsumed_target_field() {
    for w in WITNESSES {
        let r: Value = serde_json::from_str(w.raw).unwrap();
        let count = r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"]
            .as_array()
            .unwrap()
            .len();
        for step_index in 0..count {
            for variant in 0..29 {
                let raw = changed_ability(w.raw, |a| {
                    let target = &mut a["effects"][0]["steps"][step_index]["what"];
                    match variant {
                        0 => target["filter"]["control"] = Value::Null,
                        1 => target["filter"]["control"] = "opponent".into(),
                        2 => target["filter"]["control"] = "each-opponent".into(),
                        3 => target["filter"]["control"] = "owner".into(),
                        4 => target["filter"]["control"] = "unknown".into(),
                        5 => target["filter"]["control"] = json!({"player":"you"}),
                        6 => target["filter"]["owner"] = "you".into(),
                        7 => target["filter"]["types"] = json!(["artifact"]),
                        8 => target["filter"]["types"] = json!(["creature", "artifact"]),
                        9 => target["filter"]["types"] = json!([]),
                        10 => target["filter"]["subtypes"] = json!(["Human"]),
                        11 => target["filter"]["status"] = json!(["blocking"]),
                        12 => target["filter"]["tapped"] = true.into(),
                        13 => target["filter"]["extra"] = Value::Null,
                        14 => target["count"]["value"] = 0.into(),
                        15 => target["count"]["value"] = 2.into(),
                        16 => target["count"]["amount"] = "up-to".into(),
                        17 => target["upTo"] = true.into(),
                        18 => target["upTo"] = false.into(),
                        19 => target["optional"] = Value::Null,
                        20 => target["selectionId"] = "".into(),
                        21 => target["selectionId"] = Value::Null,
                        22 => target["ref"] = "each".into(),
                        23 => target["filter"]["control"] = false.into(),
                        24 => target["filter"]["cls"] = "permanent".into(),
                        25 => target["filter"]["power"] = json!({"op":"lte","value":2}),
                        26 => target["count"]["optional"] = true.into(),
                        27 => target["extra"] = true.into(),
                        _ => {
                            target["filter"].as_object_mut().unwrap().remove("control");
                            target["filter"]["owner"] = "you".into();
                        }
                    }
                });
                assert!(
                    matches!(compile(HEADER, &raw), CompileResult::Unsupported { .. }),
                    "{} step {step_index}, target variant {variant}",
                    w.name
                );
            }
        }
    }
}

#[test]
fn controlled_temporary_keyword_rejects_mismatched_selection_domains_and_occurrences() {
    for w in WITNESSES.iter().filter(|w| w.modifier.is_some()) {
        for step_index in 0..2 {
            for variant in 0..3 {
                let raw = changed_ability(w.raw, |a| {
                    let t = &mut a["effects"][0]["steps"][step_index]["what"];
                    match variant {
                        0 => {
                            t["filter"].as_object_mut().unwrap().remove("control");
                        }
                        1 => t["selectionId"] = "line:0:other".into(),
                        _ => {
                            t["filter"].as_object_mut().unwrap().remove("control");
                            t["selectionId"] = "line:0:other".into();
                        }
                    }
                });
                assert!(
                    matches!(compile(HEADER, &raw), CompileResult::Unsupported { .. }),
                    "{} step {step_index}, occurrence variant {variant}",
                    w.name
                );
            }
        }
    }
    // Direct compiler binder probes: assert mixed-domain failure precedes slot
    // reuse and cannot overwrite the first target, independently of contracts.
    let controlled = json!({"ref":"target","filter":{"types":["creature"],"control":"you"},"count":{"amount":"fixed","value":1},"selectionId":"line:0:0"});
    let mut plain = controlled.clone();
    plain["filter"].as_object_mut().unwrap().remove("control");
    let mut blocking = plain.clone();
    blocking["filter"]["status"] = json!(["blocking"]);
    for (first, second, domain) in [
        (
            &controlled,
            &plain,
            TargetDomain::ControlledBattlefieldCreature,
        ),
        (&plain, &controlled, TargetDomain::BattlefieldCreature),
        (
            &controlled,
            &blocking,
            TargetDomain::ControlledBattlefieldCreature,
        ),
        (
            &blocking,
            &controlled,
            TargetDomain::BlockingBattlefieldCreature,
        ),
    ] {
        let mut p = empty_program();
        let mut slots = BTreeMap::new();
        bind_pump_target(&mut p, &mut slots, first).unwrap();
        let before = p.targets.clone();
        assert_eq!(before[0].domain, domain);
        assert!(bind_pump_target(&mut p, &mut slots, second).is_err());
        assert_eq!(p.targets, before);
        assert_eq!(slots.len(), 1);
        assert!(bind_temporary_creature_target(&mut p, &mut slots, second).is_err());
        assert_eq!(p.targets, before);
    }
    // Blocking remains pump-only, with no prior target needed to reject it.
    assert!(
        bind_temporary_creature_target(&mut empty_program(), &mut BTreeMap::new(), &blocking)
            .is_err()
    );
}
fn empty_program() -> SpellProgram {
    SpellProgram {
        additional_sacrifice: None,
        timing: SpellTiming::Instant,
        resolution: SpellResolution::Instructions,
        functioning_zone: FunctioningZone::Stack,
        targets: vec![],
        instructions: vec![],
        modes: vec![],
    }
}

#[test]
fn controlled_temporary_keyword_rejects_incomplete_or_nonordinary_programs() {
    for w in WITNESSES {
        for variant in 0..23 {
            let raw = changed_ability(w.raw, |a| {
                let steps = &mut a["effects"][0]["steps"];
                let last = steps.as_array().unwrap().len() - 1;
                match variant {
                    0 => steps[last]["abilities"] = json!([]),
                    1 => steps[last]["abilities"][0]["keyword"]["keyword"] = "shroud".into(),
                    2 => steps[last]["abilities"][0]["keyword"]["keyword"] = "defender".into(),
                    3 => steps[last]["abilities"][0]["keyword"]["filter"] = Value::Null,
                    4 => steps[last]["abilities"][0]["keyword"]["cost"] = json!([]),
                    5 => { let duplicate = steps[last]["abilities"][0].clone(); steps[last]["abilities"].as_array_mut().unwrap().push(duplicate); },
                    6 => steps[last]["duration"]["duration"] = "forever".into(),
                    7 => steps[last]["duration"]["condition"] = json!({"condition":"you-control-creature"}),
                    8 => steps[last]["condition"] = json!({"condition":"you-control-creature"}),
                    9 => steps[last]["extra"] = true.into(),
                    10 => { let extra = steps[last].clone(); steps.as_array_mut().unwrap().push(extra); },
                    11 => steps.as_array_mut().unwrap().push(json!({"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1}})),
                    12 => a["effects"][0]["bindings"] = json!([{"variable":"unused","value":{"amount":"fixed","value":1}}]),
                    13 => { let effects = a["effects"].clone(); a["effects"] = json!([{"steps":[{"effect":"modal","count":{"exactly":1},"options":[{"effects":effects.clone()},{"effects":effects}]}]}]); },
                    14 => { a["kind"] = "activated".into(); a["costs"] = json!([{"cost":"tap"}]); },
                    15 => { a["kind"] = "triggered".into(); a["trigger"] = json!({"trigger":"enters","what":{"ref":"this","noun":"creature"}}); },
                    16 => a["condition"] = json!({"condition":"you-control-creature"}),
                    17 => a["costs"] = json!([]),
                    18 => steps[last]["abilities"][0]["keyword"]["keyword"] = "hexproof-from-black".into(),
                    19 => steps[last]["what"]["filter"]["status"] = json!(["attacking"]),
                    20 => steps[last]["abilities"][0]["gained"] = "activated".into(),
                    21 => steps[last]["abilities"][0]["keyword"]["keyword"] = Value::Null,
                    _ => a["effects"] = json!([]),
                }
            });
            assert!(
                matches!(compile(HEADER, &raw), CompileResult::Unsupported { .. }),
                "{} program variant {variant}",
                w.name
            );
        }
        // A complete, source-accounted additional sacrifice cost still rejects;
        // this reaches whole-program validation rather than a fake AST prefix.
        let raw = tests::changed_source(w.raw, |r| {
            let text = "As an additional cost to cast this spell, sacrifice an artifact.";
            r["source"]["oracle_text"] =
                format!("{text}\n{}", r["source"]["oracle_text"].as_str().unwrap()).into();
            let cost = json!({"kind":"additional-cost","costs":[{"cost":"sacrifice","what":{"ref":"filter","filter":{"types":["artifact"]},"count":{"amount":"fixed","value":1}}}]});
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
            matches!(compile(HEADER, &raw), CompileResult::Unsupported { path, .. } if path == "definition"),
            "{} additional cost",
            w.name
        );
    }
    for w in WITNESSES.iter().filter(|w| w.modifier.is_some()) {
        // Bare controlled pumps and reversed operation order remain unsupported.
        for variant in 0..4 {
            let raw = changed_ability(w.raw, |a| {
                let steps = a["effects"][0]["steps"].as_array_mut().unwrap();
                match variant {
                    0 => {
                        steps.pop();
                    }
                    1 => steps.reverse(),
                    2 => {
                        steps[0]["power"]["amount"]["amount"] = "x".into();
                    }
                    _ => {
                        steps[0]["duration"]["duration"] = "forever".into();
                    }
                }
            });
            assert!(
                matches!(compile(HEADER, &raw), CompileResult::Unsupported { .. }),
                "{} compound variant {variant}",
                w.name
            );
        }
    }
}

#[test]
fn controlled_temporary_keyword_other_creature_operators_keep_strict_scope() {
    // Direct compiler decoder probes: no global Target.filter broadening for
    // counter placement/removal/Ramos callers of bind_creature_target.
    for control in [json!("you"), Value::Null, json!("opponent")] {
        let target = json!({"ref":"target","filter":{"types":["creature"],"control":control},"count":{"amount":"fixed","value":1},"selectionId":"line:0:0"});
        assert!(serde_json::from_value::<Target>(target).is_err());
    }
    // Valid typed plain Target still binds exactly the old domain.
    let target: Target = serde_json::from_value(json!({"ref":"target","filter":{"types":["creature"]},"count":{"amount":"fixed","value":1},"selectionId":"line:0:0"})).unwrap();
    let mut p = empty_program();
    bind_creature_target(&mut p, &mut BTreeMap::new(), &target).unwrap();
    assert_eq!(p.targets[0].domain, TargetDomain::BattlefieldCreature);
}

#[test]
fn controlled_temporary_keyword_typed_ir_source_block_controls() {
    // Typed IR probes only: complete-source compilation does not manufacture
    // earlier empty/nonspell blocks on these instants.
    for w in WITNESSES {
        let mut d = lowered(w.raw);
        for instruction in &mut d.spell.as_mut().unwrap().instructions {
            match instruction {
                Instruction::ModifyPowerToughness { source_block, .. }
                | Instruction::GrantKeywords { source_block, .. } => *source_block = 1,
                _ => unreachable!(),
            }
        }
        if w.modifier.is_none() {
            d.validate().unwrap();
            assert!(temporary_creature_keyword_program(d.spell.as_ref().unwrap()).is_some());
        } else {
            assert!(d.validate().is_err());
        }
    }
}
