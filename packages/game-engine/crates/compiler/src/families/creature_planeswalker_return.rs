use super::*;
use serde_json::{Value, json};

pub(crate) fn bind_step(
    graph: &mut bindings::Bindings,
    root: &str,
    path: &str,
    what: &Value,
    selection: &str,
) {
    let scope = graph.id(root, "ability");
    let target = graph.target(&scope, &format!("{path}/what"), what, selection);
    let to = format!("{path}/to");
    let owner = graph.node(
        &to,
        "owner",
        "derived-player",
        json!({"scope":scope,"role":"owner","of":target,"rules":["108.3"]}),
    );
    graph.reference(
        &to,
        &json!({"player":"owner","of":{"ref":"it"}}),
        &owner,
        "108.3",
    );
    graph.event(root, path, "return-to-hand", &target, false);
}
pub(crate) fn lower(
    source: &Value,
    result: &ParseResult,
    binding: &Value,
    identity: &str,
) -> Result<Option<SpellProgram>, Failure> {
    let text = source["oracle_text"].as_str().unwrap_or("");
    if text != mtg_contracts::creature_planeswalker_return::CLAUSE {
        return Ok(None);
    }
    let fail = || {
        unsupported(
            "return target union",
            "requires the exact untargeted owner of a single announced creature or planeswalker",
        )
    };
    let ast: Value = decode(result.abilities.get(), "abilities")?;
    let step = &ast[0]["effects"][0]["steps"][0];
    let timing = match source["type_line"].as_str() {
        Some("Instant") => SpellTiming::Instant,
        Some("Sorcery") => SpellTiming::MainPhase,
        _ => return Err(fail()),
    };
    let p = mtg_contracts::creature_planeswalker_return::program(text, step, timing, 0)
        .ok_or_else(fail)?;
    if ast != json!([{"kind":"spell","effects":[{"steps":[step]}]}])
        || !matches!(result.lines.as_slice(),[line] if line.text==text)
    {
        return Err(fail());
    }
    let root = "/faces/0/abilities/0";
    let mut graph = bindings::Bindings::new(identity, source);
    graph.ability(root, "spell", "spell-controller");
    bind_step(
        &mut graph,
        root,
        &format!("{root}/effects/0/steps/0"),
        &step["what"],
        &p.targets[0].selection_id,
    );
    graph.validate(binding, "requires the selected permanent's exact owner")?;
    Ok(Some(p))
}
