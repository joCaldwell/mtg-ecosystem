//! Independent black-box evidence for Commander casting/return continuations.
//! Actions use player-facing handles; physical IDs are trusted assertions only.
use crate::support::visible_controller;
use crate::support::{
    action_scenario, complete, input, observe, reject, replay_equal, request, settle, submit,
};
use mtg_contracts::{Color, Quantity};
use mtg_core::*;

fn visible(game: &Game, player: PlayerId, owner: PlayerId, zone: Zone) -> ObjectHandle {
    observe(game, player)
        .view
        .objects
        .into_iter()
        .find(|object| object.owner == owner && object.zone == zone)
        .expect("missing visible object")
        .handle
}
fn activate(game: &mut Game, player: PlayerId) {
    let land = observe(game, player)
        .view
        .objects
        .into_iter()
        .find(|object| {
            visible_controller(object) == player
                && object.zone == Zone::Battlefield
                && !object.tapped
                && object.oracle_id != "cfb756dd-6fe1-430a-8f04-8b10cac62a86"
        })
        .expect("missing untapped land")
        .handle;
    input(game, game.next_test_viewer_decision().activate_mana(land));
}
fn finish_mana(game: &mut Game, undo_mana: bool) {
    input(
        game,
        Answer::Pay {
            pay: PayAnswer::Finish {
                undo_on_failure: undo_mana,
            },
        },
    );
}
fn cast_first(game: &mut Game) {
    let card = visible(
        game,
        PlayerId(0),
        PlayerId(0),
        Zone::Command { owner: PlayerId(0) },
    );
    input(
        game,
        game.next_test_viewer_decision().select(Candidate::Action {
            action: Action::Cast { card },
        }),
    );
    let Decision {
        shape: Shape::Pay { cost, .. },
        reason: Reason::Mana { .. },
        ..
    } = request(game).1
    else {
        panic!()
    };
    assert_eq!(cost.generic, Quantity::zero());
    assert_eq!(cost.colored, vec![Color::W]);
    activate(game, PlayerId(0));
    finish_mana(game, true);
    for _ in 0..4 {
        input(
            game,
            Answer::Pick {
                picks: vec![vec![0]],
            },
        );
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
fn bounce_to_choice(game: &mut Game) {
    assert_eq!(request(game).0, PlayerId(0));
    input(
        game,
        Answer::Pick {
            picks: vec![vec![0]],
        },
    );
    assert_eq!(request(game).0, PlayerId(1));
    activate(game, PlayerId(1));
    let card = visible(
        game,
        PlayerId(1),
        PlayerId(1),
        Zone::Hand { owner: PlayerId(1) },
    );
    input(
        game,
        game.next_test_viewer_decision().select(Candidate::Action {
            action: Action::Cast { card },
        }),
    );
    let found @ Decision {
        reason: Reason::Targets { .. },
        ..
    } = request(game).1
    else {
        panic!()
    };
    let candidates = found.objects_offered();
    assert_eq!(candidates.len(), 1);
    input(game, game.next_test_viewer_decision().targets(candidates));
    finish_mana(game, true);
    for _ in 0..4 {
        input(
            game,
            Answer::Pick {
                picks: vec![vec![0]],
            },
        );
    }
    assert!(matches!(
        request(game),
        (
            PlayerId(0),
            Decision {
                reason: Reason::CommanderReturn {
                    destination: Zone::Hand { owner: PlayerId(0) },
                    ..
                },
                ..
            }
        )
    ));
}
fn pass_until(game: &mut Game, wanted: fn(&Decision<Viewer>) -> bool) {
    for _ in 0..40 {
        let req = request(game).1;
        if wanted(&req) {
            return;
        }
        assert!(matches!(
            req,
            Decision {
                reason: Reason::Priority,
                ..
            }
        ));
        input(
            game,
            Answer::Pick {
                picks: vec![vec![0]],
            },
        );
    }
    panic!("expected request not reached")
}

#[test]
fn command_origin_failure_restores_identity_and_tax_with_keep_or_undo() {
    let prepared = action_scenario(ScenarioConfig::command_casting());
    let mut game = Game::new(prepared.clone());
    let original = game.object_id(CardId(0)).unwrap();
    let card = visible(
        &game,
        PlayerId(0),
        PlayerId(0),
        Zone::Command { owner: PlayerId(0) },
    );
    {
        let selected_answer = game.next_test_viewer_decision().select(Candidate::Action {
            action: Action::Cast { card },
        });
        input(&mut game, selected_answer)
    };
    finish_mana(&mut game, false);
    assert_eq!(game.object_id(CardId(0)), Some(original));
    assert_eq!(
        game.zone(original),
        Some(Zone::Command { owner: PlayerId(0) })
    );
    assert_eq!(
        game.commander_ledger().tax(CardId(0), PlayerId(0)).unwrap(),
        Quantity::zero()
    );
    cast_first(&mut game);
    bounce_to_choice(&mut game);
    {
        let selected_answer = game.next_test_viewer_decision().select(Candidate::Flag {
            flag: Flag::CommandZone,
        });
        input(&mut game, selected_answer)
    };
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
        assert_eq!(cost.generic, Quantity::from(2));
        activate(&mut branch, PlayerId(0));
        finish_mana(&mut branch, undo_mana);
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
    let prepared = action_scenario(ScenarioConfig::command_casting());
    let mut game = Game::new(prepared.clone());
    cast_first(&mut game);
    bounce_to_choice(&mut game);
    let (owner, req) = request(&game);
    for other in [PlayerId(1), PlayerId(2), PlayerId(3)] {
        reject(
            &game,
            other,
            req.id.clone(),
            game.next_test_viewer_decision().select(Candidate::Flag {
                flag: Flag::CommandZone,
            }),
        );
    }
    reject(
        &game,
        owner,
        req.id.clone(),
        Answer::Pick {
            picks: vec![vec![u32::MAX]],
        },
    );
    assert_eq!(
        game.zone(game.object_id(CardId(0)).unwrap()),
        Some(Zone::Battlefield)
    );
    for to_command in [false, true] {
        let mut branch = game.fork();
        branch.concede(PlayerId(1)).unwrap();
        assert!(matches!(settle(&mut branch), Advance::AwaitingDecision(_)));
        assert!(matches!(
            request(&branch),
            (
                PlayerId(0),
                Decision {
                    reason: Reason::CommanderReturn { .. },
                    ..
                }
            )
        ));
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
            req.id.clone(),
            branch.next_test_viewer_decision().select(Candidate::Flag {
                flag: if to_command {
                    Flag::CommandZone
                } else {
                    Flag::Destination
                },
            }),
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
        let prepared = action_scenario(config.clone());
        let mut game = Game::new(prepared.clone());
        if config == ScenarioConfig::command_casting() {
            cast_first(&mut game);
            bounce_to_choice(&mut game);
        } else {
            game.concede(PlayerId(1)).unwrap();
            assert!(matches!(settle(&mut game), Advance::AwaitingDecision(_)));
        }
        let (owner, req) = request(&game);
        assert_eq!(owner, PlayerId(0));
        assert!(matches!(
            req,
            Decision {
                reason: Reason::CommanderReturn { .. },
                ..
            }
        ));
        let QueryValue::Validated(receipt) = complete(
            &game,
            game.check_player_response(
                owner,
                req.id.clone(),
                game.next_test_viewer_decision().select(Candidate::Flag {
                    flag: Flag::CommandZone,
                }),
                Budget::new(0),
            ),
        ) else {
            panic!()
        };
        game.concede(owner).unwrap();
        assert!(game.submit(receipt).is_err());
        assert!(matches!(settle(&mut game), Advance::AwaitingDecision(_)));
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
    let prepared = action_scenario(ScenarioConfig::commander_exile());
    let mut game = Game::new(prepared.clone());
    game.concede(PlayerId(1)).unwrap();
    assert!(matches!(settle(&mut game), Advance::AwaitingDecision(_)));
    assert_eq!(
        game.zone(game.object_id(CardId(0)).unwrap()),
        Some(Zone::Exile)
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
            assert!(matches!(
                request(&branch).1,
                Decision {
                    reason: Reason::Priority,
                    ..
                }
            ));
            {
                let selected_answer = Answer::Pick {
                    picks: vec![vec![0]],
                };
                input(&mut branch, selected_answer)
            };
        }
        assert!(!matches!(
            request(&branch).1,
            Decision {
                reason: Reason::CommanderReturn { .. },
                ..
            }
        ));
        replay_equal(&branch, &prepared);
    }
}

#[test]
fn accepted_return_input_survives_source_departure_but_not_owner_departure() {
    let prepared = action_scenario(ScenarioConfig::command_casting());
    let mut game = Game::new(prepared.clone());
    cast_first(&mut game);
    bounce_to_choice(&mut game);
    for departing in [PlayerId(0), PlayerId(1), PlayerId(2)] {
        let mut branch = game.fork();
        {
            let selected_answer = branch.next_test_viewer_decision().select(Candidate::Flag {
                flag: Flag::CommandZone,
            });
            submit(&mut branch, selected_answer)
        };
        branch.concede(departing).unwrap();
        assert!(matches!(settle(&mut branch), Advance::AwaitingDecision(_)));
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
    let prepared = action_scenario(ScenarioConfig::commander_lifecycle());
    let mut game = Game::new(prepared.clone());
    pass_until(&mut game, |r| {
        matches!(
            r,
            Decision {
                reason: Reason::Attackers,
                ..
            }
        )
    });
    let found @ Decision {
        reason: Reason::Attackers,
        ..
    } = request(&game).1
    else {
        panic!()
    };
    let candidates = found.subjects();
    assert_eq!(candidates.len(), 1);
    {
        let selected_answer = game
            .next_test_viewer_decision()
            .attack(vec![(candidates[0].clone(), PlayerId(1))]);
        input(&mut game, selected_answer)
    };
    pass_until(&mut game, |r| {
        matches!(
            r,
            Decision {
                reason: Reason::Blockers,
                ..
            }
        )
    });
    let found @ Decision {
        reason: Reason::Blockers,
        ..
    } = request(&game).1
    else {
        panic!()
    };
    let candidates = found.subjects();
    let attackers = found.allowed_objects();
    {
        let selected_answer = game
            .next_test_viewer_decision()
            .block(vec![(candidates[0].clone(), attackers[0].clone())]);
        input(&mut game, selected_answer)
    };
    pass_until(&mut game, |r| {
        matches!(
            r,
            Decision {
                reason: Reason::CommanderReturn { .. },
                ..
            }
        )
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
        assert_eq!(
            branch.object_id(CardId(0)).unwrap().incarnation,
            if to_command { 2 } else { 1 }
        );
        activate(&mut branch, PlayerId(0));
        assert!(matches!(
            request(&branch).1,
            Decision {
                reason: Reason::Priority,
                ..
            }
        ));
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
    let prepared = action_scenario(ScenarioConfig::commander_lifecycle());
    let mut game = Game::new(prepared.clone());
    pass_until(&mut game, |r| {
        matches!(
            r,
            Decision {
                reason: Reason::Attackers,
                ..
            }
        )
    });
    let found @ Decision {
        reason: Reason::Attackers,
        ..
    } = request(&game).1
    else {
        panic!()
    };
    let candidates = found.subjects();
    {
        let selected_answer = game
            .next_test_viewer_decision()
            .attack(vec![(candidates[0].clone(), PlayerId(1))]);
        input(&mut game, selected_answer)
    };
    pass_until(&mut game, |r| {
        matches!(
            r,
            Decision {
                reason: Reason::Blockers,
                ..
            }
        )
    });
    {
        let selected_answer = game.next_test_viewer_decision().block(vec![]);
        input(&mut game, selected_answer)
    };
    for _ in 0..4 {
        {
            let selected_answer = Answer::Pick {
                picks: vec![vec![0]],
            };
            input(&mut game, selected_answer)
        };
    }
    assert_eq!(
        observe(&game, PlayerId(0)).view.players[1].life,
        Quantity::from(38)
    );
    let damage = &game
        .commander_ledger()
        .history(CardId(0))
        .unwrap()
        .combat_damage;
    assert_eq!(damage.get(&PlayerId(1)), Some(&Quantity::from(2)));
    assert!(game.events().iter().any(|e| matches!(
        e,
        Event::Committed {
            actual: EventOutcome::DamageCommitted { .. },
            ..
        }
    )));
    let replay = Game::replay(prepared.clone(), &game.replay_journal()).unwrap();
    assert_eq!(replay.state_digest().unwrap(), game.state_digest().unwrap());
}

#[test]
fn nested_hand_resolution_cost_source_and_cursor_forgery_fail_independent_restore() {
    let prepared = action_scenario(ScenarioConfig::command_casting());
    let mut game = Game::new(prepared.clone());
    cast_first(&mut game);
    bounce_to_choice(&mut game);
    for mutation in 0..5 {
        let mut snapshot: serde_json::Value =
            serde_json::from_str(&game.snapshot().unwrap()).unwrap();
        // The frame names its invocation; the resolving stack entry is the
        // single owner of the invocation's cost and source.
        let stack_id = snapshot["state"]["frame"]["continuation"]["frame"]["stack_id"].clone();
        let owner = snapshot["state"]["stack"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|entry| entry["id"] == stack_id)
            .expect("resolving owner on the stack");
        match mutation {
            0 => {
                owner["total_cost"]["generic"] = "5".into();
                owner["paid"]["generic"] = "5".into();
            }
            1 => {
                owner["cast_from"] = serde_json::json!({"zone":"command","owner":1});
            }
            2 => {
                owner["source"]["incarnation"] = 999.into();
            }
            3 => {
                snapshot["state"]["frame"]["continuation"]["frame"]["legal_targets"][0] =
                    false.into();
            }
            _ => {
                snapshot["state"]["frame"]["continuation"]["frame"]["cursor"] = 1.into();
            }
        }
        snapshot["state_sha256"] = mtg_contracts::digest(&snapshot["state"]).unwrap().into();
        assert!(
            Game::restore(prepared.clone(), &serde_json::to_string(&snapshot).unwrap()).is_err()
        );
    }
}
