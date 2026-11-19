//! Synthetic contract witness for a paid artifact activation. CR 109.5, 121.2,
//! 602.2 and 608.2c in the pinned edition:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

fn prepared_draw(cost: &ManaCost) -> Prepared {
    let mut catalog = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap();
    // Reuse the fixture's structural artifact fields and validated provenance.
    // The new identity and activation are synthetic.
    let mut artifact = catalog
        .definitions
        .iter()
        .find(|definition| definition.name == "Arcane Signet")
        .unwrap()
        .clone();
    artifact.oracle_id = "synthetic-paid-artifact-draw".into();
    artifact.name = "Archive Flywheel".into();
    artifact.source_labels = None;
    artifact.mana_abilities.clear();
    artifact.activated = vec![ActivatedAbility {
        functioning_zone: FunctioningZone::Battlefield,
        cost: ActivationCost::Composite {
            costs: PermanentActivationCosts {
                tap: true,
                mana: Some(cost.clone()),
                sacrifice: false,
            },
        },
        program: SpellProgram {
            additional_sacrifice: None,
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            functioning_zone: FunctioningZone::Stack,
            targets: Vec::new(),
            instructions: vec![Instruction::DrawCards {
                recipient: None,
                count: 1.into(),
                source_block: 0,
            }],
            modes: Vec::new(),
        },
    }];
    artifact.capabilities = artifact.requirements();
    assert!(
        artifact
            .capabilities
            .contains(&Capability::ActivatedArtifactDrawV1)
    );
    let oracle_id = artifact.oracle_id.clone();
    catalog.definitions.push(artifact);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();

    let mut config = ScenarioConfig::turns_permanents();
    config.profile = "paid-artifact-draw-private".into();
    config.initial.turn_step = TurnStep::PrecombatMain;
    config.commanders.clear();
    config.cards = vec![CardSetup {
        id: CardId(0),
        oracle_id,
        owner: PlayerId(1),
        controller: PlayerId(0),
        zone: Zone::Battlefield,
    }];
    for (id, oracle_id) in [ISLAND, ISLAND, ISLAND, FOREST, FOREST]
        .into_iter()
        .enumerate()
    {
        config.cards.push(CardSetup {
            id: CardId(id as u32 + 1),
            oracle_id: oracle_id.into(),
            owner: PlayerId(0),
            controller: PlayerId(0),
            zone: Zone::Battlefield,
        });
    }
    for (id, owner) in [0, 0, 1, 2, 3].into_iter().enumerate() {
        let owner = PlayerId(owner);
        config.cards.push(CardSetup {
            id: CardId(id as u32 + 6),
            oracle_id: FOREST.into(),
            owner,
            controller: owner,
            zone: Zone::Library { owner },
        });
    }
    Prepared {
        catalog_digest: digest(&catalog).unwrap(),
        catalog: Arc::new(catalog),
        profile_digest: digest(&config).unwrap(),
        config,
        match_config: None,
    }
}

#[test]
fn paid_artifact_draw_taps_source_and_draws_exactly_for_activator() {
    let cost = ManaCost {
        generic: 3.into(),
        colored: Vec::new(),
    };
    let mut game = Game::new(prepared_draw(&cost));
    let activator = PlayerId(0);
    let source = object(&game, 0);
    let q = 1;
    for id in 1..=4 {
        let land = object(&game, id);
        answer(&mut game, q, Response::ActivateMana { object: land });
    }
    assert!(matches!(request(&mut game, q), Request::Priority { .. }));
    assert_eq!(game.mana(activator, Color::U), Some(3.into()));
    assert_eq!(game.mana(activator, Color::G), Some(1.into()));

    let mut battlefield_after = game
        .state
        .rules
        .objects
        .values()
        .filter(|object| object.zone == Zone::Battlefield)
        .cloned()
        .collect::<Vec<_>>();
    battlefield_after
        .iter_mut()
        .find(|object| object.id == source)
        .unwrap()
        .tapped = true;
    let mut libraries_after = game.state.rules.libraries.clone();
    let top = libraries_after.get_mut(&activator).unwrap().remove(0);
    let before_draw = game.object_id(top).unwrap();
    assert!(game.handle_for(activator, before_draw).is_err());
    answer(
        &mut game,
        q,
        Response::ActivateAbility {
            object: source,
            ability: 0,
        },
    );
    assert!(matches!(request(&mut game, q), Request::Mana { .. }));
    assert!(!game.current(source).unwrap().tapped);
    answer(&mut game, q, Response::FinishMana { undo_mana: false });
    assert!(matches!(request(&mut game, q), Request::SpendMana { .. }));
    answer(
        &mut game,
        q,
        Response::SpendMana {
            spending: BTreeMap::from([(Color::U, 2.into()), (Color::G, 1.into())]),
        },
    );
    assert!(matches!(request(&mut game, q), Request::Priority { .. }));
    assert_eq!(game.top().unwrap().paid.as_ref(), Some(&cost));
    assert!(game.current(source).unwrap().tapped);

    resolve(&mut game, q, |_| {});
    assert_eq!(game.mana(activator, Color::U), Some(1.into()));
    assert_eq!(game.mana(activator, Color::G), Some(0.into()));
    assert!(
        game.state
            .rules
            .objects
            .values()
            .filter(|object| object.zone == Zone::Battlefield)
            .eq(&battlefield_after)
    );
    assert_eq!(game.state.rules.libraries, libraries_after);
    let drawn = game.object_id(top).unwrap();
    assert_eq!(drawn.incarnation, before_draw.incarnation + 1);
    assert_eq!(game.action_hand(activator), vec![drawn]);
    assert!(game.handle_for(activator, drawn).is_ok());
    for seat in 1..4 {
        let viewer = PlayerId(seat);
        assert!(game.action_hand(viewer).is_empty());
        assert!(game.handle_for(viewer, drawn).is_err());
    }
    recover(&game);
}
