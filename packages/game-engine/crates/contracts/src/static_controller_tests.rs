//! Synthetic typed IR controls; complete-source positives live in compiler/core.
use super::*;

fn creature() -> Definition {
    Catalog::read(include_str!(
        "../../../tests/fixtures/expected-mana-ir.json"
    ))
    .unwrap()
    .definitions
    .into_iter()
    .find(|d| d.name == "Wall of Wood")
    .unwrap()
}
fn modifier(power: i64, keywords: Vec<KeywordAbility>) -> StaticEffect {
    StaticEffect::ModifyCreatures {
        scope: CreatureScope::Controller,
        power: power.into(),
        toughness: Quantity::zero(),
        keywords,
        restrictions: vec![],
    }
}
#[test]
fn static_controller_generic_creature_modifiers_and_existing_enchantments() {
    for (power, keywords) in [
        (0, vec![KeywordAbility::Haste]),
        (7, vec![KeywordAbility::Flying, KeywordAbility::Trample]),
        (3, vec![]),
    ] {
        let effect = modifier(power, keywords);
        for kind in [CardKind::Creature, CardKind::Enchantment] {
            assert!(static_creature_modification_valid(
                kind, false, false, &effect
            ));
        }
        let mut d = creature();
        d.static_abilities = vec![StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect,
        }];
        d.capabilities = d.requirements();
        d.validate().unwrap();
        for count in [1, 2, 5, 257] {
            let mut many = d.clone();
            many.static_abilities = vec![d.static_abilities[0].clone(); count];
            many.validate().unwrap();
        }
        d.capabilities.remove(&Capability::AttachmentsV1);
        assert!(d.validate().is_err());
    }
    Catalog::read(include_str!(
        "../../../tests/fixtures/expected-attachments-ir.json"
    ))
    .unwrap()
    .validate()
    .unwrap();
}
#[test]
fn static_controller_rejects_unsupported_kind_scope_and_malformed_modifiers() {
    let effect = modifier(0, vec![KeywordAbility::Haste]);
    for kind in [
        CardKind::Artifact,
        CardKind::Land,
        CardKind::Instant,
        CardKind::Sorcery,
    ] {
        assert!(!static_creature_modification_valid(
            kind, false, false, &effect
        ));
    }
    assert!(!static_creature_modification_valid(
        CardKind::Enchantment,
        false,
        true,
        &effect
    ));
    for effect in [
        modifier(-1, vec![]),
        modifier(0, vec![]),
        modifier(0, vec![KeywordAbility::Flash]),
        modifier(0, vec![KeywordAbility::Indestructible]),
        modifier(0, vec![KeywordAbility::Haste, KeywordAbility::Haste]),
    ] {
        let mut d = creature();
        d.static_abilities = vec![StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect,
        }];
        d.capabilities = d.requirements();
        assert!(d.validate().is_err());
    }
    for scope in [
        CreatureScope::Enchanted,
        CreatureScope::Equipped,
        CreatureScope::Source,
        CreatureScope::AttackingController,
    ] {
        let mut effect = effect.clone();
        if let StaticEffect::ModifyCreatures { scope: s, .. } = &mut effect {
            *s = scope;
        }
        assert!(!static_creature_modification_valid(
            CardKind::Creature,
            false,
            false,
            &effect
        ));
    }
    let mut d = creature();
    d.static_abilities = vec![StaticAbility {
        functioning_zone: FunctioningZone::Stack,
        effect,
    }];
    d.capabilities = d.requirements();
    assert!(d.validate().is_err());
}
