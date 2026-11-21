use super::*;
use serde_json::{Value, json};

pub(super) fn lower(
    block: usize,
    result: &ParseResult,
    binding: &Value,
    source: &Value,
    identity: &str,
    kind: CardKind,
) -> Result<Option<Ability>, Failure> {
    let Some((ast, ability)) = mtg_contracts::optional_entry_search::printed(
        &result.lines[block].text,
        source_block(block)?,
        kind,
    ) else {
        return Ok(None);
    };
    let fail = "requires exact optional entry search, land successor, and shuffle binding";
    if ast != decode::<Value>(result.lines[block].ability.get(), "trigger.optional_search")? {
        return Err(unsupported("abilities", fail));
    }
    let root = format!("/faces/0/abilities/{block}");
    let trigger = format!("{root}/trigger");
    let group = format!("{root}/effects/0/steps/0");
    let search = format!("{group}/steps/0");
    let entry = format!("{group}/steps/1");
    let shuffle = format!("{group}/steps/2");
    let mut graph = bindings::Bindings::new(identity, source);
    graph.ability(&root, "triggered", "trigger-controller");
    let scope = graph.id(&root, "ability");
    let you = graph.id(&root, "you");
    let subject = graph.id("/source", "object");
    graph.reference(
        &format!("{trigger}/what"),
        &ast["trigger"]["what"],
        &subject,
        "201.5",
    );
    graph.event(&root, &trigger, "trigger:enters", &subject, false);
    graph.result(
        &trigger,
        "objects",
        "same-objects",
        Some(&json!({"types":["creature"]})),
    );
    let actor = json!({"player":"you"});
    for path in [
        format!("{group}/who"),
        format!("{search}/who"),
        format!("{search}/zones/of/0/owner"),
    ] {
        graph.reference(&path, &actor, &you, "109.5");
    }
    let steps = &ast["effects"][0]["steps"][0]["steps"];
    let expression = &steps[0]["for"];
    let query_path = format!("{search}/for");
    let query = graph.node(
        &query_path,
        "query",
        "selection",
        json!({
            "scope":scope,"expression":expression, "cardinality":"one","timing":"resolution",
            "selection":"evaluate-reference","rules":["608.2h"]
        }),
    );
    graph.reference(&query_path, expression, &query, "608.2h");
    graph.node(
        &search,
        "event",
        "event",
        json!({
            "scope":scope,"operation":"search","inputs":[query,you],"availability":"when-performed",
            "optional":true,"decider":you,"rules":["608.2c"]
        }),
    );
    graph.reference(&format!("{entry}/who"), &actor, &you, "109.5");
    graph.reference_rules(
        &format!("{entry}/what"),
        &steps[1]["what"],
        &query,
        &["608.2k", "400.7j", "608.2h"],
    );
    graph.event(&root, &entry, "move-zone", &query, true);
    graph.node(&entry, "objects", "event-result", json!({
        "event":graph.id(&entry,"event"),"result":"objects","cardinality":"one",
        "availability":"if-produced","identity":"zone-successors",
        "description":{"supertypes":["basic"],"types":["land"],"cls":"permanent"},"rules":["400.7j"]
    }));
    graph.reference(&format!("{shuffle}/who"), &actor, &you, "109.5");
    graph.reference(&format!("{shuffle}/library/owner"), &actor, &you, "109.5");
    graph.event(&root, &shuffle, "shuffle", &you, true);
    if let Some(sibling) = result.lines.get(block + 1) {
        let raw: Value = decode(sibling.ability.get(), "trigger.optional_search.sibling")?;
        let draw = mtg_contracts::optional_entry_search::draw_sibling(
            &sibling.text,
            &raw,
            source_block(block + 1)?,
        )
        .ok_or_else(|| unsupported("abilities", fail))?;
        let root = format!("/faces/0/abilities/{}", block + 1);
        let dies = format!("{root}/trigger");
        let step = format!("{root}/effects/0/steps/0");
        graph.ability(&root, "triggered", "trigger-controller");
        let you = graph.id(&root, "you");
        graph.reference(
            &format!("{dies}/what"),
            &raw["trigger"]["what"],
            &subject,
            "201.5",
        );
        graph.event(&root, &dies, "trigger:dies", &subject, false);
        graph.result(
            &dies,
            "objects",
            "same-objects",
            Some(&json!({"types":["creature"]})),
        );
        graph.node(
            &dies,
            "successor",
            "event-result",
            json!({
                "event":graph.id(&dies,"event"),"result":"objects","cardinality":"one",
                "availability":"if-produced","identity":"zone-successors",
                "description":{"types":["creature"],"cls":"card"},"rules":["400.7e","603.6c"]
            }),
        );
        graph.reference(&format!("{step}/who"), &actor, &you, "109.5");
        graph.event(
            &root,
            &step,
            "draw",
            &you,
            matches!(
                draw.program.instructions.as_slice(),
                [Instruction::OptionalDrawCards { .. }]
            ),
        );
        graph.result(
            &step,
            "objects",
            "new-objects",
            Some(&json!({"cls":"card"})),
        );
        graph.result(&step, "quantity", "measured-quantity", None);
    }
    graph.validate_ordered(binding, fail)?;
    Ok(Some(Ability::BoundTrigger(ability)))
}

#[cfg(test)]
#[test]
fn slice6a_optional_entry_search_preserves_its_independent_death_trigger() {
    let header = include_str!("../../../tests/fixtures/archive-manifest-phase1-r3.json");
    let row =
        include_str!("../../../tests/fixtures/phase1-r3/00c0543c-2a1f-4425-8283-4062d74a1637.json");
    let result = compile(header, row);
    let CompileResult::Lowered { mut definition } = result else {
        panic!("{result:?}");
    };
    let [entry, dies] = definition.triggered.as_slice() else {
        panic!("both printed triggers must lower");
    };
    assert!(matches!(
        entry.program.instructions.as_slice(),
        [
            Instruction::ChooseOptional {
                chooser: EffectRecipient::Controller,
                following: 1,
                ..
            },
            Instruction::SearchRecipientBasicLand {
                recipient: EffectRecipient::Controller,
                ..
            }
        ]
    ));
    assert_eq!(dies.condition, TriggerCondition::SelfDies);
    assert!(matches!(
        dies.program.instructions.as_slice(),
        [Instruction::OptionalDrawCards { .. }]
    ));
    definition.triggered.pop();
    definition.source_labels.as_mut().unwrap().triggered.pop();
    definition.capabilities = definition.requirements();
    assert!(definition.validate().is_err());
    let wrong = crate::tests::changed_source(row, |record| {
        let refs = record["binding"]["references"].as_array_mut().unwrap();
        let chooser = refs[1]["target"].clone();
        refs.last_mut().unwrap()["target"] = chooser;
    });
    assert!(!matches!(
        compile(header, &wrong),
        CompileResult::Lowered { .. }
    ));
}
