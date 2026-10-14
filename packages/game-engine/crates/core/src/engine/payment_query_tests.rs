use super::*;

fn payment_game() -> (Prepared, Game) {
    let prepared = Prepared::admit(
        Catalog::read(include_str!("../../../../tests/fixtures/expected-ir.json")).unwrap(),
        ScenarioConfig::land_return(),
    )
    .unwrap();
    let mut game = Game::new(prepared.clone());
    for response in [
        Response::Cast { card: CardId(1) },
        Response::Targets {
            objects: vec![ObjectId {
                card: CardId(2),
                incarnation: 0,
            }],
        },
    ] {
        let Advance::AwaitingDecision(request) = game.advance(Budget::unlimited()) else {
            panic!("expected casting decision")
        };
        game.respond(request.player(), request.decision_id(), response)
            .unwrap();
    }
    assert!(matches!(
        game.advance(Budget::unlimited()),
        Advance::AwaitingDecision(Request::Mana { .. })
    ));
    (prepared, game)
}

fn finish_query(game: &Game, mut result: QueryResult) -> QueryValue {
    for _ in 0..100 {
        result = match result {
            QueryResult::Complete(value) => return value,
            QueryResult::Incomplete { work, .. } => game.resume_work(work, Budget::new(1)),
            QueryResult::Rejected(reason) => panic!("query rejected: {reason:?}"),
        };
    }
    panic!("query did not finish")
}

fn observation(game: &Game, viewer: PlayerId) -> Observation {
    let QueryValue::Observation(observation) =
        finish_query(game, game.observe(viewer, Budget::new(0)))
    else {
        panic!("expected observation")
    };
    observation
}

#[test]
fn mana_window_is_caster_only_and_has_no_priority_recipient() {
    let (_, game) = payment_game();
    let before = game.state_digest().unwrap();
    let own = observation(&game, PlayerId(0));
    assert_eq!(own.observation_version, 6);
    assert_eq!(own.view.priority, None);
    let Some(PlayerRequest::Mana { player, cost, .. }) = own.decision else {
        panic!("expected player mana request")
    };
    assert_eq!(player, PlayerId(0));
    assert_eq!(
        cost,
        game.definition(CardId(1))
            .unwrap()
            .characteristics
            .mana_cost
    );
    for viewer in [PlayerId(1), PlayerId(2), PlayerId(3)] {
        let other = observation(&game, viewer);
        assert!(other.decision.is_none());
        assert_eq!(other.view.priority, None);
    }
    assert!(matches!(
        game.search_context(PlayerId(0), "caller-seed".into(), Budget::unlimited()),
        QueryResult::Rejected(QueryRejection::UnsupportedBoundary)
    ));
    assert_eq!(game.state_digest().unwrap(), before);
}

#[test]
fn mana_response_queries_reject_priority_target_and_foreign_handle_actions() {
    let (_, game) = payment_game();
    let own = observation(&game, PlayerId(0));
    let token = own.decision.unwrap().token().clone();
    let land = own
        .view
        .objects
        .iter()
        .find(|o| o.oracle_id == "b2c6aa39-2d2a-459c-a555-fb48ba993373")
        .unwrap()
        .handle
        .clone();
    for response in [
        PlayerResponse::Pass,
        PlayerResponse::Cast { card: land.clone() },
        PlayerResponse::Targets {
            objects: vec![land],
        },
        PlayerResponse::ActivateMana {
            object: ObjectHandle("v1-o0".into()),
        },
    ] {
        assert!(matches!(
            game.check_player_response(PlayerId(0), token.clone(), response, Budget::unlimited()),
            QueryResult::Rejected(QueryRejection::InvalidResponse)
        ));
    }
    assert!(matches!(
        game.check_player_response(
            PlayerId(1),
            token.clone(),
            PlayerResponse::FinishMana { undo_mana: true },
            Budget::unlimited()
        ),
        QueryResult::Rejected(QueryRejection::InvalidResponse)
    ));
    for undo_mana in [false, true] {
        assert!(matches!(
            game.check_player_response(
                PlayerId(0),
                token.clone(),
                PlayerResponse::FinishMana { undo_mana },
                Budget::unlimited()
            ),
            QueryResult::Complete(QueryValue::Validated(_))
        ));
    }
}

#[test]
fn mana_validation_sidecar_recovers_and_activation_invalidates_queries() {
    let (prepared, mut game) = payment_game();
    let own = observation(&game, PlayerId(0));
    let token = own.decision.unwrap().token().clone();
    let land = own
        .view
        .objects
        .iter()
        .find(|o| o.oracle_id == "b2c6aa39-2d2a-459c-a555-fb48ba993373")
        .unwrap()
        .handle
        .clone();
    let QueryResult::Incomplete { work, .. } = game.check_player_response(
        PlayerId(0),
        token.clone(),
        PlayerResponse::ActivateMana { object: land },
        Budget::new(1),
    ) else {
        panic!("expected incomplete activation validation")
    };
    let saved = game.save_work(&work).unwrap();
    let saved_value: serde_json::Value = serde_json::from_str(&saved).unwrap();
    assert_eq!(saved_value["work_version"], 5);
    let restored = Game::restore(prepared, &game.snapshot().unwrap()).unwrap();
    let restored_work = restored.restore_work(&saved).unwrap();
    assert!(matches!(
        finish_query(
            &restored,
            restored.resume_work(restored_work, Budget::new(1))
        ),
        QueryValue::Validated(_)
    ));
    let QueryResult::Incomplete { work: stale, .. } =
        game.player_decision_for(PlayerId(0), Budget::new(0))
    else {
        panic!("expected suspended decision query")
    };
    let QueryValue::Validated(receipt) =
        finish_query(&game, game.resume_work(work, Budget::new(1)))
    else {
        panic!("expected activation receipt")
    };
    game.submit(receipt).unwrap();
    assert!(matches!(
        game.resume_work(stale, Budget::unlimited()),
        QueryResult::Rejected(QueryRejection::StaleOrForeignWork)
    ));
    assert!(matches!(
        game.advance(Budget::unlimited()),
        Advance::AwaitingDecision(Request::Mana { .. })
    ));
    assert!(matches!(
        game.check_player_response(
            PlayerId(0),
            token,
            PlayerResponse::FinishMana { undo_mana: true },
            Budget::unlimited()
        ),
        QueryResult::Rejected(QueryRejection::InvalidResponse)
    ));
}
