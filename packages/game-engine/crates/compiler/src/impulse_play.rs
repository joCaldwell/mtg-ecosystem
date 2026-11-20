use super::*;
use serde_json::{Value, json};

pub(super) fn lower(
    source: &Value,
    result: &ParseResult,
    binding: &Value,
    identity: &str,
    kind: CardKind,
) -> Result<Option<SpellProgram>, Failure> {
    let text = source["oracle_text"].as_str().unwrap_or_default();
    if !mtg_contracts::impulse_play::candidate(text) {
        return Ok(None);
    }
    let ast: Value = decode(result.abilities.get(), "abilities")?;
    let Some(program) = mtg_contracts::impulse_play::printed(text, &ast, kind) else {
        return Ok(None);
    };
    let mut graph = bindings::Bindings::new(identity, source);
    let root = "/faces/0/abilities/0";
    graph.ability(root, "spell", "spell-controller");
    bind_block(&mut graph, root, &ast[0]);
    graph.validate(
        binding,
        "requires the exact same-issuer exile successors and play permission",
    )?;
    Ok(Some(program))
}

pub(super) fn bind_block(graph: &mut bindings::Bindings<'_>, root: &str, ability: &Value) {
    let exile = format!("{root}/effects/0/steps/0");
    let grant = format!("{root}/effects/1/steps/0");
    let what = format!("{exile}/what");
    let scope = graph.id(root, "ability");
    let you = graph.id(root, "you");
    let player = json!({"player":"you"});
    let query = &ability["effects"][0]["steps"][0]["what"];
    let selected = graph.node(
        &what,
        "query",
        "selection",
        json!({
            "scope":scope,"expression":query,"cardinality":"many","timing":"resolution",
            "selection":"evaluate-reference","rules":["608.2h"]
        }),
    );
    graph.reference(&format!("{what}/of/owner"), &player, &you, "109.5");
    graph.reference(&what, query, &selected, "608.2h");
    graph.event(root, &exile, "exile", &selected, false);
    let objects = graph.node(
        &exile,
        "objects",
        "event-result",
        json!({
            "event":graph.id(&exile,"event"),"result":"objects","cardinality":"many",
            "availability":"if-produced","identity":"zone-successors","description":{"cls":"card"},
            "rules":["400.7j"]
        }),
    );
    graph.result(&exile, "quantity", "measured-quantity", None);
    graph.node(
        &grant,
        "event",
        "event",
        json!({
            "scope":scope,"operation":"grant-play-permission","inputs":[objects,you],
            "availability":"when-performed","rules":["608.2c"]
        }),
    );
    let permission = &ability["effects"][1]["steps"][0];
    graph.reference_rules(
        &format!("{grant}/permission/what"),
        &permission["permission"]["what"],
        &objects,
        &["608.2k", "400.7j", "608.2h"],
    );
    graph.reference(&format!("{grant}/permission/who"), &player, &you, "109.5");
    if permission["duration"]["duration"] == "end-of-your-next-turn" {
        graph.reference(&format!("{grant}/duration/whose"), &player, &you, "109.5");
    }
}

#[cfg(test)]
#[test]
fn composition_wave_d_checks_the_permission_and_exact_successor_binding() {
    let header = include_str!("../../../tests/fixtures/archive-manifest-current.json");
    for (raw, count, window) in [
        (
            include_str!("../../../tests/fixtures/584cf0fd-112a-4ca6-9c0e-1f3228a7d325.json"),
            2,
            PlayWindow::ThroughNextControllerTurn,
        ),
        (
            include_str!("../../../tests/fixtures/a7d8c99c-cd44-4bad-82e6-7c7cff9bd89f.json"),
            2,
            PlayWindow::ThroughNextControllerTurn,
        ),
        (
            include_str!("../../../tests/fixtures/19ad338a-c89f-406d-ad1a-222c22044e1b.json"),
            3,
            PlayWindow::ThisTurn,
        ),
    ] {
        let compiled = compile(header, raw);
        let CompileResult::Lowered { definition } = compiled else {
            panic!("{compiled:?}")
        };
        assert_eq!(
            definition.spell.unwrap().instructions,
            [Instruction::ExileLibraryForPlay {
                count,
                window,
                source_block: 0
            }]
        );
        let wrong_objects = super::tests::changed_source(raw, |row| {
            let references = row["binding"]["references"].as_array_mut().unwrap();
            let query = references[1]["target"].clone();
            references[2]["target"] = query;
        });
        assert!(!matches!(
            compile(header, &wrong_objects),
            CompileResult::Lowered { .. }
        ));
        let wrong_duration = super::tests::changed_source(raw, |row| {
            row["faces"][0]["result"]["abilities"][0]["effects"][1]["steps"][0]["duration"]["duration"] =
                "end-of-game".into();
        });
        assert!(!matches!(
            compile(header, &wrong_duration),
            CompileResult::Lowered { .. }
        ));
    }
}
