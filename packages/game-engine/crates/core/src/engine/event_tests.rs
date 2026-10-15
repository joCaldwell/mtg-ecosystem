use super::*;
fn prepared() -> Prepared {
    Prepared::admit(
        Catalog::read(include_str!("../../../../tests/fixtures/expected-ir.json")).unwrap(),
        ScenarioConfig::land_return(),
    )
    .unwrap()
}
fn leaf(proposed: ProposedEvent) -> EventGroup {
    EventGroup::Occurrence { proposed }
}
fn land() -> ObjectId {
    ObjectId {
        card: CardId(0),
        incarnation: 0,
    }
}
fn mana(amount: i64) -> ProposedEvent {
    ProposedEvent::AddMana {
        player: PlayerId(0),
        source: land(),
        color: Color::U,
        amount: Quantity::from(amount),
    }
}
fn batches(game: &Game) -> Vec<&CommittedBatch> {
    game.trace
        .records
        .iter()
        .filter_map(|r| match &r.record {
            EvidenceRecord::Batch { batch } => Some(batch.as_ref()),
            _ => None,
        })
        .collect()
}
fn observe(game: &Game) -> Observation {
    let QueryResult::Complete(QueryValue::Observation(o)) =
        game.observe(PlayerId(1), Budget::unlimited())
    else {
        panic!()
    };
    o
}
#[test]
fn ordered_occurrences_capture_actual_prior_changes_and_nested_lineage() {
    let mut game = Game::new(prepared());
    let receipt = game
        .commit_batch(
            BatchCause::Fixture,
            EventGroup::Ordered {
                children: vec![
                    leaf(ProposedEvent::Tap { object: land() }),
                    EventGroup::Ordered {
                        children: vec![leaf(mana(1))],
                    },
                ],
            },
        )
        .unwrap();
    assert!(!receipt.before.objects[0].tapped);
    assert!(!receipt.occurrences[0].before.objects[0].tapped);
    assert!(receipt.occurrences[1].before.objects[0].tapped);
    assert_eq!(
        receipt.occurrences[1].id,
        OccurrenceId {
            batch: 0,
            path: vec![1, 0]
        }
    );
    assert!(
        matches!(receipt.result(&[1,0]),Some(EventOutcome::ManaAdded{amount,..}) if *amount==Quantity::from(1))
    );
    assert!(receipt.result(&[9]).is_none());
    assert_eq!(batches(&game), vec![&receipt]);
}
#[test]
fn simultaneous_independent_moves_share_pre_state_and_return_actual_successors() {
    let mut game = Game::new(prepared());
    let bear = game.object_id(CardId(2)).unwrap();
    let receipt = game
        .commit_batch(
            BatchCause::Fixture,
            EventGroup::Ordered {
                children: vec![EventGroup::Simultaneous {
                    occurrences: vec![
                        ProposedEvent::Move {
                            object: land(),
                            destination: Zone::Exile,
                        },
                        ProposedEvent::Move {
                            object: bear,
                            destination: Zone::Hand { owner: PlayerId(1) },
                        },
                    ],
                }],
            },
        )
        .unwrap();
    assert_eq!(receipt.before.objects.len(), 2);
    for (i, result) in receipt.occurrences.iter().enumerate() {
        assert_eq!(result.id.path, vec![0, i as u32]);
        assert_eq!(result.before.objects[0].zone, Zone::Battlefield);
        let EventOutcome::Moved { movement } = &result.actual else {
            panic!()
        };
        assert_eq!(movement.successor.incarnation, 1);
        assert_eq!(
            game.object_id(movement.successor.card),
            Some(movement.successor)
        );
        assert!(game.current(movement.previous).is_err());
    }
    assert_eq!(game.state.next_batch_id, 1);
    assert_eq!(game.state.next_event_sequence, 3);
}
#[test]
fn failed_children_conflicts_limits_and_overflows_roll_back_state_and_evidence() {
    let initial = Game::new(prepared());
    let mut nested = leaf(mana(1));
    for _ in 0..5 {
        nested = EventGroup::Ordered {
            children: vec![nested],
        };
    }
    for group in [
        EventGroup::Ordered {
            children: vec![leaf(ProposedEvent::Tap { object: land() }), leaf(mana(-1))],
        },
        EventGroup::Simultaneous {
            occurrences: vec![ProposedEvent::Tap { object: land() }, mana(1)],
        },
        EventGroup::Simultaneous {
            occurrences: vec![mana(1), mana(1)],
        },
        EventGroup::Ordered { children: vec![] },
        EventGroup::Simultaneous {
            occurrences: vec![],
        },
        EventGroup::Ordered {
            children: (0..17).map(|_| leaf(mana(1))).collect(),
        },
        nested,
        leaf(ProposedEvent::Move {
            object: ObjectId {
                card: CardId(0),
                incarnation: 99,
            },
            destination: Zone::Exile,
        }),
        leaf(ProposedEvent::LeaveGame {
            object: land(),
            departing_owner: PlayerId(0),
        }),
        leaf(ProposedEvent::PayMana {
            player: PlayerId(0),
            cost: ManaCost {
                generic: Quantity::from(-1),
                colored: vec![],
            },
        }),
        leaf(ProposedEvent::Move {
            object: land(),
            destination: Zone::Library { owner: PlayerId(1) },
        }),
    ] {
        let mut game = initial.fork();
        let token = game.state_token.clone();
        assert!(game.commit_batch(BatchCause::Fixture, group).is_err());
        assert_eq!(game.state, initial.state);
        assert_eq!(game.trace, initial.trace);
        assert!(Arc::ptr_eq(&token, &game.state_token));
    }
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        for event_counter in [false, true] {
            let mut game = Game::with_trace(prepared(), mode);
            if event_counter {
                game.state.next_event_sequence = u64::MAX;
            } else {
                game.state.next_batch_id = u64::MAX;
            }
            let state = game.state.clone();
            let trace = game.trace.clone();
            assert!(
                game.commit_batch(BatchCause::Fixture, leaf(mana(1)))
                    .is_err()
            );
            assert_eq!(game.state, state);
            assert_eq!(game.trace, trace);
        }
    }
}
fn run(mode: TraceMode, units: u64) -> Game {
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/expected-payment-scenario.json"
    ))
    .unwrap();
    let inputs: Vec<AcceptedInput> =
        serde_json::from_value(expected["accepted_inputs"].clone()).unwrap();
    let p = prepared();
    let mut game = Game::with_trace(p.clone(), mode);
    let mut index = 0;
    loop {
        match game.advance(Budget::new(units)) {
            Advance::AwaitingDecision(_) if index == inputs.len() => break,
            Advance::AwaitingDecision(r) => {
                let i = &inputs[index];
                assert_eq!(r.decision_id(), i.decision_id);
                game.respond(i.player, i.decision_id, i.response.clone())
                    .unwrap();
                index += 1;
            }
            Advance::Suspended { .. } => {}
            other => panic!("unexpected scenario outcome: {other:?}"),
        }
        let restored = Game::restore(p.clone(), &game.snapshot().unwrap()).unwrap();
        assert_eq!(restored.state, game.state);
        assert_eq!(restored.trace, game.trace);
        game = restored;
    }
    game
}
#[test]
fn every_trace_mode_and_budget_partition_preserves_semantics_queries_and_recovery() {
    let detailed = run(TraceMode::Detailed, u64::MAX);
    let view = observe(&detailed);
    assert_eq!(detailed.events().len(), 13);
    assert_eq!(batches(&detailed).len(), 5);
    assert!(matches!(
        batches(&detailed)[0].cause,
        BatchCause::ManaActivation { decision_id: 0, .. }
    ));
    assert!(matches!(
        batches(&detailed)[0].group,
        EventGroup::Ordered { .. }
    ));
    assert!(matches!(
        batches(&detailed)[3].cause,
        BatchCause::Instruction {
            source_block: 0,
            cursor: 0,
            ..
        }
    ));
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        for units in [1, 2, 3, u64::MAX] {
            let game = run(mode, units);
            assert_eq!(game.state, detailed.state);
            assert_eq!(
                game.state_digest().unwrap(),
                detailed.state_digest().unwrap()
            );
            assert_eq!(observe(&game), view);
            if mode == TraceMode::Off {
                assert!(game.trace.verification.is_none());
            } else {
                assert_eq!(game.trace.verification, detailed.trace.verification);
            }
            if mode != TraceMode::Detailed {
                assert!(game.events().is_empty());
                assert!(game.trace.records.is_empty());
            }
            let replay = Game::replay_with_trace(prepared(), &game.replay_journal(), mode).unwrap();
            assert_eq!(replay.trace, game.trace);
            assert_eq!(replay.state, game.state);
        }
    }
    let json: serde_json::Value = serde_json::from_str(&detailed.snapshot().unwrap()).unwrap();
    assert!(json["state"].get("trace").is_none());
    assert!(json["state"].get("events").is_none());
    assert_eq!(json["snapshot_version"], 9);
}
#[test]
fn trace_and_branch_mutation_cannot_change_rules_or_forge_recovery() {
    let game = run(TraceMode::Detailed, 1);
    let mut branch = game.fork();
    let original = game.snapshot().unwrap();
    branch.trace.records.clear();
    assert_eq!(branch.state_digest().unwrap(), game.state_digest().unwrap());
    assert_eq!(observe(&branch), observe(&game));
    assert_eq!(game.snapshot().unwrap(), original);
    assert!(Game::restore(prepared(), &branch.snapshot().unwrap()).is_err());
    let mut snapshot: Snapshot = strict_read(&original).unwrap();
    snapshot.trace.verification.as_mut().unwrap().sha256 = "forged".into();
    assert!(Game::restore(prepared(), &canonical(&snapshot).unwrap()).is_err());
    let mut snapshot: Snapshot = strict_read(&original).unwrap();
    snapshot.trace.mode = TraceMode::Off;
    assert!(Game::restore(prepared(), &canonical(&snapshot).unwrap()).is_err());
    let mut snapshot: Snapshot = strict_read(&original).unwrap();
    if let EvidenceRecord::Batch { batch } = &mut snapshot.trace.records[2].record {
        batch.before.objects[0].tapped = true;
    } else {
        panic!("expected mana batch")
    };
    assert!(Game::restore(prepared(), &canonical(&snapshot).unwrap()).is_err());
}
#[test]
fn departures_are_one_simultaneous_removal_batch_in_every_trace_mode() {
    let p = Prepared::admit(
        prepared().catalog.as_ref().clone(),
        ScenarioConfig::commander_foundation(),
    )
    .unwrap();
    let mut full = Game::with_trace(p.clone(), TraceMode::Detailed);
    for player in [0, 2, 3] {
        full.concede(PlayerId(player)).unwrap();
    }
    let journal = full.replay_journal();
    let _ = full.advance(Budget::unlimited());
    let removal = batches(&full)[0];
    assert!(matches!(removal.group, EventGroup::Simultaneous { .. }));
    assert_eq!(removal.occurrences.len(), 3);
    assert!(removal.occurrences.iter().all(|r| matches!(
        r.actual,
        EventOutcome::LeftGame {
            owner: PlayerId(0),
            ..
        }
    )));
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        let game = Game::replay_with_trace(p.clone(), &journal, mode).unwrap();
        assert_eq!(game.state, full.state);
        let restored = Game::restore(p.clone(), &game.snapshot().unwrap()).unwrap();
        assert_eq!(restored.trace, game.trace);
        if mode != TraceMode::Off {
            assert_eq!(game.trace.verification, full.trace.verification);
        }
    }
}

#[test]
fn mixed_input_trace_modes_restore_every_pending_and_suspended_departure_boundary() {
    let witness: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/expected-payment-scenario.json"
    ))
    .unwrap();
    let journal: ReplayJournal =
        serde_json::from_value(witness["departure_journal"].clone()).unwrap();
    let p = Prepared::admit(
        prepared().catalog.as_ref().clone(),
        ScenarioConfig::commander_foundation(),
    )
    .unwrap();
    // Reviewed rule boundaries: mana, cast/targets, mana-window completion,
    // payment, passes, three
    // external arrivals, illegal-target completion, and the final survivor.
    for (boundary, decisions, concessions) in [
        (0, 0, 0),
        (0, 1, 0),
        (1, 1, 0),
        (1, 2, 0),
        (2, 2, 0),
        (2, 3, 0),
        (3, 3, 0),
        (3, 4, 0),
        (4, 4, 0),
        (5, 4, 0),
        (5, 5, 0),
        (6, 5, 0),
        (6, 5, 1),
        (7, 5, 1),
        (7, 6, 1),
        (8, 6, 1),
        (8, 6, 2),
        (9, 6, 2),
        (10, 6, 2),
        (10, 6, 3),
        (11, 6, 3),
    ] {
        let prefix = ReplayJournal {
            decisions: journal.decisions[..decisions].to_vec(),
            concessions: journal.concessions[..concessions].to_vec(),
        };
        let full =
            Game::replay_to(p.clone(), &prefix, Some(boundary), TraceMode::Detailed).unwrap();
        assert_eq!(full.state.rule_steps, boundary);
        for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
            let game = Game::replay_to(p.clone(), &prefix, Some(boundary), mode).unwrap();
            assert_eq!(game.state, full.state);
            let restored = Game::restore(p.clone(), &game.snapshot().unwrap()).unwrap();
            assert_eq!(restored.trace, game.trace);
            if mode != TraceMode::Off {
                assert_eq!(game.trace.verification, full.trace.verification);
            }
        }
    }
}
