//! Private seeded 100-card setup, paid Ponder, no profile/consumer admission.
//! CR401.4/608.2c/701.24/903.9b, frozen20260925.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
const PONDER: &str =
    include_str!("../../../../tests/fixtures/02090581-61aa-4348-ad57-451be8ee91c2.json");
fn seed() -> [u8; 32] {
    // Discovery only. Actual tests run the real setup and verify the opening.
    static SEED: std::sync::OnceLock<[u8; 32]> = std::sync::OnceLock::new();
    *SEED.get_or_init(|| {
        for byte in 0..=u8::MAX {
            let mut random = MatchRandom {
                version: 1,
                seed: [byte; 32],
                stream: 0,
                word_position: RngPosition(0),
            };
            let mut cards = (1..100).collect::<Vec<u32>>();
            for i in (1..99).rev() {
                loop {
                    if let Some(j) =
                        setup::sample_shuffle_word(random.word().unwrap(), i as u32 + 1)
                    {
                        cards.swap(i, j as usize);
                        break;
                    }
                }
            }
            if cards[..7].contains(&1) {
                return [byte; 32];
            }
        }
        panic!("seed discovery")
    })
}
fn prepared() -> Prepared {
    let mut p = prepared_with_sources(&[], &[PONDER]);
    let commander = p
        .catalog
        .definitions
        .iter()
        .find(|d| d.name == "Caelorna, Coral Tyrant")
        .unwrap()
        .oracle_id
        .clone();
    let names = [
        "Ponder",
        "Storm Crow",
        "Counterspell",
        "Unsummon",
        "Swan Song",
    ];
    p.config.profile = "controller-library-order-private-seeded".into();
    p.config.mode = Mode::CommanderMatch;
    p.config.initial.turn_step = TurnStep::Untap;
    p.config.initial.libraries = None;
    p.config.cards.clear();
    p.config.commanders.clear();
    for seat in 0..4 {
        let owner = PlayerId(seat);
        let id = CardId(u32::from(seat) * 100);
        p.config.commanders.push(id);
        p.config.cards.push(CardSetup {
            id,
            oracle_id: commander.clone(),
            owner,
            controller: owner,
            zone: Zone::Command { owner },
        });
        for i in 1..100 {
            let name = if seat == 0 && i <= 5 {
                names[i as usize - 1]
            } else {
                "Island"
            };
            let oracle = p
                .catalog
                .definitions
                .iter()
                .find(|d| d.name == name)
                .unwrap()
                .oracle_id
                .clone();
            p.config.cards.push(CardSetup {
                id: CardId(id.0 + i),
                oracle_id: oracle,
                owner,
                controller: owner,
                zone: Zone::Library { owner },
            });
        }
    }
    let decks = std::array::from_fn(|seat| {
        let mut counts = BTreeMap::<String, u32>::new();
        for c in p
            .config
            .cards
            .iter()
            .filter(|c| c.owner == PlayerId(seat as u8))
        {
            *counts.entry(c.oracle_id.clone()).or_default() += 1;
        }
        Decklist {
            commander: commander.clone(),
            cards: counts
                .into_iter()
                .map(|(oracle_id, count)| DeckEntry { oracle_id, count })
                .collect(),
        }
    });
    p.match_config = Some(MatchConfig {
        profile: p.config.profile.clone(),
        decks,
        starting_player: PlayerId(0),
        seed: seed(),
    });
    p.profile_digest = digest(&(&p.config, &p.match_config)).unwrap();
    p
}
fn library(g: &Game, seat: u8) -> Vec<ObjectId> {
    g.state
        .rules
        .libraries
        .get(&PlayerId(seat))
        .into_iter()
        .flatten()
        .map(|c| g.object_id(*c).unwrap())
        .collect()
}
fn develop(g: &mut Game, q: u64) -> CardId {
    for _ in 0..1000 {
        let r = request(g, q);
        if matches!(r, Request::Mulligan { .. }) {
            g.respond(
                r.player(),
                r.decision_id(),
                Response::Mulligan { take: false },
            )
            .unwrap();
            continue;
        }
        if matches!(
            r,
            Request::Priority {
                player: PlayerId(0),
                ..
            }
        ) && g.active_player() == Some(PlayerId(0))
            && matches!(
                g.turn().step,
                TurnStep::PrecombatMain | TurnStep::PostcombatMain
            )
        {
            assert_eq!(
                g.current(object(g, 1)).unwrap().zone,
                Zone::Hand { owner: PlayerId(0) },
                "real seeded opening"
            );
            if !g
                .state
                .rules
                .objects
                .values()
                .any(|o| o.zone == Zone::Battlefield && o.controller == PlayerId(0))
            {
                let land = g
                    .state
                    .rules
                    .objects
                    .values()
                    .find(|o| {
                        o.zone == (Zone::Hand { owner: PlayerId(0) })
                            && g.definition(o.id.card).unwrap().name == "Island"
                    })
                    .unwrap()
                    .id
                    .card;
                g.respond(
                    r.player(),
                    r.decision_id(),
                    Response::PlayLand { card: land },
                )
                .unwrap();
                continue;
            }
            recover(g);
            return CardId(1);
        }
        answer(g, q, pass(&r));
    }
    panic!("develop bound")
}
fn paid(g: &mut Game, q: u64) -> Request {
    let card = develop(g, q);
    fund(g, q, "Island", 1);
    request(g, q);
    announce(g, q, card.0, vec![]);
    assert_eq!(
        g.state
            .rules
            .stack
            .last()
            .unwrap()
            .paid
            .as_ref()
            .unwrap()
            .colored,
        vec![Color::U]
    );
    for _ in 0..1000 {
        let r = request(g, q);
        if matches!(r, Request::LibraryOrder { .. }) {
            recover(g);
            return r;
        }
        assert!(!g.state.rules.stack.is_empty());
        answer(g, q, pass(&r));
    }
    panic!("order bound")
}
fn choose_order(g: &mut Game, r: &Request, objects: Vec<ObjectId>) {
    g.respond(
        r.player(),
        r.decision_id(),
        Response::LibraryOrder { objects },
    )
    .unwrap();
    recover(g);
}
fn optional(g: &mut Game, q: u64) -> Request {
    let r = request(g, q);
    assert!(matches!(r, Request::OptionalEffect { .. }));
    recover(g);
    r
}
fn choose_shuffle(g: &mut Game, r: &Request, perform: bool) {
    g.respond(
        r.player(),
        r.decision_id(),
        Response::OptionalEffect { perform },
    )
    .unwrap();
    recover(g);
}
fn finish(g: &mut Game, q: u64) {
    for _ in 0..1000 {
        let r = request(g, q);
        if g.state.rules.stack.is_empty() && matches!(r, Request::Priority { .. }) {
            recover(g);
            return;
        }
        answer(g, q, pass(&r));
    }
    panic!("finish bound")
}
#[test]
fn library_order_paid_ponder_all_permutations_keep_shuffle_and_draw() {
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        for perform in [false, true] {
            both(prepared(), |g, q| {
                let r = paid(g, q);
                let Request::LibraryOrder {
                    candidates, count, ..
                } = &r
                else {
                    panic!("order")
                };
                assert_eq!(*count, 3);
                let before = library(g, 0);
                let other = library(g, 1);
                let random = g.state.rules.match_state.as_ref().unwrap().random.clone();
                let ordered = order.iter().map(|i| candidates[*i]).collect::<Vec<_>>();
                choose_order(g, &r, ordered.clone());
                let r = optional(g, q);
                let mut expected = ordered.clone();
                expected.extend_from_slice(&before[3..]);
                assert_eq!(library(g, 0), expected);
                for o in &ordered {
                    assert_eq!(g.current(*o).unwrap().id, *o);
                }
                choose_shuffle(g, &r, perform);
                finish(g, q);
                let state = g.state.rules.match_state.as_ref().unwrap();
                assert_eq!(state.library_shuffles[0], u64::from(perform));
                assert_eq!(library(g, 1), other);
                let after = library(g, 0);
                assert_eq!(after.len(), before.len() - 1);
                if !perform {
                    assert_eq!(after, expected[1..]);
                    let drawn = object(g, ordered[0].card.0);
                    assert_ne!(drawn, ordered[0]);
                    assert_eq!(
                        g.current(drawn).unwrap().zone,
                        Zone::Hand { owner: PlayerId(0) }
                    );
                    assert_eq!(state.random, random);
                } else {
                    assert!(
                        state.random.word_position.0
                            >= random.word_position.0 + before.len() as u128 - 1
                    );
                    let remaining = after.iter().map(|o| o.card).collect::<BTreeSet<_>>();
                    let original = before.iter().map(|o| o.card).collect::<BTreeSet<_>>();
                    let missing = original.difference(&remaining).copied().collect::<Vec<_>>();
                    assert_eq!(missing.len(), 1);
                    assert_eq!(
                        g.current(object(g, missing[0].0)).unwrap().zone,
                        Zone::Hand { owner: PlayerId(0) }
                    );
                }
                if g.trace.mode == TraceMode::Detailed {
                    let b = batches(g);
                    assert_eq!(
                        b.iter()
                            .flat_map(|b| &b.occurrences)
                            .filter(|o| matches!(o.actual, EventOutcome::LibraryOrdered { .. }))
                            .count(),
                        1
                    );
                }
            });
        }
    }
}
#[test]
fn library_order_private_queries_atomic_inputs_work_receipts_and_rehashed_forgeries() {
    both(prepared(), |g, q| {
        let r = paid(g, q);
        let Request::LibraryOrder { candidates, .. } = &r else {
            panic!("order")
        };
        let state = g.state.clone();
        let trace = g.trace.clone();
        let QueryValue::Decision(Some(PlayerRequest::LibraryOrder {
            token,
            candidates: handles,
            count,
            ..
        })) = finish_query(g, g.player_decision_for(PlayerId(0), Budget::new(0)))
        else {
            panic!("public order")
        };
        assert_eq!(count, 3);
        assert_eq!(
            handles,
            candidates
                .iter()
                .map(|o| g.handle_for(PlayerId(0), *o).unwrap())
                .collect::<Vec<_>>()
        );
        for seat in 1..4 {
            let QueryValue::Observation(o) =
                finish_query(g, g.observe(PlayerId(seat), Budget::new(0)))
            else {
                panic!("view")
            };
            assert!(o.decision.is_none());
            assert!(
                !o.view
                    .objects
                    .iter()
                    .any(|o| matches!(o.zone, Zone::Library { .. }))
            );
        }
        for objects in [
            vec![],
            vec![candidates[0]; 3],
            vec![library(g, 1)[0], candidates[1], candidates[2]],
        ] {
            assert!(
                g.respond(
                    r.player(),
                    r.decision_id(),
                    Response::LibraryOrder { objects }
                )
                .is_err()
            );
            assert_eq!(g.state, state);
            assert_eq!(g.trace, trace);
        }
        let QueryResult::Incomplete { work, .. } =
            g.player_decision_for(PlayerId(0), Budget::new(2))
        else {
            panic!("work")
        };
        let saved = g.save_work(&work).unwrap();
        let work = g.restore_work(&saved).unwrap();
        assert!(matches!(
            finish_query(g, g.resume_work(work, Budget::unlimited())),
            QueryValue::Decision(Some(PlayerRequest::LibraryOrder { count: 3, .. }))
        ));
        for variant in 0..2 {
            let mut bad: Snapshot = strict_read(&g.snapshot().unwrap()).unwrap();
            if variant == 0 {
                let Frame::ResolutionChoice {
                    choice: ResolutionChoice::LibraryOrder { looked, .. },
                    ..
                } = &mut bad.state.rules.frame
                else {
                    panic!("frame")
                };
                looked.reverse();
                bad.state.rules.libraries.get_mut(&PlayerId(0)).unwrap()[..3].reverse();
            } else {
                bad.state
                    .rules
                    .match_state
                    .as_mut()
                    .unwrap()
                    .random
                    .word_position
                    .0 += 1;
            }
            bad.state_sha256 = digest(&bad.state).unwrap();
            assert!(Game::restore(g.prepared.as_ref().clone(), &canonical(&bad).unwrap()).is_err());
        }
        let response = PlayerResponse::LibraryOrder {
            objects: handles.iter().rev().cloned().collect(),
        };
        let QueryValue::Validated(receipt) = finish_query(
            g,
            g.check_player_response(PlayerId(0), token.clone(), response.clone(), Budget::new(0)),
        ) else {
            panic!("receipt")
        };
        let QueryValue::Validated(duplicate) = finish_query(
            g,
            g.check_player_response(PlayerId(0), token, response, Budget::new(0)),
        ) else {
            panic!("duplicate")
        };
        assert_eq!(g.state, state);
        g.submit(receipt).unwrap();
        assert!(g.submit(duplicate).is_err());
        assert!(g.restore_work(&saved).is_err());
        let r = optional(g, q);
        choose_shuffle(g, &r, false);
        finish(g, q);
        for h in handles {
            assert!(g.resolve_handle(PlayerId(0), &h).is_err());
        }
    });
}
#[test]
fn library_order_departures_consume_accepted_order_and_optional_inputs() {
    for stage in 0..6 {
        both(prepared(), |g, q| {
            let r = paid(g, q);
            let Request::LibraryOrder { candidates, .. } = &r else {
                panic!("order")
            };
            if stage > 0 {
                choose_order(g, &r, candidates.clone());
            }
            if stage > 1 {
                let r = optional(g, q);
                if stage > 2 {
                    choose_shuffle(g, &r, stage != 3);
                }
            }
            if stage >= 4 {
                assert!(matches!(
                    g.advance(Budget::new(1)),
                    Advance::Suspended { .. }
                ));
                assert!(matches!(
                    g.state.rules.frame,
                    Frame::ResolutionChoice {
                        choice: ResolutionChoice::Shuffle { .. },
                        ..
                    }
                ));
            }
            if stage == 5 {
                assert!(matches!(
                    g.advance(Budget::new(1)),
                    Advance::Suspended { .. }
                ));
                recover(g);
            }
            g.concede(PlayerId(0)).unwrap();
            recover(g);
            finish(g, q);
            assert!(!g.is_live(PlayerId(0)));
            assert!(g.state.rules.pending.is_none());
        });
    }
}
#[test]
fn library_order_commander_draw_replacement_keeps_next_card() {
    // Explicit historical tucked-commander scenario; no seeded shuffle is requested.
    for to_command in [false, true] {
        let mut p = prepared_with_sources(
            &[("Ponder", 0, 0, true), ("Island", 0, 0, false)],
            &[PONDER],
        );
        let id = p.config.initial_libraries()[&PlayerId(0)][2];
        p.config
            .cards
            .iter_mut()
            .find(|c| c.id == id)
            .unwrap()
            .oracle_id = p
            .catalog
            .definitions
            .iter()
            .find(|d| d.name == "Caelorna, Coral Tyrant")
            .unwrap()
            .oracle_id
            .clone();
        p.config.commanders = vec![id];
        p.profile_digest = digest(&p.config).unwrap();
        both(p, |g, q| {
            fund(g, q, "Island", 1);
            request(g, q);
            announce(g, q, 0, vec![]);
            let r = loop {
                let r = request(g, q);
                if matches!(r, Request::LibraryOrder { .. }) {
                    break r;
                }
                answer(g, q, pass(&r));
            };
            let Request::LibraryOrder { candidates, .. } = &r else {
                panic!("order")
            };
            let mut ordered = candidates.clone();
            ordered.rotate_right(1);
            assert_eq!(ordered[0].card, id);
            choose_order(g, &r, ordered.clone());
            let r = optional(g, q);
            choose_shuffle(g, &r, false);
            let r = request(g, q);
            assert!(matches!(r, Request::CommanderReturn { .. }));
            recover(g);
            g.respond(
                r.player(),
                r.decision_id(),
                Response::CommanderReturn { to_command },
            )
            .unwrap();
            finish(g, q);
            assert_eq!(library(g, 0)[0], ordered[1]);
            assert_eq!(
                g.current(object(g, id.0)).unwrap().zone,
                if to_command {
                    Zone::Command { owner: PlayerId(0) }
                } else {
                    Zone::Hand { owner: PlayerId(0) }
                }
            );
        });
    }
}
#[test]
fn library_order_typed_zero_huge_and_empty_parameter_paths() {
    for amount in ["0", "99999999999999999999999"] {
        for perform in [false, true] {
            let mut p = prepared();
            let c = Arc::make_mut(&mut p.catalog);
            let d = c
                .definitions
                .iter_mut()
                .find(|d| d.name == "Ponder")
                .unwrap();
            if let Instruction::OrderLibraryTop { amount: a, .. } =
                &mut d.spell.as_mut().unwrap().instructions[0]
            {
                *a = amount.parse().unwrap();
            }
            c.validate().unwrap();
            p.catalog_digest = digest(c).unwrap();
            both(p, |g, q| {
                let card = develop(g, q);
                fund(g, q, "Island", 1);
                request(g, q);
                announce(g, q, card.0, vec![]);
                let mut r = request(g, q);
                while matches!(r, Request::Priority { .. }) && !g.state.rules.stack.is_empty() {
                    answer(g, q, pass(&r));
                    r = request(g, q);
                }
                if amount != "0" {
                    let Request::LibraryOrder { candidates, .. } = &r else {
                        panic!("order")
                    };
                    assert_eq!(candidates, &library(g, 0));
                    choose_order(g, &r, candidates.iter().rev().copied().collect());
                    r = optional(g, q);
                }
                assert!(matches!(r, Request::OptionalEffect { .. }));
                choose_shuffle(g, &r, perform);
                finish(g, q);
                assert_eq!(
                    g.state.rules.match_state.as_ref().unwrap().library_shuffles[0],
                    u64::from(perform)
                );
            });
        }
    }
    // Empty historical scenario: ordering/shuffle-decline complete, mandatory draw fails.
    let mut p = prepared_with_sources(
        &[("Ponder", 0, 0, true), ("Island", 0, 0, false)],
        &[PONDER],
    );
    p.config
        .cards
        .retain(|c| c.zone != (Zone::Library { owner: PlayerId(0) }));
    p.config.initial.libraries = None;
    p.profile_digest = digest(&p.config).unwrap();
    both(p, |g, q| {
        fund(g, q, "Island", 1);
        request(g, q);
        announce(g, q, 0, vec![]);
        let r = loop {
            let r = request(g, q);
            if matches!(r, Request::OptionalEffect { .. }) {
                break r;
            }
            answer(g, q, pass(&r));
        };
        choose_shuffle(g, &r, false);
        finish(g, q);
        assert!(!g.is_live(PlayerId(0)));
    });
}
#[test]
fn library_order_late_group_failure_preserves_library_memory_and_trace() {
    both(prepared(), |g, q| {
        let r = paid(g, q);
        let Request::LibraryOrder { candidates, .. } = &r else {
            panic!("order")
        };
        let Frame::ResolutionChoice { frame, .. } = &g.state.rules.frame else {
            panic!("frame")
        };
        let cause = BatchCause::Instruction {
            stack_id: frame.stack_id,
            source: frame.spell.source,
            player: PlayerId(0),
            cursor: frame.cursor,
            source_block: 0,
        };
        let before = g.state.clone();
        let trace = g.trace.clone();
        assert!(
            g.commit_batch(
                cause,
                EventGroup::Ordered {
                    children: vec![
                        EventGroup::Occurrence {
                            proposed: ProposedEvent::OrderLibraryTop {
                                player: PlayerId(0),
                                amount: 3.into(),
                                looked: candidates.clone(),
                                order: candidates.iter().rev().copied().collect()
                            }
                        },
                        EventGroup::Occurrence {
                            proposed: ProposedEvent::Mill {
                                player: PlayerId(0),
                                count: (-1).into(),
                                cards: vec![]
                            }
                        }
                    ]
                }
            )
            .is_err()
        );
        assert_eq!(g.state, before);
        assert_eq!(g.trace, trace);
        choose_order(g, &r, candidates.clone());
        let r = optional(g, q);
        choose_shuffle(g, &r, false);
        finish(g, q);
    });
}
