use super::*;

fn fixture() -> Game {
    Game::new(
        Prepared::admit(
            Catalog::read(include_str!("../../../../tests/fixtures/expected-ir.json")).unwrap(),
            ScenarioConfig::land_return(),
        )
        .unwrap(),
    )
}
fn incomplete(result: QueryResult) -> Work {
    match result {
        QueryResult::Incomplete {
            work,
            required_minimum: 1,
        } => work,
        _ => panic!("expected suspended query work"),
    }
}

#[test]
fn zero_budget_defers_projection_and_request_headers_until_initialization_unit() {
    let mut game = fixture();
    // A deliberately invalid private projection fixture makes accidental header
    // construction observable. No snapshot/admission claim is made for it.
    game.state.rules.combat.push(Attacker {
        attacker: ObjectId {
            card: CardId(999),
            incarnation: 0,
        },
        defender: PlayerId(1),
        blocked: false,
        blockers: vec![],
    });
    let before = game.state.clone();
    let work = incomplete(game.observe(PlayerId(0), Budget::new(0)));
    assert!(matches!(work.data.task, Task::Initialize));
    assert_eq!(work.data.units_done, 0);
    assert!(work.cache.last_object.is_none());
    assert!(work.cache.effects.is_none());
    assert!(matches!(
        game.resume_work(work, Budget::new(1)),
        QueryResult::Rejected(QueryRejection::Invariant)
    ));
    assert_eq!(game.state, before);
}

#[test]
fn initialization_and_first_body_prefix_recover_for_every_query_kind() {
    let game = fixture();
    let token = game
        .request_header(PlayerId(0))
        .unwrap()
        .unwrap()
        .token()
        .clone();
    let queries = [
        Query::Observe {
            viewer: PlayerId(0),
        },
        Query::Decision {
            viewer: PlayerId(0),
        },
        Query::Check {
            viewer: PlayerId(0),
            token,
            response: PlayerResponse::Pass,
        },
        Query::Search {
            viewer: PlayerId(0),
            sampling_seed: "initialize-recovery".into(),
        },
    ];
    let restored = Game::restore((*game.prepared).clone(), &game.snapshot().unwrap()).unwrap();
    for query in queries {
        let mut work = game.start_work(query).unwrap();
        for initialized in [false, true] {
            assert_eq!(matches!(work.data.task, Task::Initialize), !initialized);
            let saved = game.save_work(&work).unwrap();
            let envelope: serde_json::Value = serde_json::from_str(&saved).unwrap();
            assert_eq!(envelope["work_version"], 16);
            let cold = restored.restore_work(&saved).unwrap();
            assert_eq!(cold.data, work.data);
            assert!(cold.belongs_to_viewer(PlayerId(0)));
            assert!(!cold.belongs_to_viewer(PlayerId(1)));
            let mut old = envelope.clone();
            old["work_version"] = 15.into();
            assert!(restored.restore_work(&old.to_string()).is_err());
            let mut changed = envelope;
            changed["data"]["units_done"] = (u64::from(initialized) + 1).into();
            assert!(restored.restore_work(&changed.to_string()).is_err());
            if !initialized {
                work = incomplete(game.resume_work(work, Budget::new(1)));
            }
        }
        let expected = game.resume_work(work, Budget::unlimited());
        assert!(matches!(expected, QueryResult::Complete(_)));
    }
}

#[test]
fn zero_budget_keeps_rejections_and_concession_invalidation() {
    let mut game = fixture();
    assert!(matches!(
        game.observe(PlayerId(4), Budget::new(0)),
        QueryResult::Rejected(QueryRejection::InvalidViewer)
    ));
    assert!(matches!(
        game.search_context(PlayerId(0), String::new(), Budget::new(0)),
        QueryResult::Rejected(QueryRejection::UnsupportedBoundary)
    ));
    let work = incomplete(game.player_decision_for(PlayerId(0), Budget::new(0)));
    let foreign = game.fork();
    let saved = game.save_work(&work).unwrap();
    assert!(matches!(
        foreign.resume_work(work, Budget::new(0)),
        QueryResult::Rejected(QueryRejection::StaleOrForeignWork)
    ));
    let work = game.restore_work(&saved).unwrap();
    game.concede(PlayerId(3)).unwrap();
    assert!(matches!(
        game.resume_work(work, Budget::new(0)),
        QueryResult::Rejected(QueryRejection::StaleOrForeignWork)
    ));
    assert!(matches!(
        game.observe(PlayerId(0), Budget::new(0)),
        QueryResult::Rejected(QueryRejection::UnsupportedBoundary)
    ));
    assert!(game.restore_work(&saved).is_err());
}

#[test]
fn decision_completion_has_one_initialization_unit_and_moves_its_request() {
    let game = fixture();
    let work = incomplete(game.player_decision_for(PlayerId(0), Budget::new(1)));
    assert_eq!(work.data.units_done, 1);
    let Task::Decision {
        request: Some(request),
        ..
    } = &work.data.task
    else {
        panic!("initialized decision");
    };
    let original_token_allocation = request.token().0.as_ptr();
    let QueryResult::Complete(QueryValue::Decision(Some(request))) =
        game.resume_work(work, Budget::new(1))
    else {
        panic!("priority decision after initialization and one body unit");
    };
    assert_eq!(request.token().0.as_ptr(), original_token_allocation);
}

#[test]
fn setup_zero_budget_rejects_automatic_work_and_defers_bottom_hand_candidates() {
    let prepared = Prepared::admit_pod(
        Catalog::read(include_str!(
            "../../../../tests/fixtures/expected-match-ir.json"
        ))
        .unwrap(),
        MatchConfig::for_profile("commander-foundations-v1", [0; 32], PlayerId(0)).unwrap(),
    )
    .unwrap();
    let mut game = Game::with_trace(prepared, TraceMode::Off);
    assert!(matches!(
        game.observe(PlayerId(0), Budget::new(0)),
        QueryResult::Rejected(QueryRejection::UnavailableDuringSetup)
    ));
    // Two mulligans followed by keep exercise the real bottom-opening request,
    // where the previous availability check enumerated hand candidates eagerly.
    let mut bottom = None;
    for _ in 0..16 {
        let Advance::AwaitingDecision(request) = game.advance(Budget::unlimited()) else {
            panic!("opening request");
        };
        if matches!(request, Request::BottomOpening { .. }) {
            bottom = Some(request);
            break;
        }
        let Request::Mulligan {
            mulligans_taken, ..
        } = request
        else {
            panic!("expected mulligan before bottom choice");
        };
        game.respond(
            request.player(),
            request.decision_id(),
            Response::Mulligan {
                take: mulligans_taken < 2,
            },
        )
        .unwrap();
    }
    let Request::BottomOpening {
        player,
        count,
        candidates,
        ..
    } = bottom.expect("bottom choice")
    else {
        unreachable!();
    };
    assert_eq!(count, 1);
    assert_eq!(candidates.len(), 7);
    let before = game.state.clone();
    let work = incomplete(game.player_decision_for(player, Budget::new(0)));
    assert!(matches!(work.data.task, Task::Initialize));
    assert_eq!(game.state, before);
    let work = incomplete(game.resume_work(work, Budget::new(1)));
    let Task::Decision {
        request: Some(PlayerRequest::BottomOpening { candidates, .. }),
        ..
    } = &work.data.task
    else {
        panic!("initialized bottom request");
    };
    assert!(
        candidates.is_empty(),
        "candidate rows remain budgeted after header construction"
    );
    let QueryResult::Complete(QueryValue::Decision(Some(PlayerRequest::BottomOpening {
        candidates,
        ..
    }))) = game.resume_work(work, Budget::unlimited())
    else {
        panic!("complete bottom request");
    };
    assert_eq!(candidates.len(), 7);
    assert_eq!(game.state, before);
}
