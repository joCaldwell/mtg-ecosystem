//! Complete source-lowered Locke Cole: ordinary draw then mandatory private
//! controller discard, with one frozen invocation controller throughout.
use super::*;
const LOCKE: &str = "bdcb7aed-3595-4c7e-b9da-543e92de919a";
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
            "../../../../tests/fixtures/cards-data-d2ce/bdcb7aed-3595-4c7e-b9da-543e92de919a.json"
        ),
    ) else {
        panic!("complete Locke Cole source must lower");
    };
    assert!(
        definition
            .capabilities
            .contains(&Capability::DamageControllerDiscardV1)
    );
    c.definitions.push(*definition);
    c.schema_version = mtg_contracts::CURRENT_IR_VERSION;
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    let mut config = ScenarioConfig::turns_permanents();
    config.profile = "damage-controller-discard-domain-witness".into();
    config.initial.turn_step = TurnStep::PrecombatMain;
    config.cards = vec![
        CardSetup {
            id: CardId(0),
            oracle_id: LOCKE.into(),
            owner: PlayerId(2),
            controller: PlayerId(0),
            zone: Zone::Battlefield,
        },
        CardSetup {
            id: CardId(1),
            oracle_id: SWAMP.into(),
            owner: PlayerId(0),
            controller: PlayerId(0),
            zone: Zone::Hand { owner: PlayerId(0) },
        },
        CardSetup {
            id: CardId(2),
            oracle_id: FOREST.into(),
            owner: PlayerId(1),
            controller: PlayerId(1),
            zone: Zone::Hand { owner: PlayerId(1) },
        },
    ];
    if commander {
        config.cards.push(CardSetup {
            id: CardId(3),
            oracle_id: ISAMARU.into(),
            owner: PlayerId(0),
            controller: PlayerId(0),
            zone: Zone::Library { owner: PlayerId(0) },
        });
        config.commanders.push(CardId(3));
    }
    for seat in 0..4 {
        for _ in 0..if empty && seat == 0 { 0 } else { 3 } {
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
    panic!("missing Locke Cole trigger");
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
            player: PlayerId(0),
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
fn damage_controller_discard_draws_before_private_controller_choice_and_replays() {
    for mode in [TraceMode::Off, TraceMode::Detailed] {
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
        recover(&g);
        let own = view(&g, 0).decision.unwrap();
        let PlayerRequest::ResolveDiscard { candidates, .. } = &own else {
            panic!("controller discard");
        };
        assert_eq!(candidates.len(), 2);
        assert!(view(&g, 1).decision.is_none());
        let chosen = candidates
            .iter()
            .find(|h| g.resolve_handle(PlayerId(0), h).unwrap().card == CardId(1))
            .unwrap()
            .clone();
        for h in candidates {
            let object = g.resolve_handle(PlayerId(0), h).unwrap();
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
            (PlayerId(1), r.decision_id(), object),
            (PlayerId(0), r.decision_id().wrapping_add(1), object),
            (PlayerId(0), r.decision_id(), foreign),
        ] {
            assert!(
                g.respond(
                    player,
                    decision,
                    Response::ResolveDiscard {
                        objects: vec![card]
                    }
                )
                .is_err()
            );
        }
        let foreign_handle = g.handle_for(PlayerId(1), foreign).unwrap();
        assert!(matches!(
            g.check_player_response(
                PlayerId(0),
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
            PlayerId(0),
            own.token().clone(),
            PlayerResponse::ResolveDiscard {
                objects: vec![chosen],
            },
            Budget::unlimited(),
        ) else {
            panic!("valid controller discard");
        };
        g.submit(receipt).unwrap();
        recover(&g);
        let _ = g.advance(Budget::new(1));
        assert_eq!(
            g.state.rules.objects[&CardId(1)].zone,
            Zone::Graveyard { owner: PlayerId(0) }
        );
        recover(&g);
        finish(&mut g);
        assert_eq!(hand(&g, 0), 1);
        assert_eq!(hand(&g, 1), 1);
        assert!(g.is_live(PlayerId(0)));
        recover(&g);
    }
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
    recover(&g);
    let object = g.object_id(CardId(1)).unwrap();
    answer(
        &mut g,
        Response::ResolveDiscard {
            objects: vec![object],
        },
    );
    let _ = g.advance(Budget::new(1));
    assert!(g.is_live(PlayerId(0)));
    assert_eq!(
        g.state.rules.objects[&CardId(1)].zone,
        Zone::Graveyard { owner: PlayerId(0) }
    );
    assert!(
        matches!(g.state.rules.frame,Frame::Resolve {ref frame} if frame.cursor==2 && matches!(frame.outcomes.last(),Some(InstructionOutcome::Discarded {player:PlayerId(0),movements}) if movements.len()==1))
    );
    recover(&g);
    finish(&mut g);
    assert!(!g.is_live(PlayerId(0)));
    assert!(g.events().iter().any(|e|matches!(e,Event::PlayersLost {losses} if losses.contains(&(PlayerId(0),PlayerLossReason::FailedDraw)))));
    recover(&g);
}
#[test]
fn damage_controller_discard_hidden_commander_draw_replacement_resumes_at_discard() {
    for to_command in [false, true] {
        let mut g = Game::new(prepared(false, true));
        placed(&mut g);
        assert!(matches!(
            begin_resolution(&mut g),
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
        answer(&mut g, Response::CommanderReturn { to_command });
        assert!(matches!(
            settle(&mut g),
            Request::ResolveDiscard {
                player: PlayerId(0),
                ..
            }
        ));
        assert!(
            matches!(g.state.rules.frame,Frame::ResolutionChoice {ref frame,..} if frame.cursor==1)
        );
        assert_eq!(g.state.rules.libraries[&PlayerId(0)].first(), Some(&next));
        assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), 3);
        assert_eq!(hand(&g, 0), if to_command { 1 } else { 2 });
        recover(&g);
        let object = g.object_id(CardId(1)).unwrap();
        answer(
            &mut g,
            Response::ResolveDiscard {
                objects: vec![object],
            },
        );
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
                .filter(|i| matches!(i.response, Response::CommanderReturn { .. }))
                .count(),
            1
        );
        recover(&g);
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
                Request::ResolveDiscard {
                    player: PlayerId(0),
                    ..
                }
            ));
            recover(&g);
            let object = g.object_id(CardId(1)).unwrap();
            answer(
                &mut g,
                Response::ResolveDiscard {
                    objects: vec![object],
                },
            );
            finish(&mut g);
            assert_eq!(hand(&g, 0), 1);
            if departing == 2 {
                assert!(g.object_id(CardId(0)).is_none());
            }
        }
        recover(&g);
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
    let Frame::ResolutionChoice {
        choice: ResolutionChoice::Discard { player, .. },
        ..
    } = &mut forged.state.rules.frame
    else {
        panic!("discard");
    };
    *player = PlayerId(1);
    let mut structural = g.fork();
    structural.state = forged.state.clone();
    assert!(structural.validate_actions_state().is_err());
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
fn combat_hand_cast_uses_trigger_controller_and_pays_the_child_additional_cost() {
    let mut p = prepared(false, false);
    let raw =
        include_str!("../../../../tests/fixtures/ramos/ac50ef98-1791-4dd4-9c1f-8cea7db7ade5.json");
    let mtg_compiler::CompileResult::Lowered { definition } = mtg_compiler::compile(
        include_str!("../../../../tests/fixtures/ramos/archive-manifest-current.json"),
        raw,
    ) else {
        panic!("complete combat hand cast source must lower")
    };
    assert!(
        definition
            .capabilities
            .contains(&Capability::CombatHandCastV1)
    );
    p.config.cards[0].oracle_id = definition.oracle_id.clone();
    let catalog = Arc::make_mut(&mut p.catalog);
    catalog.definitions.push(*definition);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    p.catalog_digest = digest(catalog).unwrap();
    let spell = CardId(1);
    // An ordinary main-phase spell with a target and a mandatory sacrifice.
    p.config.cards[1].oracle_id = "0c936582-d4c0-4da8-bc7e-49da5a433939".into();
    p.config.cards[2].zone = Zone::Battlefield;
    p.config.cards[2].oracle_id = "000d5588-5a4c-434e-988d-396632ade42c".into();
    p.profile_digest = digest(&p.config).unwrap();
    let mut game = Game::new(p);
    placed(&mut game);
    let source = game.object_id(CardId(0)).unwrap();
    let target = game.object_id(CardId(2)).unwrap();
    let offered = begin_resolution(&mut game);
    let Request::HandCast {
        player, candidates, ..
    } = offered
    else {
        panic!("expected private optional hand cast")
    };
    assert_eq!(player, PlayerId(0));
    assert_eq!(candidates, [game.object_id(spell).unwrap()]);
    let chooser = view(&game, 0);
    let Some(PlayerRequest::HandCast {
        token, candidates, ..
    }) = chooser.decision
    else {
        panic!("missing private chooser")
    };
    assert!(view(&game, 2).decision.is_none());
    recover(&game);
    let mut declined = game.fork();
    answer(&mut declined, Response::Pass);
    assert!(matches!(settle(&mut declined), Request::Priority { .. }));
    assert!(declined.state.rules.stack.is_empty());
    assert_eq!(
        declined
            .current(declined.object_id(spell).unwrap())
            .unwrap()
            .zone,
        Zone::Hand { owner: player }
    );
    let QueryResult::Complete(QueryValue::Validated(receipt)) = game.check_player_response(
        player,
        token,
        PlayerResponse::Cast {
            card: candidates[0].clone(),
        },
        Budget::unlimited(),
    ) else {
        panic!("private child choice rejected")
    };
    game.submit(receipt).unwrap();
    assert!(matches!(settle(&mut game), Request::Targets { .. }));
    recover(&game);
    let mut interrupted = game.fork();
    answer(
        &mut interrupted,
        Response::Targets {
            objects: vec![target],
        },
    );
    interrupted.concede(PlayerId(1)).unwrap();
    let Request::HandCast { candidates, .. } = settle(&mut interrupted) else {
        panic!("the failed target proposal must return to its owning hand offer");
    };
    assert_eq!(candidates, [interrupted.object_id(spell).unwrap()]);
    assert_eq!(interrupted.state.rules.stack.len(), 1);
    assert_eq!(
        interrupted.current(candidates[0]).unwrap().zone,
        Zone::Hand { owner: player }
    );
    recover(&interrupted);
    answer(&mut interrupted, Response::Pass);
    assert!(matches!(settle(&mut interrupted), Request::Priority { .. }));
    assert!(interrupted.state.rules.stack.is_empty());
    answer(
        &mut game,
        Response::Targets {
            objects: vec![target],
        },
    );
    assert!(matches!(settle(&mut game), Request::SacrificeCost { .. }));
    recover(&game);
    answer(&mut game, Response::SacrificeCost { object: source });
    assert!(matches!(settle(&mut game), Request::Priority { .. }));
    assert_eq!(game.state.rules.stack.len(), 1);
    let child = game.top().unwrap();
    assert_eq!(child.source.card, spell);
    assert_eq!(child.cast_from, Zone::Hand { owner: player });
    assert!(child.hand_cast_permission.is_some());
    assert!(child.total_cost.colored.is_empty() && child.total_cost.generic == Quantity::zero());
    assert_eq!(child.paid.as_ref(), Some(&child.total_cost));
    assert_eq!(
        game.current(game.object_id(source.card).unwrap())
            .unwrap()
            .zone,
        Zone::Graveyard { owner: PlayerId(2) }
    );
    recover(&game);
    let mut forged = game.fork();
    forged
        .state
        .rules
        .stack
        .last_mut()
        .unwrap()
        .hand_cast_permission = None;
    assert!(
        Game::restore(
            forged.prepared.as_ref().clone(),
            &forged.snapshot().unwrap()
        )
        .is_err()
    );
    for _ in 0..4 {
        answer(&mut game, Response::Pass);
    }
    assert!(matches!(settle(&mut game), Request::Priority { .. }));
    assert_eq!(
        game.current(game.object_id(target.card).unwrap())
            .unwrap()
            .zone,
        Zone::Graveyard { owner: PlayerId(1) }
    );
    recover(&game);
}

#[test]
fn paid_bestow_ordinary_attachment_detachment_and_illegal_target_fallback() {
    let compiled = mtg_compiler::compile(
        include_str!("../../../../tests/fixtures/ramos/archive-manifest-current.json"),
        include_str!("../../../../tests/fixtures/ramos/695ee4fe-356d-47fb-9442-d171b618151c.json"),
    );
    let mtg_compiler::CompileResult::Lowered { definition } = compiled else {
        panic!("bestow source must lower: {compiled:?}")
    };
    let mut setup = prepared(false, false);
    setup.config.cards[0].oracle_id = ISAMARU.into();
    setup.config.cards[0].owner = PlayerId(1);
    setup.config.cards[0].controller = PlayerId(1);
    setup.config.cards[1].oracle_id = definition.oracle_id.clone();
    let mut lands = Vec::new();
    for id in [
        "bc71ebf6-2056-41f7-be35-b2e5c34afa99",
        "b2c6aa39-2d2a-459c-a555-fb48ba993373",
        SWAMP,
        "a3fb7228-e76b-4e96-a40e-20b5fed75685",
        FOREST,
        SWAMP,
        SWAMP,
    ] {
        let card = CardId(setup.config.cards.len() as u32);
        lands.push(card);
        setup.config.cards.push(CardSetup {
            id: card,
            oracle_id: id.into(),
            owner: PlayerId(0),
            controller: PlayerId(0),
            zone: Zone::Battlefield,
        });
    }
    let catalog = Arc::make_mut(&mut setup.catalog);
    catalog.definitions.push(*definition.clone());
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    setup.catalog_digest = digest(catalog).unwrap();
    setup.profile_digest = digest(&setup.config).unwrap();
    for (bestow, depart_before_resolution) in [(false, false), (true, false), (true, true)] {
        let mut g = Game::new(setup.clone());
        let card = CardId(1);
        let target = g.object_id(CardId(0)).unwrap();
        let before = g.characteristics(target).unwrap();
        if bestow {
            answer(&mut g, Response::Bestow { card });
            assert!(matches!(settle(&mut g), Request::Targets { .. }));
            answer(
                &mut g,
                Response::Targets {
                    objects: vec![target],
                },
            );
        } else {
            answer(&mut g, Response::Cast { card });
        }
        assert!(matches!(settle(&mut g), Request::Mana { .. }));
        let cost = g.top().unwrap().total_cost.clone();
        assert_eq!(cost.generic, if bestow { 2.into() } else { 0.into() });
        for &land in lands.iter().take(if bestow { 7 } else { 5 }) {
            let object = g.object_id(land).unwrap();
            answer(&mut g, Response::ActivateMana { object });
        }
        answer(&mut g, Response::FinishMana { undo_mana: false });
        if matches!(settle(&mut g), Request::SpendMana { .. }) {
            let mut spending = BTreeMap::<Color, Quantity>::new();
            for c in &cost.colored {
                spending.entry(*c).or_default().add(&1.into());
            }
            spending.entry(Color::B).or_default().add(&cost.generic);
            answer(&mut g, Response::SpendMana { spending });
        }
        settle(&mut g);
        let entry = g.top().unwrap();
        assert_eq!(entry.paid.as_ref(), Some(&cost));
        assert_eq!(g.is_bestowed_spell(entry.source), bestow);
        assert_eq!(
            g.characteristics(entry.source)
                .unwrap()
                .has_type(CardKind::Creature),
            !bestow
        );
        recover(&g);
        if depart_before_resolution {
            g.concede(PlayerId(1)).unwrap();
        }
        for _ in 0..40 {
            settle(&mut g);
            if g.state.rules.stack.is_empty() {
                break;
            }
            passive(&mut g);
        }
        let permanent = g.object_id(card).unwrap();
        assert_eq!(g.zone(permanent), Some(Zone::Battlefield));
        if bestow && !depart_before_resolution {
            let c = g.characteristics(permanent).unwrap();
            assert_eq!(c.subtypes, ["Aura"]);
            assert_eq!(g.current(permanent).unwrap().attached_to, Some(target));
            let mut expected = before.power.clone().unwrap();
            expected.add(&4.into());
            assert_eq!(g.characteristics(target).unwrap().power, Some(expected));
            assert!(g.has_keyword(target, KeywordAbility::Flying));
            recover(&g);
            g.concede(PlayerId(1)).unwrap();
            settle(&mut g);
        }
        assert_eq!(g.object_id(card), Some(permanent));
        assert_eq!(g.current(permanent).unwrap().attached_to, None);
        assert_eq!(
            g.characteristics(permanent).unwrap(),
            definition.characteristics
        );
        recover(&g);
    }
}
