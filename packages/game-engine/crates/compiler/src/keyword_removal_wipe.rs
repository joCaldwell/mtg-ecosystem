use super::*;
use crate::bindings::Bindings;
use mtg_contracts::{keyword_removal_wipe_program, keyword_removal_wipe_source};
use serde_json::{Value, json};

pub(super) fn lower(
    source: &Source,
    shape: &Value,
    result: &ParseResult,
    binding: &Value,
) -> Result<Option<SpellProgram>, Failure> {
    let Some(expected) = keyword_removal_wipe_source(&source.oracle_text) else {
        return Ok(None);
    };
    let fail = || unsupported("abilities", "incomplete keyword removal and wipe");
    if source.type_line != "Sorcery"
        || source
            .oracle_text
            .lines()
            .ne(result.lines.iter().map(|line| line.text.as_str()))
        || expected != decode::<Vec<Value>>(result.abilities.get(), "abilities")?
    {
        return Err(fail());
    }
    let mut graph = Bindings::new(&source.oracle_id, shape);
    for (index, ability) in expected.iter().enumerate() {
        let base = format!("/faces/0/abilities/{index}");
        let step = format!("{base}/effects/0/steps/0");
        let path = format!("{step}/what");
        let scope = graph.id(&base, "ability");
        let what = &ability["effects"][0]["steps"][0]["what"];
        graph.ability(&base, "spell", "spell-controller");
        if index == 0 {
            let control = format!("{path}/filter/control");
            let opponents = graph.node(&control, "opponents", "derived-player",
                json!({"scope":scope,"role":"opponents","of":graph.id(&base,"you"),"rules":["102.2","109.5"]}));
            graph.reference(
                &control,
                &json!({"implicit":"opponents"}),
                &opponents,
                "102.2",
            );
        }
        let query = graph.node(
            &path,
            "query",
            "selection",
            json!({
                "scope": scope, "expression":what, "cardinality":"many","timing":"resolution",
                "selection":"evaluate-reference","rules":["608.2h"]
            }),
        );
        graph.reference(&path, what, &query, "608.2h");
        if index == 0 {
            let ability = format!("{step}/removal/abilities/0");
            let keyword = format!("{ability}/keyword");
            let recipient = graph.node(
                &ability,
                "recipient",
                "recipient",
                json!({
                    "scope":scope,"of":query,"each":true,"rules":["113.10","201.5a"]
                }),
            );
            let child = graph.node(
                &ability,
                "ability",
                "ability",
                json!({
                    "source":recipient,"face":graph.id("/faces/0","face"),
                    "abilityKind":"keywords","you":graph.id(&ability,"you"),"rules":["113","113.10"]
                }),
            );
            graph.node(&ability, "you", "runtime-player", json!({
                "scope":child,"role":"static-source-controller","rules":["109.5","301.5d","303.4e"]
            }));
            let you = graph.node(
                &keyword,
                "you",
                "runtime-player",
                json!({
                    "scope":child,"role":"static-source-controller","rules":["109.5","702.12"]
                }),
            );
            graph.node(&keyword, "keyword", "keyword-ability", json!({
                "scope":child,"keyword":"indestructible","abilities":["static"],"you":you,"rules":["702.12"]
            }));
            graph.event(&base, &step, "lose-abilities", &query, false);
        } else {
            graph.event(&base, &step, "destroy", &query, false);
            graph.result(&step, "quantity", "measured-quantity", None);
        }
    }
    graph.validate(
        binding,
        "keyword removal requires exact effect and controller bindings",
    )?;
    Ok(Some(keyword_removal_wipe_program()))
}

#[cfg(test)]
#[test]
fn keyword_removal_wipe_checks_complete_binding_and_definition() {
    let manifest = include_str!("../../../tests/fixtures/ramos/archive-manifest-current.json");
    let record =
        include_str!("../../../tests/fixtures/ramos/8d90ffb5-63da-48d5-a5a2-7b61062556f6.json");
    let result = compile(manifest, record);
    let CompileResult::Lowered { definition } = result else {
        panic!("{result:?}");
    };
    assert_eq!(
        definition.spell.as_ref(),
        Some(&keyword_removal_wipe_program())
    );
    let mut omitted = definition.clone();
    omitted.spell.as_mut().unwrap().instructions.remove(0);
    omitted.capabilities = omitted.requirements();
    assert!(omitted.validate().is_err());
    let mut cost_mismatch = definition.clone();
    cost_mismatch
        .characteristics
        .mana_cost
        .generic
        .add(&1.into());
    assert!(cost_mismatch.validate().is_err());
    let mut stripped = omitted;
    stripped.spell.as_mut().unwrap().instructions.clear();
    stripped.capabilities = stripped.requirements();
    stripped.source_labels = None;
    stripped.provenance.source_json = stripped
        .provenance
        .source_json
        .replace("indestructible", r"\u0069ndestructible");
    stripped.provenance.source_sha256 = sha256(stripped.provenance.source_json.as_bytes());
    assert!(stripped.validate().is_err());

    let forged = crate::tests::changed_source(record, |r| {
        r["binding"]["nodes"][5]["timing"] = json!("stack-announcement");
    });
    assert!(
        matches!(compile(manifest, &forged), CompileResult::Unsupported { path, .. } if path == "binding")
    );
}
