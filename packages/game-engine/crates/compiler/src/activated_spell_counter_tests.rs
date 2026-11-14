//! Exact archived Ertai source and hash-consistent unsupported AST probes.
use super::*;
use serde_json::{Value, json};
const MANIFEST: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
const ERTAI: &str =
    include_str!("../../../tests/fixtures/f2902dfd-888f-4b02-b8ad-1056f0059fc3.json");
fn changed(mutate: impl FnOnce(&mut Value)) -> String {
    tests::changed_source(ERTAI, |r| {
        mutate(&mut r["faces"][0]["result"]["abilities"][0]);
        let result = &mut r["faces"][0]["result"];
        result["lines"][0]["ability"] = result["abilities"][0].clone();
        let target = result["abilities"][0]["effects"][0]["steps"][0]["what"].clone();
        let path = "/faces/0/abilities/0/effects/0/steps/0/what";
        for n in r["binding"]["nodes"].as_array_mut().unwrap() {
            if n["node"] == "selection" && n["path"] == path {
                n["expression"] = target.clone();
            }
        }
        for n in r["binding"]["references"].as_array_mut().unwrap() {
            if n["path"] == path {
                n["expression"] = target.clone();
            }
        }
    })
}
fn lowered(raw: &str) -> Box<Definition> {
    let result = compile(MANIFEST, raw);
    let CompileResult::Lowered { definition } = result else {
        panic!("{result:?}")
    };
    definition.validate().unwrap();
    definition
}
#[test]
fn complete_ertai_preserves_composite_counter_payment_and_stack_spell_target() {
    assert_eq!(
        sha256(ERTAI.trim_end().as_bytes()),
        "c671cb99f15d2fa7d73514f9dd068045779783807fa5bca1345c3ae73839400c"
    );
    let r: Value = strict_read(ERTAI).unwrap();
    assert_eq!(
        r["source"]["oracle_text"],
        "{2}{U}{U}, {T}: Counter target spell."
    );
    let d = lowered(ERTAI);
    assert_eq!(d.name, "Ertai, Wizard Adept");
    assert_eq!(d.oracle_id, "f2902dfd-888f-4b02-b8ad-1056f0059fc3");
    assert_eq!(d.characteristics.kind, CardKind::Creature);
    assert!(d.characteristics.legendary);
    assert_eq!(
        d.characteristics.mana_cost,
        ManaCost {
            generic: 2.into(),
            colored: vec![Color::U]
        }
    );
    assert_eq!(d.characteristics.power, Some(1.into()));
    assert_eq!(d.characteristics.toughness, Some(1.into()));
    assert_eq!(d.provenance.source_index, 36694);
    assert_eq!(
        d.provenance.source_sha256,
        "83a1f96cd65c927188b3f53e50a4979df0e3ce6e04a9a0ad355b7dcb2eaee53d"
    );
    assert_eq!(
        d.provenance.ast_sha256,
        "f80cfe06818916986406db7c11b49354787f462b5ef6a61e5dc48df212359d4d"
    );
    assert_eq!(
        d.provenance.binding_sha256,
        "57b10b00209edd7a49accbe640eb07291b2c0efff4303fca761f8cd94abb8435"
    );
    assert_eq!(
        strict_read::<Value>(&d.provenance.source_json).unwrap(),
        r["source"]
    );
    assert_eq!(
        strict_read::<Value>(&d.provenance.abilities_json).unwrap(),
        r["faces"][0]["result"]["abilities"]
    );
    assert!(d.triggered.is_empty() && d.static_abilities.is_empty() && d.keywords.is_empty());
    assert_eq!(d.activated.len(), 1);
    let a = &d.activated[0];
    assert_eq!(
        a.cost,
        ActivationCost::Composite {
            costs: PermanentActivationCosts {
                mana: Some(ManaCost {
                    generic: 2.into(),
                    colored: vec![Color::U, Color::U]
                }),
                tap: true,
                sacrifice: false
            }
        }
    );
    assert_eq!(
        a.program.instructions,
        [Instruction::CounterStackObject {
            target_slot: 0,
            source_block: 0
        }]
    );
    assert_eq!(a.program.targets.len(), 1);
    assert_eq!(a.program.targets[0].domain, TargetDomain::StackSpell);
    assert_eq!(a.program.targets[0].selection_id, "line:0:7");
    assert!(a.program.targets[0].filter.is_none());
    assert!(
        d.capabilities
            .contains(&Capability::ActivatedSpellCounterV1)
    );
    let rebuilt = lowered(&changed(|_| {}));
    assert_eq!(rebuilt.activated, d.activated);
    assert_eq!(rebuilt.capabilities, d.capabilities);
}
#[test]
fn activated_counter_rejects_unrepresented_target_filters_and_occurrences() {
    for (field, value) in [
        (
            "filter",
            json!({"cls":"spell","allOf":[{"types":["creature"]}]}),
        ),
        (
            "filter",
            json!({"cls":"ability","anyOf":[{"abilityKind":"activated"},{"abilityKind":"triggered"}]}),
        ),
        ("count", json!({"amount":"fixed","value":2})),
        ("ref", json!("all")),
        ("selectionId", json!("")),
        ("upTo", json!(true)),
    ] {
        let raw = changed(|a| a["effects"][0]["steps"][0]["what"][field] = value);
        assert!(
            matches!(compile(MANIFEST, &raw), CompileResult::Unsupported { .. }),
            "{field}"
        );
    }
}
#[test]
fn activated_counter_rejects_incomplete_costs_and_extra_effects() {
    for costs in [
        json!([]),
        json!([{"cost":"tap-self"}]),
        json!([{"cost":"mana","symbols":["2","U","U"]}]),
        json!([{"cost":"mana","symbols":["-1"]},{"cost":"tap-self"}]),
    ] {
        let raw = changed(|a| a["costs"] = costs);
        assert!(matches!(
            compile(MANIFEST, &raw),
            CompileResult::Unsupported { .. }
        ));
    }
    let raw = changed(|a| {
        a["effects"].as_array_mut().unwrap().push(json!({"steps":[{"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1},"optional":false}]}))
    });
    assert!(matches!(
        compile(MANIFEST, &raw),
        CompileResult::Unsupported { .. }
    ));
}
