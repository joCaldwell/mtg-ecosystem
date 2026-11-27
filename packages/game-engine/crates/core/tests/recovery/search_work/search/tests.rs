use super::*;
fn catalog() -> Catalog {
    FixtureCatalog::Match.get().clone()
}
fn root(takes: u8) -> Game {
    let mut game = Game::with_trace(
        Prepared::admit_pod(
            catalog(),
            MatchConfig::for_profile("commander-foundations-v1", [0; 32], PlayerId(0)).unwrap(),
        )
        .unwrap(),
        TraceMode::Off,
    );
    for _ in 0..100 {
        let Advance::AwaitingDecision(request) = game.advance(Budget::unlimited()) else {
            panic!("opening should reach a decision")
        };
        if matches!(
            request,
            Decision {
                player: PlayerId(0),
                reason: Reason::Priority,
                ..
            }
        ) && game.state.rules.turn.step == TurnStep::PrecombatMain
            && game.state.rules.passes.is_empty()
        {
            return game;
        }
        let response = match &request {
            Decision {
                player,
                reason: Reason::Mulligan {
                    mulligans_taken, ..
                },
                ..
            } => request.select(Candidate::Flag {
                flag: if player.0 == 0 && *mulligans_taken < takes {
                    Flag::Mulligan
                } else {
                    Flag::Keep
                },
            }),
            found @ Decision {
                reason: Reason::BottomOpening,
                ..
            } => {
                let count = &found.required_count();
                let candidates = found.objects_offered();
                request.order_objects(candidates.iter().take(*count).copied().collect())
            }
            Decision {
                reason: Reason::Priority,
                ..
            } => Answer::Pick {
                picks: vec![vec![0]],
            },
            _ => panic!("unexpected opening request {request:?}"),
        };
        game.respond(request.player, request.id, response).unwrap();
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
        proposal_version: mtg_core::testkit::formats::PROPOSAL_VERSION,
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
        .id
        .clone();
    assert_ne!(map.token(&actual_token).unwrap(), actual_token);
    assert!(matches!(
        sample.check_player_response(
            PlayerId(0),
            actual_token.clone(),
            Answer::Pick {
                picks: vec![vec![0]]
            },
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
    other.state.rules.match_state.as_mut().unwrap().random.table = 12345;
    other
        .state
        .rules
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
        .token(&c.data().context.observation.decision.as_ref().unwrap().id)
        .unwrap();
    let QueryResult::Complete(QueryValue::Validated(receipt)) = sampled.check_player_response(
        PlayerId(0),
        token,
        Answer::Pick {
            picks: vec![vec![0]],
        },
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
    let mut view = game.state.knowledge.visibility[0]
        .history
        .last()
        .unwrap()
        .to_public();
    for owner in [PlayerId(1), PlayerId(3)] {
        view.objects.push(VisibleObject {
            loyalty_counters: Quantity::zero(),
            loyalty_used_this_turn: false,
            faces: None,
            face_down: false,
            exiled_by: None,
            phased_out: false,
            entered_this_turn: false,
            mana_ability_used: None,
            attached_to: None,
            attachments: vec![],
            keywords: vec![],
            granted_mana: vec![],
            restrictions: vec![],
            token: None,
            plus_one_counters: Quantity::zero(),
            counters: vec![],
            characteristics: Some(
                game.prepared
                    .catalog
                    .definitions
                    .iter()
                    .find(|d| d.oracle_id == oracle_id(FixtureCatalog::Match.get(), "Soul Feast"))
                    .unwrap()
                    .characteristics
                    .clone(),
            ),
            marked_damage: Quantity::zero(),
            attack_ready: false,
            handle: ObjectHandle(format!("past-{owner:?}")),
            oracle_id: oracle_id(FixtureCatalog::Match.get(), "Soul Feast").into(),
            owner,
            controller: owner,
            zone: Zone::Stack,
            tapped: false,
        });
    }
    game.state.knowledge.visibility[0].history.push_public(view);
    // Synthetic permitted-history domain fixture tests inference only; no fake
    // capture can be injected through the public API or real snapshot restore.
    let c = capture(&game);
    assert_eq!(
        c.data()
            .known_hands
            .iter()
            .filter(|k| k.oracle_id == oracle_id(FixtureCatalog::Match.get(), "Soul Feast"))
            .count(),
        2
    );
    let p = proposal(&c);
    build(&c, p.clone());
    let mut bad = p;
    let known = bad.seats[1]
        .hidden_hand
        .iter_mut()
        .find(|x| x.oracle_id == oracle_id(FixtureCatalog::Match.get(), "Soul Feast"))
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

#[test]
fn search_admission_uses_flags_and_retains_the_catalog_pin() {
    for profile in MATCH_PROFILES.iter() {
        let mut game = Game::with_trace(
            Prepared::admit_pod(
                profile.catalog().clone(),
                MatchConfig::for_profile(&profile.name, [0; 32], PlayerId(0)).unwrap(),
            )
            .unwrap(),
            TraceMode::Off,
        );
        loop {
            let Advance::AwaitingDecision(r) = game.advance(Budget::new(1024)) else {
                panic!("opening decision")
            };
            if matches!(
                r,
                Decision {
                    player: PlayerId(0),
                    reason: Reason::Priority,
                    ..
                }
            ) && game.turn().step == TurnStep::PrecombatMain
            {
                break;
            }
            let response = match &r {
                Decision {
                    reason: Reason::Mulligan { .. },
                    ..
                } => r.select(Candidate::Flag { flag: Flag::Keep }),
                found @ Decision {
                    reason: Reason::BottomOpening,
                    ..
                } => {
                    let count = &found.required_count();
                    let candidates = found.objects_offered();
                    r.order_objects(candidates[..*count].to_vec())
                }
                Decision {
                    reason: Reason::Priority,
                    ..
                } => Answer::Pick {
                    picks: vec![vec![0]],
                },
                _ => panic!("unexpected opening request"),
            };
            game.respond(r.player, r.id, response).unwrap();
        }
        assert_eq!(
            matches!(
                game.capture_search(PlayerId(0), Budget::new(0)),
                CaptureOutcome::Incomplete { .. }
            ),
            profile.search,
            "{}",
            profile.name
        );
        if profile.search {
            Arc::make_mut(&mut game.prepared).catalog_digest = "changed".into();
            assert!(matches!(
                game.capture_search(PlayerId(0), Budget::new(0)),
                CaptureOutcome::Rejected(_)
            ));
        }
    }
}

fn observe(game: &Game, viewer: PlayerId, quantum: Budget) -> Observation {
    let mut result = game.observe(viewer, quantum);
    for _ in 0..100_000 {
        match result {
            QueryResult::Complete(QueryValue::Observation(o)) => return o,
            QueryResult::Incomplete { work, .. } => result = game.resume_work(work, Budget::new(1)),
            _ => panic!("observation"),
        }
    }
    panic!("observation work bound")
}
fn stepped_capture(game: &Game) -> SearchCapture {
    let mut result = game.capture_search(PlayerId(0), Budget::new(1));
    for _ in 0..1_000_000 {
        match result {
            CaptureOutcome::Complete(c) => return c,
            CaptureOutcome::Incomplete {
                work,
                required_minimum,
            } => result = game.resume_search_capture(work, Budget::new(required_minimum.max(1))),
            CaptureOutcome::Rejected(e) => panic!("capture rejected {e:?}"),
        }
    }
    panic!("capture work bound")
}

/// CR 106.4a-b, 106.6: restricted pool buckets are public. Their projection is
/// invariant under query and capture budgets, older observations without the
/// field still read, and a sampled world rebuilds the buckets from the
/// permitted capture instead of silently dropping them. (Exact restore and
/// replay of really produced buckets: the Omen Hawker card witnesses.)
#[test]
fn restricted_pool_buckets_are_budget_invariant_and_rebuilt_in_sampled_worlds() {
    let mut game = root(0);
    let legacy = observe(&game, PlayerId(1), Budget::unlimited());
    let bucket = |restriction, color, amount: i64| RestrictedMana {
        restriction,
        color,
        amount: Quantity::from(amount),
    };
    let own = vec![bucket(ManaSpendRestriction::ActivatedAbility, Color::C, 2)];
    let other = vec![
        bucket(ManaSpendRestriction::ColorlessSpell, Color::C, 1),
        bucket(
            ManaSpendRestriction::SpellType {
                kind: CardKind::Artifact,
            },
            Color::W,
            3,
        ),
    ];
    game.state.rules.players[0].restricted_mana = own.clone();
    game.state.rules.players[2].restricted_mana = other.clone();
    game.validate_state().unwrap();
    for viewer in game.live_players() {
        let seen = observe(&game, viewer, Budget::unlimited());
        assert_eq!(seen.view.players[0].restricted_mana, own);
        assert_eq!(seen.view.players[2].restricted_mana, other);
        assert!(seen.view.players[1].restricted_mana.is_empty());
        assert_eq!(observe(&game, viewer, Budget::new(1)), seen);
        let wire = serde_json::to_string(&seen).unwrap();
        assert_eq!(serde_json::from_str::<Observation>(&wire).unwrap(), seen);
    }
    // Additive: a row serialized without restricted buckets omits the key,
    // and such a row still reads with no restricted mana.
    let wire = serde_json::to_value(&legacy).unwrap();
    assert!(
        wire["view"]["players"]
            .as_array()
            .unwrap()
            .iter()
            .all(|row| row.get("restricted_mana").is_none())
    );
    assert_eq!(serde_json::from_value::<Observation>(wire).unwrap(), legacy);
    let whole = capture(&game);
    let stepped = stepped_capture(&game);
    assert_eq!(whole.context_id(), stepped.context_id());
    assert_eq!(
        whole.data().context.observation.view.players[2].restricted_mana,
        other
    );
    for c in [&whole, &stepped] {
        let (sample, map) = build(c, proposal(c));
        let QueryResult::Complete(QueryValue::Observation(root)) =
            sample.observe(PlayerId(0), Budget::unlimited())
        else {
            panic!("sample observation")
        };
        assert_eq!(root.view.players[0].restricted_mana, own);
        assert_eq!(root.view.players[2].restricted_mana, other);
        assert_eq!(
            map.mapped_observation(&root).unwrap(),
            c.data().context.observation
        );
    }
}
