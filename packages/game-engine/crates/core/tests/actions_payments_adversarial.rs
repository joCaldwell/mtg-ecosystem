//! Independent player-facing regressions for the exact action profiles.
use mtg_contracts::{Catalog, Color, Quantity};
use mtg_core::*;
use std::collections::BTreeMap;

const EDICT: &str = "10c585c4-bf5b-4d8f-94a9-e9a5036a688f";
const RESEARCHER: &str = "240a349a-79f8-4e11-9a05-a78148e70a62";
const ISAMARU: &str = "cfb756dd-6fe1-430a-8f04-8b10cac62a86";
const TIME_EBB: &str = "30cc8f7b-3c28-40f5-8f8f-157e8212280b";
const SWAMP: &str = "56719f6a-1a6c-4c0a-8d21-18f7d7350b68";
const FOREST: &str = "b34bb2dc-c1af-4d77-b0b3-a0fb342a5fc6";
const ISLAND: &str = "b2c6aa39-2d2a-459c-a555-fb48ba993373";

fn prepared(config: ScenarioConfig) -> Prepared {
    Prepared::admit(
        Catalog::read(include_str!(
            "../../../tests/fixtures/expected-action-ir.json"
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
fn activate_land(game: &mut Game, p: &Prepared, oracle: &str) {
    let actor = request(game).0;
    let object = observe(game, actor)
        .view
        .objects
        .into_iter()
        .find(|o| {
            o.controller == actor
                && o.oracle_id == oracle
                && o.zone == Zone::Battlefield
                && !o.tapped
        })
        .expect("untapped mana source")
        .handle;
    input(game, p, PlayerResponse::ActivateMana { object });
}
fn replay_equal(game: &Game, p: &Prepared) {
    let replay = Game::replay(p.clone(), &game.replay_journal()).unwrap();
    assert_eq!(replay.state_digest().unwrap(), game.state_digest().unwrap());
}
fn pass_four(game: &mut Game, p: &Prepared) {
    for _ in 0..4 {
        input(game, p, PlayerResponse::Pass);
    }
}
fn edict(game: &mut Game, p: &Prepared, target: PlayerId) {
    let card = handle(game, PlayerId(0), EDICT, Zone::Hand { owner: PlayerId(0) });
    input(game, p, PlayerResponse::Cast { card });
    let (actor, req) = request(game);
    assert!(matches!(req, PlayerRequest::PlayerTargets { .. }));
    reject(
        game,
        actor,
        req.token().clone(),
        PlayerResponse::PlayerTargets {
            players: vec![actor],
        },
    );
    input(
        game,
        p,
        PlayerResponse::PlayerTargets {
            players: vec![target],
        },
    );
    activate_land(game, p, SWAMP);
    activate_land(game, p, FOREST);
    input(game, p, PlayerResponse::FinishMana { undo_mana: true });
    input(
        game,
        p,
        PlayerResponse::SpendMana {
            spending: BTreeMap::from([
                (Color::B, Quantity::from(1)),
                (Color::G, Quantity::from(1)),
            ]),
        },
    );
    pass_four(game, p);
}
fn activate_researcher(game: &mut Game, p: &Prepared) -> ObjectHandle {
    input(game, p, PlayerResponse::Pass);
    assert_eq!(request(game).0, PlayerId(1));
    let source = handle(game, PlayerId(1), RESEARCHER, Zone::Battlefield);
    input(
        game,
        p,
        PlayerResponse::ActivateAbility {
            object: source.clone(),
            ability: 0,
        },
    );
    assert_eq!(
        game.zone(game.object_id(CardId(5)).unwrap()),
        Some(Zone::Graveyard {
            owner: if p.config() == &ScenarioConfig::actions_borrowed() {
                PlayerId(2)
            } else {
                PlayerId(1)
            }
        })
    );
    let view = observe(game, PlayerId(1)).view;
    assert_eq!(view.stack.len(), 1);
    assert!(
        view.objects
            .iter()
            .all(|o| o.oracle_id != RESEARCHER || o.zone != Zone::Stack)
    );
    let (actor, req) = request(game);
    reject(
        game,
        actor,
        req.token().clone(),
        PlayerResponse::ActivateAbility {
            object: source.clone(),
            ability: 0,
        },
    );
    source
}

#[test]
fn opponent_with_no_creatures_is_legal_and_sacrifice_uses_control_not_ownership() {
    let p = prepared(ScenarioConfig::actions_borrowed());
    let mut empty = Game::new(p.clone());
    edict(&mut empty, &p, PlayerId(2));
    assert!(matches!(request(&empty).1, PlayerRequest::Priority { .. }));
    assert_eq!(
        empty.zone(empty.object_id(CardId(4)).unwrap()),
        Some(Zone::Graveyard { owner: PlayerId(0) })
    );
    assert_eq!(
        empty.zone(empty.object_id(CardId(5)).unwrap()),
        Some(Zone::Battlefield)
    );
    replay_equal(&empty, &p);
    let mut game = Game::new(p.clone());
    edict(&mut game, &p, PlayerId(1));
    let (actor, req) = request(&game);
    assert_eq!(actor, PlayerId(1));
    assert!(matches!(req, PlayerRequest::Sacrifice { .. }));
    let chosen = handle(&game, actor, RESEARCHER, Zone::Battlefield);
    reject(
        &game,
        PlayerId(0),
        req.token().clone(),
        PlayerResponse::Sacrifice {
            object: chosen.clone(),
        },
    );
    input(&mut game, &p, PlayerResponse::Sacrifice { object: chosen });
    assert_eq!(
        game.zone(game.object_id(CardId(5)).unwrap()),
        Some(Zone::Graveyard { owner: PlayerId(2) })
    );
    assert!(observe(&game, PlayerId(0)).view.stack.is_empty());
    replay_equal(&game, &p);
}

#[test]
fn sacrifice_choice_revalidates_source_and_affected_player_departure() {
    let p = prepared(ScenarioConfig::actions_borrowed());
    let mut game = Game::new(p.clone());
    edict(&mut game, &p, PlayerId(1));
    for departing in [PlayerId(0), PlayerId(1), PlayerId(2)] {
        for accepted in [false, true] {
            let mut branch = game.fork();
            let chosen = handle(&branch, PlayerId(1), RESEARCHER, Zone::Battlefield);
            if accepted {
                submit(
                    &mut branch,
                    PlayerResponse::Sacrifice {
                        object: chosen.clone(),
                    },
                );
            }
            branch.concede(departing).unwrap();
            assert!(matches!(
                settle(&mut branch, &p),
                Advance::AwaitingDecision(_)
            ));
            if departing == PlayerId(0) {
                if !accepted {
                    input(
                        &mut branch,
                        &p,
                        PlayerResponse::Sacrifice { object: chosen },
                    );
                }
                assert_eq!(
                    branch.zone(branch.object_id(CardId(5)).unwrap()),
                    Some(Zone::Graveyard { owner: PlayerId(2) })
                );
            } else if departing == PlayerId(1) {
                assert_eq!(
                    branch.zone(branch.object_id(CardId(5)).unwrap()),
                    Some(Zone::Exile)
                );
                assert!(matches!(request(&branch).1, PlayerRequest::Priority { .. }));
            } else {
                assert!(branch.object_id(CardId(5)).is_none());
                let (actor, req) = request(&branch);
                assert_eq!(actor, PlayerId(1));
                assert!(matches!(req, PlayerRequest::Sacrifice { .. }));
                reject(
                    &branch,
                    actor,
                    req.token().clone(),
                    PlayerResponse::Sacrifice { object: chosen },
                );
                let replacement = handle(
                    &branch,
                    actor,
                    "14c8f55d-d177-4c25-a931-ebeb9e6062a0",
                    Zone::Battlefield,
                );
                input(
                    &mut branch,
                    &p,
                    PlayerResponse::Sacrifice {
                        object: replacement,
                    },
                );
                assert_eq!(
                    branch.zone(branch.object_id(CardId(1)).unwrap()),
                    Some(Zone::Graveyard { owner: PlayerId(1) })
                );
            }
            replay_equal(&branch, &p);
        }
    }
}

#[test]
fn borrowed_ability_survives_source_owner_but_not_activator_departure_before_resolution() {
    let p = prepared(ScenarioConfig::actions_borrowed());
    let mut game = Game::new(p.clone());
    activate_researcher(&mut game, &p);
    for departing in [PlayerId(1), PlayerId(2)] {
        let mut branch = game.fork();
        branch.concede(departing).unwrap();
        assert!(matches!(
            settle(&mut branch, &p),
            Advance::AwaitingDecision(_)
        ));
        if departing == PlayerId(1) {
            assert!(observe(&branch, PlayerId(0)).view.stack.is_empty());
            assert_eq!(
                branch.zone(branch.object_id(CardId(5)).unwrap()),
                Some(Zone::Graveyard { owner: PlayerId(2) })
            );
        } else {
            assert!(branch.object_id(CardId(5)).is_none());
            for _ in 0..3 {
                input(&mut branch, &p, PlayerResponse::Pass);
            }
            assert!(matches!(
                request(&branch),
                (PlayerId(1), PlayerRequest::ResolveDiscard { .. })
            ));
            let discard = handle(
                &branch,
                PlayerId(1),
                "bc71ebf6-2056-41f7-be35-b2e5c34afa99",
                Zone::Hand { owner: PlayerId(1) },
            );
            input(
                &mut branch,
                &p,
                PlayerResponse::ResolveDiscard {
                    objects: vec![discard],
                },
            );
            assert!(observe(&branch, PlayerId(0)).view.stack.is_empty());
        }
        replay_equal(&branch, &p);
    }
}

#[test]
fn failed_ability_draw_still_discards_before_player_loss_sba() {
    let p = prepared(ScenarioConfig::actions_empty_library());
    let mut game = Game::new(p.clone());
    activate_researcher(&mut game, &p);
    pass_four(&mut game, &p);
    let (actor, req) = request(&game);
    assert_eq!(actor, PlayerId(1));
    assert!(matches!(req, PlayerRequest::ResolveDiscard { .. }));
    assert!(game.live_players().contains(&actor));
    let discard = handle(
        &game,
        actor,
        "837182db-1bf3-4a2c-bd01-1af9d9873561",
        Zone::Hand { owner: actor },
    );
    input(
        &mut game,
        &p,
        PlayerResponse::ResolveDiscard {
            objects: vec![discard],
        },
    );
    assert!(!game.live_players().contains(&actor));
    assert!(game.object_id(CardId(5)).is_none());
    replay_equal(&game, &p);
}

#[test]
fn noncard_stack_source_cost_and_program_cannot_be_forged() {
    let p = prepared(ScenarioConfig::actions());
    let mut game = Game::new(p.clone());
    activate_researcher(&mut game, &p);
    for mutation in 0..4 {
        let mut snapshot: serde_json::Value =
            serde_json::from_str(&game.snapshot().unwrap()).unwrap();
        match mutation {
            0 => snapshot["state"]["stack"][0]["kind"]["ability"] = 99.into(),
            1 => snapshot["state"]["stack"][0]["controller"] = 0.into(),
            2 => snapshot["state"]["stack"][0]["source"]["incarnation"] = 1.into(),
            _ => {
                snapshot["state"]["objects"]["5"]["zone"] =
                    serde_json::json!({"zone":"battlefield"})
            }
        }
        snapshot["state_sha256"] = mtg_contracts::digest(&snapshot["state"]).unwrap().into();
        assert!(Game::restore(p.clone(), &serde_json::to_string(&snapshot).unwrap()).is_err());
    }
}

#[test]
fn mandatory_discard_rejects_invalid_domains_and_recovers_validation_work() {
    let p = prepared(ScenarioConfig::actions());
    let mut game = Game::new(p.clone());
    activate_researcher(&mut game, &p);
    pass_four(&mut game, &p);
    let (actor, req) = request(&game);
    let PlayerRequest::ResolveDiscard { ref candidates, .. } = req else {
        panic!("discard")
    };
    let chosen = candidates[0].clone();
    for objects in [
        vec![],
        vec![chosen.clone(), chosen.clone()],
        vec![handle(
            &game,
            actor,
            RESEARCHER,
            Zone::Graveyard { owner: actor },
        )],
    ] {
        reject(
            &game,
            actor,
            req.token().clone(),
            PlayerResponse::ResolveDiscard { objects },
        );
    }
    reject(
        &game,
        PlayerId(0),
        req.token().clone(),
        PlayerResponse::ResolveDiscard {
            objects: vec![chosen.clone()],
        },
    );
    let before = game.snapshot().unwrap();
    let QueryResult::Incomplete { work, .. } = game.check_player_response(
        actor,
        req.token().clone(),
        PlayerResponse::ResolveDiscard {
            objects: vec![chosen],
        },
        Budget::new(0),
    ) else {
        panic!("query")
    };
    let saved = game.save_work(&work).unwrap();
    assert_eq!(before, game.snapshot().unwrap());
    let mut restored = Game::restore(p.clone(), &before).unwrap();
    let work = restored.restore_work(&saved).unwrap();
    let QueryValue::Validated(receipt) =
        complete(&restored, restored.resume_work(work, Budget::new(0)))
    else {
        panic!("receipt")
    };
    assert!(game.submit(receipt).is_err());
    let work = restored.restore_work(&saved).unwrap();
    let QueryValue::Validated(receipt) =
        complete(&restored, restored.resume_work(work, Budget::new(0)))
    else {
        panic!("receipt")
    };
    restored.submit(receipt).unwrap();
    assert!(matches!(
        settle(&mut restored, &p),
        Advance::AwaitingDecision(_)
    ));
    replay_equal(&restored, &p);
}

#[test]
fn freshly_cast_researcher_activates_immediately_and_keeps_historical_source_binding() {
    let p = prepared(ScenarioConfig::actions());
    let mut game = Game::new(p.clone());
    input(&mut game, &p, PlayerResponse::Pass);
    let card = handle(
        &game,
        PlayerId(1),
        "837182db-1bf3-4a2c-bd01-1af9d9873561",
        Zone::Hand { owner: PlayerId(1) },
    );
    input(&mut game, &p, PlayerResponse::Cast { card });
    let target = handle(&game, PlayerId(1), RESEARCHER, Zone::Battlefield);
    input(
        &mut game,
        &p,
        PlayerResponse::Targets {
            objects: vec![target],
        },
    );
    activate_land(&mut game, &p, ISLAND);
    input(
        &mut game,
        &p,
        PlayerResponse::FinishMana { undo_mana: true },
    );
    input(
        &mut game,
        &p,
        PlayerResponse::SpendMana {
            spending: BTreeMap::from([(Color::U, Quantity::from(1))]),
        },
    );
    pass_four(&mut game, &p);
    for _ in 0..200 {
        let req = request(&game).1;
        if game.turn().step == TurnStep::PrecombatMain && game.turn_owner() == PlayerId(1) {
            break;
        }
        let response = match req {
            PlayerRequest::Priority { .. } => PlayerResponse::Pass,
            PlayerRequest::Attackers { .. } => PlayerResponse::Attackers { attacks: vec![] },
            PlayerRequest::Blockers { .. } => PlayerResponse::Blockers { blocks: vec![] },
            _ => panic!("unexpected newborn setup request"),
        };
        input(&mut game, &p, response);
    }
    assert_eq!(game.turn_owner(), PlayerId(1));
    assert_eq!(game.turn().step, TurnStep::PrecombatMain);
    let card = handle(
        &game,
        PlayerId(1),
        RESEARCHER,
        Zone::Hand { owner: PlayerId(1) },
    );
    input(&mut game, &p, PlayerResponse::Cast { card });
    activate_land(&mut game, &p, ISLAND);
    input(
        &mut game,
        &p,
        PlayerResponse::FinishMana { undo_mana: true },
    );
    input(
        &mut game,
        &p,
        PlayerResponse::SpendMana {
            spending: BTreeMap::from([(Color::U, Quantity::from(1))]),
        },
    );
    pass_four(&mut game, &p);
    let view = observe(&game, PlayerId(1)).view;
    let newborn = view
        .objects
        .iter()
        .find(|o| o.oracle_id == RESEARCHER && o.zone == Zone::Battlefield)
        .unwrap();
    assert!(!newborn.attack_ready);
    let source = newborn.handle.clone();
    input(
        &mut game,
        &p,
        PlayerResponse::ActivateAbility {
            object: source.clone(),
            ability: 0,
        },
    );
    let before = game.snapshot().unwrap();
    let view = observe(&game, PlayerId(1)).view;
    let entry = &view.stack[0];
    assert_eq!(entry.spell, None);
    let ability = entry.ability.as_ref().unwrap();
    assert_eq!(ability.source.announced, source);
    assert_eq!(ability.source.current, None);
    assert_eq!(ability.oracle_id, RESEARCHER);
    let again = observe(&game, PlayerId(1));
    assert_eq!(
        again.view.stack[0].ability.as_ref().unwrap().handle,
        ability.handle
    );
    assert_eq!(before, game.snapshot().unwrap());
    assert_eq!(
        game.zone(game.object_id(CardId(5)).unwrap()),
        Some(Zone::Graveyard { owner: PlayerId(1) })
    );
    replay_equal(&game, &p);
}

#[test]
fn ability_draw_commander_replacement_resumes_mandatory_discard_before_return_sba() {
    let p = prepared(ScenarioConfig::actions());
    let mut game = Game::new(p.clone());
    let card = handle(
        &game,
        PlayerId(0),
        TIME_EBB,
        Zone::Hand { owner: PlayerId(0) },
    );
    input(&mut game, &p, PlayerResponse::Cast { card });
    let target = handle(&game, PlayerId(0), ISAMARU, Zone::Battlefield);
    input(
        &mut game,
        &p,
        PlayerResponse::Targets {
            objects: vec![target],
        },
    );
    for _ in 0..3 {
        activate_land(&mut game, &p, ISLAND);
    }
    input(
        &mut game,
        &p,
        PlayerResponse::FinishMana { undo_mana: true },
    );
    input(
        &mut game,
        &p,
        PlayerResponse::SpendMana {
            spending: BTreeMap::from([(Color::U, Quantity::from(3))]),
        },
    );
    pass_four(&mut game, &p);
    input(
        &mut game,
        &p,
        PlayerResponse::CommanderReturn { to_command: false },
    );
    activate_researcher(&mut game, &p);
    pass_four(&mut game, &p);
    assert!(matches!(
        request(&game),
        (
            PlayerId(1),
            PlayerRequest::CommanderReturn {
                object: None,
                destination: Zone::Hand { owner: PlayerId(1) },
                ..
            }
        )
    ));
    for to_command in [false, true] {
        let mut branch = game.fork();
        input(
            &mut branch,
            &p,
            PlayerResponse::CommanderReturn { to_command },
        );
        assert!(matches!(
            request(&branch).1,
            PlayerRequest::ResolveDiscard { .. }
        ));
        let discard = handle(
            &branch,
            PlayerId(1),
            if to_command {
                "837182db-1bf3-4a2c-bd01-1af9d9873561"
            } else {
                ISAMARU
            },
            Zone::Hand { owner: PlayerId(1) },
        );
        input(
            &mut branch,
            &p,
            PlayerResponse::ResolveDiscard {
                objects: vec![discard],
            },
        );
        if !to_command {
            assert!(matches!(
                request(&branch).1,
                PlayerRequest::CommanderReturn {
                    destination: Zone::Graveyard { owner: PlayerId(1) },
                    ..
                }
            ));
            assert!(observe(&branch, PlayerId(1)).view.stack.is_empty());
            input(
                &mut branch,
                &p,
                PlayerResponse::CommanderReturn { to_command: true },
            );
        }
        assert!(observe(&branch, PlayerId(1)).view.stack.is_empty());
        assert_eq!(
            branch.zone(branch.object_id(CardId(0)).unwrap()),
            Some(Zone::Command { owner: PlayerId(1) })
        );
        replay_equal(&branch, &p);
    }
}
