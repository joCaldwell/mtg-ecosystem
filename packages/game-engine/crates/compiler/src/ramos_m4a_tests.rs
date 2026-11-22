//! Complete frozen Apex; Wanderer is covered by the dependent M4b tests.
use super::*;
const MANIFEST: &str = include_str!("../../../tests/fixtures/ramos/archive-manifest-current.json");
const APEX: &str =
    include_str!("../../../tests/fixtures/ramos/b4d6747c-3516-4e71-be30-af007c6b1dc4.json");
fn apex() -> Definition {
    match compile(MANIFEST, APEX) {
        CompileResult::Lowered { definition } => *definition,
        other => panic!("complete Apex: {other:?}"),
    }
}
#[test]
fn ramos_m4a_frozen_apex_preserves_four_independent_same_block_instances() {
    assert_eq!(
        sha256(APEX.as_bytes()),
        "203a47f8178a07b91c366e934714d9717c76244b04d01bdcb676764f1de0fce1"
    );
    let record: serde_json::Value = strict_read(APEX).unwrap();
    assert_eq!(record["sourceIndex"], 27232);
    assert_eq!(
        record["source"]["id"],
        "8fa281e1-5c48-4bba-b8e9-88c6f5f53abb"
    );
    assert_eq!(record["binding"]["fullyBound"], true);
    assert_eq!(
        record["faces"][0]["result"]["abilities"][0]["keywords"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    let d = apex();
    d.validate().unwrap();
    assert_eq!(d.characteristics.kind, CardKind::Creature);
    assert_eq!(d.characteristics.subtypes, ["Chimera", "Hydra"]);
    assert!(!d.characteristics.legendary);
    assert_eq!(d.characteristics.power, Some(10.into()));
    assert_eq!(d.characteristics.toughness, Some(10.into()));
    assert_eq!(
        d.characteristics.mana_cost,
        ManaCost {
            generic: 8.into(),
            colored: vec![Color::G, Color::G]
        }
    );
    assert_eq!(d.characteristics.colors, [Color::G]);
    assert_eq!(d.characteristics.color_identity, [Color::G]);
    assert_eq!(d.triggered.len(), 4);
    for a in &d.triggered {
        assert_eq!(a.condition, TriggerCondition::SelfSpellCast);
        assert_eq!(a.functioning_zone, FunctioningZone::Stack);
        assert!(matches!(
            a.program.instructions.as_slice(),
            [Instruction::Cascade {
                threshold: mtg_contracts::CascadeThreshold::SourceSpellManaValue,
                source_block: 0
            }]
        ));
    }
    for c in [
        Capability::CascadeV1,
        Capability::ResolutionCastV1,
        Capability::RandomBottomV1,
    ] {
        assert!(d.capabilities.contains(&c));
        let mut missing = d.clone();
        missing.capabilities.remove(&c);
        assert!(missing.validate().is_err());
    }
    // Generic IR multiplicity has no printed-instance ceiling. These are
    // structural validator controls, not fabricated complete-source positives.
    for count in [1, 2, 5, 257] {
        let mut repeated = d.clone();
        repeated.triggered = vec![d.triggered[0].clone(); count];
        repeated.validate().unwrap();
    }
}
#[test]
fn ramos_m4a_each_instance_retains_exact_shape_and_orphan_cascade_rejects() {
    let d = apex();
    for index in 0..d.triggered.len() {
        for mutation in 0..10 {
            let mut bad = d.clone();
            let a = &mut bad.triggered[index];
            match mutation {
                0 => a.functioning_zone = FunctioningZone::Battlefield,
                1 => a.program.functioning_zone = FunctioningZone::Battlefield,
                2 => a.program.instructions.push(Instruction::draw(
                    PlayerExpr::you(Context::Triggered),
                    1.into(),
                    0,
                )),
                3 => a.program.instructions.clear(),
                4 => a.condition = TriggerCondition::SelfEntersBattlefield,
                5 => a.program.timing = SpellTiming::MainPhase,
                6 => a
                    .program
                    .instructions
                    .push(a.program.instructions[0].clone()),
                7 => a.program.targets.push(TargetSpec {
                    slot: 0,
                    selection_id: "synthetic-creature".into(),
                    chooser: Chooser::Controller,
                    stage: EvaluationStage::Announcement,
                    filter: None,
                    domain: TargetDomain::BattlefieldCreature,
                }),
                8 => a.program.modes.push(mtg_contracts::SpellMode {
                    program: a.program.clone(),
                }),
                9 => {
                    let CompileResult::Lowered { definition } = compile(
                        MANIFEST,
                        include_str!("../../../tests/fixtures/dismantling-blow.json"),
                    ) else {
                        panic!()
                    };
                    a.program.optional_additional_costs =
                        definition.spell.unwrap().optional_additional_costs;
                }
                _ => unreachable!(),
            }
            bad.capabilities = bad.requirements();
            assert!(
                bad.validate().is_err(),
                "instance {index}, mutation {mutation}"
            );
        }
    }
    let mut orphan = d.clone();
    orphan
        .spell
        .as_mut()
        .unwrap()
        .instructions
        .push(d.triggered[0].program.instructions[0].clone());
    orphan.capabilities = orphan.requirements();
    assert!(orphan.validate().is_err());
}
#[test]
fn ramos_m4a_keyword_parameters_and_mixed_programs_reject() {
    for field in ["cost", "filter", "params"] {
        let raw = tests::changed_source(APEX, |r| {
            for section in ["abilities", "lines"] {
                let a = if section == "abilities" {
                    &mut r["faces"][0]["result"][section][0]
                } else {
                    &mut r["faces"][0]["result"][section][0]["ability"]
                };
                a["keywords"][2][field] = serde_json::json!([]);
            }
        });
        assert!(
            !matches!(compile(MANIFEST, &raw), CompileResult::Lowered { .. }),
            "{field}"
        );
    }
    let raw = tests::changed_source(APEX, |r| {
        r["faces"][0]["result"]["abilities"][0]["effects"] = serde_json::json!([]);
        r["faces"][0]["result"]["lines"][0]["ability"]["effects"] = serde_json::json!([]);
    });
    assert!(!matches!(
        compile(MANIFEST, &raw),
        CompileResult::Lowered { .. }
    ));
}
