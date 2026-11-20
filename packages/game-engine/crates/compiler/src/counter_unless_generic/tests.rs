use super::*;
use crate::tests::changed_source;
use std::io::{BufRead, BufReader};

const MANIFEST: &str = include_str!("../../../../tests/fixtures/archive-manifest-current.json");

fn synthetic(symbol: &str) -> String {
    changed_source(
        include_str!("../../../../tests/fixtures/counterspell.json"),
        |r| {
            let text = format!("Counter target spell unless its controller pays {{{symbol}}}.");
            r["source"]["oracle_text"] = json!(text);
            let result = &mut r["faces"][0]["result"];
            let effect = &mut result["abilities"][0]["effects"][0];
            let who = json!({"player":"controller","of":{"ref":"it"}});
            effect["condition"] =
                json!({"condition":"pays","who":who,"cost":[{"cost":"mana","symbols":[symbol]}]});
            effect["conditionKind"] = json!("unless");
            result["lines"][0]["text"] = json!(text);
            result["lines"][0]["ability"] = result["abilities"][0].clone();
            let b = &mut r["binding"];
            let target = b["references"][0]["target"].clone();
            let path = format!("{CONDITION}/who");
            let query = format!("card:{}{path}#query", b["identity"].as_str().unwrap());
            let scope = b["nodes"][2]["id"].clone();
            b["nodes"][5]["conditional"] = json!(true);
            b["nodes"].as_array_mut().unwrap().push(json!({
            "id":query,"path":path,"node":"selection","scope":scope,"expression":who,
            "cardinality":"one","timing":"resolution","selection":"evaluate-reference","rules":["608.2h"]
        }));
            b["references"].as_array_mut().unwrap().extend([
            json!({"path":format!("{path}/of"),"expression":{"ref":"it"},"binding":"bound","target":target,"rules":["608.2k","400.7j","608.2h"]}),
            json!({"path":path,"expression":who,"binding":"bound","target":query,"rules":["608.2h"]}),
            json!({"path":format!("{CONDITION}/cost/0/payer"),"expression":{"player":"you"},"binding":"bound","target":query,"rules":["109.5"]}),
        ]);
            b["referencesBound"] = json!(true);
            b["engineFeatures"] = json!([]);
            b["counts"] = json!({"obligations":4,"bound":4,"unresolved":0});
        },
    )
}

fn witness(raw: &str, generic: Quantity) {
    use Capability::*;
    let result = compile(MANIFEST, raw);
    let CompileResult::Lowered { definition: d } = result else {
        panic!("{result:?}")
    };
    assert_eq!(
        d.capabilities,
        BTreeSet::from([
            NormalLayoutV1,
            CastInstantV1,
            StackSpellTargetV1,
            CounterStackObjectV1,
            CounterUnlessGenericV1
        ])
    );
    assert_eq!(d.characteristics.kind, CardKind::Instant);
    let p = d.spell.unwrap();
    assert_eq!(
        p.instructions,
        [Instruction::CounterSpellUnlessPays {
            target_slot: 0,
            generic,
            source_block: 0
        }]
    );
    assert_eq!(
        p.targets,
        [TargetSpec {
            slot: 0,
            selection_id: SELECTION.into(),
            chooser: Chooser::Controller,
            stage: EvaluationStage::Announcement,
            domain: TargetDomain::StackSpell,
            filter: None
        }]
    );
}

fn reject(raw: &str, case: &str) {
    assert!(
        !matches!(compile(MANIFEST, raw), CompileResult::Lowered { .. }),
        "{case}"
    );
}

#[test]
fn counter_unless_generic_complete_source() {
    let raw = synthetic("3");
    witness(&raw, 3.into());
    let large = "18446744073709551616";
    witness(&synthetic(large), large.parse().unwrap());
    for symbol in ["0", "-1", "03", "+3", "3.0", "X", "U", "2/U", "1}{2"] {
        reject(&synthetic(symbol), symbol);
    }
    for (pointer, value) in [
        ("/conditionKind", Some(json!("if"))),
        ("/conditionKind", Some(Value::Null)),
        ("/conditionKind", None),
        ("/condition", None),
        (
            "/condition/who/of",
            Some(json!({"ref":"this","noun":"spell"})),
        ),
        ("/condition/who", Some(json!({"player":"you"}))),
        ("/condition/who/of", None),
        (
            "/condition/cost/0",
            Some(json!({"cost":"mana","symbols":["3"],"payer":null})),
        ),
        ("/condition/cost/0/symbols", Some(json!(["2", "1"]))),
        ("/condition/cost/0/symbols", Some(json!(["2"]))),
        (
            "/condition/cost",
            Some(json!([{"cost":"mana","symbols":["3"]},{"cost":"tap"}])),
        ),
        (
            "/steps/0/what/filter",
            Some(json!({"cls":"spell","allOf":null})),
        ),
        ("/steps/0/what/count/value", Some(json!(2))),
        ("/steps/0/what/selectionId", Some(json!("line:1:1"))),
        ("/steps", Some(json!([]))),
    ] {
        let bad = changed_source(&raw, |r| {
            let result = &mut r["faces"][0]["result"];
            let sentence = &mut result["abilities"][0]["effects"][0];
            match value {
                Some(value) => *sentence.pointer_mut(pointer).unwrap() = value,
                None => {
                    let (parent, key) = pointer.rsplit_once('/').unwrap();
                    sentence
                        .pointer_mut(parent)
                        .unwrap()
                        .as_object_mut()
                        .unwrap()
                        .remove(key)
                        .unwrap();
                }
            }
            result["lines"][0]["ability"] = result["abilities"][0].clone();
        });
        reject(&bad, pointer);
    }
    for (pointer, value) in [
        ("/binding/nodes/4/timing", json!("resolution")),
        ("/binding/nodes/5/conditional", json!(false)),
        ("/binding/nodes/5/inputs", json!([])),
        ("/binding/nodes/6/timing", json!("stack-announcement")),
        ("/binding/counts/bound", json!(3)),
        ("/source/keywords", json!(["Split second"])),
        ("/source/type_line", json!("Instant — Lesson")),
        ("/source/mana_cost", json!("{X}{U}")),
        (
            "/faces/0/result/lines/0/text",
            json!("Counter target spell."),
        ),
        (
            "/source/oracle_text",
            json!("Counter target spell unless its controller pays {3}. Draw a card."),
        ),
    ] {
        let bad = changed_source(&raw, |r| *r.pointer_mut(pointer).unwrap() = value);
        reject(&bad, pointer);
    }
    for i in 0..4 {
        let bad = changed_source(&raw, |r| {
            // All four obligations remain counted and point at real nodes. Even the
            // implicit cost payer must resolve to the query for this particular target.
            r["binding"]["references"][i]["target"] = r["binding"]["nodes"][3]["id"].clone();
        });
        reject(&bad, "foreign reference");
        let bad = changed_source(&raw, |r| {
            r["binding"]["references"].as_array_mut().unwrap().remove(i);
            r["binding"]["counts"] = json!({"bound":3,"obligations":3,"unresolved":0});
        });
        reject(&bad, "missing reference");
    }
}

#[test]
fn counter_unless_generic_archive_probe() {
    let Some(path) = std::env::var_os("MTG_COUNTER_UNLESS_GENERIC_ARCHIVE_PROBE") else {
        witness(&synthetic("1"), 1.into());
        return;
    };
    let pins = [
        (
            20514,
            "Mindstatic",
            6,
            "c586b03b8f16300f23ec4069bacd26fc4ce1d5e6efec0b9e3302a6ec19c49583",
        ),
        (
            20717,
            "Force Spike",
            1,
            "e45aa50cc0189ae3c484300e8a10e6f814a69169d1c1a11806e9975cd6cc02d0",
        ),
        (
            22419,
            "Mana Tithe",
            1,
            "627c3e432fbea208435532209727f27bf8e34f3105351b135032940cb3dae1e0",
        ),
        (
            29840,
            "Mana Leak",
            3,
            "0c374ab42c858b02e9ffc9dce9bc025c88eb86459eb9287de4a1f0692eba0ae0",
        ),
        (
            30930,
            "Convolute",
            4,
            "695075eacb033aba982c7867a907caadc53428058f6e97a2ac6a883ce6428e06",
        ),
        (
            37794,
            "Quench",
            2,
            "339d99120502d3c4e338b035b78fba0e8a7b309499807108e706ec14fd48509d",
        ),
    ];
    // Accept the full JSONL or its exact six unmodified rows in physical order.
    let mut rows = BufReader::new(std::fs::File::open(path).unwrap())
        .lines()
        .enumerate()
        .peekable();
    let first: Value = serde_json::from_str(rows.peek().unwrap().1.as_ref().unwrap()).unwrap();
    let full_archive = first["recordType"] == "manifest";
    for (line, name, amount, hash) in pins {
        let (_, raw) = if full_archive {
            rows.find(|(i, _)| *i + 1 == line)
        } else {
            rows.next()
        }
        .expect("missing pinned record");
        let raw = raw.unwrap();
        assert_eq!(sha256(raw.as_bytes()), hash, "{name}");
        let parsed: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed["sourceIndex"], line - 2);
        assert_eq!(parsed["source"]["name"], name);
        witness(&raw, amount.into());
    }
    assert!(full_archive || rows.next().is_none());
}
