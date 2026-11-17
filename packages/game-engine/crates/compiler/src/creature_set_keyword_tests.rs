//! Complete frozen d705 sources plus synthetic AST boundary controls.
//! Native/compiler/schema execution is UNRUN here; parent owns qualification.
use super::*;
use serde_json::{Value, json};
const HEADER: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
struct Witness {
    raw: &'static str,
    oracle: &'static str,
    name: &'static str,
    record: &'static str,
    source: &'static str,
    ast: &'static str,
    binding: &'static str,
    index: u32,
    generic: i64,
    colors: &'static [Color],
    modifier: Option<(i64, i64)>,
    set: ObjectSet,
    keywords: &'static [KeywordAbility],
}
const WITNESSES: &[Witness] = &[
    Witness {
        raw: include_str!("../../../tests/fixtures/0cd28580-d718-4d0b-96e3-a836c2911c27.json"),
        oracle: "0cd28580-d718-4d0b-96e3-a836c2911c27",
        name: "Titanic Ultimatum",
        record: "300416d0ee6fc61deafaf6a8109a8659f88be4ce6de570bcb1e17c02506a40fe",
        source: "31426917ca29f50098f55cee9c43f234cbe9b1fd63f26c3571db85e3ce7540d2",
        ast: "4db64be520ece8f7e6eae9c10ed30d0c5cce40f3f08baa83be08d16efd8eb8dc",
        binding: "9dc41b63ee876779e723edd440ca2c320cc812a7675c5d57096ed5648b60cb17",
        index: 1948,
        generic: 0,
        colors: &[
            Color::R,
            Color::R,
            Color::G,
            Color::G,
            Color::G,
            Color::W,
            Color::W,
        ],
        modifier: Some((5, 5)),
        set: ObjectSet::ControllerCreatures,
        keywords: &[
            KeywordAbility::FirstStrike,
            KeywordAbility::Trample,
            KeywordAbility::Lifelink,
        ],
    },
    Witness {
        raw: include_str!("../../../tests/fixtures/204f9afe-c20b-4933-b5cd-aa572784762a.json"),
        oracle: "204f9afe-c20b-4933-b5cd-aa572784762a",
        name: "Overrun",
        record: "2a8643025c033d60ecc39f8bc28f731f40713169d6c4491be0fbefcfe33e1ccb",
        source: "b98ab20d89746f7e87f52f4bfb401f606e4e96958891c7d0cdb5f4b1dcf60828",
        ast: "4635a313be222fc3d4cb4121eb5440c4259102c455077b609b29b1dc12c759cd",
        binding: "0e3c665c1c2725b23a9359b2fce0f34e274b3b074fd2732e42b11093f0914e08",
        index: 4899,
        generic: 2,
        colors: &[Color::G, Color::G, Color::G],
        modifier: Some((3, 3)),
        set: ObjectSet::ControllerCreatures,
        keywords: &[KeywordAbility::Trample],
    },
    Witness {
        raw: include_str!("../../../tests/fixtures/49c997d5-7b41-4faf-b898-58db19055671.json"),
        oracle: "49c997d5-7b41-4faf-b898-58db19055671",
        name: "Stampede",
        record: "bfd06146f6b5ad23658bfda625f631602ca34572a007fda843769c072e7b214f",
        source: "9c24bc3409608752d675aea9280ea169d704d6e39c3f471592ea1a25a107fd2b",
        ast: "359632fc2b6f347278e200fa206e16643b65d3a0ac23079dc12dd8e58b418d0a",
        binding: "3c5800c2d4f440fdaca6eb2b2e4fcb787317cdd5c3a02bbcda82e6c957927b71",
        index: 11086,
        generic: 1,
        colors: &[Color::G, Color::G],
        modifier: Some((1, 0)),
        set: ObjectSet::AttackingCreatures,
        keywords: &[KeywordAbility::Trample],
    },
    Witness {
        raw: include_str!("../../../tests/fixtures/531f78d5-5004-4b02-99c7-b390cb342fd9.json"),
        oracle: "531f78d5-5004-4b02-99c7-b390cb342fd9",
        name: "Make a Stand",
        record: "a8e96396f441839739599d015c5680342b3dd2016a5db44c4de8b7b72df1b5de",
        source: "823dd63b7af6134fb3663304fd111a3725718c9156c94bc15b16d0422a16e376",
        ast: "ef35e3202769099abd6a8bfe83007aa28a73b476d548677914aaba9525692b16",
        binding: "c9a8ff2898e3861ddb194b98fa480e6e0c7dd80b99136eb51799bbf2fcbc3664",
        index: 12502,
        generic: 2,
        colors: &[Color::W],
        modifier: Some((1, 0)),
        set: ObjectSet::ControllerCreatures,
        keywords: &[KeywordAbility::Indestructible],
    },
    Witness {
        raw: include_str!("../../../tests/fixtures/91e33674-b13a-41de-8998-4b57016ae42f.json"),
        oracle: "91e33674-b13a-41de-8998-4b57016ae42f",
        name: "Burst of Speed",
        record: "ce5cc00cb4dfabc838dc0311adbad51906af0d359249c24ebcabcbd7766f93a3",
        source: "e3f4027606941ae30bdf7f626ab78f4a6285a23da95b7431249eb47d8ba68f69",
        ast: "033f772bdc4c3e69d98a81bf4a899e62668b687a3eaafbd121d1e8e2bb41327d",
        binding: "1e9214c6405598d27e8fb88339b2fb10837e2975dd5cac179d15f3fa9e4ce1cd",
        index: 21965,
        generic: 0,
        colors: &[Color::R],
        modifier: None,
        set: ObjectSet::ControllerCreatures,
        keywords: &[KeywordAbility::Haste],
    },
];
fn lowered(raw: &str) -> Box<Definition> {
    let result = compile(HEADER, raw);
    let CompileResult::Lowered { definition } = result else {
        panic!("expected set family: {result:?}")
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
#[test]
fn creature_set_keyword_five_exact_sources_costs_order_and_closure() {
    for w in WITNESSES {
        assert_eq!(sha256(w.raw.trim_end().as_bytes()), w.record);
        let d = lowered(w.raw);
        assert_eq!(d.name, w.name);
        assert_eq!(d.oracle_id, w.oracle);
        assert_eq!(d.provenance.source_sha256, w.source);
        assert_eq!(d.provenance.ast_sha256, w.ast);
        assert_eq!(d.provenance.binding_sha256, w.binding);
        assert_eq!(d.provenance.source_index, w.index);
        assert_eq!(
            d.characteristics.mana_cost.generic,
            Quantity::from(w.generic)
        );
        assert_eq!(d.characteristics.mana_cost.colored, w.colors);
        let expected_kind = if matches!(w.name, "Titanic Ultimatum" | "Overrun" | "Burst of Speed")
        {
            CardKind::Sorcery
        } else {
            CardKind::Instant
        };
        assert_eq!(d.characteristics.kind, expected_kind);
        let p = d.spell.as_ref().unwrap();
        assert!(p.targets.is_empty() && p.modes.is_empty() && p.additional_sacrifice.is_none());
        assert_eq!(p.functioning_zone, FunctioningZone::Stack);
        assert_eq!(p.resolution, SpellResolution::Instructions);
        assert_eq!(
            p.timing,
            if expected_kind == CardKind::Instant {
                SpellTiming::Instant
            } else {
                SpellTiming::MainPhase
            }
        );
        assert_eq!(
            p.instructions,
            vec![Instruction::GrantKeywordsSet {
                set: w.set.clone(),
                modifier: w.modifier.map(|(power, toughness)| CreatureStatModifier {
                    power: power.into(),
                    toughness: toughness.into(),
                }),
                keywords: w.keywords.to_vec(),
                duration: Duration::UntilEndOfTurn,
                source_block: 0,
            }]
        );
        let desc = temporary_creature_set_keyword_program(p).unwrap();
        assert_eq!(*desc.set, w.set);
        assert_eq!(desc.keywords, w.keywords);
        let values = w
            .modifier
            .map(|(p, t)| (Quantity::from(p), Quantity::from(t)));
        assert_eq!(desc.modifier, values.as_ref().map(|(p, t)| (p, t)));
        assert!(temporary_creature_keyword_program(p).is_none());
        let mut caps = BTreeSet::from([
            Capability::NormalLayoutV1,
            Capability::BoardWipesV1,
            Capability::TemporaryCreatureSetKeywordProgramV1,
            if expected_kind == CardKind::Instant {
                Capability::CastInstantV1
            } else {
                Capability::CastSorceryV1
            },
        ]);
        caps.extend(w.keywords.iter().map(|k| match k {
            KeywordAbility::FirstStrike => Capability::FirstStrikeV1,
            KeywordAbility::Trample => Capability::TrampleV1,
            KeywordAbility::Lifelink => Capability::LifelinkV1,
            KeywordAbility::Indestructible => Capability::IndestructibleV1,
            KeywordAbility::Haste => Capability::HasteV1,
            _ => unreachable!(),
        }));
        assert_eq!(d.capabilities, caps);
        assert_eq!(d.capabilities, d.requirements());
        assert!(
            d.keywords.is_empty()
                && d.static_abilities.is_empty()
                && d.triggered.is_empty()
                && d.activated.is_empty()
                && d.mana_abilities.is_empty()
        );
    }
}
#[test]
fn creature_set_keyword_whole_heroic_intervention_uses_distinct_permanent_family() {
    let raw = include_str!("../../../tests/fixtures/24882fa2-3fe9-4c1b-aa3d-0e6488b9db27.json");
    assert_eq!(
        sha256(raw.trim_end().as_bytes()),
        "d3f0636f6881fb3a644f9a76bcf4c3cc03af1224d71cfe1e83b693fb4158369b"
    );
    let r: Value = serde_json::from_str(raw).unwrap();
    assert_eq!(r["source"]["name"], "Heroic Intervention");
    assert_eq!(r["source"]["mana_cost"], "{1}{G}");
    assert_eq!(
        r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["what"],
        json!({"ref":"each","filter":{"cls":"permanent","control":"you"}})
    );
    let d = lowered(raw);
    let p = d.spell.as_ref().unwrap();
    assert!(temporary_creature_set_keyword_program(p).is_none());
    let desc = temporary_permanent_set_keyword_program(p).unwrap();
    assert_eq!(*desc.set, ObjectSet::ControllerPermanents);
    assert_eq!(
        desc.keywords,
        [KeywordAbility::Hexproof, KeywordAbility::Indestructible]
    );
    assert!(desc.modifier.is_none());
}
#[test]
fn creature_set_keyword_complete_selector_extras_reject_per_occurrence() {
    for w in WITNESSES {
        let r: Value = serde_json::from_str(w.raw).unwrap();
        for step in 0..r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"]
            .as_array()
            .unwrap()
            .len()
        {
            for variant in 0..17 {
                let raw = changed_ability(w.raw, |a| {
                    let what = &mut a["effects"][0]["steps"][step]["what"];
                    match variant {
                        0 => what["count"] = json!({"amount":"fixed","value":1}),
                        1 => what["selectionId"] = "line:0:0".into(),
                        2 => what["upTo"] = true.into(),
                        3 => what["condition"] = json!({"condition":"you-control-creature"}),
                        4 => what["filter"]["owner"] = "you".into(),
                        5 => what["filter"]["zone"] = json!({"zone":"battlefield"}),
                        6 => what["filter"]["colors"] = json!(["green"]),
                        7 => what["filter"]["nonTypes"] = json!(["artifact"]),
                        8 => what["filter"]["cls"] = "permanent".into(),
                        9 => what["filter"]["control"] = Value::Null,
                        10 => what["filter"]["nonToken"] = true.into(),
                        11 => what["filter"]["status"] = json!(["blocking"]),
                        12 => what["filter"]["withAbility"] = json!(["flying"]),
                        13 => what["ref"] = "target".into(),
                        14 => what["ref"] = "filter".into(),
                        15 => what["filter"]["types"] = json!(["creature", "artifact"]),
                        _ => what["filter"]["types"] = json!(["permanent"]),
                    }
                });
                assert!(
                    matches!(compile(HEADER, &raw), CompileResult::Unsupported { .. }),
                    "{} step{step} selector{variant}",
                    w.name
                );
            }
        }
    }
}
#[test]
fn creature_set_keyword_program_boundaries_and_separated_sentences_reject() {
    for w in WITNESSES {
        for variant in 0..18 {
            let raw = changed_ability(w.raw, |a| {
                let last = a["effects"][0]["steps"].as_array().unwrap().len() - 1;
                match variant {
                    0=>a["effects"][0]["steps"][last]["abilities"]=json!([]),
                    1=>a["effects"][0]["steps"][last]["abilities"][0]["keyword"]["keyword"]="vigilance".into(),
                    2=>a["effects"][0]["steps"][last]["abilities"][0]["keyword"]["cost"]=json!([]),
                    3=>a["effects"][0]["steps"][last]["duration"]["duration"]="forever".into(),
                    4=>a["effects"][0]["steps"][last]["duration"]["condition"]=json!({"condition":"you-control-creature"}),
                    5=>a["effects"][0]["steps"][last]["condition"]=json!({"condition":"you-control-creature"}),
                    6=>a["effects"][0]["bindings"]=json!([{"variable":"x","value":{"amount":"count","of":{"types":["creature"],"zone":{"zone":"battlefield"}}}}]),
                    7=>a["condition"]=json!({"condition":"you-control-creature"}),8=>a["costs"]=json!([]),
                    9=>{let i=a["effects"][0]["steps"][last]["abilities"][0].clone();a["effects"][0]["steps"][last]["abilities"].as_array_mut().unwrap().push(i);},
                    10=>{let i=a["effects"][0]["steps"][last].clone();a["effects"][0]["steps"].as_array_mut().unwrap().push(i);},
                    11=>a["effects"][0]["steps"].as_array_mut().unwrap().push(json!({"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1}})),
                    12=>{a["kind"]="activated".into();a["costs"]=json!([{"cost":"tap"}]);},
                    13=>{a["kind"]="triggered".into();a["trigger"]=json!({"trigger":"enters","what":{"ref":"this","noun":"creature"}});},
                    14=>{let effects=a["effects"].clone();a["effects"]=json!([{"steps":[{"effect":"modal","count":{"exactly":1},"options":[{"effects":effects.clone()},{"effects":effects}]}]}]);},
                    15=>a["effects"][0]["steps"][last]["abilities"][0]["keyword"]["keyword"]="first_strike".into(),
                    16=>a["effects"][0]["steps"][last]["abilities"][0]["keyword"]["filter"]=Value::Null,
                    _=>a["effects"][0]["steps"][last]["what"]["filter"]=json!({"types":["creature"]}),
                }
            });
            assert!(
                matches!(compile(HEADER, &raw), CompileResult::Unsupported { .. }),
                "{} program{variant}",
                w.name
            );
        }
        let additional = tests::changed_source(w.raw, |r| {
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
            matches!(compile(HEADER,&additional),CompileResult::Unsupported {path,..} if path=="definition"),
            "{} additional cost",
            w.name
        );
        if w.modifier.is_some() {
            let split = changed_ability(w.raw, |a| {
                let steps = a["effects"][0]["steps"].clone();
                a["effects"] = json!([{"steps":[steps[0].clone()]},{"steps":[steps[1].clone()]}]);
            });
            // CR 611.2c/613.6: distinct sentences cannot be coalesced just
            // because they happen to lower to adjacent operations in block 0.
            assert!(
                matches!(compile(HEADER, &split), CompileResult::Unsupported { .. }),
                "{} separated sentences",
                w.name
            );
            let separate = tests::changed_source(w.raw, |r| {
                let steps = r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"].clone();
                let abilities = json!([{"kind":"spell","effects":[{"steps":[steps[0].clone()]}]},
                    {"kind":"spell","effects":[{"steps":[steps[1].clone()]}]}]);
                r["faces"][0]["result"]["abilities"] = abilities.clone();
                r["source"]["oracle_text"] =
                    "Synthetic first block.\nSynthetic second block.".into();
                r["faces"][0]["result"]["lines"] = json!([
                    {"text":"Synthetic first block.","ok":true,"ability":abilities[0]},
                    {"text":"Synthetic second block.","ok":true,"ability":abilities[1]}]);
            });
            assert!(
                matches!(
                    compile(HEADER, &separate),
                    CompileResult::Unsupported { .. }
                ),
                "{} separate ability blocks",
                w.name
            );
            for variant in 0..5 {
                let raw = changed_ability(w.raw, |a| {
                    let steps = a["effects"][0]["steps"].as_array_mut().unwrap();
                    match variant {
                        0 => {
                            steps.pop();
                        }
                        1 => steps.reverse(),
                        2 => {
                            steps[0]["what"]["filter"] =
                                json!({"types":["creature"],"status":["attacking"]})
                        }
                        3 => steps[0]["power"]["amount"] = json!({"amount":"x"}),
                        _ => steps[0]["duration"]["duration"] = "forever".into(),
                    }
                });
                // The old attacking bare pump remains supported; Stampede also already uses that selector.
                if matches!(variant, 0 | 2) && w.set == ObjectSet::AttackingCreatures {
                    continue;
                }
                assert!(
                    matches!(compile(HEADER, &raw), CompileResult::Unsupported { .. }),
                    "{} compound{variant}",
                    w.name
                );
            }
        }
    }
}

#[test]
fn creature_set_keyword_typed_sentence_consumption_and_signed_parameters() {
    for w in WITNESSES {
        // Synthetic typed controls derive from frozen sources; they add no
        // source-card facts. All/each lower to the same closed captured set.
        let equivalent = changed_ability(w.raw, |a| {
            for step in a["effects"][0]["steps"].as_array_mut().unwrap() {
                step["what"]["ref"] = if step["what"]["ref"] == "all" {
                    "each"
                } else {
                    "all"
                }
                .into();
            }
        });
        assert_eq!(lowered(&equivalent).spell, lowered(w.raw).spell);
        if w.modifier.is_none() {
            continue;
        }
        for (power, toughness) in [(0, 0), (-2, 3), (i64::from(u32::MAX), -i64::from(u32::MAX))] {
            let raw = changed_ability(w.raw, |a| {
                for (field, value) in [("power", power), ("toughness", toughness)] {
                    a["effects"][0]["steps"][0][field] = json!({
                        "sign":if value<0 {-1} else {1},
                        "amount":{"amount":"fixed","value":value.unsigned_abs()}
                    });
                }
            });
            let d = lowered(&raw);
            let desc = temporary_creature_set_keyword_program(d.spell.as_ref().unwrap()).unwrap();
            assert_eq!(
                desc.modifier,
                Some((&Quantity::from(power), &Quantity::from(toughness)))
            );
            assert_eq!(d.spell.as_ref().unwrap().instructions.len(), 1);
        }
        for before in [false, true] {
            let raw = changed_ability(w.raw, |a| {
                let extra = json!({"steps":[{"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1}}]});
                let effects = a["effects"].as_array_mut().unwrap();
                if before {
                    effects.insert(0, extra);
                } else {
                    effects.push(extra);
                }
            });
            assert!(
                matches!(compile(HEADER, &raw), CompileResult::Unsupported { .. }),
                "{} must consume the whole effects array; before={before}",
                w.name
            );
        }
    }
}
