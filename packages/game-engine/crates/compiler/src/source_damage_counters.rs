use super::*;
use serde_json::{Value, json};

pub(super) fn lower(
    block: usize,
    result: &ParseResult,
    binding: &Value,
    source: &Value,
    identity: &str,
) -> Result<Option<Ability>, Failure> {
    let line = &result.lines[block];
    if !mtg_contracts::source_damage_counters::candidate(&line.text) {
        return Ok(None);
    }
    let raw: Value = decode(line.ability.get(), "trigger.source_damage_counters")?;
    let fail = "requires exact combat damage and source counter recipient";
    let ability =
        mtg_contracts::source_damage_counters::printed(&line.text, &raw, source_block(block)?)
            .ok_or_else(|| unsupported("trigger.source_damage_counters", fail))?;
    let root = format!("/faces/0/abilities/{block}");
    let trigger = format!("{root}/trigger");
    let step = format!("{root}/effects/0/steps/0");
    let mut graph = bindings::Bindings::new(identity, source);
    graph.ability(&root, "triggered", "trigger-controller");
    let physical_source = graph.id("/source", "object");
    graph.reference(
        &format!("{trigger}/what"),
        &raw["trigger"]["what"],
        &physical_source,
        "201.5",
    );
    graph.event(
        &root,
        &trigger,
        "trigger:deals-damage",
        &physical_source,
        false,
    );
    graph.result(
        &trigger,
        "objects",
        "same-objects",
        Some(&json!({"types":["creature"]})),
    );
    graph.result(&trigger, "player", "same-objects", None);
    graph.result(&trigger, "quantity", "measured-quantity", None);
    graph.reference(
        &format!("{step}/who"),
        &json!({"player":"you"}),
        &graph.id(&root, "you"),
        "109.5",
    );
    let dealt_by = graph.id(&trigger, "objects");
    graph.reference_rules(
        &format!("{step}/on"),
        &raw["effects"][0]["steps"][0]["on"],
        &dealt_by,
        &["608.2k", "400.7j", "608.2h"],
    );
    graph.event(&root, &step, "put-counters", &dealt_by, false);
    graph.result(&step, "quantity", "measured-quantity", None);
    graph.validate_block(binding, &root, fail)?;
    Ok(Some(Ability::BoundTrigger(ability)))
}

#[cfg(test)]
#[test]
fn slice6d_source_damage_counter_graph_and_intrinsic_siblings_are_exact() {
    let header = include_str!("../../../tests/fixtures/archive-manifest-phase1-r4.json");
    let record =
        include_str!("../../../tests/fixtures/phase1-r4/f819b134-24cd-4863-a46a-0a068e34ca9e.json");
    let result = compile(header, record);
    let CompileResult::Lowered { mut definition } = result else {
        panic!("{result:?}");
    };
    assert_eq!(
        definition.keywords,
        [KeywordAbility::Flying, KeywordAbility::Haste]
    );
    assert!(
        matches!(definition.triggered[0].program.instructions.as_slice(),
        [Instruction::PutPlusOneCounters { on: CounterRecipient::Source, amount, source_block: 2 }]
        if *amount == Quantity::from(2))
    );
    definition.keywords.remove(1);
    definition.capabilities = definition.requirements();
    assert!(definition.validate().is_err());
    let wrong = crate::tests::changed_source(record, |row| {
        let refs = row["binding"]["references"].as_array_mut().unwrap();
        let source = refs[0]["target"].clone();
        let recipient = refs
            .iter_mut()
            .find(|r| r["path"].as_str().unwrap().ends_with("/on"))
            .unwrap();
        recipient["target"] = source;
    });
    assert!(!matches!(
        compile(header, &wrong),
        CompileResult::Lowered { .. }
    ));
}
