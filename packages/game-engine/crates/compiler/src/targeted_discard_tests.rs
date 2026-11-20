use super::*;
use crate::tests::changed_source;
use serde_json::{Value, json};

const MANIFEST: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");

fn lowered(raw: &str) -> Box<Definition> {
    match compile(MANIFEST, raw) {
        CompileResult::Lowered { definition } => definition,
        other => panic!("{other:?}"),
    }
}

fn assert_discard(p: &SpellProgram, domain: TargetDomain, selection: &str, count: i64) {
    assert_eq!(
        p.instructions,
        [Instruction::DiscardTargetPlayer {
            target_slot: 0,
            count: count.into(),
            source_block: 0,
        }]
    );
    assert_eq!(
        p.targets,
        [TargetSpec {
            slot: 0,
            selection_id: selection.into(),
            chooser: Chooser::Controller,
            stage: EvaluationStage::Announcement,
            domain,
            filter: None,
        }]
    );
    assert!(p.modes.is_empty() && p.additional_sacrifice.is_none());
}

// A one-card query has no quantity result in the frozen binder.
fn synthetic_single() -> String {
    const RECORD: &str =
        include_str!("../../../tests/fixtures/1f544a9f-c238-4858-be19-d6cd7d023dcc.json");
    changed_source(RECORD, |r| {
        let text = "Target player discards a card.";
        r["source"]["oracle_text"] = json!(text);
        let result = &mut r["faces"][0]["result"];
        let what =
            json!({"ref":"filter","filter":{"cls":"card"},"count":{"amount":"fixed","value":1}});
        let step = &mut result["abilities"][0]["effects"][0]["steps"][0];
        *step = json!({"effect":"discard","who":step["who"],"what":what});
        result["lines"][0]["text"] = json!(text);
        result["lines"][0]["ability"] = result["abilities"][0].clone();
        let graph = &mut r["binding"];
        let nodes = graph["nodes"].as_array_mut().unwrap();
        nodes.pop();
        nodes[6]["cardinality"] = json!("one");
        nodes[6]["identity"] = json!("zone-successors");
        nodes[6]["rules"] = json!(["400.7j"]);
        let field = format!("{}/what", nodes[5]["path"].as_str().unwrap());
        let query = format!(
            "card:{}{field}#query",
            nodes[0]["identity"].as_str().unwrap()
        );
        nodes[5]["operation"] = json!("discard");
        nodes[5]["inputs"]
            .as_array_mut()
            .unwrap()
            .insert(0, json!(query));
        let selection = json!({"id":query,"node":"selection","path":field,"scope":nodes[5]["scope"],"expression":what,"cardinality":"one","timing":"resolution","selection":"evaluate-reference","rules":["608.2h"]});
        nodes.insert(5, selection);
        graph["references"].as_array_mut().unwrap().push(json!({"path":field,"expression":what,"binding":"bound","target":query,"rules":["608.2h"]}));
        graph["counts"] = json!({"bound":2,"obligations":2,"unresolved":0});
    })
}

fn modal_witness(d: &Definition) {
    let root = d.spell.as_ref().unwrap();
    assert!(
        root.targets.is_empty()
            && root.instructions.is_empty()
            && root.additional_sacrifice.is_none()
    );
    let [destroy, draw, discard] = root.modes.as_slice() else {
        panic!("expected three modes")
    };
    let labels = d.source_labels.as_ref().unwrap();
    const LABELS: [&str; 3] = [
        "• Destroy target enchantment.",
        "• Draw two cards.",
        "• Target player discards two cards.",
    ];
    let text = format!("Choose one —\n{}", LABELS.join("\n"));
    assert_eq!(labels.blocks, [text]);
    assert_eq!(labels.spell_modes, LABELS);
    assert_eq!(
        destroy.program.targets,
        [TargetSpec {
            slot: 0,
            selection_id: "line:1:2".into(),
            chooser: Chooser::Controller,
            stage: EvaluationStage::Announcement,
            domain: TargetDomain::BattlefieldPermanent,
            filter: Some(PermanentFilter {
                power: None,
                types: vec![CardKind::Enchantment],
                non_types: vec![],
                non_colors: vec![],
                opponent: false,
                tapped: false
            })
        }]
    );
    assert_eq!(
        destroy.program.instructions,
        [Instruction::DestroyPermanent {
            target_slot: 0,
            optional: false,
            regeneration_forbidden: false,
            source_block: 0
        }]
    );
    assert!(draw.program.targets.is_empty());
    assert_eq!(
        draw.program.instructions,
        vec![
            Instruction::DrawCards {
                recipient: None,
                count: 1.into(),
                source_block: 0
            };
            2
        ]
    );
    assert_discard(&discard.program, TargetDomain::AnyPlayer, "line:3:1", 2);
}

#[test]
fn targeted_discard() {
    let synthetic = synthetic_single();
    let d = lowered(&synthetic);
    assert_discard(
        d.spell.as_ref().unwrap(),
        TargetDomain::AnyPlayer,
        "line:0:0",
        1,
    );
    assert!(d.capabilities.contains(&Capability::TargetPlayerDiscardV1));
    for case in 0..10 {
        let raw = changed_source(&synthetic, |r| {
            let result = &mut r["faces"][0]["result"];
            let step = &mut result["abilities"][0]["effects"][0]["steps"][0];
            match case {
                0 => step["who"]["of"] = Value::Null,
                1 => step["who"]["player"] = json!("target-opponent"),
                2 => step["what"]["filter"]["types"] = json!(["land"]),
                3 => step["what"]["count"]["value"] = json!(2),
                4 => {
                    let text = "Target player discards a card at random.";
                    result["lines"][0]["text"] = json!(text);
                    r["source"]["oracle_text"] = json!(text);
                }
                5 => r["binding"]["references"][1]["target"] = json!("foreign-query"),
                6 => r["binding"]["nodes"][4]["timing"] = json!("resolution"),
                7 => r["binding"]["nodes"][6]["inputs"].as_array_mut().unwrap().reverse(),
                8 => r["source"]["mana_cost"] = json!("{X}{B}"),
                _ => result["abilities"][0]["effects"][0]["steps"].as_array_mut().unwrap()
                    .push(json!({"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1}})),
            }
            let result = &mut r["faces"][0]["result"];
            result["lines"][0]["ability"] = result["abilities"][0].clone();
        });
        assert!(
            !matches!(compile(MANIFEST, &raw), CompileResult::Lowered { .. }),
            "ordinary rejection {case}"
        );
    }

    if let Some(path) = std::env::var_os("MTG_TARGETED_DISCARD_ARCHIVE_PROBE") {
        archive_witnesses(path);
    }
}

fn archive_witnesses(path: std::ffi::OsString) {
    // Physical archive rows include the header; the record's sourceIndex is zero-based.
    let pins = [
        (
            17206,
            "8b6481d82325c05e1ed4d94a8c659b7622588a420f313b3c754016ff7d9e97f7",
        ),
        (
            26101,
            "2a3a7b11bd98e63efcd32fb523acd70681aa0fc36181a336682bc50230e7f9ed",
        ),
        (
            26223,
            "15ec8a0f52d4177fcaa8fd09b244d38c3697da28d76a9fd688867f642d55bb6a",
        ),
        (
            26973,
            "9825a0c13dff0f61285c58286cb7cf2d3a5e6b9d14734c182d3eeeeff6e38324",
        ),
        (
            32006,
            "8aaa516e9f4cefbfe05ae874227692c942670e83c3a7501bdb2fb8821c860510",
        ),
        (
            36900,
            "061e80a3f20672dbcac53ae5c376e0f93006fc81ba33b6b7636a32c949851f47",
        ),
    ];
    let rows = std::fs::read_to_string(path).unwrap();
    assert_eq!(rows.lines().count(), pins.len());
    for (raw, (row, hash)) in rows.lines().zip(pins) {
        assert_eq!(sha256(raw.as_bytes()), hash);
        let d = lowered(raw);
        assert_eq!(d.provenance.source_index, row - 2);
        let mut expected = BTreeSet::from([
            Capability::NormalLayoutV1,
            Capability::TargetPlayerDiscardV1,
        ]);
        expected.insert(if row == 26223 {
            Capability::OpponentTargetV1
        } else {
            Capability::AnyPlayerTargetV1
        });
        if row == 32006 {
            expected.extend([
                Capability::CastInstantV1,
                Capability::ModalChoiceV1,
                Capability::PermanentRemovalV1,
                Capability::ControllerDrawV1,
            ]);
        } else {
            expected.insert(Capability::CastSorceryV1);
        }
        assert_eq!(d.capabilities, expected);
        if row == 32006 {
            modal_witness(&d);
            for case in 0..4 {
                let bad = changed_source(raw, |r| {
                    let result = &mut r["faces"][0]["result"];
                    let modal = &mut result["abilities"][0]["effects"][0]["steps"][0];
                    match case {
                        0 => modal["count"]["exactly"] = json!(2),
                        1 => modal["options"].as_array_mut().unwrap().swap(0, 1),
                        2 => {
                            modal["options"][1]["effects"][0]["steps"][0]["amount"]["value"] =
                                json!(1)
                        }
                        _ => {
                            r["binding"]["references"][3]["target"] =
                                r["binding"]["references"][0]["target"].clone()
                        }
                    }
                    let result = &mut r["faces"][0]["result"];
                    result["lines"][0]["ability"] = result["abilities"][0].clone();
                });
                assert!(
                    !matches!(compile(MANIFEST, &bad), CompileResult::Lowered { .. }),
                    "modal rejection {case}"
                );
            }
        } else {
            let domain = if row == 26223 {
                TargetDomain::OpponentPlayer
            } else {
                TargetDomain::AnyPlayer
            };
            let count = if matches!(row, 17206 | 26973) { 3 } else { 2 };
            assert_discard(d.spell.as_ref().unwrap(), domain, "line:0:0", count);
        }
    }
}

#[test]
fn random_discard() {
    let synthetic = changed_source(&synthetic_single(), |r| {
        let text = "Target player discards a card at random.";
        r["source"]["oracle_text"] = json!(text);
        let result = &mut r["faces"][0]["result"];
        result["lines"][0]["text"] = json!(text);
        result["abilities"][0]["effects"][0]["steps"][0]["random"] = json!(true);
        result["lines"][0]["ability"] = result["abilities"][0].clone();
    });
    let random = |count| Instruction::DiscardTargetPlayerAtRandom {
        target_slot: 0,
        source_block: 0,
        count,
    };
    let fixed = |n: i64| RandomDiscardCount::Fixed { value: n.into() };
    assert_eq!(
        lowered(&synthetic).spell.unwrap().instructions,
        [random(fixed(1))]
    );
    for case in 0..8 {
        let bad = changed_source(&synthetic, |r| {
            let result = &mut r["faces"][0]["result"];
            let step = &mut result["abilities"][0]["effects"][0]["steps"][0];
            match case {
                0 => {
                    step.as_object_mut().unwrap().remove("random");
                }
                1 => step["random"] = json!(false),
                2 => step["random"] = Value::Null,
                3 => step["what"]["upTo"] = json!(true),
                4 => step["who"]["count"] = json!({"amount":"fixed","value":1}),
                5 => r["binding"]["references"][0]["target"] = json!("another player"),
                6 => r["source"]["mana_cost"] = json!("{X}{B}"),
                _ => {
                    let text = "Target player discards a card.";
                    result["lines"][0]["text"] = json!(text);
                    r["source"]["oracle_text"] = json!(text);
                }
            }
            let result = &mut r["faces"][0]["result"];
            result["lines"][0]["ability"] = result["abilities"][0].clone();
        });
        assert!(
            !matches!(compile(MANIFEST, &bad), CompileResult::Lowered { .. }),
            "case {case}"
        );
    }
    let Some(path) = std::env::var_os("MTG_RANDOM_DISCARD_ARCHIVE_PROBE") else {
        return;
    };
    let pins = [
        (
            5790,
            "d3585fefc164be392ec66bb2d7862400ce9143e1c48a34fad869ad8b17a105fe",
        ),
        (
            18241,
            "d34c738a1f8cc5d89a489e10a61071a6af48f41ae165b2be8dbcb13d2b15c8aa",
        ),
        (
            23084,
            "51eba19037e55f546977cc1760fd0a43e5854896f65b8fca909425945e11d2f4",
        ),
        (
            26398,
            "911a2740664dee606a852eef8655135e7d3e32f3e1c5da714c6217ee81c262eb",
        ),
        (
            29178,
            "72294dd965311099f3647f57ad8cb0e66ab9ee03863a3fe5f70a6a1b844a711f",
        ),
        (
            37752,
            "1c9caa1cbb82f877a5b8114c2e19c9d3670d9d51acdf43ede17ed423e454dac2",
        ),
    ];
    let rows = std::fs::read_to_string(path).unwrap();
    assert_eq!(rows.lines().count(), pins.len());
    for (raw, (row, hash)) in rows.lines().zip(pins) {
        assert_eq!(sha256(raw.as_bytes()), hash);
        let d = lowered(raw);
        assert_eq!(d.provenance.source_index, row - 2);
        let x = matches!(row, 5790 | 18241);
        assert_eq!(d.x_mana_cost, x);
        let mut expected = vec![random(if x {
            RandomDiscardCount::AnnouncedX
        } else if row == 23084 {
            fixed(2)
        } else {
            fixed(1)
        })];
        let (domain, cap) = if matches!(row, 29178 | 37752) {
            (TargetDomain::OpponentPlayer, Capability::OpponentTargetV1)
        } else {
            (TargetDomain::AnyPlayer, Capability::AnyPlayerTargetV1)
        };
        let mut caps = BTreeSet::from([
            Capability::CastSorceryV1,
            Capability::NormalLayoutV1,
            Capability::RandomPlayerDiscardV1,
            cap,
        ]);
        if row == 29178 {
            expected.push(Instruction::DiscardTargetPlayer {
                target_slot: 0,
                count: 1.into(),
                source_block: 0,
            });
            caps.insert(Capability::TargetPlayerDiscardV1);
        }
        let p = d.spell.unwrap();
        assert_eq!(p.instructions, expected);
        assert_eq!(d.capabilities, caps);
        assert_eq!(
            p.targets,
            [TargetSpec {
                slot: 0,
                selection_id: "line:0:0".into(),
                domain,
                filter: None,
                chooser: Chooser::Controller,
                stage: EvaluationStage::Announcement
            }]
        );
        if x || row == 29178 {
            let bad = changed_source(raw, |r| {
                // The count must reference this cast's announced X. The second
                // discard must use the first discard's announced player.
                let index = if x { 1 } else { 2 };
                r["binding"]["references"][index]["target"] = json!("foreign binding");
            });
            assert!(!matches!(
                compile(MANIFEST, &bad),
                CompileResult::Lowered { .. }
            ));
        }
    }
}
