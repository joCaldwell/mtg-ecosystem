use super::*;

// Frozen pre-report advance algorithm: independent state/budget/trace oracle.
impl Game {
    fn reference_advance(&mut self, mut budget: Budget) -> Advance {
        loop {
            if let Frame::Finished { winner } = self.state.frame {
                return Advance::Finished { winner };
            }
            if let Frame::Fault { reason } = &self.state.frame {
                return Advance::Fault(reason.clone());
            }
            if matches!(self.state.frame, Frame::Draw) {
                return Advance::Draw;
            }
            if self.state.pending.is_none() && self.state.pending_concessions.is_empty() {
                for p in self.live_players() {
                    match self.decision_for(p) {
                        Ok(Some(r)) => return Advance::AwaitingDecision(r),
                        Ok(None) => {}
                        Err(e) => return Advance::Fault(e),
                    }
                }
            }
            if !budget.consume() {
                return Advance::Suspended {
                    required_minimum: 1,
                };
            }
            // A rules step commits atomically. Resource/invariant failures retain
            // its pre-step state and become faults, never wins/losses/draws.
            let mut next = self.fork();
            if let Err(reason) = next.step() {
                self.state.frame = Frame::Fault {
                    reason: reason.clone(),
                };
                self.state_token = Arc::new(());
                return Advance::Fault(reason);
            }
            let Some(steps) = next.state.rule_steps.checked_add(1) else {
                return Advance::Fault("execution boundary resource limit".into());
            };
            next.state.rule_steps = steps;
            if let Err(reason) = next.refresh_visibility() {
                return Advance::Fault(reason);
            }
            self.state = next.state;
            self.trace = next.trace;
            self.state_token = Arc::new(());
        }
    }
}
fn prepared() -> Prepared {
    Prepared::admit(
        Catalog::read(include_str!("../../../../tests/fixtures/expected-ir.json")).unwrap(),
        ScenarioConfig::land_return(),
    )
    .unwrap()
}
fn fixture() -> Game {
    Game::with_trace(prepared(), TraceMode::Off)
}
fn answer(game: &mut Game, response: Response) {
    let finish = matches!(response, Response::FinishMana { .. });
    let Advance::AwaitingDecision(request) = game.advance(Budget::unlimited()) else {
        panic!("decision")
    };
    game.respond(request.player(), request.decision_id(), response)
        .unwrap();
    if finish {
        game.complete_test_mana();
    }
}
fn pending_identity(game: &Game) -> CoreInputIdentity {
    let counts = game.accepted_input_counts().unwrap();
    let input = game.state.pending.as_ref().unwrap();
    CoreInputIdentity::Decision {
        journal_ordinal: counts.decisions - 1,
        decision_id: input.decision_id,
        player: input.player,
    }
}
fn projection(game: &Game, viewer: PlayerId) -> serde_json::Value {
    match game.observe(viewer, Budget::unlimited()) {
        QueryResult::Complete(QueryValue::Observation(value)) => {
            serde_json::to_value(value).unwrap()
        }
        QueryResult::Rejected(reason) => serde_json::json!({"rejection":format!("{reason:?}")}),
        _ => panic!("query"),
    }
}
fn compare(game: &Game, budget: u64) -> (Game, AdvanceReport) {
    let mut reported = game.fork();
    let mut ordinary = game.fork();
    let mut reference = game.fork();
    let result = reported.advance_with_report(Budget::new(budget));
    assert_eq!(result.advance, ordinary.advance(Budget::new(budget)));
    assert_eq!(
        result.advance,
        reference.reference_advance(Budget::new(budget))
    );
    for other in [&ordinary, &reference] {
        assert_eq!(reported.state, other.state);
        assert_eq!(reported.trace, other.trace);
        assert_eq!(reported.journal(), other.journal());
        assert_eq!(
            reported.state_digest().unwrap(),
            other.state_digest().unwrap()
        );
        for viewer in (0..4).map(PlayerId) {
            assert_eq!(projection(&reported, viewer), projection(other, viewer));
        }
    }
    (reported, result)
}
#[test]
fn ordinary_consumption_matches_original_advance_at_all_budgets_and_trace_modes() {
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        let mut game = Game::with_trace(prepared(), mode);
        assert_eq!(
            game.accepted_input_counts().unwrap(),
            AcceptedInputCounts {
                decisions: 0,
                concessions: 0
            }
        );
        answer(&mut game, Response::Pass);
        let identity = pending_identity(&game);
        let boundary = game.rule_boundary();
        for budget in [0, 1, 64, u64::MAX] {
            let (_, report) = compare(&game, budget);
            if budget == 0 {
                assert!(report.processed.is_empty());
            } else {
                assert_eq!(
                    report.processed,
                    vec![InputProcessingFact {
                        identity,
                        cause: InputProcessingCause::RulesStep,
                        boundary: boundary + 1
                    }]
                );
            }
        }
        let mut disabled = game.fork();
        let sentinel = InputProcessingFact {
            identity,
            cause: InputProcessingCause::RulesStep,
            boundary: 999,
        };
        let mut facts = vec![sentinel];
        disabled.advance_internal::<false>(Budget::unlimited(), &mut facts);
        assert_eq!(facts, vec![sentinel]);
    }
}
#[test]
fn actor_and_target_departures_report_actual_pending_removal_not_boundary_progress() {
    for departing in [0, 1, 2, 3] {
        let mut game = fixture();
        answer(&mut game, Response::Cast { card: CardId(1) });
        // Announce target before its owner/default controller disappears.
        let Advance::AwaitingDecision(Request::Targets { .. }) = game.advance(Budget::unlimited())
        else {
            panic!("targets")
        };
        let object = game.object_id(CardId(2)).unwrap();
        answer(
            &mut game,
            Response::Targets {
                objects: vec![object],
            },
        );
        let identity = pending_identity(&game);
        game.concede(PlayerId(departing)).unwrap();
        let boundary = game.rule_boundary();
        for budget in [0, 1, 64, u64::MAX] {
            compare(&game, budget);
        }
        let (mut after, report) = compare(&game, 1);
        let concession = InputProcessingFact {
            identity: CoreInputIdentity::Concession {
                concession_ordinal: 0,
                player: PlayerId(departing),
            },
            cause: InputProcessingCause::DepartureStep,
            boundary: boundary + 1,
        };
        if departing == 3 {
            assert_eq!(report.processed, vec![concession]);
            assert!(after.state.pending.is_some());
            let next = after.advance_with_report(Budget::new(1));
            assert_eq!(next.processed[0].identity, identity);
            assert_eq!(next.processed[0].cause, InputProcessingCause::RulesStep);
        } else {
            assert_eq!(
                report.processed,
                vec![
                    InputProcessingFact {
                        identity,
                        cause: InputProcessingCause::DepartureStep,
                        boundary: boundary + 1
                    },
                    concession
                ]
            );
            assert!(after.state.pending.is_none());
        }
    }
}
#[test]
fn queued_concessions_have_distinct_ordinals_and_terminal_cleanup_reports_actual_removal() {
    for order in [[3, 0, 2], [3, 2, 1]] {
        let mut game = fixture();
        answer(&mut game, Response::Pass);
        let identity = pending_identity(&game);
        for player in order {
            game.concede(PlayerId(player)).unwrap();
        }
        assert_eq!(
            game.accepted_input_counts().unwrap(),
            AcceptedInputCounts {
                decisions: 1,
                concessions: 3
            }
        );
        for budget in [0, 1, 64, u64::MAX] {
            compare(&game, budget);
        }
        let (after, report) = compare(&game, u64::MAX);
        assert!(matches!(report.advance, Advance::Finished { .. }));
        let concessions: Vec<_> = report
            .processed
            .iter()
            .filter_map(|fact| match fact.identity {
                CoreInputIdentity::Concession {
                    concession_ordinal,
                    player,
                } => Some((concession_ordinal, player)),
                _ => None,
            })
            .collect();
        assert_eq!(
            concessions,
            order
                .into_iter()
                .enumerate()
                .map(|(ordinal, player)| (ordinal as u64, PlayerId(player)))
                .collect::<Vec<_>>()
        );
        let decisions: Vec<_> = report
            .processed
            .iter()
            .filter(|fact| matches!(fact.identity, CoreInputIdentity::Decision { .. }))
            .collect();
        assert_eq!(decisions.len(), 1);
        assert_eq!(decisions[0].identity, identity);
        assert_eq!(decisions[0].cause, InputProcessingCause::DepartureStep);
        // finish_if_ended explicitly clears pending for the survivor too.
        // Observe that removal at the final departure, not at earlier progress.
        let ordinal = order.iter().position(|player| *player == 0).unwrap_or(2);
        assert_eq!(
            decisions[0].boundary,
            game.rule_boundary() + ordinal as u64 + 1
        );
        assert!(after.state.pending.is_none());
        let mut terminal = after;
        assert!(
            terminal
                .advance_with_report(Budget::unlimited())
                .processed
                .is_empty()
        );
        // Private boundary fixture: merely starting a call in Finished must
        // not invent consumption of an unchanged pending input. This forged
        // state is not admitted/restored as a reachable game.
        terminal.state.pending = terminal.state.journal.last().cloned();
        assert!(
            terminal
                .advance_with_report(Budget::unlimited())
                .processed
                .is_empty()
        );
        assert!(terminal.state.pending.is_some());
    }
}
#[test]
fn setup_and_commander_nested_departure_removals_are_captured_at_commit() {
    let p = Prepared::admit_pod(
        Catalog::read(include_str!(
            "../../../../tests/fixtures/expected-match-ir.json"
        ))
        .unwrap(),
        MatchConfig::foundations([0; 32], PlayerId(0)),
    )
    .unwrap();
    let mut setup = Game::with_trace(p, TraceMode::Off);
    answer(&mut setup, Response::Mulligan { take: false });
    let identity = pending_identity(&setup);
    setup.concede(PlayerId(0)).unwrap();
    for budget in [0, 1, 64, u64::MAX] {
        compare(&setup, budget);
    }
    let (_, report) = compare(&setup, 1);
    assert_eq!(report.processed[0].identity, identity);
    assert_eq!(
        report.processed[0].cause,
        InputProcessingCause::DepartureStep
    );
    let p = Prepared::admit(
        Catalog::read(include_str!(
            "../../../../tests/fixtures/expected-commander-ir.json"
        ))
        .unwrap(),
        ScenarioConfig::commander_exile(),
    )
    .unwrap();
    let mut commander = Game::with_trace(p, TraceMode::Off);
    commander.concede(PlayerId(1)).unwrap();
    let Advance::AwaitingDecision(Request::CommanderReturn { .. }) =
        commander.advance(Budget::unlimited())
    else {
        panic!("return")
    };
    answer(
        &mut commander,
        Response::CommanderReturn { to_command: true },
    );
    let identity = pending_identity(&commander);
    commander.concede(PlayerId(0)).unwrap();
    for budget in [0, 1, 64, u64::MAX] {
        compare(&commander, budget);
    }
    let (_, report) = compare(&commander, 1);
    assert_eq!(report.processed[0].identity, identity);
    assert_eq!(
        report.processed[0].cause,
        InputProcessingCause::DepartureStep
    );
    assert_eq!(
        report.processed[1].identity,
        CoreInputIdentity::Concession {
            concession_ordinal: 1,
            player: PlayerId(0)
        }
    );
}
#[test]
fn failed_steps_emit_no_fact_and_prior_committed_facts_survive_later_fault_in_same_call() {
    let mut game = fixture();
    answer(&mut game, Response::Pass);
    // Domain failure after pending.take(): the staged branch must roll back.
    game.state.next_event_sequence = u64::MAX;
    let pending = game.state.pending.clone();
    let (after, report) = compare(&game, u64::MAX);
    assert!(matches!(report.advance, Advance::Fault(_)));
    assert!(report.processed.is_empty());
    assert_eq!(after.state.pending, pending);
    let mut fault = after;
    assert!(
        fault
            .advance_with_report(Budget::unlimited())
            .processed
            .is_empty()
    );
    // First input step commits, then the following stabilization step cannot
    // increment the structural boundary. Keep the first committed input fact.
    let p = Prepared::admit(
        Catalog::read(include_str!("../../../../tests/fixtures/expected-ir.json")).unwrap(),
        ScenarioConfig::combat_game(),
    )
    .unwrap();
    let mut game = Game::with_trace(p, TraceMode::Off);
    answer(&mut game, Response::Pass);
    let identity = pending_identity(&game);
    game.state.rule_steps = u64::MAX - 1;
    let (after, report) = compare(&game, u64::MAX);
    assert!(matches!(report.advance, Advance::Fault(_)));
    assert_eq!(
        report.processed,
        vec![InputProcessingFact {
            identity,
            cause: InputProcessingCause::RulesStep,
            boundary: u64::MAX
        }]
    );
    assert!(after.state.pending.is_none());
}
#[test]
fn fork_snapshot_restore_and_automatic_steps_do_not_carry_historical_receipts() {
    let mut game = fixture();
    answer(&mut game, Response::Pass);
    let identity = pending_identity(&game);
    let restored = Game::restore(prepared(), &game.snapshot().unwrap()).unwrap();
    let (mut actual, report) = compare(&restored, u64::MAX);
    assert_eq!(report.processed[0].identity, identity);
    let after = Game::restore(prepared(), &actual.snapshot().unwrap()).unwrap();
    assert!(
        actual
            .advance_with_report(Budget::unlimited())
            .processed
            .is_empty()
    );
    let mut fork = after.fork();
    assert!(
        fork.advance_with_report(Budget::unlimited())
            .processed
            .is_empty()
    );
    assert_eq!(actual.state_digest().unwrap(), fork.state_digest().unwrap());
    let json = canonical(&report.processed).unwrap();
    let decoded: Vec<InputProcessingFact> = strict_read(&json).unwrap();
    assert_eq!(decoded, report.processed);
    let mut value = serde_json::to_value(report.processed[0]).unwrap();
    value["response"] = serde_json::json!("not permitted in receipt");
    assert!(strict_read::<InputProcessingFact>(&value.to_string()).is_err());
}
