//! Typed invariant controls; complete sources are checked in compiler/core.
use super::*;
pub(super) fn intrinsic() -> Definition {
    let mut d = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap()
    .definitions
    .into_iter()
    .find(|d| d.name == "Grizzly Bears")
    .unwrap();
    d.keywords = vec![KeywordAbility::Indestructible];
    d.capabilities = d.requirements();
    d
}
#[test]
fn indestructible_intrinsic_permanent_kinds_and_exact_capability_closure() {
    for kind in [
        CardKind::Creature,
        CardKind::Artifact,
        CardKind::Enchantment,
        CardKind::Land,
    ] {
        let mut d = intrinsic();
        d.characteristics.kind = kind;
        if kind != CardKind::Creature {
            d.characteristics.power = None;
            d.characteristics.toughness = None;
        }
        if kind == CardKind::Land {
            d = Catalog::read(include_str!(
                "../../../tests/fixtures/expected-wipes-ir.json"
            ))
            .unwrap()
            .definitions
            .into_iter()
            .find(|d| d.name == "Forest")
            .unwrap();
            d.keywords = vec![KeywordAbility::Indestructible];
        }
        d.capabilities = d.requirements();
        d.validate().unwrap();
        assert!(d.capabilities.contains(&Capability::IndestructibleV1));
        d.capabilities.remove(&Capability::IndestructibleV1);
        assert!(d.validate().is_err());
    }
    let mut d = intrinsic();
    d.keywords.clear();
    assert!(d.validate().is_err(), "spare capability");
    for name in ["Lightning Bolt", "Fumigate"] {
        let mut d = Catalog::read(include_str!(
            "../../../tests/fixtures/expected-wipes-ir.json"
        ))
        .unwrap()
        .definitions
        .into_iter()
        .find(|d| d.name == name)
        .unwrap();
        d.validate().unwrap();
        d.keywords.push(KeywordAbility::Indestructible);
        d.capabilities = d.requirements();
        assert!(d.validate().is_err());
    }
}
#[test]
fn indestructible_string_only_and_no_token_or_continuous_grant() {
    assert_eq!(
        strict_read::<KeywordAbility>("\"indestructible\"").unwrap(),
        KeywordAbility::Indestructible
    );
    assert!(strict_read::<KeywordAbility>("{\"indestructible\":null}").is_err());
    let token = CreatureToken {
        colors: vec![],
        subtypes: vec!["Myr".into()],
        power: 0.into(),
        toughness: 1.into(),
        keywords: vec![KeywordAbility::Indestructible],
    };
    assert!(token.validate().is_err());
    let mut d = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap()
    .definitions
    .into_iter()
    .find(|d| d.name == "Spidersilk Armor")
    .unwrap();
    d.validate().unwrap();
    let StaticEffect::ModifyCreatures { keywords, .. } = &mut d.static_abilities[0].effect else {
        panic!("grant")
    };
    *keywords = vec![KeywordAbility::Indestructible];
    d.capabilities = d.requirements();
    assert!(d.validate().is_err());
    let mut d = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap()
    .definitions
    .into_iter()
    .find(|d| d.name == "Sol Ring")
    .unwrap();
    d.validate().unwrap();
    d.keywords = vec![KeywordAbility::Indestructible, KeywordAbility::Flying];
    d.capabilities = d.requirements();
    assert!(d.validate().is_err());
    let mut d = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap()
    .definitions
    .into_iter()
    .find(|d| d.name == "Forest")
    .unwrap();
    d.validate().unwrap();
    d.keywords = vec![KeywordAbility::Flash];
    d.capabilities = d.requirements();
    assert!(d.validate().is_err());
}
