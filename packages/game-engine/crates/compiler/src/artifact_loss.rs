//! CR 104.3e, 113.7a, 601.2c/h, 602.2b, 614.1c, 701.21a.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::bindings::Bindings;
use serde_json::{Value, json};

const STATIC: &str = "/faces/0/abilities/0";
const ACTIVATED: &str = "/faces/0/abilities/1";

fn graph<'a>(identity: &'a str, source: &Value, selection: &str) -> Bindings<'a> {
    let mut g = Bindings::new(identity, source);
    let object = g.id("/source", "object");
    g.ability(STATIC, "static", "static-source-controller");
    g.node(&format!("{STATIC}/effect"), "entering-controller", "runtime-player",
        json!({"scope":g.id(STATIC,"ability"),"role":"entering-controller","rules":["110.2a","614.12"]}));
    let this = json!({"ref":"this","noun":"artifact"});
    g.reference(&format!("{STATIC}/effect/what"), &this, &object, "201.5");
    g.ability(ACTIVATED, "activated", "activator");
    let scope = g.id(ACTIVATED, "ability");
    let payer = g.id(ACTIVATED, "you");
    for index in 0..3 {
        g.reference(
            &format!("{ACTIVATED}/costs/{index}/payer"),
            &json!({"player":"you"}),
            &payer,
            "109.5",
        );
    }
    g.reference(
        &format!("{ACTIVATED}/costs/1/source"),
        &json!({"ref":"self"}),
        &object,
        "201.5",
    );
    let sacrifice = format!("{ACTIVATED}/costs/2");
    g.reference(&format!("{sacrifice}/what"), &this, &object, "201.5");
    let event = g.node(
        &sacrifice,
        "event",
        "event",
        json!({"scope":scope,"operation":"cost:sacrifice",
        "inputs":[],"availability":"when-performed","rules":["608.2c"]}),
    );
    g.node(
        &sacrifice,
        "quantity",
        "event-result",
        json!({"event":event,"result":"quantity","cardinality":"one",
        "availability":"if-produced","identity":"measured-quantity","rules":["608.2h"]}),
    );
    let step = format!("{ACTIVATED}/effects/0/steps/0");
    let target = g.target(
        &scope,
        &format!("{step}/who"),
        &json!({"player":"target-player","selectionId":selection}),
        selection,
    );
    g.node(
        &step,
        "event",
        "event",
        json!({"scope":scope,"operation":"lose-game","inputs":[target],
        "availability":"when-performed","rules":["608.2c"]}),
    );
    g
}

pub(super) fn lower(
    source: &Source,
    raw: &Value,
    result: &ParseResult,
    binding: &Value,
    abilities: &[Ability],
) -> Result<Option<ActivatedAbility>, Failure> {
    let Some(bound) = ArtifactLossSource::parse(&source.oracle_text) else {
        fn contains_loss(node: &Value) -> bool {
            match node {
                Value::Object(fields) => {
                    node["effect"] == "lose-game" || fields.values().any(contains_loss)
                }
                Value::Array(items) => items.iter().any(contains_loss),
                _ => false,
            }
        }
        if result.abilities.get().contains("lose-game")
            && contains_loss(&decode(result.abilities.get(), "abilities")?)
        {
            return Err(unsupported(
                "effects.lose-game",
                "requires a complete bound artifact activation",
            ));
        }
        return Ok(None);
    };
    let fail = || {
        unsupported(
            "activated.loss",
            "requires the complete bound artifact cost, entry and player loss",
        )
    };
    if source.type_line != "Artifact"
        || decode::<Value>(result.abilities.get(), "abilities")? != bound.abilities
    {
        return Err(fail());
    }
    let [_, Ability::Activated { costs, effects, .. }] = abilities else {
        return Err(fail());
    };
    let [Step::LoseGame { who }] = effects[0].steps.as_slice() else {
        return Err(fail());
    };
    if who.selection_id.as_deref() != Some(bound.selection.as_str()) {
        return Err(fail());
    }
    graph(&source.oracle_id, raw, &bound.selection)
        .validate(binding, "requires complete artifact loss binding graph")?;
    let costs = lower_permanent_costs(costs, CardKind::Artifact)?;
    if costs.mana.as_ref() != Some(&bound.mana) {
        return Err(fail());
    }
    Ok(Some(ActivatedAbility {
        functioning_zone: FunctioningZone::Battlefield,
        cost: ActivationCost::Composite { costs },
        program: SpellProgram {
            timing: SpellTiming::Instant,
            functioning_zone: FunctioningZone::Stack,
            resolution: SpellResolution::Instructions,
            additional_sacrifice: None,
            optional_additional_costs: vec![],
            modes: vec![],
            targets: vec![TargetSpec {
                slot: 0,
                selection_id: bound.selection,
                chooser: Chooser::Controller,
                stage: EvaluationStage::Announcement,
                domain: TargetDomain::AnyPlayer,
                filter: None,
            }],
            instructions: vec![Instruction::LoseGameTargetPlayer {
                target_slot: 0,
                source_block: 1,
            }],
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_artifact_loss_requires_cost_text_scope_and_exact_target_graph() {
        let id = "synthetic-activated-loss";
        let name = "Synthetic Final Bell";
        let subject = "this artifact";
        let text = format!(
            "This artifact enters tapped.\n{{2}}{{U}}, {{T}}, Sacrifice {subject}: Target player loses the game."
        );
        let source = json!({"id":"synthetic-print","oracle_id":id,"name":name,"layout":"normal",
            "type_line":"Artifact","mana_cost":"{2}","colors":[],"color_identity":["U"],"oracle_text":text});
        let bound = ArtifactLossSource::parse(&text).unwrap();
        assert_eq!(bound.selection, "line:1:9");
        let lines = text
            .lines()
            .zip(bound.abilities.as_array().unwrap())
            .map(|(text, ability)| json!({"text":text,"ability":ability,"ok":true}))
            .collect::<Vec<_>>();
        let record = json!({"recordType":"card","sourceIndex":0,"oracleIdentity":id,"source":source,"status":"parsed",
            "faces":[{"sourceFaceIndex":0,"status":"attempted","result":{"name":name,"ok":true,"lines":lines,"abilities":bound.abilities}}],
            "binding":graph(id,&source,&bound.selection).synthetic_envelope()});
        let base = canonical(&record).unwrap();
        let valid = crate::tests::changed_source(&base, |_| {});
        let CompileResult::Lowered { definition: d } = compile(fixtures::MANIFEST, &valid) else {
            panic!(
                "synthetic artifact loss source: {:?}",
                compile(fixtures::MANIFEST, &valid)
            )
        };
        assert!(artifact_loss_activation(&d.activated[0]));
        assert_eq!(d.source_labels.as_ref().unwrap().activated, [1]);
        for mutation in 0..7 {
            let near = crate::tests::changed_source(&base, |r| {
                let graph = &mut r["binding"];
                match mutation {
                    0 => {
                        graph["references"]
                            .as_array_mut()
                            .unwrap()
                            .iter_mut()
                            .find(|v| v["path"].as_str().unwrap().ends_with("costs/0/payer"))
                            .unwrap()["target"] = json!("foreign-activator")
                    }
                    1 => {
                        graph["nodes"]
                            .as_array_mut()
                            .unwrap()
                            .iter_mut()
                            .find(|v| v["node"] == "selection")
                            .unwrap()["timing"] = json!("resolution")
                    }
                    2 => {
                        graph["references"]
                            .as_array_mut()
                            .unwrap()
                            .iter_mut()
                            .find(|v| v["path"].as_str().unwrap().ends_with("costs/2/what"))
                            .unwrap()["target"] = json!("another-object")
                    }
                    3 => {
                        graph["nodes"]
                            .as_array_mut()
                            .unwrap()
                            .iter_mut()
                            .find(|v| v["operation"] == "lose-game")
                            .unwrap()["inputs"] = json!([])
                    }
                    4 => graph["references"]
                        .as_array_mut()
                        .unwrap()
                        .push(json!({"path":"invented"})),
                    5 => {
                        let parse = &mut r["faces"][0]["result"];
                        parse["abilities"][1]["costs"][0]["symbols"][0] = json!("1");
                        parse["lines"][1]["ability"] = parse["abilities"][1].clone();
                    }
                    _ => {
                        let labels = text.replace("Target player", "Target opponent");
                        r["source"]["oracle_text"] = json!(labels);
                        r["faces"][0]["result"]["lines"][1]["text"] =
                            json!(labels.lines().nth(1).unwrap());
                    }
                }
            });
            assert!(
                !matches!(
                    compile(fixtures::MANIFEST, &near),
                    CompileResult::Lowered { .. }
                ),
                "mutation {mutation}"
            );
        }
    }
}
