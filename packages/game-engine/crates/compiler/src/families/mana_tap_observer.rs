use super::*;
use serde_json::{Value, json};

pub(crate) fn lower(
    source: &Source,
    shape: &Value,
    result: &ParseResult,
    binding: &Value,
) -> Result<Option<Ability>, Failure> {
    let Some((ast, trigger)) = mtg_contracts::mana_tap_observer::printed(&source.oracle_text)
    else {
        return Ok(None);
    };
    let fail = "requires exact tapped land, mana activator and damage source";
    if source.type_line != "Enchantment"
        || decode::<Value>(result.abilities.get(), "abilities")? != json!([ast])
    {
        return Err(unsupported("abilities", fail));
    }
    let root = "/faces/0/abilities/0";
    let observe = format!("{root}/trigger");
    let step = format!("{root}/effects/0/steps/0");
    let mut graph = bindings::Bindings::new(&source.oracle_id, shape);
    graph.ability(root, "triggered", "trigger-controller");
    let scope = graph.id(root, "ability");
    let mut selections = vec![];
    for name in ["what", "who"] {
        let path = format!("{observe}/{name}");
        let expression = &ast["trigger"][name];
        let query = graph.node(
            &path,
            "query",
            "selection",
            json!({
                "scope":scope,"expression":expression,"cardinality":"one","timing":"trigger-match",
                "selection":"evaluate-reference","rules":["608.2h"]
            }),
        );
        graph.reference(&path, expression, &query, "608.2h");
        selections.push(query);
    }
    graph.node(
        &observe,
        "event",
        "event",
        json!({
            "scope":scope,"operation":"trigger:taps-for-mana","inputs":selections,
            "availability":"when-performed","rules":["608.2c"]
        }),
    );
    graph.result(
        &observe,
        "objects",
        "same-objects",
        Some(&ast["trigger"]["what"]["filter"]),
    );
    graph.result(&observe, "player", "same-objects", None);
    let source_id = graph.id("/source", "object");
    let activator = graph.id(&observe, "player");
    let damage = &ast["effects"][0]["steps"][0];
    graph.reference(
        &format!("{step}/source"),
        &damage["source"],
        &source_id,
        "201.5",
    );
    graph.reference(
        &format!("{step}/to/ref"),
        &damage["to"]["ref"],
        &activator,
        "608.2k",
    );
    graph.node(&step, "event", "event", json!({
        "scope":scope,"operation":"damage","inputs":[source_id,activator],"availability":"when-performed","rules":["608.2c"]
    }));
    graph.result(&step, "quantity", "measured-quantity", None);
    graph.validate_ordered(binding, fail)?;
    Ok(Some(Ability::BoundTrigger(trigger)))
}

#[cfg(test)]
#[test]
fn slice6c_mana_tap_activator_is_exact_and_untargeted() {
    let header = include_str!("../../../../tests/fixtures/archive-manifest-phase1-r4.json");
    let row = include_str!(
        "../../../../tests/fixtures/phase1-r4/0f1afedd-c60f-454f-b84a-c8117aec0128.json"
    );
    let result = compile(header, row);
    let CompileResult::Lowered { mut definition } = result else {
        panic!("{result:?}");
    };
    let t = &definition.triggered[0];
    assert_eq!(
        t.condition,
        TriggerCondition::ManaTapped {
            filter: ManaTapFilter::Land
        }
    );
    assert!(t.program.targets.is_empty());
    assert!(matches!(t.program.instructions.as_slice(),
        [kernel!(Damage { to: kir::DamageTo::Ref { who: kir::Ref::EventPlayer { role: kir::EventPlayerRole::ActivatingPlayer } }, amount: fixed!(amount), .. })] if *amount == Quantity::from(1)));
    definition.triggered[0].program.instructions[0] = Instruction::damage(
        kir::DamageTo::Ref {
            who: kir::Ref::controller(kir::Context::Triggered),
        },
        kir::Amount::Fixed {
            value: Quantity::from(1),
        },
        0,
    );
    definition.capabilities = definition.requirements();
    assert!(definition.validate().is_err());
    let wrong = crate::tests::changed_source(row, |r| {
        let who = r["binding"]["nodes"][3]["id"].clone();
        let refs = r["binding"]["references"].as_array_mut().unwrap();
        refs.last_mut().unwrap()["target"] = who;
    });
    assert!(!matches!(
        compile(header, &wrong),
        CompileResult::Lowered { .. }
    ));
}
