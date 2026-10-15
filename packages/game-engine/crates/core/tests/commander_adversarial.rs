//! Independent black-box evidence for Commander casting/return continuations.
//! Actions use player-facing handles; physical IDs are trusted assertions only.
use mtg_contracts::{Catalog, Color, Quantity};
use mtg_core::*;

fn prepared(config: ScenarioConfig) -> Prepared {
    Prepared::admit(
        Catalog::read(include_str!(
            "../../../tests/fixtures/expected-commander-ir.json"
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
            QueryResult::Incomplete { work, .. } => {
                result = game.resume_work(work, Budget::new(1));
            }
            QueryResult::Rejected(reason) => panic!("query rejected: {reason:?}"),
        }
    }
    panic!("query did not finish")
}
fn observe(game: &Game, viewer: PlayerId) -> Observation {
    let QueryValue::Observation(view) = complete(game, game.observe(viewer, Budget::new(0))) else {
        panic!("wrong query result")
    };
    view
}
fn request(game: &Game) -> (PlayerId, PlayerRequest) {
    game.live_players()
        .into_iter()
        .find_map(
            |player| match complete(game, game.player_decision_for(player, Budget::new(0))) {
                QueryValue::Decision(Some(request)) => Some((player, request)),
                QueryValue::Decision(None) => None,
                _ => panic!("unexpected decision query"),
            },
        )
        .expect("missing player decision")
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
    panic!("automatic work did not settle")
}
fn submit(game: &mut Game, response: PlayerResponse) {
    let (actor, req) = request(game);
    let QueryValue::Validated(receipt) = complete(
        game,
        game.check_player_response(actor, req.token().clone(), response, Budget::new(0)),
    ) else {
        panic!("missing validated response")
    };
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
            QueryResult::Incomplete { work, .. } => {
                result = game.resume_work(work, Budget::new(1));
            }
            QueryResult::Complete(_) => panic!("illegal response accepted"),
        }
    }
    panic!("rejection did not finish")
}
fn visible(game: &Game, player: PlayerId, owner: PlayerId, zone: Zone) -> ObjectHandle {
    observe(game, player)
        .view
        .objects
        .into_iter()
        .find(|object| object.owner == owner && object.zone == zone)
        .expect("missing visible object")
        .handle
}
fn activate(game: &mut Game, prepared: &Prepared, player: PlayerId) {
    let land = observe(game, player)
        .view
        .objects
        .into_iter()
        .find(|object| {
            object.controller == player
                && object.zone == Zone::Battlefield
                && !object.tapped
                && object.oracle_id != "cfb756dd-6fe1-430a-8f04-8b10cac62a86"
        })
        .expect("missing untapped land")
        .handle;
    input(
        game,
        prepared,
        PlayerResponse::ActivateMana { object: land },
    );
}
fn finish_mana(game: &mut Game, prepared: &Prepared, undo_mana: bool) {
    input(game, prepared, PlayerResponse::FinishMana { undo_mana });
}
fn cast_first(game: &mut Game, prepared: &Prepared) {
    let card = visible(
        game,
        PlayerId(0),
        PlayerId(0),
        Zone::Command { owner: PlayerId(0) },
    );
    input(game, prepared, PlayerResponse::Cast { card });
    let PlayerRequest::Mana { cost, .. } = request(game).1 else {
        panic!()
    };
    assert_eq!(cost.generic, Quantity::zero());
    assert_eq!(cost.colored, vec![Color::W]);
    activate(game, prepared, PlayerId(0));
    finish_mana(game, prepared, true);
    for _ in 0..4 {
        input(game, prepared, PlayerResponse::Pass);
    }
    assert_eq!(
        game.zone(game.object_id(CardId(0)).unwrap()),
        Some(Zone::Battlefield)
    );
    assert_eq!(
        game.commander_ledger().tax(CardId(0), PlayerId(0)).unwrap(),
        Quantity::from(2)
    );
}
fn bounce_to_choice(game: &mut Game, prepared: &Prepared) {
    assert_eq!(request(game).0, PlayerId(0));
    input(game, prepared, PlayerResponse::Pass);
    assert_eq!(request(game).0, PlayerId(1));
    activate(game, prepared, PlayerId(1));
    let card = visible(
        game,
        PlayerId(1),
        PlayerId(1),
        Zone::Hand { owner: PlayerId(1) },
    );
    input(game, prepared, PlayerResponse::Cast { card });
    let PlayerRequest::Targets { candidates, .. } = request(game).1 else {
        panic!()
    };
    assert_eq!(candidates.len(), 1);
    input(
        game,
        prepared,
        PlayerResponse::Targets {
            objects: candidates,
        },
    );
    finish_mana(game, prepared, true);
    for _ in 0..4 {
        input(game, prepared, PlayerResponse::Pass);
    }
    assert!(matches!(
        request(game),
        (
            PlayerId(0),
            PlayerRequest::CommanderReturn {
                destination: Zone::Hand { owner: PlayerId(0) },
                ..
            }
        )
    ));
}
fn replay_equal(game: &Game, prepared: &Prepared) {
    let replay = Game::replay(prepared.clone(), &game.replay_journal()).unwrap();
    assert_eq!(replay.state_digest().unwrap(), game.state_digest().unwrap());
}
fn pass_until(game: &mut Game, prepared: &Prepared, wanted: fn(&PlayerRequest) -> bool) {
    for _ in 0..40 {
        let req = request(game).1;
        if wanted(&req) {
            return;
        }
        assert!(matches!(req, PlayerRequest::Priority { .. }));
        input(game, prepared, PlayerResponse::Pass);
    }
    panic!("expected request not reached")
}

#[test]
fn command_origin_failure_restores_identity_and_tax_with_keep_or_undo() {
    let prepared = prepared(ScenarioConfig::command_casting());
    let mut game = Game::new(prepared.clone());
    let original = game.object_id(CardId(0)).unwrap();
    let card = visible(
        &game,
        PlayerId(0),
        PlayerId(0),
        Zone::Command { owner: PlayerId(0) },
    );
    input(&mut game, &prepared, PlayerResponse::Cast { card });
    finish_mana(&mut game, &prepared, false);
    assert_eq!(game.object_id(CardId(0)), Some(original));
    assert_eq!(
        game.zone(original),
        Some(Zone::Command { owner: PlayerId(0) })
    );
    assert_eq!(
        game.commander_ledger().tax(CardId(0), PlayerId(0)).unwrap(),
        Quantity::zero()
    );
    cast_first(&mut game, &prepared);
    bounce_to_choice(&mut game, &prepared);
    input(
        &mut game,
        &prepared,
        PlayerResponse::CommanderReturn { to_command: true },
    );
    assert_eq!(request(&game).0, PlayerId(0));
    for undo_mana in [false, true] {
        let mut branch = game.fork();
        let original = branch.object_id(CardId(0)).unwrap();
        let card = visible(
            &branch,
            PlayerId(0),
            PlayerId(0),
            Zone::Command { owner: PlayerId(0) },
        );
        input(&mut branch, &prepared, PlayerResponse::Cast { card });
        let PlayerRequest::Mana { cost, .. } = request(&branch).1 else {
            panic!()
        };
        assert_eq!(cost.generic, Quantity::from(2));
        activate(&mut branch, &prepared, PlayerId(0));
        finish_mana(&mut branch, &prepared, undo_mana);
        assert_eq!(branch.object_id(CardId(0)), Some(original));
        assert_eq!(
            branch.zone(original),
            Some(Zone::Command { owner: PlayerId(0) })
        );
        assert_eq!(
            branch
                .commander_ledger()
                .tax(CardId(0), PlayerId(0))
                .unwrap(),
            Quantity::from(2)
        );
        let view = observe(&branch, PlayerId(0)).view;
        assert_eq!(
            view.players[0]
                .mana
                .get(&Color::W)
                .cloned()
                .unwrap_or_default(),
            Quantity::from(i64::from(!undo_mana))
        );
        assert_eq!(
            view.objects
                .iter()
                .filter(|o| o.owner == PlayerId(0) && o.tapped)
                .count(),
            if undo_mana { 1 } else { 2 }
        );
        replay_equal(&branch, &prepared);
    }
    assert_eq!(game.object_id(CardId(0)).unwrap().incarnation, 3);
}

#[test]
fn hand_replacement_owner_authentication_and_source_departure_preserve_resolution() {
    let prepared = prepared(ScenarioConfig::command_casting());
    let mut game = Game::new(prepared.clone());
    cast_first(&mut game, &prepared);
    bounce_to_choice(&mut game, &prepared);
    let (owner, req) = request(&game);
    for other in [PlayerId(1), PlayerId(2), PlayerId(3)] {
        reject(
            &game,
            other,
            req.token().clone(),
            PlayerResponse::CommanderReturn { to_command: true },
        );
    }
    reject(&game, owner, req.token().clone(), PlayerResponse::Pass);
    assert_eq!(
        game.zone(game.object_id(CardId(0)).unwrap()),
        Some(Zone::Battlefield)
    );
    for to_command in [false, true] {
        let mut branch = game.fork();
        branch.concede(PlayerId(1)).unwrap();
        assert!(matches!(
            settle(&mut branch, &prepared),
            Advance::AwaitingDecision(_)
        ));
        assert!(matches!(
            request(&branch),
            (PlayerId(0), PlayerRequest::CommanderReturn { .. })
        ));
        input(
            &mut branch,
            &prepared,
            PlayerResponse::CommanderReturn { to_command },
        );
        let destination = if to_command {
            Zone::Command { owner }
        } else {
            Zone::Hand { owner }
        };
        assert_eq!(
            branch.zone(branch.object_id(CardId(0)).unwrap()),
            Some(destination)
        );
        assert!(branch.object_id(CardId(7)).is_none());
        assert!(observe(&branch, owner).view.stack.is_empty());
        assert_eq!(
            branch.commander_ledger().tax(CardId(0), owner).unwrap(),
            Quantity::from(2)
        );
        reject(
            &branch,
            owner,
            req.token().clone(),
            PlayerResponse::CommanderReturn { to_command },
        );
        replay_equal(&branch, &prepared);
    }
}

#[test]
fn departure_invalidates_accepted_return_receipt_and_never_resurrects_owner_objects() {
    for config in [
        ScenarioConfig::command_casting(),
        ScenarioConfig::commander_exile(),
    ] {
        let prepared = prepared(config.clone());
        let mut game = Game::new(prepared.clone());
        if config == ScenarioConfig::command_casting() {
            cast_first(&mut game, &prepared);
            bounce_to_choice(&mut game, &prepared);
        } else {
            game.concede(PlayerId(1)).unwrap();
            assert!(matches!(
                settle(&mut game, &prepared),
                Advance::AwaitingDecision(_)
            ));
        }
        let (owner, req) = request(&game);
        assert_eq!(owner, PlayerId(0));
        assert!(matches!(req, PlayerRequest::CommanderReturn { .. }));
        let QueryValue::Validated(receipt) = complete(
            &game,
            game.check_player_response(
                owner,
                req.token().clone(),
                PlayerResponse::CommanderReturn { to_command: true },
                Budget::new(0),
            ),
        ) else {
            panic!()
        };
        game.concede(owner).unwrap();
        assert!(game.submit(receipt).is_err());
        assert!(matches!(
            settle(&mut game, &prepared),
            Advance::AwaitingDecision(_)
        ));
        assert!(game.object_id(CardId(0)).is_none());
        assert_eq!(
            game.commander_ledger().history(CardId(0)).unwrap().owner,
            owner
        );
        replay_equal(&game, &prepared);
    }
}

#[test]
fn exile_return_decline_is_one_shot_and_acceptance_has_no_cast_history() {
    let prepared = prepared(ScenarioConfig::commander_exile());
    let mut game = Game::new(prepared.clone());
    game.concede(PlayerId(1)).unwrap();
    assert!(matches!(
        settle(&mut game, &prepared),
        Advance::AwaitingDecision(_)
    ));
    assert_eq!(
        game.zone(game.object_id(CardId(0)).unwrap()),
        Some(Zone::Exile)
    );
    for to_command in [false, true] {
        let mut branch = game.fork();
        input(
            &mut branch,
            &prepared,
            PlayerResponse::CommanderReturn { to_command },
        );
        assert_eq!(
            branch.zone(branch.object_id(CardId(0)).unwrap()),
            Some(if to_command {
                Zone::Command { owner: PlayerId(0) }
            } else {
                Zone::Exile
            })
        );
        assert_eq!(
            branch
                .commander_ledger()
                .tax(CardId(0), PlayerId(0))
                .unwrap(),
            Quantity::zero()
        );
        for _ in 0..3 {
            assert!(matches!(request(&branch).1, PlayerRequest::Priority { .. }));
            input(&mut branch, &prepared, PlayerResponse::Pass);
        }
        assert!(!matches!(
            request(&branch).1,
            PlayerRequest::CommanderReturn { .. }
        ));
        replay_equal(&branch, &prepared);
    }
}

#[test]
fn accepted_return_input_survives_source_departure_but_not_owner_departure() {
    let prepared = prepared(ScenarioConfig::command_casting());
    let mut game = Game::new(prepared.clone());
    cast_first(&mut game, &prepared);
    bounce_to_choice(&mut game, &prepared);
    for departing in [PlayerId(0), PlayerId(1), PlayerId(2)] {
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
        if departing == PlayerId(0) {
            assert!(branch.object_id(CardId(0)).is_none());
        } else {
            assert_eq!(
                branch.zone(branch.object_id(CardId(0)).unwrap()),
                Some(Zone::Command { owner: PlayerId(0) })
            );
        }
        replay_equal(&branch, &prepared);
    }
}

#[test]
fn lethal_damage_graveyard_return_uses_a_separate_wave_and_decline_is_one_shot() {
    let prepared = prepared(ScenarioConfig::commander_lifecycle());
    let mut game = Game::new(prepared.clone());
    pass_until(&mut game, &prepared, |r| {
        matches!(r, PlayerRequest::Attackers { .. })
    });
    let PlayerRequest::Attackers { candidates, .. } = request(&game).1 else {
        panic!()
    };
    assert_eq!(candidates.len(), 1);
    input(
        &mut game,
        &prepared,
        PlayerResponse::Attackers {
            attacks: vec![(candidates[0].clone(), PlayerId(1))],
        },
    );
    pass_until(&mut game, &prepared, |r| {
        matches!(r, PlayerRequest::Blockers { .. })
    });
    let PlayerRequest::Blockers {
        candidates,
        attackers,
        ..
    } = request(&game).1
    else {
        panic!()
    };
    input(
        &mut game,
        &prepared,
        PlayerResponse::Blockers {
            blocks: vec![(candidates[0].clone(), attackers[0].clone())],
        },
    );
    pass_until(&mut game, &prepared, |r| {
        matches!(r, PlayerRequest::CommanderReturn { .. })
    });
    assert_eq!(
        game.zone(game.object_id(CardId(0)).unwrap()),
        Some(Zone::Graveyard { owner: PlayerId(0) })
    );
    assert_eq!(
        game.zone(game.object_id(CardId(14)).unwrap()),
        Some(Zone::Graveyard { owner: PlayerId(1) })
    );
    assert_eq!(game.object_id(CardId(0)).unwrap().incarnation, 1);
    for to_command in [false, true] {
        let mut branch = game.fork();
        input(
            &mut branch,
            &prepared,
            PlayerResponse::CommanderReturn { to_command },
        );
        assert_eq!(
            branch.object_id(CardId(0)).unwrap().incarnation,
            if to_command { 2 } else { 1 }
        );
        activate(&mut branch, &prepared, PlayerId(0));
        assert!(matches!(request(&branch).1, PlayerRequest::Priority { .. }));
        assert_eq!(
            branch
                .commander_ledger()
                .tax(CardId(0), PlayerId(0))
                .unwrap(),
            Quantity::zero()
        );
        replay_equal(&branch, &prepared);
    }
}

#[test]
fn actual_unblocked_commander_damage_faults_without_committing_partial_damage() {
    let prepared = prepared(ScenarioConfig::commander_lifecycle());
    let mut game = Game::new(prepared.clone());
    pass_until(&mut game, &prepared, |r| {
        matches!(r, PlayerRequest::Attackers { .. })
    });
    let PlayerRequest::Attackers { candidates, .. } = request(&game).1 else {
        panic!()
    };
    input(
        &mut game,
        &prepared,
        PlayerResponse::Attackers {
            attacks: vec![(candidates[0].clone(), PlayerId(1))],
        },
    );
    pass_until(&mut game, &prepared, |r| {
        matches!(r, PlayerRequest::Blockers { .. })
    });
    input(
        &mut game,
        &prepared,
        PlayerResponse::Blockers { blocks: vec![] },
    );
    for _ in 0..4 {
        submit(&mut game, PlayerResponse::Pass);
        let result = settle(&mut game, &prepared);
        if let Advance::Fault(reason) = result {
            assert_eq!(reason, "unsupported commander combat damage to player");
            assert_eq!(
                observe(&game, PlayerId(0)).view.players[1].life,
                Quantity::from(40)
            );
            assert!(
                game.commander_ledger()
                    .history(CardId(0))
                    .unwrap()
                    .combat_damage
                    .is_empty()
            );
            assert!(game.events().iter().all(|e| !matches!(
                e,
                Event::Committed {
                    actual: EventOutcome::DamageDealt { .. },
                    ..
                }
            )));
            assert_eq!(
                Game::replay(prepared.clone(), &game.replay_journal())
                    .err()
                    .as_deref(),
                Some("unsupported commander combat damage to player")
            );
            return;
        }
        assert!(matches!(result, Advance::AwaitingDecision(_)));
    }
    panic!("unblocked commander damage did not fault")
}

#[test]
fn nested_hand_resolution_cost_source_and_cursor_forgery_fail_independent_restore() {
    let prepared = prepared(ScenarioConfig::command_casting());
    let mut game = Game::new(prepared.clone());
    cast_first(&mut game, &prepared);
    bounce_to_choice(&mut game, &prepared);
    for mutation in 0..5 {
        let mut snapshot: serde_json::Value =
            serde_json::from_str(&game.snapshot().unwrap()).unwrap();
        let frame = &mut snapshot["state"]["frame"]["continuation"]["frame"];
        match mutation {
            0 => {
                frame["spell"]["total_cost"]["generic"] = "5".into();
                frame["spell"]["paid"]["generic"] = "5".into();
            }
            1 => {
                frame["spell"]["cast_from"] = serde_json::json!({"zone":"command","owner":1});
            }
            2 => {
                frame["spell"]["source"]["incarnation"] = 999.into();
            }
            3 => {
                frame["legal_targets"][0] = false.into();
            }
            _ => {
                frame["cursor"] = 1.into();
            }
        }
        snapshot["state_sha256"] = mtg_contracts::digest(&snapshot["state"]).unwrap().into();
        assert!(
            Game::restore(prepared.clone(), &serde_json::to_string(&snapshot).unwrap()).is_err()
        );
    }
}
