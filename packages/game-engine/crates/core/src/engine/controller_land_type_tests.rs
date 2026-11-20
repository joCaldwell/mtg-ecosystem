use super::*;
use crate::engine::equipped_indestructible_tests::{answer, recover, request};

// No source identities in the engine: the independent grant is compiled from
// an original archive; this private catalog includes both printed provider shapes.
fn game() -> Game {
    let mut p = Prepared::admit(
        Catalog::read(include_str!("../../../../tests/fixtures/expected-ir.json")).unwrap(),
        ScenarioConfig::land_return(),
    )
    .unwrap();
    let catalog = Arc::make_mut(&mut p.catalog);
    let mtg_compiler::CompileResult::Lowered { definition: grant } = mtg_compiler::compile(
        mtg_compiler::fixtures::MANIFEST,
        include_str!("../../../../tests/fixtures/synthetic-controller-land-types.json"),
    ) else {
        panic!("grant witness");
    };
    let mut printed = catalog.definition_by_oracle_id(ISLAND).unwrap().clone();
    printed.source_labels = None;
    printed.oracle_id = "synthetic-two-printed-lines".into();
    printed.name = "Synthetic Two-Line Source".into();
    printed.characteristics.basic = false;
    printed.characteristics.subtypes.clear();
    let tap = PermanentActivationCosts {
        tap: true,
        mana: None,
        sacrifice: false,
        remove_source_plus_one_counters: None,
    };
    printed.mana_abilities = vec![
        ManaAbility {
            source_block: 0,
            costs: tap.clone(),
            restriction: None,
            self_damage: None,
            production: ManaProduction::Fixed {
                symbols: vec![Color::C],
            },
        },
        ManaAbility {
            source_block: 1,
            costs: tap.clone(),
            restriction: None,
            self_damage: Some(1.into()),
            production: ManaProduction::Choice {
                colors: vec![Color::U, Color::R],
            },
        },
    ];
    printed.capabilities = printed.requirements();
    let mut dynamic = printed.clone();
    dynamic.oracle_id = "synthetic-opposing-output".into();
    dynamic.name = "Synthetic Opposing Output".into();
    dynamic.mana_abilities = vec![ManaAbility {
        source_block: 0,
        costs: tap,
        restriction: None,
        self_damage: None,
        production: ManaProduction::OpponentLandColors,
    }];
    dynamic.characteristics.color_identity.clear();
    dynamic.capabilities = dynamic.requirements();
    let (a, b, c) = (
        printed.oracle_id.clone(),
        grant.oracle_id.clone(),
        dynamic.oracle_id.clone(),
    );
    catalog.definitions.extend([printed, *grant, dynamic]);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    let payer = PlayerId(0);
    p.config.cards = vec![
        CardSetup {
            id: CardId(0),
            oracle_id: a,
            owner: payer,
            controller: payer,
            zone: Zone::Battlefield,
        },
        CardSetup {
            id: CardId(1),
            oracle_id: b,
            owner: PlayerId(1),
            controller: payer,
            zone: Zone::Battlefield,
        },
        CardSetup {
            id: CardId(2),
            oracle_id: ISLAND.into(),
            owner: payer,
            controller: payer,
            zone: Zone::Battlefield,
        },
        CardSetup {
            id: CardId(3),
            oracle_id: c,
            owner: PlayerId(2),
            controller: PlayerId(2),
            zone: Zone::Battlefield,
        },
        CardSetup {
            id: CardId(4),
            oracle_id: ISLAND.into(),
            owner: payer,
            controller: payer,
            zone: Zone::Library { owner: payer },
        },
    ];
    p.catalog_digest = digest(catalog).unwrap();
    p.profile_digest = digest(&p.config).unwrap();
    Game::with_trace(p, TraceMode::Detailed)
}

#[test]
fn controller_land_types_preserve_membership_and_offer_independent_printed_costs() {
    let mut g = game();
    let snapshot: serde_json::Value = serde_json::from_str(&g.snapshot().unwrap()).unwrap();
    assert_eq!(snapshot["snapshot_version"], 64);
    let land = g.object_id(CardId(0)).unwrap();
    let island = g.object_id(CardId(2)).unwrap();
    let off = g.object_id(CardId(4)).unwrap();
    let opposing = g.object_id(CardId(3)).unwrap();
    assert_eq!(
        g.characteristics(island).unwrap().subtypes,
        ["Island", "Plains", "Swamp", "Mountain", "Forest"]
    );
    assert!(g.characteristics(island).unwrap().basic);
    let frozen = g.object_before(land).unwrap();
    assert_eq!(
        frozen.characteristics.subtypes,
        ["Plains", "Island", "Swamp", "Mountain", "Forest"]
    );
    assert!(!frozen.characteristics.basic);
    assert_eq!(g.characteristics(off).unwrap().subtypes, ["Island"]);
    assert!(g.characteristics(opposing).unwrap().subtypes.is_empty());
    assert_eq!(g.opponent_land_mana_colors(PlayerId(2)).unwrap().len(), 5);
    // Change the default controller in an isolated layer-2 probe. The actual
    // sacrifice/departure and both mana resolutions below use normal inputs.
    let mut changed = g.fork();
    changed
        .state
        .rules
        .objects
        .get_mut(&CardId(1))
        .unwrap()
        .default_controller = PlayerId(2);
    changed.reconcile_controllers().unwrap();
    assert!(changed.characteristics(land).unwrap().subtypes.is_empty());
    assert_eq!(
        changed.characteristics(opposing).unwrap().subtypes,
        ["Plains", "Island", "Swamp", "Mountain", "Forest"]
    );
    assert_eq!(
        changed
            .mixed_mana_choices(PlayerId(2), opposing)
            .unwrap()
            .len(),
        6
    );
    assert_eq!(
        changed
            .opponent_land_mana_colors(PlayerId(2))
            .unwrap()
            .into_iter()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([Color::U, Color::R])
    );
    let mut narrowed = g.fork();
    answer(&mut narrowed, 1, Response::ActivateMana { object: island });
    assert!(matches!(request(&mut narrowed,1),Request::ManaColor{colors,..} if colors.len()==5));
    narrowed.concede(PlayerId(1)).unwrap();
    assert!(matches!(request(&mut narrowed,1),Request::ManaColor{colors,..} if colors==[Color::U]));
    recover(&narrowed);
    answer(&mut g, 1, Response::ActivateMana { object: land });
    let Request::ChooseManaAbility { choices, .. } = request(&mut g, 1) else {
        panic!("printed/intrinsic choice");
    };
    assert_eq!(choices.len(), 7);
    recover(&g);
    let mut free = g.fork();
    let seat = PlayerId(0);
    let QueryResult::Complete(QueryValue::Observation(observation)) =
        free.observe(seat, Budget::new(65536))
    else {
        panic!("chooser observation");
    };
    assert_eq!(observation.observation_version, 41);
    let Some(PlayerRequest::ChooseManaAbility {
        token,
        choices: offered,
        mana_ability_source,
        ..
    }) = observation.decision
    else {
        panic!("permitted mixed ability choice");
    };
    assert_eq!(offered, choices);
    assert_eq!(mana_ability_source, free.handle_for(seat, land).unwrap());
    let QueryResult::Complete(QueryValue::Observation(other)) =
        free.observe(PlayerId(2), Budget::new(65536))
    else {
        panic!("opponent observation");
    };
    assert!(other.decision.is_none() && other.decision_context.is_none());
    for (actor, response) in [
        (
            PlayerId(2),
            PlayerResponse::ChooseManaAbility {
                ability: ManaAbilityIdentity::Printed { index: 0 },
            },
        ),
        (
            seat,
            PlayerResponse::ChooseManaAbility {
                ability: ManaAbilityIdentity::Printed { index: 12 },
            },
        ),
        (seat, PlayerResponse::ManaColor { color: Color::U }),
    ] {
        assert!(matches!(
            free.check_player_response(actor, token.clone(), response, Budget::new(65536)),
            QueryResult::Rejected(_)
        ));
    }
    let QueryResult::Incomplete { work, .. } = free.check_player_response(
        seat,
        token,
        PlayerResponse::ChooseManaAbility {
            ability: ManaAbilityIdentity::Intrinsic { color: Color::U },
        },
        Budget::new(1),
    ) else {
        panic!("suspended mixed mana choice");
    };
    let encoded = free.save_work(&work).unwrap();
    let saved: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    assert_eq!(saved["work_version"], 37);
    let restored = free.restore_work(&encoded).unwrap();
    let QueryResult::Complete(QueryValue::Validated(receipt)) =
        free.resume_work(restored, Budget::new(65536))
    else {
        panic!("authorized mixed choice");
    };
    free.submit(receipt).unwrap();
    assert!(matches!(request(&mut free, 1), Request::Priority { .. }));
    assert_eq!(free.mana(PlayerId(0), Color::U), Some(1.into()));
    assert_eq!(free.state.rules.players[0].life, 40.into());
    recover(&free);
    let mut lost = g.fork();
    lost.concede(PlayerId(1)).unwrap();
    assert!(matches!(request(&mut lost, 1), Request::Priority { .. }));
    assert!(lost.characteristics(land).unwrap().subtypes.is_empty());
    assert_eq!(lost.characteristics(island).unwrap().subtypes, ["Island"]);
    assert_eq!(
        lost.opponent_land_mana_colors(PlayerId(2))
            .unwrap()
            .into_iter()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([Color::U, Color::R])
    );
    lost.validate_characteristic_identity(land, Zone::Battlefield, &frozen.characteristics)
        .unwrap();
    assert!(
        lost.validate_characteristic_identity(
            land,
            Zone::Hand { owner: PlayerId(0) },
            &frozen.characteristics
        )
        .is_err()
    );
    recover(&lost);
    answer(
        &mut g,
        1,
        Response::ChooseManaAbility {
            ability: ManaAbilityIdentity::Printed { index: 1 },
        },
    );
    let Request::ManaColor { colors, .. } = request(&mut g, 1) else {
        panic!("chosen printed branch output");
    };
    assert_eq!(colors, [Color::U, Color::R]);
    assert!(!g.current(land).unwrap().tapped);
    recover(&g);
    answer(&mut g, 1, Response::ManaColor { color: Color::U });
    assert!(matches!(request(&mut g, 1), Request::Priority { .. }));
    assert_eq!(g.mana(PlayerId(0), Color::U), Some(1.into()));
    assert_eq!(g.state.rules.players[0].life, 39.into());
    recover(&g);
}
