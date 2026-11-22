use super::*;
use crate::tests::changed_source;
use std::io::{BufRead, BufReader};
use std::path::Path;

const MANIFEST: &str = include_str!("../../../../../tests/fixtures/archive-manifest-current.json");
const PINS: [(usize, &str, &str); 2] = [
    (
        755,
        "Bojuka Bog",
        "9bd9cea6d63676699f88d8116a5ae5a3a3b9f621ab8ba5f380489f17934011e0",
    ),
    (
        10930,
        "Angel of Finality",
        "178acf247a92c72b31904eefe36901aa06ceb377a0efe5df505ff9fd0ab87a93",
    ),
];

// An original two-row extraction or the designated full archive. Only the
// original, un-reencoded rows count as frozen-source witnesses.
fn archived(path: &Path) -> [String; 2] {
    let mut rows = BufReader::new(std::fs::File::open(path).unwrap())
        .lines()
        .map(Result::unwrap);
    let first = rows.next().unwrap();
    if serde_json::from_str::<Value>(&first).unwrap()["recordType"] == "manifest" {
        assert_eq!(first, MANIFEST.trim_end());
        let land = rows.nth(PINS[0].0 - 2).unwrap();
        let creature = rows.nth(PINS[1].0 - PINS[0].0 - 1).unwrap();
        [land, creature]
    } else {
        let creature = rows.next().unwrap();
        assert!(
            rows.next().is_none(),
            "expected exactly two extracted records"
        );
        [first, creature]
    }
}

fn synthetic(land: bool) -> String {
    let base = if land {
        include_str!("../../../../../tests/fixtures/command-tower.json")
    } else {
        include_str!("../../../../../tests/fixtures/grizzly-bears.json")
    };
    changed_source(base, |r| {
        let source = &mut r["source"];
        let noun = if land { "land" } else { "creature" };
        let name = "Synthetic entry exile";
        source["name"] = json!(name);
        let subject = json!({"ref":"this","noun":noun});
        let mut lines = vec![];
        if land {
            source["color_identity"] = json!(["B"]);
            source["produced_mana"] = json!(["B"]);
            lines.push((
                "This land enters tapped.".into(),
                json!({"kind":"static","effect":{"static":"enters-tapped","what":subject}}),
            ));
        } else {
            source["keywords"] = json!(["Flying"]);
            lines.push((
                "Flying".into(),
                json!({"kind":"keywords","keywords":[{"keyword":"flying"}]}),
            ));
        }
        lines.push((
            format!("When this {noun} enters, exile target player's graveyard."),
            json!({"kind":"triggered","trigger":{"trigger":"enters","what":subject},
                "effects":[{"steps":[{"effect":"exile","what":graveyard()}]}]}),
        ));
        if land {
            lines.push(("{T}: Add {B}.".into(),json!({"kind":"activated","costs":[{"cost":"tap-self"}],"effects":[{"steps":[{"effect":"add-mana","who":{"player":"you"},"mana":{"mana":"fixed","symbols":["B"]}}]}]})));
        }
        source["oracle_text"] = json!(
            lines
                .iter()
                .map(|(s, _)| s.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        );
        r["binding"] =
            graph(source["oracle_id"].as_str().unwrap(), source, land).synthetic_envelope();
        let result = &mut r["faces"][0]["result"];
        result["name"] = json!(name);
        result["lines"] = lines
            .iter()
            .map(|(s, a)| json!({"text":s,"ability":a,"ok":true}))
            .collect();
        result["abilities"] = lines.into_iter().map(|(_, a)| a).collect();
    })
}

#[test]
fn entry_graveyard_exile() {
    let probe = std::env::var_os("MTG_ENTRY_GRAVEYARD_EXILE_ARCHIVE_PROBE");
    let rows = match &probe {
        Some(path) => archived(Path::new(path)),
        None => [synthetic(true), synthetic(false)],
    };
    for (i, raw) in rows.iter().enumerate() {
        let compiled = compile(MANIFEST, raw);
        let CompileResult::Lowered { definition: d } = compiled else {
            panic!("{compiled:?}")
        };
        if probe.is_some() {
            let (row, name, digest) = PINS[i];
            assert_eq!(sha256(raw.as_bytes()), digest);
            assert_eq!(d.name, name);
            assert_eq!(d.provenance.source_index as usize + 2, row);
        }
        let land = i == 0;
        use Capability::*;
        let mut caps = BTreeSet::from([
            NormalLayoutV1,
            TriggeredAbilityV1,
            SelfEntersBattlefieldV1,
            AnyPlayerTargetV1,
            EntryGraveyardExileV1,
        ]);
        caps.extend(if land {
            [LandPlayV1, EntersTappedV1, PermanentManaV1]
        } else {
            [BaseCreatureV1, CastCreatureV1, FlyingV1]
        });
        assert_eq!(d.capabilities, caps);
        let t = &d.triggered[0];
        assert_eq!(t.condition, TriggerCondition::SelfEntersBattlefield);
        assert_eq!(
            t.program.targets,
            [TargetSpec {
                slot: 0,
                selection_id: "line:1:6".into(),
                chooser: Chooser::Controller,
                stage: EvaluationStage::Announcement,
                domain: TargetDomain::AnyPlayer,
                filter: None
            }]
        );
        assert_eq!(
            t.program.instructions,
            [Instruction::exile_target_graveyard(0, 1)]
        );
        if land {
            assert_eq!(d.static_abilities[0].effect, StaticEffect::EntersTapped);
            assert_eq!(
                d.mana_abilities[0].production,
                ManaProduction::Fixed {
                    symbols: vec![Color::B]
                }
            );
            assert!(d.mana_abilities[0].costs.tap);
            assert_eq!(d.mana_abilities[0].source_block, 2);
        } else {
            assert_eq!(d.keywords, [KeywordAbility::Flying]);
            assert_eq!(
                d.spell.as_ref().unwrap().resolution,
                SpellResolution::Permanent
            );
        }

        for mutation in 0..13 {
            let wrong = changed_source(raw, |r| {
                let result = &mut r["faces"][0]["result"];
                let ability = &mut result["abilities"][1];
                let step = &mut ability["effects"][0]["steps"][0];
                match mutation {
                    0 => step["what"]["owner"]["player"] = json!("you"),
                    1 => step["what"]["zone"] = json!("library"),
                    2 => step["who"] = json!({"player":"target-player"}),
                    3 => step["optional"] = Value::Null,
                    4 => ability["trigger"]["what"]["noun"] = json!("artifact"),
                    5 => ability["effects"][0]["steps"]
                        .as_array_mut()
                        .unwrap()
                        .push(json!({"effect":"scry","amount":{"amount":"fixed","value":1}})),
                    6 => {
                        *step = json!({"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1}})
                    }
                    7 => {
                        let text = result["lines"][1]["text"]
                            .as_str()
                            .unwrap()
                            .replace("exile", "you may exile");
                        result["lines"][1]["text"] = json!(text);
                        r["source"]["oracle_text"] = json!(
                            r["source"]["oracle_text"]
                                .as_str()
                                .unwrap()
                                .replace("exile", "you may exile")
                        );
                    }
                    8 | 9 => {
                        let selection = if mutation == 8 {
                            "target"
                        } else {
                            "evaluate-reference"
                        };
                        let node = r["binding"]["nodes"]
                            .as_array_mut()
                            .unwrap()
                            .iter_mut()
                            .find(|n| n["selection"] == selection)
                            .unwrap();
                        node["timing"] = json!(if mutation == 8 {
                            "resolution"
                        } else {
                            "stack-announcement"
                        });
                    }
                    10 => {
                        let node = r["binding"]["references"]
                            .as_array_mut()
                            .unwrap()
                            .iter_mut()
                            .find(|n| n["expression"]["ref"] == "zone-cards")
                            .unwrap();
                        node["target"] = json!("wrong:owner");
                    }
                    11 => {
                        let nodes = r["binding"]["nodes"].as_array_mut().unwrap();
                        let index = nodes
                            .iter()
                            .position(|n| {
                                n["role"]
                                    == if land {
                                        "entering-controller"
                                    } else {
                                        "static-source-controller"
                                    }
                            })
                            .unwrap();
                        nodes.remove(index);
                    }
                    12 => {
                        let noun = if land { "land" } else { "creature" };
                        let text = format!("When this {noun} enters, draw a card.");
                        *step = json!({"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1}});
                        result["lines"][1]["text"] = json!(text);
                        r["source"]["oracle_text"] = json!(
                            r["source"]["oracle_text"]
                                .as_str()
                                .unwrap()
                                .replace("exile target player's graveyard.", "draw a card.")
                        );
                    }
                    _ => unreachable!(),
                }
                let result = &mut r["faces"][0]["result"];
                result["lines"][1]["ability"] = result["abilities"][1].clone();
            });
            assert!(
                !matches!(compile(MANIFEST, &wrong), CompileResult::Lowered { .. }),
                "source {i}, mutation {mutation}"
            );
        }
    }
}
