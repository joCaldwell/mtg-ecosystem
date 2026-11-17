//! Complete Heroic Intervention archive witness; extra vectors are synthetic.
//! All new native tests UNRUN; the parent owns qualification and admission.
use super::*;
use serde_json::{Value, json};
const HEADER: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
const RAW: &str = include_str!("../../../tests/fixtures/24882fa2-3fe9-4c1b-aa3d-0e6488b9db27.json");
fn lowered(raw: &str) -> Box<Definition> {
    let result = compile(HEADER, raw);
    let CompileResult::Lowered { definition } = result else {
        panic!("expected complete permanent family: {result:?}")
    };
    definition.validate().unwrap();
    definition
}
fn changed(change: impl FnOnce(&mut Value)) -> String {
    tests::changed_source(RAW, |r| {
        let a = &mut r["faces"][0]["result"]["abilities"][0];
        change(a);
        r["faces"][0]["result"]["lines"][0]["ability"] = a.clone();
    })
}
fn unsupported(raw: &str) {
    assert!(
        matches!(compile(HEADER, raw), CompileResult::Unsupported { .. }),
        "must reject complete synthetic source: {:?}",
        compile(HEADER, raw)
    );
}
#[test]
fn permanent_set_keyword_exact_heroic_source_provenance_cost_and_complete_closure() {
    assert_eq!(
        sha256(RAW.trim_end().as_bytes()),
        "d3f0636f6881fb3a644f9a76bcf4c3cc03af1224d71cfe1e83b693fb4158369b"
    );
    let r: Value = serde_json::from_str(RAW).unwrap();
    assert_eq!(r["source"]["layout"], "normal");
    assert_eq!(r["source"]["legalities"]["commander"], "legal");
    assert_eq!(
        r["source"]["oracle_text"],
        "Permanents you control gain hexproof and indestructible until end of turn."
    );
    assert_eq!(
        r["faces"][0]["result"]["abilities"],
        json!([{"kind":"spell","effects":[{"steps":[{
            "effect":"gain-abilities","what":{"ref":"each","filter":{"cls":"permanent","control":"you"}},
            "abilities":[{"gained":"keyword","keyword":{"keyword":"hexproof"}},{"gained":"keyword","keyword":{"keyword":"indestructible"}}],
            "duration":{"duration":"end-of-turn"}
        }]}]}])
    );
    let d = lowered(RAW);
    assert_eq!(d.name, "Heroic Intervention");
    assert_eq!(d.oracle_id, "24882fa2-3fe9-4c1b-aa3d-0e6488b9db27");
    assert_eq!(d.provenance.source_index, 5489);
    assert_eq!(
        d.provenance.source_sha256,
        "c3e5efdcfd9bba5274c2ee23ce69db79540c43e4a6dd690b8d5e26010ed55377"
    );
    assert_eq!(
        d.provenance.ast_sha256,
        "0bde69f5f551f4e7de9b657198715bd62da91040fe9970c0afe74e8afa5ab7f0"
    );
    assert_eq!(
        d.provenance.binding_sha256,
        "2f6d3ffaf9781c4e848bf6273a92ee6fc0124f35100804d8898da68cc8831e64"
    );
    assert_eq!(d.characteristics.kind, CardKind::Instant);
    assert_eq!(d.characteristics.mana_cost.generic, Quantity::from(1));
    assert_eq!(d.characteristics.mana_cost.colored, [Color::G]);
    let p = d.spell.as_ref().unwrap();
    assert!(p.targets.is_empty() && p.modes.is_empty() && p.additional_sacrifice.is_none());
    assert_eq!(p.timing, SpellTiming::Instant);
    assert_eq!(p.functioning_zone, FunctioningZone::Stack);
    assert_eq!(p.resolution, SpellResolution::Instructions);
    assert_eq!(
        p.instructions,
        vec![Instruction::GrantKeywordsSet {
            set: ObjectSet::ControllerPermanents,
            modifier: None,
            keywords: vec![KeywordAbility::Hexproof, KeywordAbility::Indestructible],
            duration: Duration::UntilEndOfTurn,
            source_block: 0,
        }]
    );
    assert_eq!(
        temporary_permanent_set_keyword_program(p).unwrap().keywords,
        [KeywordAbility::Hexproof, KeywordAbility::Indestructible]
    );
    assert!(temporary_creature_set_keyword_program(p).is_none());
    assert_eq!(
        d.capabilities,
        BTreeSet::from([
            Capability::NormalLayoutV1,
            Capability::CastInstantV1,
            Capability::BoardWipesV1,
            Capability::TemporaryPermanentSetKeywordProgramV1,
            Capability::AttachmentsV1,
            Capability::IndestructibleV1
        ])
    );
    assert_eq!(d.capabilities, d.requirements());
    assert!(
        d.keywords.is_empty()
            && d.static_abilities.is_empty()
            && d.activated.is_empty()
            && d.triggered.is_empty()
            && d.mana_abilities.is_empty()
    );
}
#[test]
fn permanent_set_keyword_synthetic_reviewed_vectors_and_all_selector() {
    // These typed AST controls are not source-card or admission claims.
    let all = [
        "flying",
        "deathtouch",
        "indestructible",
        "lifelink",
        "hexproof",
        "haste",
        "trample",
        "first strike",
    ];
    let mut vectors: Vec<Vec<&str>> = all.iter().map(|k| vec![*k]).collect();
    vectors.push(all.to_vec());
    vectors.push(all.into_iter().rev().collect());
    vectors.push(vec!["indestructible", "hexproof"]);
    for vector in vectors {
        let raw = changed(|a| {
            a["effects"][0]["steps"][0]["abilities"] = json!(
                vector
                    .iter()
                    .map(|keyword| json!({"gained":"keyword","keyword":{"keyword":keyword}}))
                    .collect::<Vec<_>>()
            );
            a["effects"][0]["steps"][0]["what"]["ref"] = "all".into();
        });
        let d = lowered(&raw);
        let desc = temporary_permanent_set_keyword_program(d.spell.as_ref().unwrap()).unwrap();
        assert_eq!(
            desc.keywords
                .iter()
                .map(|k| serde_json::to_value(k).unwrap())
                .collect::<Vec<_>>(),
            vector.iter().map(|k| json!(k)).collect::<Vec<_>>()
        );
        assert!(desc.modifier.is_none());
        assert!(
            !d.capabilities
                .contains(&Capability::TemporaryCreatureSetKeywordProgramV1)
        );
        assert!(!d.capabilities.contains(&Capability::CreatureTargetV1));
    }
}
#[test]
fn permanent_set_keyword_complete_malformed_selectors_and_keywords_reject() {
    for variant in 0..20 {
        unsupported(&changed(|a| {
            let what = &mut a["effects"][0]["steps"][0]["what"];
            match variant {
                0 => what["filter"]["control"] = "opponent".into(),
                1 => {
                    what["filter"].as_object_mut().unwrap().remove("control");
                }
                2 => what["filter"]["control"] = Value::Null,
                3 => what["filter"]["types"] = json!(["creature"]),
                4 => what["filter"]["nonTypes"] = json!(["land"]),
                5 => what["filter"]["status"] = json!(["attacking"]),
                6 => what["filter"]["owner"] = "you".into(),
                7 => what["filter"]["zone"] = json!({"zone":"battlefield"}),
                8 => what["filter"]["colors"] = json!(["green"]),
                9 => what["filter"]["withAbility"] = json!(["hexproof"]),
                10 => what["filter"]["cls"] = "card".into(),
                11 => {
                    what["filter"].as_object_mut().unwrap().remove("cls");
                }
                12 => what["count"] = json!({"amount":"fixed","value":1}),
                13 => what["selectionId"] = "line:0:0".into(),
                14 => what["upTo"] = true.into(),
                15 => what["ref"] = "target".into(),
                16 => what["ref"] = "filter".into(),
                17 => what["filter"] = json!({"types":["permanent"],"control":"you"}),
                18 => what["filter"]["nonToken"] = true.into(),
                _ => what["filter"]["cls"] = Value::Null,
            }
        }));
    }
    for variant in 0..10 {
        unsupported(&changed(|a| {
            let step = &mut a["effects"][0]["steps"][0];
            match variant {
                0 => step["abilities"] = json!([]),
                1 => step["abilities"][1] = step["abilities"][0].clone(),
                2 => step["abilities"][0]["keyword"]["keyword"] = "defender".into(),
                3 => step["abilities"][0]["keyword"]["keyword"] = "first_strike".into(),
                4 => step["abilities"][0]["keyword"]["cost"] = json!([]),
                5 => step["abilities"][0]["keyword"]["filter"] = Value::Null,
                6 => step["abilities"][0]["gained"] = "static".into(),
                7 => step["duration"]["duration"] = "forever".into(),
                8 => step["duration"]["condition"] = json!({"condition":"you-control-creature"}),
                _ => step["condition"] = json!({"condition":"you-control-creature"}),
            }
        }));
    }
}
#[test]
fn permanent_set_keyword_whole_source_cost_mode_trigger_and_other_opcode_fences() {
    for variant in 0..13 {
        unsupported(&changed(|a| {
            match variant {
            0=>a["condition"]=json!({"condition":"you-control-creature"}),
            1=>a["costs"]=json!([]),
            2=>a["effects"][0]["bindings"]=json!([{"variable":"x","value":{"amount":"count","of":{"types":["creature"],"zone":{"zone":"battlefield"}}}}]),
            3=>{let step=a["effects"][0]["steps"][0].clone();a["effects"][0]["steps"].as_array_mut().unwrap().push(step);},
            4=>{let effects=a["effects"].clone();a["effects"]=json!([{"steps":[{"effect":"modal","count":{"exactly":1},"options":[{"effects":effects.clone()},{"effects":effects}]}]}]);},
            5=>{a["kind"]="activated".into();a["costs"]=json!([{"cost":"tap"}]);},
            6=>{a["kind"]="triggered".into();a["trigger"]=json!({"trigger":"enters","what":{"ref":"this","noun":"creature"}});},
            7=>a["effects"][0]["steps"].as_array_mut().unwrap().push(json!({"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1}})),
            8=>{let extra=json!({"steps":[{"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1}}]});a["effects"].as_array_mut().unwrap().insert(0,extra);},
            9=>a["effects"][0]["steps"][0]=json!({"effect":"destroy","what":{"ref":"each","filter":{"cls":"permanent","control":"you"}}}),
            10=>a["effects"][0]["steps"][0]=json!({"effect":"exile","what":{"ref":"each","filter":{"cls":"permanent","control":"you"}}}),
            11=>a["effects"][0]["steps"][0]=json!({"effect":"return-to-hand","what":{"ref":"each","filter":{"cls":"permanent","control":"you"}},"to":"owner"}),
            _=>a["effects"][0]["steps"][0]=json!({"effect":"damage","source":{"ref":"self"},"amount":{"amount":"fixed","value":1},"to":{"to":"object","ref":{"ref":"each","filter":{"cls":"permanent","control":"you"}}}}),
        }
        }));
    }
    // Every permanent P/T modifier rejects, including a nominal +0/+0.
    for (power, toughness) in [(0, 0), (-2, 3), (1, -1)] {
        for form in 0..4 {
            unsupported(&changed(|a| {
                let pump = json!({"effect":"pump","what":{"ref":"each","filter":{"cls":"permanent","control":"you"}},
                    "power":{"sign":if power<0 {-1}else{1},"amount":{"amount":"fixed","value":i64::abs(power)}},
                    "toughness":{"sign":if toughness<0 {-1}else{1},"amount":{"amount":"fixed","value":i64::abs(toughness)}},
                    "duration":{"duration":"end-of-turn"}});
                let gain = a["effects"][0]["steps"][0].clone();
                a["effects"] = match form {
                    0 => json!([{"steps":[pump]}]),
                    1 => json!([{"steps":[pump,gain]}]),
                    2 => json!([{"steps":[gain,pump]}]),
                    _ => json!([{"steps":[pump]},{"steps":[gain]}]),
                };
            }));
        }
    }
    let additional = tests::changed_source(RAW, |r| {
        let text = "As an additional cost to cast this spell, sacrifice an artifact.";
        let cost = json!({"kind":"additional-cost","costs":[{"cost":"sacrifice","what":{"ref":"filter","filter":{"types":["artifact"]},"count":{"amount":"fixed","value":1}}}]});
        r["source"]["oracle_text"] =
            format!("{text}\n{}", r["source"]["oracle_text"].as_str().unwrap()).into();
        r["faces"][0]["result"]["abilities"]
            .as_array_mut()
            .unwrap()
            .insert(0, cost.clone());
        r["faces"][0]["result"]["lines"]
            .as_array_mut()
            .unwrap()
            .insert(0, json!({"text":text,"ok":true,"ability":cost}));
    });
    unsupported(&additional);
    let later_block = tests::changed_source(RAW, |r| {
        let gain = r["faces"][0]["result"]["abilities"][0].clone();
        let draw = json!({"kind":"spell","effects":[{"steps":[{"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1}}]}]});
        r["source"]["oracle_text"] = "Synthetic draw.\nSynthetic grant.".into();
        r["faces"][0]["result"]["abilities"] = json!([draw, gain]);
        r["faces"][0]["result"]["lines"] = json!([
            {"text":"Synthetic draw.","ok":true,"ability":draw},
            {"text":"Synthetic grant.","ok":true,"ability":gain}]);
    });
    unsupported(&later_block);
}
