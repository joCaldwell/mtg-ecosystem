//! CR 601.2b-c, 608.2b-d and 701.9: announce each selected mode's targets.
//! The targeted player chooses from their whole hand on resolution.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const MODAL_TEXT: &str = "Choose one —\n• Destroy target enchantment.\n• Draw two cards.\n• Target player discards two cards.";

fn fail() -> Failure {
    unsupported(
        "targeted discard",
        "requires a complete bound fixed targeted discard spell",
    )
}

pub(super) fn validate(
    source: &Source,
    result: &ParseResult,
    binding: &Value,
) -> Result<Option<TargetDomain>, Failure> {
    let text = source.oracle_text.as_str();
    let modal = text == MODAL_TEXT;
    let (actor, domain, printed_count) = if modal {
        ("target-player", TargetDomain::AnyPlayer, "two cards.")
    } else if let Some(rest) = text.strip_prefix("Target player discards ") {
        ("target-player", TargetDomain::AnyPlayer, rest)
    } else if let Some(rest) = text.strip_prefix("Target opponent discards ") {
        ("target-opponent", TargetDomain::OpponentPlayer, rest)
    } else {
        return Ok(None);
    };
    if source.mana_cost.contains("{X}") {
        return Err(fail());
    }
    let raw: Value = decode(result.abilities.get(), "abilities")?;
    let root = &raw[0]["effects"][0]["steps"][0];
    let discard = if modal {
        &root["options"][2]["effects"][0]["steps"][0]
    } else {
        root
    };
    let count = discard["what"]["count"]["value"]
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .filter(|n| *n > 0)
        .ok_or_else(fail)?;
    let suffix = if count == 1 { " card." } else { " cards." };
    if !printed_count.strip_suffix(suffix).is_some_and(|word| {
        (count == 1 && word == "a") || matches_fixed_amount(word, i64::from(count))
    }) {
        return Err(fail());
    }

    let who = json!({"player":actor,"selectionId":if modal { "line:3:1" } else { "line:0:0" }});
    let what =
        json!({"ref":"filter","filter":{"cls":"card"},"count":{"amount":"fixed","value":count}});
    let discard = json!({"effect":"discard","who":who,"what":what});
    let steps = if modal {
        vec![
            json!({"effect":"destroy","what":{"ref":"target","filter":{"types":["enchantment"]},"count":{"amount":"fixed","value":1},"selectionId":"line:1:2"}}),
            json!({"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":2}}),
            discard,
        ]
    } else {
        vec![discard]
    };
    let option = |step: &Value| json!({"effects":[{"steps":[step]}]});
    let root = if modal {
        json!({"effect":"modal","count":{"exactly":1},"options":steps.iter().map(option).collect::<Vec<_>>()})
    } else {
        steps[0].clone()
    };
    if raw != json!([{"kind":"spell","effects":[{"steps":[root]}]}])
        || *binding != expected_binding(source, &steps, count)
    {
        return Err(fail());
    }
    Ok(Some(domain))
}

// Reconstruct the frozen producer's complete graph from the already checked steps.
// The discard event consumes the whole-card query and the announced player, in that order.
fn expected_binding(source: &Source, steps: &[Value], count: u32) -> Value {
    let identity = &source.oracle_id;
    let object = format!("card:{identity}/source#object");
    let face = format!("card:{identity}/faces/0#face");
    let base = "/faces/0/abilities/0";
    let scope = format!("card:{identity}{base}#ability");
    let you = format!("card:{identity}{base}#you");
    let mut nodes = vec![
        json!({"id":object,"path":"/source","node":"source","identity":identity,"rules":["201.5"]}),
        json!({"id":face,"path":"/faces/0","node":"face","source":object,"sourceFaceIndex":0,"name":source.name,"layout":source.layout,"typeLine":source.type_line,"manaCost":source.mana_cost,"rules":["712"]}),
        json!({"id":scope,"path":base,"node":"ability","source":object,"face":face,"abilityKind":"spell","you":you,"rules":["113","109.5"]}),
        json!({"id":you,"path":base,"node":"runtime-player","scope":scope,"role":"spell-controller","rules":["109.5"]}),
    ];
    let mut references = vec![];
    let root = format!("{base}/effects/0/steps/0");
    for (index, step) in steps.iter().enumerate() {
        let path = if steps.len() == 1 {
            root.clone()
        } else {
            format!("{root}/options/{index}/effects/0/steps/0")
        };
        let op = step["effect"].as_str().expect("constructed operation");
        let cardinality = if op == "discard" && count == 1 {
            "one"
        } else {
            "many"
        };
        let mut inputs = vec![];
        for key in ["who", "what"] {
            let Some(expr) = step.get(key) else { continue };
            let field = format!("{path}/{key}");
            let (id, rule) = if let Some(selection) = expr["selectionId"].as_str() {
                let id = format!("{scope}/targets/{}", selection.replace(':', "%3A"));
                nodes.push(json!({"id":id,"path":field,"node":"selection","scope":scope,"expression":expr,"cardinality":"one","timing":"stack-announcement","selection":"target","rules":["601.2c"]}));
                (id, "601.2c")
            } else if key == "who" {
                (you.clone(), "109.5")
            } else {
                let id = format!("card:{identity}{field}#query");
                nodes.push(json!({"id":id,"path":field,"node":"selection","scope":scope,"expression":expr,"cardinality":cardinality,"timing":"resolution","selection":"evaluate-reference","rules":["608.2h"]}));
                (id, "608.2h")
            };
            references.push(json!({"path":field,"expression":expr,"binding":"bound","target":id,"rules":[rule]}));
            inputs.insert(0, id);
        }
        let event = format!("card:{identity}{path}#event");
        nodes.push(json!({"id":event,"path":path,"node":"event","scope":scope,"operation":op,"inputs":inputs,"availability":"when-performed","rules":["608.2c"]}));
        if op != "destroy" {
            let (kind, rule) = if op == "draw" {
                ("new-objects", "608.2h")
            } else {
                ("zone-successors", "400.7j")
            };
            nodes.push(json!({"id":format!("card:{identity}{path}#objects"),"path":path,"node":"event-result","event":event,"result":"objects","cardinality":cardinality,"availability":"if-produced","identity":kind,"description":{"cls":"card"},"rules":[rule]}));
            if cardinality == "many" {
                nodes.push(json!({"id":format!("card:{identity}{path}#quantity"),"path":path,"node":"event-result","event":event,"result":"quantity","cardinality":"one","availability":"if-produced","identity":"measured-quantity","rules":["608.2h"]}));
            }
        }
    }
    let total = references.len();
    json!({"artifactKind":"oracle-reference-binding","schemaVersion":1,"executable":false,"identity":identity,
        "rules":{"edition":RULES_EDITION,"url":"https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt"},
        "nodes":nodes,"references":references,"diagnostics":[],"counts":{"obligations":total,"bound":total,"unresolved":0},
        "referencesBound":true,"engineFeatures":[],"fullyBound":true})
}
