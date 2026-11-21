//! Private seeded 100-card setup, paid Brainstorm, no profile/consumer admission.
//! CR401.4/608.2c/701.24/903.9b, frozen20260925.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
const BRAINSTORM: &str =
    include_str!("../../../../../tests/fixtures/36cd2364-d113-47d1-b2c4-b088d9eb88dd.json");
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
    let mut p = prepared_with_sources(&[], &[BRAINSTORM]);
    let commander = p
        .catalog
        .definitions
        .iter()
        .find(|d| d.name == "Caelorna, Coral Tyrant")
        .unwrap()
        .oracle_id
        .clone();
    let names = [
        "Brainstorm",
        "Storm Crow",
        "Counterspell",
        "Unsummon",
        "Swan Song",
    ];
    p.config.profile = "controller-hand-return-private-seeded".into();
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
        if matches!(r, Request::HandReturn { .. }) {
            recover(g);
            return r;
        }
        assert!(!g.state.rules.stack.is_empty());
        answer(g, q, pass(&r));
    }
    panic!("order bound")
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
fn choose(g: &mut Game, r: &Request, objects: Vec<ObjectId>) {
    g.respond(
        r.player(),
        r.decision_id(),
        Response::HandReturn { objects },
    )
    .unwrap();
    recover(g);
}
#[test]
fn hand_return_paid_brainstorm_draws_three_and_returns_old_or_new_cards_in_both_orders() {
    for old in [false, true] {
        for reverse in [false, true] {
            both(prepared(), |g, q| {
                develop(g, q);
                let old_hand = g
                    .action_hand(PlayerId(0))
                    .into_iter()
                    .filter(|o| o.card != CardId(1))
                    .collect::<Vec<_>>();
                let r = paid(g, q);
                let Request::HandReturn {
                    candidates, count, ..
                } = &r
                else {
                    panic!("hand")
                };
                assert_eq!(*count, 2);
                assert_eq!(candidates.len(), old_hand.len() + 3);
                let frame = match &g.state.rules.frame {
                    Frame::ResolutionChoice { frame, .. } => frame,
                    _ => panic!("frame"),
                };
                assert!(
                    matches!(&frame.outcomes[0],InstructionOutcome::DrawAttempts{attempted,failed,..} if *attempted==3.into()&&*failed==0.into())
                );
                let mut selected = if old {
                    old_hand[..2].to_vec()
                } else {
                    candidates
                        .iter()
                        .filter(|o| !old_hand.contains(o))
                        .take(2)
                        .copied()
                        .collect()
                };
                if reverse {
                    selected.reverse()
                }
                let tail = library(g, 0);
                choose(g, &r, selected.clone());
                finish(g, q);
                let top = library(g, 0);
                assert_eq!(
                    top[..2].iter().map(|o| o.card).collect::<Vec<_>>(),
                    selected.iter().map(|o| o.card).collect::<Vec<_>>()
                );
                assert_eq!(&top[2..], tail);
                for (before, after) in selected.iter().zip(&top) {
                    assert_eq!(after.incarnation, before.incarnation + 1)
                }
                assert_eq!(g.action_hand(PlayerId(0)).len(), old_hand.len() + 1);
            });
        }
    }
}
#[test]
fn hand_return_private_query_atomic_invalid_and_stale_receipts() {
    both(prepared(), |g, q| {
        let r = paid(g, q);
        let Request::HandReturn { candidates, .. } = &r else {
            panic!()
        };
        for seat in 1..4 {
            let QueryValue::Observation(obs) =
                finish_query(g, g.observe(PlayerId(seat), Budget::new(0)))
            else {
                panic!("view")
            };
            assert!(obs.decision.is_none());
        }
        let before = g.state_digest().unwrap();
        for objects in [
            vec![],
            vec![candidates[0]],
            vec![candidates[0], candidates[0]],
            vec![candidates[0], object(g, 100)],
        ] {
            assert!(
                g.respond(
                    PlayerId(0),
                    r.decision_id(),
                    Response::HandReturn { objects }
                )
                .is_err()
            );
            assert_eq!(g.state_digest().unwrap(), before)
        }
        let QueryValue::Decision(Some(PlayerRequest::HandReturn {
            token,
            candidates: handles,
            count,
            ..
        })) = finish_query(g, g.player_decision_for(PlayerId(0), Budget::new(0)))
        else {
            panic!("public hand")
        };
        assert_eq!(count, 2);
        assert_eq!(
            handles,
            candidates
                .iter()
                .map(|o| g.handle_for(PlayerId(0), *o).unwrap())
                .collect::<Vec<_>>()
        );
        let QueryResult::Incomplete { work, .. } =
            g.player_decision_for(PlayerId(0), Budget::new(2))
        else {
            panic!("work")
        };
        let saved = g.save_work(&work).unwrap();
        let work = g.restore_work(&saved).unwrap();
        assert!(matches!(
            finish_query(g, g.resume_work(work, Budget::unlimited())),
            QueryValue::Decision(Some(PlayerRequest::HandReturn { count: 2, .. }))
        ));
        let response = PlayerResponse::HandReturn {
            objects: handles[..2].to_vec(),
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
        assert_eq!(g.state_digest().unwrap(), before);
        g.submit(receipt).unwrap();
        recover(g);
        assert!(g.submit(duplicate).is_err());
        assert!(g.restore_work(&saved).is_err());
        let mut bad: Snapshot = strict_read(&g.snapshot().unwrap()).unwrap();
        let Response::HandReturn { objects } =
            &mut bad.state.rules.pending.as_mut().unwrap().response
        else {
            panic!("accepted")
        };
        objects.reverse();
        bad.state_sha256 = digest(&bad.state).unwrap();
        assert!(Game::restore(g.prepared.as_ref().clone(), &canonical(&bad).unwrap()).is_err());

        finish(g, q);
    });
}
#[test]
fn hand_return_actor_departure_before_or_after_accepted_order() {
    for accepted in [false, true] {
        both(prepared(), |g, q| {
            let r = paid(g, q);
            let Request::HandReturn { candidates, .. } = &r else {
                panic!()
            };
            if accepted {
                choose(g, &r, candidates[..2].to_vec())
            }
            g.concede(PlayerId(0)).unwrap();
            recover(g);
            for _ in 0..100 {
                match g.advance(Budget::new(q)) {
                    Advance::Suspended { .. } => recover(g),
                    Advance::AwaitingDecision(request) => {
                        assert_ne!(request.player(), PlayerId(0));
                        break;
                    }
                    Advance::Finished { .. } => break,
                    other => panic!("{other:?}"),
                }
            }
            assert!(g.state.rules.pending.is_none());
        });
    }
}
#[test]
fn hand_return_counted_draw_progress_rehashed_forgery_rejects() {
    let mut g = Game::with_trace(prepared(), TraceMode::Detailed);
    develop(&mut g, 1);
    fund(&mut g, 1, "Island", 1);
    request(&mut g, 1);
    announce(&mut g, 1, 1, vec![]);
    for _ in 0..1000 {
        if matches!(&g.state.rules.frame,Frame::Resolve{frame} if frame.draw_progress.is_some()) {
            let mut bad: Snapshot = strict_read(&g.snapshot().unwrap()).unwrap();
            let Frame::Resolve { frame } = &mut bad.state.rules.frame else {
                panic!("draw")
            };
            let progress = frame.draw_progress.as_mut().unwrap();
            progress.completed = 0.into();
            progress.remaining = 3.into();
            bad.state_sha256 = digest(&bad.state).unwrap();
            assert!(Game::restore(g.prepared.as_ref().clone(), &canonical(&bad).unwrap()).is_err());
            recover(&g);
            return;
        }
        if let Advance::AwaitingDecision(r) = g.advance(Budget::new(1)) {
            g.respond(r.player(), r.decision_id(), pass(&r)).unwrap();
        }
    }
    panic!("draw progress")
}
#[test]
fn hand_return_commander_draw_and_library_replacements_consume_exact_attempts() {
    for case in 0..3 {
        let mut p = prepared_with_sources(
            &[("Brainstorm", 0, 0, true), ("Island", 0, 0, false)],
            &[BRAINSTORM],
        );
        let id = p.config.initial_libraries()[&PlayerId(0)][0];
        let oracle = p
            .catalog
            .definitions
            .iter()
            .find(|d| d.name == "Caelorna, Coral Tyrant")
            .unwrap()
            .oracle_id
            .clone();
        p.config
            .cards
            .iter_mut()
            .find(|c| c.id == id)
            .unwrap()
            .oracle_id = oracle;
        p.config.commanders = vec![id];
        p.profile_digest = digest(&p.config).unwrap();
        both(p, |g, q| {
            fund(g, q, "Island", 1);
            request(g, q);
            announce(g, q, 0, vec![]);
            let r = loop {
                let r = request(g, q);
                if matches!(r, Request::CommanderReturn { .. }) {
                    break r;
                }
                answer(g, q, pass(&r));
            };
            recover(g);
            g.respond(
                r.player(),
                r.decision_id(),
                Response::CommanderReturn {
                    to_command: case == 0,
                },
            )
            .unwrap();
            let r = request(g, q);
            let Request::HandReturn { candidates, .. } = &r else {
                panic!("hand return after commander draw")
            };
            let mut selected = vec![];
            if case != 0 {
                selected.push(g.object_id(id).unwrap());
            }
            selected.extend(
                candidates
                    .iter()
                    .filter(|o| o.card != id)
                    .take(2 - selected.len())
                    .copied(),
            );
            let tail = library(g, 0);
            choose(g, &r, selected.clone());
            if case != 0 {
                let r = request(g, q);
                assert!(matches!(r, Request::CommanderReturn { .. }));
                recover(g);
                g.respond(
                    r.player(),
                    r.decision_id(),
                    Response::CommanderReturn {
                        to_command: case == 2,
                    },
                )
                .unwrap();
            }
            finish(g, q);
            let top = library(g, 0);
            let expected = selected
                .iter()
                .filter(|o| !(case == 2 && o.card == id))
                .map(|o| o.card)
                .collect::<Vec<_>>();
            assert_eq!(
                top[..expected.len()]
                    .iter()
                    .map(|o| o.card)
                    .collect::<Vec<_>>(),
                expected
            );
            assert_eq!(&top[expected.len()..], tail);
            assert_eq!(
                g.current(g.object_id(id).unwrap()).unwrap().zone,
                if case == 1 {
                    Zone::Library { owner: PlayerId(0) }
                } else {
                    Zone::Command { owner: PlayerId(0) }
                }
            );
        });
    }
}
#[test]
fn hand_return_typed_zero_draw_and_unbounded_return_clamps_available_hand() {
    for draw in [0, 3] {
        for count in ["0", "999999999999999999999999"] {
            let mut p = prepared();
            let c = Arc::make_mut(&mut p.catalog);
            let d = c
                .definitions
                .iter_mut()
                .find(|d| d.name == "Brainstorm")
                .unwrap();
            d.spell.as_mut().unwrap().instructions = vec![
                Instruction::DrawControllerCards {
                    count: draw.into(),
                    source_block: 0,
                },
                Instruction::PutHandOnLibrary {
                    count: count.parse().unwrap(),
                    source_block: 0,
                },
            ];
            d.capabilities = d.requirements();
            d.validate().unwrap();
            p.catalog_digest = digest(c).unwrap();
            both(p, |g, q| {
                let card = develop(g, q);
                fund(g, q, "Island", 1);
                request(g, q);
                announce(g, q, card.0, vec![]);
                for _ in 0..1000 {
                    let r = request(g, q);
                    if let Request::HandReturn {
                        candidates, count, ..
                    } = &r
                    {
                        assert_eq!(*count, candidates.len());
                        choose(g, &r, candidates.clone());
                        break;
                    }
                    if g.state.rules.stack.is_empty() {
                        break;
                    }
                    answer(g, q, pass(&r));
                }
                finish(g, q);
                if count != "0" {
                    assert!(g.action_hand(PlayerId(0)).is_empty());
                }
            });
        }
    }
}

#[test]
fn hand_return_atomic_rollback_and_short_library_failed_attempts() {
    both(prepared(), |g, q| {
        let r = paid(g, q);
        let Request::HandReturn { candidates, .. } = &r else {
            panic!("hand")
        };
        let Frame::ResolutionChoice { frame, .. } = &g.state.rules.frame else {
            panic!("frame")
        };
        let cause = BatchCause::Instruction {
            stack_id: frame.spell.id,
            player: PlayerId(0),
            source: frame.spell.source,
            cursor: frame.cursor,
            source_block: 0,
        };
        let state = g.state.clone();
        let trace = g.trace.clone();
        assert!(
            g.commit_batch(
                cause,
                EventGroup::Ordered {
                    children: vec![
                        EventGroup::Occurrence {
                            proposed: ProposedEvent::HandReturn {
                                player: PlayerId(0),
                                count: 2.into(),
                                objects: candidates[..2].to_vec(),
                                command: vec![]
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
        assert_eq!(g.state, state);
        assert_eq!(g.trace, trace);
        choose(g, &r, candidates[..2].to_vec());
        finish(g, q);
    });
    // Historical short-library setup, not a reviewed Commander pod.
    let mut p = prepared_with_sources(
        &[("Brainstorm", 0, 0, true), ("Island", 0, 0, false)],
        &[BRAINSTORM],
    );
    let keep = p.config.initial_libraries()[&PlayerId(0)][0];
    p.config
        .cards
        .retain(|c| c.zone != (Zone::Library { owner: PlayerId(0) }) || c.id == keep);
    p.config.initial.libraries = None;
    p.profile_digest = digest(&p.config).unwrap();
    both(p, |g, q| {
        fund(g, q, "Island", 1);
        request(g, q);
        announce(g, q, 0, vec![]);
        let r = loop {
            let r = request(g, q);
            if matches!(r, Request::HandReturn { .. }) {
                break r;
            }
            answer(g, q, pass(&r));
        };
        let Request::HandReturn {
            candidates, count, ..
        } = &r
        else {
            panic!("short hand")
        };
        assert_eq!(*count, 1);
        let Frame::ResolutionChoice { frame, .. } = &g.state.rules.frame else {
            panic!("frame")
        };
        assert!(
            matches!(&frame.outcomes[0],InstructionOutcome::DrawAttempts{attempted,failed,..} if *attempted==3.into()&&*failed==2.into())
        );
        assert!(g.is_live(PlayerId(0)));
        choose(g, &r, candidates.clone());
        finish(g, q);
        assert!(!g.is_live(PlayerId(0)));
    });
}
