//! Complete paid sources. CR121.5/400.7/404.3/608.2c/903.9b.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
const SOURCES: [&str; 5] = [
    include_str!("../../../../../tests/fixtures/02b5acf3-47cb-4d39-9307-e02656f1879b.json"),
    include_str!("../../../../../tests/fixtures/05a039ad-1689-4d13-b393-6c1219583b5d.json"),
    include_str!("../../../../../tests/fixtures/9b97a7fe-c5d9-4698-a49c-183b3f1ce4db.json"),
    include_str!("../../../../../tests/fixtures/a97da5a9-17fc-4367-81e1-17ff81601a63.json"),
    include_str!("../../../../../tests/fixtures/f6bd2902-7f8b-419e-bbc7-bcab0c1b7e01.json"),
];
fn prepared(name: &'static str) -> Prepared {
    let color = if name == "Ransack the Lab" {
        "Swamp"
    } else {
        "Island"
    };
    let mut p = prepared_with_sources(
        &[
            (name, 0, 0, true),
            (color, 0, 0, false),
            (color, 0, 0, false),
            ("Blood Artist", 1, 1, false),
        ],
        &SOURCES,
    );
    let names = ["Grizzly Bears", "Island", "Bonesplitter", "Forest"];
    for (i, c) in p
        .config
        .cards
        .iter_mut()
        .filter(|c| matches!(c.zone, Zone::Library { .. }))
        .enumerate()
    {
        c.oracle_id = p
            .catalog
            .definitions
            .iter()
            .find(|d| d.name == names[i % 4])
            .unwrap()
            .oracle_id
            .clone();
    }
    let mut libraries = p.config.initial_libraries();
    for l in libraries.values_mut() {
        l.reverse();
    }
    p.config.initial.libraries = Some(libraries);
    p.profile_digest = digest(&p.config).unwrap();
    p
}
fn library(g: &Game, seat: u8) -> Vec<ObjectId> {
    g.state
        .rules
        .libraries
        .get(&PlayerId(seat))
        .into_iter()
        .flatten()
        .map(|id| g.object_id(*id).unwrap())
        .collect()
}
fn paid(g: &mut Game, q: u64) {
    let d = g.definition(CardId(0)).unwrap();
    let cost = d.characteristics.mana_cost.clone();
    let name = if cost.colored == vec![Color::B] {
        "Swamp"
    } else {
        "Island"
    };
    fund(
        g,
        q,
        name,
        cost.generic.to_string().parse::<usize>().unwrap() + 1,
    );
    request(g, q);
    announce(g, q, 0, vec![]);
    assert_eq!(g.state.rules.stack.last().unwrap().paid, Some(cost));
    recover(g);
}
fn decision(g: &mut Game, q: u64) -> (Request, Vec<ObjectId>, LibraryRemainder) {
    for _ in 0..1000 {
        let r = request(g, q);
        if let Request::LibrarySelect {
            candidates,
            remainder,
            ..
        } = &r
        {
            let result = (r.clone(), candidates.clone(), *remainder);
            recover(g);
            return result;
        }
        assert!(!g.state.rules.stack.is_empty());
        answer(g, q, pass(&r));
    }
    panic!("selection bound")
}
fn choose(g: &mut Game, r: &Request, hand: Vec<ObjectId>, remainder: Vec<ObjectId>) {
    g.respond(
        r.player(),
        r.decision_id(),
        Response::LibrarySelect { hand, remainder },
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
fn library_selection_paid_five_sources_each_choice_bottom_order_and_graveyard_successors() {
    for (name, n, destination) in [
        ("Strategic Planning", 3, LibraryRemainder::Graveyard),
        ("Sleight of Hand", 2, LibraryRemainder::Bottom),
        ("Ransack the Lab", 3, LibraryRemainder::Graveyard),
        ("Anticipate", 3, LibraryRemainder::Bottom),
        ("Impulse", 4, LibraryRemainder::Bottom),
    ] {
        for selected in 0..n {
            both(prepared(name), |g, q| {
                let before = library(g, 0);
                let opponent = library(g, 1);
                paid(g, q);
                let (r, looked, actual_dest) = decision(g, q);
                assert_eq!(looked, before[..n]);
                assert_eq!(actual_dest, destination);
                let hand = vec![looked[selected]];
                let rest = looked
                    .iter()
                    .rev()
                    .copied()
                    .filter(|o| *o != hand[0])
                    .collect::<Vec<_>>();
                choose(g, &r, hand.clone(), rest.clone());
                for _ in 0..50 {
                    if matches!(&g.state.rules.frame,Frame::Resolve{frame} if frame.cursor==1) {
                        break;
                    }
                    assert!(matches!(
                        g.advance(Budget::new(1)),
                        Advance::Suspended { .. }
                    ));
                }
                let Frame::Resolve { frame } = &g.state.rules.frame else {
                    panic!("completed selection")
                };
                assert!(
                    matches!(
                        frame.outcomes.as_slice(),
                        [InstructionOutcome::Effect {
                            actual: Some(EventOutcome::LibrarySelected { .. })
                        }]
                    ),
                    "selection is not a draw attempt"
                );
                recover(g);
                finish(g, q);
                let new = object(g, hand[0].card.0);
                assert_ne!(new, hand[0]);
                assert_eq!(
                    g.current(new).unwrap().zone,
                    Zone::Hand { owner: PlayerId(0) }
                );
                assert_eq!(library(g, 1), opponent);
                if destination == LibraryRemainder::Bottom {
                    let mut expected = before[n..].to_vec();
                    expected.extend(&rest);
                    assert_eq!(library(g, 0), expected);
                    for o in rest {
                        assert_eq!(g.current(o).unwrap().id, o);
                    }
                } else {
                    assert_eq!(library(g, 0), before[n..]);
                    for o in rest {
                        let successor = object(g, o.card.0);
                        assert_ne!(o, successor);
                        assert_eq!(
                            g.current(successor).unwrap().zone,
                            Zone::Graveyard { owner: PlayerId(0) }
                        );
                    }
                }
                assert!(g.state.rules.players.iter().all(|p| p.life == 40.into()));
                if g.trace.mode == TraceMode::Detailed {
                    let b = batches(g);
                    let actual = b
                        .iter()
                        .flat_map(|b| &b.occurrences)
                        .find_map(|o| {
                            if let EventOutcome::LibrarySelected {
                                hand, graveyard, ..
                            } = &o.actual
                            {
                                Some((hand, graveyard))
                            } else {
                                None
                            }
                        })
                        .unwrap();
                    assert_eq!(actual.0.as_ref().unwrap().previous, hand[0]);
                    assert_eq!(
                        actual.1.len(),
                        if destination == LibraryRemainder::Graveyard {
                            n - 1
                        } else {
                            0
                        }
                    );
                }
            });
        }
    }
}
#[test]
fn library_selection_private_queries_invalid_inputs_receipts_work_and_forgery() {
    both(prepared("Impulse"), |g, q| {
        paid(g, q);
        let (r, looked, _) = decision(g, q);
        let before = g.state.clone();
        let trace = g.trace.clone();
        let QueryValue::Decision(Some(PlayerRequest::LibrarySelect {
            token,
            candidates,
            remainder,
            ..
        })) = finish_query(g, g.player_decision_for(PlayerId(0), Budget::new(0)))
        else {
            panic!("private decision")
        };
        assert_eq!(remainder, LibraryRemainder::Bottom);
        assert_eq!(
            candidates,
            looked
                .iter()
                .map(|o| g.handle_for(PlayerId(0), *o).unwrap())
                .collect::<Vec<_>>()
        );
        for viewer in 1..4 {
            let QueryValue::Observation(o) =
                finish_query(g, g.observe(PlayerId(viewer), Budget::new(0)))
            else {
                panic!("observation")
            };
            assert!(o.decision.is_none());
            assert!(
                !o.view
                    .objects
                    .iter()
                    .any(|o| matches!(o.zone, Zone::Library { .. }))
            );
        }
        for (hand, rest) in [
            (vec![], looked.clone()),
            (looked.clone(), vec![]),
            (vec![looked[0]], vec![looked[0]; 3]),
            (vec![library(g, 1)[0]], looked[1..].to_vec()),
        ] {
            assert!(
                g.respond(
                    PlayerId(0),
                    r.decision_id(),
                    Response::LibrarySelect {
                        hand,
                        remainder: rest
                    }
                )
                .is_err()
            );
            assert_eq!(g.state, before);
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
            QueryValue::Decision(Some(PlayerRequest::LibrarySelect { .. }))
        ));
        let mut bad: Snapshot = strict_read(&g.snapshot().unwrap()).unwrap();
        let Frame::ResolutionChoice {
            choice: ResolutionChoice::LibrarySelect { looked: ids, .. },
            ..
        } = &mut bad.state.rules.frame
        else {
            panic!("frame")
        };
        ids.reverse();
        bad.state.rules.libraries.get_mut(&PlayerId(0)).unwrap()[..4].reverse();
        bad.state_sha256 = digest(&bad.state).unwrap();
        assert!(Game::restore(g.prepared.as_ref().clone(), &canonical(&bad).unwrap()).is_err());
        let response = PlayerResponse::LibrarySelect {
            hand: vec![candidates[1].clone()],
            remainder: vec![
                candidates[3].clone(),
                candidates[0].clone(),
                candidates[2].clone(),
            ],
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
        assert_eq!(g.state, before);
        g.submit(receipt).unwrap();
        assert!(g.submit(duplicate).is_err());
        assert!(g.restore_work(&saved).is_err());
        finish(g, q);
        for h in candidates {
            assert!(g.resolve_handle(PlayerId(0), &h).is_err());
        }
    });
}
#[test]
fn library_selection_departure_before_after_acceptance_and_commander_replacement() {
    for stage in 0..3 {
        let mut p = prepared("Strategic Planning");
        if stage == 2 {
            let id = p.config.initial.libraries.as_ref().unwrap()[&PlayerId(0)][0];
            p.config
                .cards
                .iter_mut()
                .find(|c| c.id == id)
                .unwrap()
                .oracle_id = p
                .catalog
                .definitions
                .iter()
                .find(|d| d.name == "Isamaru, Hound of Konda")
                .unwrap()
                .oracle_id
                .clone();
            p.config.commanders = vec![id];
            p.profile_digest = digest(&p.config).unwrap();
        }
        both(p, |g, q| {
            paid(g, q);
            let (r, looked, _) = decision(g, q);
            if stage > 0 {
                choose(g, &r, vec![looked[0]], looked[1..].to_vec());
            }
            if stage == 2 {
                assert!(matches!(request(g, q), Request::CommanderReturn { .. }));
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
fn library_selection_commander_hand_replacement_and_graveyard_sba_are_distinct() {
    for to_command in [false, true] {
        for selected_commander in [false, true] {
            let mut p = prepared("Strategic Planning");
            let id = p.config.initial.libraries.as_ref().unwrap()[&PlayerId(0)][0];
            p.config
                .cards
                .iter_mut()
                .find(|c| c.id == id)
                .unwrap()
                .oracle_id = p
                .catalog
                .definitions
                .iter()
                .find(|d| d.name == "Isamaru, Hound of Konda")
                .unwrap()
                .oracle_id
                .clone();
            p.config.commanders = vec![id];
            p.profile_digest = digest(&p.config).unwrap();
            both(p, |g, q| {
                paid(g, q);
                let (r, looked, _) = decision(g, q);
                let hand = looked[usize::from(!selected_commander)];
                let rest = looked.iter().copied().filter(|o| *o != hand).collect();
                choose(g, &r, vec![hand], rest);
                let r = request(g, q);
                assert!(matches!(r, Request::CommanderReturn { .. }));
                assert_eq!(g.state.rules.stack.is_empty(), !selected_commander);
                recover(g);
                g.respond(
                    r.player(),
                    r.decision_id(),
                    Response::CommanderReturn { to_command },
                )
                .unwrap();
                finish(g, q);
                assert_eq!(
                    g.current(object(g, id.0)).unwrap().zone,
                    if to_command {
                        Zone::Command { owner: PlayerId(0) }
                    } else if selected_commander {
                        Zone::Hand { owner: PlayerId(0) }
                    } else {
                        Zone::Graveyard { owner: PlayerId(0) }
                    }
                );
            });
        }
    }
}
#[test]
fn library_selection_typed_zero_short_empty_and_large_counts() {
    // Typed parameter probes, not synthetic printed-source claims.
    for (amount, len) in [("0", 12), ("3", 0), ("3", 1), ("999999999999999999999", 12)] {
        let mut p = prepared("Impulse");
        let c = Arc::make_mut(&mut p.catalog);
        let d = c
            .definitions
            .iter_mut()
            .find(|d| d.name == "Impulse")
            .unwrap();
        if let Instruction::SelectLibraryCard { amount: a, .. } =
            &mut d.spell.as_mut().unwrap().instructions[0]
        {
            *a = amount.parse().unwrap();
        }
        c.validate().unwrap();
        p.catalog_digest = digest(c).unwrap();
        let keep = p.config.initial.libraries.as_ref().unwrap()[&PlayerId(0)][..len].to_vec();
        p.config
            .cards
            .retain(|c| c.zone != (Zone::Library { owner: PlayerId(0) }) || keep.contains(&c.id));
        p.config.initial.libraries = None;
        p.config.initial.libraries = Some(p.config.initial_libraries());
        p.profile_digest = digest(&p.config).unwrap();
        both(p, |g, q| {
            let before = library(g, 0);
            paid(g, q);
            if len > 0 && amount != "0" {
                let (r, looked, _) = decision(g, q);
                assert_eq!(looked, before);
                choose(g, &r, vec![looked[0]], looked[1..].to_vec());
            }
            finish(g, q);
            assert_eq!(
                library(g, 0).len(),
                if len > 0 && amount != "0" {
                    len - 1
                } else {
                    len
                }
            );
            assert!(g.is_live(PlayerId(0)));
        });
    }
}
#[test]
fn library_selection_late_group_failure_rolls_back_every_movement_and_memory() {
    both(prepared("Strategic Planning"), |g, q| {
        paid(g, q);
        let (r, looked, destination) = decision(g, q);
        let Frame::ResolutionChoice { frame, .. } = &g.state.rules.frame else {
            panic!("frame")
        };
        let cause = BatchCause::Instruction {
            stack_id: frame.stack_id,
            player: PlayerId(0),
            source: frame.spell.source,
            cursor: frame.cursor,
            source_block: 0,
        };
        let good = ProposedEvent::LibrarySelect {
            player: PlayerId(0),
            amount: 3.into(),
            destination,
            looked: looked.clone(),
            hand: vec![looked[0]],
            remainder: looked[1..].to_vec(),
            hand_to_command: false,
        };
        let state = g.state.clone();
        let trace = g.trace.clone();
        assert!(
            g.commit_batch(
                cause,
                EventGroup::Ordered {
                    children: vec![
                        EventGroup::Occurrence { proposed: good },
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
        choose(g, &r, vec![looked[0]], looked[1..].to_vec());
        finish(g, q);
    });
}
