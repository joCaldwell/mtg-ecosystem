//! Synthetic contract witnesses for paid artifact activations. CR 109.5, 121.2,
//! 602.2, 608.2c and 701.9a in the pinned edition:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

fn prepared_draw(cost: &ManaCost, discard: bool) -> Prepared {
    let draw = Instruction::draw(PlayerExpr::you(Context::Activated), 1.into(), 0);
    let mut instructions = vec![draw.clone()];
    if discard {
        instructions.extend([
            draw,
            Instruction::DiscardCards {
                recipient: None,
                count: 1.into(),
                source_block: 0,
            },
        ]);
    }
    let (catalog, artifact) = artifact_draw(cost, false, Vec::new(), instructions);
    assert!(
        artifact
            .capabilities
            .contains(&Capability::ActivatedArtifactDrawV1)
    );
    assert_eq!(
        artifact
            .capabilities
            .contains(&Capability::ControllerDiscardV1),
        discard
    );
    prepared_artifact(catalog, artifact, discard, usize::from(discard))
}

fn artifact_draw(
    cost: &ManaCost,
    sacrifice: bool,
    mana_abilities: Vec<ManaAbility>,
    instructions: Vec<Instruction>,
) -> (Catalog, Definition) {
    let catalog = Catalog::read(include_str!(
        "../../../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap();
    // Reuse the fixture's structural artifact fields and validated provenance.
    // The new identity and abilities are synthetic.
    let mut artifact = catalog
        .definitions
        .iter()
        .find(|definition| definition.name == "Arcane Signet")
        .unwrap()
        .clone();
    artifact.oracle_id = "synthetic-paid-artifact-draw".into();
    artifact.name = "Archive Flywheel".into();
    artifact.source_labels = None;
    artifact.mana_abilities = mana_abilities;
    artifact.activated = vec![ActivatedAbility {
        functioning_zone: FunctioningZone::Battlefield,
        cost: ActivationCost::Composite {
            costs: PermanentActivationCosts {
                tap: true,
                mana: Some(cost.clone()),
                sacrifice,
                remove_source_plus_one_counters: None,
            },
        },
        program: SpellProgram {
            additional_sacrifice: None,
            optional_additional_costs: vec![],
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            functioning_zone: FunctioningZone::Stack,
            targets: Vec::new(),
            instructions,
            modes: Vec::new(),
        },
    }];
    artifact.capabilities = artifact.requirements();
    (catalog, artifact)
}

fn prepared_artifact(
    mut catalog: Catalog,
    artifact: Definition,
    existing_hand_card: bool,
    extra_library_cards: usize,
) -> Prepared {
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
    let owner = PlayerId(0);
    let extra_zones = existing_hand_card
        .then_some(Zone::Hand { owner })
        .into_iter()
        .chain(std::iter::repeat_n(
            Zone::Library { owner },
            extra_library_cards,
        ));
    for zone in extra_zones {
        config.cards.push(CardSetup {
            id: CardId(config.cards.len() as u32),
            oracle_id: FOREST.into(),
            owner,
            controller: owner,
            zone,
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
fn paid_counted_draw() {
    let cost = activation_cost();
    let (catalog, artifact) = artifact_draw(
        &cost,
        true,
        vec![ManaAbility {
            restriction: None,
            self_damage: None,
            costs: PermanentActivationCosts {
                tap: true,
                mana: None,
                sacrifice: false,
                remove_source_plus_one_counters: None,
            },
            production: ManaProduction::Fixed {
                symbols: vec![Color::C; 3],
            },
            source_block: 0,
        }],
        vec![Instruction::draw(
            PlayerExpr::you(Context::Activated),
            3.into(),
            1,
        )],
    );
    let mut game = Game::new(prepared_artifact(catalog, artifact, false, 2));
    let activator = PlayerId(0);
    let owner = PlayerId(1);
    let source = object(&game, 0);
    let q = 1;
    let mut expected_libraries = game.state.rules.libraries.clone();
    let expected_draws = expected_libraries
        .get_mut(&activator)
        .unwrap()
        .drain(..3)
        .collect::<Vec<_>>();

    answer(
        &mut game,
        q,
        Response::ActivateAbility {
            object: source,
            ability: 0,
        },
    );
    let mana_request = request(&mut game, q);
    assert!(matches!(mana_request, Request::Mana { .. }));
    let before_illegal_mana = game.state.clone();
    assert!(
        game.respond(
            activator,
            mana_request.decision_id(),
            Response::ActivateMana { object: source },
        )
        .is_err()
    );
    assert_eq!(game.state, before_illegal_mana);
    for id in 1..=3 {
        let land = object(&game, id);
        answer(&mut game, q, Response::ActivateMana { object: land });
    }
    assert!(!game.current(source).unwrap().tapped);
    answer(&mut game, q, Response::FinishMana { undo_mana: false });
    assert!(matches!(request(&mut game, q), Request::SpendMana { .. }));
    answer(
        &mut game,
        q,
        Response::SpendMana {
            spending: BTreeMap::from([(Color::U, 3.into())]),
        },
    );
    assert!(matches!(request(&mut game, q), Request::Priority { .. }));
    assert_eq!(game.mana(activator, Color::U), Some(0.into()));
    let invocation = game.top().unwrap();
    assert_eq!(invocation.paid.as_ref(), Some(&cost));
    assert_eq!(invocation.controller, activator);
    let stack_id = invocation.id;
    assert!(game.current(source).is_err());
    let successor = game.current(object(&game, 0)).unwrap();
    assert_eq!(successor.zone, Zone::Graveyard { owner });
    assert!(game.events().iter().any(|event| matches!(
        event,
        Event::Committed { actual: EventOutcome::Tapped { object }, .. } if *object == source
    )));
    assert_eq!(game.state.rules.libraries[&activator].len(), 4);
    recover(&game);

    resolve(&mut game, q, |_| {});
    assert_eq!(game.state.rules.libraries, expected_libraries);
    assert_eq!(game.action_hand(activator).len(), 3);
    assert!(game.action_hand(owner).is_empty());
    let cause = BatchCause::Instruction {
        stack_id,
        player: activator,
        source,
        cursor: 0,
        source_block: 1,
    };
    let draws = game
        .trace()
        .records
        .iter()
        .filter_map(|record| match &record.record {
            EvidenceRecord::Batch { batch } if batch.cause == cause => {
                let [draw] = &batch.occurrences[..] else {
                    panic!("expected a single draw occurrence");
                };
                let EventOutcome::Moved { movement } = &draw.actual else {
                    panic!("expected a card to move");
                };
                assert_eq!(movement.from, Zone::Library { owner: activator });
                assert_eq!(movement.destination, Zone::Hand { owner: activator });
                Some(movement.previous.card)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(draws, expected_draws);
}

fn activation_cost() -> ManaCost {
    ManaCost {
        generic: 3.into(),
        colored: Vec::new(),
    }
}

fn produce_mana(game: &mut Game, q: u64) {
    for id in 1..=4 {
        let land = object(game, id);
        answer(game, q, Response::ActivateMana { object: land });
    }
    assert!(matches!(request(game, q), Request::Priority { .. }));
    assert_eq!(game.mana(PlayerId(0), Color::U), Some(3.into()));
    assert_eq!(game.mana(PlayerId(0), Color::G), Some(1.into()));
}

fn pay_for_activation(game: &mut Game, q: u64, source: ObjectId, cost: &ManaCost) {
    answer(
        game,
        q,
        Response::ActivateAbility {
            object: source,
            ability: 0,
        },
    );
    assert!(matches!(request(game, q), Request::Mana { .. }));
    assert!(!game.current(source).unwrap().tapped);
    answer(game, q, Response::FinishMana { undo_mana: false });
    assert!(matches!(request(game, q), Request::SpendMana { .. }));
    answer(
        game,
        q,
        Response::SpendMana {
            spending: BTreeMap::from([(Color::U, 2.into()), (Color::G, 1.into())]),
        },
    );
    assert!(matches!(request(game, q), Request::Priority { .. }));
    assert_eq!(game.top().unwrap().paid.as_ref(), Some(cost));
    assert!(game.current(source).unwrap().tapped);
}

#[test]
fn paid_artifact_draw_taps_source_and_draws_exactly_for_activator() {
    let cost = activation_cost();
    let mut game = Game::new(prepared_draw(&cost, false));
    let activator = PlayerId(0);
    let source = object(&game, 0);
    let q = 1;
    produce_mana(&mut game, q);
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
    pay_for_activation(&mut game, q, source, &cost);

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

#[test]
fn copied_artifact_pays_composite_cost_and_draws_for_activator() {
    let cost = activation_cost();
    let activator = PlayerId(0);
    let mut prepared = prepared_draw(&cost, false);
    let physical_card = CardId(prepared.config.cards.len() as u32);
    prepared.config.cards.push(CardSetup {
        id: physical_card,
        oracle_id: FOREST.into(),
        owner: activator,
        controller: activator,
        zone: Zone::Hand { owner: activator },
    });
    prepared.profile_digest = digest(&prepared.config).unwrap();
    let mut game = Game::new(prepared);
    let donor = object(&game, 0);
    let previous = game.object_id(physical_card).unwrap();
    let source = ObjectId {
        incarnation: previous.incarnation + 1,
        ..previous
    };
    // Set up the copied incarnation directly to exercise activation payment.
    assert!(game.definition(physical_card).unwrap().activated.is_empty());
    game.record_entry_copy(source, donor).unwrap();
    assert_eq!(
        game.move_object(previous, Zone::Battlefield)
            .unwrap()
            .successor,
        source
    );
    let top_card = game.state.rules.libraries[&activator][0];
    let q = 1;

    produce_mana(&mut game, q);
    pay_for_activation(&mut game, q, source, &cost);
    assert_eq!(game.top().unwrap().source, source);
    assert!(!game.current(donor).unwrap().tapped);
    assert_eq!(game.mana(activator, Color::U), Some(1.into()));
    assert_eq!(game.mana(activator, Color::G), Some(0.into()));

    resolve(&mut game, q, |_| {});
    assert_eq!(
        game.action_hand(activator),
        vec![game.object_id(top_card).unwrap()]
    );
}

#[test]
fn paid_artifact_draw_discard_offers_the_full_hand_after_two_draws() {
    let cost = activation_cost();
    let mut game = Game::new(prepared_draw(&cost, true));
    let activator = PlayerId(0);
    let source = object(&game, 0);
    let q = 1;
    let [old_card] = game.action_hand(activator)[..] else {
        panic!("expected an existing hand card");
    };
    let mut libraries_after = game.state.rules.libraries.clone();
    let drawn_cards = libraries_after
        .get_mut(&activator)
        .unwrap()
        .drain(..2)
        .collect::<Vec<_>>();
    produce_mana(&mut game, q);
    pay_for_activation(&mut game, q, source, &cost);
    assert_eq!(game.mana(activator, Color::U), Some(1.into()));
    assert_eq!(game.mana(activator, Color::G), Some(0.into()));
    for _ in 0..game.live_players().len() {
        assert!(matches!(request(&mut game, q), Request::Priority { .. }));
        answer(&mut game, q, Response::Pass);
    }

    assert!(matches!(
        request(&mut game, q),
        Request::ResolveDiscard { player, count: 1, .. } if player == activator
    ));
    assert_eq!(game.state.rules.libraries, libraries_after);
    let mut drawn = drawn_cards
        .into_iter()
        .map(|card| game.object_id(card).unwrap())
        .collect::<Vec<_>>();
    drawn.sort_unstable();
    let mut expected_choices = drawn.clone();
    expected_choices.push(old_card);
    expected_choices.sort_unstable();
    recover(&game);

    let QueryResult::Complete(QueryValue::Observation(own)) =
        game.observe(activator, Budget::unlimited())
    else {
        panic!("expected the activator's observation");
    };
    let decision = own.decision.unwrap();
    let PlayerRequest::ResolveDiscard {
        candidates,
        count: 1,
        ..
    } = &decision
    else {
        panic!("expected a choice of one discard");
    };
    let mut actual_choices = candidates
        .iter()
        .map(|handle| game.resolve_handle(activator, handle).unwrap())
        .collect::<Vec<_>>();
    actual_choices.sort_unstable();
    assert_eq!(actual_choices, expected_choices);
    for seat in 1..4 {
        let viewer = PlayerId(seat);
        let QueryResult::Complete(QueryValue::Observation(other)) =
            game.observe(viewer, Budget::unlimited())
        else {
            panic!("expected the other seat's observation");
        };
        assert!(other.decision.is_none());
        for card in &expected_choices {
            assert!(game.handle_for(viewer, *card).is_err());
        }
    }
    let chosen = game.handle_for(activator, old_card).unwrap();
    let QueryResult::Complete(QueryValue::Validated(receipt)) = game.check_player_response(
        activator,
        decision.token().clone(),
        PlayerResponse::ResolveDiscard {
            objects: vec![chosen],
        },
        Budget::unlimited(),
    ) else {
        panic!("expected the existing hand card to be discardable");
    };
    game.submit(receipt).unwrap();

    assert!(matches!(request(&mut game, q), Request::Priority { .. }));
    assert!(game.state.rules.stack.is_empty());
    assert_eq!(game.state.rules.libraries, libraries_after);
    let mut hand_after = game.action_hand(activator);
    hand_after.sort_unstable();
    assert_eq!(hand_after, drawn);
    let discarded = game.object_id(old_card.card).unwrap();
    assert_eq!(discarded.incarnation, old_card.incarnation + 1);
    assert_eq!(
        game.current(discarded).unwrap().zone,
        Zone::Graveyard { owner: activator }
    );
    recover(&game);
}
