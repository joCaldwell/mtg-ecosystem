//! Complete synthetic IR witnesses; exact frozen Ramos is tested by the compiler.
use super::*;
pub(super) fn ramos() -> Definition {
    let c = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-mana-ir.json"
    ))
    .unwrap();
    let mut d = c
        .definitions
        .into_iter()
        .find(|d| d.name == "Grizzly Bears")
        .unwrap();
    d.triggered = vec![TriggeredAbility {
        functioning_zone: FunctioningZone::Battlefield,
        death_filter: None,
        intervening_if: None,
        condition: TriggerCondition::SpellCast {
            caster: CasterRelation::Controller,
            spell: TypePredicate::Any {},
        },
        program: SpellProgram {
            functioning_zone: FunctioningZone::Stack,
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            additional_sacrifice: None,
            targets: vec![],
            modes: vec![],
            instructions: vec![Instruction::PutSourcePlusOneCountersForCastColors {
                source_block: 1,
            }],
        },
    }];
    d.mana_abilities = vec![ManaAbility {
        source_block: 2,
        restriction: Some(ManaActivationRestriction::OnceEachTurn),
        costs: PermanentActivationCosts {
            tap: false,
            mana: None,
            sacrifice: false,
            remove_source_plus_one_counters: Some(5.into()),
        },
        production: ManaProduction::Fixed {
            symbols: vec![
                Color::W,
                Color::W,
                Color::U,
                Color::U,
                Color::B,
                Color::B,
                Color::R,
                Color::R,
                Color::G,
                Color::G,
            ],
        },
    }];
    d.capabilities = d.requirements();
    d
}
#[test]
fn ramos_m1_contract_roundtrip_and_complete_closed_shapes() {
    let d = ramos();
    d.validate().unwrap();
    assert_eq!(
        strict_read::<Definition>(&canonical(&d).unwrap()).unwrap(),
        d
    );
    for mutate in [
        (|d: &mut Definition| d.triggered[0].condition = TriggerCondition::SelfEntersBattlefield)
            as fn(&mut Definition),
        |d| d.mana_abilities[0].restriction = None,
        |d| d.mana_abilities[0].costs.remove_source_plus_one_counters = Some(0.into()),
        |d| d.mana_abilities[0].costs.tap = true,
        |d| d.mana_abilities[0].costs.sacrifice = true,
        |d| d.mana_abilities[0].production = ManaProduction::CommanderColor,
        |d| d.mana_abilities.push(d.mana_abilities[0].clone()),
    ] {
        let mut bad = d.clone();
        mutate(&mut bad);
        bad.capabilities = bad.requirements();
        assert!(bad.validate().is_err());
    }
    for c in [
        Capability::CastSpellColorCountersV1,
        Capability::CounterRemovalManaV1,
    ] {
        let mut bad = d.clone();
        bad.capabilities.remove(&c);
        assert!(bad.validate().is_err());
    }
    let schema = catalog_schema();
    assert!(schema["$defs"]["ManaAbility"]["allOf"].is_array());
}

#[test]
fn ramos_m1_counter_removal_is_a_parameterized_family_not_printed_quantities() {
    for (n, symbols) in [
        (1, vec![Color::C]),
        (3, vec![Color::R, Color::G, Color::G]),
        (7, vec![Color::U; 4]),
    ] {
        let mut d = ramos();
        d.mana_abilities[0].costs.remove_source_plus_one_counters = Some(n.into());
        d.mana_abilities[0].production = ManaProduction::Fixed { symbols };
        d.capabilities = d.requirements();
        d.validate().unwrap();
        assert_eq!(
            strict_read::<Definition>(&canonical(&d).unwrap()).unwrap(),
            d
        );
    }
    let mut zero = ramos();
    zero.mana_abilities[0].costs.remove_source_plus_one_counters = Some(0.into());
    assert!(zero.validate().is_err());
    let mut negative = ramos();
    negative.mana_abilities[0]
        .costs
        .remove_source_plus_one_counters = Some((-1).into());
    assert!(negative.validate().is_err());
    let mut empty = ramos();
    empty.mana_abilities[0].production = ManaProduction::Fixed { symbols: vec![] };
    assert!(empty.validate().is_err());
}
