//! Bind exact named-color protection lines and ordered archive occurrence graph.
//! CR 702.16a–g: https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::Value;

pub(super) fn lower(
    source: &Source,
    shape: &Value,
    kind: CardKind,
    result: &ParseResult,
    binding: &Value,
) -> Result<Option<Vec<KeywordAbility>>, Failure> {
    let path = "abilities.fixed_color_protection";
    let Some(blocks) = mtg_contracts::fixed_color_protection::source_keywords(
        &source.oracle_text,
        result.abilities.get(),
    )
    .map_err(|message| unsupported(path, message))?
    else {
        return Ok(None);
    };
    if kind != CardKind::Creature || source.layout != "normal" || result.lines.len() != blocks.len()
    {
        return Err(unsupported(
            path,
            "fixed-color protection requires a normal creature",
        ));
    }
    let printed = result
        .lines
        .iter()
        .map(|line| line.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    if mtg_contracts::fixed_color_protection::source_keywords(&printed, result.abilities.get())
        .map_err(|message| unsupported(path, message))?
        .as_ref()
        != Some(&blocks)
    {
        return Err(unsupported(
            path,
            "printed color protection and source labels differ",
        ));
    }
    let graph_keywords: Vec<Vec<_>> = blocks
        .iter()
        .map(|block| block.iter().map(|occurrence| occurrence[0]).collect())
        .collect();
    super::basic_landwalk::bind_intrinsic_keywords(
        &source.oracle_id,
        shape,
        binding,
        &graph_keywords,
        path,
        "requires complete fixed-color protection keyword bindings",
    )?;
    Ok(Some(blocks.into_iter().flatten().flatten().collect()))
}
