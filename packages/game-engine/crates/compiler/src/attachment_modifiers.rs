use super::*;
use serde_json::{Value, json};

pub(super) fn lower(
    block: usize,
    result: &ParseResult,
    source: &Value,
    binding: &Value,
    identity: &str,
) -> Result<Option<StaticAbility>, Failure> {
    let line = &result.lines[block];
    let Some((ability, raw)) = mtg_contracts::attachment_modifiers::printed(&line.text) else {
        return Ok(None);
    };
    if decode::<Value>(line.ability.get(), "static modifier")? != raw {
        return Err(unsupported(
            "static modifier",
            "requires exact attached creature modifier",
        ));
    }
    let base = format!("/faces/0/abilities/{block}");
    let mut graph = bindings::Bindings::new(identity, source);
    graph.ability(&base, "static", "static-source-controller");
    let scope = graph.id(&base, "ability");
    let target = graph.attachment(
        &base,
        &format!("{base}/effect/what"),
        &raw["effect"]["what"],
    );
    if let Some(index) = raw["effect"]["modifiers"]
        .as_array()
        .and_then(|mods| mods.iter().position(|m| m["modifier"] == "abilities"))
    {
        let path = format!("{base}/effect/modifiers/{index}/abilities/0");
        let recipient = graph.node(
            &path,
            "recipient",
            "recipient",
            json!({"scope":scope,"of":target,"each":false,"rules":["113.10","201.5a"]}),
        );
        let grant = graph.node(
            &path,
            "ability",
            "ability",
            json!({"source":recipient,"face":graph.id("/faces/0","face"),
            "abilityKind":"keywords","you":graph.id(&path,"you"),"rules":["113","113.10"]}),
        );
        graph.node(&path,"you","runtime-player",json!({"scope":grant,"role":"static-source-controller","rules":["109.5","301.5d","303.4e"]}));
        let kw = format!("{path}/keyword");
        let you = graph.node(
            &kw,
            "you",
            "runtime-player",
            json!({"scope":grant,"role":"static-source-controller","rules":["109.5","702.16"]}),
        );
        graph.node(&kw,"keyword","keyword-ability",json!({"scope":grant,"keyword":"protection","abilities":["static"],"you":you,"rules":["702.16"]}));
    }
    graph.validate_block(
        binding,
        &base,
        "incomplete attached creature modifier graph",
    )?;
    Ok(Some(ability))
}
