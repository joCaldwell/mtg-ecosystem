//! Complete frozen Fear of Failed Tests, with real combat and source-backed
//! Samite Healer/Furnace effects. No edited card definitions select mechanics.
use super::*;

const FEAR: &str = "34fd461b-a913-4a60-9477-ce2bd5c6fc5a";
const SWAMP: &str = "56719f6a-1a6c-4c0a-8d21-18f7d7350b68";
const HEALER: &str = "95a0ca48-d924-47f4-86ed-42c673ee778c";
const FURNACE: &str = "68715465-6cf9-4006-87e9-31f227fe9ed3";
const ISAMARU: &str = "cfb756dd-6fe1-430a-8f04-8b10cac62a86";

fn prepared(
    fears: usize,
    shields: usize,
    doubled: bool,
    library: usize,
    commander: Option<usize>,
) -> Prepared {
    let mut c = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap();
    let mtg_compiler::CompileResult::Lowered { definition } = mtg_compiler::compile(
        include_str!("../../../../tests/fixtures/cards-data-d2ce/archive-manifest-current.json"),
        include_str!(
            "../../../../tests/fixtures/cards-data-d2ce/34fd461b-a913-4a60-9477-ce2bd5c6fc5a.json"
        ),
    ) else {
        panic!("complete Fear of Failed Tests source must lower");
    };
    assert!(
        definition
            .capabilities
            .contains(&Capability::DamageQuantityDrawV1)
    );
    assert!(definition.characteristics.has_type(CardKind::Enchantment));
    assert!(definition.characteristics.has_type(CardKind::Creature));
    assert_eq!(
        definition.triggered[0].program.instructions,
        vec![Instruction::DrawDamageAmount { source_block: 0 }]
    );
    c.definitions.push(*definition);
    c.schema_version = 32;
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    let mut config = ScenarioConfig::turns_permanents();
    config.profile = "damage-quantity-draw-domain-witness".into();
    config.initial.turn_step = TurnStep::PrecombatMain;
    config.cards.clear();
    let mut push = |oracle: &str, owner: u8, controller: u8, zone: Zone| {
        let id = CardId(config.cards.len() as u32);
        config.cards.push(CardSetup {
            id,
            oracle_id: oracle.into(),
            owner: PlayerId(owner),
            controller: PlayerId(controller),
            zone,
        });
        id
    };
    for _ in 0..fears {
        push(FEAR, 2, 0, Zone::Battlefield);
    }
    for _ in 0..shields {
        push(HEALER, 0, 0, Zone::Battlefield);
    }
    if doubled {
        push(FURNACE, 3, 3, Zone::Battlefield);
    }
    let mut commanders = vec![];
    for seat in 0..4 {
        for index in 0..if seat == 0 { library } else { 3 } {
            let is_commander = seat == 0 && commander == Some(index);
            let id = push(
                if is_commander { ISAMARU } else { SWAMP },
                seat,
                seat,
                Zone::Library {
                    owner: PlayerId(seat),
                },
            );
            if is_commander {
                commanders.push(id);
            }
        }
    }
    config.commanders = commanders;
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
            other => panic!("quantity draw at {:?}: {other:?}", g.state.rules.frame),
        }
    }
    panic!("quantity draw budget");
}
fn answer(g: &mut Game, response: Response) {
    let r = settle(g);
    g.respond(r.player(), r.decision_id(), response).unwrap();
}
fn passive_request(g: &mut Game, r: Request) {
    let (player, id) = (r.player(), r.decision_id());
    let response = match r {
        Request::Priority { .. } => Response::Pass,
        Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
        Request::Blockers { .. } => Response::Blockers { blocks: vec![] },
        Request::OrderTriggers { candidates, .. } => Response::OrderTriggers { order: candidates },
        Request::DamageEffect { candidates, .. } => Response::DamageEffect {
            effect: candidates[0].effect.clone(),
            damage: candidates[0].damage,
        },
        other => panic!("unexpected passive request {other:?}"),
    };
    g.respond(player, id, response).unwrap();
}
fn placed(g: &mut Game) {
    for _ in 0..100 {
        let r = settle(g);
        if !g.state.rules.stack.is_empty() {
            assert!(matches!(r, Request::Priority { .. }));
            return;
        }
        passive_request(g, r);
    }
    panic!("missing trigger");
}
fn begin_resolution(g: &mut Game) {
    for _ in 0..g.live_players().len() {
        answer(g, Response::Pass);
    }
}
fn resolve(g: &mut Game) {
    begin_resolution(g);
    assert!(matches!(settle(g), Request::Priority { .. }));
}
fn declare(g: &mut Game, attacks: &[(u32, u8)]) {
    for _ in 0..100 {
        let r = settle(g);
        if g.state.rules.turn.number == 1 && g.state.rules.turn.step == TurnStep::DeclareAttackers {
            let attacks = attacks
                .iter()
                .map(|(card, player)| (g.object_id(CardId(*card)).unwrap(), PlayerId(*player)))
                .collect();
            g.respond(r.player(), r.decision_id(), Response::Attackers { attacks })
                .unwrap();
            return;
        }
        passive_request(g, r);
    }
    panic!("missing attackers");
}
fn shield(g: &mut Game, card: u32) {
    let object = g.object_id(CardId(card)).unwrap();
    answer(g, Response::ActivateAbility { object, ability: 0 });
    answer(
        g,
        Response::EntityTargets {
            entities: vec![TargetEntity::Player {
                player: PlayerId(1),
            }],
        },
    );
    placed(g);
    resolve(g);
}
fn library(g: &Game) -> usize {
    g.state.rules.libraries[&PlayerId(0)].len()
}
fn hand(g: &Game, player: u8) -> usize {
    g.state
        .rules
        .objects
        .values()
        .filter(|o| {
            o.zone
                == (Zone::Hand {
                    owner: PlayerId(player),
                })
        })
        .count()
}
fn recover(g: &Game) {
    let restored = Game::restore(g.prepared.as_ref().clone(), &g.snapshot().unwrap()).unwrap();
    assert_eq!(restored.state, g.state);
    assert_eq!(restored.trace, g.trace);
    let replayed = Game::replay_to(
        g.prepared.as_ref().clone(),
        &g.replay_journal(),
        Some(g.state.rules.rule_steps),
        g.trace.mode,
    )
    .unwrap();
    assert_eq!(replayed.state, g.state);
    assert_eq!(replayed.trace, g.trace);
}
fn first_draw(g: &mut Game) {
    for _ in 0..100 {
        if matches!(&g.state.rules.frame,Frame::Resolve {frame} if frame.damage_draw.as_ref().is_some_and(|p|p.completed==Quantity::from(1)&&p.remaining==Quantity::from(1)))
        {
            return;
        }
        assert!(matches!(
            g.advance(Budget::new(1)),
            Advance::Suspended { .. }
        ));
    }
    panic!("missing bounded first draw");
}
fn view(g: &Game, player: u8) -> Observation {
    let QueryResult::Complete(QueryValue::Observation(o)) =
        g.observe(PlayerId(player), Budget::unlimited())
    else {
        panic!("observation");
    };
    o
}

#[test]
fn damage_quantity_draw_actual_combat_two_budget_progress_recovery_and_admission() {
    let p = prepared(1, 0, false, 4, None);
    for (search, hosted, feature) in [
        (true, false, Feature::Search),
        (false, true, Feature::Hosting),
    ] {
        let mut profile = match_profile("commander-foundations-v1").unwrap().clone();
        profile.search = search;
        profile.hosted = hosted;
        assert!(
            matches!(profile.validate_requested_features(&p.catalog),Err(ProfileError::UnreviewedCapabilities {feature:f,capabilities}) if f==feature&&capabilities.contains(&Capability::DamageQuantityDrawV1))
        );
    }
    let mut g = Game::with_trace(p, TraceMode::Off);
    declare(&mut g, &[(0, 1)]);
    placed(&mut g);
    assert_eq!(g.state.rules.players[1].life, 38.into());
    let mut whole = Game::replay_to(
        g.prepared.as_ref().clone(),
        &g.replay_journal(),
        Some(g.state.rules.rule_steps),
        TraceMode::Detailed,
    )
    .unwrap();
    begin_resolution(&mut g);
    first_draw(&mut g);
    assert_eq!(library(&g), 3);
    assert_eq!(hand(&g, 0), 1);
    recover(&g);
    for wrong_cursor in [false, true] {
        let mut structural = g.fork();
        let Frame::Resolve { frame } = &mut structural.state.rules.frame else {
            panic!("resolve");
        };
        let progress = frame.damage_draw.as_mut().unwrap();
        if wrong_cursor {
            progress.cursor += 1;
        } else {
            progress.source_block += 1;
        }
        let Frame::Resolve { frame } = &structural.state.rules.frame else {
            panic!("resolve");
        };
        assert!(structural.validate_resolving(frame).is_err());
    }
    // Preserve the frozen total while lying about how many real draw attempts
    // happened. A fresh checksum must not turn a different history into state.
    let mut forged: Snapshot = strict_read(&g.snapshot().unwrap()).unwrap();
    let Frame::Resolve { frame } = &mut forged.state.rules.frame else {
        panic!("resolve");
    };
    let progress = frame.damage_draw.as_mut().unwrap();
    progress.completed = 0.into();
    progress.remaining = 2.into();
    let mut coherent = g.fork();
    coherent.state = forged.state.clone();
    let Frame::Resolve { frame } = &coherent.state.rules.frame else {
        panic!("resolve");
    };
    coherent.validate_resolving(frame).unwrap();
    forged.state_sha256 = digest(&forged.state).unwrap();
    assert!(
        Game::restore(
            g.prepared.as_ref().clone(),
            &serde_json::to_string(&forged).unwrap()
        )
        .is_err()
    );
    let _ = settle(&mut g);
    begin_resolution(&mut whole);
    assert!(matches!(
        whole.advance(Budget::unlimited()),
        Advance::AwaitingDecision(Request::Priority { .. })
    ));
    assert_eq!(g.state, whole.state);
    assert_eq!(g.state_digest().unwrap(), whole.state_digest().unwrap());
    assert_eq!(library(&g), 2);
    assert_eq!(hand(&g, 0), 2);
    assert_eq!(hand(&g, 1), 0);
    recover(&g);
}

#[test]
fn damage_quantity_draw_real_prevention_and_doubling_use_committed_amount() {
    for (shields, doubled, expected) in [(1, false, 1), (2, false, 0), (0, true, 4)] {
        let mut g = Game::new(prepared(1, shields, doubled, 6, None));
        for i in 0..shields {
            shield(&mut g, 1 + i as u32);
        }
        declare(&mut g, &[(0, 1)]);
        if expected == 0 {
            for _ in 0..100 {
                let r = settle(&mut g);
                if g.state.rules.turn.step == TurnStep::CombatDamage {
                    break;
                }
                passive_request(&mut g, r);
            }
            assert_eq!(g.state.rules.turn.step, TurnStep::CombatDamage);
            assert_eq!(g.state.rules.next_trigger_id, 0);
            assert!(g.state.rules.stack.is_empty());
        } else {
            placed(&mut g);
            resolve(&mut g);
        }
        assert_eq!(
            g.state.rules.players[1].life,
            Quantity::from(40 - expected as i64)
        );
        assert_eq!(hand(&g, 0), expected);
        assert_eq!(library(&g), 6 - expected);
        recover(&g);
    }
}

#[test]
fn damage_quantity_draw_two_sources_keep_independent_occurrence_quantities() {
    let mut g = Game::new(prepared(2, 1, false, 6, None));
    shield(&mut g, 2);
    declare(&mut g, &[(0, 1), (1, 3)]);
    placed(&mut g);
    assert_eq!(g.state.rules.stack.len(), 2);
    assert_eq!(g.state.rules.players[1].life, 39.into());
    assert_eq!(g.state.rules.players[3].life, 38.into());
    resolve(&mut g);
    assert!([1, 2].contains(&hand(&g, 0)));
    assert_eq!(g.state.rules.stack.len(), 1);
    resolve(&mut g);
    assert_eq!(hand(&g, 0), 3);
    assert_eq!(library(&g), 3);
    assert_eq!(g.state.rules.next_trigger_id, 2);
    recover(&g);
}

#[test]
fn damage_quantity_draw_commander_first_or_middle_is_private_and_consumes_one_attempt() {
    for position in [0, 1] {
        for to_command in [false, true] {
            let mut g = Game::new(prepared(1, 0, false, 4, Some(position)));
            let original = g.state.rules.libraries[&PlayerId(0)].clone();
            declare(&mut g, &[(0, 1)]);
            placed(&mut g);
            begin_resolution(&mut g);
            assert!(matches!(
                settle(&mut g),
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
            assert_eq!(hand(&g, 0), position);
            recover(&g);
            answer(&mut g, Response::CommanderReturn { to_command });
            let _ = settle(&mut g);
            assert_eq!(library(&g), 2);
            assert_eq!(
                g.state.rules.libraries[&PlayerId(0)].as_slice(),
                &original[2..]
            );
            assert_eq!(hand(&g, 0), if to_command { 1 } else { 2 });
            assert_eq!(
                g.state.rules.objects[&original[position]].zone,
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
                    .filter(|d| matches!(d.response, Response::CommanderReturn { .. }))
                    .count(),
                1
            );
            recover(&g);
        }
    }
}

#[test]
fn damage_quantity_draw_departures_do_not_rebind_or_resurrect_personal_draws() {
    for departing in [0, 2] {
        let mut g = Game::new(prepared(1, 0, false, 4, None));
        declare(&mut g, &[(0, 1)]);
        placed(&mut g);
        begin_resolution(&mut g);
        first_draw(&mut g);
        g.concede(PlayerId(departing)).unwrap();
        let _ = settle(&mut g);
        assert!(!g.is_live(PlayerId(departing)));
        if departing == 2 {
            assert!(g.object_id(CardId(0)).is_none());
            assert_eq!(hand(&g, 0), 2);
            assert_eq!(library(&g), 2);
        } else {
            assert_eq!(hand(&g, 0), 0);
            assert!(
                !g.state
                    .rules
                    .objects
                    .values()
                    .any(|o| o.id.card == CardId(0) && o.zone == Zone::Battlefield)
            );
        }
        assert_eq!(hand(&g, 1), 0);
        assert_eq!(hand(&g, 3), 0);
        recover(&g);
    }
}

#[test]
fn damage_quantity_draw_empty_library_defers_failed_draw_loss_until_program_finishes() {
    for available in [0, 1] {
        let mut g = Game::with_trace(prepared(1, 0, false, available, None), TraceMode::Detailed);
        declare(&mut g, &[(0, 1)]);
        placed(&mut g);
        begin_resolution(&mut g);
        for _ in 0..100 {
            if matches!(&g.state.rules.frame,Frame::Resolve {frame} if frame.cursor==1&&matches!(frame.outcomes.as_slice(),[InstructionOutcome::DamageDrawn {player:PlayerId(0),attempted,failed}] if *attempted==Quantity::from(2)&&*failed==Quantity::from(2-available as i64)))
            {
                break;
            }
            assert!(matches!(
                g.advance(Budget::new(1)),
                Advance::Suspended { .. }
            ));
        }
        assert!(matches!(&g.state.rules.frame,Frame::Resolve {frame} if frame.cursor==1));
        assert!(g.state.rules.players[0].failed_draw);
        assert!(g.is_live(PlayerId(0)));
        assert_eq!(hand(&g, 0), available);
        recover(&g);
        let _ = settle(&mut g);
        assert!(!g.is_live(PlayerId(0)));
        assert!(g.events().iter().any(|e|matches!(e,Event::PlayersLost {losses} if losses.contains(&(PlayerId(0),PlayerLossReason::FailedDraw)))));
        recover(&g);
    }
}
