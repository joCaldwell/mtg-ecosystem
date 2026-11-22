//! Bind the declared attack's same-object result to the exact pump operand.
//! CR 508.3a/611.2c: https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::bindings::Bindings;
use serde_json::{Value, json};

pub(crate) fn lower(
    card: &Source,
    shape: &Value,
    kind: CardKind,
    result: &ParseResult,
    binding: &Value,
) -> Result<Option<TriggeredAbility>, Failure> {
    let path = "abilities.source_attack_pump";
    let fail = |message: &str| unsupported(path, message);
    let parse = |text| {
        mtg_contracts::source_attack_pump::source(text, result.abilities.get())
            .map_err(|message| fail(&message))
    };
    let Some(pump) = parse(&card.oracle_text)? else {
        return Ok(None);
    };
    let labels = result
        .lines
        .iter()
        .map(|line| line.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    if kind != CardKind::Creature
        || card.layout != "normal"
        || parse(&labels)?.as_ref() != Some(&pump)
    {
        return Err(fail(
            "fixed self-attack pump requires exact normal creature source labels",
        ));
    }

    let mut graph = Bindings::new(&card.oracle_id, shape);
    for block in 0..result.lines.len() {
        let root = format!("/faces/0/abilities/{block}");
        if block != pump.block {
            let (_, keywords) = pump
                .keyword_blocks
                .iter()
                .find(|(index, _)| *index == block)
                .ok_or_else(|| fail("missing complete intrinsic keyword block"))?;
            graph.ability(&root, "keywords", "static-source-controller");
            let scope = graph.id(&root, "ability");
            for (index, &keyword) in keywords.iter().enumerate() {
                let path = format!("{root}/keywords/{index}");
                let (word, rule) = super::basic_landwalk::keyword_metadata(keyword)
                    .ok_or_else(|| fail("unsupported intrinsic keyword scope"))?;
                let role = if keyword == KeywordAbility::Flash {
                    "prospective-caster"
                } else {
                    "static-source-controller"
                };
                let you = graph.node(
                    &path,
                    "you",
                    "runtime-player",
                    json!({"scope":scope,"role":role,"rules":["109.5",rule]}),
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
            continue;
        }
        graph.ability(&root, "triggered", "trigger-controller");
        let trigger = format!("{root}/trigger");
        let step = format!("{root}/effects/0/steps/0");
        let source = graph.id("/source", "object");
        graph.reference(
            &format!("{trigger}/what"),
            &json!({"ref":"this","noun":"creature"}),
            &source,
            "201.5",
        );
        graph.event(&root, &trigger, "trigger:attacks", &source, false);
        graph.result(
            &trigger,
            "objects",
            "same-objects",
            Some(&json!({"types":["creature"]})),
        );
        let attacker = graph.id(&trigger, "objects");
        graph.reference_rules(
            &format!("{step}/what"),
            &json!({"ref":"it"}),
            &attacker,
            &["608.2k", "400.7j", "608.2h"],
        );
        graph.event(&root, &step, "pump", &attacker, false);
    }
    graph.validate_ordered(
        binding,
        "requires exact self-attack and same-object pump bindings",
    )?;
    Ok(Some(pump.ability))
}
