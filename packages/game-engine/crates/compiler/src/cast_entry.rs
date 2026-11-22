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
    let gated = d.capabilities.contains(&Capability::CastEntryConditionV1);
    if !gated && !has_fixed_creature_activation(d) {
        return Ok(());
    }
    if !gated
        && d.keywords
            .iter()
            .any(|k| k.basic_landwalk_type().is_some() || k.fixed_protection_color().is_some())
    {
        for (index, line) in source["oracle_text"]
            .as_str()
            .unwrap_or("")
            .lines()
            .enumerate()
        {
            let block = source_block(index)?;
            if let Some(spec) = fixed_creature_activation(line, &d.name, block) {
                let mut graph = Bindings::new(&d.oracle_id, source);
                bind_activation(&mut graph, block, &spec);
                graph.validate_block(
                    binding,
                    &format!("/faces/0/abilities/{index}"),
                    "requires exact fixed activation block",
                )?;
            }
        }
        return Ok(());
    }
    let mut graph = Bindings::new(&d.oracle_id, source);
    let mut bound_fixed = false;
    for (index, line) in source["oracle_text"]
        .as_str()
        .unwrap_or("")
        .lines()
        .enumerate()
    {
        let block = source_block(index)?;
        if let Some(spec) = fixed_creature_activation(line, &d.name, block) {
            bind_activation(&mut graph, block, &spec);
            bound_fixed = true;
        }
    }
    let keyword_block = bound_fixed && !d.keywords.is_empty();
    if keyword_block {
        let base = "/faces/0/abilities/0";
        graph.ability(base, "keywords", "static-source-controller");
        for (index, keyword) in d.keywords.iter().enumerate() {
            let (word, rule) = bound_fixed_activation_keyword(d, *keyword).ok_or_else(|| {
                unsupported("keywords", "unsupported fixed activation keyword binding")
            })?;
            bind_keyword(&mut graph, base, index, *keyword, word, rule);
        }
    }
    if !gated && bound_fixed {
        return graph.validate(binding, "requires exact fixed activation roles");
    }
    if !gated {
        return Ok(());
    }
    // Definition validation has proved the exact printed source, raw AST,
    // single trigger and effect. Prove that the archive graph binds those roles.
    let optional = matches!(
        d.triggered[0].program.instructions.as_slice(),
        [Instruction::OptionalDrawCards { .. }]
    );
    let you = json!({"player":"you"});
    let base = if keyword_block {
        "/faces/0/abilities/1"
    } else {
        "/faces/0/abilities/0"
    };
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
        [kernel!(Untap {
            what: kir::SetExpr::Matching { .. }
        })]
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

pub(super) fn bind_keyword(
    graph: &mut Bindings<'_>,
    base: &str,
    index: usize,
    keyword: KeywordAbility,
    word: &str,
    rule: &str,
) {
    let scope = graph.id(base, "ability");
    let path = format!("{base}/keywords/{index}");
    let role = if keyword == KeywordAbility::Flash {
        "prospective-caster"
    } else {
        "static-source-controller"
    };
    let actor = graph.node(
        &path,
        "you",
        "runtime-player",
        json!({"scope":scope,"role":role,"rules":["109.5",rule]}),
    );
    graph.node(
        &path,
        "keyword",
        "keyword-ability",
        json!({"scope":scope,"keyword":word,"abilities":["static"],"you":actor,"rules":[rule]}),
    );
}

pub(super) fn bind_activation(
    graph: &mut Bindings<'_>,
    block: u32,
    spec: &FixedCreatureActivation,
) {
    let base = format!("/faces/0/abilities/{block}");
    graph.ability(&base, "activated", "activator");
    let scope = graph.id(&base, "ability");
    let actor = graph.id(&base, "you");
    let you = json!({"player":"you"});
    graph.reference(&format!("{base}/costs/0/payer"), &you, &actor, "109.5");
    let path = format!("{base}/effects/0/steps/0");
    let step = &spec.ast["effects"][0]["steps"][0];
    let operation = step["effect"]
        .as_str()
        .expect("constructed activation has an operation");
    let mut inputs = Vec::new();
    if operation == "damage" {
        let source = graph.id("/source", "object");
        graph.reference(&format!("{path}/source"), &step["source"], &source, "201.5");
        inputs.push(source);
    }
    if let Some(target) = spec.ability.program.targets.first() {
        let (suffix, expression) = if operation == "damage" {
            ("to/ref", &step["to"]["ref"])
        } else {
            ("what", &step["what"])
        };
        inputs.push(graph.target(
            &scope,
            &format!("{path}/{suffix}"),
            expression,
            &target.selection_id,
        ));
    } else if matches!(operation, "pump" | "regenerate") {
        let source = graph.id("/source", "object");
        graph.reference(&format!("{path}/what"), &step["what"], &source, "201.5");
        inputs.push(source);
    } else {
        graph.reference(&format!("{path}/who"), &you, &actor, "109.5");
        inputs.push(actor);
    }
    graph.node(&path,"event","event",json!({"scope":scope,"operation":operation,"inputs":inputs,"availability":"when-performed","rules":["608.2c"]}));
    if matches!(operation, "damage" | "gain-life") {
        graph.result(&path, "quantity", "measured-quantity", None);
    }
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

#[test]
#[cfg(test)]
fn fixed_activations_match_paid_cost_targets_and_entry_history() {
    let manifest = include_str!("../../../tests/fixtures/archive-manifest-current.json");
    let record = include_str!("../../../tests/fixtures/synthetic-fixed-activations.json");
    let result = compile(manifest, record);
    let CompileResult::Lowered { definition } = result else {
        panic!("original composite source: {result:?}")
    };
    assert_eq!(definition.activated.len(), 3);
    for change in [
        "cost",
        "target",
        "omit",
        "block",
        "instruction",
        "type_line",
    ] {
        let mut bad = definition.clone();
        match change {
            "cost" => bad.activated[0].cost = bad.activated[1].cost.clone(),
            "target" => bad.activated[1].program.targets[0].domain = TargetDomain::AnyDamageTarget,
            "omit" => {
                bad.activated.remove(2);
                bad.source_labels = None;
            }
            "block" => bad.source_labels.as_mut().unwrap().activated.swap(0, 1),
            "type_line" => {
                let mut source: Value = serde_json::from_str(&bad.provenance.source_json).unwrap();
                source["type_line"] = json!("Artifact Creature — Scout");
                bad.provenance.source_json = source.to_string();
                bad.provenance.source_sha256 = sha256(bad.provenance.source_json.as_bytes());
            }
            _ => {
                bad.activated[2].program.instructions =
                    bad.activated[0].program.instructions.clone()
            }
        }
        bad.capabilities = bad.requirements();
        assert!(bad.validate().is_err(), "{change}");
    }
    for tail in [
        "/costs/0/payer",
        "/steps/0/to/ref",
        "/steps/0/source",
        "/steps/0/who",
    ] {
        let changed = crate::tests::changed_source(record, |r| {
            let row = r["binding"]["references"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|r| r["path"].as_str().unwrap().ends_with(tail))
                .unwrap();
            row["target"] = json!("unbound");
        });
        assert!(
            !matches!(compile(manifest, &changed), CompileResult::Lowered { .. }),
            "{tail}"
        );
    }
}

#[cfg(test)]
#[test]
fn fixed_self_pump_preserves_signed_stats_and_rejects_wrong_source() {
    let manifest = include_str!("../../../tests/fixtures/archive-manifest-current.json");
    let record = include_str!("../../../tests/fixtures/flowstone-wyvern.json");
    let CompileResult::Lowered { definition } = compile(manifest, record) else {
        panic!("exact frozen source did not lower");
    };
    assert_eq!(definition.keywords, [KeywordAbility::Flying]);
    let [activation] = definition.activated.as_slice() else {
        panic!("expected one activation");
    };
    assert_eq!(
        activation.cost,
        ActivationCost::Mana {
            cost: ManaCost {
                generic: 0.into(),
                colored: vec![Color::R],
            }
        }
    );
    assert!(activation.program.targets.is_empty());
    assert_eq!(
        activation.program.instructions,
        [Instruction::pump(
            kir::SetExpr::source(),
            2.into(),
            (-2).into(),
            1
        )]
    );
    assert!(
        definition
            .capabilities
            .contains(&Capability::TemporaryPowerToughnessV1)
    );
    assert!(!definition.capabilities.contains(&Capability::SacrificeV1));

    let wrong_source = crate::tests::changed_source(record, |r| {
        r["binding"]["references"][1]["target"] = r["binding"]["references"][0]["target"].clone();
    });
    assert!(matches!(
        compile(manifest, &wrong_source),
        CompileResult::Unsupported { .. }
    ));
    let mut forged = *definition;
    forged.activated[0]
        .program
        .instructions
        .push(Instruction::draw(
            PlayerExpr::you(Context::Activated),
            1.into(),
            1,
        ));
    forged.capabilities = forged.requirements();
    assert!(forged.validate().is_err());
}
