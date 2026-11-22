//! Complete frozen source; no compiler production expansion.
use super::*;
#[test]
fn collar_complete_source_preserves_both_keywords_and_paid_equip() {
    let raw = include_str!("../../../../tests/fixtures/f5f4dd28-f4ae-4d39-b9b8-6ebfd63c93fe.json");
    assert_eq!(
        sha256(raw.trim_end().as_bytes()),
        "5770056421b0b617e49684f854025744fa5af4a826c56a605a8b92ab2f14da5a"
    );
    let CompileResult::Lowered { definition: d } = compile(
        include_str!("../../../../tests/fixtures/archive-manifest-current.json"),
        raw,
    ) else {
        panic!("complete source rejected")
    };
    assert_eq!(d.name, "Basilisk Collar");
    assert_eq!(
        d.characteristics.mana_cost,
        ManaCost {
            generic: 1.into(),
            colored: vec![]
        }
    );
    assert!(d.characteristics.color_identity.is_empty());
    assert!(d.keywords.is_empty() && d.triggered.is_empty());
    let spell = d.spell.as_ref().unwrap();
    assert_eq!(spell.resolution, SpellResolution::Permanent);
    assert!(spell.instructions.is_empty() && spell.targets.is_empty());
    let v = serde_json::to_value(&d).unwrap();
    assert_eq!(v["static_abilities"].as_array().unwrap().len(), 1);
    let grant = &v["static_abilities"][0]["effect"]["modify_creatures"];
    assert_eq!(
        grant["keywords"],
        serde_json::json!(["deathtouch", "lifelink"])
    );
    assert_eq!(grant["power"], "0");
    assert_eq!(grant["toughness"], "0");
    assert_eq!(d.activated.len(), 1);
    assert_eq!(v["activated"][0]["cost"]["mana"]["cost"]["generic"], "2");
    assert_eq!(v["activated"][0]["program"]["timing"], "main_phase");
    assert_eq!(d.provenance.source_index, 37190);
    assert_eq!(
        d.provenance.source_sha256,
        "b9ff1efe2ee79ffb68ce5cf2b76ec936428c4b52350c845a652938f4964a21fb"
    );
    assert_eq!(
        d.provenance.ast_sha256,
        "cdca950dcfc6167277b4fbab7ad7107e0c641b503357e5d1b3513c9eb10fab86"
    );
    assert_eq!(
        d.provenance.binding_sha256,
        "cb4b3e7570231938e34ba42f76e900a77ef770bcb8dfb3ff3096c3cc966349e9"
    );
    d.validate().unwrap();
}
