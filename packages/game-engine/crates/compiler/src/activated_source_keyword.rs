//! CR 201.5, 113.10, 602.2, 611.2c: exact paid self-grant closure.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::bindings::Bindings;
use mtg_contracts::activated_source_keyword::printed_grant;
use serde_json::{Value, json};

pub(super) fn lower(
    line: &SourceLine,
    block: usize,
    original: &RawValue,
    source: &Value,
    binding: &Value,
    kind: CardKind,
) -> Result<Option<ActivatedAbility>, Failure> {
    let Some(spec) = printed_grant(&line.text) else {
        return Ok(None);
    };
    let message = "requires complete printed mana-only self keyword grant";
    let fail = || unsupported("activation", message);
    let raw: Value = decode(original.get(), "activation")?;
    let line_ast: Value = decode(line.ability.get(), "activation")?;
    if kind != CardKind::Creature
        || source["layout"] != "normal"
        || spec.ast != raw
        || raw != line_ast
    {
        return Err(fail());
    }
    let root = format!("/faces/0/abilities/{block}");
    let identity = source["oracle_id"].as_str().ok_or_else(fail)?;
    let mut graph = Bindings::new(identity, source);
    graph.ability(&root, "activated", "activator");
    let scope = graph.id(&root, "ability");
    let source_id = graph.id("/source", "object");
    graph.reference(
        &format!("{root}/costs/0/payer"),
        &json!({"player":"you"}),
        &graph.id(&root, "you"),
        "109.5",
    );
    let step = format!("{root}/effects/0/steps/0");
    graph.reference(
        &format!("{step}/what"),
        &json!({"ref":"this","noun":"creature"}),
        &source_id,
        "201.5",
    );
    for (n, &keyword) in spec.keywords.iter().enumerate() {
        let recipient = format!("{step}/abilities/{n}");
        let recipient_id = graph.node(
            &recipient,
            "recipient",
            "recipient",
            json!({"scope":scope,"of":source_id,"each":false,"rules":["113.10","201.5a"]}),
        );
        let applied = graph.node(
            &recipient,
            "ability",
            "ability",
            json!({
                "source":recipient_id,"face":graph.id("/faces/0","face"),"abilityKind":"keywords",
                "you":graph.id(&recipient,"you"),"rules":["113","113.10"]
            }),
        );
        graph.node(&recipient, "you", "runtime-player", json!({
            "scope":applied,"role":"static-source-controller","rules":["109.5","301.5d","303.4e"]
        }));
        let path = format!("{recipient}/keyword");
        let (word, rule) = super::basic_landwalk::keyword_metadata(keyword).ok_or_else(fail)?;
        let you = graph.node(
            &path,
            "you",
            "runtime-player",
            json!({
                "scope":applied,"role":"static-source-controller","rules":["109.5",rule]
            }),
        );
        graph.node(
            &path,
            "keyword",
            "keyword-ability",
            json!({
                "scope":applied,"keyword":word,"abilities":["static"],"you":you,"rules":[rule]
            }),
        );
    }
    graph.event(&root, &step, "gain-abilities", &source_id, false);
    graph.validate_block(binding, &root, message)?;
    Ok(Some(spec.activation(source_block(block)?)))
}
