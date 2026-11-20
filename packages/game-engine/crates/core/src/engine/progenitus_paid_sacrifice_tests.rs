//! Private seeded starting position with Progenitus already on the battlefield.
//! CR 118.11, 601.2h, 614.6, 700.4, 701.24c, 903.9b.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

fn prepared() -> Prepared {
    let archive = std::fs::read_to_string(
        std::env::var_os("MTG_SELF_GRAVEYARD_SHUFFLE_ARCHIVE_PROBE")
            .expect("exact Progenitus archive"),
    )
    .unwrap();
    assert_eq!(
        mtg_contracts::sha256(archive.as_bytes()),
        "c5fbc3e89be7b8c69fa6b4e2a74deec2509b214a2484e1200f7ef86cf7f64ab2"
    );
    let mut lines = archive.lines();
    let compiled = mtg_compiler::compile(lines.next().unwrap(), lines.next().unwrap());
    assert!(lines.next().is_none());
    let mtg_compiler::CompileResult::Lowered { definition } = compiled else {
        panic!("complete Progenitus source must lower: {compiled:?}");
    };
    assert_eq!(definition.name, "Progenitus");
    let progenitus = definition.oracle_id.clone();
    let mut catalog = catalog();
    catalog.definitions.push(*definition);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    let rites = catalog
        .definitions
        .iter()
        .find(|d| d.name == "Village Rites")
        .unwrap()
        .oracle_id
        .as_str();

    // The normal opening setup still shuffles all four 99-card libraries.
    let mut config = ScenarioConfig::turns_permanents();
    config.profile = "progenitus-paid-sacrifice-private".into();
    config.mode = Mode::CommanderMatch;
    config.initial.turn_step = TurnStep::Untap;
    config.cards.clear();
    let decks = std::array::from_fn(|seat| {
        let owner = PlayerId(seat as u8);
        let commander = if seat == 0 {
            progenitus.as_str()
        } else {
            YARGLE
        };
        let mut counts = BTreeMap::new();
        for slot in 0..100 {
            let id = CardId((seat as u32) * 100 + slot);
            let (oracle_id, zone) = match (seat, slot) {
                (0, 0) => (commander, Zone::Battlefield),
                (_, 0) => (commander, Zone::Command { owner }),
                (0, 1) => (rites, Zone::Library { owner }),
                _ => (SWAMP, Zone::Library { owner }),
            };
            if slot == 0 {
                config.commanders.push(id);
            }
            config.cards.push(CardSetup {
                id,
                oracle_id: oracle_id.into(),
                owner,
                controller: owner,
                zone,
            });
            *counts.entry(oracle_id.to_owned()).or_default() += 1;
        }
        Decklist {
            commander: commander.into(),
            cards: counts
                .into_iter()
                .map(|(oracle_id, count)| DeckEntry { oracle_id, count })
                .collect(),
        }
    });
    let match_config = Some(MatchConfig {
        profile: config.profile.clone(),
        decks,
        starting_player: PlayerId(0),
        // The library-order witness's first repeating-byte seed that puts
        // physical slot 1 in seat 0's opening seven.
        seed: [2; 32],
    });
    Prepared {
        catalog_digest: digest(&catalog).unwrap(),
        catalog: Arc::new(catalog),
        profile_digest: digest(&(&config, &match_config)).unwrap(),
        config,
        match_config,
    }
}

#[test]
#[ignore = "uses the exact scoped archive"]
fn printed_replacement_pays_village_rites_sacrifice_and_shuffles_before_draw() {
    let owner = PlayerId(0);
    let mut game = Game::with_trace(prepared(), TraceMode::Detailed);
    main_for(&mut game, owner.0);
    let progenitus = game.object_id(CardId(0)).unwrap();
    let rites = game.object_id(CardId(1)).unwrap();
    let hand = Zone::Hand { owner };
    assert_eq!(game.zone(rites), Some(hand));
    let swamp = game
        .state
        .rules
        .objects
        .values()
        .find(|o| o.zone == hand && game.state.rules.cards[&o.id.card].oracle_id == SWAMP)
        .unwrap()
        .id
        .card;
    answer(&mut game, Response::PlayLand { card: swamp });
    let original_library = game.state.rules.libraries[&owner].clone();
    let random_before = game
        .state
        .rules
        .match_state
        .as_ref()
        .unwrap()
        .random
        .word_position;
    let payment_events = game.events().len();

    announce(&mut game, "Village Rites", owner.0, None, Some(progenitus));
    let Request::CommanderReturn {
        player,
        object,
        destination,
        ..
    } = request(&mut game)
    else {
        panic!("owning player's library-to-command replacement");
    };
    assert_eq!(
        (player, object, destination),
        (owner, progenitus, Zone::Library { owner })
    );
    assert!(matches!(
        game.state.rules.frame,
        Frame::GraveyardGroup { .. }
    ));
    assert_eq!(game.zone(progenitus), Some(Zone::Battlefield));
    assert!(game.top().unwrap().paid.is_none());
    assert_eq!(game.state.rules.libraries[&owner], original_library);
    let stack_id = game.top().unwrap().id;
    let payment_cause = BatchCause::Payment {
        stack_id,
        player: owner,
    };
    assert!(game.events()[payment_events..].iter().any(|event| matches!(
        event,
        Event::GraveyardReplacementRevealed { cause, objects }
            if *cause == payment_cause && objects.as_slice() == [progenitus]
    )));

    let QueryResult::Complete(QueryValue::Decision(Some(choice))) =
        game.player_decision_for(owner, Budget::unlimited())
    else {
        panic!("missing public Commander choice");
    };
    let QueryResult::Complete(QueryValue::Validated(receipt)) = game.check_player_response(
        owner,
        choice.token().clone(),
        PlayerResponse::CommanderReturn { to_command: true },
        Budget::unlimited(),
    ) else {
        panic!("owner's Commander choice should validate");
    };
    game.submit(receipt).unwrap();
    assert!(matches!(request(&mut game), Request::Priority { player, .. } if player == owner));
    let spell = game.top().unwrap();
    assert_eq!(spell.source.card, rites.card);
    assert_eq!(
        spell.paid,
        Some(ManaCost {
            generic: 0.into(),
            colored: vec![Color::B]
        })
    );
    assert_eq!(spell.sacrificed.as_ref().unwrap().object, progenitus);
    assert!(game.current(game.object_id(swamp).unwrap()).unwrap().tapped);
    assert_eq!(
        game.state.rules.players[0]
            .mana
            .get(&Color::B)
            .cloned()
            .unwrap_or_default(),
        0.into()
    );
    let successor = game.object_id(progenitus.card).unwrap();
    assert_ne!(successor, progenitus);
    let command = Zone::Command { owner };
    assert_eq!(game.zone(successor), Some(command));
    let sacrifices = game.events()[payment_events..]
        .iter()
        .filter_map(|event| match event {
            Event::Committed {
                actual: EventOutcome::Sacrificed { player, movement },
                ..
            } => Some((*player, movement.clone())),
            _ => None,
        })
        .collect::<Vec<_>>();
    // CR 700.4: the actual sacrifice never moves through the graveyard.
    assert_eq!(
        sacrifices,
        [(
            owner,
            MoveOutcome {
                previous: progenitus,
                successor,
                from: Zone::Battlefield,
                destination: command,
            }
        )]
    );

    let match_state = game.state.rules.match_state.as_ref().unwrap();
    assert_eq!(match_state.library_shuffles, [1, 0, 0, 0]);
    assert!(match_state.random.word_position.0 > random_before.0);
    let shuffled = game.state.rules.libraries[&owner].clone();
    assert_ne!(shuffled, original_library);
    assert_eq!(
        shuffled.iter().collect::<BTreeSet<_>>(),
        original_library.iter().collect::<BTreeSet<_>>()
    );

    settle(&mut game);
    assert_eq!(game.state.rules.libraries[&owner], shuffled[2..]);
    for &card in &shuffled[..2] {
        assert_eq!(game.zone(game.object_id(card).unwrap()), Some(hand));
    }
    assert_eq!(
        game.zone(game.object_id(rites.card).unwrap()),
        Some(Zone::Graveyard { owner })
    );
    check(&game);
}
