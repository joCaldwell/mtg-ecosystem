//! Bind the two spell blocks to one invocation, including its completion destination.
use super::*;
use crate::bindings::Bindings;
use serde_json::{Value, json};

pub(super) fn lower(
    source: &Value,
    result: &ParseResult,
    binding: &Value,
    identity: &str,
) -> Result<Option<SpellProgram>, Failure> {
    let ast: Value = decode(result.abilities.get(), "abilities")?;
    let Some(instructions) = controller_window_instructions(
        source["name"].as_str().unwrap_or(""),
        source["oracle_text"].as_str().unwrap_or(""),
        &ast,
    ) else {
        return Ok(None);
    };
    let timing = match source["type_line"].as_str() {
        Some("Instant") => SpellTiming::Instant,
        Some("Sorcery") => SpellTiming::MainPhase,
        _ => {
            return Err(unsupported(
                "controller window",
                "requires an ordinary instant or sorcery",
            ));
        }
    };
    let root = "/faces/0/abilities/0";
    let mut graph = Bindings::new(identity, source);
    graph.ability(root, "spell", "spell-controller");
    let scope = graph.id(root, "ability");
    let you = graph.id(root, "you");
    let actor = json!({"player":"you"});
    for index in 0..2 {
        let path = format!("{root}/effects/0/steps/0/steps/{index}");
        for field in ["rule/who", "duration/whose"] {
            graph.reference(&format!("{path}/{field}"), &actor, &you, "109.5");
        }
        graph.node(&path,"event","event",json!({"scope":scope,"operation":"player-rule","inputs":[],"availability":"when-performed","rules":["608.2c"]}));
    }
    let protection = format!("{root}/effects/0/steps/1");
    for field in ["who", "duration/whose"] {
        graph.reference(&format!("{protection}/{field}"), &actor, &you, "109.5");
    }
    graph.event(root, &protection, "player-gain-abilities", &you, false);
    let phase = format!("{root}/effects/1/steps/0");
    let what = format!("{phase}/what");
    let set = &ast[0]["effects"][1]["steps"][0]["what"];
    graph.reference(&format!("{what}/filter/control"), &actor, &you, "109.5");
    let selected=graph.node(&what,"query","selection",json!({"scope":scope,"expression":set,"cardinality":"many","timing":"resolution","selection":"evaluate-reference","rules":["608.2h"]}));
    graph.reference(&what, set, &selected, "608.2h");
    graph.event(root, &phase, "phase-out", &selected, false);
    let terminal = "/faces/0/abilities/1";
    graph.ability(terminal, "spell", "spell-controller");
    let path = format!("{terminal}/effects/0/steps/0");
    let object = graph.id("/source", "object");
    graph.reference(
        &format!("{path}/what"),
        &json!({"ref":"self"}),
        &object,
        "201.5",
    );
    graph.event(terminal, &path, "exile", &object, false);
    graph.node(&path,"objects","event-result",json!({"event":graph.id(&path,"event"),"result":"objects","cardinality":"one","availability":"if-produced","identity":"zone-successors","description":{"cls":"card"},"rules":["400.7j"]}));
    graph.validate(binding,"requires both spell scopes, duration controllers, phasing set and exact stack-source reference")?;
    Ok(Some(SpellProgram {
        timing,
        resolution: SpellResolution::Instructions,
        functioning_zone: FunctioningZone::Stack,
        targets: vec![],
        modes: vec![],
        instructions,
        additional_sacrifice: None,
        optional_additional_costs: vec![],
    }))
}

#[test]
fn controller_window_rejects_cross_scope_self_and_missing_duration_edges() {
    let header = include_str!("../../../tests/fixtures/archive-manifest-current.json");
    let record =
        include_str!("../../../tests/fixtures/ramos/0d4ecdb1-ec90-497f-a7a4-1c68092b8757.json");
    assert!(matches!(
        compile(header, record),
        CompileResult::Lowered { .. }
    ));
    for change in [
        "source reference",
        "duration reference",
        "phase selection",
        "terminal result",
        "duration ast",
    ] {
        let changed = crate::tests::changed_source(record, |r| {
            let graph = &mut r["binding"];
            match change {
                "source reference" => {
                    graph["references"][8]["target"] = graph["references"][0]["target"].clone()
                }
                "duration reference" => {
                    graph["references"].as_array_mut().unwrap().remove(1);
                }
                "phase selection" => graph["nodes"][7]["timing"] = json!("stack-announcement"),
                "terminal result" => graph["nodes"][12]["cardinality"] = json!("many"),
                _ => {
                    r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][1]["duration"]
                        ["duration"] = json!("until-end-of-turn")
                }
            }
        });
        assert!(
            !matches!(compile(header, &changed), CompileResult::Lowered { .. }),
            "{change}"
        );
    }
}
