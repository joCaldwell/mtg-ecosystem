//! The combat recipient and the trigger's controller are separate participants.
use super::*;
use crate::bindings::Bindings;
use serde_json::{Value, json};

pub(crate) fn is_offer(effects: &[Sentence]) -> bool {
    let [
        Sentence {
            condition: None,
            condition_kind: None,
            bindings,
            steps,
        },
    ] = effects
    else {
        return false;
    };
    let [
        Step::PlayCard {
            instruction,
            optional: true,
        },
    ] = steps.as_slice()
    else {
        return false;
    };
    bindings.is_empty()
        && *instruction
            == mtg_contracts::combat_hand_cast_ast()[1]["effects"][0]["steps"][0]["instruction"]
}

pub(crate) fn bind(
    definition: &Definition,
    source: &Value,
    binding: &Value,
) -> Result<(), Failure> {
    let operation = binding["nodes"]
        .as_array()
        .is_some_and(|nodes| nodes.iter().any(|node| node["operation"] == "play-card"));
    if !definition
        .capabilities
        .contains(&Capability::CombatHandCastV1)
    {
        if operation
            && source["oracle_text"]
                .as_str()
                .is_some_and(|text| text.contains("cast a spell from your hand"))
        {
            return Err(unsupported(
                "binding",
                "unconsumed combat hand casting permission",
            ));
        }
        return Ok(());
    }
    let mut graph = Bindings::new(&definition.oracle_id, source);
    creature_entry_return::flying_binding(&mut graph, "/faces/0/abilities/0");
    let base = "/faces/0/abilities/1";
    let trigger = format!("{base}/trigger");
    let step = format!("{base}/effects/0/steps/0");
    let instruction = format!("{step}/instruction");
    let what = format!("{instruction}/what");
    let spell =
        json!({"ref":"filter","filter":{"cls":"spell"},"count":{"amount":"fixed","value":1}});
    let you = json!({"player":"you"});
    let source = graph.id("/source", "object");
    let controller = graph.id(base, "you");
    let scope = graph.id(base, "ability");
    let query = graph.id(&what, "query");
    graph.ability(base, "triggered", "trigger-controller");
    graph.reference(
        &format!("{trigger}/what"),
        &json!({"ref":"this","noun":"creature"}),
        &source,
        "201.5",
    );
    graph.event(base, &trigger, "trigger:deals-damage", &source, false);
    graph.result(
        &trigger,
        "objects",
        "same-objects",
        Some(&json!({"types":["creature"]})),
    );
    graph.result(&trigger, "player", "same-objects", None);
    graph.result(&trigger, "quantity", "measured-quantity", None);
    graph.reference(&format!("{instruction}/who"), &you, &controller, "109.5");
    graph.node(
        &what,
        "query",
        "selection",
        json!({"scope":scope,"expression":spell,"cardinality":"one",
        "timing":"resolution","selection":"evaluate-reference","rules":["608.2h"]}),
    );
    graph.reference(&what, &spell, &query, "608.2h");
    for actor in ["from/owner", "alternativeCost/payer"] {
        graph.reference(
            &format!("{instruction}/{actor}"),
            &you,
            &controller,
            "109.5",
        );
    }
    graph.node(
        &step,
        "event",
        "event",
        json!({"scope":scope,"operation":"play-card","inputs":[query,controller],
        "availability":"when-performed","optional":true,"decider":controller,"rules":["608.2c"]}),
    );
    graph.validate_ordered(
        binding,
        "requires one optional controller hand spell on combat damage",
    )
}
