use super::*;
use crate::tests::changed_source;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader};

const HEADER: &str = include_str!("../../../../tests/fixtures/archive-manifest-current.json");

fn changed(raw: &str, edit: impl FnOnce(&mut Value)) -> String {
    changed_source(raw, |r| {
        edit(r);
        let result = &mut r["faces"][0]["result"];
        result["lines"][0]["ability"] = result["abilities"][0].clone();
    })
}

fn synthetic(count: i64, printed: &str) -> String {
    changed(
        include_str!("../../../../tests/fixtures/counterspell.json"),
        |r| {
            let text = format!("Counter target spell. Its controller mills {printed}.");
            r["source"]["oracle_text"] = json!(text);
            r["source"]["keywords"] = json!(["Mill"]);
            let result = &mut r["faces"][0]["result"];
            result["lines"][0]["text"] = json!(text);
            let who = json!({"player":"controller","of":{"ref":"it"}});
            result["abilities"][0]["effects"]
                .as_array_mut()
                .unwrap()
                .push(json!({"steps":[
                    {"effect":"mill","who":who,"amount":{"amount":"fixed","value":count}}
                ]}));

            let binding = &mut r["binding"];
            let path = "/faces/0/abilities/0/effects/1/steps/0";
            let player = format!("{path}/who");
            let root = format!("card:{}", binding["identity"].as_str().unwrap());
            let event = format!("{root}{path}#event");
            let query = format!("{root}{player}#query");
            let scope = binding["nodes"][2]["id"].clone();
            let target = binding["references"][0]["target"].clone();
            binding["nodes"].as_array_mut().unwrap().extend([
            json!({"id":query,"node":"selection","path":player,"scope":scope,"expression":who,
                "cardinality":"one","timing":"resolution","selection":"evaluate-reference","rules":["608.2h"]}),
            json!({"id":event,"node":"event","path":path,"scope":scope,"operation":"mill",
                "inputs":[query],"availability":"when-performed","rules":["608.2c"]}),
            json!({"id":format!("{root}{path}#objects"),"node":"event-result","path":path,"event":event,
                "result":"objects","cardinality":"many","availability":"if-produced","identity":"new-objects",
                "description":{"cls":"card"},"rules":["608.2h"]}),
            json!({"id":format!("{root}{path}#quantity"),"node":"event-result","path":path,"event":event,
                "result":"quantity","cardinality":"one","availability":"if-produced","identity":"measured-quantity",
                "rules":["608.2h"]}),
        ]);
            binding["references"].as_array_mut().unwrap().extend([
            json!({"path":format!("{player}/of"),"expression":{"ref":"it"},"binding":"bound","target":target,
                "rules":["608.2k","400.7j","608.2h"]}),
            json!({"path":player,"expression":who,"binding":"bound","target":query,"rules":["608.2h"]}),
        ]);
            binding["referencesBound"] = json!(true);
            binding["engineFeatures"] = json!([]);
            binding["counts"] = json!({"obligations":3,"bound":3,"unresolved":0});
        },
    )
}

fn assert_lowered(raw: &str, count: i64) -> Box<Definition> {
    let d = match compile(HEADER, raw) {
        CompileResult::Lowered { definition } => definition,
        result => panic!("{result:?}"),
    };
    d.validate().unwrap();
    let p = d.spell.as_ref().unwrap();
    assert_eq!(
        p.targets,
        [TargetSpec {
            slot: 0,
            selection_id: "line:0:1".into(),
            chooser: Chooser::Controller,
            stage: EvaluationStage::Announcement,
            domain: TargetDomain::StackSpell,
            filter: None,
        }]
    );
    assert_eq!(
        p.instructions,
        [Instruction::CounterSpellAndMillController {
            target_slot: 0,
            count: count.into(),
            source_block: 0,
        }]
    );
    assert_eq!(
        d.capabilities,
        BTreeSet::from([
            Capability::NormalLayoutV1,
            Capability::CastInstantV1,
            Capability::CounterSpellControllerMillV1,
            Capability::StackSpellTargetV1,
            Capability::CounterStackObjectV1
        ])
    );
    d
}

#[test]
fn counter_spell_controller_mill_source_and_binding_fences() {
    for (n, printed) in [(1, "a card"), (7, "seven cards"), (103, "103 cards")] {
        assert_lowered(&synthetic(n, printed), n);
    }
    let base = synthetic(7, "seven cards");
    let mutations: &[fn(&mut Value)] = &[
        |r| r["binding"]["references"][1]["target"] = r["binding"]["nodes"][3]["id"].clone(),
        |r| {
            r["binding"]["references"][2]["target"] =
                r["binding"]["references"][0]["target"].clone()
        },
        |r| r["binding"]["references"][1]["rules"] = json!(["608.2h"]),
        |r| r["binding"]["nodes"][6]["timing"] = json!("stack-announcement"),
        |r| r["binding"]["nodes"][7]["inputs"] = json!([r["binding"]["nodes"][3]["id"]]),
        |r| {
            r["binding"]["nodes"].as_array_mut().unwrap().pop();
        },
        |r| r["binding"]["counts"]["obligations"] = json!(2),
        |r| r["source"]["type_line"] = json!("Sorcery"),
        |r| r["source"]["mana_cost"] = json!("{X}{U}"),
        |r| r["source"]["keywords"] = json!(["Mill", "Flashback"]),
        |r| {
            r["faces"][0]["result"]["abilities"][0]["effects"][1]["steps"][0]["who"]["player"] =
                json!("owner")
        },
        |r| {
            r["faces"][0]["result"]["abilities"][0]["effects"][1]["steps"][0]["who"]["of"] =
                Value::Null
        },
        |r| {
            r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["what"]["selectionId"] =
                json!("line:0:2")
        },
        |r| {
            r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["what"]["filter"]["allOf"] =
                json!([{"types":["instant"]}])
        },
        |r| {
            r["faces"][0]["result"]["abilities"][0]["effects"][1]["steps"][0]["amount"]["value"] =
                json!(2)
        },
        |r| r["faces"][0]["result"]["abilities"][0]["effects"][1]["condition"] = Value::Null,
        |r| {
            r["faces"][0]["result"]["abilities"][0]["effects"]
                .as_array_mut()
                .unwrap()
                .swap(0, 1);
        },
        |r| {
            r["faces"][0]["result"]["abilities"][0]["effects"]
                .as_array_mut()
                .unwrap()
                .pop();
        },
        |r| {
            let text = "Counter target spell. Its controller mills seven cards. Draw a card.";
            r["source"]["oracle_text"] = json!(text);
            r["faces"][0]["result"]["lines"][0]["text"] = json!(text);
        },
    ];
    for (i, edit) in mutations.iter().enumerate() {
        let bad = changed(&base, edit);
        assert!(
            !matches!(compile(HEADER, &bad), CompileResult::Lowered { .. }),
            "mutation {i}"
        );
    }
    for (n, printed) in [
        (0, "zero cards"),
        (-1, "-1 cards"),
        (1, "one cards"),
        (7, "eight cards"),
    ] {
        assert!(!matches!(
            compile(HEADER, &synthetic(n, printed)),
            CompileResult::Lowered { .. }
        ));
    }
}

#[test]
fn counter_spell_controller_mill_archive_or_synthetic() {
    let Some(path) = std::env::var_os("MTG_COUNTER_SPELL_CONTROLLER_MILL_ARCHIVE_PROBE") else {
        assert_lowered(&synthetic(3, "three cards"), 3);
        return;
    };
    let expected = [
        (
            3576,
            "Psychic Strike",
            "d870f1dd966165295b5259867a38e1dc8b79c3164ee881a9af552dfa2df3f2fb",
            2,
            1,
            [Color::U, Color::B],
        ),
        (
            10102,
            "Countermand",
            "7c145bbf6b69ed76349f9dbccce2d55ebfb9ecc783489a03b83adbd9e7ae5e93",
            4,
            2,
            [Color::U, Color::U],
        ),
        (
            16004,
            "Thought Collapse",
            "5321108cbe19ac40db0fceb79fff10e61c9823b6c6074a4ee96b3a7f84c659e9",
            3,
            1,
            [Color::U, Color::U],
        ),
        (
            27840,
            "Didn't Say Please",
            "41b1bd452dd2effba27795494f535bfee33a72e09f21ae93a5a19fdacfa94e39",
            3,
            1,
            [Color::U, Color::U],
        ),
    ];
    // Accept the complete frozen archive or its four exact rows in source order.
    let mut rows = BufReader::new(std::fs::File::open(path).unwrap())
        .lines()
        .map(Result::unwrap)
        .peekable();
    let first: Value = serde_json::from_str(rows.peek().unwrap()).unwrap();
    let full = first["recordType"] == "manifest";
    let mut previous = 0;
    for (line, name, digest, count, generic, colored) in expected {
        let raw = rows
            .nth(if full { line - previous - 1 } else { 0 })
            .unwrap();
        previous = line;
        assert_eq!(sha256(raw.as_bytes()), digest, "archive row {line}");
        let d = assert_lowered(&raw, count);
        assert_eq!(d.name, name);
        assert_eq!(d.provenance.source_index as usize, line - 2);
        assert_eq!(
            d.characteristics.mana_cost,
            ManaCost {
                generic: Quantity::from(generic),
                colored: colored.to_vec()
            }
        );
    }
    if !full {
        assert!(rows.next().is_none());
    }
}
