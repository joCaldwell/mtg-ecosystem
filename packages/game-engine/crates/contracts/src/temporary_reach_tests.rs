//! Synthetic complete-family controls, separate from printed source witnesses.
use super::*;
#[test]
fn temporary_reach_valid_vectors_have_exact_primitive_authority() {
    for keywords in [
        vec![KeywordAbility::Reach],
        vec![KeywordAbility::Flying, KeywordAbility::Reach],
        vec![KeywordAbility::Reach, KeywordAbility::Hexproof],
        vec![
            KeywordAbility::Reach,
            KeywordAbility::FirstStrike,
            KeywordAbility::Deathtouch,
        ],
    ] {
        let mut definitions = vec![];
        for modifier in [false, true] {
            for domain in [
                TargetDomain::BattlefieldCreature,
                TargetDomain::ControlledBattlefieldCreature,
            ] {
                let mut d = temporary_creature_keyword_tests::grant(&keywords, modifier);
                d.spell.as_mut().unwrap().targets[0].domain = domain;
                definitions.push(d);
            }
            for set in [
                ObjectSet::ControllerCreatures,
                ObjectSet::AttackingCreatures,
            ] {
                definitions.push(creature_set_keyword_tests::grant(set, &keywords, modifier));
            }
        }
        definitions.push(creature_set_keyword_tests::grant(
            ObjectSet::ControllerPermanents,
            &keywords,
            false,
        ));
        let mut d = temporary_permanent_keyword_tests::grant(false);
        if let Instruction::GrantKeywords { keywords: k, .. } =
            &mut d.spell.as_mut().unwrap().instructions[0]
        {
            *k = keywords.clone()
        }
        let mut filtered = d.clone();
        filtered.spell.as_mut().unwrap().targets[0].domain = TargetDomain::BattlefieldPermanent;
        filtered.spell.as_mut().unwrap().targets[0]
            .filter
            .as_mut()
            .unwrap()
            .types = vec![CardKind::Creature, CardKind::Artifact];
        definitions.push(filtered);
        definitions.push(d);
        for mut d in definitions {
            d.capabilities = d.requirements();
            d.validate().unwrap();
            assert!(d.capabilities.contains(&Capability::ReachV1));
            let mut missing = d.clone();
            missing.capabilities.remove(&Capability::ReachV1);
            assert!(missing.validate().is_err());
            for spare in [
                Capability::TemporaryFlyingSpellV1,
                Capability::TemporaryDeathtouchSpellV1,
            ] {
                let mut extra = d.clone();
                extra.capabilities.insert(spare);
                assert!(extra.validate().is_err());
            }
        }
    }
}
#[test]
fn temporary_reach_keeps_unsupported_vectors_and_legacy_subset_fences() {
    for keywords in [
        vec![],
        vec![KeywordAbility::Reach, KeywordAbility::Reach],
        vec![KeywordAbility::Reach, KeywordAbility::Vigilance],
        vec![KeywordAbility::Reach, KeywordAbility::Shroud],
    ] {
        assert!(!supported_temporary_keywords(&keywords));
    }
    let d = temporary_flying_tests::grant();
    d.validate().unwrap();
    assert!(
        !d.capabilities
            .contains(&Capability::TemporaryCreatureKeywordProgramV1)
    );
    let mut bad = d;
    bad.capabilities.insert(Capability::ReachV1);
    assert!(bad.validate().is_err());
}
