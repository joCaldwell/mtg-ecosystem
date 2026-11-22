//! CR 109.5, 201.5, 602.2, 605.1a and 121.2: the activator pays the
//! source's fixed costs, then draws the specified cards one at a time.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(crate) fn lower(
    costs: &[SourceCost],
    effects: &[Sentence],
    block: usize,
    kind: CardKind,
) -> Result<Option<ActivatedAbility>, Failure> {
    let [
        Sentence {
            bindings,
            steps,
            condition: None,
            condition_kind: None,
        },
    ] = effects
    else {
        return Ok(None);
    };
    let [
        Step::Draw {
            who,
            amount: DrawAmount::Fixed { value },
            optional: false,
        },
    ] = steps.as_slice()
    else {
        return Ok(None);
    };
    if *value <= 1 {
        return Ok(None);
    }
    if !bindings.is_empty() || who.player != "you" || who.selection_id.is_some() || who.of.is_some()
    {
        return Err(unsupported(
            "abilities.effects",
            "requires an unqualified counted draw for the activator",
        ));
    }
    let costs = lower_permanent_costs(costs, kind)?;
    let Some(mana) = &costs.mana else {
        return Err(unsupported(
            "cost",
            "counted draw requires a fixed mana cost",
        ));
    };
    let cost = if costs.tap || costs.sacrifice {
        ActivationCost::Composite { costs }
    } else {
        ActivationCost::Mana { cost: mana.clone() }
    };
    Ok(Some(ActivatedAbility {
        functioning_zone: FunctioningZone::Battlefield,
        cost,
        program: SpellProgram {
            additional_sacrifice: None,
            optional_additional_costs: vec![],
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            functioning_zone: FunctioningZone::Stack,
            targets: vec![],
            instructions: vec![Instruction::draw(
                PlayerExpr::you(Context::Activated),
                i64::from(*value).into(),
                source_block(block)?,
            )],
            modes: vec![],
        },
    }))
}

pub(crate) fn validate_source(
    definition: &Definition,
    labels: &SourceLabels,
    binding: &Value,
    raw: &str,
) -> Result<(), Failure> {
    let Some(draw_block) =
        definition
            .activated
            .iter()
            .find_map(|a| match a.program.instructions.as_slice() {
                [draw] => draw.counted_controller_draw().map(|(_, block)| block),
                _ => None,
            })
    else {
        return Ok(());
    };
    let fail = || unsupported("binding", "requires complete paid counted draw source");
    let noun = match definition.characteristics.kind {
        CardKind::Artifact => "artifact",
        CardKind::Creature => "creature",
        CardKind::Enchantment => "enchantment",
        CardKind::Land => "land",
        _ => return Err(fail()),
    };
    let identity = &definition.oracle_id;
    let source = format!("card:{identity}/source#object");
    let face = format!("card:{identity}/faces/0#face");
    let player = json!({"player":"you"});
    let this = json!({"ref":"this","noun":noun});
    let raw: Vec<Value> = decode(raw, "abilities")?;
    let mut nodes = vec![
        json!({"id":source,"path":"/source","node":"source","identity":identity}),
        json!({"id":face,"path":"/faces/0","node":"face","source":source,"sourceFaceIndex":0,"layout":"normal"}),
    ];
    let mut references = vec![];
    let mut reference = |path: String, expression: &Value, target: &str| {
        references
            .push(json!({"path":path,"expression":expression,"target":target,"binding":"bound"}));
    };
    for (ordinal, block) in raw.iter().enumerate() {
        let ordinal = source_block(ordinal)?;
        let path = format!("/faces/0/abilities/{ordinal}");
        let ability = format!("card:{identity}{path}#ability");
        let you = format!("card:{identity}{path}#you");
        let mana_block = labels.mana.contains(&ordinal);
        let enters_tapped = labels.statics.contains(&ordinal);
        let (kind, role) = if enters_tapped {
            ("static", "static-source-controller")
        } else {
            ("activated", "activator")
        };
        nodes.extend([
            json!({"id":ability,"path":path,"node":"ability","source":source,"face":face,"abilityKind":kind,"you":you}),
            json!({"id":you,"path":path,"node":"runtime-player","scope":ability,"role":role}),
        ]);
        if enters_tapped {
            if noun != "land"
                || *block
                    != json!({"kind":"static","effect":{"static":"enters-tapped","what":this}})
            {
                return Err(fail());
            }
            let effect = format!("{path}/effect");
            nodes.push(json!({"id":format!("card:{identity}{effect}#entering-controller"),"path":effect,"node":"runtime-player","scope":ability,"role":"entering-controller"}));
            reference(format!("{effect}/what"), &this, &source);
            continue;
        }
        if ordinal != draw_block && !mana_block {
            return Err(fail());
        }
        let costs = block["costs"].as_array().ok_or_else(fail)?;
        let raw_step = &block["effects"][0]["steps"][0];
        let (operation, step) = if mana_block {
            if costs.as_slice() != [json!({"cost":"tap-self"})] {
                return Err(fail());
            }
            let symbols = &raw_step["mana"]["symbols"];
            (
                "add-mana",
                json!({"effect":"add-mana","who":player,"mana":{"mana":"fixed","symbols":symbols}}),
            )
        } else {
            let value = &raw_step["amount"]["value"];
            (
                "draw",
                json!({"effect":"draw","who":player,"amount":{"amount":"fixed","value":value}}),
            )
        };
        if *block != json!({"kind":"activated","costs":costs,"effects":[{"steps":[step]}]}) {
            return Err(fail());
        }
        for (index, cost) in costs.iter().enumerate() {
            let path = format!("{path}/costs/{index}");
            reference(format!("{path}/payer"), &player, &you);
            match cost["cost"].as_str() {
                Some("mana") if *cost == json!({"cost":"mana","symbols":cost["symbols"]}) => {}
                Some("tap-self") if *cost == json!({"cost":"tap-self"}) => {
                    reference(format!("{path}/source"), &json!({"ref":"self"}), &source);
                }
                Some("sacrifice") if *cost == json!({"cost":"sacrifice","what":this}) => {
                    reference(format!("{path}/what"), &this, &source);
                    append_event(&mut nodes, identity, &path, &ability, "cost:sacrifice", &[]);
                }
                _ => return Err(fail()),
            }
        }
        let step = format!("{path}/effects/0/steps/0");
        reference(format!("{step}/who"), &player, &you);
        append_event(&mut nodes, identity, &step, &ability, operation, &[&you]);
    }
    // The archive supplies descriptive fields and rule citations as metadata.
    // Check every semantic node and reference exactly once by its expected fields.
    let matches_fields = |actual: &Value, expected: Vec<Value>| {
        actual.as_array().is_some_and(|actual| {
            actual.len() == expected.len()
                && expected.iter().all(|fields| {
                    actual
                        .iter()
                        .filter(|item| {
                            fields
                                .as_object()
                                .unwrap()
                                .iter()
                                .all(|(key, value)| item.get(key) == Some(value))
                        })
                        .count()
                        == 1
                })
        })
    };
    let count = references.len();
    if binding["identity"] != identity.as_str()
        || binding["fullyBound"] != true
        || binding["referencesBound"] != true
        || binding["diagnostics"] != json!([])
        || binding["engineFeatures"] != json!([])
        || binding["counts"] != json!({"obligations":count,"bound":count,"unresolved":0})
        || !matches_fields(&binding["nodes"], nodes)
        || !matches_fields(&binding["references"], references)
    {
        return Err(fail());
    }
    Ok(())
}

fn append_event(
    nodes: &mut Vec<Value>,
    identity: &str,
    path: &str,
    ability: &str,
    operation: &str,
    inputs: &[&str],
) {
    let event = format!("card:{identity}{path}#event");
    nodes.push(json!({"id":event,"path":path,"node":"event","scope":ability,"operation":operation,"inputs":inputs,"availability":"when-performed"}));
    if operation == "add-mana" {
        return;
    }
    nodes.push(json!({"id":format!("card:{identity}{path}#quantity"),"path":path,"node":"event-result","event":event,"result":"quantity","cardinality":"one","availability":"if-produced","identity":"measured-quantity"}));
    if operation == "draw" {
        nodes.push(json!({"id":format!("card:{identity}{path}#objects"),"path":path,"node":"event-result","event":event,"result":"objects","cardinality":"many","availability":"if-produced","identity":"new-objects","description":{"cls":"card"}}));
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::changed_source;
    use super::*;

    const MANIFEST: &str = include_str!("../../../../tests/fixtures/archive-manifest-current.json");

    fn synthetic_record() -> String {
        let identity = "11111111-0000-4000-8000-111111111111";
        let source = format!("card:{identity}/source#object");
        let name = "Archive Researcher";
        let text = "{7}: Draw two cards.";
        let path = "/faces/0/abilities/0";
        let ability = format!("card:{identity}{path}#ability");
        let you = format!("card:{identity}{path}#you");
        let face = format!("card:{identity}/faces/0#face");
        let step = format!("{path}/effects/0/steps/0");
        let ast = json!({
            "kind":"activated","costs":[{"cost":"mana","symbols":["7"]}],
            "effects":[{"steps":[{"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":2}}]}]
        });
        let mut nodes = vec![
            json!({"id":source,"path":"/source","node":"source","identity":identity}),
            json!({"id":face,"path":"/faces/0","node":"face","source":source,"sourceFaceIndex":0,"layout":"normal"}),
            json!({"id":ability,"path":path,"node":"ability","source":source,"face":face,"abilityKind":"activated","you":you}),
            json!({"id":you,"path":path,"node":"runtime-player","scope":ability,"role":"activator"}),
        ];
        append_event(&mut nodes, identity, &step, &ability, "draw", &[&you]);
        json!({
            "recordType":"card","sourceIndex":0,"oracleIdentity":identity,"status":"parsed",
            "source":{
                "id":"22222222-0000-4000-8000-222222222222","oracle_id":identity,"name":name,
                "layout":"normal","oracle_text":text,"mana_cost":"{2}{U}","type_line":"Creature — Human Wizard",
                "colors":["U"],"color_identity":["U"],"power":"2","toughness":"3"
            },
            "faces":[{"sourceFaceIndex":0,"status":"attempted","result":{
                "name":name,"ok":true,"abilities":[ast],"lines":[{"text":text,"ok":true,"ability":ast}]
            }}],
            "binding":{
                "identity":identity,"fullyBound":true,"referencesBound":true,"diagnostics":[],"engineFeatures":[],
                "counts":{"obligations":2,"bound":2,"unresolved":0},
                "nodes":nodes,
                "references":[
                    {"path":format!("{path}/costs/0/payer"),"expression":{"player":"you"},"binding":"bound","target":you},
                    {"path":format!("{step}/who"),"expression":{"player":"you"},"binding":"bound","target":you}
                ]
            }
        })
        .to_string()
    }

    fn assert_activation(
        raw: &str,
        count: i64,
        source_block: u32,
        expected_cost: ActivationCost,
    ) -> Box<Definition> {
        let result = compile(MANIFEST, raw);
        let CompileResult::Lowered { definition } = result else {
            panic!("{result:?}");
        };
        let [activation] = definition.activated.as_slice() else {
            panic!("expected exactly one activation");
        };
        assert_eq!(activation.cost, expected_cost);
        assert_eq!(
            activation.program.instructions,
            vec![Instruction::draw(
                PlayerExpr::you(Context::Activated),
                count.into(),
                source_block
            )]
        );
        assert_eq!(
            definition.source_labels.as_ref().unwrap().activated,
            [source_block]
        );
        definition
    }

    fn cost(generic: i64, blue: usize, tap: bool, sacrifice: bool) -> ActivationCost {
        let cost = ManaCost {
            generic: generic.into(),
            colored: vec![Color::U; blue],
        };
        if tap || sacrifice {
            ActivationCost::Composite {
                costs: PermanentActivationCosts {
                    mana: Some(cost),
                    tap,
                    sacrifice,
                    remove_source_plus_one_counters: None,
                },
            }
        } else {
            ActivationCost::Mana { cost }
        }
    }

    #[test]
    fn paid_counted_draw_compiles_complete_source_only() {
        let synthetic = changed_source(&synthetic_record(), |_| {});
        assert_activation(&synthetic, 2, 0, cost(7, 0, false, false));
        let qualified = changed_source(&synthetic, |record| {
            let result = &mut record["faces"][0]["result"];
            result["abilities"][0]["effects"][0]["steps"][0]["who"]["of"] = Value::Null;
            result["lines"][0]["ability"] = result["abilities"][0].clone();
        });
        let wrong_target = changed_source(&synthetic, |record| {
            record["binding"]["references"][1]["target"] =
                record["binding"]["nodes"][0]["id"].clone();
        });
        for rejected in [qualified, wrong_target] {
            let result = compile(MANIFEST, &rejected);
            assert!(
                matches!(&result, CompileResult::Unsupported { path, .. } if path == "binding"),
                "{result:?}"
            );
        }

        if let Some(path) = std::env::var_os("MTG_PAID_COUNTED_DRAW_ARCHIVE_PROBE") {
            let content = std::fs::read_to_string(path).expect("read exact archive probe JSONL");
            let expected = [
                (2129, 3, 1, cost(3, 0, true, true), Some(0), None),
                (7467, 2, 1, cost(2, 0, true, true), Some(0), None),
                (11258, 2, 0, cost(1, 1, false, true), None, None),
                (11602, 2, 0, cost(6, 1, false, false), None, None),
                (11640, 2, 1, cost(5, 1, true, true), Some(0), None),
                (11770, 2, 0, cost(1, 1, true, true), None, None),
                (14434, 2, 0, cost(7, 0, false, false), None, None),
                (15278, 3, 0, cost(6, 1, false, true), None, None),
                (19481, 2, 2, cost(4, 1, true, true), Some(1), Some(0)),
                (22642, 3, 0, cost(7, 1, true, true), None, None),
                (27624, 2, 0, cost(3, 2, false, false), None, None),
                (33734, 4, 0, cost(8, 0, true, false), None, None),
            ];
            assert_eq!(content.lines().count(), expected.len());
            for (raw, (index, count, block, cost, mana, statics)) in content.lines().zip(expected) {
                let record: Value = decode(raw, "probe").unwrap();
                assert_eq!(record["sourceIndex"], index);
                let definition = assert_activation(raw, count, block, cost);
                let labels = definition.source_labels.as_ref().unwrap();
                assert!(labels.mana.iter().copied().eq(mana));
                assert!(labels.statics.iter().copied().eq(statics));
                assert!(
                    definition
                        .mana_abilities
                        .iter()
                        .map(|a| a.source_block)
                        .eq(mana)
                );
            }
        }
    }
}
