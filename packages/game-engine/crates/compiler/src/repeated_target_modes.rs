use super::*;
use serde_json::{Value, json};

pub(super) fn lower(
    source: &Value,
    result: &ParseResult,
    binding: &Value,
    record: &Record,
) -> Result<Option<SpellProgram>, Failure> {
    let text = source["oracle_text"].as_str().unwrap_or("");
    if text.lines().next() != Some(mtg_contracts::repeated_target_modes::HEADER) {
        return Ok(None);
    }
    let fail = || {
        unsupported(
            "repeated modes",
            "requires every printed effect, source and announced target",
        )
    };
    let timing = match source["type_line"].as_str() {
        Some("Instant") => SpellTiming::Instant,
        Some("Sorcery") => SpellTiming::MainPhase,
        _ => return Err(fail()),
    };
    let ast: Value = decode(result.abilities.get(), "abilities")?;
    let child = mtg_contracts::repeated_target_modes::program(
        source["name"].as_str().ok_or_else(fail)?,
        text,
        &ast,
        timing,
    )
    .ok_or_else(fail)?;
    let block = mtg_contracts::repeated_target_modes::parser_block(&result.name, text);
    if !matches!(result.lines.as_slice(),[line] if line.text==block) {
        return Err(unsupported(
            "repeated modes.labels",
            "parser block differs from the complete printed source after self-reference normalization",
        ));
    }
    // The old archive stored seven unsupported-scope references. All three
    // hashes are verified on the original unmodified row before lowering.
    let replacement;
    let graph_data = if record.source_sha256
        == "0154de44c3a06911e0b78a7817852254d5d88e3ccde5ed39940dfd0b3db0b483"
        && record.ast_sha256.as_deref()
            == Some("f5c4f55a057fd8ae0833246abcc19cde023854e9f3d40144e84be7ba29188ee2")
        && record.binding_sha256
            == "8e1158bab7c6c83cdd65b9af75ba1bb8937bbe2fbe02e00c892c0c010b186a68"
    {
        let raw = include_str!("rebindings/repeated-modal.json");
        if sha256(raw.as_bytes())
            != "6690380c2c3b99cd411f03dcd60795232b9b029159cf0fabf23b496a149eee36"
        {
            return Err(fail());
        }
        replacement = decode(raw, "rebound modal selection")?;
        &replacement
    } else {
        binding
    };
    let base = "/faces/0/abilities/0";
    let modal = format!("{base}/effects/0/steps/0");
    let mut g = bindings::Bindings::new(&record.oracle_identity, source);
    g.ability(base, "spell", "spell-controller");
    let scope = g.id(base, "ability");
    let src = g.id("/source", "object");
    for (i, m) in child.modes.iter().enumerate() {
        let step = &ast[0]["effects"][0]["steps"][0]["options"][i]["effects"][0]["steps"][0];
        let path = format!("{modal}/options/{i}/effects/0/steps/0");
        let id = &m.program.targets[0].selection_id;
        let operation = step["effect"].as_str().ok_or_else(fail)?;
        let key = match operation {
            "phase-out" | "destroy" => "what",
            "draw" => "who",
            "exile" => "what/owner",
            "damage" => "to",
            _ => return Err(fail()),
        };
        if operation == "damage" {
            g.reference(&format!("{path}/source"), &step["source"], &src, "201.5");
        }
        let expr = if operation == "damage" {
            json!({"ref":"any-target","selectionId":id})
        } else {
            step.pointer(&format!("/{key}")).ok_or_else(fail)?.clone()
        };
        let mut target = g.target(&scope, &format!("{path}/{key}"), &expr, id);
        if operation == "exile" {
            let what = format!("{path}/what");
            target=g.node(&what,"query","selection",json!({"scope":scope,"expression":step["what"],"cardinality":"many","timing":"resolution","selection":"evaluate-reference","rules":["608.2h"]}));
            g.reference(&what, &step["what"], &target, "608.2h");
        }
        if operation == "damage" {
            g.node(&path,"event","event",json!({"scope":scope,"operation":operation,"inputs":[src,target],"availability":"when-performed","rules":["608.2c"]}));
        } else {
            g.event(base, &path, operation, &target, false);
        }
        if operation == "draw" {
            g.result(
                &path,
                "objects",
                "new-objects",
                Some(&json!({"cls":"card"})),
            );
        }
        if operation == "exile" {
            g.node(&path,"objects","event-result",json!({"event":g.id(&path,"event"),"result":"objects","cardinality":"many","availability":"if-produced","identity":"zone-successors","description":{"cls":"card"},"rules":["400.7j"]}));
        }
        if matches!(operation, "draw" | "exile" | "damage") {
            g.result(&path, "quantity", "measured-quantity", None);
        }
    }
    g.validate_ordered_with_features(graph_data,&[json!({"feature":"repeatable-modal-selection","path":modal,"message":"Each chosen occurrence needs its own announced targets and resolves in printed mode order.","rules":["700.2","601.2b","601.2c","608.2c"]})],"requires all selected repeat templates and their ordered reference graph")?;
    Ok(Some(child))
}
