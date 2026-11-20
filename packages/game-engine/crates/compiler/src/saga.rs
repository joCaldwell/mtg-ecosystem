//! Complete single-face chapter source and its ordered controller bindings.
use super::*;
use crate::bindings::Bindings;
use serde_json::{Value, json};

pub(super) fn bind(
    source: &Value,
    id: &str,
    binding: &Value,
    parsed: &ParseResult,
    abilities: &[Ability],
) -> Result<Option<Vec<mtg_contracts::LifeChapter>>, Failure> {
    let has_chapter = abilities
        .iter()
        .any(|a| matches!(a, Ability::Chapter { .. }))
        || binding["nodes"]
            .as_array()
            .is_some_and(|nodes| nodes.iter().any(|n| n["abilityKind"] == "chapter"));
    if !has_chapter && source["layout"] != "saga" {
        return Ok(None);
    }
    let invalid = || {
        unsupported(
            "abilities.chapter",
            "requires complete ordinary controller-life chapters",
        )
    };
    if source["type_line"] != "Enchantment — Saga" || source["layout"] != "saga" {
        return Err(invalid());
    }
    let text = source["oracle_text"].as_str().ok_or_else(invalid)?;
    let chapters = mtg_contracts::life_chapters(text).map_err(|_| invalid())?;
    let raw: Value = decode(parsed.abilities.get(), "abilities")?;
    if chapters.len() != abilities.len()
        || raw != Value::Array(chapters.iter().map(|c| c.ast()).collect())
        || !parsed
            .lines
            .iter()
            .map(|l| l.text.as_str())
            .eq(text.lines())
    {
        return Err(invalid());
    }
    let mut graph = Bindings::new(id, source);
    for (chapter, ability) in chapters.iter().zip(abilities) {
        let Ability::Chapter {
            chapters: numbers,
            effects,
        } = ability
        else {
            return Err(invalid());
        };
        if !chapter
            .numbers
            .iter()
            .map(|n| n.get())
            .eq(numbers.iter().copied())
            || effects.len() != 1
        {
            return Err(invalid());
        }
        let base = format!("/faces/0/abilities/{}", chapter.source_block);
        let step = format!("{base}/effects/0/steps/0");
        let you = graph.id(&base, "you");
        graph.ability(&base, "chapter", "trigger-controller");
        graph.reference(
            &format!("{step}/who"),
            &json!({"player":"you"}),
            &you,
            "109.5",
        );
        graph.event(&base, &step, "gain-life", &you, false);
        graph.result(&step, "quantity", "measured-quantity", None);
    }
    graph.validate_ordered(
        binding,
        "requires exact complete chapter actor and event graph",
    )?;
    Ok(Some(chapters))
}
