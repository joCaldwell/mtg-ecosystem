//! Complete frozen controls with synthetic, rehashed typed-filter mutations.
//! Explicit null is not an omitted optional AST operand.
use super::*;
use serde_json::{Value, json};

const HEADER: &str = include_str!("../../../../tests/fixtures/archive-manifest-current.json");
const ATTACK_HEADER: &str =
    include_str!("../../../../tests/fixtures/cards-data-d2cd/archive-manifest-current.json");
const CAST: &str = include_str!(
    "../../../../tests/fixtures/cards-data-d2cc/0adcf8d2-6eb9-4679-8ad5-f28e2f4aba39.json"
);
const ATTACK: &str = include_str!(
    "../../../../tests/fixtures/cards-data-d2cd/218f835b-4195-4ee8-a4ef-e5ba58fa5374.json"
);
const DAMAGE: &str = include_str!(
    "../../../../tests/fixtures/cards-data-d2ce/ba5055ce-6809-47a4-bee6-fb97a5b60f8a.json"
);
const FIELDS: [&str; 5] = ["cls", "types", "nonTypes", "allOf", "anyOf"];

fn changed(raw: &str, block: usize, change: impl FnOnce(&mut Value)) -> String {
    tests::changed_source(raw, |record| {
        let result = &mut record["faces"][0]["result"];
        change(&mut result["abilities"][block]["trigger"]["what"]);
        result["lines"][block]["ability"] = result["abilities"][block].clone();
    })
}

fn lowered(header: &str, raw: &str) -> Box<Definition> {
    let result = compile(header, raw);
    let CompileResult::Lowered { definition } = result else {
        panic!("complete control must lower: {result:?}")
    };
    definition.validate().unwrap();
    definition
}

fn rejects(header: &str, raw: &str) {
    let result = compile(header, raw);
    assert!(
        matches!(result, CompileResult::Unsupported { .. }),
        "present-null AST filter must reject, not erase a constraint: {result:?}"
    );
}

#[test]
fn type_filter_null_frozen_controls_preserve_catalog_ir() {
    for (header, raw, name) in [
        (HEADER, CAST, "cards-data-d2cc"),
        (ATTACK_HEADER, ATTACK, "cards-data-d2cd"),
        (HEADER, DAMAGE, "damage-matches-v2"),
    ] {
        let definition = lowered(header, raw);
        assert_eq!(
            embedded_catalog(name)
                .unwrap()
                .definitions
                .iter()
                .find(|d| d.oracle_id == definition.oracle_id),
            Some(definition.as_ref())
        );
    }
}

#[test]
fn type_filter_null_absence_and_valid_boolean_fields_remain_distinct() {
    let any_cast = changed(CAST, 0, |what| *what = json!({"cls":"spell"}));
    assert!(matches!(
        lowered(HEADER, &any_cast).triggered[0].condition,
        TriggerCondition::SpellCast {
            spell: TypePredicate::Any {},
            ..
        }
    ));
    let unrestricted_attack = changed(ATTACK, 0, |what| what["filter"] = json!({}));
    assert!(matches!(
        lowered(ATTACK_HEADER, &unrestricted_attack).triggered[0].condition,
        TriggerCondition::CreatureAttacked {
            attacker: AttackSubject::Filter {
                controller: AttackerRelation::AnyPlayer,
                types: TypePredicate::Any {},
            }
        }
    ));
    let valid = changed(CAST, 0, |what| {
        *what = json!({"cls":"spell", "allOf":[
            {"anyOf":[{"types":["instant"]},{"types":["sorcery"]}]},
            {"nonTypes":["creature"]}
        ]});
    });
    let definition = lowered(HEADER, &valid);
    let TriggerCondition::SpellCast { spell, .. } = &definition.triggered[0].condition else {
        panic!("cast predicate")
    };
    assert_eq!(
        spell,
        &TypePredicate::All {
            terms: vec![
                TypePredicate::AnyOf {
                    terms: vec![
                        TypePredicate::Type {
                            kind: CardKind::Instant
                        },
                        TypePredicate::Type {
                            kind: CardKind::Sorcery
                        },
                    ]
                },
                TypePredicate::NotType {
                    kind: CardKind::Creature
                },
            ]
        }
    );
    for control in ["you", "opponent"] {
        let valid = changed(ATTACK, 0, |what| {
            what["filter"] = json!({"control":control,"types":["creature"],
                "allOf":[{"nonTypes":["artifact"]}],"anyOf":[{"types":["creature"]}]});
        });
        lowered(ATTACK_HEADER, &valid);
    }
}

#[test]
fn type_filter_null_cast_fields_reject_at_root_and_nested_levels() {
    for field in FIELDS {
        let root = changed(CAST, 0, |what| what[field] = Value::Null);
        rejects(HEADER, &root);
        for parent in ["allOf", "anyOf"] {
            let nested = changed(CAST, 0, |what| {
                *what = json!({"cls":"spell"});
                what[parent] = json!([{"types":["creature"]}]);
                what[parent][0][field] = Value::Null;
            });
            rejects(HEADER, &nested);
        }
    }
}

#[test]
fn type_filter_null_attacker_fields_never_become_unrestricted() {
    for field in ["types", "nonTypes", "allOf", "anyOf", "control"] {
        let mutant = changed(ATTACK, 0, |what| what["filter"][field] = Value::Null);
        rejects(ATTACK_HEADER, &mutant);
    }
}

#[test]
fn type_filter_null_recursive_damage_filters_reject_missing_constraints() {
    for parent in ["allOf", "anyOf"] {
        for field in FIELDS {
            let mutant = changed(DAMAGE, 2, |what| {
                what["filter"][parent] = json!([{"types":["artifact"]}]);
                what["filter"][parent][0][field] = Value::Null;
            });
            rejects(HEADER, &mutant);
        }
    }
}

#[test]
fn type_filter_null_other_malformed_values_still_reject() {
    for value in [json!([]), json!("creature"), json!([null]), json!({})] {
        let mutant = changed(CAST, 0, |what| what["allOf"] = value);
        rejects(HEADER, &mutant);
    }
}
