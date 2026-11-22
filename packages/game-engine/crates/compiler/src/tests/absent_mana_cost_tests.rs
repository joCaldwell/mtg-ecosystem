//! Frozen controls and explicitly synthetic printed-cost mutations.
//! Mutated records are rehashed for compiler isolation, not archive admission.
use super::*;
use serde_json::{Value, json};

const NONLANDS: [&str; 4] = [
    include_str!("../../../../tests/fixtures/lightning-bolt.json"),
    include_str!("../../../../tests/fixtures/sacred-nectar.json"),
    include_str!("../../../../tests/fixtures/llanowar-elves.json"),
    include_str!("../../../../tests/fixtures/mind-stone.json"),
];
const LANDS: [&str; 2] = [
    include_str!("../../../../tests/fixtures/forest.json"),
    include_str!("../../../../tests/fixtures/island.json"),
];

fn changed_cost(raw: &str, value: Option<Value>) -> String {
    tests::changed_source(raw, |record| {
        let source = record["source"].as_object_mut().unwrap();
        if let Some(value) = value {
            source.insert("mana_cost".into(), value);
        } else {
            source.remove("mana_cost");
        }
    })
}

fn lowered(raw: &str) -> Box<Definition> {
    let result = compile(fixtures::MANIFEST, raw);
    let CompileResult::Lowered { definition } = result else {
        panic!("complete supported control must lower: {result:?}")
    };
    definition.validate().unwrap();
    definition
}

#[test]
fn frozen_printed_cost_controls_remain_supported() {
    for raw in NONLANDS {
        let record: Value = serde_json::from_str(raw).unwrap();
        assert!(!record["source"]["mana_cost"].as_str().unwrap().is_empty());
        let definition = lowered(raw);
        assert_ne!(definition.characteristics.kind, CardKind::Land);
        assert_ne!(
            definition.characteristics.mana_cost,
            ManaCost {
                generic: 0.into(),
                colored: vec![]
            }
        );
    }
}

#[test]
fn absent_nonland_cost_rejects_instead_of_becoming_zero() {
    for raw in NONLANDS {
        let mutant = changed_cost(raw, Some(json!("")));
        let result = compile(fixtures::MANIFEST, &mutant);
        assert!(
            matches!(&result, CompileResult::Unsupported { path, reason }
                if path == "source.mana_cost" && reason.contains("absent-cost")),
            "absent nonland cost must not become a payable zero cost: {result:?}"
        );
    }
}

#[test]
fn explicit_zero_nonland_cost_remains_distinct_and_supported() {
    for raw in NONLANDS {
        let original = lowered(raw);
        let zero = lowered(&changed_cost(raw, Some(json!("{0}"))));
        assert_eq!(
            zero.characteristics.mana_cost,
            ManaCost {
                generic: 0.into(),
                colored: vec![]
            }
        );
        assert_eq!(zero.characteristics.kind, original.characteristics.kind);
        assert_eq!(zero.spell, original.spell);
        assert_eq!(zero.activated, original.activated);
        assert_eq!(zero.mana_abilities, original.mana_abilities);
    }
}

#[test]
fn land_without_mana_cost_remains_supported() {
    for raw in LANDS {
        let record: Value = serde_json::from_str(raw).unwrap();
        assert_eq!(record["source"]["mana_cost"], "");
        let land = lowered(raw);
        assert_eq!(land.characteristics.kind, CardKind::Land);
        assert_eq!(
            land.characteristics.mana_cost,
            ManaCost {
                generic: 0.into(),
                colored: vec![]
            }
        );
    }
}

#[test]
fn missing_null_and_malformed_costs_do_not_select_zero() {
    for value in [None, Some(Value::Null), Some(json!(0)), Some(json!(" "))] {
        let result = compile(fixtures::MANIFEST, &changed_cost(NONLANDS[0], value));
        assert!(
            !matches!(result, CompileResult::Lowered { .. }),
            "malformed cost must reject: {result:?}"
        );
    }
}
