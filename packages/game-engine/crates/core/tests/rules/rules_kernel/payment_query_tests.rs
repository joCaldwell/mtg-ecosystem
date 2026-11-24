use super::*;

fn payment_game() -> (Prepared, Game) {
    let prepared = FixtureCatalog::Base.admit(ScenarioConfig::land_return());
    let mut game = Game::new(prepared.clone());
    Driver::cast(&mut game, CardId(1));
    let decision = Driver::request(&mut game, u64::MAX);
    let target = game.object_id(CardId(2)).unwrap();
    game.respond(decision.player, decision.id, decision.targets(vec![target]))
        .unwrap();
    assert!(matches!(
        game.advance(Budget::unlimited()),
        Advance::AwaitingDecision(Decision {
            reason: Reason::Mana { .. },
            ..
        })
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
    assert_eq!(
        own.observation_version,
        mtg_core::testkit::formats::OBSERVATION_VERSION
    );
    assert_eq!(own.view.priority, None);
    let Some(Decision {
        player,
        shape: Shape::Pay { cost, .. },
        reason: Reason::Mana { .. },
        ..
    }) = own.decision
    else {
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
    let token = own.decision.unwrap().id.clone();
    let land = own
        .view
        .objects
        .iter()
        .find(|o| o.oracle_id == "b2c6aa39-2d2a-459c-a555-fb48ba993373")
        .unwrap()
        .handle
        .clone();
    for response in [
        Answer::Pick {
            picks: vec![vec![0]],
        },
        game.next_test_viewer_decision().select(Candidate::Action {
            action: Action::Cast { card: land.clone() },
        }),
        game.next_test_viewer_decision().targets(vec![land]),
        game.next_test_viewer_decision()
            .activate_mana(ObjectHandle("v1-o0".into())),
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
            Answer::Pay {
                pay: PayAnswer::Finish {
                    undo_on_failure: true
                }
            },
            Budget::unlimited()
        ),
        QueryResult::Rejected(QueryRejection::InvalidResponse)
    ));
    for undo_mana in [false, true] {
        assert!(matches!(
            game.check_player_response(
                PlayerId(0),
                token.clone(),
                Answer::Pay {
                    pay: PayAnswer::Finish {
                        undo_on_failure: undo_mana
                    }
                },
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
    let token = own.decision.unwrap().id.clone();
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
        game.next_test_viewer_decision().activate_mana(land),
        Budget::new(1),
    ) else {
        panic!("expected incomplete activation validation")
    };
    let saved = game.save_work(&work).unwrap();
    let saved_value: serde_json::Value = serde_json::from_str(&saved).unwrap();
    assert_eq!(
        saved_value["work_version"],
        mtg_core::testkit::formats::WORK_VERSION
    );
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
        Advance::AwaitingDecision(Decision {
            reason: Reason::Mana { .. },
            ..
        })
    ));
    assert!(matches!(
        game.check_player_response(
            PlayerId(0),
            token,
            Answer::Pay {
                pay: PayAnswer::Finish {
                    undo_on_failure: true
                }
            },
            Budget::unlimited()
        ),
        QueryResult::Rejected(QueryRejection::InvalidResponse)
    ));
}
