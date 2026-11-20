//! All printed units carry the bound ability's spend predicate (CR 106.6).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::Value;

pub(super) fn lower(
    source: &Source,
    shape: &Value,
    result: &ParseResult,
    binding: &Value,
) -> Result<Option<ManaAbility>, Failure> {
    let Some((ability, ast)) = restricted_mana_source(&source.oracle_text) else {
        return Ok(None);
    };
    let type_line = source
        .type_line
        .strip_prefix("Legendary ")
        .unwrap_or(&source.type_line);
    if !type_line.starts_with("Creature — ")
        || decode::<Value>(result.abilities.get(), "abilities")? != ast
    {
        return Err(unsupported(
            "mana",
            "requires the complete printed spend restriction",
        ));
    }
    let mut graph = bindings::Bindings::new(&source.oracle_id, shape);
    if ability.source_block == 1 {
        basic_landwalk::bind_intrinsic_keyword_block(
            &mut graph,
            0,
            &[KeywordAbility::Haste],
            "mana",
        )?;
    }
    listed_mana::bind_tap_mana(
        &mut graph,
        &format!("/faces/0/abilities/{}", ability.source_block),
    );
    graph.validate(
        binding,
        "requires the complete restricted mana source and activator graph",
    )?;
    Ok(Some(ability))
}

pub(super) fn reject_unbound(abilities: &[Ability], matched: bool) -> Result<(), Failure> {
    if !matched && abilities.iter().any(|ability| matches!(ability,
        Ability::Activated { effects, .. } if effects.iter().any(|sentence| sentence.steps.iter().any(|step|
            matches!(step, Step::AddMana { spend_restriction: Some(_), .. })))))
    {
        return Err(unsupported("mana.spend_restriction", "unsupported complete restricted mana source"));
    }
    Ok(())
}

#[test]
fn restricted_mana_requires_exact_spend_restriction_and_complete_source() {
    let manifest = include_str!("../../../tests/fixtures/archive-manifest-current.json");
    for (source, spend) in [
        (
            include_str!("../../../tests/fixtures/215fd0b4-5817-4335-a0ed-1694e44bde41.json"),
            ManaSpendRestriction::ActivatedAbility,
        ),
        (
            include_str!("../../../tests/fixtures/359a570e-92a7-4954-9a05-89cc75df57e7.json"),
            ManaSpendRestriction::ColorlessSpellOrActivatedAbility,
        ),
    ] {
        let result = compile(manifest, source);
        let CompileResult::Lowered { definition } = result else {
            panic!("{result:?}")
        };
        assert!(
            matches!(definition.mana_abilities[0].production, ManaProduction::RestrictedFixed { spend: actual, .. } if actual == spend)
        );
        let bad = crate::tests::changed_source(source, |row| {
            let replacement = serde_json::json!({"restriction":"only","uses":[{"use":"cast"}]});
            row["faces"][0]["result"]["lines"][0]["ability"]["effects"][0]["steps"][0]["restriction"] =
                replacement.clone();
            row["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["restriction"] =
                replacement;
        });
        assert!(!matches!(
            compile(manifest, &bad),
            CompileResult::Lowered { .. }
        ));
    }
}

#[test]
fn restricted_mana_spell_subtypes_preserve_the_printed_haste_and_disjunction() {
    let manifest = include_str!("../../../tests/fixtures/archive-manifest-current.json");
    let source = include_str!("../../../tests/fixtures/392f6cd3-3895-42b3-8bb9-6b0c6a689a29.json");
    let CompileResult::Lowered { definition } = compile(manifest, source) else {
        panic!("{:?}", compile(manifest, source));
    };
    assert_eq!(definition.keywords, [KeywordAbility::Haste]);
    assert!(matches!(
        definition.mana_abilities[0].production,
        ManaProduction::RestrictedFixed {
            spend: ManaSpendRestriction::SpellSubtypes { .. },
            ..
        }
    ));
    assert_eq!(definition.mana_abilities[0].source_block, 1);
    let bad = crate::tests::changed_source(source, |row| {
        for path in [
            "/faces/0/result/lines/1/ability/effects/0/steps/0/restriction/uses/0/filter/allOf/0/anyOf",
            "/faces/0/result/abilities/1/effects/0/steps/0/restriction/uses/0/filter/allOf/0/anyOf",
        ] {
            row.pointer_mut(path).unwrap().as_array_mut().unwrap().pop();
        }
    });
    assert!(!matches!(
        compile(manifest, &bad),
        CompileResult::Lowered { .. }
    ));
}
