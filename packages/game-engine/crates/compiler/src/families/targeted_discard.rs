//! CR 601.2b-c, 608.2b-d and 701.9: announce each selected mode's targets.
//! Discard from the target’s hand on resolution, at random only when specified.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const MODAL_TEXT: &str = "Choose one —\n• Destroy target enchantment.\n• Draw two cards.\n• Target player discards two cards.";

fn fail() -> Failure {
    unsupported(
        "targeted discard",
        "requires a complete bound targeted discard spell",
    )
}

pub(crate) fn validate(
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
    let raw: Value = decode(result.abilities.get(), "abilities")?;
    let root = &raw[0]["effects"][0]["steps"][0];
    let discard = if modal {
        &root["options"][2]["effects"][0]["steps"][0]
    } else {
        root
    };
    let (printed, random, then_choice) = if !modal
        && let Some(n) = printed_count.strip_suffix(" at random, then discards a card.")
    {
        (n, true, true)
    } else if !modal && let Some(n) = printed_count.strip_suffix(" at random.") {
        (n, true, false)
    } else {
        (
            printed_count.strip_suffix('.').ok_or_else(fail)?,
            false,
            false,
        )
    };
    let x = printed == "X cards";
    if source.mana_cost.matches("{X}").count() != usize::from(x) || x && (!random || then_choice) {
        return Err(fail());
    }
    let amount = if x {
        json!({"amount":"x"})
    } else {
        let count = discard["what"]["count"]["value"]
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| *n > 0)
            .ok_or_else(fail)?;
        if (then_choice && count != 1)
            || !printed
                .strip_suffix(if count == 1 { " card" } else { " cards" })
                .is_some_and(|word| {
                    (count == 1 && word == "a") || matches_fixed_amount(word, i64::from(count))
                })
        {
            return Err(fail());
        }
        json!({"amount":"fixed","value":count})
    };

    let who = json!({"player":actor,"selectionId":if modal { "line:3:1" } else { "line:0:0" }});
    let what = json!({"ref":"filter","filter":{"cls":"card"},"count":amount});
    let chosen = json!({"effect":"discard","who":who,"what":what});
    let mut discard = chosen.clone();
    if random {
        discard["random"] = json!(true);
    }
    let steps = if modal {
        vec![
            json!({"effect":"destroy","what":{"ref":"target","filter":{"types":["enchantment"]},"count":{"amount":"fixed","value":1},"selectionId":"line:1:2"}}),
            json!({"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":2}}),
            discard,
        ]
    } else if then_choice {
        vec![discard, chosen]
    } else {
        vec![discard]
    };
    let option = |step: &Value| json!({"effects":[{"steps":[step]}]});
    let root_steps = if modal {
        vec![
            json!({"effect":"modal","count":{"exactly":1},"options":steps.iter().map(option).collect::<Vec<_>>()}),
        ]
    } else {
        steps.clone()
    };
    if raw != json!([{"kind":"spell","effects":[{"steps":root_steps}]}])
        || *binding != expected_binding(source, &steps, modal, x)
    {
        return Err(fail());
    }
    Ok(Some(domain))
}

// Reconstruct the frozen producer's complete graph from the already checked steps.
// The discard event consumes the whole-card query and the announced player, in that order.
fn expected_binding(source: &Source, steps: &[Value], modal: bool, x: bool) -> Value {
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
    let announced = format!("card:{identity}{base}#announced-x");
    if x {
        nodes.push(json!({"id":announced,"path":base,"node":"announced-number","scope":scope,"chooser":you,"announcement":"cast","rules":["107.3a","601.2b"]}));
    }
    let mut targets = BTreeSet::new();
    let mut references = vec![];
    let root = format!("{base}/effects/0/steps/0");
    for (index, step) in steps.iter().enumerate() {
        let path = if modal {
            format!("{root}/options/{index}/effects/0/steps/0")
        } else {
            format!("{base}/effects/0/steps/{index}")
        };
        let op = step["effect"].as_str().expect("constructed operation");
        let cardinality = if x {
            "unknown"
        } else if op == "discard" && step["what"]["count"]["value"] == 1 {
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
                if targets.insert(selection) {
                    nodes.push(json!({"id":id,"path":field,"node":"selection","scope":scope,"expression":expr,"cardinality":"one","timing":"stack-announcement","selection":"target","rules":["601.2c"]}));
                }
                (id, "601.2c")
            } else if key == "who" {
                (you.clone(), "109.5")
            } else {
                if x {
                    references.push(json!({"path":format!("{field}/count"),"expression":{"amount":"x"},"binding":"bound","target":announced,"rules":["107.3a"]}));
                }
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
            if cardinality != "one" {
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
