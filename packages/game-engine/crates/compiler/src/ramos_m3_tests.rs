//! Frozen M3 sources must remain whole-source rejections at the foundation
//! checkpoint. A shared announcement/payment primitive is not cascade support.
use super::*;
const MANIFEST: &str = include_str!("../../../tests/fixtures/ramos/archive-manifest-current.json");
fn rejected(
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
    assert!(
        matches!(&result, CompileResult::Unsupported { path, reason }
        if path == expected_path && reason == "keyword:cascade"),
        "{result:?}"
    );
}
#[test]
fn ramos_m3_frozen_artifact_creature_agent_is_not_partially_admitted() {
    let raw =
        include_str!("../../../tests/fixtures/ramos/2afbaa9a-c171-4a8b-90f3-5250d8498356.json");
    rejected(
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
fn ramos_m3_frozen_bloodbraid_haste_does_not_admit_partial_cascade() {
    let raw =
        include_str!("../../../tests/fixtures/ramos/3f0c9466-5ab9-4205-a84f-b4b27b5a678e.json");
    rejected(
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
