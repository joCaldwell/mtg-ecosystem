//! CR 106.6 and 614.12: exact split mana production and entry scope.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_contracts::paid_restricted_land as family;
use serde_json::Value;

pub(super) fn lower(
    source: &Source,
    shape: &Value,
    result: &ParseResult,
    binding: &Value,
) -> Result<Option<[ManaAbility; 2]>, Failure> {
    let Some(ast) = family::source(&source.oracle_text) else {
        return Ok(None);
    };
    if source.type_line != "Land" || decode::<Value>(result.abilities.get(), "abilities")? != ast {
        return Err(unsupported(
            "mana",
            "requires exact separate printed paid mana and entry",
        ));
    }
    let mut graph = bindings::Bindings::new(&source.oracle_id, shape);
    checkland_entry::bind_entry(&mut graph, family::query());
    listed_mana::bind_tap_mana(&mut graph, "/faces/0/abilities/1");
    hybrid_filter_mana::bind_paid_tap_mana(&mut graph, "/faces/0/abilities/2");
    graph.validate(binding, "incomplete paid restricted land reference graph")?;
    Ok(Some(family::abilities()))
}

#[test]
fn paid_restricted_land_keeps_both_abilities_and_forested_entry() {
    let manifest = include_str!("../../../tests/fixtures/archive-manifest-current.json");
    let source = include_str!("../../../tests/fixtures/de75e5dd-8a52-406c-b55c-96d686885500.json");
    let CompileResult::Lowered { definition } = compile(manifest, source) else {
        panic!("{:?}", compile(manifest, source));
    };
    assert_eq!(definition.mana_abilities, family::abilities());
    assert_eq!(definition.static_abilities, [family::entry()]);
    let mut incomplete = (*definition).clone();
    incomplete.mana_abilities.pop();
    incomplete.capabilities = incomplete.requirements();
    assert!(incomplete.validate().is_err());
}
