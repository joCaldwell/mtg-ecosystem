//! Fixed-mana bestow announcement operands, CR 118.9 and 702.103a.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::bindings::Bindings;
use serde_json::{Value, json};

/// The cost belongs to the prospective caster. A static source controller is
/// a distinct bound actor even when both happen to be the same player.
pub(super) fn cost(
    source: &Value,
    result: &ParseResult,
    binding: &Value,
    identity: &str,
) -> Result<Option<ManaCost>, Failure> {
    let text = source["oracle_text"].as_str().unwrap_or_default();
    if !text.lines().any(|line| line.starts_with("Bestow ")) {
        return Ok(None);
    }
    let unsupported = || {
        super::unsupported(
            "abilities.bestow",
            "requires one separately printed fixed-mana bestow cost and bound prospective caster",
        )
    };
    let lines = source_lines(
        text,
        result.name.as_str(),
        source["type_line"].as_str().ok_or_else(unsupported)?,
    )?;
    let mut matching = lines
        .iter()
        .enumerate()
        .filter_map(|(i, l)| l.strip_prefix("Bestow ").map(|mana| (i, mana)));
    let Some((index, printed)) = matching.next() else {
        return Err(unsupported());
    };
    if matching.next().is_some() {
        return Err(unsupported());
    }
    let mana = mana_cost(printed)?;
    let mut symbols = Vec::new();
    if mana.generic != Quantity::zero() || mana.colored.is_empty() {
        symbols.push(mana.generic.to_string())
    }
    symbols.extend(mana.colored.iter().map(|c| format!("{c:?}")));
    if symbols
        .iter()
        .map(|s| format!("{{{s}}}"))
        .collect::<String>()
        != printed
    {
        return Err(unsupported());
    }
    let ast = json!({"kind":"keywords","keywords":[{"keyword":"bestow","cost":[{"cost":"mana","symbols":symbols}]}]});
    let blocks: Vec<Value> = decode(result.abilities.get(), "abilities")?;
    let line = result.lines.get(index).ok_or_else(unsupported)?;
    if blocks.len() != lines.len()
        || result.lines.len() != lines.len()
        || blocks.get(index) != Some(&ast)
        || !line.ok
        || line.text != lines[index]
        || decode::<Value>(line.ability.get(), "abilities.bestow")? != ast
    {
        return Err(unsupported());
    }
    let root = format!("/faces/0/abilities/{index}");
    let keyword = format!("{root}/keywords/0");
    let mut graph = Bindings::new(identity, source);
    graph.ability(&root, "keywords", "static-source-controller");
    let scope = graph.id(&root, "ability");
    let caster = graph.node(
        &keyword,
        "you",
        "runtime-player",
        json!({"scope":scope,"role":"prospective-caster","rules":["109.5","702.103"]}),
    );
    graph.node(&keyword,"keyword","keyword-ability",json!({"scope":scope,"keyword":"bestow","abilities":["static"],"you":caster,"rules":["702.103"]}));
    graph.reference(
        &format!("{keyword}/cost/0/payer"),
        &json!({"player":"you"}),
        &caster,
        "109.5",
    );
    graph.validate_block(binding, &root)?;
    Ok(Some(mana))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bestow_cost_binds_printed_mana_and_distinct_caster() {
        let record: Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/ramos/695ee4fe-356d-47fb-9442-d171b618151c.json"
        ))
        .unwrap();
        let parse: ParseResult =
            serde_json::from_value(record["faces"][0]["result"].clone()).unwrap();
        let identity = record["oracleIdentity"].as_str().unwrap();
        let source = &record["source"];
        let binding = &record["binding"];
        let found = cost(source, &parse, binding, identity).unwrap().unwrap();
        assert_eq!(found.generic, 2.into());
        assert_eq!(
            found.colored,
            [Color::W, Color::U, Color::B, Color::R, Color::G]
        );
        let mut forged = binding.clone();
        forged["references"][0]["target"] = forged["nodes"][3]["id"].clone();
        assert!(cost(source, &parse, &forged, identity).is_err());
        let mut forged = source.clone();
        forged["oracle_text"] = json!(
            source["oracle_text"]
                .as_str()
                .unwrap()
                .replacen("{2}", "{3}", 1)
        );
        assert!(cost(&forged, &parse, binding, identity).is_err());
    }
}
