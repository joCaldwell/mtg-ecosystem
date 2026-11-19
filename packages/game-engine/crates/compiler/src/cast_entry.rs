//! CR 400.7c, 603.4 and 608.3a. Bind the entered object, trigger controller
//! and historical cast independently of the effect’s resolution operands.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::bindings::Bindings;
use serde_json::{Value, json};

pub(super) fn validate_binding(
    d: &Definition,
    source: &Value,
    binding: &Value,
) -> Result<(), Failure> {
    if !d.capabilities.contains(&Capability::CastEntryConditionV1) {
        return Ok(());
    }
    // Definition validation has proved the exact printed source, raw AST,
    // single trigger and effect. Prove that the archive graph binds those roles.
    let optional = matches!(
        d.triggered[0].program.instructions.as_slice(),
        [Instruction::OptionalDrawCards { .. }]
    );
    let you = json!({"player":"you"});
    let mut graph = Bindings::new(&d.oracle_id, source);
    let base = "/faces/0/abilities/0";
    graph.ability(base, "triggered", "trigger-controller");
    let actor = graph.id(base, "you");
    let source = graph.id("/source", "object");
    let enters = format!("{base}/trigger");
    graph.reference(
        &format!("{enters}/what"),
        &json!({"ref":"self"}),
        &source,
        "201.5",
    );
    graph.event(base, &enters, "trigger:enters", &source, false);
    graph.result(&enters, "objects", "same-objects", None);
    let condition = format!("{base}/condition");
    graph.reference_rules(
        &format!("{condition}/what"),
        &json!({"ref":"it"}),
        &graph.id(&enters, "objects"),
        &["608.2k", "400.7j", "608.2h"],
    );
    graph.reference(&format!("{condition}/by"), &you, &actor, "109.5");
    let step = format!("{base}/effects/0/steps/0");
    if matches!(
        d.triggered[0].program.instructions.as_slice(),
        [Instruction::UntapControllerLands { .. }]
    ) {
        let path = format!("{step}/what");
        let what = json!({"ref":"all","filter":{"types":["land"],"control":"you"}});
        let scope = graph.id(base, "ability");
        let selection = graph.node(&path,"query","selection",json!({"scope":scope,"expression":what,
            "cardinality":"many","timing":"resolution","selection":"evaluate-reference","rules":["608.2h"]}));
        graph.reference(&format!("{path}/filter/control"), &you, &actor, "109.5");
        graph.reference(&path, &what, &selection, "608.2h");
        graph.event(base, &step, "untap", &selection, false);
    } else {
        graph.reference(&format!("{step}/who"), &you, &actor, "109.5");
        graph.event(base, &step, "draw", &actor, optional);
        graph.result(
            &step,
            "objects",
            "new-objects",
            Some(&json!({"cls":"card"})),
        );
        graph.result(&step, "quantity", "measured-quantity", None);
    }
    graph.validate_with_features(binding, &[json!({
        "feature":"event-history","path":condition,
        "message":"This condition asks about earlier events or how this object was cast; the engine keeps that record.",
        "rules":["603.4","607.2","608.2i","400.7d"]
    })], "requires the exact entry-result and caster bindings")
}

#[cfg(test)]
#[test]
fn synthetic_cast_entry_requires_the_same_object_controller_and_history() {
    let record = include_str!("../../../tests/fixtures/synthetic-cast-entry.json");
    let manifest = include_str!("../../../tests/fixtures/archive-manifest-current.json");
    let CompileResult::Lowered { definition } = compile(manifest, record) else {
        panic!("original cast-entry source must lower");
    };
    assert_eq!(
        definition.triggered[0].intervening_if,
        Some(InterveningCondition::SourceCastByController)
    );
    assert!(
        definition
            .capabilities
            .contains(&Capability::CastEntryConditionV1)
    );
    for mutation in [
        "different object",
        "different player",
        "missing condition",
        "extra qualification",
        "extra history",
    ] {
        let changed = crate::tests::changed_source(record, |r| match mutation {
            "different object" => {
                r["binding"]["references"][1]["target"] =
                    json!("card:synthetic-cast-entry/source#object")
            }
            "different player" => r["binding"]["nodes"][3]["role"] = json!("spell-controller"),
            "extra history" => {
                let features = r["binding"]["engineFeatures"].as_array_mut().unwrap();
                features.push(features[0].clone());
            }
            _ => {
                let ability = &mut r["faces"][0]["result"]["abilities"][0];
                if mutation == "missing condition" {
                    ability.as_object_mut().unwrap().remove("condition");
                } else {
                    ability["condition"]["from"] = json!({"zone":"hand"});
                }
                r["faces"][0]["result"]["lines"][0]["ability"] = ability.clone();
            }
        });
        assert!(
            !matches!(compile(manifest, &changed), CompileResult::Lowered { .. }),
            "{mutation}"
        );
    }
}

#[test]
#[cfg(test)]
fn land_untap_requires_the_resolution_set_and_its_trigger_controller() {
    let record = include_str!("../../../tests/fixtures/synthetic-cast-land-untap.json");
    let manifest = include_str!("../../../tests/fixtures/archive-manifest-current.json");
    let CompileResult::Lowered { definition } = compile(manifest, record) else {
        panic!("original land entry must lower")
    };
    assert!(
        definition
            .capabilities
            .contains(&Capability::UntapControllerLandsV1)
    );
    let mut forged_print = definition.clone();
    forged_print.provenance.printing_id.push_str("-other");
    assert!(forged_print.validate().is_err());
    for change_block in [false, true] {
        let mut forged_labels = definition.clone();
        let labels = forged_labels.source_labels.as_mut().unwrap();
        if change_block {
            labels.blocks[0].push_str(" altered");
        } else {
            labels.triggered.clear();
        }
        assert!(forged_labels.validate().is_err());
    }
    for (key, value) in [("timing", "stack-announcement"), ("cardinality", "one")] {
        let changed = crate::tests::changed_source(record, |r| {
            let node = r["binding"]["nodes"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|n| n["node"] == "selection")
                .unwrap();
            node[key] = json!(value);
        });
        assert!(!matches!(
            compile(manifest, &changed),
            CompileResult::Lowered { .. }
        ));
    }
    let changed = crate::tests::changed_source(record, |r| {
        let control = r["binding"]["references"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|r| r["path"].as_str().unwrap().ends_with("/filter/control"))
            .unwrap();
        control["target"] = json!("card:synthetic-cast-land-untap/source#object");
    });
    assert!(!matches!(
        compile(manifest, &changed),
        CompileResult::Lowered { .. }
    ));
}
