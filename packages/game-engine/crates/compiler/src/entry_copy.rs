//! CR 614.12a, 702.8, 702.85 and 707.2, 707.5–6.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::bindings::Bindings;
use serde_json::{Value, json};

pub(super) const MANIFEST_FILE: &str = "archive-manifest-protege-entry-copy.json";

pub(super) fn validate_binding(
    abilities: &[Ability],
    identity: &str,
    source: &Value,
    binding: &Value,
) -> Result<(), Failure> {
    let Some((what, model, optional)) = abilities.iter().find_map(|ability| match ability {
        Ability::Static {
            effect:
                SourceStaticEffect::EntryCopy {
                    what,
                    model,
                    optional,
                },
            ..
        } => Some((what, model, optional)),
        _ => None,
    }) else {
        return Ok(());
    };

    let mut graph = Bindings::new(identity, source);
    for (block, keyword, ability_kind, actor, rule) in [
        (0, "flash", "static", "prospective-caster", "702.8"),
        (1, "cascade", "triggered", "trigger-controller", "702.85"),
    ] {
        let path = format!("/faces/0/abilities/{block}");
        graph.ability(&path, "keywords", "static-source-controller");
        let scope = graph.id(&path, "ability");
        let keyword_path = format!("{path}/keywords/0");
        let you = graph.node(
            &keyword_path,
            "you",
            "runtime-player",
            json!({
                "scope":scope,"role":actor,"rules":["109.5",rule]
            }),
        );
        graph.node(
            &keyword_path,
            "keyword",
            "keyword-ability",
            json!({
                "scope":scope,"keyword":keyword,"abilities":[ability_kind],"you":you,"rules":[rule]
            }),
        );
    }
    let path = "/faces/0/abilities/2";
    let effect = "/faces/0/abilities/2/effect";
    let entrant = "/faces/0/abilities/2/effect/what";
    let model_path = "/faces/0/abilities/2/effect/model";
    let source_id = graph.id("/source", "object");
    let you_id = graph.id(path, "you");
    let scope = graph.node(
        path,
        "ability",
        "ability",
        json!({
            "source":source_id,"face":graph.id("/faces/0","face"),
            "abilityKind":"static","you":you_id,"rules":["113","109.5"]
        }),
    );
    let you = graph.node(
        path,
        "you",
        "runtime-player",
        json!({
            "scope":scope,"role":"entering-controller","rules":["109.5","110.2","614.12"]
        }),
    );
    let proposal = graph.node(
        entrant,
        "proposal",
        "battlefield-entry-proposal",
        json!({
            "scope":scope,"object":source_id,"to":"battlefield","controller":you,
            "rules":["110.2","614.12"]
        }),
    );
    let choice = graph.node(
        model_path,
        "choice",
        "entry-copy-model-choice",
        json!({
            "scope":scope,"proposal":proposal,"chooser":you,"timing":"before-entry",
            "selection":"non-target","count":"zero-or-one","zone":"battlefield",
            "filter":model["filter"],"historyIdentity":"current-incarnation",
            "rules":["110.1","115.10a","400.7","614.12a"]
        }),
    );
    graph.node(
        effect,
        "replacement",
        "entry-copy-replacement",
        json!({
            "scope":scope,"entry":proposal,"model":choice,"timing":"as-enters",
            "values":"model-copiable-values","onDecline":"leave-entry-unchanged",
            "rules":["614.12","707.2","707.2b","707.5"]
        }),
    );
    graph.reference_rules(entrant, what, &proposal, &["201.5", "614.12"]);
    graph.reference_rules(
        "/faces/0/abilities/2/effect/model/who",
        &model["who"],
        &you,
        &["110.2", "614.12a"],
    );
    graph.reference_rules(
        "/faces/0/abilities/2/effect/optional/who",
        &optional["who"],
        &you,
        &["110.2", "614.12a"],
    );
    graph.validate_ordered_with_features(binding, &[
        json!({"feature":"entry-copy-replacement","path":effect,
            "message":"Track entry turn for current battlefield incarnations, offer the optional choice before entry, and apply the chosen model's copiable values during entry replacement processing.",
            "rules":["400.7","614.12","614.12a","616.1c","707.2","707.5"]}),
    ], "requires the complete optional battlefield entry-copy binding")
}

#[cfg(test)]
mod tests;
