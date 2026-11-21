//! Exact-source Demon of Loathing: the actual damaged player chooses a
//! controlled creature; ordinary sacrifice, death triggers and Commander SBAs
//! preserve its true owner and the frozen invocation/recipient identities.
use super::*;
const DEMON: &str = "2e55fc03-779c-4a71-beb7-aa0ade738c24";
const SWAMP: &str = "56719f6a-1a6c-4c0a-8d21-18f7d7350b68";
const BEARS: &str = "14c8f55d-d177-4c25-a931-ebeb9e6062a0";
const ARTIST: &str = "310f141c-7f37-4729-aed6-dd9c09db448d";
const ISAMARU: &str = "cfb756dd-6fe1-430a-8f04-8b10cac62a86";
fn prepared(empty: bool, commander: bool, artist: bool) -> Prepared {
    let mut c = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap();
    let mtg_compiler::CompileResult::Lowered { definition } = mtg_compiler::compile(
        include_str!("../../../../tests/fixtures/cards-data-d2ce/archive-manifest-current.json"),
        include_str!(
            "../../../../tests/fixtures/cards-data-d2ce/2e55fc03-779c-4a71-beb7-aa0ade738c24.json"
        ),
    ) else {
        panic!("complete Demon of Loathing source must lower");
    };
    c.definitions.push(*definition);
    c.schema_version = mtg_contracts::CURRENT_IR_VERSION;
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    let mut config = ScenarioConfig::turns_permanents();
    config.profile = "damage-recipient-sacrifice-domain-witness".into();
    config.initial.turn_step = TurnStep::PrecombatMain;
    config.cards = vec![
        CardSetup {
            id: CardId(0),
            oracle_id: DEMON.into(),
            owner: PlayerId(2),
            controller: PlayerId(0),
            zone: Zone::Battlefield,
        },
        CardSetup {
            id: CardId(1),
            oracle_id: if commander { ISAMARU } else { BEARS }.into(),
            owner: PlayerId(3),
            controller: PlayerId(1),
            zone: if empty {
                Zone::Graveyard { owner: PlayerId(3) }
            } else {
                Zone::Battlefield
            },
        },
        CardSetup {
            id: CardId(2),
            oracle_id: BEARS.into(),
            owner: PlayerId(0),
            controller: PlayerId(0),
            zone: Zone::Battlefield,
        },
    ];
    if commander {
        config.commanders.push(CardId(1));
    }
    if artist {
        config.cards.push(CardSetup {
            id: CardId(3),
            oracle_id: ARTIST.into(),
            owner: PlayerId(1),
            controller: PlayerId(1),
            zone: Zone::Battlefield,
        });
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

fn sacrifice(g: &mut Game) -> Request {
    placed(g);
    let r = begin_resolution(g);
    assert!(matches!(
        r,
        Request::Sacrifice {
            player: PlayerId(1),
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
fn event_player_sacrifice_exact_source_controlled_choice_true_owner_and_replay() {
    for mode in [TraceMode::Off, TraceMode::Detailed] {
        let mut g = Game::with_trace(prepared(false, false, false), mode);
        let r = sacrifice(&mut g);
        recover(&g);
        let own = view(&g, 1).decision.unwrap();
        let PlayerRequest::Sacrifice { candidates, .. } = &own else {
            panic!("sacrifice");
        };
        assert_eq!(candidates.len(), 1);
        assert!(view(&g, 0).decision.is_none());
        let chosen = g.resolve_handle(PlayerId(1), &candidates[0]).unwrap();
        assert_eq!(chosen.card, CardId(1));
        let foreign = g.object_id(CardId(2)).unwrap();
        let state = g.state.clone();
        let trace = g.trace.clone();
        for (player, decision, object) in [
            (PlayerId(0), r.decision_id(), chosen),
            (PlayerId(1), r.decision_id().wrapping_add(1), chosen),
            (PlayerId(1), r.decision_id(), foreign),
        ] {
            assert!(
                g.respond(player, decision, Response::Sacrifice { object })
                    .is_err()
            );
        }
        assert_eq!(g.state, state);
        assert_eq!(g.trace, trace);
        let QueryResult::Complete(QueryValue::Validated(receipt)) = g.check_player_response(
            PlayerId(1),
            own.token().clone(),
            PlayerResponse::Sacrifice {
                object: candidates[0].clone(),
            },
            Budget::unlimited(),
        ) else {
            panic!("valid sacrifice");
        };
        g.submit(receipt).unwrap();
        recover(&g);
        let _ = g.advance(Budget::new(1));
        assert_eq!(
            g.state.rules.objects[&CardId(1)].zone,
            Zone::Graveyard { owner: PlayerId(3) }
        );
        assert!(
            matches!(g.state.rules.frame,Frame::Resolve {ref frame} if frame.cursor==1 && matches!(frame.outcomes.as_slice(),[InstructionOutcome::Sacrificed {player:PlayerId(1),movement:Some(_)}]))
        );
        recover(&g);
        finish(&mut g);
        recover(&g);
    }
}
#[test]
fn event_player_sacrifice_empty_domain_and_victim_departure_skip_without_redirection() {
    for case in ["empty", "before_resolution", "at_choice"] {
        let mut g = Game::new(prepared(case == "empty", false, false));
        placed(&mut g);
        if case == "before_resolution" {
            g.concede(PlayerId(1)).unwrap();
            let _ = settle(&mut g);
        }
        let r = begin_resolution(&mut g);
        if case == "at_choice" {
            assert!(matches!(r, Request::Sacrifice { .. }));
            g.concede(PlayerId(1)).unwrap();
            finish(&mut g);
        } else {
            assert!(matches!(r, Request::Priority { .. }));
        }
        assert!(g.state.rules.stack.is_empty());
        assert_eq!(g.state.rules.objects[&CardId(2)].zone, Zone::Battlefield);
        assert_eq!(
            g.state.rules.objects[&CardId(1)].zone,
            if case == "empty" {
                Zone::Graveyard { owner: PlayerId(3) }
            } else {
                Zone::Exile
            }
        );
        recover(&g);
    }
}
#[test]
fn event_player_sacrifice_owner_and_started_controller_departure_preserve_victim() {
    for departing in [0, 2] {
        let mut g = Game::new(prepared(false, false, false));
        sacrifice(&mut g);
        g.concede(PlayerId(departing)).unwrap();
        assert!(matches!(
            settle(&mut g),
            Request::Sacrifice {
                player: PlayerId(1),
                ..
            }
        ));
        recover(&g);
        let chosen = g.object_id(CardId(1)).unwrap();
        answer(&mut g, Response::Sacrifice { object: chosen });
        finish(&mut g);
        assert_eq!(
            g.state.rules.objects[&CardId(1)].zone,
            Zone::Graveyard { owner: PlayerId(3) }
        );
        recover(&g);
    }
}
#[test]
fn event_player_sacrifice_uses_normal_commander_sba_and_death_trigger_paths() {
    for commander in [false, true] {
        let mut g = Game::new(prepared(false, commander, !commander));
        sacrifice(&mut g);
        let victim_life = g.state.rules.players[1].life.clone();
        let chosen = g.object_id(CardId(1)).unwrap();
        answer(&mut g, Response::Sacrifice { object: chosen });
        let _ = g.advance(Budget::new(1));
        assert_eq!(
            g.state.rules.objects[&CardId(1)].zone,
            Zone::Graveyard { owner: PlayerId(3) }
        );
        recover(&g);
        if commander {
            assert!(matches!(
                settle(&mut g),
                Request::CommanderReturn {
                    player: PlayerId(3),
                    ..
                }
            ));
            recover(&g);
            answer(&mut g, Response::CommanderReturn { to_command: true });
            finish(&mut g);
            assert_eq!(
                g.state.rules.objects[&CardId(1)].zone,
                Zone::Command { owner: PlayerId(3) }
            );
        } else {
            assert_eq!(g.state.rules.pending_triggers.len(), 1);
            assert!(matches!(
                settle(&mut g),
                Request::PlayerTargets {
                    player: PlayerId(1),
                    ..
                }
            ));
            answer(
                &mut g,
                Response::PlayerTargets {
                    players: vec![PlayerId(0)],
                },
            );
            let _ = settle(&mut g);
            assert_eq!(g.top().unwrap().source.card, CardId(3));
            let _ = begin_resolution(&mut g);
            assert!(g.state.rules.stack.is_empty());
            let mut gained = victim_life;
            gained.add(&1.into());
            assert_eq!(g.state.rules.players[1].life, gained);
            assert_eq!(g.state.rules.players[0].life, 39.into());
        }
        recover(&g);
    }
}
#[test]
fn event_player_sacrifice_coherent_recipient_forgery_cannot_authorize_recovery() {
    let mut g = Game::new(prepared(false, false, false));
    sacrifice(&mut g);
    recover(&g);
    let mut forged: Snapshot = strict_read(&g.snapshot().unwrap()).unwrap();
    let Frame::ResolutionChoice {
        frame,
        choice: ResolutionChoice::Sacrifice { player, .. },
    } = &mut forged.state.rules.frame
    else {
        panic!("sacrifice");
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
fn event_player_sacrifice_hosting_search_remain_excluded() {
    let p = prepared(false, false, false);
    for (search, hosted, feature) in [
        (true, false, Feature::Search),
        (false, true, Feature::Hosting),
    ] {
        let mut profile = match_profile("commander-foundations-v1").unwrap().clone();
        profile.search = search;
        profile.hosted = hosted;
        assert!(
            matches!(profile.validate_requested_features(&p.catalog),Err(ProfileError::UnreviewedCapabilities {feature:f,capabilities}) if f==feature&&capabilities.contains(&Capability::EventPlayerSacrificeV1))
        );
    }
}
