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
        .or_else(|| mtg_contracts::modal_targets::mixed_target_modes(text, &ast, timing))
    else {
        return Ok(None);
    };
    if !matches!(result.lines.as_slice(),[line] if line.text==text) {
        return Err(unsupported("mode", "incomplete printed mode block"));
    }
    let root = "/faces/0/abilities/0";
    let mut graph = bindings::Bindings::new(identity, source);
    graph.ability(root, "spell", "spell-controller");
    let scope = graph.id(root, "ability");
    for (i, mode) in root_program.modes.iter().enumerate() {
        let path = format!("{root}/effects/0/steps/0/options/{i}/effects/0/steps/0");
        let step = &ast[0]["effects"][0]["steps"][0]["options"][i]["effects"][0]["steps"][0];
        let clause = text
            .lines()
            .nth(i + 1)
            .and_then(|line| line.strip_prefix("• "))
            .ok_or_else(|| unsupported("mode", "missing source clause"))?;
        match step["effect"].as_str() {
            Some("return-to-hand") => {
                super::creature_planeswalker_return::bind_step(
                    &mut graph,
                    root,
                    &path,
                    &step["what"],
                    &mode.program.targets[0].selection_id,
                );
                continue;
            }
            Some("shuffle-into-library") => {
                let shape =
                    mtg_contracts::graveyard_shuffle::GraveyardShuffle::from_clause(clause, step)
                        .ok_or_else(|| unsupported("mode", "invalid targeted shuffle"))?;
                super::graveyard_shuffle::bind_step(&mut graph, root, &path, step, &shape);
                continue;
            }
            Some("counter" | "put-counters") => {
                let counters = step["effect"] == "put-counters";
                let key = if counters { "on" } else { "what" };
                if counters {
                    graph.reference(
                        &format!("{path}/who"),
                        &serde_json::json!({"player":"you"}),
                        &graph.id(root, "you"),
                        "109.5",
                    );
                }
                let target = graph.target(
                    &scope,
                    &format!("{path}/{key}"),
                    &step[key],
                    &mode.program.targets[0].selection_id,
                );
                graph.event(
                    root,
                    &path,
                    if counters { "put-counters" } else { "counter" },
                    &target,
                    false,
                );
                if counters {
                    graph.result(&path, "quantity", "measured-quantity", None);
                }
                continue;
            }
            _ => {}
        }
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
