//! Independent player-facing stack-target and activated-cost regressions.
use mtg_contracts::{Catalog, Color, Quantity};
use mtg_core::*;
use std::collections::BTreeMap;
const ISAMARU: &str = "cfb756dd-6fe1-430a-8f04-8b10cac62a86";
const LOOTER: &str = "67362406-b1ca-49e2-800d-9050bfe8742a";
const AZURE: &str = "bbf9fd47-10e1-4749-b06f-4fe72200afc7";
const COUNTER: &str = "cc187110-1148-4090-bbb8-e205694a39f5";
const STIFLE: &str = "b3b00911-ece7-4484-bc36-f211ce72b6cc";
const UNSUMMON: &str = "837182db-1bf3-4a2c-bd01-1af9d9873561";
const ISLAND: &str = "b2c6aa39-2d2a-459c-a555-fb48ba993373";
const PLAINS: &str = "bc71ebf6-2056-41f7-be35-b2e5c34afa99";
const FOREST: &str = "b34bb2dc-c1af-4d77-b0b3-a0fb342a5fc6";
fn prepared(config: ScenarioConfig) -> Prepared {
    Prepared::admit(
        Catalog::read(include_str!(
            "../../../tests/fixtures/expected-stack-ir.json"
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
                && [ISLAND, PLAINS, FOREST].contains(&o.oracle_id.as_str())
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
fn begin_mage(game: &mut Game, p: &Prepared) {
    let object = handle(game, PlayerId(0), AZURE, Zone::Battlefield);
    input(
        game,
        p,
        PlayerResponse::ActivateAbility { object, ability: 0 },
    );
    assert!(matches!(request(game).1, PlayerRequest::Mana { .. }));
}
fn mage(game: &mut Game, p: &Prepared, amounts: &[(Color, i64)]) {
    begin_mage(game, p);
    fund_all(game, p);
    spend(game, p, amounts);
}
fn cast_stack(game: &mut Game, p: &Prepared, oracle: &str) {
    let actor = request(game).0;
    let card = handle(game, actor, oracle, Zone::Hand { owner: actor });
    input(game, p, PlayerResponse::Cast { card });
    assert!(matches!(
        request(game).1,
        PlayerRequest::StackTargets { .. }
    ));
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

#[test]
fn mana_activation_failure_reverses_only_ability_and_receipts_without_resurrecting_source() {
    let p = prepared(ScenarioConfig::stack_costs_borrowed());
    for owner_leaves in [false, true] {
        for undo_mana in [false, true] {
            let mut game = Game::new(p.clone());
            let original = game.object_id(CardId(8)).unwrap();
            begin_mage(&mut game, &p);
            activate_land(&mut game, &p, ISLAND);
            if owner_leaves {
                game.concede(PlayerId(2)).unwrap();
                assert!(matches!(
                    settle(&mut game, &p),
                    Advance::AwaitingDecision(_)
                ));
            }
            input(&mut game, &p, PlayerResponse::FinishMana { undo_mana });
            assert!(observe(&game, PlayerId(0)).view.stack.is_empty());
            assert_eq!(
                game.mana(PlayerId(0), Color::U).unwrap_or_default(),
                Quantity::from(if undo_mana { 0 } else { 1 })
            );
            if owner_leaves {
                assert!(game.object_id(CardId(8)).is_none());
            } else {
                assert_eq!(game.object_id(CardId(8)), Some(original));
                assert_eq!(game.zone(original), Some(Zone::Battlefield));
            }
            assert_eq!(
                observe(&game, PlayerId(0))
                    .view
                    .objects
                    .iter()
                    .filter(|o| o.controller == PlayerId(0) && o.tapped)
                    .count(),
                usize::from(!undo_mana)
            );
            replay_equal(&game, &p);
        }
    }
}

#[test]
fn announced_mana_only_ability_survives_source_owner_departure_during_mana_and_spending() {
    let p = prepared(ScenarioConfig::stack_costs_borrowed());
    for at_spending in [false, true] {
        for accepted in [false, true] {
            let mut game = Game::new(p.clone());
            begin_mage(&mut game, &p);
            if at_spending {
                fund_all(&mut game, &p);
                if accepted {
                    submit(
                        &mut game,
                        PlayerResponse::SpendMana {
                            spending: BTreeMap::from([(Color::U, Quantity::from(4))]),
                        },
                    );
                }
            } else if accepted {
                let object = handle(&game, PlayerId(0), ISLAND, Zone::Battlefield);
                submit(&mut game, PlayerResponse::ActivateMana { object });
            }
            game.concede(PlayerId(2)).unwrap();
            assert!(matches!(
                settle(&mut game, &p),
                Advance::AwaitingDecision(_)
            ));
            assert!(game.object_id(CardId(8)).is_none());
            if !at_spending {
                fund_all(&mut game, &p);
                spend(&mut game, &p, &[(Color::U, 4)]);
            } else if !accepted {
                spend(&mut game, &p, &[(Color::U, 4)]);
            }
            let view = observe(&game, PlayerId(0)).view;
            assert_eq!(view.stack.len(), 1);
            assert_eq!(view.stack[0].ability.as_ref().unwrap().source.current, None);
            round(&mut game, &p);
            assert_eq!(
                game.zone(game.object_id(CardId(23)).unwrap()),
                Some(Zone::Hand { owner: PlayerId(0) })
            );
            replay_equal(&game, &p);
        }
    }
}

#[test]
fn two_activations_share_source_but_stifle_removes_only_bound_stack_identity_without_refund() {
    let p = prepared(ScenarioConfig::stack_costs());
    let mut game = Game::new(p.clone());
    mage(&mut game, &p, &[(Color::U, 4)]);
    mage(
        &mut game,
        &p,
        &[(Color::U, 2), (Color::W, 1), (Color::G, 1)],
    );
    let view = observe(&game, PlayerId(1)).view;
    assert_eq!(view.stack.len(), 2);
    let first = view.stack[0].handle.clone().unwrap();
    let second = view.stack[1].handle.clone().unwrap();
    assert_ne!(first, second);
    input(&mut game, &p, PlayerResponse::Pass);
    cast_stack(&mut game, &p, STIFLE);
    input(
        &mut game,
        &p,
        PlayerResponse::StackTargets {
            entries: vec![first],
        },
    );
    fund_all(&mut game, &p);
    spend(&mut game, &p, &[(Color::U, 1)]);
    pass_four(&mut game, &p);
    let view = observe(&game, PlayerId(1)).view;
    assert_eq!(view.stack.len(), 1);
    assert_eq!(view.stack[0].handle, Some(second));
    assert_eq!(
        game.mana(PlayerId(0), Color::U).unwrap_or_default(),
        Quantity::zero()
    );
    assert!(
        game.events()
            .iter()
            .all(|e| !matches!(e, Event::AbilityCompleted { .. }))
    );
    assert_eq!(game.object_id(CardId(8)).unwrap().incarnation, 0);
    pass_four(&mut game, &p);
    assert_eq!(
        game.events()
            .iter()
            .filter(|e| matches!(e, Event::AbilityCompleted { .. }))
            .count(),
        1
    );
    assert_eq!(
        game.zone(game.object_id(CardId(23)).unwrap()),
        Some(Zone::Hand { owner: PlayerId(0) })
    );
    replay_equal(&game, &p);
}

#[test]
fn counterspell_rejects_ability_and_self_but_can_counter_casters_other_spell() {
    let p = prepared(ScenarioConfig::stack_costs());
    let mut game = Game::new(p.clone());
    mage(&mut game, &p, &[(Color::U, 4)]);
    input(&mut game, &p, PlayerResponse::Pass);
    let card = handle(
        &game,
        PlayerId(1),
        UNSUMMON,
        Zone::Hand { owner: PlayerId(1) },
    );
    input(&mut game, &p, PlayerResponse::Cast { card });
    let source = handle(&game, PlayerId(1), AZURE, Zone::Battlefield);
    input(
        &mut game,
        &p,
        PlayerResponse::Targets {
            objects: vec![source],
        },
    );
    fund_all(&mut game, &p);
    spend(&mut game, &p, &[(Color::U, 1)]);
    cast_stack(&mut game, &p, COUNTER);
    let (actor, req) = request(&game);
    let view = observe(&game, actor).view;
    for index in [0, 2] {
        reject(
            &game,
            actor,
            req.token().clone(),
            PlayerResponse::StackTargets {
                entries: vec![view.stack[index].handle.clone().unwrap()],
            },
        );
    }
    input(
        &mut game,
        &p,
        PlayerResponse::StackTargets {
            entries: vec![view.stack[1].handle.clone().unwrap()],
        },
    );
    input(
        &mut game,
        &p,
        PlayerResponse::FinishMana { undo_mana: true },
    );
    spend(&mut game, &p, &[(Color::U, 2)]);
    pass_four(&mut game, &p);
    assert_eq!(
        game.zone(game.object_id(CardId(13)).unwrap()),
        Some(Zone::Graveyard { owner: PlayerId(1) })
    );
    assert_eq!(
        game.zone(game.object_id(CardId(8)).unwrap()),
        Some(Zone::Battlefield)
    );
    assert_eq!(observe(&game, actor).view.stack.len(), 1);
    pass_four(&mut game, &p);
    replay_equal(&game, &p);
}

#[test]
fn commander_counter_grave_return_waits_for_counterspell_completion_and_preserves_cast_history() {
    let p = prepared(ScenarioConfig::stack_costs());
    let mut game = Game::new(p.clone());
    let card = handle(
        &game,
        PlayerId(0),
        ISAMARU,
        Zone::Command { owner: PlayerId(0) },
    );
    input(&mut game, &p, PlayerResponse::Cast { card });
    activate_land(&mut game, &p, PLAINS);
    input(
        &mut game,
        &p,
        PlayerResponse::FinishMana { undo_mana: true },
    );
    spend(&mut game, &p, &[(Color::W, 1)]);
    input(&mut game, &p, PlayerResponse::Pass);
    cast_stack(&mut game, &p, COUNTER);
    let target = observe(&game, PlayerId(1)).view.stack[0]
        .handle
        .clone()
        .unwrap();
    input(
        &mut game,
        &p,
        PlayerResponse::StackTargets {
            entries: vec![target],
        },
    );
    fund_all(&mut game, &p);
    spend(&mut game, &p, &[(Color::U, 2)]);
    for _ in 0..3 {
        input(&mut game, &p, PlayerResponse::Pass);
    }
    submit(&mut game, PlayerResponse::Pass);
    let mut witnessed_mid_resolution = false;
    for _ in 0..50 {
        let result = game.advance(Budget::new(1));
        verify(&game, &p);
        if game.zone(game.object_id(CardId(0)).unwrap())
            == Some(Zone::Graveyard { owner: PlayerId(0) })
            && game.zone(game.object_id(CardId(11)).unwrap()) == Some(Zone::Stack)
        {
            witnessed_mid_resolution = true;
            for viewer in game.live_players() {
                assert!(!matches!(
                    observe(&game, viewer).decision,
                    Some(PlayerRequest::CommanderReturn { .. })
                ));
            }
        }
        if matches!(result, Advance::AwaitingDecision(_)) {
            break;
        }
    }
    assert!(witnessed_mid_resolution);
    assert!(matches!(
        request(&game).1,
        PlayerRequest::CommanderReturn {
            destination: Zone::Graveyard { owner: PlayerId(0) },
            ..
        }
    ));
    assert_eq!(
        game.zone(game.object_id(CardId(11)).unwrap()),
        Some(Zone::Graveyard { owner: PlayerId(1) })
    );
    assert!(observe(&game, PlayerId(0)).view.stack.is_empty());
    assert_eq!(
        game.commander_ledger().tax(CardId(0), PlayerId(0)).unwrap(),
        Quantity::from(2)
    );
    for to_command in [false, true] {
        let mut branch = game.fork();
        input(
            &mut branch,
            &p,
            PlayerResponse::CommanderReturn { to_command },
        );
        assert_eq!(
            branch
                .commander_ledger()
                .tax(CardId(0), PlayerId(0))
                .unwrap(),
            Quantity::from(2)
        );
        replay_equal(&branch, &p);
    }
}

#[test]
fn looter_bounced_recast_is_sick_until_next_turn_and_cannot_activate_twice_tapped() {
    let p = prepared(ScenarioConfig::stack_costs());
    let mut game = Game::new(p.clone());
    input(&mut game, &p, PlayerResponse::Pass);
    let card = handle(
        &game,
        PlayerId(1),
        UNSUMMON,
        Zone::Hand { owner: PlayerId(1) },
    );
    input(&mut game, &p, PlayerResponse::Cast { card });
    let source = handle(&game, PlayerId(1), LOOTER, Zone::Battlefield);
    input(
        &mut game,
        &p,
        PlayerResponse::Targets {
            objects: vec![source],
        },
    );
    fund_all(&mut game, &p);
    spend(&mut game, &p, &[(Color::U, 1)]);
    pass_four(&mut game, &p);
    let card = handle(
        &game,
        PlayerId(0),
        LOOTER,
        Zone::Hand { owner: PlayerId(0) },
    );
    input(&mut game, &p, PlayerResponse::Cast { card });
    fund_all(&mut game, &p);
    spend(&mut game, &p, &[(Color::U, 2)]);
    pass_four(&mut game, &p);
    let source = handle(&game, PlayerId(0), LOOTER, Zone::Battlefield);
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
    passive_until(&mut game, &p, |g, r| {
        g.turn().number == 5
            && g.turn().step == TurnStep::PrecombatMain
            && matches!(
                r,
                PlayerRequest::Priority {
                    player: PlayerId(0),
                    ..
                }
            )
    });
    let source = handle(&game, PlayerId(0), LOOTER, Zone::Battlefield);
    input(
        &mut game,
        &p,
        PlayerResponse::ActivateAbility {
            object: source.clone(),
            ability: 0,
        },
    );
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
        observe(&game, PlayerId(0))
            .view
            .objects
            .iter()
            .any(|o| o.oracle_id == LOOTER && o.tapped)
    );
    replay_equal(&game, &p);
}

#[test]
fn reversed_activation_handle_does_not_rebind_to_same_source_and_stack_query_work_recovers() {
    let p = prepared(ScenarioConfig::stack_costs());
    let mut game = Game::new(p.clone());
    begin_mage(&mut game, &p);
    let stale = observe(&game, PlayerId(1)).view.stack[0]
        .handle
        .clone()
        .unwrap();
    input(
        &mut game,
        &p,
        PlayerResponse::FinishMana { undo_mana: true },
    );
    mage(&mut game, &p, &[(Color::U, 4)]);
    let current = observe(&game, PlayerId(1)).view.stack[0]
        .handle
        .clone()
        .unwrap();
    assert_ne!(stale, current);
    input(&mut game, &p, PlayerResponse::Pass);
    cast_stack(&mut game, &p, STIFLE);
    let (actor, req) = request(&game);
    reject(
        &game,
        actor,
        req.token().clone(),
        PlayerResponse::StackTargets {
            entries: vec![stale],
        },
    );
    let before = game.snapshot().unwrap();
    let QueryResult::Incomplete { work, .. } = game.check_player_response(
        actor,
        req.token().clone(),
        PlayerResponse::StackTargets {
            entries: vec![current],
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
    fund_all(&mut restored, &p);
    spend(&mut restored, &p, &[(Color::U, 1)]);
    pass_four(&mut restored, &p);
    assert!(observe(&restored, PlayerId(1)).view.stack.is_empty());
    assert_eq!(
        restored.zone(restored.object_id(CardId(23)).unwrap()),
        Some(Zone::Library { owner: PlayerId(0) })
    );
    replay_equal(&restored, &p);
}

#[test]
fn accepted_stack_target_departure_reverses_unfinished_counter_and_forged_bindings_reject() {
    let p = prepared(ScenarioConfig::stack_costs());
    let mut game = Game::new(p.clone());
    let card = handle(
        &game,
        PlayerId(0),
        ISAMARU,
        Zone::Command { owner: PlayerId(0) },
    );
    input(&mut game, &p, PlayerResponse::Cast { card });
    activate_land(&mut game, &p, PLAINS);
    input(
        &mut game,
        &p,
        PlayerResponse::FinishMana { undo_mana: true },
    );
    spend(&mut game, &p, &[(Color::W, 1)]);
    input(&mut game, &p, PlayerResponse::Pass);
    cast_stack(&mut game, &p, COUNTER);
    let target = observe(&game, PlayerId(1)).view.stack[0]
        .handle
        .clone()
        .unwrap();
    for accepted in [false, true] {
        let mut branch = game.fork();
        if accepted {
            submit(
                &mut branch,
                PlayerResponse::StackTargets {
                    entries: vec![target.clone()],
                },
            );
        }
        branch.concede(PlayerId(0)).unwrap();
        assert!(matches!(
            settle(&mut branch, &p),
            Advance::AwaitingDecision(_)
        ));
        assert!(branch.object_id(CardId(0)).is_none());
        assert_eq!(
            branch.zone(branch.object_id(CardId(11)).unwrap()),
            Some(Zone::Hand { owner: PlayerId(1) })
        );
        assert_eq!(branch.object_id(CardId(11)).unwrap().incarnation, 0);
        assert!(observe(&branch, PlayerId(1)).view.stack.is_empty());
        replay_equal(&branch, &p);
    }
    input(
        &mut game,
        &p,
        PlayerResponse::StackTargets {
            entries: vec![target],
        },
    );
    for mutation in 0..4 {
        let mut snapshot: serde_json::Value =
            serde_json::from_str(&game.snapshot().unwrap()).unwrap();
        match mutation {
            0 => snapshot["state"]["stack"][1]["stack_targets"] = serde_json::json!([1]),
            1 => snapshot["state"]["stack"][1]["stack_targets"] = serde_json::json!([999]),
            2 => {
                snapshot["state"]["stack"][1]["announced_stack_targets"][0][0] =
                    "forged-announcement".into()
            }
            _ => snapshot["state"]["frame"]["cast"]["origin"] = "activated".into(),
        }
        snapshot["state_sha256"] = mtg_contracts::digest(&snapshot["state"]).unwrap().into();
        assert!(Game::restore(p.clone(), &serde_json::to_string(&snapshot).unwrap()).is_err());
    }
}

#[test]
fn started_counter_invocation_survives_caster_departure_and_does_not_rebind_removed_target() {
    let p = prepared(ScenarioConfig::stack_costs());
    let mut game = Game::new(p.clone());
    let card = handle(
        &game,
        PlayerId(0),
        ISAMARU,
        Zone::Command { owner: PlayerId(0) },
    );
    input(&mut game, &p, PlayerResponse::Cast { card });
    activate_land(&mut game, &p, PLAINS);
    input(
        &mut game,
        &p,
        PlayerResponse::FinishMana { undo_mana: true },
    );
    spend(&mut game, &p, &[(Color::W, 1)]);
    input(&mut game, &p, PlayerResponse::Pass);
    cast_stack(&mut game, &p, COUNTER);
    let target = observe(&game, PlayerId(1)).view.stack[0]
        .handle
        .clone()
        .unwrap();
    input(
        &mut game,
        &p,
        PlayerResponse::StackTargets {
            entries: vec![target],
        },
    );
    fund_all(&mut game, &p);
    spend(&mut game, &p, &[(Color::U, 2)]);
    for _ in 0..3 {
        input(&mut game, &p, PlayerResponse::Pass);
    }
    submit(&mut game, PlayerResponse::Pass);
    let mut reached = false;
    for _ in 0..20 {
        assert!(matches!(
            game.advance(Budget::new(1)),
            Advance::Suspended { .. }
        ));
        verify(&game, &p);
        let snapshot: serde_json::Value = serde_json::from_str(&game.snapshot().unwrap()).unwrap();
        if snapshot["state"]["frame"]["stage"] == "resolve"
            && snapshot["state"]["frame"]["frame"]["cursor"] == 0
        {
            reached = true;
            break;
        }
    }
    assert!(reached);
    for departing in [PlayerId(0), PlayerId(1)] {
        let mut branch = game.fork();
        branch.concede(departing).unwrap();
        assert!(matches!(
            settle(&mut branch, &p),
            Advance::AwaitingDecision(_)
        ));
        if departing == PlayerId(1) {
            assert!(branch.object_id(CardId(11)).is_none());
            assert!(matches!(
                request(&branch).1,
                PlayerRequest::CommanderReturn {
                    destination: Zone::Graveyard { owner: PlayerId(0) },
                    ..
                }
            ));
            input(
                &mut branch,
                &p,
                PlayerResponse::CommanderReturn { to_command: true },
            );
        } else {
            assert!(branch.object_id(CardId(0)).is_none());
            assert_eq!(
                branch.zone(branch.object_id(CardId(11)).unwrap()),
                Some(Zone::Graveyard { owner: PlayerId(1) })
            );
            assert!(matches!(request(&branch).1, PlayerRequest::Priority { .. }));
        }
        assert!(observe(&branch, request(&branch).0).view.stack.is_empty());
        replay_equal(&branch, &p);
    }
}
