//! Resolution-time zone and type sets. CR 109.2, 400.7, 608.2h, 701.11.
use super::*;
use crate::bindings::Bindings;
use serde_json::{Value, json};

pub(super) fn lower(
    source: &Value,
    result: &ParseResult,
    binding: &Value,
    identity: &str,
) -> Result<Option<SpellProgram>, Failure> {
    let text = source["oracle_text"].as_str().unwrap_or("");
    let modal = text.starts_with("Choose one or more —\n");
    let ordinary = SetExileClause::parse(text);
    if ordinary.is_none() && !modal {
        return if text
            .lines()
            .any(|line| SetExileClause::parse(line).is_some())
        {
            Err(unsupported(
                "set exile",
                "requires a complete set exile spell",
            ))
        } else {
            Ok(None)
        };
    }
    let fail = || {
        unsupported(
            "set exile",
            "requires one ordinary untargeted exile with a bound resolution-time set",
        )
    };
    let timing = match source["type_line"].as_str() {
        Some("Sorcery") => SpellTiming::MainPhase,
        Some("Instant") => SpellTiming::Instant,
        _ => return Err(fail()),
    };
    let root = "/faces/0/abilities/0";
    let base = format!("{root}/effects/0/steps/0");
    let mut graph = Bindings::new(identity, source);
    graph.ability(root, "spell", "spell-controller");
    if modal {
        let (clauses, ast, program) = set_exile_modes(text, timing).ok_or_else(fail)?;
        if !matches!(result.lines.as_slice(),[line] if line.text==text)
            || decode::<Value>(result.abilities.get(), "abilities")? != ast
        {
            return Err(fail());
        }
        for (index, clause) in clauses.iter().enumerate() {
            bind_clause(
                &mut graph,
                root,
                &format!("{base}/options/{index}/effects/0/steps/0"),
                clause,
            );
        }
        graph.validate(binding, "requires exact modal set exile bindings")?;
        return Ok(Some(program));
    }
    let clause = ordinary.ok_or_else(fail)?;
    if !matches!(result.lines.as_slice(),[line] if line.text==text)
        || decode::<Value>(result.abilities.get(), "abilities")? != json!([clause.ability()])
    {
        return Err(fail());
    }
    bind_clause(&mut graph, root, &base, &clause);
    graph.validate(binding, "requires the complete set exile binding graph")?;
    Ok(Some(SpellProgram {
        timing,
        resolution: SpellResolution::Instructions,
        functioning_zone: FunctioningZone::Stack,
        targets: vec![],
        instructions: vec![Instruction::act(
            Verb::Move {
                what: clause.set.kernel(Context::Spell),
                to: Destination::Exile,
            },
            0,
        )],
        modes: vec![],
        additional_sacrifice: None,
        optional_additional_costs: vec![],
    }))
}

fn bind_clause(graph: &mut Bindings, root: &str, step: &str, clause: &SetExileClause) {
    let what = format!("{step}/what");
    let scope = graph.id(root, "ability");
    let mut query = |path: &str, expression: &Value| {
        let id = graph.node(
            path,
            "query",
            "selection",
            json!({"scope":scope,"expression":expression,"cardinality":"many",
            "timing":"resolution","selection":"evaluate-reference","rules":["608.2h"]}),
        );
        graph.reference(path, expression, &id, "608.2h");
        id
    };
    if let Some(owner) = clause.what.get("owner") {
        query(&format!("{what}/owner"), owner);
    }
    let selected = query(&what, &clause.what);
    graph.event(root, step, "exile", &selected, false);
    graph.node(step,"objects","event-result",json!({"event":graph.id(step,"event"),"result":"objects","cardinality":"many",
        "availability":"if-produced","identity":"zone-successors","description":clause.successors,"rules":["400.7j"]}));
    graph.result(step, "quantity", "measured-quantity", None);
}

#[test]
fn fixed_set_exile_rejects_changed_zone_unbound_selection_and_erased_ir() {
    let manifest = include_str!("../../../tests/fixtures/archive-manifest-current.json");
    for raw in include_str!("../../../tests/fixtures/synthetic-set-exile.jsonl").lines() {
        let parsed = compile(manifest, raw);
        let CompileResult::Lowered { definition } = parsed else {
            panic!("{parsed:?}")
        };
        for change in ["selector", "erasure", "action", "damage"] {
            let mut d = definition.clone();
            let ops = &mut d.spell.as_mut().unwrap().instructions;
            match change {
                "erasure" => ops.clear(),
                "damage" => {
                    ops[0] = SetDamage::instruction(
                        vec![ObjectSet::GraveyardCards.kernel(Context::Spell)],
                        false,
                        kir::Amount::fixed(1),
                        0,
                    )
                }
                _ => {
                    let what = ObjectSet::Creatures.kernel(Context::Spell);
                    ops[0] = if change == "action" {
                        Instruction::act(
                            Verb::Destroy {
                                what: ObjectSet::GraveyardCards.kernel(Context::Spell),
                                regeneration_forbidden: false,
                            },
                            0,
                        )
                    } else {
                        Instruction::act(
                            Verb::Move {
                                what,
                                to: Destination::Exile,
                            },
                            0,
                        )
                    };
                }
            }
            d.capabilities = d.requirements();
            assert!(d.validate().is_err(), "{change}");
        }
        for change in ["zone", "timing", "event", "result"] {
            let bad = crate::tests::changed_source(raw, |r| {
                if change == "zone" {
                    let result = &mut r["faces"][0]["result"];
                    result["abilities"][0]["effects"][0]["steps"][0]["what"] =
                        json!({"ref":"all","filter":{"types":["creature"]}});
                    result["lines"][0]["ability"] = result["abilities"][0].clone();
                } else {
                    let (node, key, val) = match change {
                        "timing" => ("selection", "timing", "announcement"),
                        "event" => ("event", "operation", "destroy"),
                        _ => ("event-result", "identity", "same-objects"),
                    };
                    let row = r["binding"]["nodes"]
                        .as_array_mut()
                        .unwrap()
                        .iter_mut()
                        .find(|row| row["node"] == node)
                        .unwrap();
                    row[key] = json!(val);
                }
            });
            assert!(
                !matches!(compile(manifest, &bad), CompileResult::Lowered { .. }),
                "{change}"
            );
        }
    }
}

#[test]
fn set_exile_modes_bind_whole_source_and_each_printed_resolution_set() {
    let synthetic = include_str!("../../../tests/fixtures/synthetic-open-mode-catalog.json");
    for (manifest, raw) in [
        (
            include_str!("../../../tests/fixtures/archive-manifest-current.json"),
            synthetic,
        ),
        (
            include_str!("../../../tests/fixtures/ramos/archive-manifest-current.json"),
            include_str!("../../../tests/fixtures/ramos/4eb813fd-2d5a-4b02-8193-662681ef4e7d.json"),
        ),
    ] {
        let parsed = compile(manifest, raw);
        let CompileResult::Lowered { definition } = parsed else {
            panic!("complete set modes: {parsed:?}")
        };
        assert!(
            definition
                .capabilities
                .contains(&Capability::NonemptyModeSubsetV1)
        );
        let mut wrong_cost = definition.clone();
        wrong_cost.characteristics.mana_cost.generic.add(&1.into());
        wrong_cost.capabilities = wrong_cost.requirements();
        assert!(wrong_cost.validate().is_err());
        let mut wrong = definition.clone();
        wrong.spell.as_mut().unwrap().resolution = SpellResolution::Instructions;
        wrong.capabilities = wrong.requirements();
        assert!(wrong.validate().is_err());
    }
    let manifest = include_str!("../../../tests/fixtures/archive-manifest-current.json");
    for change in [
        "count",
        "extra condition",
        "foreign set",
        "mixed rows",
        "unbound scope",
    ] {
        let bad = crate::tests::changed_source(synthetic, |r| {
            if change == "unbound scope" {
                let nodes = r["binding"]["nodes"].as_array_mut().unwrap();
                nodes.iter_mut().find(|n| n["node"] == "selection").unwrap()["scope"] =
                    json!("another ability");
                return;
            }
            let result = &mut r["faces"][0]["result"];
            let modal = &mut result["abilities"][0]["effects"][0]["steps"][0];
            match change {
                "count" => modal["count"] = json!({"exactly":1}),
                "extra condition" => {
                    modal["options"][0]["effects"][0]["condition"] = json!({"condition":"unknown"})
                }
                "foreign set" => {
                    modal["options"][0]["effects"][0]["steps"][0]["what"] =
                        json!({"ref":"all","filter":{"types":["land"]}})
                }
                _ => modal["options"].as_array_mut().unwrap().swap(0, 1),
            }
            result["lines"][0]["ability"] = result["abilities"][0].clone();
        });
        assert!(
            !matches!(compile(manifest, &bad), CompileResult::Lowered { .. }),
            "{change}"
        );
    }
}
