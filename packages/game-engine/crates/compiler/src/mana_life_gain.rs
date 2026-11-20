//! Exact source and activator binding for off-stack mana and a subsequent life effect.
//! CR 109.5, 605.1a, 605.3b and 608.2c (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(super) fn lower(
    source: &Source,
    shape: &Value,
    result: &ParseResult,
    binding: &Value,
) -> Result<Option<ManaAbility>, Failure> {
    let Some((ability, ast)) = mana_life_gain_source(&source.oracle_text) else {
        return Ok(None);
    };
    if source.type_line != "Artifact"
        || decode::<Value>(result.abilities.get(), "abilities")? != ast
    {
        return Err(unsupported(
            "mana",
            "requires the complete printed tap mana and life gain program",
        ));
    }
    let mut graph = bindings::Bindings::new(&source.oracle_id, shape);
    let root = "/faces/0/abilities/0";
    listed_mana::bind_tap_mana(&mut graph, root);
    let step = format!("{root}/effects/1/steps/0");
    let activator = graph.id(root, "you");
    graph.reference(
        &format!("{step}/who"),
        &json!({"player":"you"}),
        &activator,
        "109.5",
    );
    graph.event(root, &step, "gain-life", &activator, false);
    graph.result(&step, "quantity", "measured-quantity", None);
    graph.validate(binding, "requires bound mana and life recipients")?;
    Ok(Some(ability))
}

#[test]
fn mana_life_gain_requires_exact_effect_and_binding() {
    let manifest = include_str!("../../../tests/fixtures/archive-manifest-current.json");
    let source = include_str!("../../../tests/fixtures/1b3d7fce-e9fe-4176-9d9a-472415826cdd.json");
    let result = compile(manifest, source);
    let CompileResult::Lowered { definition } = result else {
        panic!("{result:?}")
    };
    assert!(matches!(&definition.mana_abilities[0].production,
        ManaProduction::FixedWithLifeGain { symbols, life } if symbols == &[Color::C] && *life == 1.into()));
    let bad = crate::tests::changed_source(source, |row| {
        let nodes = row["binding"]["nodes"].as_array_mut().unwrap();
        nodes
            .iter_mut()
            .find(|node| node["operation"] == "gain-life")
            .unwrap()["inputs"] = json!([]);
    });
    assert!(
        matches!(compile(manifest, &bad), CompileResult::Unsupported { path, .. } if path == "binding")
    );
}
