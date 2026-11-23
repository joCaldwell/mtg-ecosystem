//! Independent player-facing regressions for the exact action profiles.
use crate::support::visible_controller;
use crate::support::{action_scenario, observe, reject, replay_equal, request, settle, submit};
use mtg_contracts::{Color, Quantity};
use mtg_core::*;
use std::collections::BTreeMap;

const EDICT: &str = "10c585c4-bf5b-4d8f-94a9-e9a5036a688f";

const SWAMP: &str = "56719f6a-1a6c-4c0a-8d21-18f7d7350b68";
const FOREST: &str = "b34bb2dc-c1af-4d77-b0b3-a0fb342a5fc6";

fn input(game: &mut Game, response: PlayerResponse) {
    submit(game, response);

    assert!(matches!(settle(game), Advance::AwaitingDecision(_)));
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
            o.zone == Zone::Battlefield
                && o.owner == PlayerId(2)
                && visible_controller(o) == PlayerId(1)
        })
        .expect("borrowed creature")
        .handle
}
fn activate_land(game: &mut Game, oracle: &str) {
    let actor = request(game).0;
    let object = observe(game, actor)
        .view
        .objects
        .into_iter()
        .find(|o| {
            visible_controller(o) == actor
                && o.oracle_id == oracle
                && o.zone == Zone::Battlefield
                && !o.tapped
        })
        .expect("untapped mana source")
        .handle;
    input(game, PlayerResponse::ActivateMana { object });
}
fn pass_four(game: &mut Game) {
    for _ in 0..4 {
        input(game, PlayerResponse::Pass);
    }
}
fn edict(game: &mut Game, target: PlayerId) {
    let card = handle(game, PlayerId(0), EDICT, Zone::Hand { owner: PlayerId(0) });
    input(game, PlayerResponse::Cast { card });
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
        PlayerResponse::PlayerTargets {
            players: vec![target],
        },
    );
    activate_land(game, SWAMP);
    activate_land(game, FOREST);
    input(game, PlayerResponse::FinishMana { undo_mana: true });
    input(
        game,
        PlayerResponse::SpendMana {
            spending: BTreeMap::from([
                (Color::B, Quantity::from(1)),
                (Color::G, Quantity::from(1)),
            ]),
        },
    );
    pass_four(game);
}

#[test]
fn opponent_with_no_creatures_is_legal_and_sacrifice_uses_control_not_ownership() {
    let p = action_scenario(ScenarioConfig::actions_borrowed());
    let mut empty = Game::new(p.clone());
    edict(&mut empty, PlayerId(2));
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
    edict(&mut game, PlayerId(1));
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
    input(&mut game, PlayerResponse::Sacrifice { object: chosen });
    assert_eq!(
        game.zone(game.object_id(CardId(5)).unwrap()),
        Some(Zone::Graveyard { owner: PlayerId(2) })
    );
    assert!(observe(&game, PlayerId(0)).view.stack.is_empty());
    replay_equal(&game, &p);
}

#[test]
fn sacrifice_choice_revalidates_source_and_affected_player_departure() {
    let p = action_scenario(ScenarioConfig::actions_borrowed());
    let mut game = Game::new(p.clone());
    edict(&mut game, PlayerId(1));
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
            assert!(matches!(settle(&mut branch), Advance::AwaitingDecision(_)));
            if departing == PlayerId(0) {
                if !accepted {
                    input(&mut branch, PlayerResponse::Sacrifice { object: chosen });
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
