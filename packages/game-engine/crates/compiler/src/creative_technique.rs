//! CR 400.7j, 608.2c–g, 701.20, 701.24, 702.144a, and 707.10.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::bindings::Bindings;
use serde_json::{Value, json};

pub(super) fn candidate(source: &Source, raw: &Value) -> bool {
    raw["keywords"].as_array().is_some_and(|ks| ks.iter().any(|k| k == "Demonstrate"))
        || source.oracle_text.lines().any(|line| line.starts_with("Demonstrate"))
        || source.oracle_text.contains(
            "Shuffle your library, then reveal cards from the top of it until you reveal a nonland card.",
        )
}

pub(super) fn validate_binding(
    identity: &str,
    source: &Value,
    binding: &Value,
) -> Result<(), Failure> {
    // Definition validation already proved the complete printed spell and AST.
    let mut graph = Bindings::new(identity, source);
    let keywords = "/faces/0/abilities/0";
    let demonstrate = "/faces/0/abilities/0/keywords/0";
    graph.ability(keywords, "keywords", "static-source-controller");
    let scope = graph.id(keywords, "ability");
    let you = graph.node(
        demonstrate,
        "you",
        "runtime-player",
        json!({
            "scope":scope,"role":"trigger-controller","rules":["109.5","702.144"]
        }),
    );
    graph.node(demonstrate, "keyword", "keyword-ability", json!({
        "scope":scope,"keyword":"demonstrate","abilities":["triggered"],"you":you,"rules":["702.144"]
    }));

    let spell = "/faces/0/abilities/1";
    let shuffle = "/faces/0/abilities/1/effects/0/steps/0";
    let scan = "/faces/0/abilities/1/effects/0/steps/1";
    let matched = "/faces/0/abilities/1/effects/0/steps/1/until";
    let exile = "/faces/0/abilities/1/effects/1/steps/0";
    let bottom = "/faces/0/abilities/1/effects/1/steps/1";
    let bottom_what = "/faces/0/abilities/1/effects/1/steps/1/what";
    let cast = "/faces/0/abilities/1/effects/2/steps/0";
    graph.ability(spell, "spell", "spell-controller");
    let scope = graph.id(spell, "ability");
    let you = graph.id(spell, "you");
    let player = json!({"player":"you"});
    let nonland = json!({"nonTypes":["land"],"cls":"card"});
    graph.event(spell, shuffle, "shuffle", &you, false);
    graph.event(spell, scan, "scan-library", &you, false);
    let scan_event = graph.id(scan, "event");
    let revealed = graph.node(
        scan,
        "objects",
        "event-result",
        json!({
            "event":scan_event,"result":"objects","cardinality":"many","availability":"if-produced",
            "identity":"same-objects","description":{"cls":"card"},"rules":["608.2h"]
        }),
    );
    let hit = graph.node(
        matched,
        "objects",
        "event-result",
        json!({
            "event":scan_event,"result":"objects","cardinality":"one","availability":"if-produced",
            "identity":"same-objects","description":nonland,"subsetOf":revealed,"rules":["608.2h"]
        }),
    );
    graph.event(spell, exile, "exile", &hit, false);
    let successor = graph.node(exile, "objects", "event-result", json!({
        "event":graph.id(exile,"event"),"result":"objects","cardinality":"one",
        "availability":"if-produced","identity":"zone-successors","description":nonland,"rules":["400.7j"]
    }));
    let remainder = graph.node(
        bottom_what,
        "remainder",
        "remainder",
        json!({
            "scope":scope,"pool":revealed,"excluding":hit,"rules":["608.2c"]
        }),
    );
    graph.event(spell, bottom, "move-zone", &remainder, false);
    graph.result(bottom, "quantity", "measured-quantity", None);
    graph.node(cast, "event", "event", json!({
        "scope":scope,"operation":"play-card","inputs":[successor,you],"availability":"when-performed",
        "optional":true,"decider":you,"requiresResult":successor,"rules":["608.2c"]
    }));

    for path in [shuffle, scan] {
        graph.reference(&format!("{path}/who"), &player, &you, "109.5");
        graph.reference(&format!("{path}/library/owner"), &player, &you, "109.5");
    }
    graph.reference_rules(
        &format!("{exile}/what"),
        &json!({"ref":"that","filter":{"cls":"card"}}),
        &hit,
        &["608.2k", "400.7j", "608.2h"],
    );
    graph.reference(&format!("{bottom}/who"), &player, &you, "109.5");
    graph.reference(bottom_what, &json!({"ref":"rest"}), &remainder, "608.2c");
    graph.reference(&format!("{bottom}/to/owner"), &player, &you, "109.5");
    graph.reference(&format!("{cast}/instruction/who"), &player, &you, "109.5");
    graph.reference_rules(
        &format!("{cast}/instruction/what"),
        &json!({"ref":"related","number":"singular","filter":{"cls":"card"},
            "relation":{"relation":"event-result","event":"exile","scope":"contextual"}}),
        &successor,
        &["400.7j", "608.2c"],
    );
    graph.reference(
        &format!("{cast}/instruction/alternativeCost/payer"),
        &player,
        &you,
        "109.5",
    );
    graph.validate_ordered_with_features(binding, &[
        json!({"feature":"demonstrate","path":demonstrate,
            "message":"On cast, the trigger controller may copy the spell. If they do, they choose an opponent who also copies it. Each may choose new targets for their own copy as it is put onto the stack.",
            "rules":["702.144a","707.10","707.10c"]}),
        json!({"feature":"reveal-and-cast","path":format!("{spell}/effects"),
            "message":"Shuffle the controller's library, reveal through the first nonland or exhaustion, exile the match and put the other revealed cards on the bottom in random order. If the exile successor exists, the resolving spell's controller may cast that card during resolution without paying its mana cost if legal.",
            "rules":["400.7j","601.2","608.2c","608.2d","608.2g","609.3","701.20","701.24"]}),
    ], "requires complete demonstrate and reveal-and-cast bindings")
}

#[cfg(test)]
mod tests;
