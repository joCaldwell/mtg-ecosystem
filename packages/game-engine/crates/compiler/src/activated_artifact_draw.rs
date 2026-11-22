//! CR 109.5, 201.5, 602.2, 608.2c, 121.2 and 701.9: the activator
//! pays and taps this source, then draws individually and discards as instructed.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const ABILITY: &str = "/faces/0/abilities/0";

pub(super) fn lower_two_draws(
    costs: &[SourceCost],
    effects: &[Sentence],
    block: usize,
    kind: CardKind,
) -> Result<Option<ActivatedAbility>, Failure> {
    if kind != CardKind::Artifact
        || block != 0
        || !matches!(costs, [SourceCost::Mana { .. }, SourceCost::TapSelf {}])
        || draw_discard::coalesce(effects).ok().flatten() != Some((2.into(), 1.into()))
    {
        return Ok(None);
    }
    let draw = Instruction::draw(PlayerExpr::you(Context::Activated), 1.into(), 0);
    Ok(Some(ActivatedAbility {
        functioning_zone: FunctioningZone::Battlefield,
        cost: ActivationCost::Composite {
            costs: lower_permanent_costs(costs, kind)?,
        },
        program: SpellProgram {
            additional_sacrifice: None,
            optional_additional_costs: vec![],
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            functioning_zone: FunctioningZone::Stack,
            targets: vec![],
            modes: vec![],
            instructions: vec![
                draw.clone(),
                draw,
                Instruction::DiscardCards {
                    recipient: None,
                    count: 1.into(),
                    source_block: 0,
                },
            ],
        },
    }))
}

pub(super) fn validate_source(binding: &Value, identity: &str, raw: &str) -> Result<(), Failure> {
    let fail = || unsupported("binding", "requires complete paid artifact draw closure");
    let abilities: Value = decode(raw, "abilities")?;
    let symbols = &abilities[0]["costs"][0]["symbols"];
    let steps = &abilities[0]["effects"][0]["steps"];
    let has_discard = steps.as_array().is_some_and(|steps| steps.len() == 2);
    let draw_count = match (&steps[0]["amount"]["value"], has_discard) {
        (value, _) if *value == 1 => 1,
        (value, true) if *value == 2 => 2,
        _ => return Err(fail()),
    };
    let player = json!({"player":"you"});
    let selected =
        json!({"ref":"filter","filter":{"cls":"card"},"count":{"amount":"fixed","value":1}});
    let mut expected_steps = vec![json!({
        "effect":"draw","who":player,"amount":{"amount":"fixed","value":draw_count}
    })];
    if has_discard {
        expected_steps.push(json!({"effect":"discard","who":player,"what":selected}));
    }
    // Typed lowering already checks the mana symbols. Compare the original
    // archive AST so nullable qualifiers and unconsumed clauses cannot vanish.
    if abilities
        != json!([{
            "kind":"activated",
            "costs":[{"cost":"mana","symbols":symbols},{"cost":"tap-self"}],
            "effects":[{"steps":expected_steps}]
        }])
    {
        return Err(fail());
    }
    let source = format!("card:{identity}/source#object");
    let root = format!("card:{identity}{ABILITY}");
    let ability = format!("{root}#ability");
    let you = format!("{root}#you");
    let step = format!("{ABILITY}/effects/0/steps/0");
    let event = format!("card:{identity}{step}#event");
    let discard = format!("{ABILITY}/effects/0/steps/1");
    let what = format!("{discard}/what");
    let query = format!("card:{identity}{what}#query");
    let self_reference = json!({"ref":"self"});
    let mut refs = vec![
        (format!("{ABILITY}/costs/0/payer"), &player, &you),
        (
            format!("{ABILITY}/costs/1/source"),
            &self_reference,
            &source,
        ),
        (format!("{ABILITY}/costs/1/payer"), &player, &you),
        (format!("{step}/who"), &player, &you),
    ];
    if has_discard {
        refs.extend([
            (format!("{discard}/who"), &player, &you),
            (what.clone(), &selected, &query),
        ]);
    }
    let expected_refs = refs.len();
    let actual = binding["references"].as_array().ok_or_else(fail)?;
    let nodes = binding["nodes"].as_array().ok_or_else(fail)?;
    if binding["identity"] != identity
        || binding["fullyBound"] != true
        || binding["referencesBound"] != true
        || binding["diagnostics"] != json!([])
        || binding["engineFeatures"] != json!([])
        || binding["counts"]
            != json!({"obligations":expected_refs,"bound":expected_refs,"unresolved":0})
        || actual.len() != expected_refs
        || nodes.len() != if has_discard { 10 } else { 7 }
        || refs.into_iter().any(|(path, expression, target)| {
            actual
                .iter()
                .filter(|reference| {
                    reference["path"] == path
                        && &reference["expression"] == expression
                        && reference["target"] == target.as_str()
                        && reference["binding"] == "bound"
                })
                .count()
                != 1
        })
        || !nodes.iter().any(|n| {
            n["id"] == source
                && n["path"] == "/source"
                && n["node"] == "source"
                && n["identity"] == identity
        })
        || !nodes.iter().any(|n| {
            n["id"] == ability
                && n["path"] == ABILITY
                && n["node"] == "ability"
                && n["abilityKind"] == "activated"
                && n["source"] == source
                && n["you"] == you
        })
        || !nodes.iter().any(|n| {
            n["id"] == you
                && n["path"] == ABILITY
                && n["node"] == "runtime-player"
                && n["scope"] == ability
                && n["role"] == "activator"
        })
        || !nodes.iter().any(|n| {
            n["id"] == event
                && n["path"] == step
                && n["node"] == "event"
                && n["scope"] == ability
                && n["operation"] == "draw"
                && n["inputs"] == json!([you])
        })
    {
        return Err(fail());
    }
    if has_discard {
        let face = format!("card:{identity}/faces/0#face");
        let discard_event = format!("card:{identity}{discard}#event");
        let cardinality = if draw_count == 1 { "one" } else { "many" };
        let expected = [
            json!({"id":face,"path":"/faces/0","node":"face","source":source,"sourceFaceIndex":0,"layout":"normal"}),
            json!({"id":ability,"face":face}),
            json!({"id":event,"availability":"when-performed"}),
            json!({"id":format!("card:{identity}{step}#objects"),"path":step,"node":"event-result","event":event,"result":"objects","cardinality":cardinality,"availability":"if-produced","identity":"new-objects","description":{"cls":"card"}}),
            json!({"id":format!("card:{identity}{step}#quantity"),"path":step,"node":"event-result","event":event,"result":"quantity","cardinality":"one","availability":"if-produced","identity":"measured-quantity"}),
            json!({"id":query,"path":what,"node":"selection","scope":ability,"expression":selected,"cardinality":"one","timing":"resolution","selection":"evaluate-reference"}),
            json!({"id":discard_event,"path":discard,"node":"event","scope":ability,"operation":"discard","inputs":[query,you],"availability":"when-performed"}),
            json!({"id":format!("card:{identity}{discard}#objects"),"path":discard,"node":"event-result","event":discard_event,"result":"objects","cardinality":"one","availability":"if-produced","identity":"zone-successors","description":{"cls":"card"}}),
        ];
        if expected.iter().any(|fields| {
            let fields = fields.as_object().unwrap();
            nodes
                .iter()
                .filter(|node| {
                    fields
                        .iter()
                        .all(|(key, value)| node.get(key) == Some(value))
                })
                .count()
                != 1
        }) {
            return Err(fail());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::tests::changed_source as changed;
    use super::*;

    const MANIFEST: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
    const ID: &str = "11111111-0000-4000-8000-111111111111";

    fn synthetic_record() -> String {
        let name = "Cloud Archive Capsule";
        let text = "{2}{U}, {T}: Draw a card.";
        let source = format!("card:{ID}/source#object");
        let prefix = format!("card:{ID}{ABILITY}");
        let ability = format!("{prefix}#ability");
        let activator = format!("{prefix}#you");
        let path = format!("{ABILITY}/effects/0/steps/0");
        let event = format!("card:{ID}{path}#event");
        let payer = |path: String| json!({"path":path,"expression":{"player":"you"},"binding":"bound","target":activator});
        let effect = json!({
            "kind":"activated","costs":[{"cost":"mana","symbols":["2","U"]},{"cost":"tap-self"}],
            "effects":[{"steps":[{"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1}}]}]
        });
        json!({
            "recordType":"card","sourceIndex":0,"oracleIdentity":ID,"status":"parsed",
            "source":{
                "id":"22222222-0000-4000-8000-222222222222","oracle_id":ID,"name":name,
                "layout":"normal","oracle_text":text,"mana_cost":"{1}","type_line":"Artifact",
                "colors":[],"color_identity":["U"]
            },
            "faces":[{"sourceFaceIndex":0,"status":"attempted","result":{
                "name":name,"ok":true,"abilities":[effect],
                "lines":[{"text":text,"ok":true,"ability":effect}]
            }}],
            "binding":{
                "identity":ID,"fullyBound":true,"referencesBound":true,"diagnostics":[],"engineFeatures":[],
                "counts":{"obligations":4,"bound":4,"unresolved":0},
                "nodes":[
                    {"id":source,"path":"/source","node":"source","identity":ID},
                    {"node":"face"},
                    {"id":ability,"path":ABILITY,"node":"ability","abilityKind":"activated","source":source,"you":activator},
                    {"id":activator,"path":ABILITY,"node":"runtime-player","scope":ability,"role":"activator"},
                    {"id":event,"path":path,"node":"event","scope":ability,"operation":"draw","inputs":[activator]},
                    {"node":"event-result","event":event,"result":"objects"},
                    {"node":"event-result","event":event,"result":"quantity"}
                ],
                "references":[
                    payer(format!("{ABILITY}/costs/0/payer")),
                    {"path":format!("{ABILITY}/costs/1/source"),"expression":{"ref":"self"},"binding":"bound","target":source},
                    payer(format!("{ABILITY}/costs/1/payer")),
                    payer(format!("{path}/who"))
                ]
            }
        })
        .to_string()
    }

    fn assert_activation(raw: &str, generic: i64, colored: Vec<Color>) -> Box<Definition> {
        let result = compile(MANIFEST, raw);
        let CompileResult::Lowered { definition } = result else {
            panic!("{result:?}");
        };
        let [activation] = definition.activated.as_slice() else {
            panic!("expected exactly one activation");
        };
        assert!(mtg_contracts::activated_artifact_draw_ability(activation));
        assert_eq!(
            activation.cost,
            ActivationCost::Composite {
                costs: PermanentActivationCosts {
                    mana: Some(ManaCost {
                        generic: generic.into(),
                        colored,
                    }),
                    tap: true,
                    sacrifice: false,
                    remove_source_plus_one_counters: None,
                },
            }
        );
        assert!(
            definition
                .capabilities
                .contains(&Capability::ActivatedArtifactDrawV1)
        );
        definition
    }

    #[test]
    fn paid_artifact_draw_compiles_complete_source_only() {
        let synthetic = synthetic_record();
        assert_activation(&changed(&synthetic, |_| {}), 2, vec![Color::U]);
        let qualified = changed(&synthetic, |record| {
            let result = &mut record["faces"][0]["result"];
            result["abilities"][0]["effects"][0]["steps"][0]["who"]["of"] = Value::Null;
            result["lines"][0]["ability"] = result["abilities"][0].clone();
        });
        let wrong_target = changed(&synthetic, |record| {
            record["binding"]["references"][1]["target"] =
                record["binding"]["references"][0]["target"].clone();
        });
        let extra_ref = changed(&synthetic, |record| {
            let refs = record["binding"]["references"].as_array_mut().unwrap();
            refs.push(refs[0].clone());
        });
        for bad in [qualified, wrong_target, extra_ref] {
            let result = compile(MANIFEST, &bad);
            assert!(
                matches!(&result, CompileResult::Unsupported { path, .. } if path == "binding"),
                "{result:?}"
            );
        }

        if let Some(path) = std::env::var_os("MTG_ACTIVATED_DRAW_ARCHIVE_PROBE") {
            let content = std::fs::read_to_string(path).expect("read exact archive probe JSONL");
            let records: Vec<_> = content.lines().collect();
            assert_eq!(records.len(), 2);
            for (raw, (index, cost)) in records.into_iter().zip([(8676, 4), (32788, 3)]) {
                let record: Value = decode(raw, "probe").unwrap();
                assert_eq!(record["sourceIndex"], index);
                assert_activation(raw, cost, vec![]);
            }
        }
    }

    fn synthetic_draw_discard(draw_count: u8) -> String {
        changed(&synthetic_record(), |record| {
            let text = match draw_count {
                1 => "{2}{U}, {T}: Draw a card, then discard a card.",
                2 => "{2}{U}, {T}: Draw two cards, then discard a card.",
                _ => unreachable!(),
            };
            let what = json!({"ref":"filter","filter":{"cls":"card"},"count":{"amount":"fixed","value":1}});
            record["source"]["oracle_text"] = json!(text);
            let result = &mut record["faces"][0]["result"];
            let steps = result["abilities"][0]["effects"][0]["steps"]
                .as_array_mut()
                .unwrap();
            steps[0]["amount"]["value"] = json!(draw_count);
            steps.push(json!({"effect":"discard","who":{"player":"you"},"what":what}));
            result["lines"][0]["text"] = json!(text);
            result["lines"][0]["ability"] = result["abilities"][0].clone();

            let binding = &mut record["binding"];
            let source = binding["nodes"][0]["id"].clone();
            let ability = binding["nodes"][2]["id"].clone();
            let you = binding["nodes"][3]["id"].clone();
            let draw = format!("{ABILITY}/effects/0/steps/0");
            let draw_event = format!("card:{ID}{draw}#event");
            let discard = format!("{ABILITY}/effects/0/steps/1");
            let discard_event = format!("card:{ID}{discard}#event");
            let query = format!("card:{ID}{discard}/what#query");
            let face = format!("card:{ID}/faces/0#face");
            let nodes = binding["nodes"].as_array_mut().unwrap();
            nodes[1] = json!({"id":face,"path":"/faces/0","node":"face","source":source,"sourceFaceIndex":0,"layout":"normal"});
            nodes[2]["face"] = json!(face);
            nodes[4]["availability"] = json!("when-performed");
            nodes[5] = json!({
                "id":format!("card:{ID}{draw}#objects"),"path":draw,"node":"event-result","event":draw_event,
                "result":"objects","cardinality":if draw_count == 1 {"one"} else {"many"},
                "availability":"if-produced","identity":"new-objects","description":{"cls":"card"}
            });
            nodes[6] = json!({
                "id":format!("card:{ID}{draw}#quantity"),"path":draw,"node":"event-result","event":draw_event,
                "result":"quantity","cardinality":"one","availability":"if-produced","identity":"measured-quantity"
            });
            nodes.extend([
                json!({"id":query,"path":format!("{discard}/what"),"node":"selection","scope":ability,"expression":what,
                    "cardinality":"one","timing":"resolution","selection":"evaluate-reference"}),
                json!({"id":discard_event,"path":discard,"node":"event","scope":ability,"operation":"discard",
                    "inputs":[query,you],"availability":"when-performed"}),
                json!({"id":format!("card:{ID}{discard}#objects"),"path":discard,"node":"event-result","event":discard_event,
                    "result":"objects","cardinality":"one","availability":"if-produced","identity":"zone-successors","description":{"cls":"card"}}),
            ]);
            binding["counts"] = json!({"obligations":6,"bound":6,"unresolved":0});
            binding["references"].as_array_mut().unwrap().extend([
                json!({"path":format!("{discard}/who"),"expression":{"player":"you"},"binding":"bound","target":you}),
                json!({"path":format!("{discard}/what"),"expression":what,"binding":"bound","target":query}),
            ]);
        })
    }

    fn assert_draw_discard(raw: &str, cost: i64, colored: Vec<Color>, draw_count: usize) {
        let definition = assert_activation(raw, cost, colored);
        assert!(
            definition
                .capabilities
                .contains(&Capability::ControllerDiscardV1)
        );
        let mut expected =
            vec![Instruction::draw(PlayerExpr::you(Context::Activated), 1.into(), 0); draw_count];
        expected.push(Instruction::DiscardCards {
            count: 1.into(),
            recipient: None,
            source_block: 0,
        });
        assert_eq!(definition.activated[0].program.instructions, expected);
    }

    #[test]
    fn paid_artifact_draw_discard_compiles_complete_source_only() {
        assert_draw_discard(&synthetic_draw_discard(1), 2, vec![Color::U], 1);
        let synthetic = synthetic_draw_discard(2);
        assert_draw_discard(&synthetic, 2, vec![Color::U], 2);
        let wrong_actor = changed(&synthetic, |record| {
            let result = &mut record["faces"][0]["result"];
            result["abilities"][0]["effects"][0]["steps"][1]["who"]["of"] = Value::Null;
            result["lines"][0]["ability"] = result["abilities"][0].clone();
        });
        let wrong_what = changed(&synthetic, |record| {
            let result = &mut record["faces"][0]["result"];
            result["abilities"][0]["effects"][0]["steps"][1]["what"]["count"]["value"] = json!(2);
            result["lines"][0]["ability"] = result["abilities"][0].clone();
        });
        let wrong_target = changed(&synthetic, |record| {
            record["binding"]["references"][5]["target"] =
                record["binding"]["references"][4]["target"].clone();
        });
        let wrong_query = changed(&synthetic, |record| {
            record["binding"]["nodes"][7]["timing"] = json!("announcement");
        });
        let pure_two = changed(&synthetic, |record| {
            let result = &mut record["faces"][0]["result"];
            result["abilities"][0]["effects"][0]["steps"]
                .as_array_mut()
                .unwrap()
                .pop();
            result["lines"][0]["ability"] = result["abilities"][0].clone();
        });
        for bad in [wrong_actor, wrong_what, wrong_target, wrong_query, pure_two] {
            let result = compile(MANIFEST, &bad);
            assert!(
                matches!(&result, CompileResult::Unsupported { .. }),
                "{result:?}"
            );
        }
        if let Some(path) = std::env::var_os("MTG_ACTIVATED_DISCARD_ARCHIVE_PROBE") {
            let content = std::fs::read_to_string(path).expect("read exact archive probe JSONL");
            let records: Vec<_> = content.lines().collect();
            assert_eq!(records.len(), 2);
            for (raw, (index, cost, draws)) in
                records.into_iter().zip([(7565, 2, 1), (23433, 5, 2)])
            {
                let record: Value = decode(raw, "probe").unwrap();
                assert_eq!(record["sourceIndex"], index);
                assert_draw_discard(raw, cost, vec![], draws);
            }
        }
    }
}
