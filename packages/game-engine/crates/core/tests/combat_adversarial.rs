//! Independent black-box checks for the frozen G/I configurations. Decisions use
//! player projections; physical IDs are used only by trusted test assertions.
use mtg_contracts::{Catalog, Quantity};
use mtg_core::*;

fn prepared(config: ScenarioConfig) -> Prepared {
    Prepared::admit(
        Catalog::read(include_str!("../../../tests/fixtures/expected-ir.json")).unwrap(),
        config,
    )
    .unwrap()
}
fn recovery(game: &Game, prepared: &Prepared) {
    let snapshot = game.snapshot().unwrap();
    let restored = Game::restore(prepared.clone(), &snapshot).unwrap_or_else(|error| {
        let value: serde_json::Value = serde_json::from_str(&snapshot).unwrap();
        panic!(
            "restore error {error}: frame={} pending={} boundary={} external={}",
            value["state"]["frame"],
            value["state"]["pending"],
            value["state"]["rule_steps"],
            value["state"]["external_inputs"]
        );
    });
    assert_eq!(
        restored.state_digest().unwrap(),
        game.state_digest().unwrap()
    );
    assert_eq!(restored.trace(), game.trace());
}
fn complete(game: &Game, mut query: QueryResult) -> QueryValue {
    for _ in 0..2000 {
        match query {
            QueryResult::Complete(value) => return value,
            QueryResult::Incomplete { work, .. } => {
                query = game.resume_work(work, Budget::new(1));
            }
            QueryResult::Rejected(reason) => panic!("unexpected query rejection: {reason:?}"),
        }
    }
    panic!("query failed to finish");
}
fn observe(game: &Game, player: PlayerId) -> Observation {
    let QueryValue::Observation(view) = complete(game, game.observe(player, Budget::new(0))) else {
        panic!("wrong observation result")
    };
    view
}
fn request(game: &Game) -> (PlayerId, PlayerRequest) {
    for player in game.live_players() {
        let QueryValue::Decision(Some(request)) =
            complete(game, game.player_decision_for(player, Budget::new(0)))
        else {
            continue;
        };
        return (player, request);
    }
    panic!("missing player-facing request")
}
fn settle(game: &mut Game, prepared: &Prepared, units: u64, recover: bool) -> Advance {
    for _ in 0..100 {
        let result = game.advance(Budget::new(units));
        if recover {
            recovery(game, prepared);
        }
        match result {
            Advance::Suspended { .. } => {}
            Advance::Fault(ref reason) => panic!("unexpected engine fault: {reason}"),
            _ => return result,
        }
    }
    panic!("automatic work failed to settle")
}
fn submit(game: &mut Game, response: PlayerResponse) {
    let (player, req) = request(game);
    let QueryValue::Validated(receipt) = complete(
        game,
        game.check_player_response(player, req.token().clone(), response, Budget::new(0)),
    ) else {
        panic!("missing validated response")
    };
    game.submit(receipt).unwrap();
}
fn input(game: &mut Game, prepared: &Prepared, response: PlayerResponse) {
    submit(game, response);
    recovery(game, prepared);
    assert!(matches!(
        settle(game, prepared, 1, true),
        Advance::AwaitingDecision(_)
    ));
}
fn reject(game: &Game, actor: PlayerId, token: DecisionToken, response: PlayerResponse) {
    let before = game.snapshot().unwrap();
    let mut result = game.check_player_response(actor, token, response, Budget::new(0));
    for _ in 0..200 {
        match result {
            QueryResult::Rejected(_) => {
                assert_eq!(before, game.snapshot().unwrap());
                return;
            }
            QueryResult::Incomplete { work, .. } => {
                result = game.resume_work(work, Budget::new(1));
            }
            QueryResult::Complete(_) => panic!("illegal player response accepted"),
        }
    }
    panic!("illegal-response query failed to finish")
}
fn priority_to_attackers(game: &mut Game, prepared: &Prepared) -> Vec<ObjectHandle> {
    for _ in 0..30 {
        assert!(matches!(
            settle(game, prepared, 1, true),
            Advance::AwaitingDecision(_)
        ));
        match request(game).1 {
            PlayerRequest::Attackers { candidates, .. } => return candidates,
            PlayerRequest::Priority { .. } => input(game, prepared, PlayerResponse::Pass),
            other => panic!("unexpected request before combat: {other:?}"),
        }
    }
    panic!("attack declaration not reached")
}
fn handle(
    game: &Game,
    viewer: PlayerId,
    controller: PlayerId,
    zone: Zone,
    index: usize,
) -> ObjectHandle {
    observe(game, viewer)
        .view
        .objects
        .into_iter()
        .filter(|object| object.controller == controller && object.zone == zone)
        .nth(index)
        .expect("missing visible test object")
        .handle
}
fn multi_blocks(game: &mut Game, prepared: &Prepared) {
    let attackers = priority_to_attackers(game, prepared);
    assert_eq!(attackers.len(), 3);
    input(
        game,
        prepared,
        PlayerResponse::Attackers {
            attacks: vec![
                (attackers[0].clone(), PlayerId(1)),
                (attackers[1].clone(), PlayerId(1)),
                (attackers[2].clone(), PlayerId(2)),
            ],
        },
    );
    for _ in 0..12 {
        let (actor, req) = request(game);
        match req {
            PlayerRequest::Priority { .. } => input(game, prepared, PlayerResponse::Pass),
            PlayerRequest::Blockers {
                candidates,
                attackers,
                ..
            } => {
                assert_eq!(actor, PlayerId(1));
                assert_eq!(candidates.len(), 2);
                assert_eq!(attackers.len(), 2);
                input(
                    game,
                    prepared,
                    PlayerResponse::Blockers {
                        blocks: vec![
                            (candidates[0].clone(), attackers[0].clone()),
                            (candidates[1].clone(), attackers[0].clone()),
                        ],
                    },
                );
                assert_eq!(request(game).0, PlayerId(2));
                assert!(matches!(request(game).1, PlayerRequest::Blockers { .. }));
                input(game, prepared, PlayerResponse::Blockers { blocks: vec![] });
                return;
            }
            other => panic!("unexpected declaration request: {other:?}"),
        }
    }
    panic!("blockers not reached")
}
fn damage_request(game: &mut Game, prepared: &Prepared) -> (Quantity, Vec<ObjectHandle>) {
    for _ in 0..12 {
        match request(game).1 {
            PlayerRequest::Priority { .. } => input(game, prepared, PlayerResponse::Pass),
            PlayerRequest::Damage {
                power, recipients, ..
            } => return (power, recipients),
            other => panic!("unexpected damage request: {other:?}"),
        }
    }
    panic!("damage not reached")
}

#[test]
fn current_rules_allow_nonlethal_split_and_simultaneous_multiplayer_losses() {
    let prepared = prepared(ScenarioConfig::combat_interactions());
    let mut hashes = Vec::new();
    let mut verification = Vec::new();
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        for units in [1, 7, u64::MAX] {
            let mut game = Game::with_trace(prepared.clone(), mode);
            multi_blocks(&mut game, &prepared);
            let (power, recipients) = damage_request(&mut game, &prepared);
            assert_eq!(power, Quantity::from(2));
            assert_eq!(recipients.len(), 2);
            submit(
                &mut game,
                PlayerResponse::Damage {
                    assignments: vec![
                        (recipients[0].clone(), Quantity::from(1)),
                        (recipients[1].clone(), Quantity::from(1)),
                    ],
                },
            );
            let before = game.snapshot().unwrap();
            assert!(matches!(
                game.advance(Budget::new(0)),
                Advance::Suspended { .. }
            ));
            assert_eq!(before, game.snapshot().unwrap());
            recovery(&game, &prepared);
            assert!(matches!(
                settle(&mut game, &prepared, units, true),
                Advance::AwaitingDecision(_)
            ));
            assert_eq!(game.live_players(), vec![PlayerId(0), PlayerId(3)]);
            let view = observe(&game, PlayerId(0));
            assert_eq!(view.view.players[0].life, Quantity::from(6));
            assert_eq!(view.view.players[1].life, Quantity::zero());
            assert_eq!(view.view.players[2].life, Quantity::zero());
            assert_eq!(
                game.zone(game.object_id(CardId(0)).unwrap()),
                Some(Zone::Graveyard { owner: PlayerId(0) })
            );
            assert!(game.object_id(CardId(3)).is_none());
            assert!(game.object_id(CardId(4)).is_none());
            assert!(game.object_id(CardId(5)).is_none());
            assert_eq!(view.view.priority, Some(PlayerId(0)));
            if mode == TraceMode::Detailed {
                assert!(game.events().iter().any(|event| matches!(event,
                    Event::Committed { actual: EventOutcome::DamageDealt { results, .. }, .. }
                    if results.iter().any(|result| matches!(result.recipient, DamageRecipient::Creature { object } if object.card == CardId(0))
                        && result.previous == Quantity::zero() && result.current == Quantity::from(4))
                )), "both blockers' simultaneous damage must accumulate against common pre-state");
            }
            assert!(
                game.events()
                    .iter()
                    .all(|e| !matches!(e, Event::GameWon { .. }))
            );
            let replay = Game::replay(prepared.clone(), &game.replay_journal()).unwrap();
            assert_eq!(game.state_digest().unwrap(), replay.state_digest().unwrap());
            hashes.push(game.state_digest().unwrap());
            if let Some(evidence) = &game.trace().verification {
                verification.push(evidence.clone());
            }
        }
    }
    assert!(hashes.iter().all(|hash| hash == &hashes[0]));
    assert!(verification.iter().all(|value| value == &verification[0]));
}

#[test]
fn complete_declarations_reject_duplicates_foreign_defenders_and_stale_tokens() {
    let prepared = prepared(ScenarioConfig::combat_interactions());
    let mut game = Game::new(prepared.clone());
    let attackers = priority_to_attackers(&mut game, &prepared);
    let (actor, req) = request(&game);
    let token = req.token().clone();
    for attacks in [
        vec![(attackers[0].clone(), PlayerId(0))],
        vec![(attackers[0].clone(), PlayerId(4))],
        vec![
            (attackers[0].clone(), PlayerId(1)),
            (attackers[0].clone(), PlayerId(2)),
        ],
        vec![(ObjectHandle("unseen-object".into()), PlayerId(1))],
        vec![(
            handle(&game, PlayerId(0), PlayerId(1), Zone::Battlefield, 0),
            PlayerId(1),
        )],
    ] {
        reject(
            &game,
            actor,
            token.clone(),
            PlayerResponse::Attackers { attacks },
        );
    }
    reject(
        &game,
        PlayerId(1),
        token.clone(),
        PlayerResponse::Attackers { attacks: vec![] },
    );
    input(
        &mut game,
        &prepared,
        PlayerResponse::Attackers {
            attacks: vec![
                (attackers[0].clone(), PlayerId(1)),
                (attackers[1].clone(), PlayerId(2)),
            ],
        },
    );
    reject(
        &game,
        actor,
        token,
        PlayerResponse::Attackers { attacks: vec![] },
    );
    for _ in 0..8 {
        if matches!(request(&game).1, PlayerRequest::Blockers { .. }) {
            break;
        }
        input(&mut game, &prepared, PlayerResponse::Pass);
    }
    let (actor, req) = request(&game);
    let PlayerRequest::Blockers {
        candidates,
        attackers,
        token,
        ..
    } = req
    else {
        panic!()
    };
    assert_eq!(actor, PlayerId(1));
    assert_eq!(attackers.len(), 1, "defender-specific attack domain");
    let foreign_attacker = handle(&game, actor, PlayerId(0), Zone::Battlefield, 1);
    for blocks in [
        vec![(candidates[0].clone(), foreign_attacker)],
        vec![
            (candidates[0].clone(), attackers[0].clone()),
            (candidates[0].clone(), attackers[0].clone()),
        ],
        vec![(
            handle(&game, actor, PlayerId(2), Zone::Battlefield, 0),
            attackers[0].clone(),
        )],
    ] {
        reject(
            &game,
            actor,
            token.clone(),
            PlayerResponse::Blockers { blocks },
        );
    }
    reject(&game, actor, token, PlayerResponse::Pass);
    input(
        &mut game,
        &prepared,
        PlayerResponse::Blockers { blocks: vec![] },
    );
    assert_eq!(request(&game).0, PlayerId(2), "no intervening priority");
}

#[test]
fn damage_choice_validation_query_recovery_and_fork_isolation() {
    let prepared = prepared(ScenarioConfig::combat_interactions());
    let mut game = Game::new(prepared.clone());
    multi_blocks(&mut game, &prepared);
    let (_, recipients) = damage_request(&mut game, &prepared);
    let (actor, req) = request(&game);
    for opponent in [PlayerId(1), PlayerId(2), PlayerId(3)] {
        assert!(matches!(
            complete(&game, game.player_decision_for(opponent, Budget::new(1))),
            QueryValue::Decision(None)
        ));
    }
    reject(&game, actor, req.token().clone(), PlayerResponse::Pass);
    for assignments in [
        vec![],
        vec![(recipients[0].clone(), Quantity::from(1))],
        vec![(recipients[0].clone(), Quantity::from(3))],
        vec![
            (recipients[0].clone(), Quantity::from(-1)),
            (recipients[1].clone(), Quantity::from(3)),
        ],
        vec![
            (recipients[0].clone(), Quantity::from(1)),
            (recipients[0].clone(), Quantity::from(1)),
        ],
    ] {
        reject(
            &game,
            actor,
            req.token().clone(),
            PlayerResponse::Damage { assignments },
        );
    }
    let QueryResult::Incomplete { work, .. } = game.observe(PlayerId(3), Budget::new(0)) else {
        panic!()
    };
    let saved = game.save_work(&work).unwrap();
    let before = game.snapshot().unwrap();
    let restored = Game::restore(prepared.clone(), &before).unwrap();
    assert!(matches!(
        restored.resume_work(work, Budget::unlimited()),
        QueryResult::Rejected(QueryRejection::StaleOrForeignWork)
    ));
    let recovered = restored.restore_work(&saved).unwrap();
    let QueryValue::Observation(recovered_view) =
        complete(&restored, restored.resume_work(recovered, Budget::new(1)))
    else {
        panic!()
    };
    assert_eq!(recovered_view, observe(&game, PlayerId(3)));
    assert_eq!(before, game.snapshot().unwrap());
    let response = PlayerResponse::Damage {
        assignments: vec![
            (recipients[0].clone(), Quantity::from(2)),
            (recipients[1].clone(), Quantity::zero()),
        ],
    };
    let QueryValue::Validated(receipt) = complete(
        &game,
        game.check_player_response(actor, req.token().clone(), response.clone(), Budget::new(0)),
    ) else {
        panic!()
    };
    let mut branch = game.fork();
    assert!(branch.submit(receipt).is_err());
    submit(&mut branch, response);
    settle(&mut branch, &prepared, 1, true);
    assert_eq!(before, game.snapshot().unwrap());
    assert_eq!(branch.live_players(), vec![PlayerId(0), PlayerId(3)]);
    // The 2/0 split must destroy a blocker before its owner's departure removes it.
    assert!(branch.events().iter().any(|event| matches!(event,
        Event::Committed { actual: EventOutcome::StateBasedMoved { movement, reason: CreatureLossReason::LethalDamage }, .. }
        if movement.previous.card == CardId(3) && movement.destination == (Zone::Graveyard { owner: PlayerId(1) })
    )));
    let QueryResult::Incomplete { work, .. } = game.player_decision_for(actor, Budget::new(0))
    else {
        panic!()
    };
    submit(
        &mut game,
        PlayerResponse::Damage {
            assignments: vec![
                (recipients[0].clone(), Quantity::from(1)),
                (recipients[1].clone(), Quantity::from(1)),
            ],
        },
    );
    assert!(matches!(
        game.resume_work(work, Budget::unlimited()),
        QueryResult::Rejected(QueryRejection::StaleOrForeignWork)
    ));
}

fn single_attack(game: &mut Game, prepared: &Prepared) {
    let attackers = priority_to_attackers(game, prepared);
    input(
        game,
        prepared,
        PlayerResponse::Attackers {
            attacks: vec![(attackers[0].clone(), PlayerId(1))],
        },
    );
    for _ in 0..8 {
        if matches!(request(game).1, PlayerRequest::Blockers { .. }) {
            return;
        }
        input(game, prepared, PlayerResponse::Pass);
    }
    panic!("single defender never asked to block")
}
fn pass_to_step(game: &mut Game, prepared: &Prepared, step: TurnStep) {
    for _ in 0..60 {
        if game.turn().step == step {
            return;
        }
        match request(game).1 {
            PlayerRequest::Priority { .. } => input(game, prepared, PlayerResponse::Pass),
            PlayerRequest::Blockers { .. } => {
                input(game, prepared, PlayerResponse::Blockers { blocks: vec![] })
            }
            other => panic!("unexpected request advancing steps: {other:?}"),
        }
    }
    panic!("failed to reach {step:?}")
}

#[test]
fn bounced_last_blocker_stays_blocked_and_the_instant_has_real_response_windows() {
    let prepared = prepared(ScenarioConfig::combat_interactions());
    let mut game = Game::new(prepared.clone());
    single_attack(&mut game, &prepared);
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
    assert_eq!(observe(&game, PlayerId(0)).view.combat[0].blockers.len(), 1);
    for player in 0..3 {
        assert_eq!(request(&game).0, PlayerId(player));
        input(&mut game, &prepared, PlayerResponse::Pass);
    }
    assert_eq!(request(&game).0, PlayerId(3));
    let spell = handle(
        &game,
        PlayerId(3),
        PlayerId(3),
        Zone::Hand { owner: PlayerId(3) },
        0,
    );
    input(&mut game, &prepared, PlayerResponse::Cast { card: spell });
    assert!(matches!(request(&game).1, PlayerRequest::Targets { .. }));
    let target = handle(&game, PlayerId(3), PlayerId(1), Zone::Battlefield, 0);
    input(
        &mut game,
        &prepared,
        PlayerResponse::Targets {
            objects: vec![target.clone()],
        },
    );
    let island = handle(&game, PlayerId(3), PlayerId(3), Zone::Battlefield, 0);
    input(
        &mut game,
        &prepared,
        PlayerResponse::ActivateMana { object: island },
    );
    input(
        &mut game,
        &prepared,
        PlayerResponse::FinishMana { undo_mana: true },
    );
    for expected in [3, 0, 1, 2] {
        assert_eq!(request(&game).0, PlayerId(expected));
        input(&mut game, &prepared, PlayerResponse::Pass);
    }
    assert!(observe(&game, PlayerId(0)).view.stack.is_empty());
    let combat = observe(&game, PlayerId(0)).view.combat;
    assert_eq!(combat.len(), 1);
    assert!(combat[0].blocked);
    assert!(combat[0].blockers.is_empty());
    assert_eq!(
        game.zone(game.object_id(CardId(3)).unwrap()),
        Some(Zone::Hand { owner: PlayerId(1) })
    );
    let view = observe(&game, PlayerId(3));
    assert!(
        view.view
            .objects
            .iter()
            .all(|object| object.handle != target)
    );
    assert!(
        view.view
            .objects
            .iter()
            .all(|object| object.zone != (Zone::Hand { owner: PlayerId(1) }))
    );
    pass_to_step(&mut game, &prepared, TurnStep::CombatDamage);
    assert_eq!(
        observe(&game, PlayerId(0)).view.players[1].life,
        Quantity::from(2)
    );
    assert_eq!(game.live_players().len(), 4);
    let replay = Game::replay(prepared.clone(), &game.replay_journal()).unwrap();
    assert_eq!(replay.state_digest().unwrap(), game.state_digest().unwrap());
}

#[test]
fn departed_defender_does_not_remove_surviving_attacker_or_receive_damage() {
    let prepared = prepared(ScenarioConfig::combat_interactions());
    let mut game = Game::new(prepared.clone());
    single_attack(&mut game, &prepared);
    game.concede(PlayerId(1)).unwrap();
    assert!(matches!(
        game.observe(PlayerId(0), Budget::unlimited()),
        QueryResult::Rejected(QueryRejection::UnsupportedBoundary)
    ));
    assert!(matches!(
        settle(&mut game, &prepared, 1, true),
        Advance::AwaitingDecision(_)
    ));
    let combat = observe(&game, PlayerId(0)).view.combat;
    assert_eq!(
        combat.len(),
        1,
        "defender departure alone does not remove attacker"
    );
    assert_eq!(combat[0].defender, PlayerId(1));
    pass_to_step(&mut game, &prepared, TurnStep::CombatDamage);
    assert_eq!(
        observe(&game, PlayerId(0)).view.players[1].life,
        Quantity::from(2)
    );
    assert_eq!(
        game.live_players(),
        vec![PlayerId(0), PlayerId(2), PlayerId(3)]
    );
}

#[test]
fn empty_declaration_skips_steps_but_later_removed_attackers_do_not() {
    let prepared = prepared(ScenarioConfig::combat_interactions());
    let mut empty = Game::new(prepared.clone());
    priority_to_attackers(&mut empty, &prepared);
    input(
        &mut empty,
        &prepared,
        PlayerResponse::Attackers { attacks: vec![] },
    );
    for _ in 0..4 {
        input(&mut empty, &prepared, PlayerResponse::Pass);
    }
    assert_eq!(empty.turn().step, TurnStep::EndCombat);

    let mut departed = Game::new(prepared.clone());
    let attackers = priority_to_attackers(&mut departed, &prepared);
    input(
        &mut departed,
        &prepared,
        PlayerResponse::Attackers {
            attacks: vec![(attackers[0].clone(), PlayerId(1))],
        },
    );
    departed.concede(PlayerId(0)).unwrap();
    settle(&mut departed, &prepared, 1, true);
    assert_eq!(departed.active_player(), None);
    assert!(observe(&departed, PlayerId(1)).view.combat.is_empty());
    pass_to_step(&mut departed, &prepared, TurnStep::DeclareBlockers);
    assert_eq!(departed.turn().number, 1);
    pass_to_step(&mut departed, &prepared, TurnStep::CombatDamage);
    assert_eq!(departed.turn().number, 1);
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
        other => panic!("passive policy cannot handle {other:?}"),
    }
}

#[test]
fn ordered_draws_and_cleanup_discard_preserve_hidden_information() {
    let prepared = prepared(ScenarioConfig::combat_game());
    let mut game = Game::new(prepared.clone());
    let mut discarded = false;
    for _ in 0..350 {
        assert!(matches!(
            settle(&mut game, &prepared, 1, false),
            Advance::AwaitingDecision(_)
        ));
        let (actor, req) = request(&game);
        for viewer in game.live_players() {
            let observation = observe(&game, viewer);
            assert!(
                observation
                    .view
                    .objects
                    .iter()
                    .all(|object| !matches!(object.zone, Zone::Library { .. }))
            );
            assert!(
                observation
                    .view
                    .objects
                    .iter()
                    .all(|object| !matches!(object.zone, Zone::Hand { owner } if owner != viewer))
            );
        }
        if let PlayerRequest::Discard {
            count,
            ref candidates,
            ..
        } = req
        {
            assert_eq!(actor, PlayerId(3));
            assert_eq!(count, 2, "unused Unsummon plus seven Islands plus draw");
            assert_eq!(candidates.len(), 9);
            for objects in [
                vec![],
                vec![candidates[0].clone()],
                vec![candidates[0].clone(), candidates[0].clone()],
            ] {
                reject(
                    &game,
                    actor,
                    req.token().clone(),
                    PlayerResponse::Discard { objects },
                );
            }
            let foreign = handle(&game, actor, PlayerId(0), Zone::Battlefield, 0);
            reject(
                &game,
                actor,
                req.token().clone(),
                PlayerResponse::Discard {
                    objects: vec![candidates[0].clone(), foreign],
                },
            );
            recovery(&game, &prepared);
            discarded = true;
        }
        if game.turn().number == 5 && game.turn().step == TurnStep::PrecombatMain {
            assert!(discarded);
            assert_eq!(
                game.zone(game.object_id(CardId(6)).unwrap()),
                Some(Zone::Hand { owner: PlayerId(0) })
            );
            assert_eq!(
                game.zone(game.object_id(CardId(4)).unwrap()),
                Some(Zone::Library { owner: PlayerId(0) })
            );
            assert_eq!(
                game.zone(game.object_id(CardId(16)).unwrap()),
                Some(Zone::Hand { owner: PlayerId(2) })
            );
            assert_eq!(
                game.zone(game.object_id(CardId(15)).unwrap()),
                Some(Zone::Library { owner: PlayerId(2) })
            );
            assert_eq!(observe(&game, PlayerId(3)).view.players[3].hand_count, 7);
            recovery(&game, &prepared);
            return;
        }
        submit(&mut game, passive(req));
    }
    panic!("passive policy failed to reach next P0 main phase")
}

#[test]
fn nonlethal_marked_damage_does_not_reduce_toughness_and_clears_at_cleanup() {
    let prepared = prepared(ScenarioConfig::combat_interactions());
    let mut game = Game::new(prepared.clone());
    single_attack(&mut game, &prepared);
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
            blocks: candidates
                .into_iter()
                .map(|blocker| (blocker, attackers[0].clone()))
                .collect(),
        },
    );
    let (_, recipients) = damage_request(&mut game, &prepared);
    input(
        &mut game,
        &prepared,
        PlayerResponse::Damage {
            assignments: recipients
                .into_iter()
                .map(|recipient| (recipient, Quantity::from(1)))
                .collect(),
        },
    );
    assert_eq!(game.live_players().len(), 4);
    for card in [3, 4] {
        assert_eq!(
            game.characteristics(game.object_id(CardId(card)).unwrap())
                .unwrap()
                .toughness,
            Some(Quantity::from(2))
        );
    }
    let blockers: Vec<_> = observe(&game, PlayerId(1))
        .view
        .objects
        .into_iter()
        .filter(|object| object.controller == PlayerId(1) && object.zone == Zone::Battlefield)
        .collect();
    assert_eq!(blockers.len(), 2);
    assert!(
        blockers
            .iter()
            .all(|object| object.marked_damage == Quantity::from(1) && !object.tapped)
    );
    for _ in 0..50 {
        if game.turn().number == 2 {
            assert!(
                observe(&game, PlayerId(1))
                    .view
                    .objects
                    .iter()
                    .all(|object| object.marked_damage == Quantity::zero())
            );
            return;
        }
        let response = passive(request(&game).1);
        submit(&mut game, response);
        settle(&mut game, &prepared, 1, true);
    }
    panic!("cleanup did not complete")
}

fn interaction_policy(req: PlayerRequest) -> PlayerResponse {
    match req {
        PlayerRequest::Attackers { candidates, .. } => PlayerResponse::Attackers {
            attacks: candidates
                .into_iter()
                .enumerate()
                .map(|(index, object)| (object, PlayerId(if index < 2 { 1 } else { 2 })))
                .collect(),
        },
        PlayerRequest::Blockers {
            player: PlayerId(1),
            candidates,
            attackers,
            ..
        } => PlayerResponse::Blockers {
            blocks: candidates
                .into_iter()
                .map(|blocker| (blocker, attackers[0].clone()))
                .collect(),
        },
        PlayerRequest::Damage { recipients, .. } => PlayerResponse::Damage {
            assignments: recipients
                .into_iter()
                .map(|recipient| (recipient, Quantity::from(1)))
                .collect(),
        },
        other => passive(other),
    }
}

#[test]
fn concessions_at_every_interaction_boundary_preserve_recovery_and_budget_equivalence() {
    let prepared = prepared(ScenarioConfig::combat_interactions());
    let mut witness = Game::new(prepared.clone());
    let mut boundaries = Vec::new();
    for _ in 0..140 {
        boundaries.push(witness.snapshot().unwrap());
        match witness.advance(Budget::new(1)) {
            Advance::Suspended { .. } => {}
            Advance::AwaitingDecision(_) => {
                if witness.turn().step == TurnStep::CombatDamage
                    && witness.live_players().len() == 2
                {
                    boundaries.push(witness.snapshot().unwrap());
                    break;
                }
                let response = interaction_policy(request(&witness).1);
                submit(&mut witness, response);
            }
            other => panic!("interaction witness stopped: {other:?}"),
        }
    }
    assert_eq!(witness.live_players(), vec![PlayerId(0), PlayerId(3)]);
    assert!(boundaries.len() > 30);
    let boundary_count = boundaries.len();
    let mut departure_cases = 0;
    for snapshot in boundaries {
        let base = Game::restore(prepared.clone(), &snapshot).unwrap();
        for player in base.live_players() {
            departure_cases += 1;
            let mut small = base.fork();
            let mut large = base.fork();
            small.concede(player).unwrap();
            large.concede(player).unwrap();
            recovery(&small, &prepared);
            let small_result = settle(&mut small, &prepared, 1, true);
            let large_result = settle(&mut large, &prepared, u64::MAX, false);
            assert_eq!(small_result, large_result);
            assert_eq!(small.state_digest().unwrap(), large.state_digest().unwrap());
            assert_eq!(small.trace().verification, large.trace().verification);
            assert!(!small.live_players().contains(&player));
            match small_result {
                Advance::AwaitingDecision(_) => {
                    let (actor, req) = request(&small);
                    assert!(small.live_players().contains(&actor));
                    // Every surviving decision has at least one legal complete response.
                    let response = match req {
                        PlayerRequest::Damage {
                            recipients, power, ..
                        } => PlayerResponse::Damage {
                            assignments: recipients
                                .into_iter()
                                .enumerate()
                                .map(|(index, recipient)| {
                                    (
                                        recipient,
                                        if index == 0 {
                                            power.clone()
                                        } else {
                                            Quantity::zero()
                                        },
                                    )
                                })
                                .collect(),
                        },
                        other => passive(other),
                    };
                    submit(&mut small, response);
                    recovery(&small, &prepared);
                }
                Advance::Finished { winner } => assert_eq!(small.live_players(), vec![winner]),
                other => panic!("concession produced unexpected outcome: {other:?}"),
            }
        }
    }
    eprintln!(
        "independent I boundary sweep: {boundary_count} saved boundaries, {departure_cases} concession branches"
    );
}

#[test]
fn resolved_creature_cannot_attack_until_its_controller_begins_another_turn() {
    let prepared = prepared(ScenarioConfig::combat_game());
    let mut game = Game::new(prepared.clone());
    settle(&mut game, &prepared, 1, true);
    let land = handle(
        &game,
        PlayerId(0),
        PlayerId(0),
        Zone::Hand { owner: PlayerId(0) },
        0,
    );
    input(
        &mut game,
        &prepared,
        PlayerResponse::PlayLand { object: land },
    );
    let creature = handle(
        &game,
        PlayerId(0),
        PlayerId(0),
        Zone::Hand { owner: PlayerId(0) },
        0,
    );
    input(
        &mut game,
        &prepared,
        PlayerResponse::Cast { card: creature },
    );
    for index in [0, 2] {
        let land = handle(&game, PlayerId(0), PlayerId(0), Zone::Battlefield, index);
        input(
            &mut game,
            &prepared,
            PlayerResponse::ActivateMana { object: land },
        );
    }
    input(
        &mut game,
        &prepared,
        PlayerResponse::FinishMana { undo_mana: true },
    );
    for _ in 0..4 {
        input(&mut game, &prepared, PlayerResponse::Pass);
    }
    let candidates = priority_to_attackers(&mut game, &prepared);
    assert_eq!(candidates.len(), 1);
    let (actor, req) = request(&game);
    let newer = handle(&game, actor, actor, Zone::Battlefield, 3);
    assert_ne!(candidates[0], newer);
    reject(
        &game,
        actor,
        req.token().clone(),
        PlayerResponse::Attackers {
            attacks: vec![(newer.clone(), PlayerId(1))],
        },
    );
    for _ in 0..350 {
        if game.turn().number == 5 {
            let view = observe(&game, actor);
            let newer = view
                .view
                .objects
                .iter()
                .find(|object| object.handle == newer)
                .unwrap();
            assert!(newer.attack_ready);
            assert!(!newer.tapped);
            assert_eq!(game.turn().lands_played, 0);
            recovery(&game, &prepared);
            return;
        }
        let response = passive(request(&game).1);
        submit(&mut game, response);
        assert!(matches!(
            settle(&mut game, &prepared, 1, false),
            Advance::AwaitingDecision(_)
        ));
    }
    panic!("next controller turn did not start")
}
