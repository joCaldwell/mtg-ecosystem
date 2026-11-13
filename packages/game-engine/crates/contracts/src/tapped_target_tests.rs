//! Synthetic IR predicate probes based on retained removal programs. These do
//! not change the source attribution or membership of any admitted catalog.
use super::*;

fn removal() -> Definition {
    Catalog::read(include_str!(
        "../../../tests/fixtures/expected-removal-ir.json"
    ))
    .unwrap()
    .definitions
    .into_iter()
    .find(|d| d.name == "Naturalize")
    .unwrap()
}

#[test]
fn tapped_filter_requires_its_capability_and_rejects_stack_status() {
    let mut d = removal();
    let old = serde_json::to_value(&d).unwrap();
    assert!(old["spell"]["targets"][0]["filter"].get("tapped").is_none());
    d.spell.as_mut().unwrap().targets[0]
        .filter
        .as_mut()
        .unwrap()
        .tapped = true;
    assert!(d.validate().is_err(), "missing capability must reject");
    d.capabilities = d.requirements();
    d.validate().unwrap();
    assert!(
        d.capabilities
            .contains(&Capability::TappedPermanentTargetV1)
    );
    assert_eq!(
        strict_read::<Definition>(&serde_json::to_string(&d).unwrap()).unwrap(),
        d
    );
    let mut p = d.spell.unwrap();
    p.targets[0].domain = TargetDomain::StackSpell;
    p.instructions = vec![Instruction::CounterStackObject {
        target_slot: 0,
        source_block: 0,
    }];
    assert!(validate_instruction_program(&p).is_err());
    p.targets[0].filter.as_mut().unwrap().tapped = false;
    validate_instruction_program(&p).unwrap();
}

#[test]
fn all_nested_target_programs_require_tapped_capability() {
    let mut child = removal().spell.unwrap();
    child.targets[0].filter.as_mut().unwrap().tapped = true;
    let mut modal = child.clone();
    modal.targets.clear();
    modal.instructions.clear();
    modal.modes = vec![
        SpellMode {
            program: child.clone(),
        },
        SpellMode {
            program: child.clone(),
        },
    ];
    validate_instruction_program(&modal).unwrap();
    let mut required = BTreeSet::new();
    add_program_requirements(&mut required, &modal);
    assert!(required.contains(&Capability::TappedPermanentTargetV1));
    // The same requirement collector is used for spells, activated abilities
    // and triggers. A nested target cannot evade the admission fence.
    child.targets[0].filter.as_mut().unwrap().tapped = false;
    let mut ordinary = BTreeSet::new();
    add_program_requirements(&mut ordinary, &child);
    assert!(!ordinary.contains(&Capability::TappedPermanentTargetV1));
}

#[test]
fn common_candidate_contract_rejects_distinct_slot_filters() {
    let mut p = removal().spell.unwrap();
    let mut second = p.targets[0].clone();
    second.slot = 1;
    second.selection_id.push_str("/second");
    p.targets.push(second);
    p.instructions.push(Instruction::DestroyPermanent {
        target_slot: 1,
        optional: false,
        regeneration_forbidden: false,
        source_block: 1,
    });
    validate_instruction_program(&p).unwrap();
    p.targets[1].filter.as_mut().unwrap().tapped = true;
    assert_eq!(
        validate_instruction_program(&p).unwrap_err(),
        "mixed target filters are unsupported"
    );
    p.targets[0].filter.as_mut().unwrap().tapped = true;
    validate_instruction_program(&p).unwrap();
    p.targets[1]
        .filter
        .as_mut()
        .unwrap()
        .non_colors
        .push(Color::B);
    assert_eq!(
        validate_instruction_program(&p).unwrap_err(),
        "mixed target filters are unsupported"
    );
}
