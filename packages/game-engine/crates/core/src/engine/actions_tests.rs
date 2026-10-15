use super::*;

fn prepared(config: ScenarioConfig) -> Prepared {
    Prepared::admit(
        Catalog::read(include_str!(
            "../../../../tests/fixtures/expected-action-ir.json"
        ))
        .unwrap(),
        config,
    )
    .unwrap()
}
fn settle(g: &mut Game) -> Request {
    let Advance::AwaitingDecision(request) = g.advance(Budget::unlimited()) else {
        panic!("expected request")
    };
    request
}
fn answer(g: &mut Game, response: Response) {
    let request = settle(g);
    g.respond(request.player(), request.decision_id(), response)
        .unwrap();
}
fn recover(g: &Game) {
    let restored = Game::restore(g.prepared.clone(), &g.snapshot().unwrap()).unwrap();
    assert_eq!(restored.state_digest().unwrap(), g.state_digest().unwrap());
    assert_eq!(
        digest(restored.trace()).unwrap(),
        digest(g.trace()).unwrap()
    );
}
fn view(g: &Game, player: u8) -> Observation {
    let QueryResult::Complete(QueryValue::Observation(view)) =
        g.observe(PlayerId(player), Budget::unlimited())
    else {
        panic!("observation")
    };
    view
}
fn activate_land(g: &mut Game, card: u32) {
    answer(
        g,
        Response::ActivateMana {
            object: g.object_id(CardId(card)).unwrap(),
        },
    );
}
fn cast_edict(g: &mut Game, target: u8) {
    answer(g, Response::Cast { card: CardId(4) });
    assert!(matches!(
        settle(g),
        Request::PlayerTargets {
            player: PlayerId(0),
            ..
        }
    ));
    answer(
        g,
        Response::PlayerTargets {
            players: vec![PlayerId(target)],
        },
    );
    activate_land(g, 2);
    activate_land(g, 3);
    answer(g, Response::FinishMana { undo_mana: true });
    answer(
        g,
        Response::SpendMana {
            spending: BTreeMap::from([
                (Color::B, Quantity::from(1)),
                (Color::G, Quantity::from(1)),
            ]),
        },
    );
    settle(g);
}
fn pass_round(g: &mut Game) {
    for _ in 0..g.live_players().len() {
        answer(g, Response::Pass);
    }
    settle(g);
}
fn ability(g: &mut Game) {
    answer(g, Response::Pass);
    let source = g.object_id(CardId(5)).unwrap();
    answer(
        g,
        Response::ActivateAbility {
            object: source,
            ability: 0,
        },
    );
    settle(g);
}

#[test]
fn opponent_announcement_and_resolution_sacrifice_have_distinct_choosers() {
    let mut g = Game::new(prepared(ScenarioConfig::actions()));
    cast_edict(&mut g, 1);
    recover(&g);
    assert_eq!(g.top().unwrap().player_targets, vec![PlayerId(1)]);
    assert!(g.top().unwrap().targets.is_empty());
    assert_eq!(view(&g, 0).view.stack[0].player_targets, vec![PlayerId(1)]);
    pass_round(&mut g);
    let Request::Sacrifice {
        player, candidates, ..
    } = settle(&mut g)
    else {
        panic!("sacrifice")
    };
    assert_eq!(player, PlayerId(1));
    assert_eq!(
        candidates
            .iter()
            .map(|object| object.card)
            .collect::<Vec<_>>(),
        vec![CardId(0), CardId(1), CardId(5)]
    );
    assert!(view(&g, 0).decision.is_none());
    assert!(view(&g, 1).view.priority.is_none());
    let before = g.snapshot().unwrap();
    assert!(
        g.respond(
            PlayerId(0),
            g.state.revision,
            Response::Sacrifice {
                object: candidates[1]
            }
        )
        .is_err()
    );
    assert_eq!(g.snapshot().unwrap(), before);
    answer(
        &mut g,
        Response::Sacrifice {
            object: candidates[1],
        },
    );
    settle(&mut g);
    recover(&g);
    assert_eq!(
        g.state.objects[&CardId(1)].zone,
        Zone::Graveyard { owner: PlayerId(1) }
    );
    assert_eq!(
        g.state.objects[&CardId(4)].zone,
        Zone::Graveyard { owner: PlayerId(0) }
    );
}
#[test]
fn sacrificing_commander_waits_until_whole_spell_finishes_before_return_sba() {
    let mut g = Game::new(prepared(ScenarioConfig::actions()));
    cast_edict(&mut g, 1);
    pass_round(&mut g);
    let commander = g.object_id(CardId(0)).unwrap();
    answer(&mut g, Response::Sacrifice { object: commander });
    assert!(matches!(
        g.advance(Budget::new(1)),
        Advance::Suspended { .. }
    ));
    assert!(matches!(g.state.frame, Frame::Resolve { .. }));
    assert_eq!(
        g.state.objects[&CardId(0)].zone,
        Zone::Graveyard { owner: PlayerId(1) }
    );
    assert!(
        g.state
            .commander_returns
            .contains(&g.object_id(CardId(0)).unwrap())
    );
    assert!(g.decision_for(PlayerId(1)).unwrap().is_none());
    recover(&g);
    assert!(matches!(
        settle(&mut g),
        Request::CommanderReturn {
            player: PlayerId(1),
            ..
        }
    ));
    assert_eq!(
        g.state.objects[&CardId(4)].zone,
        Zone::Graveyard { owner: PlayerId(0) }
    );
    answer(&mut g, Response::CommanderReturn { to_command: true });
    settle(&mut g);
    recover(&g);
}
#[test]
fn sacrifice_source_ability_has_own_stack_identity_and_historical_reference() {
    let mut g = Game::new(prepared(ScenarioConfig::actions()));
    let before_source = view(&g, 1)
        .view
        .objects
        .into_iter()
        .find(|object| object.oracle_id == HAPLESS)
        .unwrap()
        .handle;
    let source = g.object_id(CardId(5)).unwrap();
    ability(&mut g);
    recover(&g);
    assert_eq!(
        g.state.objects[&CardId(5)].zone,
        Zone::Graveyard { owner: PlayerId(1) }
    );
    assert_eq!(g.state.stack.len(), 1);
    assert!(!g.top().unwrap().kind.is_spell());
    assert_eq!(g.top().unwrap().source, source);
    let own = view(&g, 1);
    let entry = &own.view.stack[0];
    assert!(entry.spell.is_none());
    let ability = entry.ability.as_ref().unwrap();
    assert_eq!(ability.source.announced, before_source);
    assert!(ability.source.current.is_none());
    assert!(ability.handle.0.starts_with("v1-a"));
    assert!(
        g.resolve_handle(PlayerId(1), &ability.source.announced)
            .is_err()
    );
    assert_eq!(
        g.events()
            .iter()
            .filter(|event| matches!(event, Event::AbilityActivated { .. }))
            .count(),
        1
    );
    assert!(
        !g.events()
            .iter()
            .any(|event| matches!(event, Event::SpellCast { .. }))
    );
    pass_round(&mut g);
    let Request::ResolveDiscard {
        player,
        count,
        candidates,
        ..
    } = settle(&mut g)
    else {
        panic!("discard")
    };
    assert_eq!(player, PlayerId(1));
    assert_eq!(count, 1);
    let drawn = *candidates
        .iter()
        .find(|object| object.card == CardId(17))
        .unwrap();
    answer(
        &mut g,
        Response::ResolveDiscard {
            objects: vec![drawn],
        },
    );
    settle(&mut g);
    recover(&g);
    assert!(g.state.stack.is_empty());
    assert_eq!(
        g.object_id(CardId(5)).unwrap().incarnation,
        1,
        "ability completion must not move sacrificed source again"
    );
    assert_eq!(
        g.state.objects[&CardId(17)].zone,
        Zone::Graveyard { owner: PlayerId(1) }
    );
    assert!(
        g.events()
            .iter()
            .any(|event| matches!(event, Event::AbilityCompleted { .. }))
    );
}
#[test]
fn accepted_activation_is_discarded_if_source_owner_leaves_before_cost() {
    let mut g = Game::new(prepared(ScenarioConfig::actions_borrowed()));
    answer(&mut g, Response::Pass);
    let source = g.object_id(CardId(5)).unwrap();
    answer(
        &mut g,
        Response::ActivateAbility {
            object: source,
            ability: 0,
        },
    );
    g.concede(PlayerId(2)).unwrap();
    assert!(matches!(
        settle(&mut g),
        Request::Priority {
            player: PlayerId(1),
            ..
        }
    ));
    assert!(g.state.stack.is_empty());
    assert!(
        !g.events()
            .iter()
            .any(|event| matches!(event, Event::AbilityActivated { .. }))
    );
    recover(&g);
}
#[test]
fn resolution_choice_query_sidecar_recovers_and_does_not_authorize_foreign_forks() {
    let mut g = Game::new(prepared(ScenarioConfig::actions()));
    ability(&mut g);
    pass_round(&mut g);
    let request = view(&g, 1).decision.unwrap();
    let PlayerRequest::ResolveDiscard { candidates, .. } = &request else {
        panic!("discard")
    };
    let QueryResult::Incomplete { work, .. } = g.check_player_response(
        PlayerId(1),
        request.token().clone(),
        PlayerResponse::ResolveDiscard {
            objects: vec![candidates[0].clone()],
        },
        Budget::new(1),
    ) else {
        panic!("suspended query")
    };
    let saved = g.save_work(&work).unwrap();
    let restored = Game::restore(g.prepared.clone(), &g.snapshot().unwrap()).unwrap();
    let recovered = restored.restore_work(&saved).unwrap();
    let mut result = restored.resume_work(recovered, Budget::new(1));
    let receipt = loop {
        match result {
            QueryResult::Incomplete { work, .. } => {
                result = restored.resume_work(work, Budget::new(1))
            }
            QueryResult::Complete(QueryValue::Validated(receipt)) => break receipt,
            _ => panic!("validation"),
        }
    };
    assert!(g.submit(receipt).is_err());
    let object = g.resolve_handle(PlayerId(1), &candidates[0]).unwrap();
    answer(
        &mut g,
        Response::ResolveDiscard {
            objects: vec![object],
        },
    );
    settle(&mut g);
    recover(&g);
    assert!(matches!(
        g.resume_work(work, Budget::unlimited()),
        QueryResult::Rejected(QueryRejection::StaleOrForeignWork)
    ));
}
#[test]
fn rehashed_ability_and_resolution_metadata_cannot_forge_recovery() {
    let mut g = Game::new(prepared(ScenarioConfig::actions()));
    ability(&mut g);
    for mutation in 0..4 {
        let mut snapshot: Snapshot = strict_read(&g.snapshot().unwrap()).unwrap();
        match mutation {
            0 => snapshot.state.stack[0].controller = PlayerId(2),
            1 => snapshot.state.stack[0].source.incarnation = 10,
            2 => snapshot.state.stack[0].kind = StackKind::Spell,
            _ => {
                if let StackKind::Activated { ability, .. } = &mut snapshot.state.stack[0].kind {
                    *ability = 3;
                }
            }
        }
        snapshot.state_sha256 = digest(&snapshot.state).unwrap();
        assert!(Game::restore(g.prepared.clone(), &canonical(&snapshot).unwrap()).is_err());
    }
    pass_round(&mut g);
    let mut snapshot: Snapshot = strict_read(&g.snapshot().unwrap()).unwrap();
    if let Frame::ResolutionChoice { frame, .. } = &mut snapshot.state.frame {
        frame.cursor = 0;
    }
    snapshot.state_sha256 = digest(&snapshot.state).unwrap();
    assert!(Game::restore(g.prepared.clone(), &canonical(&snapshot).unwrap()).is_err());
}
