use super::*;
use serde_json::{Value, json};

fn bind_step(graph: &mut bindings::Bindings<'_>, root: &str, step: &Value) -> Result<(), Failure> {
    let where_ = format!("{root}/effects/0/steps/0");
    let scope = graph.id(root, "ability");
    let what = &step["what"];
    let id = what["selectionId"]
        .as_str()
        .ok_or_else(|| unsupported("combat.restriction", "missing target identity"))?;
    let target = graph.target(&scope, &format!("{where_}/what"), what, id);
    graph.event(root, &where_, "cant", &target, false);
    Ok(())
}

pub(super) fn lower_spell(
    source: &Value,
    result: &ParseResult,
    binding: &Value,
    identity: &str,
    kind: CardKind,
) -> Result<Option<SpellProgram>, Failure> {
    let text = source["oracle_text"].as_str().unwrap_or_default();
    if !text
        .lines()
        .next()
        .is_some_and(|l| mtg_contracts::combat_window::restriction(l).is_some())
    {
        return Ok(None);
    }
    let raw: Value = decode(result.abilities.get(), "combat.restriction")?;
    let Some(p) = mtg_contracts::combat_window::spell(text, &raw, kind) else {
        return Ok(None);
    };
    let mut graph = bindings::Bindings::new(identity, source);
    let root = "/faces/0/abilities/0";
    graph.ability(root, "spell", "spell-controller");
    bind_step(&mut graph, root, &raw[0]["effects"][0]["steps"][0])?;
    if p.instructions.len() == 2 {
        let root = "/faces/0/abilities/1";
        let step = format!("{root}/effects/0/steps/0");
        graph.ability(root, "spell", "spell-controller");
        let you = graph.id(root, "you");
        graph.reference(
            &format!("{step}/who"),
            &json!({"player":"you"}),
            &you,
            "109.5",
        );
        graph.event(root, &step, "draw", &you, false);
        graph.result(
            &step,
            "objects",
            "new-objects",
            Some(&json!({"cls":"card"})),
        );
        graph.result(&step, "quantity", "measured-quantity", None);
    }
    graph.validate(binding, "requires exact combat restriction spell bindings")?;
    Ok(Some(p))
}

pub(super) fn lower_activation(
    block: usize,
    result: &ParseResult,
    binding: &Value,
    source: &Value,
    identity: &str,
) -> Result<Option<Ability>, Failure> {
    let line = &result.lines[block];
    if mtg_contracts::combat_window::split_activation(&line.text).is_none() {
        return Ok(None);
    }
    let ast: Value = decode(line.ability.get(), "combat.restriction")?;
    let a = mtg_contracts::combat_window::activation(&line.text, &ast, source_block(block)?)
        .ok_or_else(|| {
            unsupported(
                "combat.restriction",
                "requires complete mana and tap cost and creature target",
            )
        })?;
    let root = format!("/faces/0/abilities/{block}");
    let mut graph = bindings::Bindings::new(identity, source);
    graph.ability(&root, "activated", "activator");
    let you = graph.id(&root, "you");
    let source = graph.id("/source", "object");
    for i in 0..=1 {
        graph.reference(
            &format!("{root}/costs/{i}/payer"),
            &json!({"player":"you"}),
            &you,
            "109.5",
        );
    }
    graph.reference(
        &format!("{root}/costs/1/source"),
        &json!({"ref":"self"}),
        &source,
        "201.5",
    );
    bind_step(&mut graph, &root, &ast["effects"][0]["steps"][0])?;
    graph.validate_block(
        binding,
        &root,
        "requires exact combat restriction activation binding",
    )?;
    Ok(Some(Ability::BoundActivation(a)))
}
