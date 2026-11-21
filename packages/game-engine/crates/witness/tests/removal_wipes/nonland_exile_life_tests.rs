//! CR 601.2c/h, 608.2b/c and 701.13a, pinned 2026-09-25:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

fn observation(g: &Game, viewer: PlayerId) -> Observation {
    let QueryResult::Complete(QueryValue::Observation(observation)) =
        g.observe(viewer, Budget::unlimited())
    else {
        panic!("public observation unavailable");
    };
    observation
}

fn decision(g: &mut Game) -> (PlayerId, PlayerRequest) {
    let Advance::AwaitingDecision(request) = g.advance(Budget::new(10_000)) else {
        panic!("expected a decision");
    };
    let player = request.player();
    let QueryResult::Complete(QueryValue::Decision(Some(request))) =
        g.player_decision_for(player, Budget::unlimited())
    else {
        panic!("public decision unavailable");
    };
    (player, request)
}

fn submit(g: &mut Game, player: PlayerId, request: &PlayerRequest, response: PlayerResponse) {
    let QueryResult::Complete(QueryValue::Validated(receipt)) = g.check_player_response(
        player,
        request.token().clone(),
        response,
        Budget::unlimited(),
    ) else {
        panic!("public response rejected for {request:?}");
    };
    g.submit(receipt).unwrap();
}

fn answer(g: &mut Game, expected_player: PlayerId, response: PlayerResponse) {
    let (player, request) = decision(g);
    assert_eq!(player, expected_player);
    submit(g, player, &request, response);
}

fn object(view: &PermittedView, oracle_id: &str) -> ObjectHandle {
    view.objects
        .iter()
        .find(|o| o.oracle_id == oracle_id)
        .unwrap()
        .handle
        .clone()
}

fn finish_stack(g: &mut Game, viewer: PlayerId) -> PermittedView {
    for _ in 0..12 {
        let (player, request) = decision(g);
        let view = observation(g, viewer).view;
        if view.stack.is_empty() {
            return view;
        }
        assert!(matches!(request, PlayerRequest::Priority { .. }));
        submit(g, player, &request, PlayerResponse::Pass);
    }
    panic!("stack did not resolve within twelve priority decisions");
}

#[test]
fn nonland_exile_life_paid_public_resolution_and_fizzle() {
    let mtg_compiler::CompileResult::Lowered { definition } = mtg_witness::compile(
        include_str!("../../../../tests/fixtures/ramos/archive-manifest-current.json"),
        include_str!("../../../../tests/fixtures/ramos/ad09b3c3-c8e7-481c-8c45-e7f234935117.json"),
    ) else {
        panic!("complete frozen nonland exile spell must lower");
    };
    let exile_spell = definition.oracle_id.clone();
    let mut catalog = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap();
    catalog.definitions.push(*definition);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();

    let caster = PlayerId(0);
    let opponent = PlayerId(1);
    let mut config = ScenarioConfig::turns_permanents();
    config.profile = "nonland-exile-life-source-witness".into();
    config.initial.turn_step = TurnStep::PrecombatMain;
    config.cards = [
        (exile_spell.as_str(), caster, Zone::Hand { owner: caster }),
        (PLAINS, caster, Zone::Battlefield),
        (PLAINS, caster, Zone::Battlefield),
        (SWAMP, caster, Zone::Battlefield),
        (BEARS, opponent, Zone::Battlefield),
        (SPELL, opponent, Zone::Hand { owner: opponent }),
        (ISLAND, opponent, Zone::Battlefield),
    ]
    .into_iter()
    .enumerate()
    .map(|(id, (oracle_id, owner, zone))| CardSetup {
        id: CardId(id as u32),
        oracle_id: oracle_id.into(),
        owner,
        controller: owner,
        zone,
    })
    .collect();
    let mut game = Game::with_trace(
        Prepared {
            catalog_digest: digest(&catalog).unwrap(),
            profile_digest: digest(&config).unwrap(),
            catalog: Arc::new(catalog),
            config,
            match_config: None,
        },
        TraceMode::Detailed,
    );
    let initial = observation(&game, caster).view;
    let target = object(&initial, BEARS);
    let land = object(&initial, SWAMP);
    let lands: Vec<_> = initial
        .objects
        .iter()
        .filter(|o| o.controller == caster && (o.oracle_id == PLAINS || o.oracle_id == SWAMP))
        .map(|o| o.handle.clone())
        .collect();
    assert_eq!(lands.len(), 3);
    answer(
        &mut game,
        caster,
        PlayerResponse::Cast {
            card: object(&initial, &exile_spell),
        },
    );
    let (player, request) = decision(&mut game);
    let PlayerRequest::Targets { candidates, .. } = &request else {
        panic!("expected nonland target announcement");
    };
    assert_eq!(player, caster);
    assert_eq!(candidates.as_slice(), std::slice::from_ref(&target));
    assert!(matches!(
        game.check_player_response(
            caster,
            request.token().clone(),
            PlayerResponse::Targets {
                objects: vec![land]
            },
            Budget::unlimited(),
        ),
        QueryResult::Rejected(QueryRejection::InvalidResponse)
    ));
    submit(
        &mut game,
        caster,
        &request,
        PlayerResponse::Targets {
            objects: vec![target.clone()],
        },
    );
    for land in lands {
        answer(
            &mut game,
            caster,
            PlayerResponse::ActivateMana { object: land },
        );
    }
    answer(
        &mut game,
        caster,
        PlayerResponse::FinishMana { undo_mana: false },
    );
    let (player, request) = decision(&mut game);
    let PlayerRequest::SpendMana {
        cost, available, ..
    } = &request
    else {
        panic!("expected printed mana payment");
    };
    let spending = BTreeMap::from([(Color::W, 2.into()), (Color::B, 1.into())]);
    assert_eq!(player, caster);
    assert_eq!(cost.generic, 1.into());
    assert_eq!(cost.colored, [Color::W, Color::B]);
    assert_eq!(available, &spending);
    submit(
        &mut game,
        caster,
        &request,
        PlayerResponse::SpendMana { spending },
    );
    let _ = decision(&mut game);
    assert!(observation(&game, caster).view.stack[0].cast_complete);
    let mut fizzled = game.fork();

    let resolved = finish_stack(&mut game, caster);
    let exiled = resolved
        .objects
        .iter()
        .find(|o| o.oracle_id == BEARS)
        .unwrap();
    assert_eq!(exiled.zone, Zone::Exile);
    assert_ne!(exiled.handle, target);
    assert!(resolved.players[1].exile.contains(&exiled.handle));
    assert_eq!(resolved.players[0].life, 37.into());
    assert_eq!(resolved.players[1].life, 40.into());
    let exile_event = game
        .events()
        .iter()
        .position(|event| {
            matches!(event, Event::Committed { actual: EventOutcome::Moved { movement }, .. }
                if movement.destination == Zone::Exile)
        })
        .unwrap();
    let life_event = game
        .events()
        .iter()
        .position(|event| {
            matches!(event, Event::Committed { actual: EventOutcome::LifeLost { player, amount, .. }, .. }
                if *player == caster && *amount == 3.into())
        })
        .unwrap();
    assert!(exile_event < life_event);

    answer(&mut fizzled, caster, PlayerResponse::Pass);
    let view = observation(&fizzled, opponent).view;
    answer(
        &mut fizzled,
        opponent,
        PlayerResponse::Cast {
            card: object(&view, SPELL),
        },
    );
    answer(
        &mut fizzled,
        opponent,
        PlayerResponse::Targets {
            objects: vec![object(&view, BEARS)],
        },
    );
    answer(
        &mut fizzled,
        opponent,
        PlayerResponse::ActivateMana {
            object: object(&view, ISLAND),
        },
    );
    answer(
        &mut fizzled,
        opponent,
        PlayerResponse::FinishMana { undo_mana: false },
    );
    answer(
        &mut fizzled,
        opponent,
        PlayerResponse::SpendMana {
            spending: BTreeMap::from([(Color::U, 1.into())]),
        },
    );
    let view = finish_stack(&mut fizzled, opponent);
    assert!(
        view.objects
            .iter()
            .any(|o| { o.oracle_id == BEARS && o.zone == (Zone::Hand { owner: opponent }) })
    );
    assert!(
        view.objects.iter().any(|o| {
            o.oracle_id == exile_spell && o.zone == (Zone::Graveyard { owner: caster })
        })
    );
    assert!(view.players[1].exile.is_empty());
    assert_eq!(view.players[0].life, 40.into());
}
