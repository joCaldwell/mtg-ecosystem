//! CR 106.7, 107.4e, 601.2b/h, 602.2b, 605.3b and 733.1 (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(super) fn lower(
    abilities: &[Ability],
    result: &ParseResult,
    source: &Value,
    binding: &Value,
    card: &Source,
) -> Result<Option<[ManaAbility; 2]>, Failure> {
    if !abilities.iter().any(|ability| {
        matches!(ability, Ability::Activated { costs, effects, .. }
            if costs.iter().any(|cost| matches!(cost, SourceCost::Mana { symbols }
                if symbols.iter().any(|s| s.contains('/'))))
            && effects.iter().flat_map(|sentence| &sentence.steps).any(|step|
                matches!(step, Step::AddMana { mana: SourceMana::Choice { .. }, .. })))
    }) {
        return Ok(None);
    }
    let bad = || {
        unsupported(
            "mana.filter",
            "requires a complete two-color hybrid filter mana source",
        )
    };
    let raw: Value = decode(result.abilities.get(), "abilities")?;
    let symbol = raw[1]["costs"][0]["symbols"][0].as_str().ok_or_else(bad)?;
    let (first, second) = symbol.split_once('/').ok_or_else(bad)?;
    let color = |s: &str| serde_json::from_value::<Color>(json!(s)).map_err(|_| bad());
    let colors = [color(first)?, color(second)?];
    let (printed, expected) = hybrid_filter_source(colors).ok_or_else(bad)?;
    if card.type_line != "Land"
        || !card.mana_cost.is_empty()
        || card.oracle_text != printed
        || raw != expected
    {
        return Err(bad());
    }
    let mut graph = bindings::Bindings::new(&card.oracle_id, source);
    listed_mana::bind_tap_mana(&mut graph, "/faces/0/abilities/0");
    bind_paid_tap_mana(&mut graph, "/faces/0/abilities/1");
    graph.validate(
        binding,
        "requires complete hybrid payment and mana-output binding",
    )?;
    Ok(Some(hybrid_filter_abilities(colors)))
}

pub(super) fn bind_paid_tap_mana(graph: &mut bindings::Bindings, root: &str) {
    let mana = format!("{root}/effects/0/steps/0");
    let you = graph.id(root, "you");
    graph.ability(root, "activated", "activator");
    graph.event(root, &mana, "add-mana", &you, false);
    graph.reference(
        &format!("{root}/costs/0/payer"),
        &json!({"player":"you"}),
        &you,
        "109.5",
    );
    graph.reference(
        &format!("{root}/costs/1/source"),
        &json!({"ref":"self"}),
        &graph.id("/source", "object"),
        "201.5",
    );
    graph.reference(
        &format!("{root}/costs/1/payer"),
        &json!({"player":"you"}),
        &you,
        "109.5",
    );
    graph.reference(
        &format!("{mana}/who"),
        &json!({"player":"you"}),
        &you,
        "109.5",
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hybrid_filter_mana_compiles_the_full_source_and_rejects_wrong_payer() {
        let source =
            include_str!("../../../tests/fixtures/f1603384-4361-49c9-98aa-7785fc3504c4.json");
        let manifest = include_str!("../../../tests/fixtures/archive-manifest-current.json");
        let result = compile(manifest, source);
        let CompileResult::Lowered { definition } = result else {
            panic!("{result:?}")
        };
        assert_eq!(
            definition.mana_abilities,
            hybrid_filter_abilities([Color::U, Color::R])
        );
        let wrong = crate::tests::changed_source(source, |record| {
            let references = record["binding"]["references"].as_array_mut().unwrap();
            let payer = references
                .iter_mut()
                .find(|reference| reference["path"] == "/faces/0/abilities/1/costs/0/payer")
                .unwrap();
            let target = payer["target"]
                .as_str()
                .unwrap()
                .replace("abilities/1", "abilities/0");
            payer["target"] = json!(target);
        });
        assert!(
            matches!(compile(manifest, &wrong), CompileResult::Unsupported { path, .. } if path == "binding")
        );
    }
}
