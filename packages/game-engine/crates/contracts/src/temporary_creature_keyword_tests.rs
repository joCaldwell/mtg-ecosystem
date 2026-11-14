//! Synthetic typed family controls. New-source native tests are UNRUN here;
//! the parent owns combined contracts/compiler/runtime/schema qualification.
use super::*;

pub(super) fn grant(keywords: &[KeywordAbility], modifier: bool) -> Definition {
    let mut d = if modifier {
        compound_indestructible_tests::compound()
    } else {
        temporary_indestructible_tests::grant()
    };
    let Instruction::GrantKeywords {
        keywords: granted, ..
    } = d.spell.as_mut().unwrap().instructions.last_mut().unwrap()
    else {
        unreachable!()
    };
    *granted = keywords.to_vec();
    d.capabilities = d.requirements();
    d
}

#[test]
fn temporary_creature_keyword_family_preserves_subset_fences() {
    for d in [
        temporary_flying_tests::grant(),
        temporary_deathtouch_tests::grant(),
        temporary_indestructible_tests::grant(),
        compound_indestructible_tests::compound(),
    ] {
        d.validate().unwrap();
        assert!(
            !d.requirements()
                .contains(&Capability::TemporaryCreatureKeywordProgramV1)
        );
        let mut spare = d;
        spare
            .capabilities
            .insert(Capability::TemporaryCreatureKeywordProgramV1);
        assert!(spare.validate().is_err());
    }
    for (keywords, modifier) in [
        (vec![KeywordAbility::Lifelink], false),
        (
            vec![KeywordAbility::Flying, KeywordAbility::Indestructible],
            false,
        ),
        (vec![KeywordAbility::Flying], true),
        (vec![KeywordAbility::Deathtouch], true),
        (vec![KeywordAbility::Lifelink], true),
    ] {
        let d = grant(&keywords, modifier);
        d.validate().unwrap();
        assert!(
            d.capabilities
                .contains(&Capability::TemporaryCreatureKeywordProgramV1)
        );
        let descriptor = temporary_creature_keyword_program(d.spell.as_ref().unwrap()).unwrap();
        assert_eq!(descriptor.keywords, keywords);
        assert_eq!(descriptor.modifier.is_some(), modifier);
    }
}
