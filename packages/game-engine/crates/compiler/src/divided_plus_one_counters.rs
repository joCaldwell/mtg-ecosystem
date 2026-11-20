//! CR 601.2b–d: select the mode, then announce the target count, distinct objects
//! and positive division before costs. The frozen binder cannot express the
//! number choice. Only this complete printed clause and exact raw AST supply it.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::bindings::Bindings;
use serde_json::{Value, json};

const TEXT: &str = "Choose one —\n• Exile target creature with power 3 or greater.\n• You draw two cards and you lose 2 life.\n• Distribute two +1/+1 counters among one or two target creatures.";
const BASE: &str = "/faces/0/abilities/0";
const EXILE: &str = "line:1:2";
const AMONG: &str = "line:3:13";

fn abilities() -> Value {
    let fixed = |value| json!({"amount":"fixed","value":value});
    let exile = json!({"effect":"exile","what":{"ref":"target","filter":{"types":["creature"],
        "properties":[{"property":"power","comparison":{"op":"ge","value":fixed(3)}}]},
        "count":fixed(1),"selectionId":EXILE}});
    let controller = |effect| json!({"effect":effect,"who":{"player":"you"},"amount":fixed(2)});
    let division = json!({"effect":"distribute-counters","counter":{"type":"+1/+1"},"count":fixed(2),
        "among":{"ref":"target","filter":{"types":["creature"]},"count":{"amount":"chosen","minimum":1,"maximum":fixed(2)},"selectionId":AMONG}});
    let option = |steps: Vec<Value>| json!({"effects":[{"steps":steps}]});
    json!([{"kind":"spell","effects":[{"steps":[{"effect":"modal","count":{"exactly":1},"options":[
        option(vec![exile]),option(vec![controller("draw"),controller("lose-life")]),option(vec![division])
    ]}]}]}])
}

// Build every node, reference and diagnostic from the independently verified
// operands. In particular, the 'unknown' selection and candidate reference
// are diagnostic evidence only. Neither is executable selection authority.
fn expected_binding(identity: &str, source: &Value, ast: &Value) -> Value {
    let options = &ast[0]["effects"][0]["steps"][0]["options"];
    let step = |option, index| {
        format!("{BASE}/effects/0/steps/0/options/{option}/effects/0/steps/{index}")
    };
    let mut graph = Bindings::new(identity, source);
    graph.ability(BASE, "spell", "spell-controller");
    let scope = graph.id(BASE, "ability");
    let you = graph.id(BASE, "you");

    let exile = step(0, 0);
    let what = &options[0]["effects"][0]["steps"][0]["what"];
    let target = graph.target(&scope, &format!("{exile}/what"), what, EXILE);
    graph.event(BASE, &exile, "exile", &target, false);
    let mut description = what["filter"].clone();
    description["cls"] = json!("card");
    graph.node(&exile, "objects", "event-result", json!({"event":graph.id(&exile,"event"),"result":"objects",
        "cardinality":"one","availability":"if-produced","identity":"zone-successors","description":description,"rules":["400.7j"]}));

    for (index, op) in ["draw", "lose-life"].into_iter().enumerate() {
        let path = step(1, index);
        graph.reference(
            &format!("{path}/who"),
            &json!({"player":"you"}),
            &you,
            "109.5",
        );
        graph.event(BASE, &path, op, &you, false);
        if op == "draw" {
            graph.node(&path,"objects","event-result",json!({"event":graph.id(&path,"event"),"result":"objects","cardinality":"many",
                "availability":"if-produced","identity":"new-objects","description":{"cls":"card"},"rules":["608.2h"]}));
        }
        graph.result(&path, "quantity", "measured-quantity", None);
    }

    let path = step(2, 0);
    let among = format!("{path}/among");
    graph.target_with_unresolved_count(
        &scope,
        &among,
        &options[2]["effects"][0]["steps"][0]["among"],
        AMONG,
    );
    graph.with_diagnostics(vec![
        json!({"code":"unsupported-number-binding","path":format!("{among}/count"),
            "message":"This number needs a phase-specific choice identity and constraint scope.","rules":["107.3","601.2b"]}),
        json!({"code":"unsupported-scope","path":path,
            "message":"This instruction's event/result associations are not yet established.","rules":["608.2c"]}),
        json!({"code":"unresolved-dependency","path":among,
            "message":"This reference depends on an unresolved identity or number definition.","rules":["608.2h"]}),
    ])
}

pub(super) fn validate(
    source: &Value,
    result: &ParseResult,
    binding: &Value,
) -> Result<bool, Failure> {
    if source["oracle_text"] != TEXT {
        return Ok(false);
    }
    let raw: Value = decode(result.abilities.get(), "abilities")?;
    if source["type_line"] != "Instant"
        || source["keywords"] != json!([])
        || source["mana_cost"]
            .as_str()
            .is_none_or(|cost| cost.is_empty() || cost.contains("{X}"))
        || !matches!(result.lines.as_slice(), [line] if line.text == TEXT)
        || raw != abilities()
    {
        return Err(unsupported(
            "divided_plus_one_counters",
            "requires the complete choose-one instant and its exact three options",
        ));
    }
    let identity = source["oracle_id"]
        .as_str()
        .ok_or_else(|| invalid("source.oracle_id", "missing identity"))?;
    if *binding != expected_binding(identity, source, &raw) {
        return Err(unsupported(
            "binding",
            "requires the complete modal graph with only the divided-counter operand unresolved",
        ));
    }
    Ok(true)
}

#[cfg(test)]
mod tests;
