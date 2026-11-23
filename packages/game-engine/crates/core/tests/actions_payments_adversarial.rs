//! Independent player-facing regressions for the exact action profiles.
use mtg_contracts::{Catalog, Color, Quantity};
use mtg_core::*;
use std::collections::BTreeMap;

const EDICT: &str = "10c585c4-bf5b-4d8f-94a9-e9a5036a688f";

const SWAMP: &str = "56719f6a-1a6c-4c0a-8d21-18f7d7350b68";
const FOREST: &str = "b34bb2dc-c1af-4d77-b0b3-a0fb342a5fc6";

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
fn borrowed_creature(game: &Game, viewer: PlayerId) -> ObjectHandle {
    observe(game, viewer)
        .view
        .objects
        .into_iter()
        .find(|o| {
            o.zone == Zone::Battlefield && o.owner == PlayerId(2) && o.controller == PlayerId(1)
        })
        .expect("borrowed creature")
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
    let chosen = borrowed_creature(&game, actor);
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
            let chosen = borrowed_creature(&branch, PlayerId(1));
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
