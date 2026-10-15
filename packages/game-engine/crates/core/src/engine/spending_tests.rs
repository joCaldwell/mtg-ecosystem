use super::*;

fn game() -> Game {
    let catalog = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-action-ir.json"
    ))
    .unwrap();
    Game::new(Prepared::admit(catalog, ScenarioConfig::mixed_mana()).unwrap())
}
fn settle(g: &mut Game) -> Request {
    for _ in 0..200 {
        match g.advance(Budget::new(1)) {
            Advance::AwaitingDecision(r) => return r,
            Advance::Suspended { .. } => (),
            other => panic!("unexpected {other:?}"),
        }
    }
    panic!("no request")
}
fn answer(g: &mut Game, response: Response) {
    let r = settle(g);
    g.respond(r.player(), r.decision_id(), response).unwrap();
}
fn selection(black: i64, green: i64) -> BTreeMap<Color, Quantity> {
    [(Color::B, black.into()), (Color::G, green.into())].into()
}
fn spending_game() -> Game {
    let mut g = game();
    answer(&mut g, Response::Cast { card: CardId(0) });
    for card in 1..=9 {
        let object = g.object_id(CardId(card)).unwrap();
        answer(&mut g, Response::ActivateMana { object });
    }
    answer(&mut g, Response::FinishMana { undo_mana: true });
    assert!(matches!(settle(&mut g), Request::SpendMana { .. }));
    g
}

#[test]
fn player_selects_generic_colors_and_both_branches_recover_exactly() {
    let g = spending_game();
    for (spending, remaining_b, remaining_g) in [(selection(2, 4), 2, 1), (selection(4, 2), 0, 3)] {
        let mut branch = g.fork();
        answer(
            &mut branch,
            Response::SpendMana {
                spending: spending.clone(),
            },
        );
        settle(&mut branch);
        assert_eq!(branch.mana(PlayerId(0), Color::B), Some(remaining_b.into()));
        assert_eq!(branch.mana(PlayerId(0), Color::G), Some(remaining_g.into()));
        let restored = Game::restore(branch.prepared.clone(), &branch.snapshot().unwrap()).unwrap();
        let replay = Game::replay(branch.prepared.clone(), &branch.replay_journal()).unwrap();
        assert_eq!(
            branch.state_digest().unwrap(),
            restored.state_digest().unwrap()
        );
        assert_eq!(
            branch.state_digest().unwrap(),
            replay.state_digest().unwrap()
        );
        assert!(branch.events().iter().any(|event| matches!(event,
            Event::Committed { actual: EventOutcome::ManaPaidSelected { spending: actual, .. }, .. } if actual == &spending
        )));
    }
    assert_eq!(g.mana(PlayerId(0), Color::B), Some(4.into()));
    assert_eq!(g.mana(PlayerId(0), Color::G), Some(5.into()));
}

#[test]
fn invalid_selections_do_not_mutate_state_receipts_or_history() {
    let mut g = spending_game();
    let request = settle(&mut g);
    let mut zero = selection(2, 4);
    zero.insert(Color::U, Quantity::zero());
    let mut unavailable = BTreeMap::new();
    unavailable.insert(Color::U, 6.into());
    for spending in [
        selection(1, 5),
        selection(2, 3),
        selection(3, 4),
        selection(5, 1),
        selection(-1, 7),
        zero,
        unavailable,
        BTreeMap::new(),
    ] {
        let before = g.snapshot().unwrap();
        let journal = g.replay_journal();
        assert!(
            g.respond(
                request.player(),
                request.decision_id(),
                Response::SpendMana { spending }
            )
            .is_err()
        );
        assert_eq!(g.snapshot().unwrap(), before);
        assert_eq!(g.replay_journal(), journal);
    }
    assert!(
        g.respond(
            PlayerId(1),
            request.decision_id(),
            Response::SpendMana {
                spending: selection(2, 4)
            }
        )
        .is_err()
    );
    assert!(
        g.respond(
            request.player(),
            request.decision_id(),
            Response::FinishMana { undo_mana: true }
        )
        .is_err()
    );
    answer(
        &mut g,
        Response::SpendMana {
            spending: selection(2, 4),
        },
    );
    settle(&mut g);
}

#[test]
fn exact_large_generic_costs_never_round_or_spend_extra() {
    let mut g = game();
    let huge: Quantity = "900719925474099312345678901234567890".parse().unwrap();
    g.state.players[0].mana = [(Color::B, huge.clone()), (Color::G, 1.into())].into();
    let cost = ManaCost {
        generic: huge.clone(),
        colored: vec![Color::G],
    };
    let spending = [(Color::B, huge), (Color::G, 1.into())].into();
    let remaining = g
        .selected_payment_pool(PlayerId(0), &cost, &spending)
        .unwrap();
    assert!(remaining.values().all(|n| *n == Quantity::zero()));
    assert!(
        g.selected_payment_pool(PlayerId(0), &cost, &selection(4, 1))
            .is_err()
    );
}

fn query(game: &Game, mut result: QueryResult) -> QueryValue {
    for _ in 0..200 {
        match result {
            QueryResult::Complete(value) => return value,
            QueryResult::Incomplete { work, .. } => result = game.resume_work(work, Budget::new(1)),
            QueryResult::Rejected(reason) => panic!("query rejected: {reason:?}"),
        }
    }
    panic!("query budget exhausted")
}

#[test]
fn spending_queries_are_private_pure_resumable_and_executor_scoped() {
    let mut g = spending_game();
    let before = g.snapshot().unwrap();
    let QueryValue::Observation(own) = query(&g, g.observe(PlayerId(0), Budget::new(0))) else {
        panic!("observation")
    };
    let Some(PlayerRequest::SpendMana {
        token,
        available,
        cost,
        ..
    }) = own.decision
    else {
        panic!("spending")
    };
    assert_eq!(own.view.priority, None);
    assert_eq!(available, selection(4, 5));
    assert_eq!(cost.generic, 3.into());
    for player in [PlayerId(1), PlayerId(2), PlayerId(3)] {
        let QueryValue::Observation(other) = query(&g, g.observe(player, Budget::new(1))) else {
            panic!("observation")
        };
        assert!(other.decision.is_none());
        assert_eq!(other.view.priority, None);
    }
    let QueryResult::Incomplete { work, .. } = g.check_player_response(
        PlayerId(0),
        token,
        PlayerResponse::SpendMana {
            spending: selection(2, 4),
        },
        Budget::new(0),
    ) else {
        panic!("incomplete validation")
    };
    let saved = g.save_work(&work).unwrap();
    let restored = Game::restore(g.prepared.clone(), &g.snapshot().unwrap()).unwrap();
    let recovered = restored.restore_work(&saved).unwrap();
    let QueryValue::Validated(foreign) =
        query(&restored, restored.resume_work(recovered, Budget::new(1)))
    else {
        panic!("receipt")
    };
    assert!(g.submit(foreign).is_err());
    assert_eq!(g.snapshot().unwrap(), before);
    let QueryResult::Incomplete { work: stale, .. } = g.observe(PlayerId(0), Budget::new(0)) else {
        panic!("incomplete observation")
    };
    let QueryValue::Validated(receipt) = query(&g, g.resume_work(work, Budget::new(1))) else {
        panic!("receipt")
    };
    g.submit(receipt).unwrap();
    assert!(matches!(
        g.resume_work(stale, Budget::unlimited()),
        QueryResult::Rejected(QueryRejection::StaleOrForeignWork)
    ));
    settle(&mut g);
}

#[test]
fn failed_mixed_cast_keeps_or_undoes_only_its_recorded_mana() {
    for undo_mana in [false, true] {
        let mut g = game();
        answer(
            &mut g,
            Response::ActivateMana {
                object: ObjectId {
                    card: CardId(1),
                    incarnation: 0,
                },
            },
        );
        answer(&mut g, Response::Cast { card: CardId(0) });
        answer(
            &mut g,
            Response::ActivateMana {
                object: ObjectId {
                    card: CardId(5),
                    incarnation: 0,
                },
            },
        );
        answer(&mut g, Response::FinishMana { undo_mana });
        assert!(matches!(
            settle(&mut g),
            Request::Priority {
                player: PlayerId(0),
                ..
            }
        ));
        assert_eq!(
            g.zone(g.object_id(CardId(0)).unwrap()),
            Some(Zone::Command { owner: PlayerId(0) })
        );
        assert_eq!(g.mana(PlayerId(0), Color::B), Some(1.into()));
        assert_eq!(
            g.mana(PlayerId(0), Color::G),
            Some(if undo_mana { 0.into() } else { 1.into() })
        );
        assert_eq!(
            g.current(g.object_id(CardId(5)).unwrap()).unwrap().tapped,
            !undo_mana
        );
        assert!(
            g.commander_ledger()
                .history(CardId(0))
                .unwrap()
                .command_casts
                .is_empty()
        );
        Game::restore(g.prepared.clone(), &g.snapshot().unwrap()).unwrap();
    }
}

#[test]
fn pending_spending_survives_unrelated_departure_but_never_resurrects_a_departed_payer() {
    for player in [PlayerId(2), PlayerId(0)] {
        let mut g = spending_game();
        answer(
            &mut g,
            Response::SpendMana {
                spending: selection(2, 4),
            },
        );
        g.concede(player).unwrap();
        settle(&mut g);
        if player == PlayerId(0) {
            assert!(g.object_id(CardId(0)).is_none());
            assert!(g.state.stack.is_empty());
            // Departed seats retain their last-known table for diagnostics;
            // the accepted spend must neither pay nor reverse that table.
            assert_eq!(g.state.players[0].mana, selection(4, 5));
            assert_eq!(
                g.state.players[0].last_known.as_ref().unwrap().mana,
                selection(4, 5)
            );
            assert!(
                g.commander_ledger()
                    .history(CardId(0))
                    .unwrap()
                    .command_casts
                    .is_empty()
            );
        } else {
            assert_eq!(g.mana(PlayerId(0), Color::B), Some(2.into()));
            assert_eq!(g.state.stack.len(), 1);
            assert!(g.state.stack[0].paid.is_some());
        }
        let restored = Game::restore(g.prepared.clone(), &g.snapshot().unwrap()).unwrap();
        assert_eq!(restored.state_digest().unwrap(), g.state_digest().unwrap());
    }
}

#[test]
fn targeted_player_leaving_at_accepted_spending_reverses_the_incomplete_cast() {
    let catalog = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-action-ir.json"
    ))
    .unwrap();
    let mut g = Game::new(Prepared::admit(catalog, ScenarioConfig::actions()).unwrap());
    answer(&mut g, Response::Cast { card: CardId(4) });
    answer(
        &mut g,
        Response::PlayerTargets {
            players: vec![PlayerId(1)],
        },
    );
    for card in [2, 3] {
        let object = g.object_id(CardId(card)).unwrap();
        answer(&mut g, Response::ActivateMana { object });
    }
    answer(&mut g, Response::FinishMana { undo_mana: true });
    assert!(matches!(settle(&mut g), Request::SpendMana { .. }));
    answer(
        &mut g,
        Response::SpendMana {
            spending: selection(1, 1),
        },
    );
    g.concede(PlayerId(1)).unwrap();
    settle(&mut g);
    assert!(g.state.stack.is_empty());
    assert_eq!(
        g.zone(g.object_id(CardId(4)).unwrap()),
        Some(Zone::Hand { owner: PlayerId(0) })
    );
    assert!(g.state.players[0].mana.is_empty());
    for card in [2, 3] {
        assert!(
            !g.current(g.object_id(CardId(card)).unwrap())
                .unwrap()
                .tapped
        );
    }
    let replay = Game::replay(g.prepared.clone(), &g.replay_journal()).unwrap();
    assert_eq!(replay.state_digest().unwrap(), g.state_digest().unwrap());
}
