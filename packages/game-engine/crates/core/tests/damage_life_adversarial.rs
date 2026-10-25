//! Independent player-facing noncombat damage, direct life, and target regressions.
use mtg_contracts::{Catalog, Color, Quantity};
use mtg_core::*;
use std::collections::BTreeMap;
const ISAMARU: &str = "cfb756dd-6fe1-430a-8f04-8b10cac62a86";
const SORCERER: &str = "5e961d15-5972-4e4b-9385-1cd7cd7c6bbe";
const FEAST: &str = "8186fd80-015f-470c-9e1c-cbf45764a057";
const HANDS: &str = "3cc48835-3ac0-4774-b380-f9b21d2dc974";
const NECTAR: &str = "30870ee5-6ad7-48a9-983e-d3b018f2344f";
const UNSUMMON: &str = "837182db-1bf3-4a2c-bd01-1af9d9873561";
const ISLAND: &str = "b2c6aa39-2d2a-459c-a555-fb48ba993373";
const PLAINS: &str = "bc71ebf6-2056-41f7-be35-b2e5c34afa99";
const MOUNTAIN: &str = "a3fb7228-e76b-4e96-a40e-20b5fed75685";
const SWAMP: &str = "56719f6a-1a6c-4c0a-8d21-18f7d7350b68";
fn prepared(config: ScenarioConfig) -> Prepared {
    Prepared::admit(
        Catalog::read(include_str!(
            "../../../tests/fixtures/expected-damage-life-ir.json"
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
                && [ISLAND, PLAINS, MOUNTAIN, SWAMP].contains(&o.oracle_id.as_str())
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
fn entity_target(game: &mut Game, p: &Prepared, entity: PlayerTargetEntity) {
    input(
        game,
        p,
        PlayerResponse::EntityTargets {
            entities: vec![entity],
        },
    );
}
fn begin_sorcerer(game: &mut Game, p: &Prepared) {
    let object = handle(game, PlayerId(0), SORCERER, Zone::Battlefield);
    input(
        game,
        p,
        PlayerResponse::ActivateAbility { object, ability: 0 },
    );
    assert!(matches!(
        request(game).1,
        PlayerRequest::EntityTargets { .. }
    ));
}
fn targeted_spell(
    game: &mut Game,
    p: &Prepared,
    oracle: &str,
    player: PlayerId,
    payment: &[(Color, i64)],
) {
    begin_spell(game, p, oracle);
    player_target(game, p, player);
    fund_all(game, p);
    spend(game, p, payment);
}
fn frame(game: &Game) -> serde_json::Value {
    serde_json::from_str::<serde_json::Value>(&game.snapshot().unwrap()).unwrap()["state"]["frame"]
        .clone()
}
fn advance_to_cursor(game: &mut Game, p: &Prepared, cursor: u64) {
    for _ in 0..50 {
        let f = frame(game);
        if f["stage"] == "resolve" && f["frame"]["cursor"] == cursor {
            return;
        }
        assert!(matches!(
            game.advance(Budget::new(1)),
            Advance::Suspended { .. }
        ));
        verify(game, p);
    }
    panic!("resolution cursor not reached");
}

#[test]
fn mixed_targets_reject_noncreatures_wrong_response_and_queries_do_not_tap_or_mint() {
    let p = prepared(ScenarioConfig::damage_life());
    let mut game = Game::new(p.clone());
    begin_sorcerer(&mut game, &p);
    let (actor, req) = request(&game);
    let before = game.snapshot().unwrap();
    let first = observe(&game, actor);
    for _ in 0..3 {
        assert_eq!(first, observe(&game, actor));
    }
    assert_eq!(before, game.snapshot().unwrap());
    let land = handle(&game, actor, MOUNTAIN, Zone::Battlefield);
    reject(
        &game,
        actor,
        req.token().clone(),
        PlayerResponse::EntityTargets {
            entities: vec![PlayerTargetEntity::Object { object: land }],
        },
    );
    reject(
        &game,
        actor,
        req.token().clone(),
        PlayerResponse::PlayerTargets {
            players: vec![actor],
        },
    );
    assert!(
        !observe(&game, actor)
            .view
            .objects
            .iter()
            .find(|o| o.oracle_id == SORCERER && o.controller == actor)
            .unwrap()
            .tapped
    );
    let source = handle(&game, actor, SORCERER, Zone::Battlefield);
    entity_target(&mut game, &p, PlayerTargetEntity::Object { object: source });
    assert!(
        observe(&game, actor)
            .view
            .objects
            .iter()
            .find(|o| o.oracle_id == SORCERER && o.controller == actor)
            .unwrap()
            .tapped
    );
    round(&mut game, &p);
    assert_eq!(
        game.zone(game.object_id(CardId(0)).unwrap()),
        Some(Zone::Graveyard { owner: actor })
    );
    assert!(
        game.events()
            .iter()
            .any(|e| matches!(e, Event::AbilityCompleted { .. }))
    );
    assert_eq!(life(&game, actor), Quantity::from(40));
    replay_equal(&game, &p);
}

#[test]
fn borrowed_damage_source_survives_owner_departure_and_uses_historical_source() {
    let p = prepared(ScenarioConfig::damage_life_borrowed());
    let mut game = Game::new(p.clone());
    let source = game.object_id(CardId(0)).unwrap();
    begin_sorcerer(&mut game, &p);
    entity_target(
        &mut game,
        &p,
        PlayerTargetEntity::Player {
            player: PlayerId(1),
        },
    );
    let old_stack = observe(&game, PlayerId(0)).view.stack[0].handle.clone();
    game.concede(PlayerId(2)).unwrap();
    assert!(matches!(
        settle(&mut game, &p),
        Advance::AwaitingDecision(_)
    ));
    assert!(game.object_id(CardId(0)).is_none());
    let ability = observe(&game, PlayerId(0)).view.stack[0]
        .ability
        .clone()
        .unwrap();
    assert_eq!(ability.source.current, None);
    assert_eq!(observe(&game, PlayerId(0)).view.stack[0].handle, old_stack);
    round(&mut game, &p);
    assert_eq!(life(&game, PlayerId(1)), Quantity::from(39));
    assert!(game.events().iter().any(|e| match e {
        Event::Committed {
            actual: EventOutcome::DamageCommitted { actual, .. },
            ..
        } => actual.iter().any(|damage| damage.source.object == source),
        _ => false,
    }));
    replay_equal(&game, &p);
}

#[test]
fn soul_feast_self_temporarily_zero_then_gain_precedes_any_sba_and_snapshot_replay() {
    let p = prepared(ScenarioConfig::damage_life_low());
    let mut game = Game::new(p.clone());
    targeted_spell(
        &mut game,
        &p,
        FEAST,
        PlayerId(0),
        &[(Color::B, 2), (Color::R, 2), (Color::U, 1)],
    );
    for _ in 0..3 {
        input(&mut game, &p, PlayerResponse::Pass);
    }
    submit(&mut game, PlayerResponse::Pass);
    advance_to_cursor(&mut game, &p, 1);
    assert_eq!(life(&game, PlayerId(0)), Quantity::zero());
    assert_eq!(game.live_players().len(), 4);
    verify(&game, &p);
    assert!(matches!(
        settle(&mut game, &p),
        Advance::AwaitingDecision(_)
    ));
    assert_eq!(life(&game, PlayerId(0)), Quantity::from(4));
    assert_eq!(game.live_players().len(), 4);
    let values = game
        .events()
        .iter()
        .filter_map(|e| match e {
            Event::Committed {
                actual:
                    EventOutcome::LifeLost {
                        previous, current, ..
                    },
                ..
            }
            | Event::Committed {
                actual:
                    EventOutcome::LifeGained {
                        previous, current, ..
                    },
                ..
            } => Some((previous.clone(), current.clone())),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        values,
        vec![
            (Quantity::from(4), Quantity::zero()),
            (Quantity::zero(), Quantity::from(4))
        ]
    );
    replay_equal(&game, &p);
}

#[test]
fn illegal_player_target_cancels_entire_feast_and_hands_including_untargeted_followup() {
    for (oracle, payment) in [
        (FEAST, vec![(Color::B, 2), (Color::R, 2), (Color::U, 1)]),
        (HANDS, vec![(Color::W, 1), (Color::R, 2)]),
    ] {
        let p = prepared(ScenarioConfig::damage_life());
        let mut game = Game::new(p.clone());
        targeted_spell(&mut game, &p, oracle, PlayerId(2), &payment);
        game.concede(PlayerId(2)).unwrap();
        assert!(matches!(
            settle(&mut game, &p),
            Advance::AwaitingDecision(_)
        ));
        round(&mut game, &p);
        assert_eq!(life(&game, PlayerId(0)), Quantity::from(40));
        assert_eq!(
            game.zone(game.object_id(CardId(12)).unwrap()),
            Some(Zone::Library { owner: PlayerId(0) })
        );
        assert!(game.events().iter().any(|e| matches!(
            e,
            Event::SpellCompleted {
                resolved: false,
                ..
            }
        )));
        replay_equal(&game, &p);
    }
}

#[test]
fn started_life_program_survives_caster_departure_without_redirecting_its_gain() {
    let p = prepared(ScenarioConfig::damage_life_low());
    let mut game = Game::new(p.clone());
    targeted_spell(
        &mut game,
        &p,
        FEAST,
        PlayerId(1),
        &[(Color::B, 2), (Color::R, 2), (Color::U, 1)],
    );
    for _ in 0..3 {
        input(&mut game, &p, PlayerResponse::Pass);
    }
    submit(&mut game, PlayerResponse::Pass);
    advance_to_cursor(&mut game, &p, 0);
    game.concede(PlayerId(0)).unwrap();
    assert!(matches!(
        settle(&mut game, &p),
        Advance::AwaitingDecision(_)
    ));
    assert_eq!(game.live_players(), vec![PlayerId(2), PlayerId(3)]);
    let serialized = serde_json::to_value(game.events()).unwrap();
    assert!(
        serialized
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["actual"]["outcome"] == "life_lost" && e["actual"]["player"] == 1)
    );
    assert!(
        !serialized
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["actual"]["outcome"] == "life_gained")
    );
    replay_equal(&game, &p);
}

#[test]
fn hands_empty_library_gains_before_failed_draw_loss_and_targetless_nectar_resolves() {
    let p = prepared(ScenarioConfig::damage_life_empty_library());
    let mut game = Game::new(p.clone());
    targeted_spell(
        &mut game,
        &p,
        HANDS,
        PlayerId(1),
        &[(Color::W, 1), (Color::R, 2)],
    );
    for _ in 0..3 {
        input(&mut game, &p, PlayerResponse::Pass);
    }
    submit(&mut game, PlayerResponse::Pass);
    advance_to_cursor(&mut game, &p, 1);
    assert_eq!(life(&game, PlayerId(1)), Quantity::from(44));
    assert!(game.live_players().contains(&PlayerId(0)));
    assert!(matches!(
        settle(&mut game, &p),
        Advance::AwaitingDecision(_)
    ));
    assert!(!game.live_players().contains(&PlayerId(0)));
    assert!(game.events().iter().any(|e| matches!(e, Event::PlayersLost { losses } if losses == &vec![(PlayerId(0),PlayerLossReason::FailedDraw)])));
    replay_equal(&game, &p);
    let p = prepared(ScenarioConfig::damage_life_low());
    let mut game = Game::new(p.clone());
    begin_spell(&mut game, &p, NECTAR);
    assert!(matches!(request(&game).1, PlayerRequest::Mana { .. }));
    fund_all(&mut game, &p);
    spend(&mut game, &p, &[(Color::W, 1), (Color::R, 1)]);
    round(&mut game, &p);
    assert_eq!(life(&game, PlayerId(0)), Quantity::from(8));
    replay_equal(&game, &p);
}

#[test]
fn healing_nested_commander_draw_exposes_no_hidden_handle_and_stale_receipt_cannot_commit() {
    const TIME_EBB: &str = "30cc8f7b-3c28-40f5-8f8f-157e8212280b";
    for to_command in [false, true] {
        let p = prepared(ScenarioConfig::damage_life_library());
        let mut game = Game::new(p.clone());
        begin_spell(&mut game, &p, TIME_EBB);
        let commander = handle(&game, PlayerId(0), ISAMARU, Zone::Battlefield);
        input(
            &mut game,
            &p,
            PlayerResponse::Targets {
                objects: vec![commander],
            },
        );
        fund_all(&mut game, &p);
        spend(&mut game, &p, &[(Color::U, 1), (Color::B, 2)]);
        round(&mut game, &p);
        input(
            &mut game,
            &p,
            PlayerResponse::CommanderReturn { to_command: false },
        );
        targeted_spell(
            &mut game,
            &p,
            HANDS,
            PlayerId(0),
            &[(Color::W, 1), (Color::R, 2)],
        );
        round(&mut game, &p);
        let (actor, req) = request(&game);
        assert_eq!(actor, PlayerId(0));
        assert!(matches!(
            &req,
            PlayerRequest::CommanderReturn { object: None, .. }
        ));
        assert_eq!(life(&game, PlayerId(0)), Quantity::from(44));
        assert!(
            observe(&game, actor)
                .view
                .objects
                .iter()
                .all(|o| o.zone != Zone::Library { owner: actor })
        );
        let QueryValue::Validated(receipt) = complete(
            &game,
            game.check_player_response(
                actor,
                req.token().clone(),
                PlayerResponse::CommanderReturn { to_command },
                Budget::new(0),
            ),
        ) else {
            panic!("receipt");
        };
        game.concede(PlayerId(2)).unwrap();
        assert!(game.submit(receipt).is_err());
        assert!(matches!(
            settle(&mut game, &p),
            Advance::AwaitingDecision(_)
        ));
        input(
            &mut game,
            &p,
            PlayerResponse::CommanderReturn { to_command },
        );
        assert_eq!(
            game.zone(game.object_id(CardId(14)).unwrap()),
            Some(if to_command {
                Zone::Command { owner: actor }
            } else {
                Zone::Hand { owner: actor }
            })
        );
        assert_eq!(
            game.zone(game.object_id(CardId(12)).unwrap()),
            Some(Zone::Library { owner: actor })
        );
        assert_eq!(life(&game, PlayerId(0)), Quantity::from(44));
        replay_equal(&game, &p);
    }
}

#[test]
fn noncombat_marks_accumulate_across_sources_and_cleanup_clears_survivor_marks() {
    const BEARS: &str = "14c8f55d-d177-4c25-a931-ebeb9e6062a0";
    for through_cleanup in [false, true] {
        let p = prepared(ScenarioConfig::damage_life());
        let mut game = Game::new(p.clone());
        begin_sorcerer(&mut game, &p);
        let bear = handle(&game, PlayerId(0), BEARS, Zone::Battlefield);
        entity_target(&mut game, &p, PlayerTargetEntity::Object { object: bear });
        round(&mut game, &p);
        assert_eq!(
            observe(&game, PlayerId(0))
                .view
                .objects
                .iter()
                .find(|o| o.oracle_id == BEARS)
                .unwrap()
                .marked_damage,
            Quantity::from(1)
        );
        if through_cleanup {
            passive_until(&mut game, &p, |g, r| {
                matches!(r, PlayerRequest::Priority { .. })
                    && g.turn().number == 5
                    && observe(g, PlayerId(0)).view.turn_owner == PlayerId(0)
                    && g.turn().step == TurnStep::PrecombatMain
            });
            assert_eq!(
                observe(&game, PlayerId(0))
                    .view
                    .objects
                    .iter()
                    .find(|o| o.oracle_id == BEARS)
                    .unwrap()
                    .marked_damage,
                Quantity::zero()
            );
            begin_sorcerer(&mut game, &p);
        } else {
            input(&mut game, &p, PlayerResponse::Pass);
            input(&mut game, &p, PlayerResponse::Pass);
            assert_eq!(request(&game).0, PlayerId(2));
            let object = observe(&game, PlayerId(2))
                .view
                .objects
                .into_iter()
                .find(|o| o.oracle_id == SORCERER && o.controller == PlayerId(2))
                .unwrap()
                .handle;
            input(
                &mut game,
                &p,
                PlayerResponse::ActivateAbility { object, ability: 0 },
            );
        }
        let actor = request(&game).0;
        let bear = handle(&game, actor, BEARS, Zone::Battlefield);
        entity_target(&mut game, &p, PlayerTargetEntity::Object { object: bear });
        round(&mut game, &p);
        assert_eq!(
            game.zone(game.object_id(CardId(15)).unwrap()),
            Some(if through_cleanup {
                Zone::Battlefield
            } else {
                Zone::Graveyard { owner: PlayerId(1) }
            })
        );
        if through_cleanup {
            assert_eq!(
                observe(&game, PlayerId(0))
                    .view
                    .objects
                    .iter()
                    .find(|o| o.oracle_id == BEARS)
                    .unwrap()
                    .marked_damage,
                Quantity::from(1)
            );
        }
        replay_equal(&game, &p);
    }
}

#[test]
fn entity_receipts_are_state_scoped_and_rehashed_source_or_target_forgeries_fail_restore() {
    let p = prepared(ScenarioConfig::damage_life());
    let mut game = Game::new(p.clone());
    begin_sorcerer(&mut game, &p);
    let (actor, req) = request(&game);
    let foreign = handle(&game, PlayerId(1), SORCERER, Zone::Battlefield);
    reject(
        &game,
        actor,
        req.token().clone(),
        PlayerResponse::EntityTargets {
            entities: vec![PlayerTargetEntity::Object { object: foreign }],
        },
    );
    let response = PlayerResponse::EntityTargets {
        entities: vec![PlayerTargetEntity::Player {
            player: PlayerId(1),
        }],
    };
    let QueryValue::Validated(receipt) = complete(
        &game,
        game.check_player_response(actor, req.token().clone(), response, Budget::new(0)),
    ) else {
        panic!("receipt");
    };
    game.concede(PlayerId(2)).unwrap();
    assert!(game.submit(receipt).is_err());
    assert!(matches!(
        settle(&mut game, &p),
        Advance::AwaitingDecision(_)
    ));
    entity_target(
        &mut game,
        &p,
        PlayerTargetEntity::Player {
            player: PlayerId(1),
        },
    );
    let snapshot: serde_json::Value = serde_json::from_str(&game.snapshot().unwrap()).unwrap();
    for source in [false, true] {
        let mut forged = snapshot.clone();
        if source {
            forged["state"]["stack"][0]["source_facts"]["controller"] = serde_json::json!(3);
        } else {
            forged["state"]["stack"][0]["entity_targets"][0]["player"] = serde_json::json!(3);
        }
        forged["state_sha256"] =
            serde_json::json!(mtg_contracts::digest(&forged["state"]).unwrap());
        assert!(Game::restore(p.clone(), &serde_json::to_string(&forged).unwrap()).is_err());
    }
    round(&mut game, &p);
    assert_eq!(life(&game, PlayerId(1)), Quantity::from(39));
    replay_equal(&game, &p);
}

#[test]
fn bounced_recast_sorcerer_rejects_immediate_targeted_tap_activation() {
    let p = prepared(ScenarioConfig::damage_life());
    let mut game = Game::new(p.clone());
    let original = game.object_id(CardId(0)).unwrap();
    input(&mut game, &p, PlayerResponse::Pass);
    begin_spell(&mut game, &p, UNSUMMON);
    let object = handle(&game, PlayerId(1), SORCERER, Zone::Battlefield);
    input(
        &mut game,
        &p,
        PlayerResponse::Targets {
            objects: vec![object],
        },
    );
    fund_all(&mut game, &p);
    spend(&mut game, &p, &[(Color::U, 1)]);
    round(&mut game, &p);
    begin_spell(&mut game, &p, SORCERER);
    fund_all(&mut game, &p);
    spend(&mut game, &p, &[(Color::U, 1), (Color::R, 2)]);
    round(&mut game, &p);
    assert_ne!(game.object_id(CardId(0)), Some(original));
    let source = handle(&game, PlayerId(0), SORCERER, Zone::Battlefield);
    let (actor, req) = request(&game);
    reject(
        &game,
        actor,
        req.token().clone(),
        PlayerResponse::ActivateAbility {
            object: source,
            ability: 0,
        },
    );
    assert!(
        !observe(&game, actor)
            .view
            .objects
            .iter()
            .find(|o| o.oracle_id == SORCERER && o.controller == actor)
            .unwrap()
            .attack_ready
    );
    replay_equal(&game, &p);
}

#[test]
fn pending_target_departure_and_unpayable_borrowed_tap_reverse_without_damage_or_tap() {
    for source_leaves in [false, true] {
        for accepted in [false, true] {
            let p = prepared(if source_leaves {
                ScenarioConfig::damage_life_borrowed()
            } else {
                ScenarioConfig::damage_life()
            });
            let mut game = Game::new(p.clone());
            begin_sorcerer(&mut game, &p);
            if accepted {
                submit(
                    &mut game,
                    PlayerResponse::EntityTargets {
                        entities: vec![PlayerTargetEntity::Player {
                            player: PlayerId(1),
                        }],
                    },
                );
            }
            game.concede(if source_leaves {
                PlayerId(2)
            } else {
                PlayerId(1)
            })
            .unwrap();
            assert!(matches!(
                settle(&mut game, &p),
                Advance::AwaitingDecision(_)
            ));
            if !source_leaves && !accepted {
                // No target was selected: the still-payable source can choose another live target.
                entity_target(
                    &mut game,
                    &p,
                    PlayerTargetEntity::Player {
                        player: PlayerId(0),
                    },
                );
                round(&mut game, &p);
                assert_eq!(life(&game, PlayerId(0)), Quantity::from(39));
            } else {
                assert!(matches!(request(&game).1, PlayerRequest::Priority { .. }));
                assert!(observe(&game, PlayerId(0)).view.stack.is_empty());
                assert!(
                    !game
                        .events()
                        .iter()
                        .any(|e| matches!(e, Event::AbilityActivated { .. }))
                );
                if source_leaves {
                    assert!(game.object_id(CardId(0)).is_none());
                } else {
                    assert!(
                        !observe(&game, PlayerId(0))
                            .view
                            .objects
                            .iter()
                            .find(|o| o.oracle_id == SORCERER && o.controller == PlayerId(0))
                            .unwrap()
                            .tapped
                    );
                }
            }
            replay_equal(&game, &p);
        }
    }
}

#[test]
fn accepted_any_player_self_target_survives_unrelated_concession_during_announcement() {
    let p = prepared(ScenarioConfig::damage_life_low());
    let mut game = Game::new(p.clone());
    begin_spell(&mut game, &p, FEAST);
    submit(
        &mut game,
        PlayerResponse::PlayerTargets {
            players: vec![PlayerId(0)],
        },
    );
    game.concede(PlayerId(2)).unwrap();
    assert!(matches!(
        settle(&mut game, &p),
        Advance::AwaitingDecision(_)
    ));
    assert!(matches!(request(&game).1, PlayerRequest::Mana { .. }));
    fund_all(&mut game, &p);
    spend(
        &mut game,
        &p,
        &[(Color::B, 2), (Color::R, 2), (Color::U, 1)],
    );
    round(&mut game, &p);
    assert_eq!(life(&game, PlayerId(0)), Quantity::from(4));
    assert!(
        !game
            .events()
            .iter()
            .any(|e| matches!(e, Event::CastReversed { .. }))
    );
    replay_equal(&game, &p);
}

#[test]
fn started_damage_invocation_retains_source_after_controller_departure_and_skips_departed_target() {
    for departed in [PlayerId(0), PlayerId(1)] {
        let p = prepared(ScenarioConfig::damage_life());
        let mut game = Game::new(p.clone());
        let source = game.object_id(CardId(0)).unwrap();
        begin_sorcerer(&mut game, &p);
        entity_target(
            &mut game,
            &p,
            PlayerTargetEntity::Player {
                player: PlayerId(1),
            },
        );
        for _ in 0..3 {
            input(&mut game, &p, PlayerResponse::Pass);
        }
        submit(&mut game, PlayerResponse::Pass);
        advance_to_cursor(&mut game, &p, 0);
        game.concede(departed).unwrap();
        assert!(matches!(
            settle(&mut game, &p),
            Advance::AwaitingDecision(_)
        ));
        let damage = game
            .events()
            .iter()
            .filter_map(|e| match e {
                Event::Committed {
                    actual: EventOutcome::DamageCommitted { actual, .. },
                    ..
                } => actual.first(),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(damage.len(), usize::from(departed == PlayerId(0)));
        if departed == PlayerId(0) {
            assert_eq!(life(&game, PlayerId(1)), Quantity::from(39));
            assert_eq!(damage[0].source.object, source);
            assert_eq!(damage[0].controller, PlayerId(0));
            assert!(game.object_id(CardId(0)).is_none());
        }
        assert_eq!(
            game.events()
                .iter()
                .filter(|e| matches!(e, Event::AbilityCompleted { .. }))
                .count(),
            1
        );
        replay_equal(&game, &p);
    }
}
