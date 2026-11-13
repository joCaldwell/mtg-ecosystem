//! Exact complete archive rows plus explicitly synthetic negative mutations.
use super::*;
const MANIFEST: &str =
    include_str!("../../../tests/fixtures/cards-data-d2cd/archive-manifest-current.json");
const CAUSE: &str = include_str!(
    "../../../tests/fixtures/cards-data-d2cd/218f835b-4195-4ee8-a4ef-e5ba58fa5374.json"
);
const REGISAUR: &str = include_str!(
    "../../../tests/fixtures/cards-data-d2cd/3d563149-7d4e-43e5-b701-28e3212051ec.json"
);
const HELLKITE: &str = include_str!(
    "../../../tests/fixtures/cards-data-d2cd/27596982-f4ab-488f-a974-5b0404438504.json"
);
fn lower(raw: &str) -> Definition {
    match compile(MANIFEST, raw) {
        CompileResult::Lowered { definition } => *definition,
        other => panic!("{other:?}"),
    }
}
fn changed(raw: &str, f: impl FnOnce(&mut serde_json::Value)) -> String {
    tests::changed_source(raw, |r| {
        f(&mut r["faces"][0]["result"]["abilities"][0]);
        r["faces"][0]["result"]["lines"][0]["ability"] =
            r["faces"][0]["result"]["abilities"][0].clone();
    })
}
#[test]
fn d2cd_archive_programs_and_explicit_whole_card_negatives() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/cards-data-d2cd");
    let provenance: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("provenance.json")).unwrap())
            .unwrap();
    for row in provenance["records"].as_array().unwrap() {
        let raw = std::fs::read_to_string(root.join(row["file"].as_str().unwrap())).unwrap();
        let positive = [
            1408, 5072, 5815, 5904, 9239, 10788, 13375, 21257, 21408, 22914, 23644, 27705, 28258,
            30695, 34896, 35516, 37667,
        ]
        .contains(&row["archive_line"].as_u64().unwrap());
        if !positive {
            assert!(
                matches!(compile(MANIFEST, &raw), CompileResult::Unsupported { .. }),
                "{}",
                row["file"]
            );
            continue;
        }
        let d = lower(&raw);
        assert!(matches!(
            d.triggered[0].condition,
            TriggerCondition::CreatureAttacked { .. }
        ));
        assert_eq!(d.capabilities, d.requirements());
        assert!(d.capabilities.contains(&Capability::AttackTriggerV1));
        let mut bad = d.clone();
        bad.capabilities.remove(&Capability::AttackTriggerV1);
        assert!(bad.validate().is_err());
        Catalog {
            schema_version: mtg_contracts::CURRENT_IR_VERSION,
            rules_edition: RULES_EDITION.into(),
            rules_sha256: RULES_SHA256.into(),
            definitions: vec![d.clone()],
        }
        .validate()
        .unwrap();
    }
    // draw two is already a sequence of singleton draws, not a runtime count2.
    assert!(
        matches!(lower(HELLKITE).triggered[0].program.instructions.as_slice(),[Instruction::DrawCards {count:a,..},Instruction::DrawCards {count:b,..}] if *a==1.into() && *b==1.into())
    );
    assert!(matches!(
        lower(REGISAUR).triggered[0].program.instructions.as_slice(),
        [Instruction::SacrificeDamage {
            optional: false,
            ..
        }]
    ));
}
#[test]
fn d2cd_synthetic_relations_and_compositional_full_type_membership() {
    for (control, relation) in [
        (None, AttackerRelation::AnyPlayer),
        (Some("you"), AttackerRelation::Controller),
        (Some("opponent"), AttackerRelation::Opponent),
    ] {
        for (filter, expected) in [
            (
                serde_json::json!({"types":["artifact","creature"]}),
                [false, true, false],
            ),
            (
                serde_json::json!({"types":["creature"],"anyOf":[{"types":["artifact"]},{"types":["enchantment"]}]}),
                [false, true, true],
            ),
            (
                serde_json::json!({"allOf":[{"types":["creature"]},{"nonTypes":["artifact"]}]}),
                [true, false, true],
            ),
        ] {
            let raw = changed(CAUSE, |a| {
                a["trigger"]["what"]["filter"] = filter;
                if let Some(c) = control {
                    a["trigger"]["what"]["filter"]["control"] = c.into();
                }
            });
            let d = lower(&raw);
            let TriggerCondition::CreatureAttacked {
                attacker: AttackSubject::Filter { controller, types },
            } = &d.triggered[0].condition
            else {
                panic!("filter")
            };
            assert_eq!(*controller, relation);
            for (extra, expected) in [
                vec![],
                vec![CardKind::Artifact],
                vec![CardKind::Enchantment],
            ]
            .into_iter()
            .zip(expected)
            {
                let mut c = lower(REGISAUR).characteristics;
                c.additional_types = extra;
                assert_eq!(types.matches(&c), expected);
            }
        }
    }
    // An attacker-pronoun damage instruction cannot silently become damage
    // from a distinct observing enchantment/creature. Exact self is required.
    let raw = changed(
        REGISAUR,
        |a| a["trigger"]["what"] = serde_json::json!({"ref":"filter","filter":{"types":["creature"],"control":"you"},"count":{"amount":"fixed","value":1}}),
    );
    assert!(matches!(
        compile(MANIFEST, &raw),
        CompileResult::Unsupported { .. }
    ));
}
#[test]
fn d2cd_synthetic_unknown_qualifiers_grouping_history_zones_and_recipients_reject() {
    let mut mutations = vec![];
    for (field, value) in [
        ("alone", serde_json::json!(true)),
        ("alongside", serde_json::json!({"types":["creature"]})),
        ("whom", serde_json::json!({"player":"you"})),
        ("blocked", serde_json::json!(false)),
        ("during", serde_json::json!("your")),
        ("costs", serde_json::json!([])),
        (
            "occurrence",
            serde_json::json!({"ordinal":1,"period":"turn"}),
        ),
        ("unknown", serde_json::json!(true)),
    ] {
        mutations.push(changed(CAUSE, |a| a["trigger"][field] = value));
    }
    for (field, value) in [
        ("status", serde_json::json!(["attacking"])),
        ("subtypes", serde_json::json!(["Human"])),
        ("supertypes", serde_json::json!(["legendary"])),
        ("colors", serde_json::json!(["red"])),
        ("own", serde_json::json!(true)),
        ("other", serde_json::json!(true)),
        ("nonToken", serde_json::json!(true)),
        ("control", serde_json::json!("owner")),
        ("cls", serde_json::json!("permanent")),
        ("anyOf", serde_json::json!([])),
        (
            "allOf",
            serde_json::json!([{"types":["creature"],"status":["tapped"]}]),
        ),
    ] {
        mutations.push(changed(CAUSE, |a| {
            a["trigger"]["what"]["filter"][field] = value
        }));
    }
    for trigger in [
        "player-attacks",
        "attacks-or-blocks",
        "becomes-blocked",
        "attacks-and-isnt-blocked",
    ] {
        mutations.push(changed(CAUSE, |a| a["trigger"]["trigger"] = trigger.into()));
    }
    mutations.push(changed(CAUSE, |a| {
        a["trigger"].as_object_mut().unwrap().remove("what");
    }));
    mutations.push(changed(CAUSE, |a| {
        a["trigger"]["what"]["count"] = serde_json::json!({"amount":"one-or-more"})
    }));
    mutations.push(changed(CAUSE,|a|a["condition"]=serde_json::json!({"condition":"status","what":{"ref":"this","noun":"enchantment"},"status":"tapped"})));
    mutations.push(changed(CAUSE, |a| {
        a["limit"] = serde_json::json!({"maximum":1,"period":"turn"})
    }));
    mutations.push(changed(CAUSE, |a| {
        a["functioningZone"] = "graveyard".into()
    }));
    mutations.push(changed(CAUSE, |a| {
        a["effects"][0]["steps"][0]["who"] = serde_json::json!({"player":"defending-player"})
    }));
    mutations.push(changed(CAUSE, |a| {
        a["effects"][0]["steps"][0]["who"] = serde_json::json!({"player":"that-player"})
    }));
    mutations.push(changed(CAUSE, |a| {
        a["effects"][0]["steps"][0]["optional"] = true.into()
    }));
    for raw in mutations {
        assert!(
            matches!(compile(MANIFEST, &raw), CompileResult::Unsupported { .. }),
            "{:?}",
            compile(MANIFEST, &raw)
        );
    }
    for raw in [
        serde_json::json!({"subject":"source","unknown":true}),
        serde_json::json!({"subject":"filter","controller":"any_player","types":{"predicate":"any"},"status":"attacking"}),
    ] {
        assert!(serde_json::from_value::<AttackSubject>(raw).is_err());
    }
    let mut d = lower(CAUSE);
    d.triggered[0].program.instructions[0] = Instruction::GainLife {
        recipient: LifeRecipient::EventPlayer,
        amount: 1.into(),
        source_block: 0,
    };
    d.capabilities = d.requirements();
    assert!(d.validate().is_err());
}
