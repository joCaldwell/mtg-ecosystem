//! Bind the resolution query to the spell controller and prove its complete scope.
use super::*;
use serde_json::{Value, json};

pub(super) fn bind(
    definition: &Definition,
    source: &Value,
    binding: &Value,
    result: &ParseResult,
) -> Result<(), Failure> {
    if !definition.spell.as_ref().is_some_and(|spell| {
        spell.instructions.iter().any(|i| {
            matches!(
                i,
                Instruction::PumpSet {
                    set: ObjectSet::ControllerCreatures,
                    ..
                }
            )
        })
    }) {
        return Ok(());
    }
    let root = "/faces/0/abilities/0";
    let step = format!("{root}/effects/0/steps/0");
    let path = format!("{step}/what");
    let ast: Value = decode(result.abilities.get(), "abilities")?;
    let what = &ast[0]["effects"][0]["steps"][0]["what"];
    let mut graph = bindings::Bindings::new(&definition.oracle_id, source);
    graph.ability(root, "spell", "spell-controller");
    let scope = graph.id(root, "ability");
    let you = graph.id(root, "you");
    graph.reference(
        &format!("{path}/filter/control"),
        &json!({"player":"you"}),
        &you,
        "109.5",
    );
    let query = graph.node(
        &path,
        "query",
        "selection",
        json!({
            "scope":scope,"expression":what,"cardinality":"many","timing":"resolution",
            "selection":"evaluate-reference","rules":["608.2h"]
        }),
    );
    graph.reference(&path, what, &query, "608.2h");
    graph.event(root, &step, "pump", &query, false);
    graph.validate(
        binding,
        "requires the spell controller's exact creature-set pump",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controller_creature_pump_frozen_source_and_rejects_later_rider() {
        let manifest = include_str!("../../../tests/fixtures/archive-manifest-current.json");
        let record = include_str!("../../../tests/fixtures/ethereal-guidance.json");
        let CompileResult::Lowered { definition } = compile(manifest, record) else {
            panic!("exact frozen source did not lower");
        };
        let spell = definition.spell.as_ref().unwrap();
        assert!(spell.targets.is_empty());
        assert_eq!(
            spell.instructions,
            [Instruction::PumpSet {
                set: ObjectSet::ControllerCreatures,
                power: 2.into(),
                toughness: 1.into(),
                source_block: 0,
            }]
        );
        let mut forged = *definition;
        forged
            .spell
            .as_mut()
            .unwrap()
            .instructions
            .push(Instruction::DrawCards {
                recipient: None,
                count: 1.into(),
                source_block: 0,
            });
        forged.capabilities = forged.requirements();
        assert!(forged.validate().is_err());

        let wrong_controller = crate::tests::changed_source(record, |r| {
            r["binding"]["references"][0]["target"] = json!("absent controller");
        });
        assert!(matches!(
            compile(manifest, &wrong_controller),
            CompileResult::Unsupported { .. }
        ));
    }
}
