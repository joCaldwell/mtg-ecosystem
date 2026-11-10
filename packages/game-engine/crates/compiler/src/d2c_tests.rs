use super::*;
const MANIFEST: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
const ARENA: &str = include_str!(
    "../../../tests/fixtures/cards-data-d2c/ee579a32-a048-4335-b966-231ba731cdea.json"
);
fn catalog() -> Catalog {
    catalog::build_catalog(std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/catalogs/cards-data-d2c.json"
    )))
    .unwrap()
}
fn changed(change: impl FnOnce(&mut serde_json::Value)) -> String {
    tests::changed_source(ARENA, |r| {
        change(&mut r["faces"][0]["result"]["abilities"][0]);
        r["faces"][0]["result"]["lines"][0]["ability"] =
            r["faces"][0]["result"]["abilities"][0].clone();
    })
}
#[test]
fn d2c_complete_sources_minimum_version_and_capability_closure() {
    let c = catalog();
    assert_eq!(c.definitions.len(), 11);
    for d in &c.definitions {
        assert!(d.capabilities.contains(&Capability::StepBeginningTriggerV1));
        assert_eq!(d.capabilities, d.requirements());
        d.validate().unwrap();
        let mut bad = d.clone();
        bad.capabilities.remove(&Capability::StepBeginningTriggerV1);
        assert!(bad.validate().is_err());
    }
    let arena = c
        .definitions
        .iter()
        .find(|d| d.name == "Phyrexian Arena")
        .unwrap();
    assert_eq!(
        arena.triggered[0].condition,
        TriggerCondition::StepBegan {
            step: BeginningStep::Upkeep,
            turn_relation: TurnRelation::Controller
        }
    );
    assert!(matches!(
        arena.triggered[0].program.instructions.as_slice(),
        [Instruction::DrawCards { .. }, Instruction::LoseLife { .. }]
    ));
    let mut old = c.clone();
    old.schema_version = 20;
    assert!(old.validate().unwrap_err().contains("IR v21"));
    for step in ["upkeep", "draw-step", "end-step"] {
        for whose in ["your", "each-player", "each-opponent"] {
            let raw = changed(|a| {
                a["trigger"]["phase"] = step.into();
                a["trigger"]["whose"] = whose.into();
            });
            let CompileResult::Lowered { definition } = compile(MANIFEST, &raw) else {
                panic!("{step}/{whose}")
            };
            definition.validate().unwrap();
            assert_eq!(
                definition.triggered[0].condition.reporting_key(),
                "step_began"
            );
        }
    }
}
#[test]
fn d2c_whole_cards_reject_conditions_optionality_unknown_relations_and_zones() {
    for raw in [
        changed(|a| a["condition"] = serde_json::json!({"condition":"unknown"})),
        changed(|a| a["trigger"]["whose"] = "that-player".into()),
        changed(|a| {
            a["trigger"]["whose"] = serde_json::json!({"player":"controller","of":{"ref":"enchanted","noun":"creature"}})
        }),
        changed(|a| a["trigger"]["phase"] = "precombat-main".into()),
        changed(|a| a["trigger"]["phase"] = "combat".into()),
        changed(|a| a["trigger"]["phase"] = "cleanup".into()),
        changed(|a| a["trigger"]["during"] = "each-opponent".into()),
        changed(|a| a["trigger"]["from"] = "graveyard".into()),
        changed(|a| a["functioningZone"] = "graveyard".into()),
        changed(|a| {
            // IR27 admits an optional controller draw of one card, while the
            // optional draw-two shape remains outside this finite instruction.
            a["effects"][0]["steps"][0]["optional"] = true.into();
            a["effects"][0]["steps"][0]["amount"]["value"] = 2.into();
        }),
        changed(|a| a["effects"][0]["steps"][0]["effect"] = "pay-cost".into()),
    ] {
        assert!(
            matches!(compile(MANIFEST, &raw), CompileResult::Unsupported { .. }),
            "{}",
            serde_json::to_string(&compile(MANIFEST, &raw)).unwrap()
        );
    }
    // This synthetic AST variant exercises the newly supported optional draw,
    // not a claim that the archived Arena text contains "may". The following
    // life-loss instruction remains mandatory and in its original position.
    let optional = changed(|a| a["effects"][0]["steps"][0]["optional"] = true.into());
    let CompileResult::Lowered { definition } = compile(MANIFEST, &optional) else {
        panic!("IR27 optional controller draw on the existing step trigger")
    };
    assert!(matches!(
        definition.triggered[0].program.instructions.as_slice(),
        [
            Instruction::OptionalDrawCards { source_block: 0 },
            Instruction::LoseLife { .. },
        ]
    ));
    assert!(
        definition
            .capabilities
            .contains(&Capability::OptionalControllerDrawV1)
    );
    let mine = include_str!(
        "../../../tests/fixtures/cards-data-d2c/d26b27db-a567-4631-b4b6-7294222fbdd1.json"
    );
    assert!(matches!(
        compile(MANIFEST, mine),
        CompileResult::Lowered { .. }
    ));
    let mut bad = catalog().definitions.remove(0);
    bad.triggered[0].functioning_zone = FunctioningZone::Stack;
    assert!(bad.validate().is_err());
}

#[test]
fn d2c_unsupported_each_opponent_draw_rejects_complete_source_and_ir() {
    let raw = include_str!(
        "../../../tests/fixtures/cards-data-d2c/01dc86f3-5ebb-4b10-bf68-8fc9f232c724.json"
    );
    assert!(
        matches!(compile(MANIFEST,raw),CompileResult::Unsupported {reason,..} if reason=="unsupported each-opponent draw")
    );
    let mut d = catalog()
        .definitions
        .into_iter()
        .find(|d| d.name == "Phyrexian Arena")
        .unwrap();
    if let Instruction::DrawCards { recipient, .. } = &mut d.triggered[0].program.instructions[0] {
        *recipient = Some(LifeRecipient::EachOpponent);
    }
    d.capabilities = d.requirements();
    assert!(d.validate().is_err());
}
