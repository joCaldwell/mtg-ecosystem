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
        g.next_test_decision()
            .activate_mana(g.object_id(CardId(card)).unwrap()),
    );
}
fn cast_edict(g: &mut Game, target: u8) {
    Driver::answer(g, u64::MAX, g.next_test_decision().cast(CardId(4)));
    assert!(matches!(
        Driver::request(g, u64::MAX),
        Decision {
            player: PlayerId(0),
            reason: Reason::Targets { .. },
            ..
        }
    ));
    Driver::answer(
        g,
        u64::MAX,
        g.next_test_decision()
            .player_targets(vec![PlayerId(target)]),
    );
    activate_land(g, 2);
    activate_land(g, 3);
    Driver::answer(
        g,
        u64::MAX,
        Answer::Pay {
            pay: PayAnswer::Finish {
                undo_on_failure: true,
            },
        },
    );
    Driver::answer(
        g,
        u64::MAX,
        Answer::Pay {
            pay: PayAnswer::Spend {
                unrestricted: BTreeMap::from([
                    (Color::B, Quantity::from(1)),
                    (Color::G, Quantity::from(1)),
                ]),
            },
        },
    );
    Driver::request(g, u64::MAX);
}
fn pass_round(g: &mut Game) {
    for _ in 0..g.live_players().len() {
        Driver::answer(
            g,
            u64::MAX,
            Answer::Pick {
                picks: vec![vec![0]],
            },
        );
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
    let found @ Decision {
        reason: Reason::Sacrifice,
        ..
    } = Driver::request(&mut g, u64::MAX)
    else {
        panic!("sacrifice")
    };
    let player = found.player;
    let candidates = found.objects_offered();
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
            g.next_test_decision().pick_object(candidates[1])
        )
        .is_err()
    );
    assert_eq!(g.snapshot().unwrap(), before);
    {
        let selected_answer = g.next_test_decision().pick_object(candidates[1]);
        Driver::answer(&mut g, u64::MAX, selected_answer)
    };
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
    {
        let selected_answer = g.next_test_decision().pick_object(commander);
        Driver::answer(&mut g, u64::MAX, selected_answer)
    };
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
        Decision {
            player: PlayerId(1),
            reason: Reason::CommanderReturn { .. },
            ..
        }
    ));
    assert_eq!(
        g.state.rules.objects[&CardId(4)].zone,
        Zone::Graveyard { owner: PlayerId(0) }
    );
    {
        let selected_answer = g.next_test_decision().select(Candidate::Flag {
            flag: Flag::CommandZone,
        });
        Driver::answer(&mut g, u64::MAX, selected_answer)
    };
    Driver::request(&mut g, u64::MAX);
}

#[test]
fn resolution_choice_query_sidecar_recovers_and_does_not_authorize_foreign_forks() {
    let mut g = Game::new(FixtureCatalog::Action.admit(ScenarioConfig::actions()));
    cast_edict(&mut g, 1);
    pass_round(&mut g);
    let request = view(&g, 1).decision.unwrap();
    let found @ Decision {
        reason: Reason::Sacrifice,
        ..
    } = &request
    else {
        panic!("sacrifice")
    };
    let candidates = found.objects_offered();
    let QueryResult::Incomplete { work, .. } = g.check_player_response(
        PlayerId(1),
        request.id.clone(),
        g.next_test_viewer_decision()
            .pick_object(candidates[0].clone()),
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
    {
        let selected_answer = g.next_test_decision().pick_object(object);
        Driver::answer(&mut g, u64::MAX, selected_answer)
    };
    Driver::request(&mut g, u64::MAX);

    assert!(matches!(
        g.resume_work(work, Budget::unlimited()),
        QueryResult::Rejected(QueryRejection::StaleOrForeignWork)
    ));
}
