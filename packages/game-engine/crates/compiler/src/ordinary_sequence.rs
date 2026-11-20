//! Reconstruct the full archived operand graph of an ordinary scalar sequence.
use super::*;
use serde_json::{Value, json};

pub(super) fn lower(
    source: &Value,
    result: &ParseResult,
    binding: &Value,
    identity: &str,
    kind: CardKind,
) -> Result<Option<SpellProgram>, Failure> {
    let text = source["oracle_text"].as_str().unwrap_or_default();
    if !mtg_contracts::ordinary_sequence::printed_candidate(text) {
        return Ok(None);
    }
    let fail = || {
        unsupported(
            "scalar sequence",
            "requires complete ordinary printed and typed steps",
        )
    };
    let timing = match kind {
        CardKind::Instant => SpellTiming::Instant,
        CardKind::Sorcery => SpellTiming::MainPhase,
        _ => return Err(fail()),
    };
    let raw: Vec<Value> = decode(result.abilities.get(), "abilities")?;
    let program =
        mtg_contracts::ordinary_sequence::lower(text, &json!(raw), timing).ok_or_else(fail)?;
    bind(source, &raw, binding, identity)?;
    Ok(Some(program))
}

pub(super) fn bind(
    source: &Value,
    raw: &[Value],
    binding: &Value,
    identity: &str,
) -> Result<(), Failure> {
    let fail = || {
        unsupported(
            "binding",
            "scalar sequence requires its complete archived operand graph",
        )
    };
    let mut graph = bindings::Bindings::new(identity, source);
    let controller = json!({"player":"you"});
    for (block, ability) in raw.iter().enumerate() {
        let base = format!("/faces/0/abilities/{block}");
        graph.ability(&base, "spell", "spell-controller");
        let scope = graph.id(&base, "ability");
        let you = graph.id(&base, "you");
        for (effect, sentence) in ability["effects"]
            .as_array()
            .ok_or_else(fail)?
            .iter()
            .enumerate()
        {
            for (step, operation) in sentence["steps"]
                .as_array()
                .ok_or_else(fail)?
                .iter()
                .enumerate()
            {
                let at = format!("{base}/effects/{effect}/steps/{step}");
                let op = operation["effect"].as_str().ok_or_else(fail)?;
                let inputs = match op {
                    "scry" | "surveil" => vec![],
                    "additional-land-plays" => {
                        graph.reference(&format!("{at}/allowance/who"), &controller, &you, "109.5");
                        vec![]
                    }
                    "destroy" | "exile" | "return-to-hand" => {
                        let what = &operation["what"];
                        let id = what["selectionId"].as_str().ok_or_else(fail)?;
                        let target = graph.target(&scope, &format!("{at}/what"), what, id);
                        if op == "return-to-hand" {
                            bind_owner(&mut graph, &scope, &at, &target);
                        }
                        vec![target]
                    }
                    "mill" if operation["who"]["player"] == "target-player" => {
                        let who = &operation["who"];
                        let id = who["selectionId"].as_str().ok_or_else(fail)?;
                        vec![graph.target(&scope, &format!("{at}/who"), who, id)]
                    }
                    "draw" | "mill" | "gain-life" | "lose-life" => {
                        graph.reference(&format!("{at}/who"), &controller, &you, "109.5");
                        vec![you.clone()]
                    }
                    "discard" => {
                        let what = &operation["what"];
                        let path = format!("{at}/what");
                        let cardinality = cardinality(&what["count"]["value"]);
                        let selected = graph.node(&path, "query", "selection", json!({
                            "scope":scope,"expression":what,"cardinality":cardinality,
                            "timing":"resolution","selection":"evaluate-reference","rules":["608.2h"]
                        }));
                        graph.reference(&format!("{at}/who"), &controller, &you, "109.5");
                        graph.reference(&path, what, &selected, "608.2h");
                        vec![selected, you.clone()]
                    }
                    _ => return Err(fail()),
                };
                let event = graph.node(&at, "event", "event", json!({
                    "scope":scope,"operation":op,"inputs":inputs,"availability":"when-performed","rules":["608.2c"]
                }));
                if op == "exile" {
                    let mut description = operation["what"]["filter"].clone();
                    description["cls"] = json!("card");
                    graph.node(
                        &at,
                        "objects",
                        "event-result",
                        json!({
                            "event":event,"result":"objects","cardinality":"one",
                            "availability":"if-produced","identity":"zone-successors",
                            "description":description,"rules":["400.7j"]
                        }),
                    );
                }
                if matches!(op, "draw" | "mill" | "discard") {
                    let (value, identity, rule) = if op == "discard" {
                        (
                            &operation["what"]["count"]["value"],
                            "zone-successors",
                            "400.7j",
                        )
                    } else {
                        (&operation["amount"]["value"], "new-objects", "608.2h")
                    };
                    graph.node(&at, "objects", "event-result", json!({
                        "event":event,"result":"objects","cardinality":cardinality(value),
                        "availability":"if-produced","identity":identity,"description":{"cls":"card"},"rules":[rule]
                    }));
                }
                if matches!(op, "draw" | "mill" | "gain-life" | "lose-life")
                    || (op == "discard" && operation["what"]["count"]["value"] != 1)
                {
                    graph.result(&at, "quantity", "measured-quantity", None);
                }
            }
        }
    }
    graph.validate(
        binding,
        "scalar sequence requires its complete archived operand graph",
    )
}

fn cardinality(value: &Value) -> &'static str {
    if *value == 1 { "one" } else { "many" }
}

fn bind_owner(graph: &mut bindings::Bindings<'_>, scope: &str, at: &str, target: &str) {
    let path = format!("{at}/to");
    let owner = graph.node(
        &path,
        "owner",
        "derived-player",
        json!({
            "scope":scope,"role":"owner","of":target,"rules":["108.3"]
        }),
    );
    graph.reference(
        &path,
        &json!({"player":"owner","of":{"ref":"it"}}),
        &owner,
        "108.3",
    );
}
