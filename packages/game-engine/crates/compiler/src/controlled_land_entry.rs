//! CR 109.5, 603.2c, 603.3a/b and 603.6a (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(super) fn lower(
    trigger: &SourceTrigger,
    effects: &[Sentence],
    intervening: Option<&SourceCondition>,
    block: usize,
) -> Result<Option<TriggeredAbility>, Failure> {
    let SourceTrigger::Enters {
        what: TriggerObject::Filter { filter, count },
    } = trigger
    else {
        return Ok(None);
    };
    if filter.types != ["land"] {
        return Ok(None);
    }
    let bad = || {
        unsupported(
            "abilities.trigger",
            "requires the complete controller land entry trigger and effect",
        )
    };
    if block != 0
        || intervening.is_some()
        || filter.control.as_deref() != Some("you")
        || filter.other
        || filter.non_token
        || !filter.non_subtypes.is_empty()
        || filter.properties.is_some()
        || !fixed_one(count)
    {
        return Err(bad());
    }
    let [
        Sentence {
            condition: None,
            condition_kind: None,
            bindings,
            steps,
        },
    ] = effects
    else {
        return Err(bad());
    };
    let [
        Step::GainLife {
            who,
            amount,
            for_each: None,
        },
        rest @ ..,
    ] = steps.as_slice()
    else {
        return Err(bad());
    };
    let you = |p: &PlayerSelector| p.player == "you" && p.of.is_none() && p.selection_id.is_none();
    if !bindings.is_empty() || !you(who) {
        return Err(bad());
    }
    let draw = match rest {
        [] => false,
        [
            Step::Draw {
                who,
                amount,
                optional: false,
            },
        ] if you(who) && amount.fixed_one() => true,
        _ => return Err(bad()),
    };
    let amount = serde_json::from_value(amount.clone()).map_err(|_| bad())?;
    Ok(Some(controlled_land_entry_trigger(
        fixed_positive(&amount)?,
        draw,
    )))
}

pub(super) fn bind(d: &Definition, source: &Value, binding: &Value) -> Result<(), Failure> {
    if !d.capabilities.contains(&Capability::ControlledLandEntryV1) {
        return Ok(());
    }
    let mut graph = bindings::Bindings::new(&d.oracle_id, source);
    let root = "/faces/0/abilities/0";
    let trigger = format!("{root}/trigger");
    let subject = format!("{trigger}/what");
    let scope = graph.id(root, "ability");
    let you = graph.id(root, "you");
    let expr = controlled_land_query();
    graph.ability(root, "triggered", "trigger-controller");
    let query = graph.node(
        &subject,
        "query",
        "selection",
        json!({
            "scope":scope,"expression":expr,"cardinality":"one","timing":"trigger-match",
            "selection":"evaluate-reference","rules":["608.2h"]
        }),
    );
    graph.reference(
        &format!("{subject}/filter/control"),
        &json!({"player":"you"}),
        &you,
        "109.5",
    );
    graph.reference(&subject, &expr, &query, "608.2h");
    graph.event(root, &trigger, "trigger:enters", &query, false);
    graph.result(&trigger, "objects", "same-objects", Some(&expr["filter"]));
    for (index, instruction) in d.triggered[0].program.instructions.iter().enumerate() {
        let step = format!("{root}/effects/0/steps/{index}");
        let operation = match instruction {
            Instruction::DrawCards { .. } => "draw",
            Instruction::Proliferate { .. } => "keyword-action",
            _ => "gain-life",
        };
        graph.reference(
            &format!("{step}/who"),
            &json!({"player":"you"}),
            &you,
            "109.5",
        );
        graph.event(root, &step, operation, &you, false);
        if operation == "draw" {
            graph.result(
                &step,
                "objects",
                "new-objects",
                Some(&json!({"cls":"card"})),
            );
        }
        if operation != "keyword-action" {
            graph.result(&step, "quantity", "measured-quantity", None);
        }
    }
    graph.validate(
        binding,
        "requires a complete controller land entry trigger binding",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    const MANIFEST: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
    #[test]
    fn controlled_land_entry_lowers_two_exact_programs_and_rejects_wrong_binding() {
        for (source, draw) in [
            (
                include_str!("../../../tests/fixtures/263b526c-8b81-44ee-a7c4-5c24bf9b42a4.json"),
                false,
            ),
            (
                include_str!("../../../tests/fixtures/0715e860-3b3b-4331-9718-207973e94fee.json"),
                true,
            ),
        ] {
            let result = compile(MANIFEST, source);
            let CompileResult::Lowered { definition } = result else {
                panic!("{result:?}")
            };
            assert_eq!(
                definition.triggered,
                [controlled_land_entry_trigger(1.into(), draw)]
            );
            let altered = crate::tests::changed_source(source, |r| {
                let refs = r["binding"]["references"].as_array_mut().unwrap();
                refs[0]["target"] = json!("foreign-controller");
            });
            assert!(
                matches!(compile(MANIFEST,&altered),CompileResult::Unsupported {path,..} if path=="binding")
            );
        }
    }
}

#[test]
fn proliferate_landfall_uses_the_complete_archived_controller_binding() {
    let manifest = include_str!("../../../tests/fixtures/archive-manifest-current.json");
    let source = include_str!("../../../tests/fixtures/b45fdeab-00cc-4422-af9f-66f30a880a7c.json");
    let CompileResult::Lowered { definition } = compile(manifest, source) else {
        panic!("{:?}", compile(manifest, source));
    };
    assert_eq!(
        definition.triggered,
        [mtg_contracts::proliferate::landfall()]
    );
    let mut omitted = (*definition).clone();
    omitted.triggered.clear();
    omitted.capabilities = omitted.requirements();
    assert!(omitted.validate().is_err());
}
