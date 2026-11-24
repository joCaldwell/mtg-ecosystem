//! Saved and resumed query Work: caches rehydrate only on the same validated
//! State, never cross executors or later states, and forged history, cursors,
//! budgets or build identities are rejected before any unit is accepted.
use super::*;

fn match_root() -> Game {
    let c = FixtureCatalog::Match.get().clone();
    let mut game = Game::with_trace(
        Prepared::admit_pod(
            c,
            MatchConfig::for_profile("commander-foundations-v1", [0; 32], PlayerId(0)).unwrap(),
        )
        .unwrap(),
        TraceMode::Off,
    );
    for _ in 0..100 {
        let Advance::AwaitingDecision(request) = game.advance(Budget::unlimited()) else {
            panic!("opening")
        };
        if matches!(
            request,
            Request::Priority {
                player: PlayerId(0),
                ..
            }
        ) && game.turn().step == TurnStep::PrecombatMain
        {
            return game;
        }
        let response = match request {
            Request::Mulligan { .. } => Response::Mulligan { take: false },
            Request::Priority { .. } => Response::Pass,
            _ => panic!("unexpected opening choice"),
        };
        game.respond(request.player(), request.decision_id(), response)
            .unwrap();
    }
    panic!("opening did not finish")
}
#[test]
fn optimized_work_caches_rehydrate_and_never_cross_executor_or_state_boundaries() {
    let mut game = match_root();
    let viewer = PlayerId(0);
    let QueryResult::Incomplete { work, .. } = game.observe(viewer, Budget::new(64)) else {
        panic!("mid observation")
    };
    assert!(work.cache.effects.is_some());
    assert!(work.cache.last_object.is_some());
    let saved = game.save_work(&work).unwrap();
    let fork = game.fork();
    assert!(matches!(
        fork.resume_work(work, Budget::unlimited()),
        QueryResult::Rejected(QueryRejection::StaleOrForeignWork)
    ));
    let recovered = fork.restore_work(&saved).unwrap();
    assert!(recovered.cache.effects.is_some());
    assert!(recovered.cache.last_object.is_some());
    let QueryResult::Complete(QueryValue::Observation(view)) =
        fork.resume_work(recovered, Budget::unlimited())
    else {
        panic!("recovered observation")
    };
    let QueryResult::Complete(QueryValue::Observation(expected)) =
        game.observe(viewer, Budget::unlimited())
    else {
        panic!("expected observation")
    };
    assert_eq!(view, expected);
    let mut tampered: serde_json::Value = serde_json::from_str(&saved).unwrap();
    tampered["data"]["task"]["builder"]["cursor"] = 1.into();
    assert!(game.restore_work(&tampered.to_string()).is_err());
    let QueryResult::Incomplete { work, .. } = game.observe(viewer, Budget::new(64)) else {
        panic!("new work")
    };
    game.concede(PlayerId(3)).unwrap();
    assert!(matches!(
        game.resume_work(work, Budget::unlimited()),
        QueryResult::Rejected(QueryRejection::StaleOrForeignWork)
    ));
    assert!(game.restore_work(&saved).is_err());
}

#[test]
fn shared_search_saved_prefix_recomputes_all_history_and_preserves_executor_authority() {
    let mut game = match_root();
    let viewer = PlayerId(0);
    let prefix_len = game.state.knowledge.visibility[0].history.len();
    assert!(prefix_len >= 2);
    let units = game.observation_work_units(viewer).unwrap() + prefix_len as u64 + 1;
    let QueryResult::Incomplete { work, .. } =
        game.search_context(viewer, "prefix-seed".into(), Budget::new(units))
    else {
        panic!("history prefix")
    };
    let Task::Search { history, .. } = &work.data.task else {
        panic!("search")
    };
    assert_eq!(history.len(), prefix_len);
    let saved = game.save_work(&work).unwrap();
    let wire: serde_json::Value = serde_json::from_str(&saved).unwrap();
    let rows = wire["data"]["task"]["history"].as_array().unwrap();
    let distinct = (1..rows.len())
        .find(|i| rows[0] != rows[*i])
        .expect("natural root retains distinct setup histories");
    for variant in 0..9 {
        let mut bad = wire.clone();
        match variant {
            0 => {
                bad["data"]["task"]["history"][0]["viewer"] = 3.into();
            }
            1 => {
                bad["data"]["task"]["history"][0]["turn"]["number"] = 123456.into();
            }
            2 => {
                bad["data"]["task"]["history"]
                    .as_array_mut()
                    .unwrap()
                    .remove(0);
            }
            3 => {
                bad["data"]["task"]["history"]
                    .as_array_mut()
                    .unwrap()
                    .swap(0, distinct);
            }
            4 => {
                bad["data"]["units_done"] = (units + 1).into();
            }
            5 => {
                bad["data"]["task"]["builder"]["cursor"] = 0.into();
            }
            6 => {
                bad["data"]["query"]["sampling_seed"] = "different".into();
            }
            7 => {
                bad["build_id"] = "foreign".into();
            }
            8 => {
                bad["data"]["task"]["history"][0]["extra"] = true.into();
            }
            _ => unreachable!(),
        }
        if variant == 6 {
            // Seed is caller-supplied query data, not authority: changing it
            // coherently before any dependent scratch is legal. It must remain
            // exact on completion, never be filled from actual hidden RNG.
            let recovered = game.restore_work(&bad.to_string()).unwrap();
            let QueryResult::Complete(QueryValue::SearchContext(c)) =
                game.resume_work(recovered, Budget::unlimited())
            else {
                panic!("seed-only recovery")
            };
            assert_eq!(c.sampling_seed, "different");
        } else {
            assert!(
                game.restore_work(&bad.to_string()).is_err(),
                "forgery {variant}"
            );
        }
    }
    let fork = game.fork();
    assert!(matches!(
        fork.resume_work(work, Budget::unlimited()),
        QueryResult::Rejected(QueryRejection::StaleOrForeignWork)
    ));
    // Explicit saved-work recovery rebinds to the validated same State; direct
    // work transfer cannot do so. Replay every original unit before accepting.
    let rebound = fork.restore_work(&saved).unwrap();
    assert!(matches!(
        fork.resume_work(rebound, Budget::unlimited()),
        QueryResult::Complete(QueryValue::SearchContext(_))
    ));
    game.concede(PlayerId(3)).unwrap();
    assert!(game.restore_work(&saved).is_err());
}
