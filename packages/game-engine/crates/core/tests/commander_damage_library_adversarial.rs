//! Independent player-facing regression evidence for damage and hidden draws.
use mtg_contracts::{Catalog, Quantity};
use mtg_core::*;

const ISAMARU: &str = "cfb756dd-6fe1-430a-8f04-8b10cac62a86";
const BEARS: &str = "14c8f55d-d177-4c25-a931-ebeb9e6062a0";

fn library_prepared() -> Prepared {
    Prepared::admit(
        Catalog::read(include_str!(
            "../../../tests/fixtures/expected-library-ir.json"
        ))
        .unwrap(),
        ScenarioConfig::commander_library(),
    )
    .unwrap()
}
fn damage_prepared(borrowed: bool) -> Prepared {
    Prepared::admit(
        Catalog::read(include_str!(
            "../../../tests/fixtures/expected-damage-ir.json"
        ))
        .unwrap(),
        if borrowed {
            ScenarioConfig::commander_damage_borrowed()
        } else {
            ScenarioConfig::commander_damage()
        },
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
    panic!("query did not complete")
}
fn observe(game: &Game, player: PlayerId) -> Observation {
    let QueryValue::Observation(view) = complete(game, game.observe(player, Budget::new(0))) else {
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
fn verify(game: &Game, prepared: &Prepared) {
    let restored = Game::restore(prepared.clone(), &game.snapshot().unwrap()).unwrap();
    assert_eq!(
        restored.state_digest().unwrap(),
        game.state_digest().unwrap()
    );
    assert_eq!(restored.trace(), game.trace());
}
fn settle(game: &mut Game, prepared: &Prepared) -> Advance {
    for _ in 0..100 {
        let result = game.advance(Budget::new(1));
        verify(game, prepared);
        if !matches!(result, Advance::Suspended { .. }) {
            return result;
        }
    }
    panic!("automatic work did not finish")
}
fn receipt(game: &Game, response: PlayerResponse) -> ValidatedResponse {
    let (actor, req) = request(game);
    let QueryValue::Validated(receipt) = complete(
        game,
        game.check_player_response(actor, req.token().clone(), response, Budget::new(0)),
    ) else {
        panic!("validated")
    };
    receipt
}
fn submit(game: &mut Game, response: PlayerResponse) {
    let receipt = receipt(game, response);
    game.submit(receipt).unwrap();
}
fn input(game: &mut Game, prepared: &Prepared, response: PlayerResponse) {
    submit(game, response);
    verify(game, prepared);
    assert!(matches!(
        settle(game, prepared),
        Advance::AwaitingDecision(_)
    ));
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
    panic!("rejection did not finish")
}
fn replay_equal(game: &Game, prepared: &Prepared) {
    let replay = Game::replay(prepared.clone(), &game.replay_journal()).unwrap();
    assert_eq!(replay.state_digest().unwrap(), game.state_digest().unwrap());
}
fn passive(req: PlayerRequest) -> PlayerResponse {
    match req {
        PlayerRequest::Priority { .. } => PlayerResponse::Pass,
        PlayerRequest::Attackers { .. } => PlayerResponse::Attackers { attacks: vec![] },
        PlayerRequest::Blockers { .. } => PlayerResponse::Blockers { blocks: vec![] },
        PlayerRequest::Discard {
            count, candidates, ..
        } => PlayerResponse::Discard {
            objects: candidates.into_iter().take(count).collect(),
        },
        _ => panic!("unexpected passive request"),
    }
}
fn until(game: &mut Game, prepared: &Prepared, wanted: impl Fn(&Game, &PlayerRequest) -> bool) {
    for _ in 0..250 {
        let req = request(game).1;
        if wanted(game, &req) {
            return;
        }
        input(game, prepared, passive(req));
    }
    panic!("expected boundary not reached")
}
fn library_order(game: &Game, player: PlayerId) -> Vec<CardId> {
    let snapshot: serde_json::Value = serde_json::from_str(&game.snapshot().unwrap()).unwrap();
    serde_json::from_value(snapshot["state"]["libraries"][player.0.to_string()].clone()).unwrap()
}
fn target_handle(game: &Game, viewer: PlayerId, oracle: &str) -> ObjectHandle {
    observe(game, viewer)
        .view
        .objects
        .into_iter()
        .find(|o| o.oracle_id == oracle && o.zone == Zone::Battlefield)
        .expect("target")
        .handle
}
fn tuck(game: &mut Game, prepared: &Prepared, oracle: &str) -> ObjectHandle {
    let card = observe(game, PlayerId(0))
        .view
        .objects
        .into_iter()
        .find(|o| o.zone == Zone::Hand { owner: PlayerId(0) })
        .unwrap()
        .handle;
    input(game, prepared, PlayerResponse::Cast { card });
    let target = target_handle(game, PlayerId(0), oracle);
    input(
        game,
        prepared,
        PlayerResponse::Targets {
            objects: vec![target.clone()],
        },
    );
    for _ in 0..3 {
        let land = observe(game, PlayerId(0))
            .view
            .objects
            .into_iter()
            .find(|o| o.controller == PlayerId(0) && o.zone == Zone::Battlefield && !o.tapped)
            .unwrap()
            .handle;
        input(
            game,
            prepared,
            PlayerResponse::ActivateMana { object: land },
        );
    }
    input(
        game,
        prepared,
        PlayerResponse::FinishMana { undo_mana: true },
    );
    for _ in 0..4 {
        input(game, prepared, PlayerResponse::Pass);
    }
    target
}
fn declined_tuck_to_draw(game: &mut Game, prepared: &Prepared) {
    tuck(game, prepared, ISAMARU);
    input(
        game,
        prepared,
        PlayerResponse::CommanderReturn { to_command: false },
    );
    assert_eq!(
        library_order(game, PlayerId(1)),
        vec![CardId(4), CardId(10), CardId(11)]
    );
    until(game, prepared, |_, req| {
        matches!(
            req,
            PlayerRequest::CommanderReturn {
                destination: Zone::Hand { .. },
                ..
            }
        )
    });
    assert_eq!(game.turn().step, TurnStep::Draw);
    assert_eq!(request(game).0, PlayerId(1));
}

#[test]
fn declined_tuck_has_hidden_draw_replacement_then_exactly_one_draw_and_recast() {
    let prepared = library_prepared();
    let mut game = Game::new(prepared.clone());
    let old_target = target_handle(&game, PlayerId(1), ISAMARU);
    let designation = observe(&game, PlayerId(1)).view.commanders[0]
        .designation
        .clone();
    declined_tuck_to_draw(&mut game, &prepared);
    let (owner, req) = request(&game);
    let PlayerRequest::CommanderReturn {
        object,
        designation: returned_designation,
        token,
        ..
    } = req
    else {
        panic!()
    };
    assert_eq!(object, None);
    assert_eq!(designation, returned_designation);
    for viewer in game.live_players() {
        let view = observe(&game, viewer).view;
        assert!(
            view.objects
                .iter()
                .all(|o| !matches!(o.zone, Zone::Library { .. }))
        );
        assert!(view.objects.iter().all(|o| o.oracle_id != ISAMARU));
        assert_eq!(view.players[1].library_count, 3);
        assert!(view.objects.iter().all(|o| o.handle != old_target));
    }
    for other in [PlayerId(0), PlayerId(2), PlayerId(3)] {
        reject(
            &game,
            other,
            token.clone(),
            PlayerResponse::CommanderReturn { to_command: true },
        );
    }
    reject(&game, owner, token.clone(), PlayerResponse::Pass);
    for to_command in [false, true] {
        let mut branch = game.fork();
        input(
            &mut branch,
            &prepared,
            PlayerResponse::CommanderReturn { to_command },
        );
        assert_eq!(library_order(&branch, owner), vec![CardId(10), CardId(11)]);
        assert_eq!(
            branch.zone(branch.object_id(CardId(4)).unwrap()),
            Some(if to_command {
                Zone::Command { owner }
            } else {
                Zone::Hand { owner }
            })
        );
        assert_eq!(branch.object_id(CardId(4)).unwrap().incarnation, 2);
        for viewer in branch.live_players() {
            let seen = observe(&branch, viewer)
                .view
                .objects
                .iter()
                .any(|o| o.oracle_id == ISAMARU);
            assert_eq!(seen, to_command || viewer == owner);
        }
        reject(
            &branch,
            owner,
            token.clone(),
            PlayerResponse::CommanderReturn { to_command },
        );
        until(&mut branch, &prepared, |g, req| {
            g.turn().step == TurnStep::PrecombatMain
                && matches!(
                    req,
                    PlayerRequest::Priority {
                        player: PlayerId(1),
                        ..
                    }
                )
        });
        reject(
            &branch,
            owner,
            request(&branch).1.token().clone(),
            PlayerResponse::Cast {
                card: old_target.clone(),
            },
        );
        let card = observe(&branch, owner)
            .view
            .objects
            .into_iter()
            .find(|o| o.oracle_id == ISAMARU)
            .unwrap()
            .handle;
        input(&mut branch, &prepared, PlayerResponse::Cast { card });
        let PlayerRequest::Mana { cost, .. } = request(&branch).1 else {
            panic!()
        };
        assert_eq!(cost.generic, Quantity::zero());
        let land = observe(&branch, owner)
            .view
            .objects
            .into_iter()
            .find(|o| o.owner == owner && o.zone == Zone::Battlefield)
            .unwrap()
            .handle;
        input(
            &mut branch,
            &prepared,
            PlayerResponse::ActivateMana { object: land },
        );
        input(
            &mut branch,
            &prepared,
            PlayerResponse::FinishMana { undo_mana: true },
        );
        assert_eq!(
            branch.commander_ledger().tax(CardId(4), owner).unwrap(),
            Quantity::from(if to_command { 2 } else { 0 })
        );
        assert_eq!(library_order(&branch, owner), vec![CardId(10), CardId(11)]);
        replay_equal(&branch, &prepared);
    }
}

#[test]
fn ordinary_creature_top_placement_draws_actual_top_without_commander_choice() {
    let prepared = library_prepared();
    let mut game = Game::new(prepared.clone());
    let old = tuck(&mut game, &prepared, BEARS);
    assert_eq!(
        library_order(&game, PlayerId(2)),
        vec![CardId(16), CardId(12), CardId(13)]
    );
    assert!(
        observe(&game, PlayerId(0))
            .view
            .objects
            .iter()
            .all(|o| o.handle != old)
    );
    until(&mut game, &prepared, |g, req| {
        g.turn_owner() == PlayerId(2)
            && g.turn().step == TurnStep::Draw
            && matches!(req, PlayerRequest::Priority { .. })
    });
    assert_eq!(
        library_order(&game, PlayerId(2)),
        vec![CardId(12), CardId(13)]
    );
    assert_eq!(
        game.zone(game.object_id(CardId(16)).unwrap()),
        Some(Zone::Hand { owner: PlayerId(2) })
    );
    for viewer in game.live_players() {
        assert_eq!(
            observe(&game, viewer)
                .view
                .objects
                .iter()
                .any(|o| o.oracle_id == BEARS),
            viewer == PlayerId(2)
        );
    }
    replay_equal(&game, &prepared);
}

#[test]
fn library_move_choice_continues_once_after_its_sorcery_owner_departs() {
    let prepared = library_prepared();
    let mut game = Game::new(prepared.clone());
    tuck(&mut game, &prepared, ISAMARU);
    assert!(matches!(
        request(&game).1,
        PlayerRequest::CommanderReturn {
            object: Some(_),
            destination: Zone::Library { owner: PlayerId(1) },
            ..
        }
    ));
    for accepted in [false, true] {
        for to_command in [false, true] {
            let mut branch = game.fork();
            if accepted {
                submit(&mut branch, PlayerResponse::CommanderReturn { to_command });
            }
            branch.concede(PlayerId(0)).unwrap();
            assert!(matches!(
                settle(&mut branch, &prepared),
                Advance::AwaitingDecision(_)
            ));
            if !accepted {
                input(
                    &mut branch,
                    &prepared,
                    PlayerResponse::CommanderReturn { to_command },
                );
            }
            assert_eq!(branch.object_id(CardId(4)).unwrap().incarnation, 1);
            assert_eq!(
                library_order(&branch, PlayerId(1)),
                if to_command {
                    vec![CardId(10), CardId(11)]
                } else {
                    vec![CardId(4), CardId(10), CardId(11)]
                }
            );
            assert!(branch.object_id(CardId(0)).is_none());
            assert!(observe(&branch, PlayerId(1)).view.stack.is_empty());
            replay_equal(&branch, &prepared);
        }
    }
}

#[test]
fn hidden_draw_choice_receipts_and_accepted_inputs_revalidate_concessions() {
    let prepared = library_prepared();
    let mut game = Game::new(prepared.clone());
    declined_tuck_to_draw(&mut game, &prepared);
    for departing in [PlayerId(0), PlayerId(1), PlayerId(2)] {
        let mut stale = game.fork();
        let saved = receipt(&stale, PlayerResponse::CommanderReturn { to_command: true });
        stale.concede(departing).unwrap();
        assert!(stale.submit(saved).is_err());
        let mut branch = game.fork();
        submit(
            &mut branch,
            PlayerResponse::CommanderReturn { to_command: true },
        );
        branch.concede(departing).unwrap();
        assert!(matches!(
            settle(&mut branch, &prepared),
            Advance::AwaitingDecision(_)
        ));
        if departing == PlayerId(1) {
            assert!(branch.object_id(CardId(4)).is_none());
        } else {
            assert_eq!(
                branch.zone(branch.object_id(CardId(4)).unwrap()),
                Some(Zone::Command { owner: PlayerId(1) })
            );
            assert_eq!(
                library_order(&branch, PlayerId(1)),
                vec![CardId(10), CardId(11)]
            );
        }
        replay_equal(&branch, &prepared);
    }
}

#[test]
fn nested_draw_identity_and_order_forgeries_fail_rehashed_snapshot_restore() {
    let prepared = library_prepared();
    let mut game = Game::new(prepared.clone());
    declined_tuck_to_draw(&mut game, &prepared);
    for mutation in 0..4 {
        let mut snapshot: serde_json::Value =
            serde_json::from_str(&game.snapshot().unwrap()).unwrap();
        match mutation {
            0 => snapshot["state"]["libraries"]["1"] = serde_json::json!([10, 4, 11]),
            1 => snapshot["state"]["frame"]["object"]["incarnation"] = 99.into(),
            2 => {
                snapshot["state"]["frame"]["destination"] =
                    serde_json::json!({"zone":"hand","owner":2})
            }
            _ => {
                snapshot["state"]["commanders"]["records"]["4"]["command_casts"]["1"] = "12".into()
            }
        }
        snapshot["state_sha256"] = mtg_contracts::digest(&snapshot["state"]).unwrap().into();
        assert!(
            Game::restore(prepared.clone(), &serde_json::to_string(&snapshot).unwrap()).is_err()
        );
    }
}

#[test]
fn hidden_draw_validation_work_recovers_without_minting_library_handles() {
    let prepared = library_prepared();
    let mut game = Game::new(prepared.clone());
    declined_tuck_to_draw(&mut game, &prepared);
    let (owner, req) = request(&game);
    let before = game.snapshot().unwrap();
    let QueryResult::Incomplete { work, .. } = game.check_player_response(
        owner,
        req.token().clone(),
        PlayerResponse::CommanderReturn { to_command: false },
        Budget::new(0),
    ) else {
        panic!("suspended query")
    };
    let saved = game.save_work(&work).unwrap();
    assert_eq!(before, game.snapshot().unwrap());
    let mut restored = Game::restore(prepared.clone(), &before).unwrap();
    let work = restored.restore_work(&saved).unwrap();
    let QueryValue::Validated(foreign) =
        complete(&restored, restored.resume_work(work, Budget::new(0)))
    else {
        panic!("validated")
    };
    assert!(game.submit(foreign).is_err());
    let work = restored.restore_work(&saved).unwrap();
    let QueryValue::Validated(local) =
        complete(&restored, restored.resume_work(work, Budget::new(0)))
    else {
        panic!("validated")
    };
    assert_eq!(before, restored.snapshot().unwrap());
    restored.submit(local).unwrap();
    assert!(matches!(
        settle(&mut restored, &prepared),
        Advance::AwaitingDecision(_)
    ));
    assert_eq!(
        library_order(&restored, owner),
        vec![CardId(10), CardId(11)]
    );
    assert_eq!(
        restored.zone(restored.object_id(CardId(4)).unwrap()),
        Some(Zone::Hand { owner })
    );
    replay_equal(&restored, &prepared);
    game.concede(PlayerId(2)).unwrap();
    assert!(game.restore_work(&saved).is_err());
}

#[test]
fn actual_commander_threshold_preserves_physical_history_after_controller_departure() {
    let prepared = damage_prepared(true);
    let mut game = Game::new(prepared.clone());
    let mut hits = 0;
    let mut verified_first_hit = false;
    for _ in 0..400 {
        if !game.live_players().contains(&PlayerId(2)) {
            break;
        }
        let req = request(&game).1;
        let response = match req {
            PlayerRequest::Attackers {
                player: PlayerId(1),
                candidates,
                ..
            } => {
                hits += 1;
                PlayerResponse::Attackers {
                    attacks: vec![(candidates[0].clone(), PlayerId(2))],
                }
            }
            req => passive(req),
        };
        submit(&mut game, response);
        for _ in 0..100 {
            match game.advance(Budget::new(1)) {
                Advance::Suspended { .. } => continue,
                Advance::AwaitingDecision(_) => break,
                other => panic!("unexpected damage scenario outcome {other:?}"),
            }
        }
        if game
            .commander_ledger()
            .history(CardId(0))
            .unwrap()
            .combat_damage
            .get(&PlayerId(2))
            == Some(&Quantity::from(18))
        {
            assert!(game.live_players().contains(&PlayerId(2)));
            assert_eq!(
                observe(&game, PlayerId(0)).view.players[2].life,
                Quantity::from(22)
            );
            if !verified_first_hit {
                verify(&game, &prepared);
                verified_first_hit = true;
            }
        }
    }
    assert_eq!(hits, 2);
    assert!(verified_first_hit);
    verify(&game, &prepared);
    assert!(!game.live_players().contains(&PlayerId(2)));
    assert_eq!(
        observe(&game, PlayerId(0)).view.players[2].life,
        Quantity::from(4)
    );
    let history = game.commander_ledger().history(CardId(0)).unwrap().clone();
    assert_eq!(
        history.combat_damage.get(&PlayerId(2)),
        Some(&Quantity::from(36))
    );
    assert_eq!(game.live_players().len(), 3);
    game.concede(PlayerId(1)).unwrap();
    assert!(matches!(
        settle(&mut game, &prepared),
        Advance::AwaitingDecision(_)
    ));
    for to_command in [false, true] {
        let mut branch = game.fork();
        input(
            &mut branch,
            &prepared,
            PlayerResponse::CommanderReturn { to_command },
        );
        assert_eq!(branch.commander_ledger().history(CardId(0)), Some(&history));
        assert_eq!(
            branch.zone(branch.object_id(CardId(0)).unwrap()),
            Some(if to_command {
                Zone::Command { owner: PlayerId(0) }
            } else {
                Zone::Exile
            })
        );
        replay_equal(&branch, &prepared);
    }
}
