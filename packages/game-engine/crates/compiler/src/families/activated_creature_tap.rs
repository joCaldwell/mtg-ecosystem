//! The tap cost names the source and the effect names one announced creature.
//! CR 201.5, 601.2c, 602.2b and 701.26b.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(crate) fn bind(d: &Definition, source: &Value, binding: &Value) -> Result<(), Failure> {
    if !d.capabilities.contains(&Capability::ActivatedCreatureTapV1) {
        return Ok(());
    }
    let mut graph = bindings::Bindings::new(&d.oracle_id, source);
    if !d.keywords.is_empty() {
        let root = "/faces/0/abilities/0";
        graph.ability(root, "keywords", "static-source-controller");
        let scope = graph.id(root, "ability");
        for (index, keyword) in d.keywords.iter().enumerate() {
            let (word, rule) = mtg_contracts::activated_creature_tap::keyword(*keyword)
                .ok_or_else(|| unsupported("binding", "unbound tap source keyword"))?;
            let path = format!("{root}/keywords/{index}");
            let you = graph.node(
                &path,
                "you",
                "runtime-player",
                json!({
                    "scope":scope,"role":"static-source-controller","rules":["109.5",rule]
                }),
            );
            graph.node(
                &path,
                "keyword",
                "keyword-ability",
                json!({
                    "scope":scope,"keyword":word,"abilities":["static"],"you":you,"rules":[rule]
                }),
            );
        }
    }
    // The contract has checked the two costs and the whole printed target.
    let block = usize::from(!d.keywords.is_empty());
    let root = format!("/faces/0/abilities/{block}");
    let step = format!("{root}/effects/0/steps/0");
    graph.ability(&root, "activated", "activator");
    let scope = graph.id(&root, "ability");
    let payer = graph.id(&root, "you");
    let who = json!({"player":"you"});
    let object = graph.id("/source", "object");
    graph.reference(&format!("{root}/costs/0/payer"), &who, &payer, "109.5");
    graph.reference(
        &format!("{root}/costs/1/source"),
        &json!({"ref":"self"}),
        &object,
        "201.5",
    );
    graph.reference(&format!("{root}/costs/1/payer"), &who, &payer, "109.5");
    let selection = &d.activated[0].program.targets[0].selection_id;
    let target = graph.target(
        &scope,
        &format!("{step}/what"),
        &json!({
            "ref":"target","filter":{"types":["creature"]},
            "count":{"amount":"fixed","value":1},"selectionId":selection
        }),
        selection,
    );
    graph.event(&root, &step, "tap", &target, false);
    graph.validate(
        binding,
        "requires exact source tap cost, payer and creature target",
    )
}
