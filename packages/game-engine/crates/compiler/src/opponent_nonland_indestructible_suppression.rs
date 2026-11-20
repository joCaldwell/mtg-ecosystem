//! Both queries use the resolving spell's controller and current battlefield.
//! CR 109.5, 608.2c/h, 611.2c and 702.12.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::bindings::Bindings;
use mtg_contracts::opponent_nonland_indestructible_suppression as contract;
use serde_json::{Value, json};

const SUPPRESS: &str = "/faces/0/abilities/0";
const DESTROY: &str = "/faces/0/abilities/1";
const REMOVED: &str = "/faces/0/abilities/0/effects/0/steps/0/removal/abilities/0";
const GRAPH_ERROR: &str =
    "requires both resolution-time nonland selections and the bound indestructible removal";

fn bind_set(graph: &mut Bindings<'_>, base: &str, expression: &Value) -> String {
    let path = format!("{base}/effects/0/steps/0/what");
    let id = graph.node(
        &path,
        "query",
        "selection",
        json!({
            "scope":graph.id(base,"ability"),"expression":expression,"cardinality":"many",
            "timing":"resolution","selection":"evaluate-reference","rules":["608.2h"]
        }),
    );
    graph.reference(&path, expression, &id, "608.2h");
    id
}

pub(super) fn lower(
    source: &Value,
    result: &ParseResult,
    binding: &Value,
) -> Result<Option<[Instruction; 2]>, Failure> {
    if source["oracle_text"] != contract::TEXT {
        // A rewritten spell cannot leave the removal's original binding unconsumed.
        if binding["nodes"].as_array().is_some_and(|nodes| {
            nodes.iter().any(|n| {
                n["path"] == "/faces/0/abilities/0/effects/0/steps/0/removal/abilities/0/keyword"
                    && n["node"] == "keyword-ability"
                    && n["keyword"] == "indestructible"
            }) && nodes.iter().any(|n| {
                n["path"] == "/faces/0/abilities/1/effects/0/steps/0"
                    && n["node"] == "event"
                    && n["operation"] == "destroy"
            })
        }) {
            return Err(unsupported("binding", GRAPH_ERROR));
        }
        return Ok(None);
    }
    let expected = contract::abilities();
    if source["type_line"] != "Sorcery"
        || !result
            .lines
            .iter()
            .map(|l| l.text.as_str())
            .eq(contract::TEXT.lines())
        || decode::<Value>(result.abilities.get(), "abilities")? != expected
    {
        return Err(unsupported(
            "abilities",
            "requires both exact suppression and destruction spell blocks",
        ));
    }
    let identity = source["oracle_id"]
        .as_str()
        .ok_or_else(|| invalid("source.oracle_id", "missing identity"))?;
    let mut graph = Bindings::new(identity, source);
    graph.ability(SUPPRESS, "spell", "spell-controller");
    let scope = graph.id(SUPPRESS, "ability");
    let suppress_step = format!("{SUPPRESS}/effects/0/steps/0");
    let control = format!("{suppress_step}/what/filter/control");
    let opponents = graph.node(
        &control,
        "opponents",
        "derived-player",
        json!({
            "scope":scope,"role":"opponents","of":graph.id(SUPPRESS,"you"),"rules":["102.2","109.5"]
        }),
    );
    graph.reference(
        &control,
        &json!({"implicit":"opponents"}),
        &opponents,
        "102.2",
    );
    let affected = bind_set(
        &mut graph,
        SUPPRESS,
        &expected[0]["effects"][0]["steps"][0]["what"],
    );
    let recipient = graph.node(
        REMOVED,
        "recipient",
        "recipient",
        json!({
            "scope":scope,"of":affected,"each":true,"rules":["113.10","201.5a"]
        }),
    );
    let removed_ability = graph.node(
        REMOVED,
        "ability",
        "ability",
        json!({
            "source":recipient,"face":graph.id("/faces/0","face"),"abilityKind":"keywords",
            "you":graph.id(REMOVED,"you"),"rules":["113","113.10"]
        }),
    );
    graph.node(REMOVED,"you","runtime-player",json!({
        "scope":removed_ability,"role":"static-source-controller","rules":["109.5","301.5d","303.4e"]
    }));
    let keyword = format!("{REMOVED}/keyword");
    let controller = graph.node(
        &keyword,
        "you",
        "runtime-player",
        json!({
            "scope":removed_ability,"role":"static-source-controller","rules":["109.5","702.12"]
        }),
    );
    graph.node(&keyword,"keyword","keyword-ability",json!({
        "scope":removed_ability,"keyword":"indestructible","abilities":["static"],"you":controller,"rules":["702.12"]
    }));
    graph.event(SUPPRESS, &suppress_step, "lose-abilities", &affected, false);

    graph.ability(DESTROY, "spell", "spell-controller");
    let all_nonlands = bind_set(
        &mut graph,
        DESTROY,
        &expected[1]["effects"][0]["steps"][0]["what"],
    );
    let destroy_step = format!("{DESTROY}/effects/0/steps/0");
    graph.event(DESTROY, &destroy_step, "destroy", &all_nonlands, false);
    graph.result(&destroy_step, "quantity", "measured-quantity", None);
    graph.validate_ordered(binding, GRAPH_ERROR)?;
    Ok(Some(contract::instructions()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::changed_source;

    #[test]
    fn opponent_nonland_indestructible_suppression() {
        const RECORD: &str =
            include_str!("../../../tests/fixtures/ramos/8d90ffb5-63da-48d5-a5a2-7b61062556f6.json");
        const HEADER: &str =
            include_str!("../../../tests/fixtures/ramos/archive-manifest-current.json");
        assert_eq!(
            sha256(RECORD.trim_end().as_bytes()),
            "83e9a63cc55e283331ca28292fa70f8fd111e2bf944c51a8855a4bf89e92ab53"
        );
        let compiled = compile(HEADER, RECORD);
        let CompileResult::Lowered { definition } = compiled else {
            panic!("{compiled:?}")
        };
        assert_eq!(
            definition.spell.unwrap().instructions,
            contract::instructions()
        );
        assert_eq!(
            definition.capabilities,
            BTreeSet::from([
                Capability::NormalLayoutV1,
                Capability::CastSorceryV1,
                Capability::BoardWipesV1,
                Capability::OpponentNonlandIndestructibleSuppressionV1
            ])
        );
        for (pointer, value) in [
            ("/binding/nodes/4/of", json!("active-player")),
            ("/binding/nodes/5/timing", json!("stack-announcement")),
            ("/binding/nodes/6/each", json!(false)),
            ("/binding/nodes/10/keyword", json!("hexproof")),
            (
                "/binding/nodes/14/expression/filter",
                json!({"nonTypes":["land"],"cls":"permanent","control":"opponent"}),
            ),
            ("/binding/references/2/target", json!("opponents")),
            ("/binding/nodes/16/identity", json!("same-objects")),
        ] {
            let bad = changed_source(RECORD, |r| {
                *r.pointer_mut(pointer).unwrap() = value;
            });
            assert!(
                matches!(compile(HEADER,&bad), CompileResult::Unsupported { path, .. } if path == "binding"),
                "{pointer}"
            );
        }
        for (field, value) in [
            ("duration", json!({"duration":"forever"})),
            ("removal", json!(null)),
            ("optional", json!(false)),
        ] {
            let bad = changed_source(RECORD, |r| {
                let parsed = &mut r["faces"][0]["result"];
                parsed["abilities"][0]["effects"][0]["steps"][0][field] = value;
                parsed["lines"][0]["ability"] = parsed["abilities"][0].clone();
            });
            assert!(
                matches!(compile(HEADER,&bad), CompileResult::Unsupported { path, .. } if path == "abilities")
            );
        }
        let bad = changed_source(RECORD, |r| {
            r["source"]["oracle_text"] = json!("Destroy all nonland permanents.");
            let parsed = &mut r["faces"][0]["result"];
            parsed["abilities"].as_array_mut().unwrap().remove(0);
            parsed["lines"].as_array_mut().unwrap().remove(0);
        });
        assert!(
            matches!(compile(HEADER,&bad), CompileResult::Unsupported { path, .. } if path == "binding")
        );
        let bad = changed_source(RECORD, |r| {
            r["faces"][0]["result"]["lines"][0]["ability"]["abilityWord"] = Value::Null
        });
        assert!(
            matches!(compile(HEADER,&bad), CompileResult::Invalid { path, .. } if path == "lines")
        );
    }
}
