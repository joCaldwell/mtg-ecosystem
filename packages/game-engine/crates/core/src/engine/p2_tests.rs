use super::*;
fn prepared() -> Prepared {
    Prepared::admit(
        Catalog::read(include_str!("../../../../tests/fixtures/expected-ir.json")).unwrap(),
        ScenarioConfig::land_return(),
    )
    .unwrap()
}
fn inputs() -> Vec<AcceptedInput> {
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/expected-payment-scenario.json"
    ))
    .unwrap();
    serde_json::from_value(expected["accepted_inputs"].clone()).unwrap()
}
fn finish_query(game: &Game, mut result: QueryResult) -> QueryValue {
    for _ in 0..1000 {
        result = match result {
            QueryResult::Complete(value) => return value,
            QueryResult::Incomplete {
                work,
                required_minimum,
            } => {
                assert_eq!(required_minimum, 1);
                game.resume_work(work, Budget::new(1))
            }
            QueryResult::Rejected(reason) => panic!("query rejected: {reason:?}"),
        };
    }
    panic!("query made no progress")
}
fn observe(game: &Game, viewer: PlayerId) -> Observation {
    match finish_query(game, game.observe(viewer, Budget::new(0))) {
        QueryValue::Observation(o) => o,
        _ => panic!("wrong query result"),
    }
}
fn context(game: &Game, viewer: PlayerId) -> SearchContext {
    match finish_query(
        game,
        game.search_context(viewer, "separate-sampling-seed".into(), Budget::new(1)),
    ) {
        QueryValue::SearchContext(c) => c,
        _ => panic!("wrong query result"),
    }
}
#[test]
fn execution_budget_partitions_and_every_suspended_boundary_restore_exactly() {
    let p = prepared();
    let input = inputs();
    let mut reference = Game::new(p.clone());
    for i in &input {
        let _ = reference.advance(Budget::unlimited());
        reference
            .respond(i.player, i.decision_id, i.response.clone())
            .unwrap();
    }
    let _ = reference.advance(Budget::unlimited());
    let expected = Game::replay(p.clone(), &reference.replay_journal()).unwrap();
    for units in [1, 2, 3, 7, u64::MAX] {
        let mut game = Game::new(p.clone());
        let mut index = 0;
        let mut suspensions = 0;
        loop {
            let before = game.state_digest().unwrap();
            if game.state.pending.is_some()
                || matches!(
                    game.state.frame,
                    Frame::Payment { .. } | Frame::Resolve { .. }
                )
            {
                assert!(matches!(
                    game.advance(Budget::new(0)),
                    Advance::Suspended {
                        required_minimum: 1
                    }
                ));
                assert_eq!(before, game.state_digest().unwrap());
            }
            match game.advance(Budget::new(units)) {
                Advance::AwaitingDecision(_) if index == input.len() => break,
                Advance::AwaitingDecision(request) => {
                    let i = &input[index];
                    assert_eq!(request.player(), i.player);
                    assert_eq!(request.decision_id(), i.decision_id);
                    game.respond(i.player, i.decision_id, i.response.clone())
                        .unwrap();
                    index += 1;
                }
                Advance::Suspended { .. } => {
                    suspensions += 1;
                }
                Advance::Finished { .. } => panic!("unexpected finished fixture"),
                Advance::Draw => panic!("unexpected draw"),
                Advance::Fault(e) => panic!("execution fault: {e}"),
            }
            let snapshot = game.snapshot().unwrap();
            let restored = Game::restore(p.clone(), &snapshot).unwrap();
            assert_eq!(restored.state, game.state);
            game = restored;
        }
        assert_eq!(expected.state, game.state);
        assert_eq!(expected.events(), game.events());
        if units == 1 {
            assert!(suspensions >= 2);
        }
    }
}
#[test]
fn query_budgets_work_sidecars_and_foreign_work_are_pure() {
    let p = prepared();
    let mut game = Game::new(p.clone());
    let before = game.state_digest().unwrap();
    let QueryResult::Incomplete { work, .. } = game.observe(PlayerId(0), Budget::new(2)) else {
        panic!("expected partial observation")
    };
    let saved = game.save_work(&work).unwrap();
    let restored = Game::restore(p.clone(), &game.snapshot().unwrap()).unwrap();
    assert!(matches!(
        restored.resume_work(work, Budget::unlimited()),
        QueryResult::Rejected(QueryRejection::StaleOrForeignWork)
    ));
    let work = restored.restore_work(&saved).unwrap();
    let QueryValue::Observation(recovered) =
        finish_query(&restored, restored.resume_work(work, Budget::new(1)))
    else {
        panic!("wrong observation")
    };
    assert_eq!(recovered, observe(&game, PlayerId(0)));
    let mut tampered: serde_json::Value = serde_json::from_str(&saved).unwrap();
    tampered["data"]["units_done"] = 99999999_u64.into();
    assert!(
        restored
            .restore_work(&canonical(&tampered).unwrap())
            .is_err()
    );
    let mut tampered: serde_json::Value = serde_json::from_str(&saved).unwrap();
    tampered["data"]["task"]["builder"]["view"]["players"][0]["life"] = "999".into();
    assert!(
        restored
            .restore_work(&canonical(&tampered).unwrap())
            .is_err()
    );
    for viewer in [3, 1, 0, 2, 0] {
        let _ = observe(&game, PlayerId(viewer));
        let _ = finish_query(
            &game,
            game.player_decision_for(PlayerId(viewer), Budget::new(0)),
        );
        let _ = context(&game, PlayerId(viewer));
    }
    assert_eq!(before, game.state_digest().unwrap());
    let QueryResult::Incomplete { work, .. } = game.observe(PlayerId(0), Budget::new(0)) else {
        panic!("expected work")
    };
    let receipt = game.check_response(PlayerId(0), 0, Response::Pass).unwrap();
    game.submit(receipt).unwrap();
    assert!(matches!(
        game.resume_work(work, Budget::new(1)),
        QueryResult::Rejected(QueryRejection::StaleOrForeignWork)
    ));
    assert!(game.restore_work(&saved).is_err());
}
#[test]
fn player_types_hide_other_hands_and_library_identities_and_frames() {
    let mut game = Game::new(prepared());
    let own = observe(&game, PlayerId(0));
    let other = observe(&game, PlayerId(1));
    assert_eq!(own.view.objects.len(), 3);
    assert_eq!(other.view.objects.len(), 2);
    assert_eq!(other.view.players[0].hand_count, 1);
    assert!(!other.view.objects.iter().any(|o| o.oracle_id == SPELL));
    let json = canonical(&other).unwrap();
    for forbidden in [
        "incarnation",
        "source_json",
        "state_sha256",
        "physical",
        "frame",
        "journal",
    ] {
        assert!(!json.contains(forbidden));
    }
    // Test-only privacy fixture; no library gameplay or new configuration is
    // admitted. Even its owner cannot see a hidden library's card identity.
    game.state.objects.get_mut(&CardId(1)).unwrap().zone = Zone::Library { owner: PlayerId(0) };
    game.refresh_visibility().unwrap();
    let o = observe(&game, PlayerId(0));
    assert_eq!(o.view.players[0].library_count, 1);
    assert!(!o.view.objects.iter().any(|o| o.oracle_id == SPELL));
}
#[test]
fn identical_permitted_histories_produce_identical_contexts_with_different_hidden_ids() {
    let original = Game::new(prepared());
    let mut other = original.fork();
    let physical = other.state.cards.remove(&CardId(1)).unwrap();
    let mut object = other.state.objects.remove(&CardId(1)).unwrap();
    object.id = ObjectId {
        card: CardId(99),
        incarnation: 42,
    };
    other.state.cards.insert(CardId(99), physical);
    other.state.objects.insert(CardId(99), object);
    other.refresh_visibility().unwrap();
    assert_ne!(
        original.state_digest().unwrap(),
        other.state_digest().unwrap()
    );
    assert_eq!(
        observe(&original, PlayerId(1)),
        observe(&other, PlayerId(1))
    );
    assert_eq!(
        context(&original, PlayerId(1)),
        context(&other, PlayerId(1))
    );
    let json = canonical(&context(&original, PlayerId(1))).unwrap();
    for forbidden in [
        "incarnation",
        "state_sha256",
        "initial_snapshot",
        "frame",
        "rule_steps",
        "game_seed",
    ] {
        assert!(!json.contains(forbidden));
    }
}
#[test]
fn player_handles_tokens_receipts_and_empty_domains_are_checked() {
    let mut game = Game::new(prepared());
    let own = observe(&game, PlayerId(0));
    let foreign = observe(&game, PlayerId(1));
    let token = own.decision.unwrap().token().clone();
    let own_land = own
        .view
        .objects
        .iter()
        .find(|o| o.oracle_id == ISLAND)
        .unwrap()
        .handle
        .clone();
    let foreign_land = foreign
        .view
        .objects
        .iter()
        .find(|o| o.oracle_id == ISLAND)
        .unwrap()
        .handle
        .clone();
    assert!(matches!(
        game.check_player_response(
            PlayerId(0),
            token.clone(),
            PlayerResponse::ActivateMana {
                object: foreign_land
            },
            Budget::unlimited()
        ),
        QueryResult::Rejected(QueryRejection::InvalidResponse)
    ));
    let receipt = match finish_query(
        &game,
        game.check_player_response(
            PlayerId(0),
            token.clone(),
            PlayerResponse::ActivateMana { object: own_land },
            Budget::new(1),
        ),
    ) {
        QueryValue::Validated(r) => r,
        _ => panic!("missing receipt"),
    };
    let stale = game.check_response(PlayerId(0), 0, Response::Pass).unwrap();
    game.submit(receipt).unwrap();
    assert!(game.submit(stale).is_err());
    let _ = game.advance(Budget::unlimited());
    assert!(matches!(
        game.check_player_response(
            PlayerId(0),
            token,
            PlayerResponse::Pass,
            Budget::unlimited()
        ),
        QueryResult::Rejected(QueryRejection::InvalidResponse)
    ));
    game.respond(PlayerId(0), 1, Response::Cast { card: CardId(1) })
        .unwrap();
    let _ = game.advance(Budget::unlimited());
    assert!(matches!(
        game.search_context(PlayerId(0), "seed".into(), Budget::unlimited()),
        QueryResult::Rejected(QueryRejection::UnsupportedBoundary)
    ));
    let stale_handle = observe(&game, PlayerId(0))
        .view
        .objects
        .iter()
        .find(|o| o.oracle_id == BEARS)
        .unwrap()
        .handle
        .clone();
    let old = game.object_id(CardId(2)).unwrap();
    game.move_object(old, Zone::Hand { owner: PlayerId(1) })
        .unwrap();
    game.refresh_visibility().unwrap();
    let QueryResult::Incomplete { work, .. } =
        game.player_decision_for(PlayerId(0), Budget::new(0))
    else {
        panic!("incomplete is not an empty domain")
    };
    let QueryValue::Decision(Some(PlayerRequest::Targets {
        token, candidates, ..
    })) = finish_query(&game, game.resume_work(work, Budget::new(1)))
    else {
        panic!("missing target request")
    };
    assert!(candidates.is_empty());
    assert!(matches!(
        game.check_player_response(
            PlayerId(0),
            token,
            PlayerResponse::Targets {
                objects: vec![stale_handle]
            },
            Budget::unlimited()
        ),
        QueryResult::Rejected(QueryRejection::InvalidResponse)
    ));
    assert!(
        observe(&game, PlayerId(0))
            .view
            .objects
            .iter()
            .all(|o| o.oracle_id != BEARS)
    );
}
#[test]
fn query_work_invalidates_during_automatic_execution_even_without_input_revision_change() {
    let mut game = Game::new(prepared());
    for i in inputs().iter().take(4) {
        let _ = game.advance(Budget::unlimited());
        game.respond(i.player, i.decision_id, i.response.clone())
            .unwrap();
    }
    assert!(matches!(
        game.advance(Budget::new(1)),
        Advance::Suspended { .. }
    ));
    let revision = game.state.revision;
    let QueryResult::Incomplete { work, .. } = game.observe(PlayerId(0), Budget::new(1)) else {
        panic!("missing work")
    };
    let _ = game.advance(Budget::new(1));
    assert_eq!(game.state.revision, revision);
    assert!(matches!(
        game.resume_work(work, Budget::unlimited()),
        QueryResult::Rejected(QueryRejection::StaleOrForeignWork)
    ));
}
