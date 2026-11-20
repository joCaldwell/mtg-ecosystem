use super::*;
use serde_json::{Value, json};

pub(super) fn lower(
    source: &Value,
    result: &ParseResult,
    binding: &Value,
    identity: &str,
    kind: CardKind,
) -> Result<Option<SpellProgram>, Failure> {
    use mtg_contracts::optional_block::exile_search_spec;
    let Some((ast, program)) =
        exile_search_spec(source["oracle_text"].as_str().unwrap_or_default(), kind)
    else {
        return Ok(None);
    };
    if binding["referencesBound"] != true {
        return Ok(None);
    }
    let archived: Value = decode(result.abilities.get(), "abilities")?;
    if archived != ast {
        return Err(unsupported(
            "optional block",
            "requires exact search, entry, and shuffle clauses",
        ));
    }
    let root = "/faces/0/abilities/0";
    let exile = format!("{root}/effects/0/steps/0");
    let group = format!("{root}/effects/1/steps/0");
    let search = format!("{group}/steps/0");
    let entry = format!("{group}/steps/1");
    let shuffle = format!("{group}/steps/2");
    let mut graph = bindings::Bindings::new(identity, source);
    graph.ability(root, "spell", "spell-controller");
    let scope = graph.id(root, "ability");
    let lead = &ast[0]["effects"][0]["steps"][0];
    let target = graph.target(
        &scope,
        &format!("{exile}/what"),
        &lead["what"],
        &program.targets[0].selection_id,
    );
    graph.event(root, &exile, "exile", &target, false);
    let exiled = graph.id(&exile, "event");
    graph.node(&exile, "objects", "event-result", json!({
        "event":exiled, "result":"objects", "cardinality":"one", "availability":"if-produced",
        "identity":"zone-successors", "description":{"types":["creature"],"cls":"card"},"rules":["400.7j"]
    }));
    let rules = [
        "110.2a", "400.7", "608.2b", "608.2c", "608.2d", "608.2h", "701.23b", "701.24a",
    ];
    let body = &ast[0]["effects"][1]["steps"][0];
    let who_path = format!("{group}/who");
    let player = graph.node(&who_path, "controller-before-exile", "derived-player", json!({
        "scope":scope, "role":"controller", "of":target, "information":"current-or-last-known", "rules":rules
    }));
    for path in [
        who_path,
        format!("{search}/who"),
        format!("{entry}/who"),
        format!("{shuffle}/who"),
        format!("{shuffle}/library/owner"),
    ] {
        graph.reference_rules(&format!("{path}/of"), &json!({"ref":"it"}), &target, &rules);
        graph.reference_rules(&path, &body["who"], &player, &rules);
    }
    graph.reference(
        &format!("{search}/zones/of/0/owner"),
        &json!({"player":"they"}),
        &player,
        "608.2k",
    );
    let query_path = format!("{search}/for");
    let query = graph.node(&query_path, "query", "selection", json!({
        "scope":scope, "expression":body["steps"][0]["for"], "cardinality":"one", "timing":"resolution",
        "selection":"evaluate-reference", "rules":["608.2h"]
    }));
    graph.reference(&query_path, &body["steps"][0]["for"], &query, "608.2h");
    let decision = graph.node(
        &group,
        "event",
        "event",
        json!({
            "scope":scope, "operation":"optional-controller-search", "inputs":[player],
            "availability":"when-performed", "optional":true, "decider":player, "rules":["608.2c"]
        }),
    );
    let searched = graph.node(&search, "event", "event", json!({
        "scope":scope, "operation":"search", "inputs":[player,query], "availability":"when-performed",
        "conditional":true, "condition":decision, "rules":["608.2c"]
    }));
    let found = graph.node(&search,"objects","event-result",json!({
        "event":searched, "result":"objects", "cardinality":"one", "availability":"if-produced",
        "identity":"same-objects", "description":body["steps"][0]["for"]["filter"], "rules":["608.2h"]
    }));
    graph.reference_rules(
        &format!("{entry}/what"),
        &body["steps"][1]["what"],
        &found,
        &rules,
    );
    let moved = graph.node(&entry,"event","event",json!({
        "scope":scope, "operation":"move-zone", "inputs":[player,found], "availability":"when-performed",
        "conditional":true, "condition":decision, "requiresResult":found, "rules":["608.2c"]
    }));
    let shuffled = graph.node(&shuffle,"event","event",json!({
        "scope":scope, "operation":"shuffle", "inputs":[player], "availability":"when-performed",
        "conditional":true, "condition":decision, "rules":["608.2c"]
    }));
    graph.node(
        &group,
        "controller-search",
        "controller-search",
        json!({
            "scope":scope, "exile":exiled, "player":player, "decision":decision,
            "search":searched, "entry":moved, "shuffle":shuffled, "rules":rules
        }),
    );
    graph.validate_ordered_with_features(binding, &[json!({
        "feature":"controller-search", "path":group,
        "message":"Use the exiled target's controller or last known controller for the single optional search, entry and shuffle sequence, including searches that find nothing.",
        "rules":rules
    })], "requires the exact target-controller and optional search result graph")?;
    Ok(Some(program))
}

#[cfg(test)]
#[test]
fn slice6a_optional_search_requires_bound_chooser_and_complete_operations() {
    let raw =
        include_str!("../../../tests/fixtures/phase1-r3/d683d985-9888-4d21-8b5f-69e69ce4a03b.json");
    let header = include_str!("../../../tests/fixtures/archive-manifest-phase1-r3.json");
    let result = compile(header, raw);
    let CompileResult::Lowered { mut definition } = result else {
        panic!("{result:?}");
    };
    assert!(
        definition
            .capabilities
            .contains(&Capability::OptionalEffectBlockV1)
    );
    definition.spell.as_mut().unwrap().instructions.remove(1);
    definition.capabilities = definition.requirements();
    assert!(definition.validate().is_err());
    let wrong = super::tests::changed_source(raw, |row| {
        let refs = row["binding"]["references"].as_array_mut().unwrap();
        let target = refs[0]["target"].clone();
        refs[2]["target"] = target;
    });
    assert!(!matches!(
        compile(header, &wrong),
        CompileResult::Lowered { .. }
    ));
}
