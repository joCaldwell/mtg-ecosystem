//! Independent ordinary-trigger scheduling, departure, and recovery regressions.
use mtg_contracts::{Catalog, Color, Quantity};
use mtg_core::*;
use std::collections::BTreeMap;
const ARTIST: &str = "310f141c-7f37-4729-aed6-dd9c09db448d";
const HAPLESS: &str = "240a349a-79f8-4e11-9a05-a78148e70a62";
const VISIONARY: &str = "c6a3a882-a127-4590-93d7-679ef4313efe";
const STIFLE: &str = "b3b00911-ece7-4484-bc36-f211ce72b6cc";
const ISLAND: &str = "b2c6aa39-2d2a-459c-a555-fb48ba993373";
const PLAINS: &str = "bc71ebf6-2056-41f7-be35-b2e5c34afa99";
const MOUNTAIN: &str = "a3fb7228-e76b-4e96-a40e-20b5fed75685";
const SWAMP: &str = "56719f6a-1a6c-4c0a-8d21-18f7d7350b68";
const FOREST: &str = "b34bb2dc-c1af-4d77-b0b3-a0fb342a5fc6";
fn prepared(config: ScenarioConfig) -> Prepared {
    Prepared::admit(
        Catalog::read(include_str!(
            "../../../tests/fixtures/expected-trigger-ir.json"
        ))
        .unwrap(),
        config,
    )
    .unwrap()
}
fn complete(game: &Game, mut result: QueryResult) -> QueryValue {
    for _ in 0..2000 {
        match result {
            QueryResult::Complete(value) => return value,
            QueryResult::Incomplete { work, .. } => result = game.resume_work(work, Budget::new(1)),
            QueryResult::Rejected(reason) => panic!("query rejected {reason:?}"),
        }
    }
    panic!("query did not finish")
}
fn observe(game: &Game, viewer: PlayerId) -> Observation {
    let QueryValue::Observation(view) = complete(game, game.observe(viewer, Budget::new(0))) else {
        panic!("observation")
    };
    view
}
fn request(game: &Game) -> (PlayerId, PlayerRequest) {
    game.live_players()
        .into_iter()
        .find_map(
            |player| match complete(game, game.player_decision_for(player, Budget::new(0))) {
                QueryValue::Decision(Some(req)) => Some((player, req)),
                QueryValue::Decision(None) => None,
                _ => panic!("decision"),
            },
        )
        .expect("missing decision")
}
fn verify(game: &Game, p: &Prepared) {
    let restored = Game::restore(p.clone(), &game.snapshot().unwrap()).unwrap();
    assert_eq!(
        restored.state_digest().unwrap(),
        game.state_digest().unwrap()
    );
    assert_eq!(restored.trace(), game.trace());
}
fn settle(game: &mut Game, p: &Prepared) -> Advance {
    for _ in 0..100 {
        let result = game.advance(Budget::new(1));
        verify(game, p);
        if !matches!(result, Advance::Suspended { .. }) {
            return result;
        }
    }
    panic!("automatic work did not settle")
}
fn submit(game: &mut Game, response: PlayerResponse) {
    let (actor, req) = request(game);
    let QueryValue::Validated(receipt) = complete(
        game,
        game.check_player_response(actor, req.token().clone(), response, Budget::new(0)),
    ) else {
        panic!("validated")
    };
    game.submit(receipt).unwrap();
}
fn input(game: &mut Game, p: &Prepared, response: PlayerResponse) {
    submit(game, response);
    verify(game, p);
    assert!(matches!(settle(game, p), Advance::AwaitingDecision(_)));
}
fn reject(game: &Game, actor: PlayerId, token: DecisionToken, response: PlayerResponse) {
    let before = game.snapshot().unwrap();
    let mut result = game.check_player_response(actor, token, response, Budget::new(0));
    for _ in 0..2000 {
        match result {
            QueryResult::Rejected(_) => {
                assert_eq!(before, game.snapshot().unwrap());
                return;
            }
            QueryResult::Incomplete { work, .. } => result = game.resume_work(work, Budget::new(1)),
            QueryResult::Complete(_) => panic!("illegal response accepted"),
        }
    }
    panic!("rejection did not settle")
}
fn handle(game: &Game, viewer: PlayerId, oracle: &str, zone: Zone) -> ObjectHandle {
    observe(game, viewer)
        .view
        .objects
        .into_iter()
        .find(|o| o.oracle_id == oracle && o.zone == zone)
        .expect("visible card")
        .handle
}
fn replay_equal(game: &Game, p: &Prepared) {
    let replay = Game::replay(p.clone(), &game.replay_journal()).unwrap();
    assert_eq!(replay.state_digest().unwrap(), game.state_digest().unwrap());
}
fn spend(game: &mut Game, p: &Prepared, amounts: &[(Color, i64)]) {
    input(
        game,
        p,
        PlayerResponse::SpendMana {
            spending: amounts
                .iter()
                .map(|(c, n)| (*c, Quantity::from(*n)))
                .collect::<BTreeMap<_, _>>(),
        },
    );
}
fn fund_all(game: &mut Game, p: &Prepared) {
    let actor = request(game).0;
    loop {
        let source = observe(game, actor).view.objects.into_iter().find(|o| {
            o.zone == Zone::Battlefield
                && o.controller == actor
                && !o.tapped
                && [ISLAND, PLAINS, MOUNTAIN, SWAMP, FOREST].contains(&o.oracle_id.as_str())
        });
        let Some(source) = source else { break };
        input(
            game,
            p,
            PlayerResponse::ActivateMana {
                object: source.handle,
            },
        );
    }
    input(game, p, PlayerResponse::FinishMana { undo_mana: true });
}
fn round(game: &mut Game, p: &Prepared) {
    for _ in 0..game.live_players().len() {
        input(game, p, PlayerResponse::Pass);
    }
}
fn passive_until(game: &mut Game, p: &Prepared, wanted: impl Fn(&Game, &PlayerRequest) -> bool) {
    for _ in 0..250 {
        let req = request(game).1;
        if wanted(game, &req) {
            verify(game, p);
            return;
        }
        let response = match req {
            PlayerRequest::Priority { .. } => PlayerResponse::Pass,
            PlayerRequest::Attackers { .. } => PlayerResponse::Attackers { attacks: vec![] },
            PlayerRequest::Blockers { .. } => PlayerResponse::Blockers { blocks: vec![] },
            _ => panic!("unexpected passive setup"),
        };
        submit(game, response);
        for _ in 0..100 {
            match game.advance(Budget::new(1)) {
                Advance::Suspended { .. } => continue,
                Advance::AwaitingDecision(_) => break,
                other => panic!("unexpected passive setup outcome {other:?}"),
            }
        }
    }
    panic!("wanted boundary not reached")
}

fn life(game: &Game, player: PlayerId) -> Quantity {
    observe(game, PlayerId(0))
        .view
        .players
        .iter()
        .find(|p| p.player == player)
        .unwrap()
        .life
        .clone()
}
fn begin_spell(game: &mut Game, p: &Prepared, oracle: &str) {
    let actor = request(game).0;
    let card = handle(game, actor, oracle, Zone::Hand { owner: actor });
    input(game, p, PlayerResponse::Cast { card });
}
fn player_target(game: &mut Game, p: &Prepared, player: PlayerId) {
    input(
        game,
        p,
        PlayerResponse::PlayerTargets {
            players: vec![player],
        },
    );
}
fn begin_researcher(game: &mut Game, p: &Prepared) {
    let actor = request(game).0;
    let object = handle(game, actor, HAPLESS, Zone::Battlefield);
    input(
        game,
        p,
        PlayerResponse::ActivateAbility { object, ability: 0 },
    );
    assert!(matches!(
        request(game).1,
        PlayerRequest::OrderTriggers { .. }
    ));
}
fn order(game: &mut Game, p: &Prepared, reversed: bool) {
    let PlayerRequest::OrderTriggers { candidates, .. } = request(game).1 else {
        panic!("ordering");
    };
    let mut handles = candidates.into_iter().map(|t| t.handle).collect::<Vec<_>>();
    if reversed {
        handles.reverse();
    }
    input(game, p, PlayerResponse::OrderTriggers { order: handles });
}
fn place_all(game: &mut Game, p: &Prepared) {
    for _ in 0..100 {
        match request(game).1 {
            PlayerRequest::OrderTriggers { .. } => order(game, p, false),
            PlayerRequest::PlayerTargets { .. } => player_target(game, p, PlayerId(3)),
            PlayerRequest::Priority { .. } => return,
            other => panic!("placement request {other:?}"),
        }
    }
    panic!("placement did not finish")
}
fn resolve_all(game: &mut Game, p: &Prepared) {
    for _ in 0..100 {
        match request(game).1 {
            PlayerRequest::Priority { .. } if observe(game, PlayerId(0)).view.stack.is_empty() => {
                return;
            }
            PlayerRequest::Priority { .. } => input(game, p, PlayerResponse::Pass),
            PlayerRequest::OrderTriggers { .. } => order(game, p, false),
            PlayerRequest::PlayerTargets { .. } => player_target(game, p, PlayerId(3)),
            PlayerRequest::ResolveDiscard { candidates, .. } => input(
                game,
                p,
                PlayerResponse::ResolveDiscard {
                    objects: vec![candidates[0].clone()],
                },
            ),
            other => panic!("resolution request {other:?}"),
        }
    }
    panic!("resolution did not finish")
}

#[test]
fn accepted_order_survives_borrowed_source_owner_departure_without_losing_survivor_trigger() {
    for accepted in [false, true] {
        let p = prepared(ScenarioConfig::triggers_borrowed());
        let mut game = Game::new(p.clone());
        begin_researcher(&mut game, &p);
        let (actor, req) = request(&game);
        let PlayerRequest::OrderTriggers { candidates, .. } = &req else {
            panic!("order");
        };
        assert_eq!(actor, PlayerId(0));
        assert_eq!(candidates.len(), 2);
        let order_handles = candidates.iter().rev().map(|c| c.handle.clone()).collect();
        let response = PlayerResponse::OrderTriggers {
            order: order_handles,
        };
        let QueryValue::Validated(receipt) = complete(
            &game,
            game.check_player_response(actor, req.token().clone(), response, Budget::new(0)),
        ) else {
            panic!("receipt");
        };
        if accepted {
            game.submit(receipt).unwrap();
            game.concede(PlayerId(2)).unwrap();
        } else {
            game.concede(PlayerId(2)).unwrap();
            assert!(game.submit(receipt).is_err());
        }
        assert!(matches!(
            settle(&mut game, &p),
            Advance::AwaitingDecision(_)
        ));
        if accepted {
            assert!(matches!(
                request(&game).1,
                PlayerRequest::PlayerTargets { .. }
            ));
        } else {
            let PlayerRequest::OrderTriggers { candidates, .. } = request(&game).1 else {
                panic!("retained order");
            };
            assert_eq!(candidates.len(), 2);
            assert_eq!(
                candidates
                    .iter()
                    .filter(|c| c.source.current.is_none())
                    .count(),
                1
            );
            order(&mut game, &p, true);
        }
        place_all(&mut game, &p);
        let stack = observe(&game, PlayerId(0)).view.stack;
        assert_eq!(stack.iter().filter(|s| s.triggered.is_some()).count(), 4);
        assert_eq!(
            stack
                .iter()
                .filter_map(|s| s.triggered.as_ref())
                .filter(|t| t.source.current.is_none())
                .count(),
            1
        );
        assert!(stack.iter().all(|s| s.controller != PlayerId(2)));
        resolve_all(&mut game, &p);
        assert_eq!(life(&game, PlayerId(0)), Quantity::from(42));
        assert_eq!(life(&game, PlayerId(1)), Quantity::from(41));
        assert_eq!(life(&game, PlayerId(3)), Quantity::from(37));
        replay_equal(&game, &p);
    }
}

#[test]
fn controller_departure_discards_accepted_order_and_its_pending_objects_without_death_retrigger() {
    for accepted in [false, true] {
        let p = prepared(ScenarioConfig::triggers_borrowed());
        let mut game = Game::new(p.clone());
        begin_researcher(&mut game, &p);
        if accepted {
            let PlayerRequest::OrderTriggers { candidates, .. } = request(&game).1 else {
                panic!("order");
            };
            submit(
                &mut game,
                PlayerResponse::OrderTriggers {
                    order: candidates.into_iter().rev().map(|c| c.handle).collect(),
                },
            );
        }
        game.concede(PlayerId(0)).unwrap();
        assert!(matches!(
            settle(&mut game, &p),
            Advance::AwaitingDecision(_)
        ));
        assert_eq!(request(&game).0, PlayerId(1));
        place_all(&mut game, &p);
        let view = observe(&game, PlayerId(1)).view;
        assert_eq!(view.active, None);
        assert_eq!(view.turn_owner, PlayerId(0));
        assert_eq!(view.priority, Some(PlayerId(1)));
        assert_eq!(view.stack.len(), 3);
        assert!(
            view.stack
                .iter()
                .all(|s| s.triggered.is_some() && s.controller != PlayerId(0))
        );
        assert_eq!(
            game.zone(game.object_id(CardId(1)).unwrap()),
            Some(Zone::Exile)
        );
        resolve_all(&mut game, &p);
        assert_eq!(life(&game, PlayerId(1)), Quantity::from(41));
        assert_eq!(life(&game, PlayerId(2)), Quantity::from(41));
        assert_eq!(life(&game, PlayerId(3)), Quantity::from(38));
        replay_equal(&game, &p);
    }
}

#[test]
fn order_handles_permutations_query_work_and_validation_receipts_are_executor_scoped() {
    let p = prepared(ScenarioConfig::triggers());
    let mut game = Game::new(p.clone());
    begin_researcher(&mut game, &p);
    let (actor, req) = request(&game);
    let PlayerRequest::OrderTriggers { candidates, .. } = &req else {
        panic!("order");
    };
    let before = game.snapshot().unwrap();
    let view = observe(&game, actor);
    for _ in 0..3 {
        assert_eq!(view, observe(&game, actor));
    }
    assert_eq!(before, game.snapshot().unwrap());
    for bad in [
        vec![],
        vec![candidates[0].handle.clone()],
        vec![candidates[0].handle.clone(), candidates[0].handle.clone()],
    ] {
        reject(
            &game,
            actor,
            req.token().clone(),
            PlayerResponse::OrderTriggers { order: bad },
        );
    }
    reject(
        &game,
        PlayerId(1),
        req.token().clone(),
        PlayerResponse::OrderTriggers {
            order: candidates.iter().map(|c| c.handle.clone()).collect(),
        },
    );
    let response = PlayerResponse::OrderTriggers {
        order: candidates.iter().rev().map(|c| c.handle.clone()).collect(),
    };
    let QueryResult::Incomplete { work, .. } =
        game.check_player_response(actor, req.token().clone(), response.clone(), Budget::new(0))
    else {
        panic!("budgeted work");
    };
    let saved = game.save_work(&work).unwrap();
    let json: serde_json::Value = serde_json::from_str(&saved).unwrap();
    assert_eq!(json["work_version"], 11);
    let mut forged = json;
    forged["data"]["units_done"] = serde_json::json!(1);
    assert!(
        game.restore_work(&serde_json::to_string(&forged).unwrap())
            .is_err()
    );
    let mut restored = Game::restore(p.clone(), &before).unwrap();
    let work = restored.restore_work(&saved).unwrap();
    let QueryValue::Validated(receipt) =
        complete(&restored, restored.resume_work(work, Budget::new(1)))
    else {
        panic!("recovered receipt");
    };
    assert!(game.submit(receipt).is_err());
    assert_eq!(before, game.snapshot().unwrap());
    let work = restored.restore_work(&saved).unwrap();
    let QueryValue::Validated(receipt) =
        complete(&restored, restored.resume_work(work, Budget::new(1)))
    else {
        panic!("local receipt");
    };
    restored.submit(receipt).unwrap();
    assert!(matches!(
        settle(&mut restored, &p),
        Advance::AwaitingDecision(_)
    ));
    input(&mut game, &p, response);
    assert_eq!(
        game.state_digest().unwrap(),
        restored.state_digest().unwrap()
    );
    place_all(&mut game, &p);
    resolve_all(&mut game, &p);
    replay_equal(&game, &p);
}

#[test]
fn pending_and_incomplete_triggers_grant_no_priority_but_completed_entry_is_stifle_target() {
    let p = prepared(ScenarioConfig::triggers());
    let mut game = Game::new(p.clone());
    begin_researcher(&mut game, &p);
    let stack = observe(&game, PlayerId(0)).view.stack;
    assert_eq!(stack.len(), 1);
    assert!(stack[0].ability.is_some());
    assert!(stack[0].triggered.is_none());
    order(&mut game, &p, false);
    let (actor, req) = request(&game);
    assert_eq!(actor, PlayerId(0));
    assert!(matches!(req, PlayerRequest::PlayerTargets { .. }));
    reject(&game, actor, req.token().clone(), PlayerResponse::Pass);
    reject(
        &game,
        actor,
        req.token().clone(),
        PlayerResponse::FinishMana { undo_mana: false },
    );
    let stack = observe(&game, actor).view.stack;
    assert_eq!(stack.len(), 2);
    assert!(stack.last().unwrap().triggered.is_some());
    assert!(!stack.last().unwrap().cast_complete);
    player_target(&mut game, &p, PlayerId(3));
    place_all(&mut game, &p);
    let stack = observe(&game, PlayerId(1)).view.stack;
    assert_eq!(stack.len(), 6);
    assert!(stack.iter().all(|s| s.cast_complete));
    let target = stack
        .iter()
        .find(|s| s.triggered.is_some() && s.controller == PlayerId(0))
        .unwrap()
        .handle
        .clone()
        .unwrap();
    input(&mut game, &p, PlayerResponse::Pass);
    begin_spell(&mut game, &p, STIFLE);
    let PlayerRequest::StackTargets { candidates, .. } = request(&game).1 else {
        panic!("counter targets");
    };
    assert_eq!(candidates.len(), 6);
    assert!(candidates.contains(&target));
    input(
        &mut game,
        &p,
        PlayerResponse::StackTargets {
            entries: vec![target],
        },
    );
    fund_all(&mut game, &p);
    spend(&mut game, &p, &[(Color::U, 1)]);
    round(&mut game, &p);
    assert_eq!(
        observe(&game, PlayerId(0))
            .view
            .stack
            .iter()
            .filter(|s| s.triggered.is_some())
            .count(),
        4
    );
    assert!(game.events().iter().any(|e| matches!(
        e,
        Event::Committed {
            actual: EventOutcome::StackObjectCountered {
                spell: false,
                movement: None,
                ..
            },
            ..
        }
    )));
    resolve_all(&mut game, &p);
    assert_eq!(life(&game, PlayerId(0)), Quantity::from(41));
    assert_eq!(life(&game, PlayerId(3)), Quantity::from(37));
    assert_eq!(
        game.events()
            .iter()
            .filter(|e| matches!(e, Event::TriggeredAbilityCompleted { .. }))
            .count(),
        4
    );
    replay_equal(&game, &p);
}

#[test]
fn pending_provenance_and_placement_continuation_forgeries_are_not_authorized_by_rehash() {
    let p = prepared(ScenarioConfig::triggers());
    let mut game = Game::new(p.clone());
    begin_researcher(&mut game, &p);
    let before: serde_json::Value = serde_json::from_str(&game.snapshot().unwrap()).unwrap();
    assert_eq!(
        before["state"]["pending_triggers"]
            .as_array()
            .unwrap()
            .len(),
        5
    );
    for mutation in 0..5 {
        let mut forged = before.clone();
        match mutation {
            0 => forged["state"]["pending_triggers"][0]["controller"] = serde_json::json!(3),
            1 => forged["state"]["pending_triggers"][0]["event"]["path"] = serde_json::json!([500]),
            2 => {
                forged["state"]["pending_triggers"][0]["source_facts"]["tapped"] =
                    serde_json::json!(true)
            }
            3 => {
                forged["state"]["pending_triggers"][1]["id"] =
                    forged["state"]["pending_triggers"][0]["id"].clone()
            }
            _ => {
                forged["state"]["frame"]["placement"]["remaining"] = serde_json::json!([1, 0, 2, 3])
            }
        }
        forged["state_sha256"] =
            serde_json::json!(mtg_contracts::digest(&forged["state"]).unwrap());
        assert!(Game::restore(p.clone(), &serde_json::to_string(&forged).unwrap()).is_err());
    }
    order(&mut game, &p, false);
    let before: serde_json::Value = serde_json::from_str(&game.snapshot().unwrap()).unwrap();
    for mutation in 0..3 {
        let mut forged = before.clone();
        let last = forged["state"]["stack"].as_array().unwrap().len() - 1;
        match mutation {
            0 => forged["state"]["stack"][last]["kind"]["trigger_id"] = serde_json::json!(999),
            1 => forged["state"]["frame"]["stack_id"] = serde_json::json!(999),
            _ => {
                forged["state"]["stack"][last]["source_facts"]["controller"] = serde_json::json!(3)
            }
        }
        forged["state_sha256"] =
            serde_json::json!(mtg_contracts::digest(&forged["state"]).unwrap());
        assert!(Game::restore(p.clone(), &serde_json::to_string(&forged).unwrap()).is_err());
    }
    place_all(&mut game, &p);
    resolve_all(&mut game, &p);
    replay_equal(&game, &p);
}

#[test]
fn simultaneous_dying_observers_each_capture_each_occurrence_once_before_any_observer_moves() {
    const BEARS: &str = "14c8f55d-d177-4c25-a931-ebeb9e6062a0";
    let p = prepared(ScenarioConfig::triggers());
    let mut game = Game::new(p.clone());
    passive_until(&mut game, &p, |_, r| {
        matches!(r, PlayerRequest::Attackers { .. })
    });
    let artists = observe(&game, PlayerId(0))
        .view
        .objects
        .into_iter()
        .filter(|o| {
            o.oracle_id == ARTIST && o.controller == PlayerId(0) && o.zone == Zone::Battlefield
        })
        .map(|o| o.handle)
        .collect::<Vec<_>>();
    assert_eq!(artists.len(), 2);
    input(
        &mut game,
        &p,
        PlayerResponse::Attackers {
            attacks: vec![
                (artists[0].clone(), PlayerId(1)),
                (artists[1].clone(), PlayerId(2)),
            ],
        },
    );
    for player in [PlayerId(1), PlayerId(2)] {
        passive_until(&mut game, &p, |_, r| {
            matches!(r, PlayerRequest::Blockers { .. })
        });
        let (actor, req) = request(&game);
        assert_eq!(actor, player);
        let PlayerRequest::Blockers { attackers, .. } = req else {
            panic!("blocks");
        };
        assert_eq!(attackers.len(), 1);
        let bear = observe(&game, player)
            .view
            .objects
            .into_iter()
            .find(|o| o.oracle_id == BEARS && o.controller == player && o.zone == Zone::Battlefield)
            .unwrap()
            .handle;
        input(
            &mut game,
            &p,
            PlayerResponse::Blockers {
                blocks: vec![(bear, attackers[0].clone())],
            },
        );
    }
    passive_until(&mut game, &p, |_, r| {
        matches!(r, PlayerRequest::OrderTriggers { .. })
    });
    let collected = game
        .events()
        .iter()
        .filter_map(|e| match e {
            Event::TriggerCollected {
                source,
                player,
                subject,
                occurrence,
                ..
            } => Some((source.card, *player, subject.card, occurrence)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(collected.len(), 10);
    let mut pairs = std::collections::BTreeSet::new();
    let mut batches = std::collections::BTreeSet::new();
    let mut occurrences = std::collections::BTreeSet::new();
    for (source, player, subject, event) in collected {
        assert!(pairs.insert((source, subject)));
        assert_eq!(
            player,
            match source.0 {
                1 | 2 => PlayerId(0),
                16 => PlayerId(1),
                28 => PlayerId(2),
                34 => PlayerId(3),
                _ => panic!("unexpected observer"),
            }
        );
        assert!([CardId(1), CardId(2)].contains(&subject));
        batches.insert(event.batch);
        occurrences.insert(serde_json::to_string(event).unwrap());
    }
    assert_eq!(batches.len(), 1);
    assert_eq!(occurrences.len(), 2);
    for source in [1, 2, 16, 28, 34] {
        for subject in [1, 2] {
            assert!(pairs.contains(&(CardId(source), CardId(subject))));
        }
    }
    let PlayerRequest::OrderTriggers { candidates, .. } = request(&game).1 else {
        panic!("order");
    };
    assert_eq!(candidates.len(), 4);
    assert!(candidates.iter().all(|c| c.source.current.is_none()));
    order(&mut game, &p, true);
    place_all(&mut game, &p);
    let stack = observe(&game, PlayerId(0)).view.stack;
    assert_eq!(stack.len(), 10);
    assert_eq!(
        stack.iter().map(|s| s.controller).collect::<Vec<_>>(),
        vec![
            PlayerId(0),
            PlayerId(0),
            PlayerId(0),
            PlayerId(0),
            PlayerId(1),
            PlayerId(1),
            PlayerId(2),
            PlayerId(2),
            PlayerId(3),
            PlayerId(3)
        ]
    );
    resolve_all(&mut game, &p);
    assert_eq!(life(&game, PlayerId(0)), Quantity::from(44));
    assert_eq!(life(&game, PlayerId(1)), Quantity::from(42));
    assert_eq!(life(&game, PlayerId(2)), Quantity::from(42));
    assert_eq!(life(&game, PlayerId(3)), Quantity::from(32));
    assert_eq!(
        game.events()
            .iter()
            .filter(|e| matches!(e, Event::TriggeredAbilityCompleted { .. }))
            .count(),
        10
    );
    replay_equal(&game, &p);
}

#[test]
fn etb_pending_source_handles_are_public_and_controller_departure_does_not_turn_leave_into_dies() {
    let p = prepared(ScenarioConfig::triggers());
    let mut game = Game::new(p.clone());
    begin_spell(&mut game, &p, VISIONARY);
    fund_all(&mut game, &p);
    spend(&mut game, &p, &[(Color::G, 1), (Color::U, 1)]);
    round(&mut game, &p);
    let PlayerRequest::OrderTriggers { candidates, .. } = request(&game).1 else {
        panic!("ETB order");
    };
    assert_eq!(candidates.len(), 2);
    assert!(candidates.iter().all(|c| c.source.current.is_some()));
    let visionary = handle(&game, PlayerId(0), VISIONARY, Zone::Battlefield);
    let draw = candidates
        .iter()
        .find(|c| c.oracle_id == VISIONARY)
        .unwrap();
    assert_eq!(draw.source.current, Some(visionary.clone()));
    assert_eq!(draw.source.announced, visionary);
    assert_eq!(
        game.events()
            .iter()
            .filter(|e| matches!(e, Event::TriggerCollected { .. }))
            .count(),
        4
    );
    let entered = game.object_id(CardId(4)).unwrap();
    assert!(game.events().iter().all(|event| match event {
        Event::TriggerCollected { subject, .. } => *subject == entered,
        _ => true,
    }));
    submit(
        &mut game,
        PlayerResponse::OrderTriggers {
            order: candidates.into_iter().rev().map(|c| c.handle).collect(),
        },
    );
    game.concede(PlayerId(0)).unwrap();
    assert!(matches!(
        settle(&mut game, &p),
        Advance::AwaitingDecision(_)
    ));
    place_all(&mut game, &p);
    let stack = observe(&game, PlayerId(1)).view.stack;
    assert_eq!(stack.len(), 2);
    assert!(
        stack
            .iter()
            .all(|s| s.triggered.is_some() && s.controller != PlayerId(0))
    );
    resolve_all(&mut game, &p);
    assert_eq!(life(&game, PlayerId(1)), Quantity::from(41));
    assert_eq!(life(&game, PlayerId(2)), Quantity::from(41));
    assert_eq!(life(&game, PlayerId(3)), Quantity::from(40));
    assert_eq!(
        game.events()
            .iter()
            .filter(|e| matches!(e, Event::TriggerCollected { .. }))
            .count(),
        4
    );
    replay_equal(&game, &p);
}

#[test]
fn consumed_trigger_handles_cannot_authorize_a_later_event_from_the_same_live_sources() {
    const PRODIGAL: &str = "5e961d15-5972-4e4b-9385-1cd7cd7c6bbe";
    let p = prepared(ScenarioConfig::triggers());
    let mut game = Game::new(p.clone());
    begin_researcher(&mut game, &p);
    let PlayerRequest::OrderTriggers { candidates, .. } = request(&game).1 else {
        panic!("first order");
    };
    let old = candidates.into_iter().map(|t| t.handle).collect::<Vec<_>>();
    order(&mut game, &p, false);
    place_all(&mut game, &p);
    resolve_all(&mut game, &p);
    let source = handle(&game, PlayerId(0), PRODIGAL, Zone::Battlefield);
    input(
        &mut game,
        &p,
        PlayerResponse::ActivateAbility {
            object: source.clone(),
            ability: 0,
        },
    );
    input(
        &mut game,
        &p,
        PlayerResponse::EntityTargets {
            entities: vec![PlayerTargetEntity::Object { object: source }],
        },
    );
    round(&mut game, &p);
    let (actor, req) = request(&game);
    let PlayerRequest::OrderTriggers { candidates, .. } = &req else {
        panic!("second order");
    };
    assert_eq!(candidates.len(), 2);
    assert!(candidates.iter().all(|t| !old.contains(&t.handle)));
    reject(
        &game,
        actor,
        req.token().clone(),
        PlayerResponse::OrderTriggers { order: old },
    );
    order(&mut game, &p, true);
    place_all(&mut game, &p);
    resolve_all(&mut game, &p);
    assert_eq!(
        game.events()
            .iter()
            .filter(|e| matches!(e, Event::TriggerCollected { .. }))
            .count(),
        10
    );
    assert_eq!(life(&game, PlayerId(0)), Quantity::from(44));
    assert_eq!(life(&game, PlayerId(3)), Quantity::from(32));
    replay_equal(&game, &p);
}
