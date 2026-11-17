//! Synthetic typed composition controls, separate from exact Escort source tests.
use super::*;

pub(super) fn grant(keywords: &[KeywordAbility]) -> Definition {
    let mut d = tap_source_destroy_tests::tap_source_destroy();
    let ordinary = creature_set_keyword_tests::grant(ObjectSet::ControllerCreatures, keywords, false);
    d.activated[0].cost = ActivationCost::SacrificeSource;
    d.activated[0].program = ordinary.spell.unwrap();
    d.capabilities = d.requirements();
    d
}

#[test]
fn sacrifice_set_parameterized_keyword_composition_and_exact_capabilities() {
    for keywords in [vec![KeywordAbility::Indestructible],
        vec![KeywordAbility::Flying, KeywordAbility::Lifelink],
        vec![KeywordAbility::Hexproof, KeywordAbility::Haste, KeywordAbility::FirstStrike]] {
        let d = grant(&keywords);
        d.validate().unwrap();
        let p = sacrifice_source_creature_set_keyword_ability(&d.activated[0]).unwrap();
        assert_eq!(p.keywords, keywords);
        assert!(p.modifier.is_none());
        let mut expected = BTreeSet::from([Capability::NormalLayoutV1, Capability::BaseCreatureV1,
            Capability::CastCreatureV1, Capability::ActivatedAbilityV1, Capability::SacrificeSourceCostV1,
            Capability::BoardWipesV1, Capability::TemporaryCreatureSetKeywordProgramV1]);
        expected.extend(keywords.iter().map(|k| temporary_keyword_primitive_capability(*k).unwrap()));
        assert_eq!(d.capabilities, expected);
        let encoded = serde_json::to_string(&d).unwrap();
        let round: Definition = strict_read(&encoded).unwrap();
        round.validate().unwrap();
        assert_eq!(round, d);
        for cap in &expected {
            let mut bad = d.clone(); bad.capabilities.remove(cap);
            assert!(bad.validate().is_err(), "missing {cap:?}");
        }
        for cap in [Capability::SacrificeV1, Capability::TemporaryPermanentSetKeywordProgramV1,
            Capability::TemporaryIndestructibleSpellV1, Capability::CreatureTargetV1] {
            let mut bad = d.clone(); bad.capabilities.insert(cap);
            assert!(bad.validate().is_err(), "spare {cap:?}");
        }
    }
}

#[test]
fn sacrifice_set_source_cost_program_closure_rejects_synthetic_mutations() {
    let baseline = grant(&[KeywordAbility::Indestructible]);
    for variant in 0..23 {
        let mut d = baseline.clone();
        let a = &mut d.activated[0];
        match variant {
            0 => a.cost = ActivationCost::TapSource,
            1 => a.cost = ActivationCost::Mana { cost: ManaCost { generic: 0.into(), colored: vec![] } },
            2 => a.cost = ActivationCost::SacrificePermanent { filter: SacrificeFilter { types: vec![CardKind::Creature], subtypes: vec![] }, mana: None },
            3 => a.functioning_zone = FunctioningZone::Stack,
            4 => a.program.timing = SpellTiming::MainPhase,
            5 => a.program.additional_sacrifice = Some(SacrificeFilter { types: vec![CardKind::Creature], subtypes: vec![] }),
            6 => a.program.instructions.push(a.program.instructions[0].clone()),
            7 => a.program.targets = temporary_creature_keyword_tests::grant(&[KeywordAbility::Indestructible], false).spell.unwrap().targets,
            8 => { let p = a.program.clone(); a.program.instructions.clear(); a.program.modes = vec![SpellMode { program: p }]; },
            9..=13 => if let Instruction::GrantKeywordsSet { set, modifier, source_block, keywords, .. } = &mut a.program.instructions[0] {
                match variant {
                    9 => *set = ObjectSet::AttackingCreatures,
                    10 => *set = ObjectSet::ControllerPermanents,
                    11 => *modifier = Some(CreatureStatModifier { power: 0.into(), toughness: 0.into() }),
                    12 => *source_block = 1,
                    _ => keywords.push(KeywordAbility::Indestructible),
                }
            },
            14 => d.characteristics.kind = CardKind::Artifact,
            15 => d.characteristics.additional_types = vec![CardKind::Artifact],
            16 => d.activated.push(d.activated[0].clone()),
            17 => d.keywords = vec![KeywordAbility::Flying],
            18 => d.intrinsic_zone = Some(FunctioningZone::Battlefield),
            19 => d.x_mana_cost = true,
            20 => d.static_abilities.push(StaticAbility { functioning_zone: FunctioningZone::Battlefield, effect: StaticEffect::CantBeCountered }),
            21 => d.characteristics.legendary = true,
            _ => a.program.optional_additional_costs.push(OptionalAdditionalManaCost { identity: "synthetic-cost".into(), source_block: 0, mana: ManaCost { generic: 0.into(), colored: vec![] } }),
        }
        d.capabilities = d.requirements();
        assert!(d.validate().is_err(), "synthetic mutation {variant}");
    }
    let mut triggered = Catalog::read(include_str!("../../../tests/fixtures/expected-effects-ir.json")).unwrap()
        .definitions.into_iter().find(|d| d.name == "Blood Artist").unwrap();
    triggered.triggered[0].program = baseline.activated[0].program.clone();
    triggered.capabilities = triggered.requirements();
    assert!(triggered.validate().is_err());
    // Ordinary families retain their existing independent admission.
    creature_set_keyword_tests::grant(ObjectSet::ControllerCreatures, &[KeywordAbility::Indestructible], false).validate().unwrap();
    creature_set_keyword_tests::grant(ObjectSet::ControllerPermanents, &[KeywordAbility::Indestructible], false).validate().unwrap();
}
