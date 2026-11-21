//! Frozen Tamiyo source and synthetic complete-source controls; native UNRUN.
use super::*;
use serde_json::{Value, json};
const HEADER: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
const RAW: &str = include_str!("../../../tests/fixtures/bb2b324c-970a-4920-884e-c92ba49669f0.json");
fn lowered(raw: &str) -> Box<Definition> {
    let result = compile(HEADER, raw);
    let CompileResult::Lowered { definition } = result else {
        panic!("complete lowering required: {result:?}")
    };
    definition.validate().unwrap();
    definition
}
fn changed(change: impl FnOnce(&mut Value)) -> String {
    tests::changed_source(RAW, |r| {
        let a = &mut r["faces"][0]["result"]["abilities"][0];
        change(a);
        r["faces"][0]["result"]["lines"][0]["ability"] = a.clone();
        if r["faces"][0]["result"]["abilities"][0]["effects"]
            .as_array()
            .unwrap()
            .len()
            == 1
        {
            r["binding"]["references"]
                .as_array_mut()
                .unwrap()
                .retain(|v| !v["path"].as_str().unwrap().contains("/effects/1/"));
            r["binding"]["nodes"]
                .as_array_mut()
                .unwrap()
                .retain(|v| !v["path"].as_str().unwrap().contains("/effects/1/"));
            r["binding"]["counts"] = json!({"obligations":2,"bound":2,"unresolved":0});
        }
    })
}
fn rejects(raw: &str) {
    assert!(
        !matches!(compile(HEADER, raw), CompileResult::Lowered { .. }),
        "unexpected lowering: {raw}"
    );
}
#[test]
fn temporary_permanent_keyword_compiler_exact_tamiyo_archive_source_and_provenance() {
    assert_eq!(
        sha256(RAW.trim_end().as_bytes()),
        "914e134284083b1540c71d66f115424264560de04ee18674b39f0270daa836cd"
    );
    let r: Value = serde_json::from_str(RAW).unwrap();
    assert_eq!(r["source"]["layout"], "normal");
    assert_eq!(r["source"]["type_line"], "Instant");
    assert_eq!(r["source"]["legalities"]["commander"], "legal");
    assert_eq!(r["source"]["mana_cost"], "{G}");
    assert_eq!(
        r["binding"]["counts"],
        json!({"obligations":3,"bound":3,"unresolved":0})
    );
    assert_eq!(r["binding"]["diagnostics"], json!([]));
    assert_eq!(
        r["faces"][0]["result"]["lines"][0]["text"],
        "Target permanent you control gains hexproof and indestructible until end of turn. You gain 2 life."
    );
    assert!(
        r["source"]["oracle_text"].as_str().unwrap().starts_with(
            r["faces"][0]["result"]["lines"][0]["text"]
                .as_str()
                .unwrap()
        )
    );
    assert_eq!(
        r["faces"][0]["result"]["abilities"],
        json!([{"kind":"spell","effects":[
            {"steps":[{"effect":"gain-abilities","what":{"ref":"target","filter":{"cls":"permanent","control":"you"},"count":{"amount":"fixed","value":1},"selectionId":"line:0:0"},
                "abilities":[{"gained":"keyword","keyword":{"keyword":"hexproof"}},{"gained":"keyword","keyword":{"keyword":"indestructible"}}],"duration":{"duration":"end-of-turn"}}]},
            {"steps":[{"effect":"gain-life","who":{"player":"you"},"amount":{"amount":"fixed","value":2}}]}
        ]}])
    );
    let d = lowered(RAW);
    assert_eq!(d.name, "Tamiyo's Safekeeping");
    assert_eq!(d.oracle_id, "bb2b324c-970a-4920-884e-c92ba49669f0");
    assert_eq!(d.provenance.source_index, 28166);
    assert_eq!(
        d.provenance.source_sha256,
        "48e40697b4ba35ab71ae951b900f756eeeb6b0ab5d3a9e4ca6434326474e0a78"
    );
    assert_eq!(
        d.provenance.ast_sha256,
        "050500699479157305337569bca1c1d753142907cc8ba254403d7ecc75053b86"
    );
    assert_eq!(
        d.provenance.binding_sha256,
        "2ecd1544ded73bedd155b56966a2e3d3886b3c83288e58f5081a5c0ae85a5ec6"
    );
    assert_eq!(d.characteristics.mana_cost.colored, [Color::G]);
    assert_eq!(d.characteristics.mana_cost.generic, Quantity::zero());
    let p = d.spell.as_ref().unwrap();
    assert_eq!(
        p.targets,
        [TargetSpec {
            slot: 0,
            selection_id: "line:0:0".into(),
            chooser: Chooser::Controller,
            stage: EvaluationStage::Announcement,
            domain: TargetDomain::ControlledBattlefieldPermanent,
            filter: Some(PermanentFilter {
                power: None,
                types: vec![],
                non_types: vec![],
                non_colors: vec![],
                opponent: false,
                tapped: false
            })
        }]
    );
    assert_eq!(
        p.instructions,
        [
            Instruction::GrantKeywords {
                target_slot: 0,
                modifier: None,
                keywords: vec![KeywordAbility::Hexproof, KeywordAbility::Indestructible],
                duration: Duration::UntilEndOfTurn,
                source_block: 0
            },
            Instruction::act(
                Verb::GainLife {
                    player: PlayerExpr::you(Context::Spell),
                    amount: Amount::fixed(2)
                },
                0
            )
        ]
    );
    let descriptor = temporary_targeted_keyword_program(p).unwrap();
    assert!(descriptor.modifier.is_none());
    assert_eq!(descriptor.controller_life, Some(&Quantity::from(2)));
    assert!(temporary_creature_keyword_program(p).is_none());
    assert!(compound_indestructible_parameters(p).is_none());
    assert_eq!(
        d.capabilities,
        BTreeSet::from([
            Capability::NormalLayoutV1,
            Capability::CastInstantV1,
            Capability::TemporaryPermanentKeywordProgramV1,
            Capability::PermanentRemovalV1,
            Capability::AttachmentsV1,
            Capability::IndestructibleV1,
            Capability::LifeGainV1
        ])
    );
}
#[test]
fn temporary_permanent_keyword_compiler_synthetic_full_sources_parameters_and_identity() {
    let keywords = [
        "flying",
        "deathtouch",
        "indestructible",
        "lifelink",
        "hexproof",
        "haste",
        "trample",
        "first strike",
    ];
    let vectors: Vec<Vec<&str>> = keywords
        .iter()
        .map(|k| vec![*k])
        .chain([keywords.to_vec(), keywords.into_iter().rev().collect()])
        .collect();
    for vector in vectors {
        for life in [None, Some(0), Some(2), Some(u32::MAX)] {
            let raw = changed(|a| {
                a["effects"][0]["steps"][0]["abilities"] = json!(
                    vector
                        .iter()
                        .map(|k| json!({"gained":"keyword","keyword":{"keyword":k}}))
                        .collect::<Vec<_>>()
                );
                if let Some(value) = life {
                    a["effects"][1]["steps"][0]["amount"]["value"] = value.into();
                } else {
                    a["effects"].as_array_mut().unwrap().pop();
                }
            });
            let d = lowered(&raw);
            let descriptor =
                temporary_permanent_keyword_program(d.spell.as_ref().unwrap()).unwrap();
            assert_eq!(
                descriptor.controller_life.cloned(),
                life.map(|v| Quantity::from(i64::from(v)))
            );
            assert_eq!(
                descriptor
                    .keywords
                    .iter()
                    .map(|k| serde_json::to_value(k).unwrap())
                    .collect::<Vec<_>>(),
                vector.iter().map(|k| json!(k)).collect::<Vec<_>>()
            );
        }
    }
    let id = "00000000-0000-4000-8000-000000000071";
    let mut r: Value = serde_json::from_str(RAW).unwrap();
    let text = serde_json::to_string(&r)
        .unwrap()
        .replace("bb2b324c-970a-4920-884e-c92ba49669f0", id)
        .replace("Tamiyo's Safekeeping", "Synthetic Permanent Protection");
    r = serde_json::from_str(&text).unwrap();
    let raw = tests::changed_source(&r.to_string(), |r| {
        r["source"]["type_line"] = "Sorcery".into();
    });
    let d = lowered(&raw);
    assert_eq!(d.oracle_id, id);
    assert_eq!(d.name, "Synthetic Permanent Protection");
    assert_eq!(d.spell.as_ref().unwrap().timing, SpellTiming::MainPhase);
    assert!(d.capabilities.contains(&Capability::CastSorceryV1));
}
#[test]
fn temporary_permanent_keyword_compiler_rejects_complete_selector_and_keyword_near_misses() {
    for case in 0..37 {
        let raw = changed(|a| {
            let g = &mut a["effects"][0]["steps"][0];
            match case {
                0 => g["what"]["filter"]["control"] = Value::Null,
                1 => {
                    g["what"]["filter"]
                        .as_object_mut()
                        .unwrap()
                        .remove("control");
                }
                2 => g["what"]["filter"]["control"] = "opponent".into(),
                3 => g["what"]["filter"]["control"] = "owner".into(),
                4 => g["what"]["filter"]["types"] = json!(["creature"]),
                5 => g["what"]["filter"]["nonTypes"] = json!(["land"]),
                6 => g["what"]["filter"]["status"] = json!(["tapped"]),
                7 => g["what"]["filter"]["colors"] = json!(["green"]),
                8 => g["what"]["filter"]["subtypes"] = json!(["Goblin"]),
                9 => g["what"]["filter"]["extra"] = true.into(),
                10 => g["what"]["count"]["value"] = 0.into(),
                11 => g["what"]["count"]["value"] = 2.into(),
                12 => g["what"]["selectionId"] = "".into(),
                13 => g["what"]["selectionId"] = Value::Null,
                14 => g["what"]["upTo"] = false.into(),
                15 => g["what"]["ref"] = "each".into(),
                16 => g["what"] = Value::Null,
                17 => g["duration"] = Value::Null,
                18 => g["duration"]["duration"] = "forever".into(),
                19 => g["duration"]["condition"] = Value::Null,
                20 => g["abilities"] = json!([]),
                21 => {
                    let k = g["abilities"][0].clone();
                    g["abilities"].as_array_mut().unwrap().push(k);
                }
                22 => g["abilities"][0]["keyword"]["keyword"] = "shroud".into(),
                23 => g["abilities"][0]["keyword"]["keyword"] = "first_strike".into(),
                24 => g["abilities"][0]["keyword"]["filter"] = Value::Null,
                25 => g["abilities"][0]["gained"] = "activated".into(),
                26 => g["extra"] = true.into(),
                27 => g["condition"] = Value::Null,
                28 => a["effects"][0]["bindings"] = json!([{"variable":"unused"}]),
                29 => a["effects"][1]["bindings"] = json!([{"variable":"unused"}]),
                30 => a["effects"].as_array_mut().unwrap().reverse(),
                31 => {
                    let extra = g.clone();
                    a["effects"][0]["steps"].as_array_mut().unwrap().push(extra);
                }
                32 => {
                    let extra = a["effects"][1].clone();
                    a["effects"].as_array_mut().unwrap().push(extra);
                }
                33 => {
                    let tail = a["effects"][1]["steps"][0].clone();
                    a["effects"].as_array_mut().unwrap().pop();
                    a["effects"][0]["steps"].as_array_mut().unwrap().push(tail);
                }
                34 => a["condition"] = json!({"condition":"you-control-creature"}),
                35 => {
                    a["effects"][0]["steps"][0]["what"]["filter"] =
                        json!({"types":["creature"],"control":"you"})
                }
                _ => {
                    let child = a["effects"].clone();
                    a["effects"] = json!([{"steps":[{"effect":"modal","count":{"exactly":1},"options":[{"effects":child.clone()},{"effects":child}]}]}]);
                }
            }
        });
        rejects(&raw);
    }
}
#[test]
fn temporary_permanent_keyword_compiler_rejects_explicit_null_life_recipient_qualifiers() {
    // Keep the canonical binding unchanged while rehashing the changed AST.
    // Explicit qualifiers are not an unqualified invocation-controller source.
    lowered(RAW);
    let accepted: Vec<_> = ["selectionId", "of"]
        .into_iter()
        .filter(|field| {
            let raw = changed(|a| {
                a["effects"][1]["steps"][0]["who"][*field] = Value::Null;
            });
            matches!(compile(HEADER, &raw), CompileResult::Lowered { .. })
        })
        .collect();
    assert!(
        accepted.is_empty(),
        "null qualifiers accepted: {accepted:?}"
    );
}
#[test]
fn temporary_permanent_keyword_compiler_rejects_life_and_context_near_misses() {
    for case in 0..23 {
        let raw = changed(|a| {
            let l = &mut a["effects"][1]["steps"][0];
            match case {
            0 => l["who"] = Value::Null,
            1 => l["who"]["player"] = "each-opponent".into(),
            2 => l["who"]["selectionId"] = "line:0:0".into(),
            3 => l["who"]["of"] = json!({"ref":"it"}),
            4 => l["who"]["extra"] = true.into(),
            5 => l["amount"] = Value::Null,
            6 => l["amount"]["amount"] = "x".into(),
            7 => l["amount"]["value"] = (-1).into(),
            8 => l["amount"]["extra"] = true.into(),
            9 => l["forEach"] = Value::Null,
            10 => l["forEach"] = json!({"ref":"each","filter":{"types":["creature"]}}),
            11 => l["effect"] = "lose-life".into(),
            12 => l["condition"] = Value::Null,
            13 => l["extra"] = true.into(),
            14 => a["effects"][1]["steps"] = json!([]),
            15 => { let extra = l.clone(); a["effects"][1]["steps"].as_array_mut().unwrap().push(extra); },
            16 => a["effects"][1]["steps"][0] = json!({"effect":"scry","who":{"player":"you"},"amount":{"amount":"fixed","value":1}}),
            17 => { a["kind"] = "activated".into(); a["costs"] = json!([{"cost":"tap"}]); },
            18 => { a["kind"] = "triggered".into(); a["trigger"] = json!({"trigger":"enters","what":{"ref":"self"}}); },
            19 => a["costs"] = json!([]),
            20 => a["effects"][0]["sourceBlock"] = 1.into(),
            21 => a["effects"][0]["steps"].as_array_mut().unwrap().insert(0, json!({"effect":"pump","what":{"ref":"target","filter":{"cls":"permanent","control":"you"},"count":{"amount":"fixed","value":1},"selectionId":"line:0:0"},"power":{"amount":"fixed","value":1},"toughness":{"amount":"fixed","value":1},"duration":{"duration":"end-of-turn"}})),
            _ => a["effects"][0]["steps"][0]["abilities"].as_array_mut().unwrap().push(json!({"gained":"keyword","keyword":{"keyword":"scry"}})),
        }
        });
        rejects(&raw);
    }
    for kind in ["Creature", "Artifact", "Enchantment", "Land"] {
        rejects(&tests::changed_source(RAW, |r| {
            r["source"]["type_line"] = kind.into()
        }));
    }
    rejects(&tests::changed_source(RAW, |r| {
        r["source"]["mana_cost"] = "{X}{G}".into()
    }));
}
#[test]
fn temporary_permanent_keyword_compiler_reused_selection_and_whole_source_guards() {
    let mut p = lowered(RAW).spell.clone().unwrap();
    let mut slots = BTreeMap::from([("line:0:0".into(), 0)]);
    let target = json!({"ref":"target","filter":{"cls":"permanent","control":"you"},"count":{"amount":"fixed","value":1},"selectionId":"line:0:0"});
    assert_eq!(
        bind_temporary_permanent_target(&mut p, &mut slots, &target).unwrap(),
        0
    );
    p.targets[0].filter.as_mut().unwrap().tapped = true;
    assert!(bind_temporary_permanent_target(&mut p, &mut slots, &target).is_err());
    p.targets[0].domain = TargetDomain::ControlledBattlefieldCreature;
    assert!(bind_temporary_permanent_target(&mut p, &mut slots, &target).is_err());
    for case in 0..5 {
        let raw = tests::changed_source(RAW, |r| match case {
            0 => {
                let a = r["faces"][0]["result"]["abilities"][0].clone();
                r["faces"][0]["result"]["abilities"]
                    .as_array_mut()
                    .unwrap()
                    .push(a.clone());
                r["faces"][0]["result"]["lines"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"text":"extra","ok":true,"ability":a}));
            }
            1 => {
                r["faces"][0]["result"]["lines"][0]["ability"]["effects"][0]["steps"][0]["what"]["filter"]
                    ["control"] = "opponent".into()
            }
            2 => r["binding"]["fullyBound"] = false.into(),
            3 => r["binding"]["diagnostics"] = json!([{"code":"unsupported-reference"}]),
            _ => r["faces"][0]["result"]["lines"]
                .as_array_mut()
                .unwrap()
                .push(json!({"text":"Scry 1.","ok":false})),
        });
        rejects(&raw);
    }
    let mut r: Value = serde_json::from_str(RAW).unwrap();
    r["sourceSha256"] = "0".repeat(64).into();
    rejects(&r.to_string());
}

#[test]
fn temporary_permanent_keyword_compiler_generic_zero_life_stays_rejected() {
    for raw in [
        include_str!("../../../tests/fixtures/sacred-nectar.json"),
        include_str!("../../../tests/fixtures/soul-feast.json"),
    ] {
        let r: Value = serde_json::from_str(raw).unwrap();
        for (block, a) in r["faces"][0]["result"]["abilities"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            for (sentence, effect) in a["effects"].as_array().unwrap().iter().enumerate() {
                for (step, value) in effect["steps"].as_array().unwrap().iter().enumerate() {
                    if !matches!(value["effect"].as_str(), Some("gain-life" | "lose-life")) {
                        continue;
                    }
                    let changed = tests::changed_source(raw, |r| {
                        r["faces"][0]["result"]["abilities"][block]["effects"][sentence]["steps"]
                            [step]["amount"]["value"] = 0.into();
                        r["faces"][0]["result"]["lines"][block]["ability"] =
                            r["faces"][0]["result"]["abilities"][block].clone();
                    });
                    rejects(&changed);
                }
            }
        }
    }
}
#[test]
fn temporary_permanent_keyword_compiler_rejects_rehashed_binding_near_misses() {
    for case in 0..12 {
        let raw = tests::changed_source(RAW, |r| {
            let b = &mut r["binding"];
            match case {
                0 => b["references"][0]["expression"] = json!({"player":"opponent"}),
                1 => b["references"][1]["expression"]["filter"]["control"] = "opponent".into(),
                2 => b["references"][2]["target"] = "unbound".into(),
                3 => b["references"][2]["binding"] = "unresolved".into(),
                4 => {
                    b["references"].as_array_mut().unwrap().pop();
                }
                5 => {
                    let duplicate = b["references"][0].clone();
                    b["references"].as_array_mut().unwrap().push(duplicate);
                }
                6 => b["referencesBound"] = false.into(),
                7 => b["counts"]["unresolved"] = 1.into(),
                8 => b["identity"] = "other".into(),
                9 => b["nodes"]
                    .as_array_mut()
                    .unwrap()
                    .retain(|n| n["node"] != "selection"),
                10 => b["nodes"]
                    .as_array_mut()
                    .unwrap()
                    .retain(|n| n["node"] != "runtime-player"),
                _ => b["references"][1]["path"] = "/wrong".into(),
            }
        });
        rejects(&raw);
    }
}

#[test]
fn temporary_permanent_keyword_rejects_unconsumed_conditions() {
    for sentence in 0..2 {
        for field in ["condition", "conditionKind"] {
            rejects(&changed(|a| {
                a["effects"][sentence][field] = if field == "conditionKind" {
                    json!("if")
                } else {
                    json!({"condition":"kicked","what":{"ref":"this","noun":"spell"},"comparison":{"op":"ge","value":{"amount":"fixed","value":1}}})
                };
            }));
        }
    }
}
