//! Ordinary printed-X spells. CR 107.3a, 601.2b-c, 608.2b-c and 121.2.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

fn fail() -> Failure {
    unsupported(
        "announced_x_draw",
        "requires complete bound announced-X draw spell",
    )
}

pub(super) fn lower(
    raw: &str,
    text: &str,
    binding: &Value,
    identity: &str,
    mana: &str,
    program: &mut SpellProgram,
    slots: &mut BTreeMap<String, u32>,
) -> Result<(), Failure> {
    let (target, operations): (bool, &[&str]) = match normalization::literal_spelling(text).as_str()
    {
        "Draw X cards." => (false, &["draw"]),
        "Target player draws X cards." => (true, &["draw"]),
        "Target player draws X cards and loses X life." => (true, &["draw", "lose-life"]),
        "You gain X life and draw X cards." => (false, &["gain-life", "draw"]),
        "Draw X cards, then discard a card." => (false, &["draw", "discard"]),
        _ => return Err(fail()),
    };
    let base = "/faces/0/abilities/0";
    let root = format!("card:{identity}{base}");
    let scope = format!("{root}#ability");
    let you = format!("{root}#you");
    let x = format!("{root}#announced-x");
    // The parser allocates the first target in this one-line spell at line:0:0.
    let actor = if target {
        json!({"player":"target-player","selectionId":"line:0:0"})
    } else {
        json!({"player":"you"})
    };
    let actor_id = if target {
        format!("{scope}/targets/line%3A0%3A0")
    } else {
        you.clone()
    };
    let recipient = if target {
        LifeRecipient::Target {
            target_slot: bind_target(program, slots, "line:0:0", TargetDomain::AnyPlayer)?,
        }
    } else {
        LifeRecipient::Controller
    };
    let mut steps = vec![];
    let mut refs = vec![];
    let source = format!("card:{identity}/source#object");
    let face = format!("card:{identity}/faces/0#face");
    let mut nodes = vec![
        json!({"id":source,"path":"/source","node":"source","identity":identity}),
        json!({"id":face,"path":"/faces/0","node":"face","source":source,"sourceFaceIndex":0,"layout":"normal","manaCost":mana}),
        json!({"id":scope,"path":base,"node":"ability","source":source,"face":face,"abilityKind":"spell","you":you}),
        json!({"id":you,"path":base,"node":"runtime-player","scope":scope,"role":"spell-controller"}),
        json!({"id":x,"path":base,"node":"announced-number","scope":scope,"chooser":you,"announcement":"cast"}),
    ];
    for (i, &op) in operations.iter().enumerate() {
        let path = format!("{base}/effects/0/steps/{i}");
        let who = format!("{path}/who");
        let event = format!("card:{identity}{path}#event");
        refs.push(json!({"path":who,"expression":actor,"binding":"bound","target":actor_id}));
        if target && i == 0 {
            nodes.push(json!({"id":actor_id,"path":who,"node":"selection","scope":scope,"expression":actor,"cardinality":"one","timing":"stack-announcement","selection":"target"}));
        }
        let mut inputs = vec![actor_id.clone()];
        if op == "discard" {
            let what = json!({"ref":"filter","filter":{"cls":"card"},"count":{"amount":"fixed","value":1}});
            let path = format!("{path}/what");
            let query = format!("card:{identity}{path}#query");
            steps.push(json!({"effect":op,"who":actor,"what":what}));
            refs.push(json!({"path":path,"expression":what,"binding":"bound","target":query}));
            nodes.push(json!({"id":query,"path":path,"node":"selection","scope":scope,"expression":what,"cardinality":"one","timing":"resolution","selection":"evaluate-reference"}));
            inputs.insert(0, query);
        } else {
            let amount = json!({"amount":"x"});
            steps.push(json!({"effect":op,"who":actor,"amount":amount}));
            refs.push(json!({"path":format!("{path}/amount"),"expression":amount,"binding":"bound","target":x}));
        }
        nodes.push(json!({"id":event,"path":path,"node":"event","scope":scope,"operation":op,"inputs":inputs,"availability":"when-performed"}));
        if matches!(op, "draw" | "discard") {
            let (cardinality, identity_kind) = if op == "discard" {
                ("one", "zone-successors")
            } else {
                ("unknown", "new-objects")
            };
            nodes.push(json!({"id":format!("card:{identity}{path}#objects"),"path":path,"node":"event-result","event":event,"result":"objects","cardinality":cardinality,"availability":"if-produced","identity":identity_kind,"description":{"cls":"card"}}));
        }
        if op != "discard" {
            nodes.push(json!({"id":format!("card:{identity}{path}#quantity"),"path":path,"node":"event-result","event":event,"result":"quantity","cardinality":"one","availability":"if-produced","identity":"measured-quantity"}));
        }
        program.instructions.push(match op {
            "draw" => Instruction::DrawAnnouncedX {
                recipient: recipient.clone(),
                source_block: 0,
            },
            "discard" => Instruction::DiscardCards {
                recipient: None,
                count: 1.into(),
                source_block: 0,
            },
            "gain-life" | "lose-life" => Instruction::announced_x_life(
                recipient.kernel(Context::Spell).expect("spell recipient"),
                op == "gain-life",
                0,
            ),
            _ => unreachable!(),
        });
    }
    let abilities: Value = decode(raw, "abilities")?;
    let count = refs.len();
    if abilities != json!([{"kind":"spell","effects":[{"steps":steps}]}])
        || binding["identity"] != identity
        || binding["fullyBound"] != true
        || binding["referencesBound"] != true
        || binding["diagnostics"] != json!([])
        || binding["engineFeatures"] != json!([])
        || binding["counts"] != json!({"obligations":count,"bound":count,"unresolved":0})
        || !complete(&binding["nodes"], &nodes)
        || !complete(&binding["references"], &refs)
    {
        return Err(fail());
    }
    Ok(())
}

// Every expected semantic record occurs once. Rule citations and display metadata
// remain archive diagnostics.
fn complete(actual: &Value, expected: &[Value]) -> bool {
    actual.as_array().is_some_and(|rows| {
        rows.len() == expected.len()
            && expected.iter().all(|expected| {
                rows.iter()
                    .filter(|row| {
                        expected
                            .as_object()
                            .unwrap()
                            .iter()
                            .all(|(k, v)| row.get(k) == Some(v))
                    })
                    .count()
                    == 1
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    const MANIFEST: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
    fn synthetic_target_draw() -> String {
        const OPPORTUNITY: &str =
            include_str!("../../../tests/fixtures/1f544a9f-c238-4858-be19-d6cd7d023dcc.json");
        crate::tests::changed_source(OPPORTUNITY, |r| {
            let base = "/faces/0/abilities/0";
            let root = format!("card:{}{base}", r["oracleIdentity"].as_str().unwrap());
            let x = format!("{root}#announced-x");
            let text = "Target player draws X cards.";
            r["source"]["oracle_text"] = json!(text);
            r["source"]["mana_cost"] = json!("{X}{U}{U}");
            let result = &mut r["faces"][0]["result"];
            result["abilities"][0]["effects"][0]["steps"][0]["amount"] = json!({"amount":"x"});
            result["lines"][0]["ability"] = result["abilities"][0].clone();
            result["lines"][0]["text"] = json!(text);
            let binding = &mut r["binding"];
            binding["nodes"][1]["manaCost"] = json!("{X}{U}{U}");
            binding["nodes"][6]["cardinality"] = json!("unknown");
            binding["nodes"].as_array_mut().unwrap().push(json!({"id":x,"node":"announced-number","path":base,"scope":format!("{root}#ability"),"chooser":format!("{root}#you"),"announcement":"cast","rules":["107.3a","601.2b"]}));
            binding["references"].as_array_mut().unwrap().push(json!({"path":format!("{base}/effects/0/steps/0/amount"),"expression":{"amount":"x"},"binding":"bound","target":x,"rules":["107.3a"]}));
            binding["counts"] = json!({"obligations":2,"bound":2,"unresolved":0});
        })
    }

    fn witness(raw: &str, index: u64, hash: &str) {
        assert_eq!(sha256(raw.trim_end().as_bytes()), hash);
        let CompileResult::Lowered { definition: d } = compile(MANIFEST, raw) else {
            panic!("{:?}", compile(MANIFEST, raw))
        };
        assert_eq!(u64::from(d.provenance.source_index), index);
        assert!(d.x_mana_cost && d.capabilities.contains(&Capability::AnnouncedXDrawV1));
        let target = matches!(index, 1940 | 6121 | 23053);
        let recipient = if target {
            LifeRecipient::Target { target_slot: 0 }
        } else {
            LifeRecipient::Controller
        };
        let draw = Instruction::DrawAnnouncedX {
            recipient: recipient.clone(),
            source_block: 0,
        };
        let expected = match index {
            6121 => vec![
                draw,
                Instruction::announced_x_life(recipient.kernel(Context::Spell).unwrap(), false, 0),
            ],
            17136 => vec![
                Instruction::announced_x_life(recipient.kernel(Context::Spell).unwrap(), true, 0),
                draw,
            ],
            26764 => vec![
                draw,
                Instruction::DiscardCards {
                    recipient: None,
                    count: 1.into(),
                    source_block: 0,
                },
            ],
            _ => vec![draw],
        };
        let p = d.spell.as_ref().unwrap();
        assert_eq!(p.instructions, expected);
        assert_eq!(p.targets.len(), usize::from(target));
    }
    #[test]
    fn announced_x_draw() {
        const PINS: [(u64, &str); 6] = [
            (
                1940,
                "08baf2e4aec9fafcb5992947b92af238e22de9f90b58130e7ca1f238698f017a",
            ),
            (
                6121,
                "58a8241cd35acc6b584acbaf88ca1feaf7ac7a71aa401bb3c555b84ab372c0d2",
            ),
            (
                7787,
                "62aeafc30c18aec21e83fe20e71cb2afe27bcdb523029642b384f4463097c2b0",
            ),
            (
                17136,
                "c9161f5e15d6c72482d42b949597551473a033d2dc7b2a290cd0d980e6abe321",
            ),
            (
                23053,
                "2ada3989fe5ae23acced0b2f02061abbc8eb9a5b9a7b97c293b4525312602d43",
            ),
            (
                26764,
                "6759d08cb2f3c94056fe225b41494ac1694825630c396b8516dc0138605a4b50",
            ),
        ];
        let synthetic = synthetic_target_draw();
        let CompileResult::Lowered { definition } = compile(MANIFEST, &synthetic) else {
            panic!("{:?}", compile(MANIFEST, &synthetic));
        };
        assert_eq!(
            definition.spell.unwrap().instructions,
            [Instruction::DrawAnnouncedX {
                recipient: LifeRecipient::Target { target_slot: 0 },
                source_block: 0
            }]
        );
        for case in 0..10 {
            let bad = crate::tests::changed_source(&synthetic, |r| {
                let result = &mut r["faces"][0]["result"];
                let sentence = &mut result["abilities"][0]["effects"][0];
                match case {
                    0 => sentence["steps"][0]["who"]["of"] = Value::Null,
                    1 => sentence["steps"][0]["who"]["selectionId"] = json!("another"),
                    2 => sentence["condition"] = Value::Null,
                    3 => sentence["steps"][0]["amount"] = json!({"amount":"x","value":1}),
                    4 => sentence["bindings"] = json!([{"variable":"x","value":1}]),
                    5 => r["source"]["mana_cost"] = json!("{B}{B}"),
                    6 => r["source"]["mana_cost"] = json!("{X}{X}{B}{B}"),
                    7 => r["binding"]["references"][1]["target"] = json!("foreign"),
                    8 => r["binding"]["nodes"][8]["chooser"] = json!("foreign"),
                    _ => {
                        let altered = json!("Target player draws X cards and gains X life.");
                        result["lines"][0]["text"] = altered.clone();
                        r["source"]["oracle_text"] = altered;
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
        if let Some(path) = std::env::var_os("MTG_ANNOUNCED_X_DRAW_ARCHIVE_PROBE") {
            let rows = std::fs::read_to_string(path).unwrap();
            assert_eq!(rows.lines().count(), PINS.len());
            for (raw, (index, hash)) in rows.lines().zip(PINS) {
                witness(raw, index, hash);
            }
        }
    }
}
