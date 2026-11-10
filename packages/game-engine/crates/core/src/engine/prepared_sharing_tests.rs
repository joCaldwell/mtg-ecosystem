//! Source-only oracle tests for immutable Prepared ownership, not timing claims.
use super::*;

fn historical() -> Prepared {
    Prepared::admit(
        Catalog::read(include_str!("../../../../tests/fixtures/expected-ir.json")).unwrap(),
        ScenarioConfig::land_return(),
    )
    .unwrap()
}
fn admitted_match() -> Prepared {
    Prepared::admit_pod(
        Catalog::read(include_str!(
            "../../../../tests/fixtures/expected-match-ir.json"
        ))
        .unwrap(),
        MatchConfig::for_profile("commander-foundations-v1", [21; 32], PlayerId(0)).unwrap(),
    )
    .unwrap()
}

// Independently spell out ALL original derived-Clone Prepared fields. A new
// Prepared field must be accounted for here at compile time. Catalog was already
// shared in the frozen implementation; all other owned data are deeply copied.
// This is the old fork ownership oracle, NOT an alternate rules interpreter.
fn old_deep_owned_fork(game: &Game) -> Game {
    Game {
        prepared: Arc::new(Prepared {
            match_config: game.prepared.match_config.clone(),
            catalog: Arc::clone(&game.prepared.catalog),
            catalog_digest: game.prepared.catalog_digest.clone(),
            config: game.prepared.config.clone(),
            profile_digest: game.prepared.profile_digest.clone(),
        }),
        state: game.state.clone(),
        trace: game.trace.clone(),
        executor: Arc::new(()),
        state_token: Arc::new(()),
        in_rule_step: false,
    }
}
fn complete(game: &Game, mut value: QueryResult) -> QueryValue {
    for _ in 0..100_000 {
        match value {
            QueryResult::Complete(v) => return v,
            QueryResult::Incomplete { work, .. } => value = game.resume_work(work, Budget::new(1)),
            QueryResult::Rejected(e) => panic!("query rejected: {e:?}"),
        }
    }
    panic!("bounded query did not complete")
}
fn observation(game: &Game, viewer: PlayerId) -> Observation {
    match complete(game, game.observe(viewer, Budget::new(1))) {
        QueryValue::Observation(v) => v,
        _ => panic!("wrong query result"),
    }
}
fn ready(game: &mut Game, units: u64) -> Request {
    for _ in 0..10_000 {
        match game.advance(Budget::new(units)) {
            Advance::AwaitingDecision(r) => return r,
            Advance::Suspended { .. } => (),
            other => panic!("unexpected boundary: {other:?}"),
        }
    }
    panic!("bounded advance did not complete")
}
fn full_equal(a: &Game, b: &Game) {
    assert_eq!(a.state, b.state);
    assert_eq!(a.trace, b.trace);
    assert_eq!(a.config(), b.config());
    assert_eq!(a.prepared.match_config(), b.prepared.match_config());
    assert_eq!(a.prepared.catalog_digest(), b.prepared.catalog_digest());
    assert_eq!(a.prepared.profile_digest(), b.prepared.profile_digest());
    // Full generic canonical State, independent of production history/digest
    // memoization. No fields, history rows, RNG or trace facts are projected out.
    let generic = canonical(&a.state).unwrap();
    assert_eq!(generic, canonical(&b.state).unwrap());
    assert_eq!(sha256(generic.as_bytes()), a.state_digest().unwrap());
    assert_eq!(sha256(generic.as_bytes()), b.state_digest().unwrap());
    assert_eq!(a.snapshot().unwrap(), b.snapshot().unwrap());
}
fn checked_pass(game: &Game) -> ValidatedResponse {
    let o = observation(game, PlayerId(0));
    let QueryValue::Validated(receipt) = complete(
        game,
        game.check_player_response(
            PlayerId(0),
            o.decision.unwrap().token().clone(),
            PlayerResponse::Pass,
            Budget::new(1),
        ),
    ) else {
        panic!("pass validation")
    };
    receipt
}

#[test]
fn prepared_arc_matches_old_deep_fork_and_keeps_receipts_and_work_executor_scoped() {
    let parent = Game::with_trace(historical(), TraceMode::Detailed);
    let mut shared = parent.fork();
    let owned = old_deep_owned_fork(&parent);
    assert!(Arc::ptr_eq(&parent.prepared, &shared.prepared));
    assert!(!Arc::ptr_eq(&parent.prepared, &owned.prepared));
    assert!(Arc::ptr_eq(
        &parent.prepared.catalog,
        &owned.prepared.catalog
    ));
    assert_ne!(
        parent.config().cards.as_ptr(),
        owned.config().cards.as_ptr()
    );
    full_equal(&shared, &owned);
    let before = canonical(&shared.state).unwrap();
    assert!(shared.submit(checked_pass(&parent)).is_err());
    assert_eq!(before, canonical(&shared.state).unwrap());
    let QueryResult::Incomplete { work, .. } = parent.observe(PlayerId(0), Budget::new(0)) else {
        panic!("Budget0 must suspend")
    };
    assert!(matches!(
        shared.resume_work(work, Budget::unlimited()),
        QueryResult::Rejected(QueryRejection::StaleOrForeignWork)
    ));
    shared.submit(checked_pass(&shared)).unwrap();
    assert_ne!(shared.state, parent.state);
    assert!(Arc::ptr_eq(&shared.prepared, &parent.prepared));
    assert!(parent.state.rules.pending.is_none());
}

#[test]
fn prepared_arc_frozen_payment_journal_matches_owned_oracle_at_every_budget_boundary() {
    let raw: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/d1b2-payment-inputs.json"
    ))
    .unwrap();
    let inputs: Vec<AcceptedInput> =
        serde_json::from_value(raw["accepted_inputs"].clone()).unwrap();
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        let mut actual = Game::with_trace(historical(), mode);
        let mut reference = old_deep_owned_fork(&actual);
        for input in &inputs {
            // Every top-level reference operation starts from an independently
            // owned Prepared copy. Nested rules remain the identical engine;
            // this tests representation equivalence, not independent MTG rules.
            reference = old_deep_owned_fork(&reference);
            let a = ready(&mut actual, 1);
            let b = ready(&mut reference, u64::MAX);
            assert_eq!(a, b);
            full_equal(&actual, &reference);
            assert_eq!(a.player(), input.player);
            assert_eq!(a.decision_id(), input.decision_id);
            actual
                .respond(input.player, input.decision_id, input.response.clone())
                .unwrap();
            reference
                .respond(input.player, input.decision_id, input.response.clone())
                .unwrap();
            full_equal(&actual, &reference);
        }
        ready(&mut actual, 1);
        ready(&mut reference, u64::MAX);
        full_equal(&actual, &reference);
        let replay = Game::replay_with_trace(
            actual.prepared.as_ref().clone(),
            &actual.replay_journal(),
            mode,
        )
        .unwrap();
        full_equal(&actual, &replay);
    }
}

fn setup_and_empty_combat_policy(r: &Request) -> Response {
    match r {
        Request::Mulligan {
            player,
            mulligans_taken,
            can_mulligan,
            ..
        } => Response::Mulligan {
            take: *can_mulligan && *mulligans_taken < player.0,
        },
        Request::BottomOpening {
            candidates, count, ..
        } => Response::BottomOpening {
            objects: candidates.iter().rev().take(*count).copied().collect(),
        },
        Request::Priority { .. } => Response::Pass,
        Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
        Request::Blockers { .. } => Response::Blockers { blocks: vec![] },
        Request::Discard {
            candidates, count, ..
        } => Response::Discard {
            objects: candidates.iter().rev().take(*count).copied().collect(),
        },
        other => panic!("undeclared fixture request: {other:?}"),
    }
}
#[test]
fn prepared_arc_real400_decks_setup_knowledge_and_departures_match_deep_owned_reference() {
    let root = Game::with_trace(admitted_match(), TraceMode::Off);
    let mut actual = root.fork();
    let mut reference = old_deep_owned_fork(&root);
    for i in 0..64 {
        if let Some(player) = match i {
            24 => Some(PlayerId(2)),
            48 => Some(PlayerId(1)),
            _ => None,
        } {
            actual.concede(player).unwrap();
            reference.concede(player).unwrap();
        }
        reference = old_deep_owned_fork(&reference);
        let a = ready(&mut actual, 1);
        let b = ready(&mut reference, u64::MAX);
        assert_eq!(a, b);
        full_equal(&actual, &reference);
        for viewer in (0..4).map(PlayerId) {
            assert_eq!(
                observation(&actual, viewer),
                observation(&reference, viewer)
            );
        }
        let response = setup_and_empty_combat_policy(&a);
        actual
            .respond(a.player(), a.decision_id(), response.clone())
            .unwrap();
        reference
            .respond(b.player(), b.decision_id(), response)
            .unwrap();
        full_equal(&actual, &reference);
    }
    ready(&mut actual, 1);
    ready(&mut reference, u64::MAX);
    full_equal(&actual, &reference);
    assert_eq!(actual.state.rules.cards.len(), 400);
    assert_eq!(actual.config().cards.len(), 400);
    assert_eq!(actual.live_players(), [PlayerId(0), PlayerId(3)]);
    let restored = Game::restore(
        actual.prepared.as_ref().clone(),
        &actual.snapshot().unwrap(),
    )
    .unwrap();
    full_equal(&actual, &restored);
}

#[test]
fn prepared_arc_failed_atomic_batch_keeps_state_trace_and_preexisting_receipt_valid() {
    let parent = Game::with_trace(historical(), TraceMode::Detailed);
    let mut actual = parent.fork();
    let mut reference = old_deep_owned_fork(&parent);
    for game in [&mut actual, &mut reference] {
        let before = canonical(&game.state).unwrap();
        let trace = canonical(&game.trace).unwrap();
        let receipt = checked_pass(game);
        let object = game.object_id(CardId(0)).unwrap();
        let group = EventGroup::Ordered {
            children: vec![
                EventGroup::Occurrence {
                    proposed: ProposedEvent::Tap { object },
                },
                EventGroup::Occurrence {
                    proposed: ProposedEvent::AddMana {
                        player: PlayerId(0),
                        source: object,
                        color: Color::U,
                        amount: Quantity::from(-1),
                    },
                },
            ],
        };
        assert!(game.commit_batch(BatchCause::Fixture, group).is_err());
        assert_eq!(before, canonical(&game.state).unwrap());
        assert_eq!(trace, canonical(&game.trace).unwrap());
        game.submit(receipt).unwrap();
    }
    full_equal(&actual, &reference);
    assert!(Arc::ptr_eq(&parent.prepared, &actual.prepared));
    assert!(parent.state.rules.pending.is_none());
}

#[test]
fn prepared_arc_private_cow_tamper_does_not_mutate_admitted_parent() {
    let parent = Game::with_trace(admitted_match(), TraceMode::Off);
    let mut branch = parent.fork();
    let original = canonical(parent.config()).unwrap();
    // Deliberate internal corruption only. Public Game exposes no mutable
    // Prepared access; any future deliberate mutation must explicitly detach.
    Arc::make_mut(&mut branch.prepared).config.cards[0].owner = PlayerId(3);
    assert!(!Arc::ptr_eq(&parent.prepared, &branch.prepared));
    assert_eq!(canonical(parent.config()).unwrap(), original);
    assert!(parent.validate_state().is_ok());
    assert!(branch.validate_state().is_err());
    let restored = Game::restore(
        parent.prepared.as_ref().clone(),
        &parent.snapshot().unwrap(),
    )
    .unwrap();
    full_equal(&parent, &restored);
}
