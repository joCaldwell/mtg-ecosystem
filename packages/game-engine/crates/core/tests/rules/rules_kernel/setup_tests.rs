use super::*;
fn prepared() -> Prepared {
    Prepared::admit_pod(
        FixtureCatalog::Match.get().clone(),
        MatchConfig::for_profile("commander-foundations-v1", [0; 32], PlayerId(2)).unwrap(),
    )
    .unwrap()
}
fn answer(g: &mut Game, r: Decision<Rules>, response: Answer) {
    g.respond(r.player, r.id, response).unwrap();
}
fn take_round(g: &mut Game, take: bool) {
    for _ in 0..4 {
        let r = Driver::request(g, u64::MAX);
        assert!(matches!(
            r,
            Decision {
                reason: Reason::Mulligan { .. },
                ..
            }
        ));
        answer(
            g,
            r,
            g.next_test_decision().select(Candidate::Flag {
                flag: if take { Flag::Mulligan } else { Flag::Keep },
            }),
        );
    }
}
fn hand(g: &Game, p: PlayerId) -> Vec<ObjectId> {
    g.state
        .rules
        .objects
        .values()
        .filter(|o| o.zone == (Zone::Hand { owner: p }))
        .map(|o| o.id)
        .collect()
}
#[test]
fn chacha_official_vectors_cursor_and_exhaustion_are_exact() {
    // rand_chacha0.10.0's independently cited IETF test vectors1/2:
    // https://datatracker.ietf.org/doc/html/draft-nir-cfrg-chacha20-poly1305-04
    let mut random = MatchRandom {
        version: 1,
        seed: [0; 32],
        stream: 0,
        word_position: RngPosition(0),
    };
    let expected = [
        0xade0b876, 0x903df1a0, 0xe56a5d40, 0x28bd8653, 0xb819d2bd, 0x1aed8da0, 0xccef36a8,
        0xc70d778b, 0x7c5941da, 0x8d485751, 0x3fe02477, 0x374ad8b8, 0xf4b8436a, 0x1ca11815,
        0x69b687c3, 0x8665eeb2,
    ];
    for value in expected {
        assert_eq!(random.word().unwrap(), value);
    }
    assert_eq!(random.word_position, RngPosition(16));
    assert_eq!(random.word().unwrap(), 0xbee7079f);
    let json = canonical(&random).unwrap();
    let mut copy: MatchRandom = strict_read(&json).unwrap();
    assert_eq!(copy.word().unwrap(), random.word().unwrap());
    random.word_position = RngPosition((1u128 << 68) - 1);
    random.word().unwrap();
    assert_eq!(random.word_position, RngPosition(1u128 << 68));
    let before = random.clone();
    assert!(random.word().is_err());
    assert_eq!(random, before);
    for bad in ["01", "-1", "295147905179352825857"] {
        assert!(strict_read::<RngPosition>(&format!("\"{bad}\"")).is_err());
    }
    assert_eq!(sample_shuffle_word(u32::MAX, 3), None);
    assert_eq!(sample_shuffle_word(u32::MAX - 1, 3), Some(2));
    assert_eq!(sample_shuffle_word(u32::MAX, 2), Some(1));
}
#[test]
fn setup_is_budget_partition_invariant_and_automatic_queries_reveal_nothing() {
    let p = prepared();
    let mut a = Game::new(p.clone());
    let mut b = a.fork();
    assert!(matches!(
        a.observe(PlayerId(0), Budget::unlimited()),
        QueryResult::Rejected(QueryRejection::UnavailableDuringSetup)
    ));
    assert!(
        a.state
            .knowledge
            .visibility
            .iter()
            .all(|k| k.history.is_empty())
    );
    let req = Driver::request(&mut a, u64::MAX);
    let mut steps = 0;
    loop {
        match b.advance(Budget::new(1)) {
            Advance::AwaitingDecision(r) => {
                assert_eq!(r, req);
                break;
            }
            Advance::Suspended { .. } => {
                steps += 1;
                assert!(matches!(
                    b.observe(PlayerId(0), Budget::new(1)),
                    QueryResult::Rejected(QueryRejection::UnavailableDuringSetup)
                ));
                assert!(
                    b.state
                        .knowledge
                        .visibility
                        .iter()
                        .all(|k| k.history.is_empty())
                );
            }
            x => panic!("{x:?}"),
        }
    }
    assert!(steps >= 419);
    assert_eq!(a.state, b.state);
    assert_eq!(a.trace, b.trace);
    assert_eq!(req.player, PlayerId(2));
    for seat in 0..4 {
        assert_eq!(hand(&a, PlayerId(seat)).len(), 7);
        assert_eq!(a.state.rules.libraries[&PlayerId(seat)].len(), 92);
    }
    let restored = Game::restore(p, &a.snapshot().unwrap()).unwrap();
    assert_eq!(restored.state, a.state);
}
#[test]
fn declarations_wait_for_all_free_paid_and_max_mulligans_use_ordered_bottoms() {
    let mut g = Game::new(prepared());
    let first = Driver::request(&mut g, u64::MAX);
    let old = g.state.rules.libraries.clone();
    let random = g.state.rules.match_state.as_ref().unwrap().random.clone();
    let original = hand(&g, PlayerId(2));
    {
        let selected_answer = g.next_test_decision().select(Candidate::Flag {
            flag: Flag::Mulligan,
        });
        answer(&mut g, first, selected_answer)
    };
    let next = Driver::request(&mut g, u64::MAX);
    assert_eq!(g.state.rules.libraries, old);
    assert_eq!(g.state.rules.match_state.as_ref().unwrap().random, random);
    assert_eq!(hand(&g, PlayerId(2)), original);
    assert_eq!(next.player, PlayerId(3));
    {
        let selected_answer = g.next_test_decision().select(Candidate::Flag {
            flag: Flag::Mulligan,
        });
        answer(&mut g, next, selected_answer)
    };
    for _ in 0..2 {
        let r = Driver::request(&mut g, u64::MAX);
        {
            let selected_answer = g.next_test_decision().select(Candidate::Flag {
                flag: Flag::Mulligan,
            });
            answer(&mut g, r, selected_answer)
        };
    }
    for taken in 1..=8 {
        let r = Driver::request(&mut g, u64::MAX);
        if taken == 1 {
            assert!(matches!(
                r,
                Decision {
                    reason: Reason::Mulligan {
                        mulligans_taken: 1,
                        bottom_if_mulligan: 1,
                        ..
                    },
                    ..
                }
            ));
            {
                let selected_answer = g.next_test_decision().select(Candidate::Flag {
                    flag: Flag::Mulligan,
                });
                answer(&mut g, r, selected_answer)
            };
            for _ in 0..3 {
                let r = Driver::request(&mut g, u64::MAX);
                {
                    let selected_answer = g.next_test_decision().select(Candidate::Flag {
                        flag: Flag::Mulligan,
                    });
                    answer(&mut g, r, selected_answer)
                };
            }
        } else {
            for i in 0..4 {
                let r = if i == 0 {
                    r.clone()
                } else {
                    Driver::request(&mut g, u64::MAX)
                };
                let found @ Decision {
                    reason: Reason::BottomOpening,
                    ..
                } = &r
                else {
                    panic!("{r:?}")
                };
                let count = &found.required_count();
                let candidates = found.objects_offered();
                assert_eq!(*count, (taken - 1) as usize);
                let objects = candidates
                    .iter()
                    .rev()
                    .take(*count)
                    .copied()
                    .collect::<Vec<_>>();
                let player = r.player;
                {
                    let selected_answer = g.next_test_decision().order_objects(objects.clone());
                    answer(&mut g, r, selected_answer)
                };
                let _ = g.advance(Budget::new(1));
                let lib = &g.state.rules.libraries[&player];
                assert_eq!(
                    &lib[lib.len() - objects.len()..],
                    objects
                        .iter()
                        .map(|o| o.card)
                        .collect::<Vec<_>>()
                        .as_slice()
                );
                assert_eq!(hand(&g, player).len(), 8 - taken as usize);
            }
            if taken < 8 {
                take_round(&mut g, true);
            }
        }
    }
    let r = Driver::request(&mut g, u64::MAX);
    assert!(matches!(
        r,
        Decision {
            reason: Reason::Priority,
            ..
        }
    ));
    assert!(g.state.rules.match_state.as_ref().unwrap().setup.is_none());
    for seat in 0..4 {
        assert!(hand(&g, PlayerId(seat)).is_empty());
        assert_eq!(g.state.rules.libraries[&PlayerId(seat)].len(), 99);
    }
}
#[test]
fn first_player_draws_and_seed_order_are_trusted_only() {
    let mut g = Game::new(prepared());
    take_round(&mut g, false);
    let r = Driver::request(&mut g, u64::MAX);
    assert!(matches!(
        r,
        Decision {
            player: PlayerId(2),
            reason: Reason::Priority,
            ..
        }
    ));
    assert_eq!(g.turn().step, TurnStep::Upkeep);
    for _ in 0..4 {
        let r = Driver::request(&mut g, u64::MAX);
        {
            let selected_answer = Answer::Pick {
                picks: vec![vec![0]],
            };
            answer(&mut g, r, selected_answer)
        };
    }
    let _ = Driver::request(&mut g, u64::MAX);
    assert_eq!(g.turn().step, TurnStep::Draw);
    assert_eq!(hand(&g, PlayerId(2)).len(), 8);
    let QueryResult::Complete(QueryValue::Observation(o)) =
        g.observe(PlayerId(0), Budget::unlimited())
    else {
        panic!("observation")
    };
    let text = canonical(&o).unwrap();
    assert!(!text.contains("word_position"));
    assert!(!text.contains("seed"));
    assert_eq!(
        o.view
            .objects
            .iter()
            .filter(|o| matches!(o.zone,Zone::Hand{owner} if owner==PlayerId(2)))
            .count(),
        0
    );
}
#[test]
fn opening_bottom_validation_is_budgeted_recoverable_and_executor_scoped() {
    let p = prepared();
    let mut g = Game::new(p.clone());
    take_round(&mut g, true);
    take_round(&mut g, true);
    let r = Driver::request(&mut g, u64::MAX);
    let found @ Decision {
        reason: Reason::BottomOpening,
        ..
    } = r
    else {
        panic!("bottom")
    };
    let player = found.player;
    let count = found.required_count();
    assert_eq!(count, 1);
    let before = g.state_digest().unwrap();
    let q = g.player_decision_for(player, Budget::new(1));
    let QueryResult::Incomplete { work, .. } = q else {
        panic!("budgeted decision")
    };
    let saved = g.save_work(&work).unwrap();
    let restored = Game::restore(p, &g.snapshot().unwrap()).unwrap();
    let work = restored.restore_work(&saved).unwrap();
    let QueryResult::Complete(QueryValue::Decision(Some(
        found @ Decision {
            reason: Reason::BottomOpening,
            ..
        },
    ))) = restored.resume_work(work, Budget::unlimited())
    else {
        panic!("restored bottom")
    };
    let token = found.id.clone();
    let candidates = found.objects_offered();
    assert_eq!(candidates.len(), 7);
    let response = g
        .next_test_viewer_decision()
        .order_objects(vec![candidates[0].clone()]);
    let QueryResult::Incomplete { work, .. } =
        restored.check_player_response(player, token, response, Budget::new(1))
    else {
        panic!("answer validation budget")
    };
    let saved = restored.save_work(&work).unwrap();
    let work = restored.restore_work(&saved).unwrap();
    let QueryResult::Complete(QueryValue::Validated(receipt)) =
        restored.resume_work(work, Budget::unlimited())
    else {
        panic!("receipt")
    };
    assert!(g.submit(receipt).is_err());
    assert_eq!(before, g.state_digest().unwrap());
}
#[test]
fn setup_atomic_events_reject_wrong_phase_and_rehashed_rng_queue_order_forgeries() {
    let p = prepared();
    let mut g = Game::new(p.clone());
    let before = g.state.clone();
    let trace = g.trace.clone();
    assert!(
        g.commit(
            ProposedEvent::Setup {
                action: SetupAction::ShuffleWord {
                    player: PlayerId(0),
                    index: 98
                }
            },
            BatchCause::Setup
        )
        .is_err()
    );
    assert_eq!(g.state, before);
    assert_eq!(g.trace, trace);
    let _ = g.advance(Budget::new(3));
    let json = g.snapshot().unwrap();
    for path in 0..6 {
        let mut raw: serde_json::Value = serde_json::from_str(&json).unwrap();
        match path {
            0 => raw["state"]["match_state"]["random"]["seed"][0] = 1.into(),
            1 => raw["state"]["match_state"]["random"]["word_position"] = "4".into(),
            2 => raw["state"]["match_state"]["setup"]["phase"]["index"] = 94.into(),
            3 => {
                raw["state"]["match_state"]["setup"]["phase"]["players"] =
                    serde_json::json!([3, 2, 0, 1])
            }
            4 => {
                let a = raw["state"]["libraries"]["2"][0].clone();
                raw["state"]["libraries"]["2"][0] = raw["state"]["libraries"]["2"][1].clone();
                raw["state"]["libraries"]["2"][1] = a;
            }
            _ => raw["state"]["match_state"]["setup"]["players"][0]["mulligans_taken"] = 1.into(),
        }
        raw["state_sha256"] = digest(&raw["state"]).unwrap().into();
        assert!(strict_read::<Snapshot>(&canonical(&raw).unwrap()).is_ok());
        assert!(
            Game::restore(p.clone(), &canonical(&raw).unwrap()).is_err(),
            "mutant{path}"
        );
    }
}
#[test]
fn large_match_groups_and_multiple_fresh_commanders_remain_atomic() {
    let mut g = Game::new(prepared());
    take_round(&mut g, false);
    let _ = Driver::request(&mut g, u64::MAX);
    // Domain setup: all99 library cards of one seat enter battlefield. This is
    // not a production setup claim; it verifies match group capacity and checked
    // aggregate atomicity for full100-card departures/untaps.
    let ids = g
        .state
        .rules
        .objects
        .values()
        .filter(|o| {
            g.state.rules.cards[&o.id.card].owner == PlayerId(0)
                && matches!(o.zone, Zone::Library { .. })
        })
        .map(|o| o.id)
        .collect::<Vec<_>>();
    for id in &ids {
        let o = g.state.rules.objects.get_mut(&id.card).unwrap();
        o.zone = Zone::Battlefield;
        o.tapped = true;
    }
    g.state
        .rules
        .libraries
        .get_mut(&PlayerId(0))
        .unwrap()
        .clear();
    let events = ids
        .iter()
        .map(|object| ProposedEvent::Untap { object: *object })
        .collect::<Vec<_>>();
    g.commit_batch(
        BatchCause::UntapStep,
        EventGroup::Simultaneous {
            occurrences: events,
        },
    )
    .unwrap();
    assert!(ids.iter().all(|id| !g.current(*id).unwrap().tapped));
    for card in [CardId(0), CardId(200)] {
        let id = g.object_id(card).unwrap();
        g.move_object_for(
            id,
            Zone::Graveyard {
                owner: g.state.rules.cards[&card].owner,
            },
            BatchCause::Fixture,
        )
        .unwrap();
    }
    assert_eq!(g.state.rules.commander_returns.len(), 2);
    g.state.rules.frames = vec![Frame::Priority];
    let resume = Frame::Stabilize {
        resume: FrameLink(0),
        cleanup: false,
        changed: false,
    };
    assert!(g.offer_commander_sba(resume).unwrap());
    assert!(g.state.rules.commander_returns.is_empty());
    let r = Driver::request(&mut g, u64::MAX);
    assert_eq!(r.player, PlayerId(2));
    {
        let selected_answer = g.next_test_decision().select(Candidate::Flag {
            flag: Flag::CommandZone,
        });
        answer(&mut g, r, selected_answer)
    };
    let r = Driver::request(&mut g, u64::MAX);
    assert_eq!(r.player, PlayerId(0));
    assert!(matches!(
        g.state.rules.objects[&CardId(200)].zone,
        Zone::Graveyard { .. }
    ));
    let Decision {
        reason:
            Reason::CommanderReturn {
                previous: previous_choices,
                ..
            },
        ..
    } = &r
    else {
        panic!("return")
    };
    assert_eq!(previous_choices.len(), 1);
    assert_eq!(previous_choices[0].0.card, CardId(200));
    assert!(previous_choices[0].1);
    let QueryResult::Complete(QueryValue::Observation(o)) =
        g.observe(PlayerId(0), Budget::unlimited())
    else {
        panic!("choice observation")
    };
    let Some(Decision {
        reason:
            Reason::CommanderReturn {
                previous: previous_choices,
                ..
            },
        ..
    }) = o.decision
    else {
        panic!("visible return")
    };
    assert_eq!(previous_choices[0].owner, PlayerId(2));
    assert!(previous_choices[0].to_command);
    {
        let selected_answer = g.next_test_decision().select(Candidate::Flag {
            flag: Flag::CommandZone,
        });
        answer(&mut g, r, selected_answer)
    };
    let _ = g.advance(Budget::new(1));
    assert_eq!(
        g.state.rules.objects[&CardId(0)].zone,
        Zone::Command { owner: PlayerId(0) }
    );
    assert_eq!(
        g.state.rules.objects[&CardId(200)].zone,
        Zone::Command { owner: PlayerId(2) }
    );
    let batch = g
        .trace
        .records
        .iter()
        .rev()
        .find_map(|record| match &record.record {
            EvidenceRecord::Batch { batch } if batch.cause == BatchCause::CommanderReturn => {
                Some(batch)
            }
            _ => None,
        })
        .unwrap();
    assert!(matches!(&batch.group,EventGroup::Simultaneous{occurrences} if occurrences.len()==2));
}
