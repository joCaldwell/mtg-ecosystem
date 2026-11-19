//! Synthetic contract probes. Frozen complete sources belong to compiler/core.
use super::*;
pub(super) fn definition() -> Definition {
    let mut d = conditional_lands::tests::land();
    d.static_abilities.clear();
    d.characteristics.subtypes.clear();
    d.activated = vec![ActivatedAbility {
        functioning_zone: FunctioningZone::Battlefield,
        cost: ActivationCost::TapPayLifeSacrificeSource { life: 1.into() },
        program: typed_land_search_tests::typed_search().spell.unwrap(),
    }];
    d.activated[0].program.timing = SpellTiming::Instant;
    d.capabilities = d.requirements();
    d
}
#[test]
fn typed_fetchland_contract_nonnegative_exact_quantity_and_complete_closure() {
    let d = definition();
    for life in [
        "0",
        "1",
        "2",
        "18446744073709551615",
        "999999999999999999999999999999999999999999999",
    ] {
        let mut probe = d.clone();
        probe.activated[0].cost = ActivationCost::TapPayLifeSacrificeSource {
            life: life.parse().unwrap(),
        };
        probe.validate().unwrap();
        let roundtrip: Definition = strict_read(&serde_json::to_string(&probe).unwrap()).unwrap();
        assert_eq!(roundtrip, probe);
    }
    for mutation in 0..14 {
        let mut probe = d.clone();
        match mutation {
            0 => {
                probe.activated[0].cost =
                    ActivationCost::TapPayLifeSacrificeSource { life: (-1).into() }
            }
            1 => probe.activated[0].cost = ActivationCost::SacrificeSource,
            2 => probe.activated[0]
                .program
                .instructions
                .push(Instruction::DrawCards {
                    count: 1.into(),
                    recipient: None,
                    source_block: 0,
                }),
            3 => probe.activated[0].program.optional_additional_costs.push(
                OptionalAdditionalManaCost {
                    identity: "foreign".into(),
                    source_block: 0,
                    mana: ManaCost {
                        generic: 1.into(),
                        colored: vec![],
                    },
                },
            ),
            4 => probe.characteristics.kind = CardKind::Creature,
            5 => probe.characteristics.subtypes.push("Forest".into()),
            6 => probe.characteristics.basic = true,
            7 => probe.static_abilities.push(StaticAbility {
                functioning_zone: FunctioningZone::Battlefield,
                effect: StaticEffect::EntersTapped,
            }),
            8 => probe.activated.push(probe.activated[0].clone()),
            9 => probe.activated[0].functioning_zone = FunctioningZone::Stack,
            10 => probe.activated[0].program.timing = SpellTiming::MainPhase,
            11 => probe.activated[0].program.functioning_zone = FunctioningZone::Battlefield,
            12 => probe.activated[0].program.resolution = SpellResolution::Permanent,
            _ => probe.activated[0].program.instructions.push(
                Instruction::DrawCardsIfAdditionalCost {
                    cost_identity: "foreign".into(),
                    count: 1.into(),
                    source_block: 0,
                },
            ),
        }
        probe.capabilities = probe.requirements();
        assert!(probe.validate().is_err(), "mutation {mutation}");
    }
    for cap in d.capabilities.clone() {
        let mut probe = d.clone();
        probe.capabilities.remove(&cap);
        assert!(probe.validate().is_err(), "missing {cap:?}");
    }
    for value in [
        serde_json::json!(null),
        serde_json::json!(1),
        serde_json::json!("01"),
        serde_json::json!({"life":"1","payer":"owner"}),
    ] {
        let mut raw = serde_json::to_value(&d).unwrap();
        raw["activated"][0]["cost"]["tap_pay_life_sacrifice_source"] = value;
        assert!(strict_read::<Definition>(&raw.to_string()).is_err());
    }
}
