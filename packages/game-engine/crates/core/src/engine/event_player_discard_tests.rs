//! Complete source-lowered Dimir Cutpurse. The damaged player discards from
//! their own private hand before the frozen invocation controller draws.
use super::*;
const CUTPURSE: &str = "56c1a548-c80c-432b-bf92-d4ef6117b298";
const SWAMP: &str = "56719f6a-1a6c-4c0a-8d21-18f7d7350b68";
const FOREST: &str = "b34bb2dc-c1af-4d77-b0b3-a0fb342a5fc6";
const ISAMARU: &str = "cfb756dd-6fe1-430a-8f04-8b10cac62a86";
fn prepared(empty: bool, commander: bool) -> Prepared {
    let mut c = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap();
    let mtg_compiler::CompileResult::Lowered { definition } = mtg_compiler::compile(
        include_str!("../../../../tests/fixtures/cards-data-d2ce/archive-manifest-current.json"),
        include_str!(
            "../../../../tests/fixtures/cards-data-d2ce/56c1a548-c80c-432b-bf92-d4ef6117b298.json"
        ),
    ) else {
        panic!("complete Dimir Cutpurse source must lower")
    };
    c.definitions.push(*definition);
    c.schema_version = 28;
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    let mut config = ScenarioConfig::turns_permanents();
    config.profile = "damage-recipient-discard-domain-witness".into();
    config.initial.turn_step = TurnStep::PrecombatMain;
    config.cards = vec![CardSetup {
        id: CardId(0),
        oracle_id: CUTPURSE.into(),
        owner: PlayerId(2),
        controller: PlayerId(0),
        zone: Zone::Battlefield,
    }];
    for (id, oracle, owner) in [
        (1, if commander { ISAMARU } else { SWAMP }, 1),
        (2, FOREST, 1),
        (3, SWAMP, 0),
    ] {
        config.cards.push(CardSetup {
            id: CardId(id),
            oracle_id: oracle.into(),
            owner: PlayerId(owner),
            controller: PlayerId(owner),
            zone: if empty && owner == 1 {
                Zone::Graveyard {
                    owner: PlayerId(owner),
                }
            } else {
                Zone::Hand {
                    owner: PlayerId(owner),
                }
            },
        });
    }
    if commander {
        config.commanders.push(CardId(1));
    }
    for seat in 0..4 {
        for _ in 0..3 {
            config.cards.push(CardSetup {
                id: CardId(config.cards.len() as u32),
                oracle_id: SWAMP.into(),
                owner: PlayerId(seat),
                controller: PlayerId(seat),
                zone: Zone::Library {
                    owner: PlayerId(seat),
                },
            });
        }
    }
    Prepared {
        catalog_digest: digest(&c).unwrap(),
        catalog: Arc::new(c),
        profile_digest: digest(&config).unwrap(),
        config,
        match_config: None,
    }
}
fn settle(g: &mut Game) -> Request {
    for _ in 0..500 {
        match g.advance(Budget::new(1)) {
            Advance::AwaitingDecision(r) => return r,
            Advance::Suspended { .. } => (),
            other => panic!("step witness at {:?}: {other:?}", g.state.rules.frame),
        }
    }
    panic!("step witness budget")
}
fn answer(g: &mut Game, response: Response) {
    let r = settle(g);
    g.respond(r.player(), r.decision_id(), response).unwrap();
}
fn passive(g: &mut Game) {
    let r = settle(g);
    let (player, decision_id) = (r.player(), r.decision_id());
    let response = match r {
        Request::Priority { .. } => Response::Pass,
        Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
        Request::Blockers { .. } => Response::Blockers { blocks: vec![] },
        Request::OrderTriggers { candidates, .. } => Response::OrderTriggers { order: candidates },
        Request::Discard {
            count, candidates, ..
        } => Response::Discard {
            objects: candidates.into_iter().take(count).collect(),
        },
        other => panic!("unexpected passive request {other:?}"),
    };
    g.respond(player, decision_id, response).unwrap();
}
fn recover(g: &Game) {
    let restored = Game::restore(g.prepared.as_ref().clone(), &g.snapshot().unwrap()).unwrap();
    assert!(restored.state == g.state);
    assert_eq!(restored.trace, g.trace);
    let replayed = Game::replay_to(
        g.prepared.as_ref().clone(),
        &g.replay_journal(),
        Some(g.state.rules.rule_steps),
        g.trace.mode,
    )
    .unwrap();
    assert!(replayed.state == g.state);
    assert_eq!(replayed.trace, g.trace);
}
fn to_step(g: &mut Game, number: u64, step: TurnStep) {
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

fn placed(g: &mut Game) {
    to_step(g, 1, TurnStep::DeclareAttackers);
    let source = g.object_id(CardId(0)).unwrap();
    answer(
        g,
        Response::Attackers {
            attacks: vec![(source, PlayerId(1))],
        },
    );
    for _ in 0..100 {
        let r = settle(g);
        if !g.state.rules.stack.is_empty() {
            assert!(matches!(r, Request::Priority { .. }));
            return;
        }
        passive(g);
    }
    panic!("missing Cutpurse trigger");
}
fn begin_resolution(g: &mut Game) -> Request {
    for _ in 0..g.live_players().len() {
        answer(g, Response::Pass);
    }
    settle(g)
}
fn discard(g: &mut Game) -> Request {
    placed(g);
    let r = begin_resolution(g);
    assert!(matches!(
        r,
        Request::ResolveDiscard {
            player: PlayerId(1),
            count: 1,
            ..
        }
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
fn event_player_discard_exact_source_private_hand_ordered_draw_and_replay() {
    for mode in [TraceMode::Off, TraceMode::Detailed] {
        let mut g = Game::with_trace(prepared(false, false), mode);
        let r = discard(&mut g);
        recover(&g);
        let own = view(&g, 1).decision.unwrap();
        let PlayerRequest::ResolveDiscard {
            candidates, count, ..
        } = &own
        else {
            panic!("private discard");
        };
        assert_eq!(*count, 1);
        assert_eq!(candidates.len(), 2);
        assert!(view(&g, 0).decision.is_none());
        assert!(view(&g, 2).decision.is_none());
        for h in candidates {
            let object = g.resolve_handle(PlayerId(1), h).unwrap();
            assert_eq!(
                g.current(object).unwrap().zone,
                Zone::Hand { owner: PlayerId(1) }
            );
        }
        let chosen = g.resolve_handle(PlayerId(1), &candidates[0]).unwrap();
        let foreign = g.object_id(CardId(3)).unwrap();
        let state = g.state.clone();
        let trace = g.trace.clone();
        for (player, decision, object) in [
            (PlayerId(0), r.decision_id(), chosen),
            (PlayerId(1), r.decision_id().wrapping_add(1), chosen),
            (PlayerId(1), r.decision_id(), foreign),
        ] {
            assert!(
                g.respond(
                    player,
                    decision,
                    Response::ResolveDiscard {
                        objects: vec![object]
                    }
                )
                .is_err()
            );
        }
        let foreign_handle = g.handle_for(PlayerId(0), foreign).unwrap();
        assert!(matches!(
            g.check_player_response(
                PlayerId(1),
                own.token().clone(),
                PlayerResponse::ResolveDiscard {
                    objects: vec![foreign_handle]
                },
                Budget::unlimited()
            ),
            QueryResult::Rejected(_)
        ));
        assert_eq!(g.state, state);
        assert_eq!(g.trace, trace);
        let QueryResult::Complete(QueryValue::Validated(receipt)) = g.check_player_response(
            PlayerId(1),
            own.token().clone(),
            PlayerResponse::ResolveDiscard {
                objects: vec![candidates[0].clone()],
            },
            Budget::unlimited(),
        ) else {
            panic!("valid private discard");
        };
        g.submit(receipt).unwrap();
        recover(&g);
        let _ = g.advance(Budget::new(1));
        assert_eq!(
            g.state.rules.objects[&chosen.card].zone,
            Zone::Graveyard { owner: PlayerId(1) }
        );
        assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), 3);
        assert!(matches!(g.state.rules.frame,Frame::Resolve {ref frame} if frame.cursor==1));
        recover(&g);
        finish(&mut g);
        assert_eq!(hand(&g, 1), 1);
        assert_eq!(hand(&g, 0), 2);
        assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), 2);
        recover(&g);
    }
}
#[test]
fn event_player_discard_empty_hand_and_victim_departure_still_draw_for_controller() {
    for case in ["empty", "before_resolution", "at_choice"] {
        let mut g = Game::new(prepared(case == "empty", false));
        placed(&mut g);
        if case == "before_resolution" {
            g.concede(PlayerId(1)).unwrap();
            let _ = settle(&mut g);
        }
        let r = begin_resolution(&mut g);
        if case == "at_choice" {
            assert!(matches!(r, Request::ResolveDiscard { .. }));
            g.concede(PlayerId(1)).unwrap();
            finish(&mut g);
        } else {
            assert!(matches!(r, Request::Priority { .. }));
        }
        assert!(g.state.rules.stack.is_empty());
        assert_eq!(hand(&g, 0), 2);
        assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), 2);
        recover(&g);
    }
}
#[test]
fn event_player_discard_departures_preserve_frozen_victim_and_never_redirect_controller_draw() {
    for departing in [0, 2] {
        let mut g = Game::new(prepared(false, false));
        discard(&mut g);
        g.concede(PlayerId(departing)).unwrap();
        let r = settle(&mut g);
        assert!(matches!(
            r,
            Request::ResolveDiscard {
                player: PlayerId(1),
                ..
            }
        ));
        recover(&g);
        let victim_card = g.object_id(CardId(1)).unwrap();
        answer(
            &mut g,
            Response::ResolveDiscard {
                objects: vec![victim_card],
            },
        );
        finish(&mut g);
        assert_eq!(hand(&g, 1), 1);
        assert_eq!(hand(&g, 3), 0);
        assert_eq!(g.state.rules.libraries[&PlayerId(3)].len(), 3);
        if departing == 2 {
            assert_eq!(hand(&g, 0), 2);
            assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), 2);
        } else {
            assert!(!g.is_live(PlayerId(0)));
            assert_eq!(hand(&g, 0), 0);
        }
        recover(&g);
    }
}
#[test]
fn event_player_discard_hidden_commander_waits_for_controller_draw_before_sba_return() {
    let mut g = Game::new(prepared(false, true));
    discard(&mut g);
    let commander = g.object_id(CardId(1)).unwrap();
    answer(
        &mut g,
        Response::ResolveDiscard {
            objects: vec![commander],
        },
    );
    let _ = g.advance(Budget::new(1));
    assert_eq!(
        g.state.rules.objects[&CardId(1)].zone,
        Zone::Graveyard { owner: PlayerId(1) }
    );
    assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), 3);
    recover(&g);
    assert!(matches!(
        settle(&mut g),
        Request::CommanderReturn {
            player: PlayerId(1),
            ..
        }
    ));
    assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), 2);
    recover(&g);
    answer(&mut g, Response::CommanderReturn { to_command: true });
    finish(&mut g);
    assert_eq!(
        g.state.rules.objects[&CardId(1)].zone,
        Zone::Command { owner: PlayerId(1) }
    );
    recover(&g);
}
#[test]
fn event_player_discard_coherent_recipient_forgery_cannot_authorize_recovery() {
    let mut g = Game::new(prepared(false, false));
    discard(&mut g);
    recover(&g);
    let mut forged: Snapshot = strict_read(&g.snapshot().unwrap()).unwrap();
    let Frame::ResolutionChoice {
        frame,
        choice: ResolutionChoice::Discard { player, .. },
    } = &mut forged.state.rules.frame
    else {
        panic!("discard");
    };
    *player = PlayerId(0);
    let StackKind::Triggered {
        witness: TriggerEventWitness::DamageDealt { damage, .. },
        ..
    } = &mut frame.spell.kind
    else {
        panic!("witness");
    };
    damage.recipient = DamageRecipient::Player {
        player: PlayerId(0),
    };
    *forged.state.rules.stack.last_mut().unwrap() = frame.spell.clone();
    let mut structural = g.fork();
    structural.state = forged.state.clone();
    structural.validate_trigger_state().unwrap();
    structural.validate_actions_state().unwrap();
    if let Frame::ResolutionChoice { frame, .. } = &structural.state.rules.frame {
        structural.validate_resolving(frame).unwrap();
    }
    forged.state_sha256 = digest(&forged.state).unwrap();
    assert!(
        Game::restore(
            g.prepared.as_ref().clone(),
            &serde_json::to_string(&forged).unwrap()
        )
        .is_err()
    );
}
#[test]
fn event_player_discard_hosting_search_remain_excluded() {
    let p = prepared(false, false);
    for (search, hosted, feature) in [
        (true, false, Feature::Search),
        (false, true, Feature::Hosting),
    ] {
        let mut profile = match_profile("commander-foundations-v1").unwrap().clone();
        profile.search = search;
        profile.hosted = hosted;
        assert!(
            matches!(profile.validate_requested_features(&p.catalog),Err(ProfileError::UnreviewedCapabilities {feature:f,capabilities}) if f==feature&&capabilities.contains(&Capability::EventPlayerDiscardV1))
        );
    }
}
