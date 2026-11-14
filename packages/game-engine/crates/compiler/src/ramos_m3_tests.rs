//! Complete frozen M3 source lowering and strict shared cascade guards.
use super::*;
const MANIFEST: &str = include_str!("../../../tests/fixtures/ramos/archive-manifest-current.json");
fn complete(
    raw: &str,
    file_sha256: &str,
    source_sha256: &str,
    ast_sha256: &str,
    expected_path: &str,
) {
    assert_eq!(sha256(raw.as_bytes()), file_sha256);
    let record: serde_json::Value = strict_read(raw).unwrap();
    assert_eq!(record["sourceSha256"], source_sha256);
    assert_eq!(record["astSha256"], ast_sha256);
    assert_eq!(record["source"]["layout"], "normal");
    assert_eq!(record["binding"]["fullyBound"], true);
    let result = compile(MANIFEST, raw);
    let CompileResult::Lowered { definition } = result else {
        panic!("{expected_path}: {result:?}")
    };
    definition.validate().unwrap();
    assert_eq!(definition.triggered.len(), 1);
    assert_eq!(
        definition.triggered[0].condition,
        TriggerCondition::SelfSpellCast
    );
    assert_eq!(
        definition.triggered[0].functioning_zone,
        FunctioningZone::Stack
    );
    for capability in [
        Capability::CascadeV1,
        Capability::ResolutionCastV1,
        Capability::RandomBottomV1,
    ] {
        assert!(definition.capabilities.contains(&capability));
        let mut missing = definition.clone();
        missing.capabilities.remove(&capability);
        assert!(missing.validate().is_err());
    }
    let mut wrong = definition.clone();
    wrong.triggered[0].functioning_zone = FunctioningZone::Battlefield;
    assert!(wrong.validate().is_err());
    let mut repeated = definition.clone();
    repeated.triggered.push(repeated.triggered[0].clone());
    repeated.validate().unwrap(); // CR 702.85c, separate indexed occurrences.
    let mut wrong = definition.clone();
    wrong.triggered[0].condition = TriggerCondition::SelfEntersBattlefield;
    wrong.capabilities = wrong.requirements();
    assert!(wrong.validate().is_err());
    if definition.name == "Shardless Agent" {
        assert!(definition.characteristics.has_type(CardKind::Artifact));
        assert!(definition.characteristics.has_type(CardKind::Creature));
    }
    if definition.name == "Bloodbraid Elf" {
        assert!(definition.keywords.contains(&KeywordAbility::Haste));
    }
}
#[test]
fn ramos_m3_frozen_artifact_creature_agent_lowers_complete_stack_cascade() {
    let raw =
        include_str!("../../../tests/fixtures/ramos/2afbaa9a-c171-4a8b-90f3-5250d8498356.json");
    complete(
        raw,
        "343297e44e4d7a6229956fe0a26ea9196e7f116ad50d65947aa61b0c4194cc81",
        "5bb608651d0437646252f3561bdd1999822a5d2874ac0b44ed16421cd69b4ad3",
        "81e10bd7447faae0a18cf13c6f683c11009cfef9040a8f74833173181f20cbeb",
        "abilities[0].keywords[0]",
    );
    let record: serde_json::Value = strict_read(raw).unwrap();
    assert_eq!(
        record["source"]["type_line"],
        "Artifact Creature — Human Rogue"
    );
    assert_eq!(record["source"]["mana_cost"], "{1}{G}{U}");
    assert_eq!(record["source"]["keywords"], serde_json::json!(["Cascade"]));
}
#[test]
fn ramos_m3_frozen_bloodbraid_preserves_haste_and_cascade() {
    let raw =
        include_str!("../../../tests/fixtures/ramos/3f0c9466-5ab9-4205-a84f-b4b27b5a678e.json");
    complete(
        raw,
        "7ae2f1c71a9c786567f8f68fc9d69fef5d886808953098ae0559cf34c73bad52",
        "7b74156a9ae813c46183a139f7ad21b436e13c7c8645e807e7b15b6791a69d52",
        "1f82b2d96c86eeddcae912574ecaab8d17fa8a244f0a51e618c0a79dcce19df8",
        "abilities[1].keywords[0]",
    );
    let record: serde_json::Value = strict_read(raw).unwrap();
    assert_eq!(record["source"]["type_line"], "Creature — Elf Berserker");
    assert_eq!(record["source"]["mana_cost"], "{2}{R}{G}");
    assert_eq!(
        record["source"]["keywords"],
        serde_json::json!(["Haste", "Cascade"])
    );
}

#[test]
fn ramos_m3_missing_nonland_mana_cost_is_rejected_without_silently_becoming_zero() {
    // Synthetic source mutation isolates the representation guard. Both exact
    // immutable positives above remain unchanged and still lower completely.
    let raw = tests::changed_source(
        include_str!("../../../tests/fixtures/ramos/2afbaa9a-c171-4a8b-90f3-5250d8498356.json"),
        |record| record["source"]["mana_cost"] = "".into(),
    );
    let result = compile(MANIFEST, &raw);
    assert!(
        matches!(&result, CompileResult::Unsupported { path, reason } if path == "source.mana_cost" && reason.contains("absent-cost representation")),
        "{result:?}"
    );
}
