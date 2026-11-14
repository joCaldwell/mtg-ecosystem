//! Typed controls, independent of Plate's intrinsic keyword.
use super::*;
pub(super) fn equipped() -> Definition {
    let mut d = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-attachments-ir.json"
    ))
    .unwrap()
    .definitions
    .into_iter()
    .find(|d| d.name == "Bonesplitter")
    .unwrap();
    d.static_abilities[0].effect = StaticEffect::ModifyCreatures {
        scope: CreatureScope::Equipped,
        power: 0.into(),
        toughness: 0.into(),
        keywords: vec![KeywordAbility::Indestructible],
        restrictions: vec![],
    };
    d.capabilities = d.requirements();
    d
}
#[test]
fn equipped_indestructible_exact_capability_without_intrinsic() {
    let mut d = equipped();
    assert!(d.keywords.is_empty());
    d.validate().unwrap();
    assert!(d.capabilities.contains(&Capability::IndestructibleV1));
    d.capabilities.remove(&Capability::IndestructibleV1);
    assert!(d.validate().is_err());
    d = equipped();
    d.static_abilities.clear();
    assert!(d.validate().is_err(), "spare capability");
}
#[test]
fn equipped_indestructible_rejects_every_other_grant_context() {
    let base = equipped();
    for scope in [
        CreatureScope::Controller,
        CreatureScope::Enchanted,
        CreatureScope::Source,
        CreatureScope::AttackingController,
    ] {
        let mut d = base.clone();
        let StaticEffect::ModifyCreatures { scope: s, .. } = &mut d.static_abilities[0].effect
        else {
            unreachable!()
        };
        *s = scope;
        d.capabilities = d.requirements();
        assert!(d.validate().is_err());
    }
    for variant in 0..9 {
        let mut d = base.clone();
        let StaticEffect::ModifyCreatures {
            power,
            toughness,
            keywords,
            restrictions,
            ..
        } = &mut d.static_abilities[0].effect
        else {
            unreachable!()
        };
        match variant {
            0 => *power = 1.into(),
            1 => *toughness = 1.into(),
            2 => keywords.push(KeywordAbility::Flying),
            3 => keywords.push(KeywordAbility::Indestructible),
            4 => restrictions.push(CreatureRestriction::CantBlock),
            5 => d.characteristics.subtypes.clear(),
            6 => d.characteristics.kind = CardKind::Enchantment,
            7 => d.characteristics.subtypes.push("Aura".into()),
            _ => d.static_abilities[0].functioning_zone = FunctioningZone::Stack,
        }
        d.capabilities = d.requirements();
        assert!(d.validate().is_err(), "variant {variant}");
    }
}
