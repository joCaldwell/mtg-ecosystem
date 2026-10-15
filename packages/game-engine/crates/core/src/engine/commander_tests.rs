use super::*;

fn catalog() -> Catalog {
    Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-commander-ir.json"
    ))
    .unwrap()
}
fn game(config: ScenarioConfig) -> Game {
    Game::new(Prepared::admit(catalog(), config).unwrap())
}
fn recover(g: &Game) {
    let restored = Game::restore(g.prepared.clone(), &g.snapshot().unwrap()).unwrap();
    assert_eq!(restored.state, g.state);
    assert_eq!(restored.trace, g.trace);
}
fn settle(g: &mut Game) -> Request {
    for _ in 0..100 {
        let result = g.advance(Budget::new(1));
        recover(g);
        match result {
            Advance::AwaitingDecision(r) => return r,
            Advance::Suspended { .. } => {}
            other => panic!("unexpected {other:?}"),
        }
    }
    panic!("no request")
}
fn answer(g: &mut Game, response: Response) {
    let r = settle(g);
    g.respond(r.player(), r.decision_id(), response).unwrap();
    recover(g);
}
fn mana(g: &mut Game, card: u32) {
    answer(
        g,
        Response::ActivateMana {
            object: g.object_id(CardId(card)).unwrap(),
        },
    );
}
fn cast_commander(g: &mut Game, lands: &[u32]) {
    answer(g, Response::Cast { card: CardId(0) });
    for land in lands {
        mana(g, *land);
    }
    answer(g, Response::FinishMana { undo_mana: true });
    settle(g);
}
fn resolve(g: &mut Game) {
    for _ in 0..4 {
        answer(g, Response::Pass);
    }
    settle(g);
}
fn bounce(g: &mut Game) {
    answer(g, Response::Pass); // P1 receives priority
    answer(g, Response::Cast { card: CardId(7) });
    answer(
        g,
        Response::Targets {
            objects: vec![g.object_id(CardId(0)).unwrap()],
        },
    );
    mana(g, 6);
    answer(g, Response::FinishMana { undo_mana: true });
    for _ in 0..4 {
        answer(g, Response::Pass);
    }
    assert!(matches!(
        settle(g),
        Request::CommanderReturn {
            player: PlayerId(0),
            destination: Zone::Hand { owner: PlayerId(0) },
            ..
        }
    ));
}
fn observation(g: &Game, player: u8) -> Observation {
    let QueryResult::Complete(QueryValue::Observation(o)) =
        g.observe(PlayerId(player), Budget::unlimited())
    else {
        panic!("observation")
    };
    o
}
#[test]
fn command_casts_charge_exact_tax_and_failed_cast_reverses_original_zone() {
    let mut g = game(ScenarioConfig::command_casting());
    let original = g.object_id(CardId(0)).unwrap();
    answer(&mut g, Response::Cast { card: CardId(0) });
    assert!(
        matches!(settle(&mut g), Request::Mana {cost, ..} if cost.generic == Quantity::zero() && cost.colored == vec![Color::W])
    );
    answer(&mut g, Response::FinishMana { undo_mana: true });
    settle(&mut g);
    assert_eq!(g.object_id(CardId(0)), Some(original));
    assert_eq!(
        g.state.objects[&CardId(0)].zone,
        Zone::Command { owner: PlayerId(0) }
    );
    assert_eq!(
        g.commander_ledger().tax(CardId(0), PlayerId(0)).unwrap(),
        Quantity::zero()
    );
    cast_commander(&mut g, &[1]);
    assert_eq!(
        g.commander_ledger().tax(CardId(0), PlayerId(0)).unwrap(),
        Quantity::from(2)
    );
    resolve(&mut g);
    bounce(&mut g);
    answer(&mut g, Response::CommanderReturn { to_command: true });
    settle(&mut g);
    let returned = g.object_id(CardId(0)).unwrap();
    answer(&mut g, Response::Cast { card: CardId(0) });
    assert!(
        matches!(settle(&mut g), Request::Mana {cost, ..} if cost.generic == Quantity::from(2) && cost.colored == vec![Color::W])
    );
    mana(&mut g, 2);
    answer(&mut g, Response::FinishMana { undo_mana: true });
    settle(&mut g);
    assert_eq!(g.object_id(CardId(0)), Some(returned));
    assert!(!g.state.objects[&CardId(2)].tapped);
    assert_eq!(
        g.commander_ledger().tax(CardId(0), PlayerId(0)).unwrap(),
        Quantity::from(2)
    );
    cast_commander(&mut g, &[2, 3, 4]);
    assert_eq!(
        g.commander_ledger().tax(CardId(0), PlayerId(0)).unwrap(),
        Quantity::from(4)
    );
    assert_eq!(
        g.top().unwrap().paid.as_ref().unwrap().generic,
        Quantity::from(2)
    );
    resolve(&mut g);
}
#[test]
fn declined_hand_replacement_allows_untaxed_hand_cast_and_scoped_owner_choice() {
    let mut g = game(ScenarioConfig::command_casting());
    cast_commander(&mut g, &[1]);
    resolve(&mut g);
    bounce(&mut g);
    let own = observation(&g, 0);
    let Some(PlayerRequest::CommanderReturn { token, object, .. }) = own.decision else {
        panic!("owner choice")
    };
    assert!(observation(&g, 1).decision.is_none());
    let before = g.snapshot().unwrap();
    assert!(
        g.respond(
            PlayerId(1),
            g.state.revision,
            Response::CommanderReturn { to_command: true }
        )
        .is_err()
    );
    assert_eq!(before, g.snapshot().unwrap());
    let QueryResult::Complete(QueryValue::Validated(receipt)) = g.check_player_response(
        PlayerId(0),
        token.clone(),
        PlayerResponse::CommanderReturn { to_command: false },
        Budget::unlimited(),
    ) else {
        panic!("validated")
    };
    let mut branch = g.fork();
    assert!(branch.submit(receipt).is_err());
    answer(&mut g, Response::CommanderReturn { to_command: false });
    settle(&mut g);
    assert!(g.resolve_handle(PlayerId(0), &object).is_err());
    assert!(matches!(
        g.check_player_response(
            PlayerId(0),
            token,
            PlayerResponse::CommanderReturn { to_command: true },
            Budget::unlimited()
        ),
        QueryResult::Rejected(_)
    ));
    cast_commander(&mut g, &[2]);
    assert_eq!(
        g.top().unwrap().paid.as_ref().unwrap().generic,
        Quantity::zero()
    );
    assert_eq!(
        g.commander_ledger().tax(CardId(0), PlayerId(0)).unwrap(),
        Quantity::from(2)
    );
}
#[test]
fn lethal_combat_enters_graveyard_then_fresh_sba_choice_is_consumed_once() {
    for to_command in [true, false] {
        let mut g = game(ScenarioConfig::commander_lifecycle());
        loop {
            if matches!(settle(&mut g), Request::Attackers { .. }) {
                break;
            }
            answer(&mut g, Response::Pass);
        }
        let commander = g.object_id(CardId(0)).unwrap();
        answer(
            &mut g,
            Response::Attackers {
                attacks: vec![(commander, PlayerId(1))],
            },
        );
        loop {
            if matches!(settle(&mut g), Request::Blockers { .. }) {
                break;
            }
            answer(&mut g, Response::Pass);
        }
        let blocker = g.object_id(CardId(14)).unwrap();
        answer(
            &mut g,
            Response::Blockers {
                blocks: vec![(blocker, commander)],
            },
        );
        loop {
            if matches!(settle(&mut g), Request::CommanderReturn { .. }) {
                break;
            }
            answer(&mut g, Response::Pass);
        }
        assert_eq!(
            g.state.objects[&CardId(0)].zone,
            Zone::Graveyard { owner: PlayerId(0) }
        );
        assert_eq!(
            g.state.objects[&CardId(14)].zone,
            Zone::Graveyard { owner: PlayerId(1) }
        );
        assert!(g.state.commander_returns.is_empty());
        answer(&mut g, Response::CommanderReturn { to_command });
        assert!(matches!(settle(&mut g), Request::Priority { .. }));
        assert_eq!(
            g.state.objects[&CardId(0)].zone,
            if to_command {
                Zone::Command { owner: PlayerId(0) }
            } else {
                Zone::Graveyard { owner: PlayerId(0) }
            }
        );
        answer(&mut g, Response::Pass);
        assert!(matches!(settle(&mut g), Request::Priority { .. }));
    }
}
#[test]
fn departure_exile_chooses_owner_and_owner_departure_never_resurrects_commander() {
    for to_command in [true, false] {
        let mut g = game(ScenarioConfig::commander_exile());
        g.concede(PlayerId(1)).unwrap();
        assert!(matches!(
            settle(&mut g),
            Request::CommanderReturn {
                player: PlayerId(0),
                destination: Zone::Exile,
                ..
            }
        ));
        answer(&mut g, Response::CommanderReturn { to_command });
        settle(&mut g);
        assert_eq!(
            g.state.objects[&CardId(0)].zone,
            if to_command {
                Zone::Command { owner: PlayerId(0) }
            } else {
                Zone::Exile
            }
        );
        assert!(g.state.commander_returns.is_empty());
    }
    let mut g = game(ScenarioConfig::commander_exile());
    g.concede(PlayerId(1)).unwrap();
    settle(&mut g);
    g.concede(PlayerId(0)).unwrap();
    settle(&mut g);
    assert!(!g.state.objects.contains_key(&CardId(0)));
    assert!(g.commander_ledger().history(CardId(0)).is_some());
}
#[test]
fn commander_damage_is_an_atomic_explicit_unsupported_boundary() {
    let mut g = game(ScenarioConfig::commander_lifecycle());
    let before = g.snapshot().unwrap();
    let result = g.commit(
        ProposedEvent::CombatDamage {
            damage: vec![
                DamageOccurrence {
                    source: g.object_id(CardId(14)).unwrap(),
                    controller: PlayerId(1),
                    recipient: DamageRecipient::Player {
                        player: PlayerId(2),
                    },
                    amount: Quantity::from(2),
                },
                DamageOccurrence {
                    source: g.object_id(CardId(0)).unwrap(),
                    controller: PlayerId(0),
                    recipient: DamageRecipient::Player {
                        player: PlayerId(1),
                    },
                    amount: Quantity::from(2),
                },
            ],
        },
        BatchCause::Combat,
    );
    assert_eq!(
        result.unwrap_err(),
        "unsupported commander combat damage to player"
    );
    assert_eq!(g.snapshot().unwrap(), before);
}
#[test]
fn exact_commander_profiles_reject_changes_and_cross_catalog_admission() {
    let old = Catalog::read(include_str!("../../../../tests/fixtures/expected-ir.json")).unwrap();
    for config in [
        ScenarioConfig::command_casting(),
        ScenarioConfig::commander_lifecycle(),
        ScenarioConfig::commander_exile(),
    ] {
        assert!(Prepared::admit(old.clone(), config.clone()).is_err());
        let mutations: [fn(&mut ScenarioConfig); 7] = [
            |c| c.cards[0].owner = PlayerId(1),
            |c| c.cards[0].controller = PlayerId(2),
            |c| c.commanders.clear(),
            |c| c.cards[1].oracle_id = ISLAND.into(),
            |c| c.cards.reverse(),
            |c| c.mode = Mode::CommanderMatch,
            |c| c.cards[0].zone = Zone::Exile,
        ];
        for mutation in mutations {
            let mut bad = config.clone();
            mutation(&mut bad);
            assert!(Prepared::admit(catalog(), bad).is_err());
        }
    }
    assert!(Prepared::admit(catalog(), ScenarioConfig::combat_game()).is_err());
}
#[test]
fn commander_metadata_and_continuations_cannot_be_forged() {
    let mut g = game(ScenarioConfig::command_casting());
    cast_commander(&mut g, &[1]);
    resolve(&mut g);
    bounce(&mut g);
    for mutation in 0..4 {
        let mut snapshot: Snapshot = strict_read(&g.snapshot().unwrap()).unwrap();
        match mutation {
            0 => snapshot
                .state
                .commanders
                .records
                .get_mut(&CardId(0))
                .unwrap()
                .command_casts
                .insert(PlayerId(0), Quantity::from(20))
                .map(|_| ()),
            1 => {
                snapshot
                    .state
                    .commander_returns
                    .insert(g.object_id(CardId(0)).unwrap());
                None
            }
            2 => {
                if let Frame::CommanderReturn { destination, .. } = &mut snapshot.state.frame {
                    *destination = Zone::Exile;
                }
                None
            }
            _ => {
                if let Frame::CommanderReturn { continuation, .. } = &mut snapshot.state.frame
                    && let CommanderContinuation::Hand { frame, .. } = continuation.as_mut()
                {
                    frame.cursor = 1;
                }
                None
            }
        };
        snapshot.state_sha256 = digest(&snapshot.state).unwrap();
        assert!(Game::restore(g.prepared.clone(), &canonical(&snapshot).unwrap()).is_err());
    }
}

#[test]
fn commander_choice_validation_sidecar_recovers_and_choice_invalidates_work() {
    let mut g = game(ScenarioConfig::command_casting());
    cast_commander(&mut g, &[1]);
    resolve(&mut g);
    bounce(&mut g);
    let token = observation(&g, 0).decision.unwrap().token().clone();
    let QueryResult::Incomplete { work, .. } = g.check_player_response(
        PlayerId(0),
        token,
        PlayerResponse::CommanderReturn { to_command: true },
        Budget::new(1),
    ) else {
        panic!("expected suspended owner validation")
    };
    let saved = g.save_work(&work).unwrap();
    let before = g.snapshot().unwrap();
    let mut restored = Game::restore(g.prepared.clone(), &before).unwrap();
    let recovered_work = restored.restore_work(&saved).unwrap();
    let mut result = restored.resume_work(recovered_work, Budget::new(1));
    let receipt = loop {
        match result {
            QueryResult::Complete(QueryValue::Validated(receipt)) => break receipt,
            QueryResult::Incomplete { work, .. } => {
                result = restored.resume_work(work, Budget::new(1))
            }
            _ => panic!("owner validation recovery failed"),
        }
    };
    assert_eq!(g.snapshot().unwrap(), before);
    restored.submit(receipt).unwrap();
    settle(&mut restored);
    answer(&mut g, Response::CommanderReturn { to_command: true });
    settle(&mut g);
    assert_eq!(restored.state, g.state);
    assert_eq!(restored.trace, g.trace);
    assert!(matches!(
        g.resume_work(work, Budget::unlimited()),
        QueryResult::Rejected(QueryRejection::StaleOrForeignWork)
    ));
}
