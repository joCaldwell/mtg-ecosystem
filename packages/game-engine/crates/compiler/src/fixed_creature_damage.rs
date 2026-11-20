//! Bind the complete fixed damage sentence to its spell source and one target.
use super::*;
use serde_json::Value;

pub(super) fn lower(
    source: &Value,
    result: &ParseResult,
    binding: &Value,
    identity: &str,
) -> Result<Option<SpellProgram>, Failure> {
    if !matches!(result.lines.as_slice(), [line] if line.text.ends_with(" damage to target creature."))
    {
        return Ok(None);
    }
    let ast: Value = decode(result.abilities.get(), "abilities")?;
    let Some(program) = mtg_contracts::fixed_creature_damage::program(source, &ast) else {
        return Ok(None);
    };
    let root = "/faces/0/abilities/0";
    let mut graph = bindings::Bindings::new(identity, source);
    graph.ability(root, "spell", "spell-controller");
    announced_x_effects::bind_step(
        &mut graph,
        &format!("{root}/effects/0/steps/0"),
        &ast[0]["effects"][0]["steps"][0],
    );
    graph.validate(
        binding,
        "requires the spell's own damage and announced creature",
    )?;
    Ok(Some(program))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_creature_damage_exact_frozen_source_and_binding() {
        let manifest = include_str!("../../../tests/fixtures/archive-manifest-current.json");
        let record = include_str!("../../../tests/fixtures/breath-of-fire.json");
        let CompileResult::Lowered { definition } = compile(manifest, record) else {
            panic!("exact frozen source did not lower");
        };
        let existing_activation =
            include_str!("../../../tests/fixtures/ramos/23270e4a-a222-4ffb-a946-d0e20d665187.json");
        assert!(matches!(
            compile(manifest, existing_activation),
            CompileResult::Lowered { .. }
        ));
        let spell = definition.spell.as_ref().unwrap();
        assert!(matches!(spell.targets.as_slice(), [target]
            if target.domain == TargetDomain::BattlefieldCreature && target.selection_id == "line:0:5"));
        assert_eq!(
            spell.instructions,
            [Instruction::DealDamage {
                target_slot: 0,
                amount: 2.into(),
                source_block: 0,
            }]
        );

        let wrong_binding = crate::tests::changed_source(record, |record| {
            record["binding"]["references"][0]["target"] = serde_json::json!("unbound source");
        });
        assert!(matches!(
            compile(manifest, &wrong_binding),
            CompileResult::Unsupported { .. }
        ));
        let mut forged = *definition;
        forged.spell.as_mut().unwrap().instructions[0] = Instruction::DealDamage {
            target_slot: 0,
            amount: 3.into(),
            source_block: 0,
        };
        assert!(forged.validate().is_err());
    }
}
