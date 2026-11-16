//! Synthetic contract/predicate probes; paid source evidence lives in core/compiler.
use super::*;
pub(super) fn typed_search() -> Definition {
    let mut d = basic_land_split_tests::split_search();
    d.spell.as_mut().unwrap().instructions = vec![Instruction::SearchTypedLands {
        search: TypedLandSearch {
            any_basic_land_types: vec![BasicLandType::Forest],
            maximum: 3.into(),
            force_tapped: false,
        },
        source_block: 0,
    }];
    d.capabilities = d.requirements();
    d
}
#[test]
fn typed_land_search_contract_strict_complete_program_and_unbounded_positive_quantity() {
    let d = typed_search();
    d.validate().unwrap();
    assert!(d.capabilities.contains(&Capability::TypedLandSearchV1));
    assert!(!d.capabilities.contains(&Capability::BasicLandSearchV1));
    for maximum in [
        "1",
        "2",
        "3",
        "101",
        "999999999999999999999999999999999999999",
    ] {
        let mut probe = d.clone();
        let Instruction::SearchTypedLands { search, .. } =
            &mut probe.spell.as_mut().unwrap().instructions[0]
        else {
            unreachable!()
        };
        search.maximum = maximum.parse().unwrap();
        probe.validate().unwrap();
    }
    for maximum in ["0", "-1"] {
        let mut probe = d.clone();
        let Instruction::SearchTypedLands { search, .. } =
            &mut probe.spell.as_mut().unwrap().instructions[0]
        else {
            unreachable!()
        };
        search.maximum = maximum.parse().unwrap();
        assert!(probe.validate().is_err());
    }
    for types in [
        vec![],
        vec![BasicLandType::Forest, BasicLandType::Forest],
        vec![BasicLandType::Forest, BasicLandType::Island],
    ] {
        let mut probe = d.clone();
        let Instruction::SearchTypedLands { search, .. } =
            &mut probe.spell.as_mut().unwrap().instructions[0]
        else {
            unreachable!()
        };
        search.any_basic_land_types = types;
        assert!(probe.validate().is_err());
    }
    let mut missing = d.clone();
    missing.capabilities.remove(&Capability::TypedLandSearchV1);
    assert!(missing.validate().is_err());
    let mut mixed = d.clone();
    mixed
        .spell
        .as_mut()
        .unwrap()
        .instructions
        .push(Instruction::DrawCards {
            count: 1.into(),
            recipient: None,
            source_block: 0,
        });
    mixed.capabilities = mixed.requirements();
    assert!(mixed.validate().is_err());
    let mut modal = d.clone();
    let child = modal.spell.as_ref().unwrap().clone();
    modal.spell.as_mut().unwrap().instructions.clear();
    modal.spell.as_mut().unwrap().modes = vec![
        SpellMode {
            program: child.clone(),
        },
        SpellMode { program: child },
    ];
    modal.capabilities = modal.requirements();
    assert!(modal.validate().is_err());
    for (pointer, value) in [
        (
            "/spell/instructions/0/search/unknown",
            serde_json::json!(true),
        ),
        (
            "/spell/instructions/0/search/any_basic_land_types/0",
            serde_json::json!("Desert"),
        ),
        ("/spell/instructions/0/search/maximum", serde_json::json!(3)),
    ] {
        let mut value0 = serde_json::to_value(&d).unwrap();
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        let at = value0.pointer_mut(parent).unwrap();
        if let Ok(i) = key.parse::<usize>() {
            at[i] = value;
        } else {
            at[key] = value;
        }
        assert!(strict_read::<Definition>(&value0.to_string()).is_err());
    }
}
#[test]
fn typed_land_search_synthetic_nonbasic_characteristic_predicate_only() {
    let mut c = typed_search().characteristics;
    c.kind = CardKind::Land;
    c.basic = false;
    c.subtypes = vec!["Forest".into(), "Island".into()];
    let forest = TypedLandSearch {
        any_basic_land_types: vec![BasicLandType::Forest],
        maximum: 1.into(),
        force_tapped: false,
    };
    let farseek = TypedLandSearch {
        any_basic_land_types: vec![
            BasicLandType::Plains,
            BasicLandType::Island,
            BasicLandType::Swamp,
            BasicLandType::Mountain,
        ],
        maximum: 1.into(),
        force_tapped: true,
    };
    assert!(forest.accepts(&c));
    assert!(farseek.accepts(&c));
    c.subtypes = vec!["Forest".into()];
    assert!(!farseek.accepts(&c));
    c.subtypes.clear(); // Command Tower's untyped land shape cannot satisfy it.
    assert!(!forest.accepts(&c));
    c.kind = CardKind::Creature;
    c.subtypes = vec!["Forest".into()];
    assert!(!forest.accepts(&c));
    // This is no valid Definition or admitted nonbasic witness. Complete shock,
    // cycling and conditional-entry source support is still missing.
}
