//! Private complete-source paid Surveil witnesses. No consumer/pool admission.
//! Archive d705222b280a71cb; CR701.25/400.7/903.9a, frozen20260925.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
const CONSIDER: &str =
    include_str!("../../../../../tests/fixtures/4c9bcba6-87b5-4fb3-97ee-6fe5b739337d.json");
const CURATE: &str =
    include_str!("../../../../../tests/fixtures/75e8907d-f726-4a63-ba8b-c0fd4936734e.json");
fn prepared(rows: &[Row]) -> Prepared {
    let mut p = prepared_with_sources(rows, &[CONSIDER, CURATE]);
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
            .find(|d| d.name == names[i % names.len()])
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
fn paid(g: &mut Game, q: u64, id: u32) {
    let cost = g
        .definition(CardId(id))
        .unwrap()
        .characteristics
        .mana_cost
        .clone();
    fund(
        g,
        q,
        "Island",
        cost.generic.to_string().parse::<usize>().unwrap() + cost.colored.len(),
    );
    request(g, q);
    announce(g, q, id, vec![]);
    assert_eq!(g.state.rules.stack.last().unwrap().paid, Some(cost));
    recover(g);
}
fn decision(g: &mut Game, q: u64) -> (Request, Vec<ObjectId>) {
    for _ in 0..1000 {
        let r = request(g, q);
        if let Request::Surveil { candidates, .. } = &r {
            let ids = candidates.clone();
            recover(g);
            return (r, ids);
        }
        assert!(!g.state.rules.stack.is_empty());
        answer(g, q, pass(&r));
    }
    panic!("bounded Surveil")
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
    panic!("bounded finish")
}
fn choose(g: &mut Game, r: &Request, top: Vec<ObjectId>, graveyard: Vec<ObjectId>) {
    g.respond(
        r.player(),
        r.decision_id(),
        Response::Surveil { top, graveyard },
    )
    .unwrap();
    recover(g);
}
#[test]
fn controller_surveil_paid_sources_all_partitions_movement_and_draw_order() {
    for (name, n, orders) in [
        ("Consider", 1, vec![([0, 1], 0), ([0, 1], 1)]),
        (
            "Curate",
            2,
            vec![
                ([0, 1], 0),
                ([1, 0], 0),
                ([0, 1], 1),
                ([1, 0], 1),
                ([0, 1], 2),
                ([1, 0], 2),
            ],
        ),
    ] {
        for (order, kept) in orders {
            let mut rows = vec![(name, 0, 0, true), ("Blood Artist", 1, 1, false)];
            lands(&mut rows, "Island", 0, 2);
            both(prepared(&rows), |g, q| {
                let before = library(g, 0);
                let other = library(g, 1);
                paid(g, q, 0);
                let (r, looked) = decision(g, q);
                assert_eq!(looked, before[..n]);
                let ordered = if n == 1 {
                    vec![looked[0]]
                } else {
                    order.iter().map(|i| looked[*i]).collect()
                };
                let top = ordered[..kept].to_vec();
                let graveyard = ordered[kept..].to_vec();
                choose(g, &r, top.clone(), graveyard.clone());
                finish(g, q);
                let mut expected = top.iter().chain(&before[n..]).copied().collect::<Vec<_>>();
                let drawn = expected.remove(0);
                assert_eq!(library(g, 0), expected);
                assert_eq!(library(g, 1), other);
                assert_eq!(
                    g.current(object(g, drawn.card.0)).unwrap().zone,
                    Zone::Hand { owner: PlayerId(0) }
                );
                assert_ne!(object(g, drawn.card.0), drawn);
                for id in &graveyard {
                    let successor = object(g, id.card.0);
                    assert_ne!(*id, successor);
                    assert_eq!(
                        g.current(successor).unwrap().zone,
                        Zone::Graveyard { owner: PlayerId(0) }
                    );
                }
                for p in &g.state.rules.players {
                    assert_eq!(p.life, 40.into(), "library moves are not dies");
                }
                if g.trace.mode == TraceMode::Detailed {
                    let events = batches(g)
                        .iter()
                        .flat_map(|b| &b.occurrences)
                        .filter(|o| matches!(o.actual, EventOutcome::Surveilled { .. }))
                        .map(|o| o.actual.clone())
                        .collect::<Vec<_>>();
                    assert_eq!(events.len(), 1);
                    let EventOutcome::Surveilled {
                        movements,
                        top: actual_top,
                        graveyard: actual_graveyard,
                        ..
                    } = &events[0]
                    else {
                        unreachable!()
                    };
                    assert_eq!(actual_top, &top);
                    assert_eq!(actual_graveyard, &graveyard);
                    assert_eq!(movements.len(), graveyard.len());
                    for (m, old) in movements.iter().zip(&graveyard) {
                        assert_eq!(m.previous, *old);
                        assert_eq!(m.from, Zone::Library { owner: PlayerId(0) });
                        assert_eq!(m.destination, Zone::Graveyard { owner: PlayerId(0) });
                    }
                }
            });
        }
    }
}
#[test]
fn controller_surveil_private_queries_invalid_partitions_and_recovery() {
    both(
        prepared(&[
            ("Curate", 0, 0, true),
            ("Island", 0, 0, false),
            ("Island", 0, 0, false),
        ]),
        |g, q| {
            paid(g, q, 0);
            let (r, looked) = decision(g, q);
            let before = g.state.clone();
            let trace = g.trace.clone();
            let QueryValue::Decision(Some(PlayerRequest::Surveil {
                token, candidates, ..
            })) = finish_query(g, g.player_decision_for(PlayerId(0), Budget::new(0)))
            else {
                panic!("public Surveil")
            };
            assert_eq!(
                candidates,
                looked
                    .iter()
                    .map(|o| g.handle_for(PlayerId(0), *o).unwrap())
                    .collect::<Vec<_>>()
            );
            for seat in 1..4 {
                let QueryValue::Observation(o) =
                    finish_query(g, g.observe(PlayerId(seat), Budget::new(0)))
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
            for (top, graveyard) in [
                (vec![], vec![]),
                (looked.clone(), looked.clone()),
                (vec![looked[0]], vec![looked[0]]),
                (vec![library(g, 1)[0]], vec![looked[1]]),
            ] {
                assert!(
                    g.respond(
                        PlayerId(0),
                        r.decision_id(),
                        Response::Surveil { top, graveyard }
                    )
                    .is_err()
                );
                assert_eq!(g.state, before);
                assert_eq!(g.trace, trace);
            }
            let QueryResult::Incomplete { work, .. } =
                g.player_decision_for(PlayerId(0), Budget::new(2))
            else {
                panic!("partial work")
            };
            let saved = g.save_work(&work).unwrap();
            let work = g.restore_work(&saved).unwrap();
            let QueryValue::Decision(Some(actual)) =
                finish_query(g, g.resume_work(work, Budget::unlimited()))
            else {
                panic!("restored decision")
            };
            assert!(matches!(actual,PlayerRequest::Surveil {candidates:ids,..} if ids==candidates));
            let wire = g.snapshot().unwrap();
            let mut bad: Snapshot = strict_read(&wire).unwrap();
            let Frame::ResolutionChoice {
                choice: ResolutionChoice::Surveil { looked: ids, .. },
                ..
            } = &mut bad.state.rules.frame
            else {
                unreachable!()
            };
            ids.reverse();
            bad.state
                .rules
                .libraries
                .get_mut(&PlayerId(0))
                .unwrap()
                .swap(0, 1);
            bad.state_sha256 = digest(&bad.state).unwrap();
            assert!(Game::restore(g.prepared.as_ref().clone(), &canonical(&bad).unwrap()).is_err());
            let response = PlayerResponse::Surveil {
                top: vec![candidates[1].clone()],
                graveyard: vec![candidates[0].clone()],
            };
            let QueryValue::Validated(receipt) = finish_query(
                g,
                g.check_player_response(
                    PlayerId(0),
                    token.clone(),
                    response.clone(),
                    Budget::new(0),
                ),
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
            finish(g, q);
            for h in candidates {
                assert!(g.resolve_handle(PlayerId(0), &h).is_err());
            }
        },
    );
}
#[test]
fn controller_surveil_departed_chooser_and_atomic_late_failure() {
    for accepted in [false, true] {
        both(
            prepared(&[("Consider", 0, 0, true), ("Island", 0, 0, false)]),
            |g, q| {
                paid(g, q, 0);
                let (r, looked) = decision(g, q);
                if accepted {
                    choose(g, &r, vec![], looked);
                }
                g.concede(PlayerId(0)).unwrap();
                recover(g);
                finish(g, q);
                assert!(!g.is_live(PlayerId(0)));
                assert!(g.state.rules.pending.is_none());
            },
        );
    }
    both(
        prepared(&[
            ("Curate", 0, 0, true),
            ("Island", 0, 0, false),
            ("Island", 0, 0, false),
        ]),
        |g, q| {
            paid(g, q, 0);
            let (r, looked) = decision(g, q);
            let Frame::ResolutionChoice { frame, .. } = &g.state.rules.frame else {
                unreachable!()
            };
            let cause = BatchCause::Instruction {
                stack_id: frame.stack_id,
                player: PlayerId(0),
                source: frame.spell.source,
                cursor: frame.cursor,
                source_block: 0,
            };
            let before = g.state.clone();
            let trace = g.trace.clone();
            let good = ProposedEvent::Surveil {
                player: PlayerId(0),
                amount: 2.into(),
                looked: looked.clone(),
                top: vec![],
                graveyard: looked.clone(),
            };
            let bad = ProposedEvent::Scry {
                player: PlayerId(0),
                amount: (-1).into(),
                looked: vec![],
                top: vec![],
                bottom: vec![],
            };
            assert!(
                g.commit_batch(
                    cause,
                    EventGroup::Ordered {
                        children: vec![
                            EventGroup::Occurrence { proposed: good },
                            EventGroup::Occurrence { proposed: bad }
                        ]
                    }
                )
                .is_err()
            );
            assert_eq!(g.state, before);
            assert_eq!(g.trace, trace);
            choose(g, &r, vec![], looked);
            finish(g, q);
        },
    );
}

#[test]
fn controller_surveil_commander_returns_only_after_complete_spell_draw() {
    for return_to_command in [false, true] {
        let mut p = prepared(&[("Consider", 0, 0, true), ("Island", 0, 0, false)]);
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
            let hand = g
                .state
                .rules
                .objects
                .values()
                .filter(|o| o.zone == (Zone::Hand { owner: PlayerId(0) }))
                .count();
            paid(g, q, 0);
            let (r, looked) = decision(g, q);
            assert_eq!(looked[0].card, id);
            choose(g, &r, vec![], looked);
            let r = request(g, q);
            assert!(matches!(r, Request::CommanderReturn { .. }));
            assert!(g.state.rules.stack.is_empty());
            assert_eq!(
                g.state
                    .rules
                    .objects
                    .values()
                    .filter(|o| o.zone == (Zone::Hand { owner: PlayerId(0) }))
                    .count(),
                hand
            );
            recover(g);
            g.respond(
                r.player(),
                r.decision_id(),
                Response::CommanderReturn {
                    to_command: return_to_command,
                },
            )
            .unwrap();
            finish(g, q);
            assert_eq!(
                g.current(object(g, id.0)).unwrap().zone,
                if return_to_command {
                    Zone::Command { owner: PlayerId(0) }
                } else {
                    Zone::Graveyard { owner: PlayerId(0) }
                }
            );
        });
    }
}

#[test]
fn controller_surveil_kernel_zero_empty_and_large_parameter_counts() {
    // Typed parameter controls only, no synthetic printed-source claim.
    for (amount, empty) in [
        ("0", false),
        ("2", true),
        ("999999999999999999999999", false),
    ] {
        let mut p = prepared(&[("Consider", 0, 0, true), ("Island", 0, 0, false)]);
        let c = Arc::make_mut(&mut p.catalog);
        let d = c
            .definitions
            .iter_mut()
            .find(|d| d.name == "Consider")
            .unwrap();
        d.name = "Synthetic controller Surveil count".into();
        d.spell.as_mut().unwrap().instructions = vec![Instruction::Surveil {
            amount: amount.parse().unwrap(),
            source_block: 0,
        }];
        d.capabilities = d.requirements();
        c.validate().unwrap();
        p.catalog_digest = digest(c).unwrap();
        if empty {
            p.config
                .cards
                .retain(|c| c.zone != (Zone::Library { owner: PlayerId(0) }));
            p.config.initial.libraries = None;
            p.config.initial.libraries = Some(p.config.initial_libraries());
            p.profile_digest = digest(&p.config).unwrap();
        }
        both(p, |g, q| {
            let before = library(g, 0);
            paid(g, q, 0);
            if !empty && amount != "0" {
                let (r, looked) = decision(g, q);
                assert_eq!(looked, before);
                choose(g, &r, looked, vec![]);
            }
            finish(g, q);
            assert_eq!(library(g, 0), before);
            if g.trace.mode == TraceMode::Detailed {
                assert_eq!(
                    batches(g)
                        .iter()
                        .flat_map(|b| &b.occurrences)
                        .filter(|o| matches!(o.actual, EventOutcome::Surveilled { .. }))
                        .count(),
                    usize::from(amount != "0")
                );
            }
        });
    }
}
