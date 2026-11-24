//! Independent player-facing regression evidence for damage and hidden draws.
use crate::support::visible_controller;
use crate::support::{
    action_scenario, complete, input, observe, receipt, reject, replay_equal, request, settle,
    submit,
};
use mtg_contracts::Quantity;
use mtg_core::*;

const ISAMARU: &str = "cfb756dd-6fe1-430a-8f04-8b10cac62a86";
const BEARS: &str = "14c8f55d-d177-4c25-a931-ebeb9e6062a0";

fn library_prepared() -> Prepared {
    action_scenario(ScenarioConfig::commander_library())
}
fn damage_prepared(borrowed: bool) -> Prepared {
    action_scenario(if borrowed {
        ScenarioConfig::commander_damage_borrowed()
    } else {
        ScenarioConfig::commander_damage()
    })
}
fn passive(req: Decision<Viewer>) -> Answer {
    match req.clone() {
        Decision {
            reason: Reason::Priority,
            ..
        } => Answer::Pick {
            picks: vec![vec![0]],
        },
        Decision {
            reason: Reason::Attackers,
            ..
        } => req.attack(vec![]),
        Decision {
            reason: Reason::Blockers,
            ..
        } => req.block(vec![]),
        found @ Decision {
            reason: Reason::Discard,
            ..
        } => {
            let count = found.required_count();
            let candidates = found.objects_offered();
            req.pick_objects(candidates.into_iter().take(count).collect())
        }
        _ => panic!("unexpected passive request"),
    }
}
fn until(game: &mut Game, wanted: impl Fn(&Game, &Decision<Viewer>) -> bool) {
    for _ in 0..250 {
        let req = request(game).1;
        if wanted(game, &req) {
            return;
        }
        input(game, passive(req));
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
fn tuck(game: &mut Game, oracle: &str) -> ObjectHandle {
    let card = observe(game, PlayerId(0))
        .view
        .objects
        .into_iter()
        .find(|o| o.zone == Zone::Hand { owner: PlayerId(0) })
        .unwrap()
        .handle;
    input(
        game,
        game.next_test_viewer_decision().select(Candidate::Action {
            action: Action::Cast { card },
        }),
    );
    let target = target_handle(game, PlayerId(0), oracle);
    input(
        game,
        game.next_test_viewer_decision()
            .targets(vec![target.clone()]),
    );
    for _ in 0..3 {
        let land = observe(game, PlayerId(0))
            .view
            .objects
            .into_iter()
            .find(|o| {
                visible_controller(o) == PlayerId(0) && o.zone == Zone::Battlefield && !o.tapped
            })
            .unwrap()
            .handle;
        input(game, game.next_test_viewer_decision().activate_mana(land));
    }
    input(
        game,
        Answer::Pay {
            pay: PayAnswer::Finish {
                undo_on_failure: true,
            },
        },
    );
    for _ in 0..4 {
        input(
            game,
            Answer::Pick {
                picks: vec![vec![0]],
            },
        );
    }
    target
}
fn declined_tuck_to_draw(game: &mut Game) {
    tuck(game, ISAMARU);
    input(
        game,
        game.next_test_viewer_decision().select(Candidate::Flag {
            flag: Flag::Destination,
        }),
    );
    assert_eq!(
        library_order(game, PlayerId(1)),
        vec![CardId(4), CardId(10), CardId(11)]
    );
    until(game, |_, req| {
        matches!(
            req,
            Decision {
                reason: Reason::CommanderReturn {
                    destination: Zone::Hand { .. },
                    ..
                },
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
    declined_tuck_to_draw(&mut game);
    let (owner, req) = request(&game);
    let Decision {
        id: token,
        reason:
            Reason::CommanderReturn {
                object,
                designation: returned_designation,
                ..
            },
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
            game.next_test_viewer_decision().select(Candidate::Flag {
                flag: Flag::CommandZone,
            }),
        );
    }
    reject(
        &game,
        owner,
        token.clone(),
        Answer::Pick {
            picks: vec![vec![u32::MAX]],
        },
    );
    for to_command in [false, true] {
        let mut branch = game.fork();
        {
            let selected_answer = branch.next_test_viewer_decision().select(Candidate::Flag {
                flag: if to_command {
                    Flag::CommandZone
                } else {
                    Flag::Destination
                },
            });
            input(&mut branch, selected_answer)
        };
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
            branch.next_test_viewer_decision().select(Candidate::Flag {
                flag: if to_command {
                    Flag::CommandZone
                } else {
                    Flag::Destination
                },
            }),
        );
        until(&mut branch, |g, req| {
            g.turn().step == TurnStep::PrecombatMain
                && matches!(
                    req,
                    Decision {
                        player: PlayerId(1),
                        reason: Reason::Priority,
                        ..
                    }
                )
        });
        reject(
            &branch,
            owner,
            request(&branch).1.id.clone(),
            branch
                .next_test_viewer_decision()
                .select(Candidate::Action {
                    action: Action::Cast {
                        card: old_target.clone(),
                    },
                }),
        );
        let card = observe(&branch, owner)
            .view
            .objects
            .into_iter()
            .find(|o| o.oracle_id == ISAMARU)
            .unwrap()
            .handle;
        {
            let selected_answer = branch
                .next_test_viewer_decision()
                .select(Candidate::Action {
                    action: Action::Cast { card },
                });
            input(&mut branch, selected_answer)
        };
        let Decision {
            shape: Shape::Pay { cost, .. },
            reason: Reason::Mana { .. },
            ..
        } = request(&branch).1
        else {
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
        {
            let selected_answer = branch.next_test_viewer_decision().activate_mana(land);
            input(&mut branch, selected_answer)
        };
        {
            let selected_answer = Answer::Pay {
                pay: PayAnswer::Finish {
                    undo_on_failure: true,
                },
            };
            input(&mut branch, selected_answer)
        };
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
    let old = tuck(&mut game, BEARS);
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
    until(&mut game, |g, req| {
        g.turn_owner() == PlayerId(2)
            && g.turn().step == TurnStep::Draw
            && matches!(
                req,
                Decision {
                    reason: Reason::Priority,
                    ..
                }
            )
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
    tuck(&mut game, ISAMARU);
    assert!(matches!(
        request(&game).1,
        Decision {
            reason: Reason::CommanderReturn {
                object: Some(_),
                destination: Zone::Library { owner: PlayerId(1) },
                ..
            },
            ..
        }
    ));
    for accepted in [false, true] {
        for to_command in [false, true] {
            let mut branch = game.fork();
            if accepted {
                {
                    let selected_answer =
                        branch.next_test_viewer_decision().select(Candidate::Flag {
                            flag: if to_command {
                                Flag::CommandZone
                            } else {
                                Flag::Destination
                            },
                        });
                    submit(&mut branch, selected_answer)
                };
            }
            branch.concede(PlayerId(0)).unwrap();
            assert!(matches!(settle(&mut branch), Advance::AwaitingDecision(_)));
            if !accepted {
                {
                    let selected_answer =
                        branch.next_test_viewer_decision().select(Candidate::Flag {
                            flag: if to_command {
                                Flag::CommandZone
                            } else {
                                Flag::Destination
                            },
                        });
                    input(&mut branch, selected_answer)
                };
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
    declined_tuck_to_draw(&mut game);
    for departing in [PlayerId(0), PlayerId(1), PlayerId(2)] {
        let mut stale = game.fork();
        let saved = receipt(
            &stale,
            stale.next_test_viewer_decision().select(Candidate::Flag {
                flag: Flag::CommandZone,
            }),
        );
        stale.concede(departing).unwrap();
        assert!(stale.submit(saved).is_err());
        let mut branch = game.fork();
        {
            let selected_answer = branch.next_test_viewer_decision().select(Candidate::Flag {
                flag: Flag::CommandZone,
            });
            submit(&mut branch, selected_answer)
        };
        branch.concede(departing).unwrap();
        assert!(matches!(settle(&mut branch), Advance::AwaitingDecision(_)));
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
    declined_tuck_to_draw(&mut game);
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
    declined_tuck_to_draw(&mut game);
    let (owner, req) = request(&game);
    let before = game.snapshot().unwrap();
    let QueryResult::Incomplete { work, .. } = game.check_player_response(
        owner,
        req.id.clone(),
        game.next_test_viewer_decision().select(Candidate::Flag {
            flag: Flag::Destination,
        }),
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
        settle(&mut restored),
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
        let response = match req.clone() {
            found @ Decision {
                reason: Reason::Attackers,
                ..
            } if (found.player) == PlayerId(1) => {
                let candidates = found.subjects();
                {
                    hits += 1;
                    req.attack(vec![(candidates[0].clone(), PlayerId(2))])
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
                verified_first_hit = true;
            }
        }
    }
    assert_eq!(hits, 2);
    assert!(verified_first_hit);

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
    assert!(matches!(settle(&mut game), Advance::AwaitingDecision(_)));
    for to_command in [false, true] {
        let mut branch = game.fork();
        {
            let selected_answer = branch.next_test_viewer_decision().select(Candidate::Flag {
                flag: if to_command {
                    Flag::CommandZone
                } else {
                    Flag::Destination
                },
            });
            input(&mut branch, selected_answer)
        };
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
