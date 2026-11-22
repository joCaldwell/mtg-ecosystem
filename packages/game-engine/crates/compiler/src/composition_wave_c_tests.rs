use super::*;

fn lowered(raw: &str) -> Definition {
    let header = include_str!("../../../tests/fixtures/archive-manifest-current.json");
    match compile(header, raw) {
        CompileResult::Lowered { definition } => *definition,
        other => panic!(
            "{}: {other:?}",
            strict_read::<serde_json::Value>(raw).unwrap()["source"]["name"]
        ),
    }
}

#[test]
fn composition_wave_c_exact_cast_predicates_preserve_or_and_effects() {
    let mut sram = lowered(include_str!(
        "../../../tests/fixtures/7e00b0cd-d212-4604-ba07-da21f4fe00b0.json"
    ));
    assert!(
        sram.capabilities
            .contains(&Capability::CastSubtypePredicateV1)
    );
    let TriggerCondition::SpellCast {
        spell: TypePredicate::AnyOf { terms },
        ..
    } = &mut sram.triggered[0].condition
    else {
        panic!("missing subtype disjunction")
    };
    assert_eq!(terms.len(), 3);
    terms.remove(1);
    sram.capabilities = sram.requirements();
    assert!(sram.validate().is_err());
    let sire = lowered(include_str!(
        "../../../tests/fixtures/68f1db12-82fb-4bf1-908d-db38fd67efe5.json"
    ));
    assert!(matches!(
        sire.triggered[0].program.instructions.as_slice(),
        [Instruction::OptionalDrawCards { .. }]
    ));
    let knight = lowered(include_str!(
        "../../../tests/fixtures/37733e29-3ade-4bfd-89c9-efd13b548a53.json"
    ));
    assert!(matches!(
        knight.triggered[0].program.instructions.as_slice(),
        [kernel!(CreateTokens {
            token: kir::TokenSpec::Creature { .. },
            ..
        })]
    ));
}

#[test]
fn composition_wave_c_restrictions_preserve_mana_sibling_and_ordered_draw() {
    let mut land = lowered(include_str!(
        "../../../tests/fixtures/f29dc596-2121-4421-8463-15f6c2e8b9b3.json"
    ));
    assert_eq!(land.mana_abilities.len(), 1);
    assert_eq!(land.activated.len(), 1);
    land.mana_abilities.clear();
    land.source_labels = None;
    land.capabilities = land.requirements();
    assert!(land.validate().is_err());
    let spell = lowered(include_str!(
        "../../../tests/fixtures/791fbaf1-43eb-4555-9275-c4b344f65942.json"
    ));
    assert!(matches!(
        spell.spell.unwrap().instructions.as_slice(),
        [
            Instruction::GrantCreatureRestriction { .. },
            kernel!(Draw {
                player: kir::PlayerExpr::Is { .. },
                amount: fixed!(_)
            })
        ]
    ));
}
