//! Paid frozen-source witness for CR 109.5/611.3/702.12b.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::equipped_indestructible_tests::{cast, object};
use super::temporary_indestructible_tests::target;
use super::*;

#[test]
fn avacyn_paid_static_indestructible_ends_on_departure() {
    let raw = include_str!(
        "../../../../tests/fixtures/ramos/216cb26e-8da9-478b-bfbc-8030f7adee72.json"
    );
    assert_eq!(
        sha256(raw.trim_end().as_bytes()),
        "1e3889b435b776572907a40d9ad5f67ecf15a29ee4806ece95f04b6fa7c1af0e"
    );
    let result = mtg_compiler::compile(
        include_str!("../../../../tests/fixtures/ramos/archive-manifest-current.json"),
        raw,
    );
    let mtg_compiler::CompileResult::Lowered { definition } = result else {
        panic!("{result:?}");
    };
    assert_eq!(
        definition.keywords,
        [
            KeywordAbility::Flying,
            KeywordAbility::Vigilance,
            KeywordAbility::Indestructible,
        ]
    );
    assert_eq!(
        definition.static_abilities,
        [StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::GrantOtherControlledPermanentsIndestructible,
        }]
    );
    assert!(
        definition
            .capabilities
            .contains(&Capability::StaticPermanentIndestructibleGrantV1)
    );
    let avacyn = definition.oracle_id.clone();
    let mut catalog = Catalog::read(include_str!("../../../../tests/fixtures/expected-wipes-ir.json"))
        .unwrap();
    catalog.definitions.push(*definition);
    catalog.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();

    let active = PlayerId(0);
    let opponent = PlayerId(1);
    let hand = Zone::Hand { owner: active };
    let naturalize = "bdb3ca68-ec1f-4e16-81cc-d23f8f52c728";
    let unsummon = "837182db-1bf3-4a2c-bd01-1af9d9873561";
    let sol_ring = "6ad8011d-3471-4369-9d68-b264cc027487";
    let mut config = ScenarioConfig::turns_permanents();
    config.profile = "static-indestructible-private-witness".into();
    config.initial.turn_step = TurnStep::PrecombatMain;
    config.cards = [
        (avacyn.as_str(), active, active, hand),
        (sol_ring, opponent, active, Zone::Battlefield),
        (sol_ring, opponent, opponent, Zone::Battlefield),
        (naturalize, active, active, hand),
        (unsummon, active, active, hand),
        (naturalize, active, active, hand),
    ]
    .into_iter()
    .chain(
        std::iter::repeat_n(PLAINS, 8)
            .chain([FOREST, FOREST, ISLAND, FOREST, FOREST])
            .map(|land| (land, active, active, Zone::Battlefield)),
    )
    .enumerate()
    .map(|(id, (oracle_id, owner, controller, zone))| CardSetup {
        id: CardId(id as u32),
        oracle_id: oracle_id.into(),
        owner,
        controller,
        zone,
    })
    .collect();
    let mut game = Game::with_trace(
        Prepared {
            catalog_digest: digest(&catalog).unwrap(),
            catalog: Arc::new(catalog),
            profile_digest: digest(&config).unwrap(),
            config,
            match_config: None,
        },
        TraceMode::Detailed,
    );
    let recipient = object(&game, 1);
    assert!(!game.has_keyword(recipient, KeywordAbility::Indestructible));
    cast(&mut game, 1, 0, vec![]);
    let source = object(&game, 0);
    assert_eq!(game.zone(source), Some(Zone::Battlefield));
    assert!(game.static_modifications(source).is_empty());
    assert!(game.has_keyword(source, KeywordAbility::Indestructible));
    assert!((6..14).all(|id| game.current(object(&game, id)).unwrap().tapped));
    assert!(game.has_keyword(recipient, KeywordAbility::Indestructible));
    assert!(!game.has_keyword(object(&game, 2), KeywordAbility::Indestructible));

    let destroy = target(&game, 1);
    cast(&mut game, 1, 3, destroy.clone());
    assert_eq!(game.zone(recipient), Some(Zone::Battlefield));
    assert!(game.events().iter().any(|event| matches!(
        event,
        Event::Committed { actual: EventOutcome::DestructionPrevented { object }, .. }
            if *object == recipient
    )));

    let bounce = target(&game, 0);
    cast(&mut game, 1, 4, bounce);
    assert_eq!(game.zone(object(&game, 0)), Some(hand));
    assert!(!game.has_keyword(recipient, KeywordAbility::Indestructible));
    cast(&mut game, 1, 5, destroy);
    assert_eq!(
        game.zone(object(&game, 1)),
        Some(Zone::Graveyard { owner: opponent })
    );
}
