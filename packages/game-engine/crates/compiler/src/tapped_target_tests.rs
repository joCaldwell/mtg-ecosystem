//! Complete retained tapped-target sources plus synthetic scope-consumption probes.
use super::*;
use serde_json::{Value, json};
const MANIFEST: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
const SWIFT: &str =
    include_str!("../../../tests/fixtures/5820d760-8ed7-4abf-a959-c06ed86a3730.json");
const ASSASSINATE: &str =
    include_str!("../../../tests/fixtures/ce979f36-84f2-4419-b400-971263494cc6.json");
fn changed_target(text: &str, mutate: impl FnOnce(&mut Value)) -> String {
    tests::changed_source(SWIFT, |record| {
        record["source"]["oracle_text"] = text.into();
        let result = &mut record["faces"][0]["result"];
        let target = &mut result["abilities"][0]["effects"][0]["steps"][0]["what"];
        mutate(target);
        let target = target.clone();
        result["lines"][0]["text"] = text.into();
        result["lines"][0]["ability"] = result["abilities"][0].clone();
        let path = "/faces/0/abilities/0/effects/0/steps/0/what";
        for node in record["binding"]["nodes"].as_array_mut().unwrap() {
            if node["node"] == "selection" && node["path"] == path {
                node["expression"] = target.clone();
            }
        }
        for reference in record["binding"]["references"].as_array_mut().unwrap() {
            if reference["path"] == path {
                reference["expression"] = target.clone();
            }
        }
    })
}
#[test]
fn complete_tapped_creature_spells_preserve_source_and_current_target_predicate() {
    for (raw, name, record_hash, index, timing, generic, color) in [
        (
            SWIFT,
            "Swift Response",
            "846775a53caafd4ba4de5cea84b944109805e45670b3c390463b58ff25f5675e",
            13229,
            SpellTiming::Instant,
            1,
            Color::W,
        ),
        (
            ASSASSINATE,
            "Assassinate",
            "07e35ab6d6ec3c8bc9b1114d166992f7a9c701a63805ec75042aba1e18a80506",
            31146,
            SpellTiming::MainPhase,
            2,
            Color::B,
        ),
    ] {
        assert_eq!(sha256(raw.trim_end().as_bytes()), record_hash);
        let record: Value = strict_read(raw).unwrap();
        assert_eq!(
            record["source"]["oracle_text"],
            "Destroy target tapped creature."
        );
        let CompileResult::Lowered { definition } = compile(MANIFEST, raw) else {
            panic!("{name}: complete source")
        };
        assert_eq!(definition.name, name);
        assert_eq!(definition.provenance.source_index, index);
        assert_eq!(
            definition.oracle_id,
            record["oracleIdentity"].as_str().unwrap()
        );
        assert_eq!(
            definition.provenance.source_sha256,
            record["sourceSha256"].as_str().unwrap()
        );
        assert_eq!(
            definition.provenance.ast_sha256,
            "afc7f92d512e7e935d2446583f2d4a2d920c9eb8b32c1d8f946b1c60281be094"
        );
        assert_eq!(
            definition.provenance.binding_sha256,
            record["bindingSha256"].as_str().unwrap()
        );
        assert_eq!(
            strict_read::<Value>(&definition.provenance.source_json).unwrap(),
            record["source"]
        );
        assert_eq!(
            strict_read::<Value>(&definition.provenance.abilities_json).unwrap(),
            record["faces"][0]["result"]["abilities"]
        );
        assert_eq!(
            definition.characteristics.mana_cost,
            ManaCost {
                generic: Quantity::from(generic),
                colored: vec![color]
            }
        );
        assert_eq!(
            definition.characteristics.kind,
            if timing == SpellTiming::Instant {
                CardKind::Instant
            } else {
                CardKind::Sorcery
            }
        );
        assert!(
            definition.activated.is_empty()
                && definition.triggered.is_empty()
                && definition.static_abilities.is_empty()
        );
        assert_eq!(
            definition.spell,
            Some(SpellProgram {
                additional_sacrifice: None,
                timing,
                resolution: SpellResolution::Instructions,
                functioning_zone: FunctioningZone::Stack,
                modes: vec![],
                targets: vec![TargetSpec {
                    slot: 0,
                    selection_id: "line:0:1".into(),
                    chooser: Chooser::Controller,
                    stage: EvaluationStage::Announcement,
                    domain: TargetDomain::BattlefieldPermanent,
                    filter: Some(PermanentFilter {
                        types: vec![CardKind::Creature],
                        non_types: vec![],
                        non_colors: vec![],
                        opponent: false,
                        tapped: true
                    })
                }],
                instructions: vec![Instruction::DestroyPermanent {
                    target_slot: 0,
                    optional: false,
                    regeneration_forbidden: false,
                    source_block: 0
                }],
            })
        );
        assert_eq!(
            definition.capabilities,
            BTreeSet::from([
                Capability::NormalLayoutV1,
                Capability::PermanentRemovalV1,
                Capability::TappedPermanentTargetV1,
                if timing == SpellTiming::Instant {
                    Capability::CastInstantV1
                } else {
                    Capability::CastSorceryV1
                }
            ])
        );
        definition.validate().unwrap();
    }
}
#[test]
fn tapped_target_lowering_rejects_unrepresented_statuses_and_target_qualifiers() {
    for status in [
        Value::Null,
        json!([]),
        json!(["untapped"]),
        json!(["blocking"]),
        json!(["tapped", "attacking"]),
        json!(["tapped", "tapped"]),
        json!("tapped"),
        json!(true),
    ] {
        let mutant = changed_target("Destroy target tapped creature.", |target| {
            target["filter"]["status"] = status
        });
        let result = compile(MANIFEST, &mutant);
        assert!(
            matches!(result, CompileResult::Unsupported { ref path, .. } if path == "filter.status"),
            "{result:?}"
        );
    }
    for (text, field, value) in [
        (
            "Destroy target tapped creature you own.",
            "owner",
            json!("you"),
        ),
        (
            "Destroy target tapped creature you control.",
            "control",
            json!("you"),
        ),
        (
            "Destroy target tapped Human creature.",
            "subtypes",
            json!(["Human"]),
        ),
        (
            "Destroy target tapped artifact creature.",
            "types",
            json!(["artifact", "creature"]),
        ),
    ] {
        let mutant = changed_target(text, |target| target["filter"][field] = value);
        assert!(
            matches!(
                compile(MANIFEST, &mutant),
                CompileResult::Unsupported { .. }
            ),
            "{text}"
        );
    }
    for (field, value) in [
        ("count", json!({"amount":"fixed","value":2})),
        ("count", json!({"amount":"fixed","value":0})),
        ("ref", json!("each")),
        ("selectionId", json!("")),
        ("of", json!({"ref":"this","noun":"creature"})),
    ] {
        let mutant = changed_target("Destroy target tapped creature.", |target| {
            target[field] = value
        });
        assert!(
            matches!(
                compile(MANIFEST, &mutant),
                CompileResult::Unsupported { .. }
            ),
            "{field}"
        );
    }
}
#[test]
fn tapped_target_composes_only_with_represented_filters_and_cannot_overwrite_reused_slot() {
    let mutant = changed_target(
        "Destroy target tapped nonartifact, nonblack creature an opponent controls.",
        |target| {
            target["filter"]["nonTypes"] = json!(["artifact"]);
            target["filter"]["nonColors"] = json!(["black"]);
            target["filter"]["control"] = json!("opponent");
        },
    );
    let CompileResult::Lowered { definition } = compile(MANIFEST, &mutant) else {
        panic!("complete represented filter composition")
    };
    assert_eq!(
        definition.spell.as_ref().unwrap().targets[0].filter,
        Some(PermanentFilter {
            types: vec![CardKind::Creature],
            non_types: vec![CardKind::Artifact],
            non_colors: vec![Color::B],
            opponent: true,
            tapped: true,
        })
    );
    let record: Value = strict_read(SWIFT).unwrap();
    let target =
        record["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["what"].clone();
    let mut program = definition.spell.unwrap();
    program.targets.clear();
    program.instructions.clear();
    let mut slots = BTreeMap::new();
    assert_eq!(
        bind_permanent_target(&mut program, &mut slots, &target).unwrap(),
        0
    );
    let before = program.targets[0].clone();
    let mut relaxed = target.clone();
    relaxed["filter"].as_object_mut().unwrap().remove("status");
    assert!(bind_permanent_target(&mut program, &mut slots, &relaxed).is_err());
    assert_eq!(program.targets[0], before);
    // Existing unqualified permanent removal retains its complete definition;
    // the omitted source status means no restriction and derives no new label.
    let catalog = embedded_catalog("removal").unwrap();
    let raw = include_str!("../../../tests/fixtures/disenchant.json");
    let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, raw) else {
        panic!("original Disenchant")
    };
    assert_eq!(
        catalog.definition_by_oracle_id(&definition.oracle_id),
        Some(&*definition)
    );
    assert!(
        !definition.spell.as_ref().unwrap().targets[0]
            .filter
            .as_ref()
            .unwrap()
            .tapped
    );
    assert!(
        !definition
            .capabilities
            .contains(&Capability::TappedPermanentTargetV1)
    );
}
