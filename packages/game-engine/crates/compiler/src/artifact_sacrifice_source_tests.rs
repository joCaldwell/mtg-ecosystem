//! Complete retained artifact-sacrifice source plus synthetic rejection probes.
use super::*;
use serde_json::{Value, json};

const MANIFEST: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
const SHRAPNEL: &str =
    include_str!("../../../tests/fixtures/d3c623f3-c7c9-47c0-8096-6dee3267baff.json");

fn changed(cost_text: &str, damage_text: &str, mutate: impl FnOnce(&mut Value)) -> String {
    tests::changed_source(SHRAPNEL, |record| {
        record["source"]["oracle_text"] = format!("{cost_text}\n{damage_text}").into();
        mutate(&mut record["faces"][0]["result"]["abilities"]);
        let text = source_lines(
            record["source"]["oracle_text"].as_str().unwrap(),
            "Shrapnel Blast",
            "Instant",
        )
        .unwrap();
        let result = &mut record["faces"][0]["result"];
        for (i, line) in text.into_iter().enumerate() {
            result["lines"][i]["text"] = line.into();
            result["lines"][i]["ability"] = result["abilities"][i].clone();
        }
        let cost = result["abilities"][0]["costs"][0]["what"].clone();
        let source = result["abilities"][1]["effects"][0]["steps"][0]["source"].clone();
        let cost_path = "/faces/0/abilities/0/costs/0/what";
        let source_path = "/faces/0/abilities/1/effects/0/steps/0/source";
        let cost_id = format!("card:d3c623f3-c7c9-47c0-8096-6dee3267baff{cost_path}#query");
        for node in record["binding"]["nodes"].as_array_mut().unwrap() {
            if node["path"] == cost_path && node["node"] == "selection" {
                node["expression"] = cost.clone();
            }
            if source == json!({"ref":"it"})
                && node["node"] == "event"
                && node["operation"] == "damage"
            {
                node["inputs"][0] = cost_id.clone().into();
            }
        }
        for reference in record["binding"]["references"].as_array_mut().unwrap() {
            if reference["path"] == cost_path {
                reference["expression"] = cost.clone();
            }
            if reference["path"] == source_path {
                reference["expression"] = source.clone();
                if source == json!({"ref":"it"}) {
                    reference["target"] = cost_id.clone().into();
                }
            }
        }
    })
}

#[test]
fn complete_shrapnel_blast_lowers_artifact_cost_and_fixed_self_damage_without_omission() {
    assert_eq!(
        sha256(SHRAPNEL.trim_end().as_bytes()),
        "deef5ed88d2210422b321244abac071288309e377d744e501611c6571de01d45"
    );
    let raw: Value = strict_read(SHRAPNEL).unwrap();
    assert_eq!(
        raw["source"]["oracle_text"],
        "As an additional cost to cast this spell, sacrifice an artifact.\nShrapnel Blast deals 5 damage to any target."
    );
    assert_eq!(raw["binding"]["fullyBound"], true);
    let result = compile(MANIFEST, SHRAPNEL);
    let CompileResult::Lowered { definition: d } = result else {
        panic!("{result:?}")
    };
    assert_eq!(d.name, "Shrapnel Blast");
    assert_eq!(d.oracle_id, "d3c623f3-c7c9-47c0-8096-6dee3267baff");
    assert_eq!(d.characteristics.kind, CardKind::Instant);
    assert_eq!(
        d.characteristics.mana_cost,
        ManaCost {
            generic: 1.into(),
            colored: vec![Color::R]
        }
    );
    assert_eq!(d.provenance.source_index, 31965);
    assert_eq!(
        d.provenance.source_sha256,
        "193d679c7e8f8cfa8d8b7fd18b9bbfa3d9a5a33e53cd04dc20f6e31ca6908e9e"
    );
    assert_eq!(
        d.provenance.ast_sha256,
        "a92cb719516aa466cf5877a0f0437cd48b0a29474d3da5a90f10e2b0052538d0"
    );
    assert_eq!(
        d.provenance.binding_sha256,
        "963d11a1e3d02ffd4cc86e01d59c674c970d66564c7a5c18efa54fed24e781a8"
    );
    assert_eq!(
        serde_json::from_str::<Value>(&d.provenance.source_json).unwrap(),
        raw["source"]
    );
    assert_eq!(
        serde_json::from_str::<Value>(&d.provenance.abilities_json).unwrap(),
        raw["faces"][0]["result"]["abilities"]
    );
    assert!(d.activated.is_empty() && d.triggered.is_empty() && d.static_abilities.is_empty());
    assert_eq!(
        d.spell,
        Some(SpellProgram {
            additional_sacrifice: Some(SacrificeFilter {
                types: vec![CardKind::Artifact],
                subtypes: vec![]
            }),
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            functioning_zone: FunctioningZone::Stack,
            modes: vec![],
            targets: vec![TargetSpec {
                slot: 0,
                selection_id: "line:1:6".into(),
                chooser: Chooser::Controller,
                stage: EvaluationStage::Announcement,
                domain: TargetDomain::AnyDamageTarget,
                filter: None
            }],
            instructions: vec![Instruction::DealDamage {
                target_slot: 0,
                amount: 5.into(),
                source_block: 1
            }],
        })
    );
    assert_eq!(
        d.capabilities,
        BTreeSet::from([
            Capability::NormalLayoutV1,
            Capability::CastInstantV1,
            Capability::SacrificeV1,
            Capability::AnyDamageTargetV1,
            Capability::NoncombatDamageV1
        ])
    );
    d.validate().unwrap();
}

#[test]
fn artifact_sacrifice_source_rejects_unconsumed_cost_cardinality_and_qualifiers() {
    for (field, value) in [
        ("count", json!({"amount":"fixed","value":0})),
        ("count", json!({"amount":"fixed","value":2})),
        ("count", json!({"amount":"X"})),
        ("ref", json!("all")),
        ("selectionId", json!("cost-selection")),
        ("other", json!(true)),
    ] {
        let raw = changed(
            "As an additional cost to cast this spell, sacrifice an artifact.",
            "Shrapnel Blast deals 5 damage to any target.",
            |a| a[0]["costs"][0]["what"][field] = value,
        );
        let result = compile(MANIFEST, &raw);
        assert!(
            matches!(result, CompileResult::Unsupported { .. }),
            "{field}: {result:?}"
        );
    }
    for filter in [
        json!({"types":["artifact"],"control":"you"}),
        json!({"types":["artifact"],"control":"opponent"}),
        json!({"types":["artifact"],"status":["tapped"]}),
        json!({"types":["artifact"],"owner":"you"}),
        json!({"types":["artifact","creature"]}),
        json!({"types":["artifact"],"unknown":true}),
        json!({"types":["artifact"],"subtypes":["Myr"]}),
        Value::Null,
    ] {
        let raw = changed(
            "As an additional cost to cast this spell, sacrifice a qualified artifact.",
            "Shrapnel Blast deals 5 damage to any target.",
            |a| a[0]["costs"][0]["what"]["filter"] = filter,
        );
        let result = compile(MANIFEST, &raw);
        assert!(
            matches!(result, CompileResult::Unsupported { .. }),
            "{result:?}"
        );
    }
    let raw = changed(
        "As an additional cost to cast this spell, sacrifice an artifact.",
        "Shrapnel Blast deals 5 damage to any target.",
        |a| a[0]["costs"] = json!([]),
    );
    assert!(matches!(
        compile(MANIFEST, &raw),
        CompileResult::Unsupported { .. }
    ));
    let missing = tests::changed_source(SHRAPNEL, |record| {
        // Preserve the printed additional-cost clause and its line. Removing
        // its aggregate AST block must fail whole-source accounting.
        record["faces"][0]["result"]["abilities"]
            .as_array_mut()
            .unwrap()
            .remove(0);
        // Reindex the retained damage source proof so this specifically
        // reaches whole-source line accounting, rather than an earlier
        // unbound self-reference rejection.
        for reference in record["binding"]["references"].as_array_mut().unwrap() {
            if reference["path"] == "/faces/0/abilities/1/effects/0/steps/0/source" {
                reference["path"] = "/faces/0/abilities/0/effects/0/steps/0/source".into();
            }
        }
    });
    assert!(
        matches!(compile(MANIFEST, &missing), CompileResult::Invalid { ref path, .. } if path == "lines")
    );
}

#[test]
fn artifact_sacrifice_source_keeps_damage_quantity_and_source_proofs_strict() {
    for amount in [
        json!({"amount":"fixed","value":0}),
        json!({"amount":"X"}),
        json!({"amount":"attribute","of":{"ref":"related","number":"singular","filter":{"types":["creature"]},"relation":{"relation":"event-result","event":"sacrifice","scope":"contextual"}},"attribute":"power"}),
    ] {
        let raw = changed(
            "As an additional cost to cast this spell, sacrifice an artifact.",
            "Shrapnel Blast deals a synthetic quantity of damage to any target.",
            |a| a[1]["effects"][0]["steps"][0]["amount"] = amount,
        );
        let result = compile(MANIFEST, &raw);
        assert!(
            matches!(result, CompileResult::Unsupported { .. }),
            "{result:?}"
        );
    }
    let raw = changed(
        "As an additional cost to cast this spell, sacrifice an artifact.",
        "It deals 5 damage to any target.",
        |a| a[1]["effects"][0]["steps"][0]["source"] = json!({"ref":"it"}),
    );
    let probe: Value = strict_read(&raw).unwrap();
    let reference = probe["binding"]["references"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["path"] == "/faces/0/abilities/1/effects/0/steps/0/source")
        .unwrap();
    assert_eq!(reference["expression"], json!({"ref":"it"}));
    assert!(
        reference["target"]
            .as_str()
            .unwrap()
            .ends_with("/costs/0/what#query")
    );
    let result = compile(MANIFEST, &raw);
    assert!(
        // The shared ordinary-spell guard uses damage.source (activations use
        // abilities.effects.source). Its semantic rejection must survive even
        // a diagnostic graph that binds It to the sacrificed-object query.
        matches!(result, CompileResult::Unsupported { ref path, ref reason }
            if path == "damage.source" && reason == "a spell must refer to its own spell source"),
        "{result:?}"
    );
    let diagnostic_changed = tests::changed_source(SHRAPNEL, |record| {
        record["binding"]["references"]
            .as_array_mut()
            .unwrap()
            .retain(|r| r["path"] != "/faces/0/abilities/1/effects/0/steps/0/source");
    });
    // Exact damage-source Self is an unambiguous AST operator. Its diagnostic
    // reference is not executable authority (self_reference.rs); a validly
    // hashed diagnostic change must preserve the semantic program and caps.
    let result = compile(MANIFEST, &diagnostic_changed);
    let CompileResult::Lowered {
        definition: changed,
    } = result
    else {
        panic!("{result:?}")
    };
    let CompileResult::Lowered {
        definition: baseline,
    } = compile(MANIFEST, SHRAPNEL)
    else {
        panic!("complete source")
    };
    assert_eq!(changed.characteristics, baseline.characteristics);
    assert_eq!(changed.spell, baseline.spell);
    assert_eq!(changed.capabilities, baseline.capabilities);

    // The graph still participates in archive integrity. Change only its hash
    // back, after the helper has rehashed every compact source/AST value, so
    // the rejection cannot be confused with a source serialization mismatch.
    let mut stale: Value = strict_read(&diagnostic_changed).unwrap();
    let original: Value = strict_read(SHRAPNEL).unwrap();
    assert_ne!(stale["bindingSha256"], original["bindingSha256"]);
    stale["bindingSha256"] = original["bindingSha256"].clone();
    let result = compile(MANIFEST, &canonical(&stale).unwrap());
    assert!(
        matches!(result, CompileResult::Invalid { ref path, ref reason }
            if path == "record" && reason == "source or diagnostic binding hash mismatch"),
        "{result:?}"
    );
}

#[test]
fn existing_complete_additional_sacrifice_definitions_remain_equal() {
    let c = embedded_catalog("sacrifice").unwrap();
    for (name, raw) in [
        (
            "Costly Plunder",
            include_str!("../../../tests/fixtures/costly-plunder.json"),
        ),
        ("Fling", include_str!("../../../tests/fixtures/fling.json")),
        (
            "Village Rites",
            include_str!("../../../tests/fixtures/village-rites.json"),
        ),
    ] {
        let result = compile(fixtures::MANIFEST, raw);
        let CompileResult::Lowered { definition } = result else {
            panic!("{name}: {result:?}")
        };
        assert_eq!(
            c.definitions
                .iter()
                .find(|d| d.oracle_id == definition.oracle_id),
            Some(definition.as_ref()),
            "{name}"
        );
    }
}
