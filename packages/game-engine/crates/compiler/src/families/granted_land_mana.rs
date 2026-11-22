use super::*;
use crate::bindings::Bindings;
use mtg_contracts::{granted_land_mana_printed, granted_land_mana_source};
use serde_json::{Value, json};

pub(crate) fn lower(
    source: &Source,
    shape: &Value,
    result: &ParseResult,
    binding: &Value,
) -> Result<Option<ManaAbility>, Failure> {
    let Some(expected) = granted_land_mana_source(&source.oracle_text) else {
        return Ok(None);
    };
    if source.type_line != "Artifact"
        || source
            .oracle_text
            .lines()
            .ne(result.lines.iter().map(|line| line.text.as_str()))
        || expected != decode::<[Value; 2]>(result.abilities.get(), "abilities")?
    {
        return Err(unsupported(
            "abilities",
            "requires a complete granted and printed tap mana pair",
        ));
    }
    let base = "/faces/0/abilities/0";
    let selection = format!("{base}/effect/what");
    let recipient_path = format!("{base}/effect/modifiers/0/abilities/0");
    let ability_path = format!("{recipient_path}/ability");
    let player = json!({"player":"you"});
    let what = &expected[0]["effect"]["what"];
    let mut graph = Bindings::new(&source.oracle_id, shape);
    graph.ability(base, "static", "static-source-controller");
    let scope = graph.id(base, "ability");
    graph.reference(
        &format!("{selection}/filter/control"),
        &player,
        &graph.id(base, "you"),
        "109.5",
    );
    let query = graph.node(
        &selection,
        "query",
        "selection",
        json!({
            "scope":scope,"expression":what,"cardinality":"many","timing":"continuous",
            "selection":"evaluate-reference","rules":["608.2h"]
        }),
    );
    graph.reference(&selection, what, &query, "608.2h");
    let recipient = graph.node(
        &recipient_path,
        "recipient",
        "recipient",
        json!({
            "scope":scope,"of":query,"each":true,"rules":["113.10","201.5a"]
        }),
    );
    let activator = graph.id(&ability_path, "you");
    let scope = graph.node(
        &ability_path,
        "ability",
        "ability",
        json!({
            "source":recipient,"face":graph.id("/faces/0","face"),"abilityKind":"activated",
            "you":activator,"rules":["113","109.5"]
        }),
    );
    graph.node(
        &ability_path,
        "you",
        "runtime-player",
        json!({
            "scope":scope,"role":"activator","rules":["109.5"]
        }),
    );
    let printed = "/faces/0/abilities/1";
    graph.ability(printed, "activated", "activator");
    for (path, origin) in [
        (&*ability_path, recipient),
        (printed, graph.id("/source", "object")),
    ] {
        let activator = graph.id(path, "you");
        graph.reference(
            &format!("{path}/costs/0/source"),
            &json!({"ref":"self"}),
            &origin,
            "201.5",
        );
        graph.reference(
            &format!("{path}/costs/0/payer"),
            &player,
            &activator,
            "109.5",
        );
        let step = format!("{path}/effects/0/steps/0");
        graph.reference(&format!("{step}/who"), &player, &activator, "109.5");
        graph.event(path, &step, "add-mana", &activator, false);
    }
    graph.validate(
        binding,
        "granted tap mana must bind to its receiving land and activator",
    )?;
    Ok(Some(granted_land_mana_printed()))
}

#[cfg(test)]
#[test]
fn granted_land_mana_binds_both_sources() {
    let manifest = include_str!("../../../../tests/fixtures/ramos/archive-manifest-current.json");
    let record =
        include_str!("../../../../tests/fixtures/ramos/539f5396-d99a-417d-a84c-dff7930b5900.json");
    let result = compile(manifest, record);
    let CompileResult::Lowered { definition } = result else {
        panic!("{result:?}");
    };
    assert_eq!(definition.mana_abilities, [granted_land_mana_printed()]);
    let forged = crate::tests::changed_source(record, |r| {
        r["binding"]["references"][2]["target"] = format!(
            "card:{}/source#object",
            r["oracleIdentity"].as_str().unwrap()
        )
        .into();
    });
    assert!(
        matches!(compile(manifest, &forged), CompileResult::Unsupported { path, .. } if path == "binding")
    );
}
