//! Complete paid sources, CR701.17/400.7/608.2b-c/903.9a, frozen20260925.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
const TOME: &str =
    include_str!("../../../../../tests/fixtures/446a9d7e-9c1a-4e83-8342-2ed5acae2eed.json");
const THOUGHT: &str =
    include_str!("../../../../../tests/fixtures/83101ba8-a569-4827-8c53-9ca0dfcd59a7.json");
const MENTAL: &str =
    include_str!("../../../../../tests/fixtures/e8d5f31c-7abf-4fbb-977e-8353a97daf7a.json");
fn prepared(rows: &[Row]) -> Prepared {
    prepared_with_sources(rows, &[TOME, THOUGHT, MENTAL])
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
fn paid(g: &mut Game, q: u64, target: Option<PlayerId>) {
    fund(g, q, "Island", 1);
    request(g, q);
    answer(g, q, Response::Cast { card: CardId(0) });
    loop {
        let r = request(g, q);
        let response = match &r {
            Request::PlayerTargets { candidates, .. } => {
                assert_eq!(
                    candidates,
                    &vec![PlayerId(0), PlayerId(1), PlayerId(2), PlayerId(3)]
                );
                let state = g.state.clone();
                let trace = g.trace.clone();
                assert!(
                    g.respond(
                        r.player(),
                        r.decision_id(),
                        Response::PlayerTargets {
                            players: vec![PlayerId(9)]
                        }
                    )
                    .is_err()
                );
                assert_eq!(state, g.state);
                assert_eq!(trace, g.trace);
                Response::PlayerTargets {
                    players: vec![target.unwrap()],
                }
            }
            Request::Mana { .. } => Response::FinishMana { undo_mana: false },
            Request::SpendMana { .. } => Response::SpendMana {
                spending: BTreeMap::from([(Color::U, 1.into())]),
            },
            Request::Priority { .. } => break,
            x => panic!("payment {x:?}"),
        };
        g.respond(r.player(), r.decision_id(), response).unwrap();
        recover(g);
    }
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
fn mill_done(g: &Game) -> bool {
    matches!(&g.state.rules.frame,Frame::Resolve { frame:f } if f.cursor==1)
}
#[test]
fn fixed_mill_paid_complete_sources_player_targets_order_draw_and_public_successors() {
    for (name, count, target, draw) in [
        ("Mental Note", 2, None, true),
        ("Thought Scour", 2, Some(0), true),
        ("Thought Scour", 2, Some(1), true),
        ("Tome Scour", 5, Some(2), false),
    ] {
        both(
            prepared(&[
                (name, 0, 0, true),
                ("Island", 0, 0, false),
                ("Blood Artist", 3, 3, false),
            ]),
            |g, q| {
                let seat = target.unwrap_or(0);
                let before = library(g, seat);
                let caster = library(g, 0);
                let other = library(g, 3);
                paid(g, q, target.map(PlayerId));
                finish(g, q);
                let draw_from = if draw && seat == 0 { count } else { 0 };
                assert_eq!(
                    library(g, seat),
                    before[count + usize::from(draw && seat == 0)..]
                );
                if seat != 0 {
                    assert_eq!(library(g, 0), caster[usize::from(draw)..]);
                }
                assert_eq!(library(g, 3), other);
                if draw {
                    assert_eq!(
                        g.current(object(g, caster[draw_from].card.0)).unwrap().zone,
                        Zone::Hand { owner: PlayerId(0) }
                    );
                }
                for old in &before[..count] {
                    let new = object(g, old.card.0);
                    assert_ne!(*old, new);
                    assert_eq!(
                        g.current(new).unwrap().zone,
                        Zone::Graveyard {
                            owner: PlayerId(seat)
                        }
                    );
                    for viewer in 0..4 {
                        let h = g.handle_for(PlayerId(viewer), new).unwrap();
                        assert_eq!(g.resolve_handle(PlayerId(viewer), &h).unwrap(), new);
                    }
                }
                assert!(
                    g.state.rules.players.iter().all(|p| p.life == 40.into()),
                    "library movements are not dies"
                );
                if g.trace.mode == TraceMode::Detailed {
                    let b = batches(g);
                    let entries = b
                        .iter()
                        .flat_map(|b| &b.occurrences)
                        .filter_map(|o| {
                            if let EventOutcome::Milled { player, movements } = &o.actual {
                                Some((*player, movements))
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(entries.len(), 1);
                    assert_eq!(entries[0].0, PlayerId(seat));
                    assert_eq!(
                        entries[0].1.iter().map(|m| m.previous).collect::<Vec<_>>(),
                        before[..count]
                    );
                }
            },
        );
    }
}
#[test]
fn fixed_mill_target_departure_fizzles_whole_spell_but_completed_mill_keeps_draw() {
    for after_mill in [false, true] {
        both(
            prepared(&[("Thought Scour", 0, 0, true), ("Island", 0, 0, false)]),
            |g, q| {
                let caster = library(g, 0);
                paid(g, q, Some(PlayerId(1)));
                if after_mill {
                    // Every accepted boundary uses one rules step here, independent of outer q.
                    for _ in 0..500 {
                        if mill_done(g) {
                            break;
                        }
                        match g.advance(Budget::new(1)) {
                            Advance::AwaitingDecision(r) => {
                                g.respond(r.player(), r.decision_id(), pass(&r)).unwrap();
                            }
                            Advance::Suspended { .. } => (),
                            x => panic!("{x:?}"),
                        }
                    }
                    assert!(mill_done(g));
                    recover(g);
                }
                g.concede(PlayerId(1)).unwrap();
                recover(g);
                finish(g, q);
                assert_eq!(library(g, 0), caster[usize::from(after_mill)..]);
            },
        );
    }
}
#[test]
fn fixed_mill_commander_return_waits_for_draw_and_complete_resolution() {
    for return_to_command in [false, true] {
        let mut p = prepared(&[("Mental Note", 0, 0, true), ("Island", 0, 0, false)]);
        let id = p.config.initial_libraries()[&PlayerId(0)][0];
        let oracle = p
            .catalog
            .definitions
            .iter()
            .find(|d| d.name == "Isamaru, Hound of Konda")
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
            let before = library(g, 0);
            paid(g, q, None);
            let r = loop {
                let r = request(g, q);
                if matches!(r, Request::CommanderReturn { .. }) {
                    break r;
                }
                answer(g, q, pass(&r));
            };
            assert!(g.state.rules.stack.is_empty());
            assert_eq!(
                g.current(object(g, before[2].card.0)).unwrap().zone,
                Zone::Hand { owner: PlayerId(0) }
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
fn fixed_mill_typed_zero_short_empty_and_arbitrary_large_counts() {
    // Typed parameter probes only; the three paid printed counts remain exact.
    for (amount, len) in [
        ("0", 12),
        ("2", 0),
        ("5", 1),
        ("999999999999999999999999", 12),
    ] {
        let mut p = prepared(&[("Tome Scour", 0, 0, true), ("Island", 0, 0, false)]);
        let c = Arc::make_mut(&mut p.catalog);
        let d = c
            .definitions
            .iter_mut()
            .find(|d| d.name == "Tome Scour")
            .unwrap();
        if let Instruction::MillCards { count, .. } = &mut d.spell.as_mut().unwrap().instructions[0]
        {
            *count = amount.parse().unwrap();
        }
        d.capabilities = d.requirements();
        c.validate().unwrap();
        p.catalog_digest = digest(c).unwrap();
        let keep = p.config.initial_libraries()[&PlayerId(1)][..len].to_vec();
        p.config
            .cards
            .retain(|c| c.zone != (Zone::Library { owner: PlayerId(1) }) || keep.contains(&c.id));
        p.config.initial.libraries = Some(p.config.initial_libraries());
        p.profile_digest = digest(&p.config).unwrap();
        both(p, |g, q| {
            let before = library(g, 1);
            paid(g, q, Some(PlayerId(1)));
            finish(g, q);
            let n = if amount == "0" { 0 } else { len };
            assert_eq!(library(g, 1), before[n..]);
            assert!(g.is_live(PlayerId(1)), "mill empty is not failed draw");
        });
    }
}
#[test]
fn fixed_mill_atomic_rollback_and_authenticated_completed_prefix() {
    both(
        prepared(&[("Mental Note", 0, 0, true), ("Island", 0, 0, false)]),
        |g, q| {
            paid(g, q, None);
            // Capture the real resolving prefix before its first instruction.
            for _ in 0..500 {
                if matches!(&g.state.rules.frame,Frame::Resolve { frame:f } if f.cursor==0) {
                    break;
                }
                match g.advance(Budget::new(1)) {
                    Advance::AwaitingDecision(r) => {
                        g.respond(r.player(), r.decision_id(), pass(&r)).unwrap();
                    }
                    Advance::Suspended { .. } => (),
                    x => panic!("{x:?}"),
                }
            }
            let Frame::Resolve { frame: f } = &g.state.rules.frame else {
                panic!("resolving")
            };
            let cause = BatchCause::Instruction {
                stack_id: f.stack_id,
                source: g.entry(f.stack_id).unwrap().source,
                player: g.entry(f.stack_id).unwrap().controller,
                cursor: 0,
                source_block: 0,
            };
            let cards = library(g, 0)[..2].to_vec();
            let state = g.state.clone();
            let trace = g.trace.clone();
            assert!(
                g.commit_batch(
                    cause,
                    EventGroup::Ordered {
                        children: vec![
                            EventGroup::Occurrence {
                                proposed: ProposedEvent::Mill {
                                    player: PlayerId(0),
                                    count: 2.into(),
                                    cards
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
            assert_eq!(state, g.state);
            assert_eq!(trace, g.trace);
            for _ in 0..50 {
                if mill_done(g) {
                    break;
                }
                assert!(matches!(
                    g.advance(Budget::new(1)),
                    Advance::Suspended { .. }
                ));
            }
            assert!(mill_done(g));
            recover(g);
            let mut bad: Snapshot = strict_read(&g.snapshot().unwrap()).unwrap();
            bad.state
                .rules
                .libraries
                .get_mut(&PlayerId(0))
                .unwrap()
                .swap(0, 1);
            bad.state_sha256 = digest(&bad.state).unwrap();
            assert!(Game::restore(g.prepared.as_ref().clone(), &canonical(&bad).unwrap()).is_err());
            finish(g, q);
        },
    );
}
