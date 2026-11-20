//! Paid frozen Ramos source and recovery across CR 305.1–3/514.2.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::engine::temporary_indestructible_tests::through_cleanup;

const ACTIVE: PlayerId = PlayerId(0);
const OTHER: PlayerId = PlayerId(1);
const SPELL: CardId = CardId(0);
const OTHER_LAND: CardId = CardId(6);

fn prepared_urban_evolution() -> Prepared {
    let result = mtg_compiler::compile(
        include_str!("../../../../tests/fixtures/ramos/archive-manifest-current.json"),
        include_str!("../../../../tests/fixtures/ramos/c715e525-fc3a-4cab-94a0-04e0a22a5013.json"),
    );
    let mtg_compiler::CompileResult::Lowered { definition } = result else {
        panic!("{result:?}");
    };
    assert_eq!(
        definition.capabilities,
        BTreeSet::from([
            Capability::NormalLayoutV1,
            Capability::CastSorceryV1,
            Capability::ControllerDrawLandAllowanceV1,
        ])
    );
    assert_eq!(definition.characteristics.mana_cost.generic, 3.into());
    assert_eq!(
        definition.characteristics.mana_cost.colored,
        [Color::G, Color::U]
    );
    let program = definition.spell.as_ref().unwrap();
    assert!(program.optional_additional_costs.is_empty());
    assert_eq!(
        program.instructions,
        [
            Instruction::DrawControllerCards {
                count: 3.into(),
                source_block: 0
            },
            Instruction::GrantLandPlaysThisTurn {
                count: 1.into(),
                source_block: 0
            },
        ]
    );

    let mut catalog =
        Catalog::read(include_str!("../../../../tests/fixtures/expected-ir.json")).unwrap();
    let oracle_id = definition.oracle_id.clone();
    catalog.definitions.push(*definition);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    let mut config = ScenarioConfig::turns_permanents();
    config.profile = "draw-land-allowance-private-witness".into();
    config.initial.turn_step = TurnStep::PrecombatMain;
    config.commanders.clear();
    config.cards = vec![CardSetup {
        id: SPELL,
        oracle_id,
        owner: ACTIVE,
        controller: ACTIVE,
        zone: Zone::Hand { owner: ACTIVE },
    }];
    for (oracle_id, owner, zone) in [
        (ISLAND, ACTIVE, Zone::Battlefield),
        (FOREST, ACTIVE, Zone::Battlefield),
        (FOREST, ACTIVE, Zone::Battlefield),
        (FOREST, ACTIVE, Zone::Battlefield),
        (FOREST, ACTIVE, Zone::Battlefield),
        (FOREST, OTHER, Zone::Hand { owner: OTHER }),
        (FOREST, ACTIVE, Zone::Library { owner: ACTIVE }),
        (ISLAND, ACTIVE, Zone::Library { owner: ACTIVE }),
        (FOREST, ACTIVE, Zone::Library { owner: ACTIVE }),
        (ISLAND, OTHER, Zone::Library { owner: OTHER }),
    ] {
        config.cards.push(CardSetup {
            id: CardId(config.cards.len() as u32),
            oracle_id: oracle_id.into(),
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

fn offered_land(game: &Game, viewer: PlayerId, card: CardId) -> bool {
    let session = game.player_session(viewer);
    let QueryResult::Complete(QueryValue::Observation(observation)) =
        session.observe(Budget::unlimited())
    else {
        panic!("missing observation");
    };
    assert_eq!(observation.observation_version, 45);
    let token = observation.decision.unwrap().token().clone();
    let land = game
        .handle_for(viewer, game.object_id(card).unwrap())
        .unwrap();
    let result = session.action_beginnings(token, ActionScope::Window, 64, Budget::unlimited());
    let ActionQueryOutcome::Complete { page } = result.outcome() else {
        panic!("missing complete action query");
    };
    page.beginnings.iter().any(|beginning| {
        matches!(&beginning.initiation, ActionInitiation::PlayLand { object } if *object == land)
    })
}

#[test]
fn urban_evolution_paid_draw_then_two_land_plays_and_cleanup_replay() {
    let mut game = Game::with_trace(prepared_urban_evolution(), TraceMode::Detailed);
    let turn = game.turn().number;
    let drawn = game.state.rules.libraries[&ACTIVE]
        .iter()
        .copied()
        .collect::<Vec<_>>();
    announce(&mut game, 1, 0, vec![]);
    assert!(game.state.rules.stack.last().unwrap().paid.is_some());
    resolve(&mut game, 1, |_| {});
    assert!(game.state.rules.libraries[&ACTIVE].is_empty());
    assert_eq!(game.action_hand(ACTIVE).len(), drawn.len());
    let draw_order = game.events().iter().filter_map(|event| match event {
        Event::Committed {
            actual: EventOutcome::Moved { movement },
            ..
        } if movement.from == (Zone::Library { owner: ACTIVE })
            && movement.destination == (Zone::Hand { owner: ACTIVE }) =>
        {
            Some(movement.successor.card)
        }
        _ => None,
    });
    assert!(draw_order.eq(drawn.iter().copied()));
    assert_eq!(
        game.zone(game.object_id(SPELL).unwrap()),
        Some(Zone::Graveyard { owner: ACTIVE })
    );
    assert!(matches!(
        game.visible_effects(OTHER).unwrap().as_slice(),
        [VisibleEffect::AdditionalLandPlays { origin, count, expires_in_cleanup_of_turn, .. }]
            if origin.controller == ACTIVE && origin.source.current.is_none()
                && *count == 1.into() && *expires_in_cleanup_of_turn == turn
    ));
    recover(&game);

    let mut other_priority = game.fork();
    answer(&mut other_priority, 1, Response::Pass);
    let off_turn = request(&mut other_priority, 1);
    assert!(matches!(off_turn, Request::Priority { player: OTHER, .. }));
    assert!(!offered_land(&other_priority, OTHER, OTHER_LAND));
    assert!(
        other_priority
            .respond(
                OTHER,
                off_turn.decision_id(),
                Response::PlayLand { card: OTHER_LAND }
            )
            .is_err()
    );

    for (index, &card) in drawn[..2].iter().enumerate() {
        assert!(offered_land(&game, ACTIVE, card));
        answer(&mut game, 1, Response::PlayLand { card });
        assert!(matches!(
            request(&mut game, 1),
            Request::Priority { player: ACTIVE, .. }
        ));
        assert_eq!(
            game.zone(game.object_id(card).unwrap()),
            Some(Zone::Battlefield)
        );
        assert_eq!(game.turn().lands_played, (index + 1) as u32);
    }
    let third = drawn[2];
    assert!(!offered_land(&game, ACTIVE, third));
    let priority = request(&mut game, 1);
    let state = game.state_digest().unwrap();
    assert!(
        game.respond(
            ACTIVE,
            priority.decision_id(),
            Response::PlayLand { card: third }
        )
        .is_err()
    );
    assert_eq!(game.state_digest().unwrap(), state);
    recover(&game);

    for _ in 0..200 {
        match game.advance(Budget::new(1)) {
            Advance::Suspended { .. } => {}
            Advance::AwaitingDecision(r) => {
                game.respond(r.player(), r.decision_id(), pass(&r)).unwrap();
            }
            outcome => panic!("{outcome:?}"),
        }
        if game.turn().step == TurnStep::Cleanup && game.state.rules.effects.is_empty() {
            assert_eq!(game.turn().number, turn);
            assert_eq!(game.turn().lands_played, 2);
            assert!(game.visible_effects(OTHER).unwrap().is_empty());
            assert!(game.events().iter().any(|event| matches!(
                event,
                Event::Committed { actual: EventOutcome::EffectsCleaned { expired, .. }, .. }
                    if expired.len() == 1
            )));
            recover(&game);
            through_cleanup(&mut game, 1);
            assert_eq!(game.turn().lands_played, 0);
            recover(&game);
            return;
        }
    }
    panic!("land allowance did not expire in cleanup");
}
