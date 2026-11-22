use super::*;
use mtg_contracts::intrinsic_keyword_blocks::source;
use serde_json::Value;

pub(crate) fn lower(
    card: &Source,
    shape: &Value,
    kind: CardKind,
    result: &ParseResult,
    binding: &Value,
) -> Result<BTreeMap<usize, Vec<KeywordAbility>>, Failure> {
    let field = "abilities.intrinsic_keywords";
    let blocks = source(&card.oracle_text, result.abilities.get())
        .map_err(|message| unsupported(field, message))?;
    if blocks.is_empty() {
        return Ok(BTreeMap::new());
    }
    if kind != CardKind::Creature || card.layout != "normal" {
        return Err(unsupported(field, "requires a normal creature source"));
    }
    let labels = result
        .lines
        .iter()
        .map(|line| line.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    if source(&labels, result.abilities.get()).map_err(|message| unsupported(field, message))?
        != blocks
    {
        return Err(unsupported(
            field,
            "intrinsic keyword labels differ from the source",
        ));
    }
    if blocks.len() == result.lines.len() {
        let occurrences = blocks
            .iter()
            .map(|b| b.occurrences.iter().map(|words| words[0]).collect())
            .collect::<Vec<_>>();
        basic_landwalk::bind_intrinsic_keywords(
            &card.oracle_id,
            shape,
            binding,
            &occurrences,
            field,
            "requires complete intrinsic keyword binding graph",
        )?;
        return Ok(blocks
            .into_iter()
            .map(|b| (b.index, b.occurrences.into_iter().flatten().collect()))
            .collect());
    }
    for entry in binding["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .chain(binding["references"].as_array().into_iter().flatten())
    {
        if let Some(path) = entry["path"]
            .as_str()
            .and_then(|p| p.strip_prefix("/faces/0/abilities/"))
        {
            if path
                .split('/')
                .next()
                .and_then(|n| n.parse::<usize>().ok())
                .is_none_or(|i| i >= result.lines.len())
            {
                return Err(unsupported(
                    "binding",
                    "intrinsic composition has an unowned ability block",
                ));
            }
        }
    }
    let mut programs = BTreeMap::new();
    for block in blocks {
        let mut graph = bindings::Bindings::new(&card.oracle_id, shape);
        let root = format!("/faces/0/abilities/{}", block.index);
        basic_landwalk::bind_block(
            &mut graph,
            &root,
            block.occurrences.iter().map(|words| words[0]),
            field,
        )?;
        graph.validate_block(
            binding,
            &root,
            "requires complete intrinsic keyword binding block",
        )?;
        programs.insert(
            block.index,
            block.occurrences.into_iter().flatten().collect(),
        );
    }
    Ok(programs)
}
