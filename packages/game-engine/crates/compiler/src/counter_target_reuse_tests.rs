//! Synthetic complete counter bodies derived from retained Swan Song's typed
//! selector. These do not change frozen card facts or expected catalog IR.
use super::*;
use serde_json::{Value, json};

const SWAN: &str = include_str!("../../../tests/fixtures/swan-song.json");

fn spell_filter(types: &[&str]) -> Value {
    json!({"cls":"spell","allOf":[{"anyOf":types.iter().map(|kind| json!({"types":[kind]})).collect::<Vec<_>>()}]})
}

fn counter_body(first: Value, second: Value, distinct: bool) -> String {
    tests::changed_source(SWAN, |record| {
        // The original Bird effect is intentionally absent from this synthetic
        // complete body. It refers to one target's controller and would reject
        // distinct target slots independently of the filter-loss regression.
        let text = if distinct {
            "Synthetic counter target spell, then counter a second target spell."
        } else {
            "Synthetic counter target spell, then counter that same selected spell."
        };
        record["source"]["oracle_text"] = text.into();
        let original =
            record["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0].clone();
        let mut steps = [original.clone(), original];
        steps[0]["what"]["filter"] = first;
        steps[1]["what"]["filter"] = second;
        if distinct {
            steps[1]["what"]["selectionId"] = "line:0:2".into();
        }
        let ability = json!({"kind":"spell","effects":[{"steps":steps}]});
        let result = &mut record["faces"][0]["result"];
        result["abilities"] = json!([ability]);
        result["lines"][0]["text"] = text.into();
        result["lines"][0]["ability"] = ability;

        // Rebuild the diagnostic selection/event portion so neither stale
        // Bird references nor a hash error can stand in for semantic rejection.
        let prefix = "card:8ddfc283-c9b4-41a5-af88-cf0068e986cc/faces/0/abilities/0";
        let scope = format!("{prefix}#ability");
        let binding = &mut record["binding"];
        let nodes = binding["nodes"].as_array_mut().unwrap();
        nodes.retain(|node| {
            matches!(
                node["node"].as_str(),
                Some("source" | "face" | "ability" | "runtime-player")
            )
        });
        let mut refs = vec![];
        let mut selections = BTreeSet::new();
        for (index, step) in steps.iter().enumerate() {
            let event_path = format!("/faces/0/abilities/0/effects/0/steps/{index}");
            let path = format!("{event_path}/what");
            let id = step["what"]["selectionId"].as_str().unwrap();
            let selection = format!("{scope}/targets/{}", id.replace(':', "%3A"));
            if selections.insert(id) {
                nodes.push(json!({"id":selection,"path":path,"node":"selection","scope":scope,"expression":step["what"],"cardinality":"one","timing":"stack-announcement","selection":"target","rules":["601.2c"]}));
            }
            nodes.push(json!({"id":format!("card:8ddfc283-c9b4-41a5-af88-cf0068e986cc{event_path}#event"),"path":event_path,"node":"event","scope":scope,"operation":"counter","inputs":[selection],"availability":"when-performed","rules":["608.2c"]}));
            refs.push(json!({"path":path,"expression":step["what"],"binding":"bound","target":selection,"rules":["601.2c"]}));
        }
        binding["references"] = refs.into();
        binding["diagnostics"] = json!([]);
        binding["counts"] = json!({"obligations":2,"bound":2,"unresolved":0});
        binding["referencesBound"] = true.into();
        binding["engineFeatures"] = json!([]);
        binding["fullyBound"] = true.into();
    })
}

#[test]
fn repeated_counter_selector_must_not_overwrite_a_conflicting_typed_filter() {
    let original = spell_filter(&["enchantment", "instant", "sorcery"]);
    let incompatible = spell_filter(&["artifact", "creature"]);
    for (first, second) in [
        (original.clone(), incompatible.clone()),
        (incompatible, original),
    ] {
        let raw = counter_body(first, second, false);
        let result = compile(fixtures::MANIFEST, &raw);
        assert!(
            matches!(result, CompileResult::Unsupported { .. }),
            "reusing one printed selector must not silently replace its first restriction: {result:?}"
        );
    }
}

#[test]
fn matching_counter_selectors_preserve_reuse_and_distinct_occurrences() {
    let catalog = embedded_catalog("wipes").unwrap();
    let result = compile(fixtures::MANIFEST, SWAN);
    let CompileResult::Lowered { definition } = result else {
        panic!("{result:?}")
    };
    assert_eq!(
        catalog
            .definitions
            .iter()
            .find(|d| d.oracle_id == definition.oracle_id),
        Some(definition.as_ref())
    );
    let filter = spell_filter(&["enchantment", "instant", "sorcery"]);
    for distinct in [false, true] {
        let raw = counter_body(filter.clone(), filter.clone(), distinct);
        let result = compile(fixtures::MANIFEST, &raw);
        let CompileResult::Lowered { definition } = result else {
            panic!("distinct={distinct}: {result:?}")
        };
        let p = definition.spell.as_ref().unwrap();
        assert_eq!(p.targets.len(), if distinct { 2 } else { 1 });
        assert_eq!(
            p.targets[0].filter.as_ref().unwrap().types,
            vec![CardKind::Enchantment, CardKind::Instant, CardKind::Sorcery]
        );
        assert_eq!(
            p.instructions,
            vec![
                Instruction::CounterStackObject {
                    target_slot: 0,
                    source_block: 0
                },
                Instruction::CounterStackObject {
                    target_slot: u32::from(distinct),
                    source_block: 0
                }
            ]
        );
        if distinct {
            assert_eq!(p.targets[1].selection_id, "line:0:2");
            assert_eq!(p.targets[0].filter, p.targets[1].filter);
        }
        definition.validate().unwrap();
    }
}

#[test]
fn filtered_and_unfiltered_counter_uses_preserve_the_represented_intersection() {
    // An unfiltered use adds no restriction to this same printed selector.
    // Keeping the one complete typed filter represents both uses in either
    // order, so this regression must not invent a new rejection boundary.
    let typed = spell_filter(&["enchantment", "instant", "sorcery"]);
    let any = json!({"cls":"spell"});
    for (first, second) in [(typed.clone(), any.clone()), (any, typed)] {
        let raw = counter_body(first, second, false);
        let result = compile(fixtures::MANIFEST, &raw);
        let CompileResult::Lowered { definition } = result else {
            panic!("{result:?}")
        };
        let p = definition.spell.as_ref().unwrap();
        assert_eq!(p.targets.len(), 1);
        assert_eq!(
            p.targets[0].filter.as_ref().unwrap().types,
            vec![CardKind::Enchantment, CardKind::Instant, CardKind::Sorcery]
        );
        assert_eq!(
            p.instructions,
            vec![
                Instruction::CounterStackObject {
                    target_slot: 0,
                    source_block: 0
                },
                Instruction::CounterStackObject {
                    target_slot: 0,
                    source_block: 0
                }
            ]
        );
        definition.validate().unwrap();
    }
}
