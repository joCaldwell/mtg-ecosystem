use super::*;

fn view(g: &Game, player: u8) -> Observation {
    let QueryResult::Complete(QueryValue::Observation(view)) =
        g.observe(PlayerId(player), Budget::unlimited())
    else {
        panic!("observation")
    };
    view
}
fn activate_land(g: &mut Game, card: u32) {
    Driver::answer(
        g,
        u64::MAX,
        Response::ActivateMana {
            object: g.object_id(CardId(card)).unwrap(),
        },
    );
}
fn cast_edict(g: &mut Game, target: u8) {
    Driver::answer(g, u64::MAX, Response::Cast { card: CardId(4) });
    assert!(matches!(
        Driver::request(g, u64::MAX),
        Request::PlayerTargets {
            player: PlayerId(0),
            ..
        }
    ));
    Driver::answer(
        g,
        u64::MAX,
        Response::PlayerTargets {
            players: vec![PlayerId(target)],
        },
    );
    activate_land(g, 2);
    activate_land(g, 3);
    Driver::answer(g, u64::MAX, Response::FinishMana { undo_mana: true });
    Driver::answer(
        g,
        u64::MAX,
        Response::SpendMana {
            spending: BTreeMap::from([
                (Color::B, Quantity::from(1)),
                (Color::G, Quantity::from(1)),
            ]),
        },
    );
    Driver::request(g, u64::MAX);
}
fn pass_round(g: &mut Game) {
    for _ in 0..g.live_players().len() {
        Driver::answer(g, u64::MAX, Response::Pass);
    }
    Driver::request(g, u64::MAX);
}

#[test]
fn opponent_announcement_and_resolution_sacrifice_have_distinct_choosers() {
    let mut g = Game::new(FixtureCatalog::Action.admit(ScenarioConfig::actions()));
    cast_edict(&mut g, 1);

    assert_eq!(g.top().unwrap().player_targets, vec![PlayerId(1)]);
    assert!(g.top().unwrap().targets.is_empty());
    assert_eq!(view(&g, 0).view.stack[0].player_targets, vec![PlayerId(1)]);
    pass_round(&mut g);
    let Request::Sacrifice {
        player, candidates, ..
    } = Driver::request(&mut g, u64::MAX)
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
            g.state.rules.revision,
            Response::Sacrifice {
                object: candidates[1]
            }
        )
        .is_err()
    );
    assert_eq!(g.snapshot().unwrap(), before);
    Driver::answer(
        &mut g,
        u64::MAX,
        Response::Sacrifice {
            object: candidates[1],
        },
    );
    Driver::request(&mut g, u64::MAX);

    assert_eq!(
        g.state.rules.objects[&CardId(1)].zone,
        Zone::Graveyard { owner: PlayerId(1) }
    );
    assert_eq!(
        g.state.rules.objects[&CardId(4)].zone,
        Zone::Graveyard { owner: PlayerId(0) }
    );
}
#[test]
fn sacrificing_commander_waits_until_whole_spell_finishes_before_return_sba() {
    let mut g = Game::new(FixtureCatalog::Action.admit(ScenarioConfig::actions()));
    cast_edict(&mut g, 1);
    pass_round(&mut g);
    let commander = g.object_id(CardId(0)).unwrap();
    Driver::answer(&mut g, u64::MAX, Response::Sacrifice { object: commander });
    assert!(matches!(
        g.advance(Budget::new(1)),
        Advance::Suspended { .. }
    ));
    assert!(matches!(g.state.rules.frame, Frame::Resolve { .. }));
    assert_eq!(
        g.state.rules.objects[&CardId(0)].zone,
        Zone::Graveyard { owner: PlayerId(1) }
    );
    assert!(
        g.state
            .rules
            .commander_returns
            .contains(&g.object_id(CardId(0)).unwrap())
    );
    assert!(g.decision_for(PlayerId(1)).unwrap().is_none());

    assert!(matches!(
        Driver::request(&mut g, u64::MAX),
        Request::CommanderReturn {
            player: PlayerId(1),
            ..
        }
    ));
    assert_eq!(
        g.state.rules.objects[&CardId(4)].zone,
        Zone::Graveyard { owner: PlayerId(0) }
    );
    Driver::answer(
        &mut g,
        u64::MAX,
        Response::CommanderReturn { to_command: true },
    );
    Driver::request(&mut g, u64::MAX);
}

#[test]
fn resolution_choice_query_sidecar_recovers_and_does_not_authorize_foreign_forks() {
    let mut g = Game::new(FixtureCatalog::Action.admit(ScenarioConfig::actions()));
    cast_edict(&mut g, 1);
    pass_round(&mut g);
    let request = view(&g, 1).decision.unwrap();
    let PlayerRequest::Sacrifice { candidates, .. } = &request else {
        panic!("sacrifice")
    };
    let QueryResult::Incomplete { work, .. } = g.check_player_response(
        PlayerId(1),
        request.token().clone(),
        PlayerResponse::Sacrifice {
            object: candidates[0].clone(),
        },
        Budget::new(1),
    ) else {
        panic!("suspended query")
    };
    let saved = g.save_work(&work).unwrap();
    let restored = Game::restore(g.prepared.as_ref().clone(), &g.snapshot().unwrap()).unwrap();
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
    Driver::answer(&mut g, u64::MAX, Response::Sacrifice { object });
    Driver::request(&mut g, u64::MAX);

    assert!(matches!(
        g.resume_work(work, Budget::unlimited()),
        QueryResult::Rejected(QueryRejection::StaleOrForeignWork)
    ));
}
