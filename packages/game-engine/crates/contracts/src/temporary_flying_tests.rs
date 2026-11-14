//! Typed IR controls; complete source is independently tested by the compiler.
use super::*;
pub(super) fn grant() -> Definition {
    let mut d = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-effects-ir.json"
    ))
    .unwrap()
    .definitions
    .into_iter()
    .find(|d| d.name == "Giant Growth")
    .unwrap();
    d.spell.as_mut().unwrap().instructions = vec![Instruction::GrantKeywords {
        target_slot: 0,
        keywords: vec![KeywordAbility::Flying],
        duration: Duration::UntilEndOfTurn,
        source_block: 0,
    }];
    d.capabilities = d.requirements();
    d
}
#[test]
fn temporary_flying_exact_capabilities_and_program() {
    let d = grant();
    d.validate().unwrap();
    for cap in [Capability::TemporaryFlyingSpellV1, Capability::FlyingV1] {
        let mut bad = d.clone();
        bad.capabilities.remove(&cap);
        assert!(bad.validate().is_err());
    }
    for variant in 0..9 {
        let mut b = d.clone();
        let p = b.spell.as_mut().unwrap();
        match variant {
            0 => {
                if let Instruction::GrantKeywords { keywords, .. } = &mut p.instructions[0] {
                    keywords.clear();
                }
            }
            1 => {
                if let Instruction::GrantKeywords { keywords, .. } = &mut p.instructions[0] {
                    keywords.push(KeywordAbility::Indestructible);
                }
            }
            2 => {
                if let Instruction::GrantKeywords { keywords, .. } = &mut p.instructions[0] {
                    keywords.push(KeywordAbility::Flying);
                }
            }
            3 => p.targets[0].domain = TargetDomain::BlockingBattlefieldCreature,
            4 => p.instructions.push(p.instructions[0].clone()),
            5 => {
                let child = p.clone();
                p.targets.clear();
                p.instructions.clear();
                p.modes = vec![
                    SpellMode {
                        program: child.clone(),
                    },
                    SpellMode { program: child },
                ];
            }
            6 => b.characteristics.kind = CardKind::Creature,
            7 => p.functioning_zone = FunctioningZone::Battlefield,
            _ => {
                if let Instruction::GrantKeywords { target_slot, .. } = &mut p.instructions[0] {
                    *target_slot = 1;
                }
            }
        }
        b.capabilities = b.requirements();
        assert!(b.validate().is_err(), "variant {variant}");
    }
    let mut spare = d;
    spare.spell.as_mut().unwrap().instructions.clear();
    assert!(spare.validate().is_err());
}

#[test]
fn temporary_flying_rejects_activation_and_trigger_contexts() {
    let c = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-effects-ir.json"
    ))
    .unwrap();
    for name in ["Samite Healer", "Blood Artist"] {
        let mut d = c
            .definitions
            .iter()
            .find(|d| d.name == name)
            .unwrap()
            .clone();
        d.validate().unwrap();
        let p = grant().spell.unwrap();
        if let Some(a) = d.activated.first_mut() {
            a.program = p;
        } else {
            d.triggered[0].program = p;
        }
        d.capabilities = d.requirements();
        assert_eq!(
            d.validate().unwrap_err(),
            "temporary indestructible requires an ordinary single-target spell"
        );
    }
}

#[test]
fn temporary_flying_rejects_additional_cost_and_empty_occurrence() {
    let d = grant();
    let c = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap();
    let mut cost = d.clone();
    cost.spell.as_mut().unwrap().additional_sacrifice = c
        .definitions
        .iter()
        .find(|d| d.name == "Bone Splinters")
        .unwrap()
        .spell
        .as_ref()
        .unwrap()
        .additional_sacrifice
        .clone();
    cost.capabilities = cost.requirements();
    assert!(cost.validate().is_err());
    let mut empty = d;
    empty.spell.as_mut().unwrap().targets[0]
        .selection_id
        .clear();
    assert!(empty.validate().is_err());
}

#[test]
fn temporary_flying_rejects_spare_capabilities_and_other_single_keywords() {
    let d = grant();
    for cap in [
        Capability::TemporaryIndestructibleSpellV1,
        Capability::IndestructibleV1,
        Capability::ReachV1,
    ] {
        let mut b = d.clone();
        b.capabilities.insert(cap);
        assert!(b.validate().is_err());
    }
    for keyword in [
        KeywordAbility::Reach,
        KeywordAbility::Haste,
        KeywordAbility::Deathtouch,
    ] {
        let mut b = d.clone();
        if let Instruction::GrantKeywords { keywords, .. } =
            &mut b.spell.as_mut().unwrap().instructions[0]
        {
            *keywords = vec![keyword];
        }
        b.capabilities = b.requirements();
        assert!(b.validate().is_err());
    }
}
