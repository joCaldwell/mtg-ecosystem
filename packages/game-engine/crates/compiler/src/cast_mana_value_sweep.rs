//! CR 109.5, 201.5, 608.2h and 702.85c. Both cascade instances are distinct.
//! The controller's history excludes the source, and recipients are evaluated on resolution.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::bindings::Bindings;
use serde_json::{Value, json};

pub(super) fn validate_binding(
    identity: &str,
    source: &Value,
    binding: &Value,
) -> Result<(), Failure> {
    let mut graph = Bindings::new(identity, source);
    let keywords = "/faces/0/abilities/0";
    graph.ability(keywords, "keywords", "static-source-controller");
    let scope = graph.id(keywords, "ability");
    for i in 0..2 {
        let path = format!("{keywords}/keywords/{i}");
        let you = graph.node(
            &path,
            "you",
            "runtime-player",
            json!({
                "scope":scope, "role":"trigger-controller", "rules":["109.5","702.85"]
            }),
        );
        graph.node(&path, "keyword", "keyword-ability", json!({
            "scope":scope, "keyword":"cascade", "abilities":["triggered"], "you":you, "rules":["702.85"]
        }));
    }

    let spell = "/faces/0/abilities/1";
    graph.ability(spell, "spell", "spell-controller");
    let scope = graph.id(spell, "ability");
    let you = graph.id(spell, "you");
    let object = graph.id("/source", "object");
    let step = format!("{spell}/effects/0/steps/0");
    let set = format!("{step}/to/ref");
    let control = format!("{set}/filter/control");
    graph.reference(
        &format!("{step}/source"),
        &json!({"ref":"self"}),
        &object,
        "201.5",
    );
    graph.reference(
        &format!("{step}/amount/who"),
        &json!({"player":"you"}),
        &you,
        "109.5",
    );
    graph.reference(
        &format!("{step}/amount/excluding"),
        &json!({"ref":"this","noun":"spell"}),
        &object,
        "201.5",
    );
    let opponents = graph.node(
        &control,
        "opponents",
        "derived-player",
        json!({
            "scope":scope, "role":"opponents", "of":you, "rules":["102.2","109.5"]
        }),
    );
    graph.reference(
        &control,
        &json!({"implicit":"opponents"}),
        &opponents,
        "102.2",
    );
    let creatures = json!({"ref":"each","filter":{"types":["creature"],"control":"opponent"}});
    let query = graph.node(
        &set,
        "query",
        "selection",
        json!({
            "scope":scope,"expression":creatures,"cardinality":"many","timing":"resolution",
            "selection":"evaluate-reference","rules":["608.2h"]
        }),
    );
    graph.reference(&set, &creatures, &query, "608.2h");
    graph.node(
        &step,
        "event",
        "event",
        json!({
            "scope":scope,"operation":"damage","inputs":[object,query],
            "availability":"when-performed","rules":["608.2c"]
        }),
    );
    graph.result(&step, "quantity", "measured-quantity", None);
    graph.validate_ordered(
        binding,
        "requires complete cascade and cast mana value damage bindings",
    )
}

#[test]
fn cast_mana_value_sweep() {
    use crate::tests::changed_source;
    let historical =
        include_str!("../../../tests/fixtures/ramos/0623c51a-e829-413f-a3a0-817c0902821e.json");
    assert!(
        matches!(compile(include_str!("../../../tests/fixtures/ramos/archive-manifest-current.json"),historical),
        CompileResult::Unsupported { path, .. } if path == "status")
    );
    let Some(path) = std::env::var_os("MTG_CAST_MANA_VALUE_SWEEP_ARCHIVE_PROBE") else {
        return;
    };
    let archive = std::fs::read_to_string(path).unwrap();
    assert_eq!(
        sha256(archive.as_bytes()),
        "47e7bd830e97010eade2f758d51631f8fb211638945b40799d6e17cd9ff79f84"
    );
    let mut lines = archive.lines();
    let header = lines.next().unwrap();
    let row = lines.next().unwrap();
    assert!(lines.next().is_none());
    assert_eq!(
        header,
        include_str!("../../../tests/fixtures/archive-manifest-cast-mana-value-sweep.json")
            .trim_end()
    );
    let origin = accepted_manifest(header).unwrap();
    assert_eq!(
        origin.record_sha256,
        BTreeSet::from([sha256(row.as_bytes())])
    );
    let compiled = compile(header, row);
    let CompileResult::Lowered { definition: d } = compiled else {
        panic!("{compiled:?}")
    };
    assert_eq!(
        d.provenance.source_sha256,
        "76f4484f84903b410fab2a07c008126fd1e66a2268385a42276f73a0f1042c78"
    );
    assert_eq!(d.source_labels.as_ref().unwrap().triggered, [0, 0]);
    assert_eq!(d.triggered.len(), 2);
    assert!(
        d.triggered
            .iter()
            .all(|t| t.condition == TriggerCondition::SelfSpellCast
                && t.program == canonical_cascade_program(0))
    );
    assert_eq!(
        d.spell.unwrap().instructions,
        [Instruction::DamageSet {
            sets: vec![ObjectSet::OpponentCreatures],
            each_player: false,
            amount: WipeAmount::OtherCastsManaValue,
            source_block: 1
        }]
    );
    assert!(d.capabilities.contains(&Capability::CastManaValueSweepV1));

    for reference in 0..5 {
        let wrong = changed_source(row, |r| {
            r["binding"]["references"][reference]["target"] = json!("wrong scope");
        });
        assert!(
            matches!(compile(header, &wrong), CompileResult::Unsupported {path,..} if path=="binding")
        );
    }
    // A duplicate keyword binding cannot stand in for the second instance.
    let wrong = changed_source(row, |r| {
        let nodes = &mut r["binding"]["nodes"];
        nodes[7] = nodes[5].clone();
    });
    assert!(
        matches!(compile(header, &wrong), CompileResult::Unsupported {path,..} if path == "binding")
    );
    for field in ["during", "who", "excluding"] {
        let wrong = changed_source(row, |r| {
            let result = &mut r["faces"][0]["result"];
            let ability = &mut result["abilities"][1];
            ability["effects"][0]["steps"][0]["amount"][field] = json!(null);
            result["lines"][1]["ability"] = ability.clone();
        });
        assert!(!matches!(
            compile(header, &wrong),
            CompileResult::Lowered { .. }
        ));
    }
}
