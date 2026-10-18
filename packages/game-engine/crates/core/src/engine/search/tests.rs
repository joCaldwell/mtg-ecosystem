use super::*;
fn catalog() -> Catalog {
    Catalog::read(include_str!(
        "../../../../../tests/fixtures/expected-match-ir.json"
    ))
    .unwrap()
}
fn root(takes: u8) -> Game {
    let mut game = Game::with_trace(
        Prepared::admit_pod(catalog(), MatchConfig::foundations([0; 32], PlayerId(0))).unwrap(),
        TraceMode::Off,
    );
    for _ in 0..100 {
        let Advance::AwaitingDecision(request) = game.advance(Budget::unlimited()) else {
            panic!("opening should reach a decision")
        };
        if matches!(
            request,
            Request::Priority {
                player: PlayerId(0),
                ..
            }
        ) && game.state.turn.step == TurnStep::PrecombatMain
            && game.state.passes.is_empty()
        {
            return game;
        }
        let response = match &request {
            Request::Mulligan {
                player,
                mulligans_taken,
                ..
            } => Response::Mulligan {
                take: player.0 == 0 && *mulligans_taken < takes,
            },
            Request::BottomOpening {
                count, candidates, ..
            } => Response::BottomOpening {
                objects: candidates.iter().take(*count).copied().collect(),
            },
            Request::Priority { .. } => Response::Pass,
            _ => panic!("unexpected opening request {request:?}"),
        };
        game.respond(request.player(), request.decision_id(), response)
            .unwrap();
    }
    panic!("opening did not finish")
}
fn capture(game: &Game) -> SearchCapture {
    match game.capture_search(PlayerId(0), Budget::unlimited()) {
        CaptureOutcome::Complete(c) => c,
        _ => panic!("supported root capture"),
    }
}
fn proposal(capture: &SearchCapture) -> WorldProposal {
    let data = capture.data();
    let view = &data.context.observation.view;
    let mut seq = 0;
    let seats = std::array::from_fn(|seat| {
        let player = PlayerId(seat as u8);
        if view.players[seat].status == PlayerStatus::Left {
            return SeatHypothesis {
                hidden_hand: vec![],
                ordered_library: vec![],
            };
        }
        let mut remaining = data.profile.decks[seat]
            .cards
            .iter()
            .map(|r| (r.oracle_id.clone(), r.count))
            .collect::<BTreeMap<_, _>>();
        for object in view.objects.iter().filter(|o| o.owner == player) {
            *remaining.get_mut(&object.oracle_id).unwrap() -= 1;
        }
        let mut add = |id: String| {
            seq += 1;
            HypotheticalCard {
                symbol: format!("local-{seq}"),
                oracle_id: id,
            }
        };
        let mut hand = vec![];
        for known in data.known_hands.iter().filter(|k| k.player == player) {
            *remaining.get_mut(&known.oracle_id).unwrap() -= 1;
            hand.push(add(known.oracle_id.clone()));
        }
        let mut bottom = vec![];
        if player == view.viewer {
            for card in &data.known_bottom {
                *remaining.get_mut(&card.oracle_id).unwrap() -= 1;
                bottom.push(add(card.oracle_id.clone()));
            }
        }
        let count = if player == view.viewer {
            0
        } else {
            view.players[seat].hand_count as usize
        };
        for (id, n) in &mut remaining {
            while *n > 0 && hand.len() < count {
                *n -= 1;
                hand.push(add(id.clone()));
            }
        }
        let mut library = vec![];
        for (id, n) in remaining {
            for _ in 0..n {
                library.push(add(id.clone()));
            }
        }
        library.extend(bottom);
        SeatHypothesis {
            hidden_hand: hand,
            ordered_library: library,
        }
    });
    WorldProposal {
        proposal_version: 1,
        context_id: capture.context_id().into(),
        sampler_version: SEARCH_SAMPLER.into(),
        world_index: 0,
        seats,
        future_seed: [9; 32],
    }
}
fn build(capture: &SearchCapture, p: WorldProposal) -> (SampledGame, RootHandleMap) {
    match SampledGame::build(catalog(), capture, p, Budget::unlimited()) {
        SampleBuildOutcome::Complete { game, root_map } => (game, root_map),
        SampleBuildOutcome::Rejected(e) => panic!("{e:?}"),
        _ => panic!("unlimited build"),
    }
}
#[test]
fn bottom_positions_are_applied_memories_and_cross_suffix_edge_exactly() {
    let a = RememberedCard {
        memory_ref: "memory-a".into(),
        oracle_id: FOREST.into(),
    };
    let b = RememberedCard {
        memory_ref: "memory-b".into(),
        oracle_id: SWAMP.into(),
    };
    let mut actions = vec![
        OwnKnowledgeAction::OpeningRedraw,
        OwnKnowledgeAction::OpeningBottom {
            cards: vec![a.clone(), b.clone()],
        },
    ];
    assert_eq!(
        known_suffix(&actions, 94).unwrap(),
        vec![a.clone(), b.clone()]
    );
    actions.extend((0..92).map(|_| OwnKnowledgeAction::Drawn {
        oracle_id: PLAINS.into(),
    }));
    assert_eq!(
        known_suffix(&actions, 2).unwrap(),
        vec![a.clone(), b.clone()]
    );
    actions.push(OwnKnowledgeAction::Drawn {
        oracle_id: FOREST.into(),
    });
    assert_eq!(known_suffix(&actions, 1).unwrap(), vec![b.clone()]);
    actions.push(OwnKnowledgeAction::Drawn {
        oracle_id: SWAMP.into(),
    });
    assert!(known_suffix(&actions, 0).unwrap().is_empty());
    actions[94] = OwnKnowledgeAction::Drawn {
        oracle_id: SWAMP.into(),
    };
    assert!(matches!(
        known_suffix(&actions, 0),
        Err(SearchRejection::UnsupportedHistory(_))
    ));
    actions.push(OwnKnowledgeAction::OpeningRedraw);
    assert!(matches!(
        known_suffix(&actions, 92),
        Err(SearchRejection::UnsupportedHistory(_))
    ));
    let reshuffled = vec![
        OwnKnowledgeAction::OpeningRedraw,
        OwnKnowledgeAction::OpeningBottom {
            cards: vec![a.clone(), b],
        },
        OwnKnowledgeAction::OpeningRedraw,
    ];
    assert!(known_suffix(&reshuffled, 92).unwrap().is_empty());
    let mut another_basic = a.clone();
    another_basic.memory_ref = "another-basic".into();
    let repeated = vec![OwnKnowledgeAction::OpeningBottom {
        cards: vec![a.clone(), another_basic.clone()],
    }];
    assert_eq!(known_suffix(&repeated, 94).unwrap(), vec![a, another_basic]);
}
#[test]
fn paid_opening_capture_world_projection_namespace_and_independent_hidden_origin() {
    let game = root(2);
    let c = capture(&game);
    assert_eq!(c.data().known_bottom.len(), 1);
    assert!(
        c.data()
            .own_knowledge
            .iter()
            .any(|a| matches!(a, OwnKnowledgeAction::OpeningRedraw))
    );
    assert!(
        c.data()
            .own_knowledge
            .iter()
            .any(|a| matches!(a, OwnKnowledgeAction::Drawn { .. }))
    );
    let p = proposal(&c);
    let (sample, map) = build(&c, p.clone());
    let QueryResult::Complete(QueryValue::Observation(observation)) =
        sample.observe(PlayerId(0), Budget::unlimited())
    else {
        panic!("sample observation")
    };
    assert_eq!(
        map.mapped_observation(&observation).unwrap(),
        c.data().context.observation
    );
    let actual_token = c
        .data()
        .context
        .observation
        .decision
        .as_ref()
        .unwrap()
        .token();
    assert_ne!(map.token(actual_token).unwrap(), *actual_token);
    assert!(matches!(
        sample.check_player_response(
            PlayerId(0),
            actual_token.clone(),
            PlayerResponse::Pass,
            Budget::unlimited()
        ),
        QueryResult::Rejected(_)
    ));
    let mut invisible_change = p.clone();
    invisible_change.seats[1].ordered_library.reverse();
    invisible_change.future_seed = [123; 32];
    invisible_change.world_index = 73;
    let (other_world, _) = build(&c, invisible_change);
    for viewer in (0..4).map(PlayerId) {
        let QueryResult::Complete(QueryValue::Observation(a)) =
            sample.observe(viewer, Budget::unlimited())
        else {
            panic!("sample view")
        };
        let QueryResult::Complete(QueryValue::Observation(b)) =
            other_world.observe(viewer, Budget::unlimited())
        else {
            panic!("paired view")
        };
        assert_eq!(
            a, b,
            "unknown library/RNG changed viewer {viewer:?} permissions"
        );
    }
    let QueryResult::Incomplete { work, .. } = sample.observe(PlayerId(0), Budget::new(1)) else {
        panic!("sample work")
    };
    assert!(matches!(
        other_world.resume_work(work, Budget::unlimited()),
        QueryResult::Rejected(QueryRejection::StaleOrForeignWork)
    ));
    let mut relabeled = p.clone();
    relabeled.future_seed = [2; 32];
    relabeled.world_index = 42;
    for seat in &mut relabeled.seats {
        for card in seat.hidden_hand.iter_mut().chain(&mut seat.ordered_library) {
            card.symbol.push('x');
        }
    }
    assert_eq!(
        p.assignment_digest().unwrap(),
        relabeled.assignment_digest().unwrap()
    );
    let mut other = game.fork();
    // Private non-reachability fixture for information-flow independence: only
    // unobserved state changes. It is never exported as a real restore origin.
    other
        .state
        .match_state
        .as_mut()
        .unwrap()
        .random
        .word_position = RngPosition(12345);
    other
        .state
        .libraries
        .get_mut(&PlayerId(1))
        .unwrap()
        .reverse();
    let c2 = capture(&other);
    assert_eq!(c.data(), c2.data());
    assert_eq!(c.context_id(), c2.context_id());
    let (same, _) = build(&c2, p);
    assert_eq!(sample.state_digest().unwrap(), same.state_digest().unwrap());
}
#[test]
fn reconstruction_budget_forks_restore_and_complete_accounting_are_strict() {
    let mut game = root(0);
    let c = capture(&game);
    let p = proposal(&c);
    let (expected, _) = build(&c, p.clone());
    let mut outcome = SampledGame::build(catalog(), &c, p.clone(), Budget::new(0));
    let mut units = 0;
    let (mut sampled, map) = loop {
        match outcome {
            SampleBuildOutcome::Incomplete { work, .. } => {
                units += 1;
                outcome = work.resume(Budget::new(1));
            }
            SampleBuildOutcome::Complete { game, root_map } => break (game, root_map),
            SampleBuildOutcome::Rejected(e) => panic!("{e:?}"),
        }
    };
    assert!(units >= 400);
    assert_eq!(
        sampled.state_digest().unwrap(),
        expected.state_digest().unwrap()
    );
    let token = map
        .token(
            c.data()
                .context
                .observation
                .decision
                .as_ref()
                .unwrap()
                .token(),
        )
        .unwrap();
    let QueryResult::Complete(QueryValue::Validated(receipt)) = sampled.check_player_response(
        PlayerId(0),
        token,
        PlayerResponse::Pass,
        Budget::unlimited(),
    ) else {
        panic!("valid sample pass")
    };
    sampled.submit_validated(receipt).unwrap();
    let json = sampled.snapshot().unwrap();
    let mut restore = SampledGame::restore(catalog(), &c, &json, Budget::new(0));
    let restored = loop {
        match restore {
            SampleRestoreOutcome::Incomplete { work, .. } => restore = work.resume(Budget::new(1)),
            SampleRestoreOutcome::Complete(g) => break g,
            SampleRestoreOutcome::Rejected(e) => panic!("{e:?}"),
        }
    };
    assert_eq!(
        sampled.state_digest().unwrap(),
        restored.state_digest().unwrap()
    );
    assert!(Game::restore(game.prepared.as_ref().clone(), &json).is_err());
    assert!(
        Game::restore_from_reader(
            game.prepared.as_ref().clone(),
            &mut std::io::Cursor::new(json.as_bytes())
        )
        .is_err()
    );
    let mut oversized_snapshot: serde_json::Value = serde_json::from_str(&json).unwrap();
    oversized_snapshot["proposal"]["seats"][1]["ordered_library"][0]["symbol"] =
        "x".repeat(129).into();
    assert!(
        matches!(SampledGame::restore(catalog(), &c, &oversized_snapshot.to_string(), Budget::new(0)),
        SampleRestoreOutcome::Rejected(SearchRejection::InvalidProposal(reason)) if reason=="hypothetical identity length bound")
    );
    let before = sampled.state_digest().unwrap();
    let mut fork = sampled.fork();
    fork.advance(Budget::new(1));
    assert_eq!(sampled.state_digest().unwrap(), before);
    let mut oversized = p.clone();
    oversized.seats[1].ordered_library[0].symbol = "x".repeat(129);
    assert!(oversized.assignment_digest().is_err());
    assert!(matches!(
        SampledGame::build(catalog(), &c, oversized, Budget::new(0)),
        SampleBuildOutcome::Rejected(SearchRejection::InvalidProposal(_))
    ));
    let mut bad = p.clone();
    bad.seats[1].ordered_library.pop();
    assert!(matches!(
        SampledGame::build(catalog(), &c, bad, Budget::unlimited()),
        SampleBuildOutcome::Rejected(SearchRejection::InvalidProposal(_))
    ));
    let mut bad = p.clone();
    bad.seats[1].ordered_library[0].symbol = bad.seats[2].ordered_library[0].symbol.clone();
    assert!(matches!(
        SampledGame::build(catalog(), &c, bad, Budget::unlimited()),
        SampleBuildOutcome::Rejected(SearchRejection::InvalidProposal(_))
    ));
    let CaptureOutcome::Incomplete { work, .. } = game.capture_search(PlayerId(0), Budget::new(0))
    else {
        panic!("capture work")
    };
    game.concede(PlayerId(3)).unwrap();
    assert!(matches!(
        game.resume_search_capture(work, Budget::unlimited()),
        CaptureOutcome::Rejected(SearchRejection::StaleWork)
    ));
}
#[test]
fn survivors_and_public_reversal_memories_are_owner_scoped() {
    let mut game = root(0);
    for player in [PlayerId(3), PlayerId(2)] {
        game.concede(player).unwrap();
        let _ = game.advance(Budget::unlimited());
        let c = capture(&game);
        assert_eq!(
            c.data().context.observation.view.live_turn_order.len(),
            3 - usize::from(player.0 == 2)
        );
        build(&c, proposal(&c));
    }
    // Historical public-to-hidden reversal information can be checked without
    // replaying a different owner's private hand/history. Two owners' same
    // Oracle are separate constraints, even though the public decks are equal.
    let mut game = root(0);
    let mut view = game.state.visibility[0].history.last().unwrap().to_public();
    for owner in [PlayerId(1), PlayerId(3)] {
        view.objects.push(VisibleObject {
            characteristics: Some(
                game.prepared
                    .catalog
                    .definitions
                    .iter()
                    .find(|d| d.oracle_id == SOUL_FEAST)
                    .unwrap()
                    .characteristics
                    .clone(),
            ),
            marked_damage: Quantity::zero(),
            attack_ready: false,
            handle: ObjectHandle(format!("past-{owner:?}")),
            oracle_id: SOUL_FEAST.into(),
            owner,
            controller: owner,
            zone: Zone::Stack,
            tapped: false,
        });
    }
    game.state.visibility[0].history.push_public(view);
    // Synthetic permitted-history domain fixture tests inference only; no fake
    // capture can be injected through the public API or real snapshot restore.
    let c = capture(&game);
    assert_eq!(
        c.data()
            .known_hands
            .iter()
            .filter(|k| k.oracle_id == SOUL_FEAST)
            .count(),
        2
    );
    let p = proposal(&c);
    build(&c, p.clone());
    let mut bad = p;
    let known = bad.seats[1]
        .hidden_hand
        .iter_mut()
        .find(|x| x.oracle_id == SOUL_FEAST)
        .unwrap();
    known.oracle_id = FOREST.into();
    assert!(matches!(
        SampledGame::build(catalog(), &c, bad, Budget::unlimited()),
        SampleBuildOutcome::Rejected(SearchRejection::InvalidProposal(_))
    ));
}

#[test]
fn sampler_identity_ignores_only_build_binding() {
    let g = root(0);
    let capture = capture(&g);
    let data = capture.data();
    let mut changed = data.clone();
    changed.profile.engine_build_id = "another exact build".into();
    let history = "ordered-history-test-digest";
    assert_ne!(
        super::capture::capture_identity(data, history, true).unwrap(),
        super::capture::capture_identity(&changed, history, true).unwrap()
    );
    assert_eq!(
        super::capture::capture_identity(data, history, false).unwrap(),
        super::capture::capture_identity(&changed, history, false).unwrap()
    );
    changed.profile.rules_digest.push('x');
    assert_ne!(
        super::capture::capture_identity(data, history, false).unwrap(),
        super::capture::capture_identity(&changed, history, false).unwrap()
    );
    assert_ne!(
        super::capture::capture_identity(data, history, false).unwrap(),
        super::capture::capture_identity(data, "reordered-history", false).unwrap()
    );
}
