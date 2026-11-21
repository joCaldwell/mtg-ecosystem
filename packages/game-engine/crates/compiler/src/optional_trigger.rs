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
    let line = &result.lines[block];
    if !mtg_contracts::optional_trigger::candidate(&line.text, kind) {
        return Ok(None);
    }
    let raw: Value = decode(line.ability.get(), "trigger.optional")?;
    let fail = "requires the complete optional life trigger and controller binding";
    let ability =
        mtg_contracts::optional_trigger::printed(&line.text, &raw, kind, source_block(block)?)
            .ok_or_else(|| unsupported("trigger.optional", fail))?;
    let root = format!("/faces/0/abilities/{block}");
    let trigger = format!("{root}/trigger");
    let step = format!("{root}/effects/0/steps/0");
    let mut graph = bindings::Bindings::new(identity, source);
    graph.ability(&root, "triggered", "trigger-controller");
    let scope = graph.id(&root, "ability");
    let you = graph.id(&root, "you");
    let who = json!({"player":"you"});
    if ability.condition == TriggerCondition::OtherCreatureEntersBattlefield {
        let what = &raw["trigger"]["what"];
        let path = format!("{trigger}/what");
        let query = graph.node(
            &path,
            "query",
            "selection",
            json!({
                "scope":scope, "expression":what, "cardinality":"one", "timing":"trigger-match",
                "selection":"evaluate-reference", "rules":["608.2h"]
            }),
        );
        graph.reference(
            &format!("{path}/filter/other"),
            &json!({"ref":"self"}),
            &graph.id("/source", "object"),
            "201.5",
        );
        graph.reference(&path, what, &query, "608.2h");
        graph.event(&root, &trigger, "trigger:enters", &query, false);
        graph.result(&trigger, "objects", "same-objects", Some(&what["filter"]));
    } else {
        graph.reference(&format!("{trigger}/whose"), &who, &you, "109.5");
        graph.node(
            &trigger,
            "event",
            "event",
            json!({"scope":scope,"operation":"trigger:phase",
            "inputs":[],"availability":"when-performed","rules":["608.2c"]}),
        );
    }
    graph.reference(&format!("{step}/who"), &who, &you, "109.5");
    graph.event(&root, &step, "gain-life", &you, true);
    graph.result(&step, "quantity", "measured-quantity", None);
    graph.validate_block(binding, &root, fail)?;
    Ok(Some(Ability::BoundTrigger(ability)))
}

#[cfg(test)]
#[test]
fn slice6a_optional_trigger_lowers_same_choice_for_two_trigger_conditions() {
    let header = include_str!("../../../tests/fixtures/archive-manifest-phase1-r3.json");
    for (record, condition) in [
        (
            include_str!(
                "../../../tests/fixtures/phase1-r3/045a9d3d-c20d-427f-a77b-0bffc6f55526.json"
            ),
            TriggerCondition::OtherCreatureEntersBattlefield,
        ),
        (
            include_str!(
                "../../../tests/fixtures/phase1-r3/2f47661e-a107-4d98-8773-0a068a63df49.json"
            ),
            TriggerCondition::StepBegan {
                step: BeginningStep::Upkeep,
                turn_relation: TurnRelation::Controller,
            },
        ),
    ] {
        let result = compile(header, record);
        let CompileResult::Lowered { mut definition } = result else {
            panic!("{result:?}");
        };
        let trigger = &mut definition.triggered[0];
        assert_eq!(trigger.condition, condition);
        assert!(matches!(trigger.program.instructions.as_slice(),
            [Instruction::ChooseOptional { chooser: EffectRecipient::Controller, following: 1, .. },
                kernel!(GainLife { player: you!(), amount: fixed!(amount) })]
                if *amount == Quantity::from(1)));
        trigger.program.instructions.remove(0);
        definition.capabilities = definition.requirements();
        assert!(definition.validate().is_err());
        let wrong = crate::tests::changed_source(record, |row| {
            let steps = &mut row["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"];
            steps[0]["optional"] = json!(false);
        });
        assert!(!matches!(
            compile(header, &wrong),
            CompileResult::Lowered { .. }
        ));
    }
}
