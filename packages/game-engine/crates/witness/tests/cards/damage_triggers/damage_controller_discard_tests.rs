//! Complete source-lowered Locke Cole: ordinary draw then mandatory private
//! controller discard, with one frozen invocation controller throughout.
use super::*;
const ISAMARU: &str = "cfb756dd-6fe1-430a-8f04-8b10cac62a86";
pub(super) fn prepared(empty: bool, commander: bool) -> Prepared {
    let mut c = FixtureCatalog::Witness.get().clone();
    let mtg_compiler::CompileResult::Lowered { definition } = mtg_witness::compile(
        include_str!("../../../../../tests/fixtures/archive-manifest-phase1-r5.json"),
        include_str!(
            "../../../../../tests/fixtures/phase1-r5/bdcb7aed-3595-4c7e-b9da-543e92de919a.json"
        ),
    ) else {
        panic!("complete Locke Cole source must lower");
    };
    assert!(
        definition
            .capabilities
            .contains(&Capability::DamageControllerDiscardV1)
    );
    if let Some(existing) = c.definition_by_oracle_id(&definition.oracle_id) {
        assert_eq!(existing, definition.as_ref());
    } else {
        c.definitions.push(*definition);
    }
    c.schema_version = mtg_contracts::CURRENT_IR_VERSION;
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    let mut board =
        Board::new(c, "damage-controller-discard-domain-witness").step(TurnStep::PrecombatMain);
    board = board
        .card("Locke Cole", 2, 0, Zone::Battlefield)
        .card("Swamp", 0, 0, Zone::Hand { owner: PlayerId(0) })
        .card("Forest", 1, 1, Zone::Hand { owner: PlayerId(1) });
    if commander {
        board = board
            .card(
                "Isamaru, Hound of Konda",
                0,
                0,
                Zone::Library { owner: PlayerId(0) },
            )
            .commander(CardId(3));
    }
    for seat in 0..4 {
        if seat != 0 || !empty {
            for _ in 0..3 {
                board = board.card(
                    "Swamp",
                    seat,
                    seat,
                    Zone::Library {
                        owner: PlayerId(seat),
                    },
                );
            }
        }
    }
    board.prepare()
}
pub(super) fn settle(g: &mut Game) -> Decision<Rules> {
    for _ in 0..500 {
        match g.advance(Budget::new(1)) {
            Advance::AwaitingDecision(r) => return r,
            Advance::Suspended { .. } => (),
            other => panic!("step witness at {:?}: {other:?}", g.state.rules.frame),
        }
    }
    panic!("step witness budget")
}
pub(super) fn answer(g: &mut Game, response: Answer) {
    let r = settle(g);
    g.respond(r.player, r.id, response).unwrap();
}
pub(super) fn passive(g: &mut Game) {
    let r = settle(g);
    let (player, decision_id) = (r.player, r.id);
    let response = match r.clone() {
        Decision {
            reason: Reason::Priority,
            ..
        } => Answer::Pick {
            picks: vec![vec![0]],
        },
        Decision {
            reason: Reason::Attackers,
            ..
        } => r.attack(vec![]),
        Decision {
            reason: Reason::Blockers,
            ..
        } => r.block(vec![]),
        found @ Decision {
            reason: Reason::OrderTriggers,
            ..
        } => {
            let candidates = found
                .candidates
                .iter()
                .filter_map(|c| {
                    if let Candidate::Trigger { trigger } = c {
                        Some(*trigger)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            Answer::Order {
                order: (candidates)
                    .into_iter()
                    .map(|trigger| r.test_index(Candidate::Trigger { trigger }))
                    .collect(),
            }
        }
        found @ Decision {
            reason: Reason::Discard,
            ..
        } => {
            let count = found.required_count();
            let candidates = found.objects_offered();
            r.pick_objects(candidates.into_iter().take(count).collect())
        }
        other => panic!("unexpected passive request {other:?}"),
    };
    g.respond(player, decision_id, response).unwrap();
}
pub(super) fn to_step(g: &mut Game, number: u64, step: TurnStep) {
    for _ in 0..500 {
        let _ = settle(g);
        if g.state.rules.turn.number == number && g.state.rules.turn.step == step {
            return;
        }
        passive(g);
    }
    panic!("missing step {number}/{step:?}")
}
fn view(g: &Game, player: u8) -> Observation {
    let QueryResult::Complete(QueryValue::Observation(o)) =
        g.observe(PlayerId(player), Budget::unlimited())
    else {
        panic!("observation")
    };
    o
}

pub(super) fn placed(g: &mut Game) {
    to_step(g, 1, TurnStep::DeclareAttackers);
    let source = g.object_id(CardId(0)).unwrap();
    answer(
        g,
        g.next_test_decision().attack(vec![(source, PlayerId(1))]),
    );
    for _ in 0..100 {
        let r = settle(g);
        if !g.state.rules.stack.is_empty() {
            assert!(matches!(
                r,
                Decision {
                    reason: Reason::Priority,
                    ..
                }
            ));
            return;
        }
        passive(g);
    }
    panic!("missing Locke Cole trigger");
}
pub(super) fn begin_resolution(g: &mut Game) -> Decision<Rules> {
    for _ in 0..g.live_players().len() {
        answer(
            g,
            Answer::Pick {
                picks: vec![vec![0]],
            },
        );
    }
    settle(g)
}

fn discard(g: &mut Game) -> Decision<Rules> {
    placed(g);
    let r = begin_resolution(g);
    assert!(matches!(
        r,
        Decision {player:PlayerId(0),reason:Reason::ResolveDiscard,..} if r.required_count()==1
    ));
    r
}
fn finish(g: &mut Game) {
    let _ = settle(g);
    assert!(g.state.rules.stack.is_empty());
}
fn hand(g: &Game, p: u8) -> usize {
    g.state
        .rules
        .objects
        .values()
        .filter(|o| o.zone == (Zone::Hand { owner: PlayerId(p) }))
        .count()
}
#[test]
fn damage_controller_discard_draws_before_private_controller_choice() {
    let mode = TraceMode::Detailed;
    let mut g = Game::with_trace(prepared(false, false), mode);
    let r = discard(&mut g);
    assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), 2);
    // Complete source includes lifelink: its actual two combat damage
    // gains two life before the triggered draw/discard program resolves.
    assert_eq!(g.state.rules.players[0].life, 42.into());
    assert_eq!(g.state.rules.players[1].life, 38.into());
    assert_eq!(hand(&g, 0), 2);
    assert_eq!(hand(&g, 1), 1);
    assert!(
        matches!(g.state.rules.frame,Frame::ResolutionChoice {ref frame,..} if frame.cursor==1 && matches!(frame.outcomes.as_slice(),[InstructionOutcome::Drawn {player:PlayerId(0),failed:false,movement:Some(_)}]))
    );

    let own = view(&g, 0).decision.unwrap();
    let found @ Decision {
        reason: Reason::ResolveDiscard,
        ..
    } = &own
    else {
        panic!("controller discard");
    };
    let candidates = found.objects_offered();
    assert_eq!(candidates.len(), 2);
    assert!(view(&g, 1).decision.is_none());
    let chosen = candidates
        .iter()
        .find(|h| g.resolve_handle(PlayerId(0), h).unwrap().card == CardId(1))
        .unwrap()
        .clone();
    for h in candidates {
        let object = g.resolve_handle(PlayerId(0), &h).unwrap();
        assert_eq!(
            g.current(object).unwrap().zone,
            Zone::Hand { owner: PlayerId(0) }
        );
    }
    let object = g.object_id(CardId(1)).unwrap();
    let foreign = g.object_id(CardId(2)).unwrap();
    let state = g.state.clone();
    let trace = g.trace.clone();
    for (player, decision, card) in [
        (PlayerId(1), r.id, object),
        (PlayerId(0), r.id.wrapping_add(1), object),
        (PlayerId(0), r.id, foreign),
    ] {
        assert!(
            g.respond(
                player,
                decision,
                g.next_test_decision().pick_objects(vec![card])
            )
            .is_err()
        );
    }
    let foreign_handle = g.handle_for(PlayerId(1), foreign).unwrap();
    assert!(matches!(
        g.check_player_response(
            PlayerId(0),
            own.id.clone(),
            g.next_test_viewer_decision()
                .pick_objects(vec![foreign_handle]),
            Budget::unlimited()
        ),
        QueryResult::Rejected(_)
    ));
    assert_eq!(g.state, state);
    assert_eq!(g.trace, trace);
    let QueryResult::Complete(QueryValue::Validated(receipt)) = g.check_player_response(
        PlayerId(0),
        own.id.clone(),
        g.next_test_viewer_decision().pick_objects(vec![chosen]),
        Budget::unlimited(),
    ) else {
        panic!("valid controller discard");
    };
    g.submit(receipt).unwrap();

    let _ = g.advance(Budget::new(1));
    assert_eq!(
        g.state.rules.objects[&CardId(1)].zone,
        Zone::Graveyard { owner: PlayerId(0) }
    );

    finish(&mut g);
    assert_eq!(hand(&g, 0), 1);
    assert_eq!(hand(&g, 1), 1);
    assert!(g.is_live(PlayerId(0)));
}
#[test]
fn damage_controller_discard_empty_library_still_discards_before_failed_draw_sba() {
    let mut g = Game::with_trace(prepared(true, false), TraceMode::Detailed);
    discard(&mut g);
    assert!(g.state.rules.players[0].failed_draw);
    assert!(g.is_live(PlayerId(0)));
    assert_eq!(hand(&g, 0), 1);
    assert!(
        matches!(g.state.rules.frame,Frame::ResolutionChoice {ref frame,..} if matches!(frame.outcomes.as_slice(),[InstructionOutcome::Drawn {player:PlayerId(0),failed:true,movement:None}]))
    );

    let object = g.object_id(CardId(1)).unwrap();
    {
        let selected_answer = g.next_test_decision().pick_objects(vec![object]);
        answer(&mut g, selected_answer)
    };
    let _ = g.advance(Budget::new(1));
    assert!(g.is_live(PlayerId(0)));
    assert_eq!(
        g.state.rules.objects[&CardId(1)].zone,
        Zone::Graveyard { owner: PlayerId(0) }
    );
    assert!(
        matches!(g.state.rules.frame,Frame::Resolve {ref frame} if frame.cursor==2 && matches!(frame.outcomes.last(),Some(InstructionOutcome::Discarded {player:PlayerId(0),movements}) if movements.len()==1))
    );

    finish(&mut g);
    assert!(!g.is_live(PlayerId(0)));
    assert!(g.events().iter().any(|e|matches!(e,Event::PlayersLost {losses} if losses.contains(&(PlayerId(0),PlayerLossReason::FailedDraw)))));
}
#[test]
fn damage_controller_discard_hidden_commander_draw_replacement_resumes_at_discard() {
    for to_command in [false, true] {
        let mut g = Game::new(prepared(false, true));
        placed(&mut g);
        let return_revision = g.next_test_decision().id;
        assert!(matches!(
            begin_resolution(&mut g),
            Decision {
                player: PlayerId(0),
                reason: Reason::CommanderReturn {
                    destination: Zone::Hand { owner: PlayerId(0) },
                    ..
                },
                ..
            }
        ));
        assert!(matches!(
            view(&g, 0).decision,
            Some(Decision {
                reason: Reason::CommanderReturn { object: None, .. },
                ..
            })
        ));
        for viewer in 0..4 {
            assert!(
                !view(&g, viewer)
                    .view
                    .objects
                    .iter()
                    .any(|o| o.oracle_id == ISAMARU)
            );
        }
        let next = g.state.rules.libraries[&PlayerId(0)][1];

        {
            let selected_answer = g.next_test_decision().select(Candidate::Flag {
                flag: if to_command {
                    Flag::CommandZone
                } else {
                    Flag::Destination
                },
            });
            answer(&mut g, selected_answer)
        };
        assert!(matches!(
            settle(&mut g),
            Decision {
                player: PlayerId(0),
                reason: Reason::ResolveDiscard,
                ..
            }
        ));
        assert!(
            matches!(g.state.rules.frame,Frame::ResolutionChoice {ref frame,..} if frame.cursor==1)
        );
        assert_eq!(g.state.rules.libraries[&PlayerId(0)].first(), Some(&next));
        assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), 3);
        assert_eq!(hand(&g, 0), if to_command { 1 } else { 2 });

        let object = g.object_id(CardId(1)).unwrap();
        {
            let selected_answer = g.next_test_decision().pick_objects(vec![object]);
            answer(&mut g, selected_answer)
        };
        finish(&mut g);
        assert_eq!(
            g.state.rules.objects[&CardId(3)].zone,
            if to_command {
                Zone::Command { owner: PlayerId(0) }
            } else {
                Zone::Hand { owner: PlayerId(0) }
            }
        );
        assert_eq!(g.state.rules.libraries[&PlayerId(0)].first(), Some(&next));
        assert_eq!(hand(&g, 0), usize::from(!to_command));
        assert_eq!(
            g.replay_journal()
                .decisions
                .iter()
                .filter(|i| i.decision_id == return_revision)
                .count(),
            1
        );
    }
}
#[test]
fn damage_controller_discard_departures_keep_frozen_controller_and_never_redirect() {
    for departing in [0, 1, 2] {
        let mut g = Game::new(prepared(false, false));
        discard(&mut g);
        g.concede(PlayerId(departing)).unwrap();
        if departing == 0 {
            finish(&mut g);
            assert!(!g.is_live(PlayerId(0)));
            assert_eq!(hand(&g, 1), 1);
            assert_eq!(hand(&g, 3), 0);
        } else {
            assert!(matches!(
                settle(&mut g),
                Decision {
                    player: PlayerId(0),
                    reason: Reason::ResolveDiscard,
                    ..
                }
            ));

            let object = g.object_id(CardId(1)).unwrap();
            {
                let selected_answer = g.next_test_decision().pick_objects(vec![object]);
                answer(&mut g, selected_answer)
            };
            finish(&mut g);
            assert_eq!(hand(&g, 0), 1);
            if departing == 2 {
                assert!(g.object_id(CardId(0)).is_none());
            }
        }
    }
}
#[test]
fn damage_controller_discard_choice_binding_and_host_search_admission_remain_strict() {
    let p = prepared(false, false);
    for (search, hosted, feature) in [
        (true, false, Feature::Search),
        (false, true, Feature::Hosting),
    ] {
        let mut profile = match_profile("commander-foundations-v1").unwrap().clone();
        profile.search = search;
        profile.hosted = hosted;
        assert!(
            matches!(profile.validate_requested_features(&p.catalog),Err(ProfileError::UnreviewedCapabilities {feature:f,capabilities}) if f==feature&&capabilities.contains(&Capability::DamageControllerDiscardV1))
        );
    }
    let mut g = Game::new(p);
    discard(&mut g);
    let mut forged: Snapshot = strict_read(&g.snapshot().unwrap()).unwrap();
    let mut frame = serde_json::to_value(&forged.state.rules.frame).unwrap();
    let picker = frame
        .pointer_mut("/task/stage/remaining/0")
        .expect("discard chooser");
    *picker = serde_json::json!(1);
    forged.state.rules.frame = serde_json::from_value(frame).unwrap();
    let mut structural = g.fork();
    structural.state = forged.state.clone();
    assert!(structural.validate_state().is_err());
    forged.state_sha256 = digest(&forged.state).unwrap();
    assert!(
        Game::restore(
            g.prepared.as_ref().clone(),
            &serde_json::to_string(&forged).unwrap()
        )
        .is_err()
    );
}
