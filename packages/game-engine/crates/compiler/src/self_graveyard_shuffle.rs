//! CR 113.6b, 614.1a, 701.24c, 702.16j.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::bindings::Bindings;
use mtg_contracts::self_graveyard_shuffle::{
    PROTECTION_TEXT, REPLACEMENT_TEXT, abilities, candidate, source_matches,
};
use serde_json::{Value, json};

pub(super) fn validate(
    source: &Source,
    shape: &Value,
    result: &ParseResult,
    binding: &Value,
) -> Result<bool, Failure> {
    let raw = result.abilities.get();
    let path = "abilities.self_graveyard_shuffle";
    if !candidate(&source.oracle_text, raw).map_err(|err| invalid(path, err))? {
        return Ok(false);
    }
    let ast: Value = decode(raw, path)?;
    let blocks: Vec<_> = result.lines.iter().map(|line| line.text.as_str()).collect();
    if !source_matches(shape, &source.name)
        || blocks != [PROTECTION_TEXT, REPLACEMENT_TEXT]
        || ast != abilities()
    {
        return Err(unsupported(
            path,
            "requires the complete printed protection and graveyard replacement",
        ));
    }
    validate_binding(&source.oracle_id, shape, binding)?;
    Ok(true)
}

fn validate_binding(identity: &str, source: &Value, binding: &Value) -> Result<(), Failure> {
    let mut graph = Bindings::new(identity, source);
    let keywords = "/faces/0/abilities/0";
    let protection = "/faces/0/abilities/0/keywords/0";
    graph.ability(keywords, "keywords", "static-source-controller");
    let scope = graph.id(keywords, "ability");
    let you = graph.node(
        protection,
        "you",
        "runtime-player",
        json!({
            "scope":scope,"role":"static-source-controller","rules":["109.5","702.16"]
        }),
    );
    graph.node(
        protection,
        "keyword",
        "keyword-ability",
        json!({
            "scope":scope,"keyword":"protection","protectionFrom":"everything",
            "abilities":["static"],"you":you,"rules":["702.16"]
        }),
    );

    let block = "/faces/0/abilities/1";
    let replacement = format!("{block}/effect/replacement");
    let event = format!("{replacement}/event");
    let replace = format!("{replacement}/replace");
    let reveal = format!("{replace}/effects/0/steps/0");
    let shuffle = format!("{replace}/effects/0/steps/1");
    let owner_path = format!("{shuffle}/library/owner");
    let object = graph.id("/source", "object");
    let scope = graph.node(
        block,
        "ability",
        "ability",
        json!({
            "source":object,"face":graph.id("/faces/0","face"),"abilityKind":"static",
            "you":graph.id(block,"you"),"rules":["113","109.5"]
        }),
    );
    let you = graph.node(
        block,
        "you",
        "runtime-player",
        json!({
            "scope":scope,"role":"replacement-source-controller-or-owner","rules":["108.4a","109.5"]
        }),
    );
    let owner = graph.node(
        &owner_path,
        "owner",
        "derived-player",
        json!({
            "scope":scope,"role":"owner","of":object,"rules":["108.3","400.3"]
        }),
    );
    let proposal = graph.node(
        &event,
        "proposal",
        "zone-change-proposal",
        json!({
            "scope":scope,"object":object,"from":"anywhere","to":{"zone":"graveyard","owner":owner},
            "rules":["400.3","400.6","614.1a"]
        }),
    );
    graph.node(
        &replace,
        "replacement",
        "reveal-and-shuffle-replacement",
        json!({
            "scope":scope,"replaces":proposal,"occurrence":"each",
            "reveal":{"actor":you,"object":object},
            "shuffleInto":{"actor":you,"object":object,"library":{"zone":"library","owner":owner}},
            "rules":["614.6","701.20a","701.20b","701.24a","701.24c"]
        }),
    );

    let player = json!({"player":"you"});
    let affected = json!({"ref":"replacement-event-object","role":"affected"});
    graph.reference_rules(
        &format!("{event}/what"),
        &json!({"ref":"self"}),
        &object,
        &["201.5", "400.6"],
    );
    graph.reference_rules(
        &format!("{reveal}/who"),
        &player,
        &you,
        &["108.4a", "109.5"],
    );
    graph.reference_rules(
        &format!("{reveal}/what"),
        &affected,
        &object,
        &["201.5", "614.6", "701.20b"],
    );
    graph.reference_rules(
        &format!("{shuffle}/who"),
        &player,
        &you,
        &["108.4a", "109.5"],
    );
    graph.reference_rules(
        &format!("{shuffle}/what/0"),
        &affected,
        &object,
        &["614.6", "701.20b", "701.24c"],
    );
    graph.reference_rules(
        &owner_path,
        &json!({"player":"owner","of":affected}),
        &owner,
        &["108.3", "400.3"],
    );
    let features = [
        json!({"feature":"protection-from-everything","path":protection,
            "message":"Apply protection against all sources for targeting, damage, attachments, and blocking.",
            "rules":["702.16j"]}),
        json!({"feature":"graveyard-reveal-and-shuffle","path":replacement,
            "message":"Apply the replacement to a proposed graveyard move and shuffle the owner's library even if another effect redirects the library move.",
            "rules":["400.6","614.5","614.6","616.1","701.24c"]}),
    ];
    let message = "requires complete ordered protection and graveyard replacement bindings";
    if binding["engineFeatures"] != json!(features) {
        return Err(unsupported("binding", message));
    }
    graph.validate_ordered_with_features(binding, &features, message)
}

#[cfg(test)]
mod tests;
