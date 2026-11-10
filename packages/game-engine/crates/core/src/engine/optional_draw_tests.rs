//! Complete source-lowered Neurok Commando; the optional choice is made only
//! during resolution and shares ordinary draw/Commander replacement behavior.
use super::*;
const COMMANDO: &str = "48936751-429b-4c40-88f5-72acde6278cc";
const SWAMP: &str = "56719f6a-1a6c-4c0a-8d21-18f7d7350b68";
const ISAMARU: &str = "cfb756dd-6fe1-430a-8f04-8b10cac62a86";
fn prepared(empty: bool, commander: bool) -> Prepared {
    let mut c = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap();
    let mtg_compiler::CompileResult::Lowered { definition } = mtg_compiler::compile(
        include_str!("../../../../tests/fixtures/cards-data-d2ce/archive-manifest-current.json"),
        include_str!(
            "../../../../tests/fixtures/cards-data-d2ce/48936751-429b-4c40-88f5-72acde6278cc.json"
        ),
    ) else {
        panic!("complete Neurok Commando source must lower")
    };
    assert!(definition.keywords.contains(&KeywordAbility::Shroud));
    c.definitions.push(*definition);
    c.schema_version = 27;
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    let mut config = ScenarioConfig::turns_permanents();
    config.profile = "optional-draw-domain-witness".into();
    config.initial.turn_step = TurnStep::PrecombatMain;
    config.cards = vec![CardSetup {
        id: CardId(0),
        oracle_id: COMMANDO.into(),
        owner: PlayerId(2),
        controller: PlayerId(0),
        zone: Zone::Battlefield,
    }];
    if commander {
        config.cards.push(CardSetup {
            id: CardId(1),
            oracle_id: ISAMARU.into(),
            owner: PlayerId(0),
            controller: PlayerId(0),
            zone: Zone::Library { owner: PlayerId(0) },
        });
        config.commanders.push(CardId(1));
    }
    for seat in 0..4 {
        for _ in 0..if seat == 0 && empty { 0 } else { 3 } {
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
fn modes() -> [TraceMode; 2] {
    [TraceMode::Off, TraceMode::Detailed]
}
fn view(g: &Game, player: u8) -> Observation {
    let QueryResult::Complete(QueryValue::Observation(o)) =
        g.observe(PlayerId(player), Budget::unlimited())
    else {
        panic!("observation")
    };
    o
}
fn optional(g: &mut Game) -> Request {
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
            assert!(
                matches!(r, Request::Priority { .. }),
                "trigger must be placed before option"
            );
            break;
        }
        passive(g);
    }
    assert!(matches!(g.top().unwrap().kind, StackKind::Triggered { .. }));
    recover(g);
    for _ in 0..g.live_players().len() {
        answer(g, Response::Pass);
    }
    let r = settle(g);
    assert!(matches!(
        r,
        Request::OptionalEffect {
            player: PlayerId(0),
            ..
        }
    ));
    assert!(matches!(
        g.state.rules.frame,
        Frame::ResolutionChoice {
            choice: ResolutionChoice::OptionalDraw { .. },
            ..
        }
    ));
    r
}
fn finish(g: &mut Game) {
    let _ = settle(g);
    assert!(g.state.rules.stack.is_empty());
}
#[test]
fn optional_draw_exact_source_accept_decline_private_request_and_recovery() {
    for mode in modes() {
        for perform in [false, true] {
            let mut g = Game::with_trace(prepared(false, false), mode);
            let r = optional(&mut g);
            recover(&g);
            let own = view(&g, 0);
            assert!(matches!(
                own.decision,
                Some(PlayerRequest::OptionalEffect { .. })
            ));
            assert_eq!(own.view.priority, None);
            assert_eq!(own.view.players[0].library_count, 3);
            assert!(view(&g, 1).decision.is_none());
            let serialized = serde_json::to_value(own.decision).unwrap();
            assert!(serialized.get("objects").is_none() && serialized.get("candidates").is_none());
            let state = g.state.clone();
            let trace = g.trace.clone();
            assert!(
                g.respond(
                    PlayerId(1),
                    r.decision_id(),
                    Response::OptionalEffect { perform }
                )
                .is_err()
            );
            assert!(
                g.respond(
                    PlayerId(0),
                    r.decision_id().wrapping_add(1),
                    Response::OptionalEffect { perform }
                )
                .is_err()
            );
            assert_eq!(g.state, state);
            assert_eq!(g.trace, trace);
            answer(&mut g, Response::OptionalEffect { perform });
            recover(&g);
            finish(&mut g);
            assert_eq!(
                g.state.rules.libraries[&PlayerId(0)].len(),
                if perform { 2 } else { 3 }
            );
            assert_eq!(
                g.state
                    .rules
                    .objects
                    .values()
                    .filter(|o| o.zone == (Zone::Hand { owner: PlayerId(0) }))
                    .count(),
                usize::from(perform)
            );
            assert!(!g.state.rules.players[0].failed_draw);
            recover(&g);
        }
    }
}
#[test]
fn optional_draw_empty_library_decline_avoids_failed_draw_accept_uses_normal_sba() {
    for perform in [false, true] {
        let mut g = Game::with_trace(prepared(true, false), TraceMode::Detailed);
        optional(&mut g);
        recover(&g);
        answer(&mut g, Response::OptionalEffect { perform });
        finish(&mut g);
        assert_eq!(g.is_live(PlayerId(0)), !perform);
        assert_eq!(g.state.rules.players[0].failed_draw, perform);
        assert_eq!(g.events().iter().filter(|e|matches!(e,Event::PlayersLost {losses} if losses.contains(&(PlayerId(0),PlayerLossReason::FailedDraw)))).count(),usize::from(perform));
        recover(&g);
    }
}
#[test]
fn optional_draw_hidden_commander_replacement_preserves_next_card_and_accepted_option() {
    for to_command in [false, true] {
        let mut g = Game::with_trace(prepared(false, true), TraceMode::Detailed);
        optional(&mut g);
        answer(&mut g, Response::OptionalEffect { perform: true });
        let r = settle(&mut g);
        assert!(matches!(
            r,
            Request::CommanderReturn {
                player: PlayerId(0),
                destination: Zone::Hand { owner: PlayerId(0) },
                ..
            }
        ));
        assert!(matches!(
            view(&g, 0).decision,
            Some(PlayerRequest::CommanderReturn { object: None, .. })
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
        recover(&g);
        let before = g.state.clone();
        assert!(
            g.respond(
                PlayerId(0),
                r.decision_id(),
                Response::OptionalEffect { perform: true }
            )
            .is_err()
        );
        assert_eq!(g.state, before);
        // Rehashed snapshot with a declined accepted input cannot authorize a
        // Commander continuation that exists only after affirmative acceptance.
        let mut forged: serde_json::Value = serde_json::from_str(&g.snapshot().unwrap()).unwrap();
        let input = forged["state"]["journal"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .rev()
            .find(|i| i["response"]["response"] == "optional_effect")
            .unwrap();
        input["response"]["perform"] = false.into();
        forged["state_sha256"] = digest(&forged["state"]).unwrap().into();
        assert!(Game::restore(g.prepared.as_ref().clone(), &forged.to_string()).is_err());
        answer(&mut g, Response::CommanderReturn { to_command });
        finish(&mut g);
        assert_eq!(g.state.rules.libraries[&PlayerId(0)].first(), Some(&next));
        assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), 3);
        assert_eq!(
            g.state.rules.objects[&CardId(1)].zone,
            if to_command {
                Zone::Command { owner: PlayerId(0) }
            } else {
                Zone::Hand { owner: PlayerId(0) }
            }
        );
        assert_eq!(
            g.replay_journal()
                .decisions
                .iter()
                .filter(|i| matches!(i.response, Response::OptionalEffect { perform: true }))
                .count(),
            1
        );
        recover(&g);
    }
}
#[test]
fn optional_draw_owner_departure_preserves_surviving_invocation_controller() {
    for mode in modes() {
        let mut g = Game::with_trace(prepared(false, false), mode);
        optional(&mut g);
        g.concede(PlayerId(2)).unwrap();
        assert!(matches!(
            settle(&mut g),
            Request::OptionalEffect {
                player: PlayerId(0),
                ..
            }
        ));
        assert!(g.object_id(CardId(0)).is_none());
        recover(&g);
        answer(&mut g, Response::OptionalEffect { perform: true });
        finish(&mut g);
        assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), 2);
        recover(&g);
    }
}
#[test]
fn optional_draw_departed_controller_skips_option_without_redirecting_draw() {
    let mut g = Game::with_trace(prepared(false, false), TraceMode::Detailed);
    optional(&mut g);
    let others =
        [PlayerId(1), PlayerId(2), PlayerId(3)].map(|p| (p, g.state.rules.libraries[&p].clone()));
    g.concede(PlayerId(0)).unwrap();
    finish(&mut g);
    assert!(!g.is_live(PlayerId(0)));
    assert!(!g.state.rules.players[0].failed_draw);
    for (player, library) in others {
        assert_eq!(g.state.rules.libraries[&player], library);
    }
    assert!(
        !g.replay_journal()
            .decisions
            .iter()
            .any(|i| { matches!(i.response, Response::OptionalEffect { .. }) })
    );
    recover(&g);
}

#[test]
fn optional_draw_hosting_search_remain_excluded() {
    let p = prepared(false, false);
    for (search, hosted, feature) in [
        (true, false, Feature::Search),
        (false, true, Feature::Hosting),
    ] {
        let mut profile = match_profile("commander-foundations-v1").unwrap().clone();
        profile.search = search;
        profile.hosted = hosted;
        assert!(
            matches!(profile.validate_requested_features(&p.catalog),Err(ProfileError::UnreviewedCapabilities {feature:f,capabilities}) if f==feature&&capabilities.contains(&Capability::OptionalControllerDrawV1))
        );
    }
}
