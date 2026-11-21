use super::*;

fn prepared() -> Prepared {
    Prepared::admit(
        Catalog::read(include_str!("../../../../tests/fixtures/expected-ir.json")).unwrap(),
        ScenarioConfig::land_return(),
    )
    .unwrap()
}

fn inputs() -> Vec<AcceptedInput> {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/d1b2-payment-inputs.json"
    ))
    .unwrap();
    serde_json::from_value(fixture["accepted_inputs"].clone()).unwrap()
}

fn assert_equal_games(actual: &Game, reference: &Game) {
    assert_eq!(actual.state, reference.state);
    assert_eq!(actual.trace, reference.trace);
    assert_eq!(
        actual.state_digest().unwrap(),
        reference.state_digest().unwrap()
    );
    assert_eq!(actual.snapshot().unwrap(), reference.snapshot().unwrap());
    // Transaction ownership never escapes to a published game or branch.
    assert!(!actual.in_rule_step);
    assert!(!actual.fork().in_rule_step);
}

#[test]
fn coalesced_steps_match_nested_reference_at_every_input_and_budget_boundary() {
    let inputs = inputs();
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        for units in [1, 2, 7, u64::MAX] {
            let mut actual = Game::with_trace(prepared(), mode);
            let mut reference = Game::with_trace(prepared(), mode);
            let mut index = 0;
            let mut finished = false;
            for _ in 0..1_000 {
                let outcome = actual.advance(Budget::new(units));
                let expected =
                    reference.advance_internal::<false, false>(Budget::new(units), &mut vec![]);
                assert_eq!(outcome, expected);
                assert_equal_games(&actual, &reference);
                match outcome {
                    Advance::AwaitingDecision(_) if index == inputs.len() => {
                        finished = true;
                        break;
                    }
                    Advance::AwaitingDecision(request) => {
                        let input = &inputs[index];
                        assert_eq!(request.player(), input.player);
                        assert_eq!(request.decision_id(), input.decision_id);
                        for game in [&mut actual, &mut reference] {
                            game.respond(input.player, input.decision_id, input.response.clone())
                                .unwrap();
                        }
                        index += 1;
                        assert_equal_games(&actual, &reference);
                    }
                    Advance::Suspended { .. } => {}
                    other => panic!("unexpected fixture result: {other:?}"),
                }
            }
            assert!(finished, "fixture did not reach its final decision");
        }
    }
}

fn queued_mana(mode: TraceMode) -> Game {
    let mut game = Game::with_trace(prepared(), mode);
    game.respond(
        PlayerId(0),
        0,
        Response::ActivateMana {
            object: game.object_id(CardId(0)).unwrap(),
        },
    )
    .unwrap();
    game
}

fn assert_sticky_atomic_fault(game: &mut Game, expected_reason: &str) {
    let mut expected = game.state.clone();
    let trace = game.trace.clone();
    let token = game.state_token.clone();
    let QueryResult::Incomplete { work, .. } = game.observe(PlayerId(0), Budget::new(0)) else {
        panic!("expected pending query work")
    };
    assert_eq!(
        game.advance(Budget::new(1)),
        Advance::Fault(expected_reason.into())
    );
    expected.rules.frame = Frame::Fault {
        reason: expected_reason.into(),
    };
    expected.rules.typed_fault = None;
    assert_eq!(game.state, expected);
    assert_eq!(game.trace, trace);
    assert!(!Arc::ptr_eq(&token, &game.state_token));
    assert!(matches!(
        game.resume_work(work, Budget::unlimited()),
        QueryResult::Rejected(QueryRejection::StaleOrForeignWork)
    ));
    let fault_snapshot = game.snapshot().unwrap();
    assert_eq!(
        game.advance(Budget::new(0)),
        Advance::Fault(expected_reason.into())
    );
    assert_eq!(
        game.advance(Budget::unlimited()),
        Advance::Fault(expected_reason.into())
    );
    assert_eq!(game.snapshot().unwrap(), fault_snapshot);
    assert!(!game.in_rule_step);
}

#[test]
fn late_event_failure_rolls_back_earlier_coalesced_changes_and_evidence() {
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        let mut actual = queued_mana(mode);
        // The tap and its record succeed, then the mana event's record fails.
        // Neither mutation nor the earlier evidence may escape the rule step.
        actual.state.rules.next_event_sequence = u64::MAX - 1;
        let mut reference = actual.fork();
        assert_sticky_atomic_fault(&mut actual, "event sequence resource limit");
        assert_eq!(
            reference.advance_internal::<false, false>(Budget::new(1), &mut vec![]),
            Advance::Fault("event sequence resource limit".into())
        );
        assert_equal_games(&actual, &reference);
        assert!(!actual.state.rules.objects[&CardId(0)].tapped);
        assert_eq!(actual.mana(PlayerId(0), Color::U), Some(Quantity::zero()));
    }
}

#[test]
fn rule_boundary_overflow_is_a_sticky_atomic_fault() {
    let mut game = queued_mana(TraceMode::Detailed);
    game.state.rules.rule_steps = u64::MAX;
    assert_sticky_atomic_fault(&mut game, "execution boundary resource limit");
}

#[test]
fn decision_invariant_fault_is_sticky_and_invalidates_receipts_and_work() {
    let mut game = Game::new(prepared());
    game.respond(PlayerId(0), 0, Response::Cast { card: CardId(1) })
        .unwrap();
    assert!(matches!(
        game.advance(Budget::unlimited()),
        Advance::AwaitingDecision(Request::Targets { .. })
    ));
    let receipt = game
        .check_response(
            PlayerId(0),
            game.state.rules.revision,
            Response::Targets {
                objects: vec![game.object_id(CardId(2)).unwrap()],
            },
        )
        .unwrap();
    let QueryResult::Incomplete { work, .. } = game.observe(PlayerId(0), Budget::new(0)) else {
        panic!("expected pending query work")
    };
    // Private malformed-state probe, not an admitted/recoverable fixture.
    game.state.rules.stack.clear();
    let reason = game.decision_for(PlayerId(0)).unwrap_err();
    let trace = game.trace.clone();
    let mut expected = game.state.clone();
    assert_eq!(game.advance(Budget::new(0)), Advance::Fault(reason.clone()));
    expected.rules.frame = Frame::Fault {
        reason: reason.clone(),
    };
    assert_eq!(game.state, expected);
    assert_eq!(game.trace, trace);
    assert!(game.submit(receipt).is_err());
    assert!(matches!(
        game.resume_work(work, Budget::unlimited()),
        QueryResult::Rejected(QueryRejection::StaleOrForeignWork)
    ));
    let snapshot = game.snapshot().unwrap();
    assert_eq!(game.advance(Budget::unlimited()), Advance::Fault(reason));
    assert_eq!(game.snapshot().unwrap(), snapshot);
}

#[test]
fn payment_distinguishes_insufficient_funds_from_invalid_state() {
    let mut game = Game::new(prepared());
    let cost = ManaCost {
        generic: Quantity::from(1),
        colored: vec![],
    };
    assert_eq!(game.can_pay(PlayerId(0), &cost), Ok(false));
    assert_eq!(
        game.can_pay(PlayerId(4), &cost),
        Err("missing paying player".into())
    );
    let negative_cost = ManaCost {
        generic: Quantity::from(-1),
        colored: vec![],
    };
    assert_eq!(
        game.can_pay(PlayerId(0), &negative_cost),
        Err("negative mana cost".into())
    );
    game.state.rules.players[0]
        .mana
        .insert(Color::U, Quantity::from(-1));
    assert_eq!(
        game.can_pay(PlayerId(0), &cost),
        Err("negative mana pool".into())
    );
    game.state.rules.players[0]
        .mana
        .insert(Color::U, Quantity::from(1));
    let before = game.snapshot().unwrap();
    assert_eq!(game.can_pay(PlayerId(0), &cost), Ok(true));
    assert_eq!(game.snapshot().unwrap(), before);
}
