//! Fixed mode counts with independent printed target-player selections.
use super::*;
use serde_json::Value;

pub(super) fn lower(
    source: &Value,
    result: &ParseResult,
    binding: &Value,
    identity: &str,
) -> Result<Option<SpellProgram>, Failure> {
    let text = source["oracle_text"].as_str().unwrap_or("");
    if !matches!(text.lines().next(), Some("Choose two —" | "Choose three —")) {
        return Ok(None);
    }
    let ast: Value = decode(result.abilities.get(), "abilities")?;
    let timing = match source["type_line"].as_str() {
        Some("Instant") => SpellTiming::Instant,
        Some("Sorcery") => SpellTiming::MainPhase,
        _ => return Ok(None),
    };
    let Some(root_program) = mtg_contracts::modal_targets::fixed_player_modes(text, &ast, timing)
    else {
        return Ok(None);
    };
    let root = "/faces/0/abilities/0";
    let mut graph = bindings::Bindings::new(identity, source);
    graph.ability(root, "spell", "spell-controller");
    let scope = graph.id(root, "ability");
    for (i, mode) in root_program.modes.iter().enumerate() {
        let path = format!("{root}/effects/0/steps/0/options/{i}/effects/0/steps/0");
        let step = &ast[0]["effects"][0]["steps"][0]["options"][i]["effects"][0]["steps"][0];
        let target = graph.target(
            &scope,
            &format!("{path}/who"),
            &step["who"],
            &mode.program.targets[0].selection_id,
        );
        graph.event(
            root,
            &path,
            step["effect"]
                .as_str()
                .ok_or_else(|| unsupported("mode", "missing operation"))?,
            &target,
            false,
        );
        graph.result(&path, "quantity", "measured-quantity", None);
    }
    graph.validate(
        binding,
        "requires every modal target and event in its printed option",
    )?;
    Ok(Some(root_program))
}
