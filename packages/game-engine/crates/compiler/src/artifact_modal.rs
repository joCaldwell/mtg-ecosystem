//! Bind the payment selection to the activator and each modal operand to the
//! correct child. Payment is independent of the announced target.
use super::*;
use crate::bindings::Bindings;
use serde_json::{Value, json};

pub(super) fn bind(
    result: &ParseResult,
    source: &Value,
    binding: &Value,
    identity: &str,
) -> Result<BTreeMap<usize, ActivatedAbility>, Failure> {
    let mut lowered = BTreeMap::new();
    let mut first_line = 0;
    for (block, line) in result.lines.iter().enumerate() {
        let start = first_line;
        first_line += line.text.lines().count();
        if !line
            .text
            .lines()
            .next()
            .is_some_and(|s| s.ends_with(" artifacts: Choose one —"))
        {
            continue;
        }
        let fail = || {
            unsupported(
                "activated.artifact_set",
                "requires a complete fixed artifact payment and supported choices",
            )
        };
        let bound =
            BoundArtifactMode::parse(&line.text, source_block(block)?, start).ok_or_else(fail)?;
        if decode::<Value>(line.ability.get(), "abilities")? != bound.ast {
            return Err(fail());
        }
        let mut g = Bindings::new(identity, source);
        let root = format!("/faces/0/abilities/{block}");
        let scope = g.id(&root, "ability");
        let you = g.id(&root, "you");
        let player = json!({"player":"you"});
        g.ability(&root, "activated", "activator");
        for (index, cost) in bound.ast["costs"]
            .as_array()
            .ok_or_else(fail)?
            .iter()
            .enumerate()
        {
            let path = format!("{root}/costs/{index}");
            g.reference(&format!("{path}/payer"), &player, &you, "109.5");
            if cost["cost"] == "sacrifice" {
                let what = format!("{path}/what");
                let query=g.node(&what,"query","selection",json!({"scope":scope,"expression":cost["what"],
                    "cardinality":"many","timing":"payment","selection":"evaluate-reference","rules":["608.2h"]}));
                g.reference(&what, &cost["what"], &query, "608.2h");
                g.node(
                    &path,
                    "event",
                    "event",
                    json!({"scope":scope,"operation":"cost:sacrifice","inputs":[],
                    "availability":"when-performed","rules":["608.2c"]}),
                );
                g.result(&path, "quantity", "measured-quantity", None);
            }
        }
        for (index, mode) in bound.ast["effects"][0]["steps"][0]["options"]
            .as_array()
            .ok_or_else(fail)?
            .iter()
            .enumerate()
        {
            let path = format!("{root}/effects/0/steps/0/options/{index}/effects/0/steps/0");
            let step = &mode["effects"][0]["steps"][0];
            let op = step["effect"].as_str().ok_or_else(fail)?;
            if op == "damage" {
                let target = &step["to"]["selectionId"];
                let object = g.id("/source", "object");
                g.reference(&format!("{path}/source"), &step["source"], &object, "201.5");
                let expression = json!({"ref":"any-target","selectionId":target});
                let target = g.target(
                    &scope,
                    &format!("{path}/to"),
                    &expression,
                    target.as_str().ok_or_else(fail)?,
                );
                g.node(
                    &path,
                    "event",
                    "event",
                    json!({"scope":scope,"operation":"damage","inputs":[object,target],
                    "availability":"when-performed","rules":["608.2c"]}),
                );
                g.result(&path, "quantity", "measured-quantity", None);
                continue;
            }
            let input = if op == "pump" {
                let target = &step["what"];
                g.target(
                    &scope,
                    &format!("{path}/what"),
                    target,
                    target["selectionId"].as_str().ok_or_else(fail)?,
                )
            } else {
                g.reference(&format!("{path}/who"), &player, &you, "109.5");
                you.clone()
            };
            g.event(&root, &path, op, &input, false);
            if op == "draw" {
                g.result(
                    &path,
                    "objects",
                    "new-objects",
                    Some(&json!({"cls":"card"})),
                );
            }
            if op != "pump" {
                g.result(&path, "quantity", "measured-quantity", None);
            }
        }
        g.validate_block(binding, &root)?;
        lowered.insert(block, bound.ability);
    }
    Ok(lowered)
}

#[test]
fn counted_artifact_modes_bind_payment_and_each_printed_option() {
    let manifest = include_str!("../../../tests/fixtures/archive-manifest-current.json");
    let raw = include_str!("../../../tests/fixtures/synthetic-counted-artifact-modal.json");
    let parsed = compile(manifest, raw);
    let CompileResult::Lowered { definition } = parsed else {
        panic!("{parsed:?}")
    };
    let a = &definition.activated[0];
    assert!(matches!(
        a.cost,
        ActivationCost::SacrificePermanent { count: 3, .. }
    ));
    assert_eq!(a.program.modes.len(), 4);
    for change in ["count", "order", "omission", "selection"] {
        let mut d = definition.clone();
        match change {
            "count" => {
                let ActivationCost::SacrificePermanent { count, .. } = &mut d.activated[0].cost
                else {
                    panic!()
                };
                *count = 2;
            }
            "order" => d.activated[0].program.modes.swap(0, 2),
            "omission" => {
                d.activated.clear();
                d.source_labels = None;
            }
            _ => {
                d.activated[0].program.modes[1].program.targets[0].selection_id = "line:1:1".into()
            }
        }
        d.capabilities = d.requirements();
        assert!(d.validate().is_err(), "{change}");
    }
    for change in [
        "payer", "payment", "target", "event", "result", "operator", "number",
    ] {
        let edited = crate::tests::changed_source(raw, |r| {
            if matches!(change, "operator" | "number") {
                let result = &mut r["faces"][0]["result"];
                let opt = &mut result["abilities"][0]["effects"][0]["steps"][0]["options"][0]["effects"]
                    [0]["steps"][0];
                if change == "number" {
                    opt["amount"]["value"] = json!(7)
                } else {
                    opt["effect"] = json!("lose-life")
                }
                result["lines"][0]["ability"] = result["abilities"][0].clone();
                return;
            }
            let (field, kind, key, value) = match change {
                "payer" => ("references", "/costs/1/payer", "target", json!("other")),
                "payment" => (
                    "nodes",
                    "evaluate-reference",
                    "timing",
                    json!("stack-announcement"),
                ),
                "target" => ("nodes", "target", "scope", json!("other")),
                "event" => ("nodes", "pump", "inputs", json!([])),
                _ => ("nodes", "quantity", "identity", json!("same-objects")),
            };
            let row = r["binding"][field]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|r| {
                    r["selection"] == kind
                        || r["operation"] == kind
                        || r["result"] == kind
                        || r["path"].as_str().unwrap().ends_with(kind)
                })
                .unwrap();
            row[key] = value;
        });
        assert!(
            !matches!(compile(manifest, &edited), CompileResult::Lowered { .. }),
            "{change}"
        );
    }
}
