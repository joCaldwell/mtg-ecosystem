//! Source-only candidate tests. Complete bounded archive rows, never expected IR.
use super::*;
const MANIFEST: &str =
    include_str!("../../../tests/fixtures/cards-data-d2ce/archive-manifest-current.json");
const FOX: &str = include_str!(
    "../../../tests/fixtures/cards-data-d2ce/51f4eb1f-abf2-425a-b78e-cafac177da89.json"
);
fn rows() -> Vec<(serde_json::Value, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/cards-data-d2ce");
    let p: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("provenance.json")).unwrap())
            .unwrap();
    p["records"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r.clone(),
                std::fs::read_to_string(root.join(r["file"].as_str().unwrap())).unwrap(),
            )
        })
        .collect()
}
fn lower(raw: &str) -> Definition {
    match compile(MANIFEST, raw) {
        CompileResult::Lowered { definition } => *definition,
        other => panic!("{other:?}"),
    }
}
fn changed(f: impl FnOnce(&mut serde_json::Value)) -> String {
    tests::changed_source(FOX, |r| {
        let a = &mut r["faces"][0]["result"]["abilities"][0];
        f(a);
        r["faces"][0]["result"]["lines"][0]["ability"] =
            r["faces"][0]["result"]["abilities"][0].clone();
    })
}
#[test]
fn d2ce_complete_archive_rows_provenance_programs_and_whole_card_negatives() {
    let mut admitted = 0;
    for (row, raw) in rows() {
        assert_eq!(sha256(raw.trim_end().as_bytes()), row["record_sha256"]);
        let source: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(source["source"]["legalities"]["commander"], "legal");
        assert_eq!(source["sourceSha256"], row["source_sha256"]);
        assert_eq!(source["astSha256"], row["ast_sha256"]);
        if ![4538, 9081, 12306, 14941, 19442, 21636, 28189]
            .contains(&row["archive_line"].as_u64().unwrap())
        {
            assert!(
                matches!(compile(MANIFEST, &raw), CompileResult::Unsupported { .. }),
                "{}: {:?}",
                source["source"]["name"],
                compile(MANIFEST, &raw)
            );
            continue;
        }
        admitted += 1;
        let d = lower(&raw);
        assert_eq!(d.triggered.len(), 1);
        assert!(matches!(
            d.triggered[0].condition,
            TriggerCondition::DamageDealt {
                source: DamageTriggerSource::Source {},
                kind: TriggerDamageKind::Combat,
                recipient: DamageTriggerRecipient::Player,
            }
        ));
        assert_eq!(d.capabilities, d.requirements());
        assert!(d.capabilities.contains(&Capability::DamageTriggerV1));
        assert_eq!(d.provenance.source_sha256, row["source_sha256"]);
        assert_eq!(d.provenance.ast_sha256, row["ast_sha256"]);
        let mut bad = d.clone();
        bad.capabilities.remove(&Capability::DamageTriggerV1);
        assert!(bad.validate().is_err());
        for version in [24, 25, 26] {
            let c = Catalog {
                schema_version: version,
                rules_edition: RULES_EDITION.into(),
                rules_sha256: RULES_SHA256.into(),
                definitions: vec![d.clone()],
            };
            assert_eq!(c.validate().is_ok(), version == 25, "{} v{version}", d.name);
        }
    }
    assert_eq!(admitted, 7);
}
#[test]
fn d2ce_rejects_partial_kind_source_recipient_optional_quantity_and_condition_shapes() {
    let mut mutations = vec![];
    for v in [
        serde_json::json!(false),
        serde_json::json!(null),
        serde_json::json!("true"),
    ] {
        mutations.push(changed(|a| a["trigger"]["combat"] = v));
    }
    mutations.push(changed(|a| {
        a["trigger"].as_object_mut().unwrap().remove("combat");
    }));
    for v in [
        serde_json::json!({"player":"an-opponent"}),
        serde_json::json!({"player":"a-player"}),
        serde_json::json!({"ref":"this","noun":"creature"}),
        serde_json::json!(null),
    ] {
        mutations.push(changed(|a| a["trigger"]["to"] = v));
    }
    for v in [
        serde_json::json!({"ref":"it"}),
        serde_json::json!({"ref":"filter","filter":{"types":["creature"]},"count":{"amount":"fixed","value":1}}),
        serde_json::json!({"ref":"this","noun":"permanent"}),
        serde_json::json!({"ref":"this","noun":"creature","extra":true}),
    ] {
        mutations.push(changed(|a| a["trigger"]["what"] = v));
    }
    for (k, v) in [
        ("grouping", serde_json::json!("one-or-more")),
        ("unknown", serde_json::json!(true)),
        ("amount", serde_json::json!({"amount":"fixed","value":2})),
        (
            "occurrence",
            serde_json::json!({"ordinal":1,"period":"turn"}),
        ),
    ] {
        mutations.push(changed(|a| a["trigger"][k] = v));
    }
    mutations.push(changed(|a| {
        a["effects"][0]["steps"][0]["optional"] = true.into()
    }));
    mutations.push(changed(|a| {
        a["effects"][0]["steps"][0]["amount"] = serde_json::json!({"amount":"that-much"})
    }));
    mutations.push(changed(|a| {
        a["effects"][0]["steps"][0]["who"] = serde_json::json!({"player":"that-player"})
    }));
    mutations.push(changed(|a| a["condition"] = serde_json::json!({"condition":"status","what":{"ref":"this","noun":"creature"},"status":"tapped"})));
    mutations.push(changed(|a| a["functioningZone"] = "graveyard".into()));
    for raw in mutations {
        assert!(
            matches!(compile(MANIFEST, &raw), CompileResult::Unsupported { .. }),
            "{:?}",
            compile(MANIFEST, &raw)
        );
    }
    // Source-name canonicalization requires the exact archive binding. These
    // are explicitly synthetic source/binding mutations, not archived cards.
    let unbound = changed(|a| a["trigger"]["what"] = serde_json::json!({"ref":"self"}));
    assert!(matches!(
        compile(MANIFEST, &unbound),
        CompileResult::Unsupported { .. }
    ));
    let bound = tests::changed_source(&unbound, |r| {
        let reference = r["binding"]["references"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|r| r["path"] == "/faces/0/abilities/0/trigger/what")
            .unwrap();
        reference["expression"] = serde_json::json!({"ref":"self"});
    });
    assert!(
        lower(&bound)
            .capabilities
            .contains(&Capability::DamageTriggerV1)
    );
}
#[test]
fn d2ce_ir_strict_dimensions_mandatory_program_and_minimum_version() {
    let d = lower(FOX);
    for condition in [
        serde_json::json!({"damage_dealt":{"source":{"subject":"source","extra":true},"kind":"combat","recipient":"player"}}),
        serde_json::json!({"damage_dealt":{"source":{"subject":"filter"},"kind":"combat","recipient":"player"}}),
        serde_json::json!({"damage_dealt":{"source":{"subject":"source"},"kind":"noncombat","recipient":"player"}}),
        serde_json::json!({"damage_dealt":{"source":{"subject":"source"},"kind":true,"recipient":"player"}}),
        serde_json::json!({"damage_dealt":{"source":{"subject":"source"},"kind":"combat","recipient":"opponent"}}),
        serde_json::json!({"damage_dealt":{"source":{"subject":"source"},"kind":"combat","recipient":"player","unknown":1}}),
    ] {
        assert!(serde_json::from_value::<TriggerCondition>(condition).is_err());
    }
    let mut event = d.clone();
    if let Instruction::DrawCards { recipient, .. } =
        &mut event.triggered[0].program.instructions[0]
    {
        *recipient = Some(LifeRecipient::EventPlayer);
    } else {
        panic!("draw");
    }
    event.capabilities = event.requirements();
    assert!(event.validate().is_err());
    let mut optional = d.clone();
    optional.triggered[0].intervening_if = Some(InterveningCondition::ControllerHandEmpty);
    optional.capabilities = optional.requirements();
    assert!(optional.validate().is_err());
}
