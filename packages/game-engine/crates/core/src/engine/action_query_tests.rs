use super::*;

fn scenario() -> Game {
    Game::new(
        Prepared::admit(
            Catalog::read(include_str!("../../../../tests/fixtures/expected-ir.json")).unwrap(),
            ScenarioConfig::land_return(),
        )
        .unwrap(),
    )
}
fn observation(game: &Game, viewer: PlayerId) -> Observation {
    match game.player_session(viewer).observe(Budget::unlimited()) {
        QueryResult::Complete(QueryValue::Observation(value)) => value,
        _ => panic!("observation unavailable"),
    }
}
fn token(game: &Game, viewer: PlayerId) -> DecisionToken {
    observation(game, viewer).decision.unwrap().token().clone()
}
fn collect(
    game: &Game,
    viewer: PlayerId,
    scope: ActionScope,
    page_size: u16,
    units: u64,
) -> (Vec<ActionBeginning>, Vec<ActionQueryOutcome>) {
    let session = game.player_session(viewer);
    let mut result =
        session.action_beginnings(token(game, viewer), scope, page_size, Budget::new(0));
    let mut all = vec![];
    let mut transcript = vec![];
    for _ in 0..20000 {
        let (outcome, continuation) = result.into_parts();
        transcript.push(outcome.clone());
        match outcome {
            ActionQueryOutcome::Complete { page } => {
                assert!(continuation.is_none());
                assert!(page.context.is_some());
                all.extend(page.beginnings);
                return (all, transcript);
            }
            ActionQueryOutcome::Incomplete {
                page,
                required_minimum,
            } => {
                assert_eq!(required_minimum, 1);
                assert!(page.beginnings.len() <= page_size as usize);
                all.extend(page.beginnings);
                result = session.resume_actions(continuation.unwrap(), Budget::new(units));
            }
            other => panic!("unexpected outcome {other:?}"),
        }
    }
    panic!("no bounded progress")
}
fn answer(game: &mut Game, response: PlayerResponse) {
    let viewer = game.decision_for(PlayerId(0)).unwrap().unwrap().player();
    let receipt = match game.player_session(viewer).check_response(
        token(game, viewer),
        response,
        Budget::unlimited(),
    ) {
        QueryResult::Complete(QueryValue::Validated(receipt)) => receipt,
        _ => panic!("expected valid response"),
    };
    game.submit(receipt).unwrap();
    let _ = game.advance(Budget::unlimited());
}
fn check_agreement(game: &Game, viewer: PlayerId) -> Vec<ActionBeginning> {
    let view = observation(game, viewer);
    let request = view.decision.unwrap();
    let mut objects = view.view.objects;
    objects.sort_by(|a, b| a.handle.0.cmp(&b.handle.0));
    let priority = matches!(request, PlayerRequest::Priority { .. });
    let mut candidates = if priority {
        vec![ActionInitiation::Pass]
    } else {
        vec![
            ActionInitiation::FinishMana { undo_mana: false },
            ActionInitiation::FinishMana { undo_mana: true },
        ]
    };
    for object in objects {
        if priority {
            candidates.push(ActionInitiation::PlayLand {
                object: object.handle.clone(),
            });
            candidates.push(ActionInitiation::Cast {
                card: object.handle.clone(),
            });
        }
        candidates.push(ActionInitiation::ActivateMana {
            object: object.handle.clone(),
        });
        if priority {
            let id = game.resolve_handle(viewer, &object.handle).unwrap();
            // Include one nonexistent ability as a negative witness.
            for ability in 0..=game.definition(id.card).unwrap().activated.len() as u32 {
                candidates.push(ActionInitiation::ActivateAbility {
                    object: object.handle.clone(),
                    ability,
                });
            }
        }
    }
    let expected = candidates
        .into_iter()
        .filter(|candidate| {
            matches!(
                game.player_session(viewer).check_response(
                    request.token().clone(),
                    candidate.response(),
                    Budget::unlimited()
                ),
                QueryResult::Complete(QueryValue::Validated(_))
            )
        })
        .collect::<Vec<_>>();
    let (actual, _) = collect(game, viewer, ActionScope::Window, 64, u64::MAX);
    assert_eq!(
        actual
            .iter()
            .map(|b| b.initiation.clone())
            .collect::<Vec<_>>(),
        expected
    );
    actual
}

#[test]
fn partitions_paging_cancellation_and_queries_preserve_every_publication() {
    let game = scenario();
    eprintln!(
        "core test identity: {} profile={} opt={} ir={}",
        BUILD_ID,
        crate::BUILD_PROFILE,
        crate::BUILD_OPT_LEVEL,
        mtg_contracts::CURRENT_IR_VERSION
    );
    let before = game.snapshot().unwrap();
    let digest_before = game.state_digest().unwrap();
    let before_views = (0..4)
        .map(|p| observation(&game, PlayerId(p)))
        .collect::<Vec<_>>();
    let expected = check_agreement(&game, PlayerId(0));
    for units in [1, 2, 7, 10000] {
        for page in [1, 2, 64] {
            assert_eq!(
                collect(&game, PlayerId(0), ActionScope::Window, page, units).0,
                expected
            );
        }
    }
    for units in [0, 1, 2, 3, 7] {
        let result = game.player_session(PlayerId(0)).action_beginnings(
            token(&game, PlayerId(0)),
            ActionScope::Window,
            64,
            Budget::new(units),
        );
        result.into_parts().1.unwrap().cancel();
    }
    let session = game.player_session(PlayerId(0));
    let (zero, work) = session
        .action_beginnings(
            token(&game, PlayerId(0)),
            ActionScope::Window,
            64,
            Budget::new(0),
        )
        .into_parts();
    let (still_zero, work) = session
        .resume_actions(work.unwrap(), Budget::new(0))
        .into_parts();
    assert_eq!(zero, still_zero);
    work.unwrap().cancel();
    assert_eq!(game.snapshot().unwrap(), before);
    assert_eq!(game.state_digest().unwrap(), digest_before);
    assert_eq!(
        (0..4)
            .map(|p| observation(&game, PlayerId(p)))
            .collect::<Vec<_>>(),
        before_views
    );
    let (_, transcript) = collect(&game, PlayerId(0), ActionScope::Window, 64, 1);
    eprintln!(
        "scenario priority: {} beginnings, {} single-unit resumes, max outcome bytes {}",
        expected.len(),
        transcript.len() - 1,
        transcript
            .iter()
            .map(|p| serde_json::to_vec(p).unwrap().len())
            .max()
            .unwrap()
    );
}

#[test]
fn stale_foreign_tokens_viewers_executors_and_epochs_fail_closed() {
    let mut game = scenario();
    let original = token(&game, PlayerId(0));
    for (viewer, token) in [
        (PlayerId(1), original.clone()),
        (PlayerId(0), DecisionToken("foreign".into())),
    ] {
        assert_eq!(
            game.player_session(viewer)
                .action_beginnings(token, ActionScope::Window, 64, Budget::new(1))
                .outcome(),
            &ActionQueryOutcome::Stale
        );
    }
    assert_eq!(
        game.player_session(PlayerId(4))
            .action_beginnings(original.clone(), ActionScope::Window, 64, Budget::new(0))
            .outcome(),
        &ActionQueryOutcome::Unavailable {
            reason: ActionUnavailable::InvalidViewer
        }
    );
    for page_size in [0, 65, u16::MAX] {
        assert_eq!(
            game.player_session(PlayerId(0))
                .action_beginnings(
                    original.clone(),
                    ActionScope::Window,
                    page_size,
                    Budget::new(0)
                )
                .outcome(),
            &ActionQueryOutcome::Unavailable {
                reason: ActionUnavailable::InvalidPageSize
            }
        );
    }
    let start = || {
        game.player_session(PlayerId(0))
            .action_beginnings(original.clone(), ActionScope::Window, 64, Budget::new(3))
            .into_parts()
            .1
            .unwrap()
    };
    assert_eq!(
        game.player_session(PlayerId(1))
            .resume_actions(start(), Budget::unlimited())
            .outcome(),
        &ActionQueryOutcome::Stale
    );
    assert_eq!(
        game.fork()
            .player_session(PlayerId(0))
            .resume_actions(start(), Budget::unlimited())
            .outcome(),
        &ActionQueryOutcome::Stale
    );
    let pending = start();
    answer(&mut game, PlayerResponse::Pass);
    assert_eq!(
        game.player_session(PlayerId(0))
            .resume_actions(pending, Budget::new(0))
            .outcome(),
        &ActionQueryOutcome::Stale
    );
    assert_eq!(
        game.player_session(PlayerId(0))
            .action_beginnings(original, ActionScope::Window, 64, Budget::new(1))
            .outcome(),
        &ActionQueryOutcome::Stale
    );
}

#[test]
fn object_scopes_use_current_viewer_handles_and_never_include_global_actions() {
    let mut game = scenario();
    let viewer = PlayerId(0);
    let objects = observation(&game, viewer).view.objects;
    let all = check_agreement(&game, viewer);
    for object in &objects {
        let scoped = collect(
            &game,
            viewer,
            ActionScope::Object {
                object: object.handle.clone(),
            },
            1,
            1,
        )
        .0;
        let expected = all
            .iter()
            .filter(|b| match &b.initiation {
                ActionInitiation::PlayLand { object: handle }
                | ActionInitiation::ActivateMana { object: handle }
                | ActionInitiation::ActivateAbility { object: handle, .. }
                | ActionInitiation::Cast { card: handle } => handle == &object.handle,
                _ => false,
            })
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(scoped, expected);
    }
    let foreign = observation(&game, PlayerId(1)).view.objects[0]
        .handle
        .clone();
    for object in [ObjectHandle("unknown".into()), foreign] {
        assert_eq!(
            game.player_session(viewer)
                .action_beginnings(
                    token(&game, viewer),
                    ActionScope::Object { object },
                    64,
                    Budget::unlimited()
                )
                .outcome(),
            &ActionQueryOutcome::Unavailable {
                reason: ActionUnavailable::Scope
            }
        );
    }
    let visible = objects
        .iter()
        .find(|o| o.oracle_id == BEARS)
        .unwrap()
        .handle
        .clone();
    let old = game.resolve_handle(viewer, &visible).unwrap();
    game.move_object(old, Zone::Hand { owner: PlayerId(1) })
        .unwrap();
    game.refresh_visibility().unwrap();
    assert_eq!(
        game.player_session(viewer)
            .action_beginnings(
                token(&game, viewer),
                ActionScope::Object { object: visible },
                64,
                Budget::unlimited()
            )
            .outcome(),
        &ActionQueryOutcome::Unavailable {
            reason: ActionUnavailable::Scope
        }
    );
}

#[test]
fn hidden_identity_order_and_card_facts_do_not_change_results_or_budget_transcripts() {
    let mut original = scenario();
    // Seat zero gets priority after seat one's private hand has been installed.
    let mut other = original.fork();
    for game in [&mut original, &mut other] {
        let id = game.object_id(CardId(2)).unwrap();
        game.move_object(id, Zone::Hand { owner: PlayerId(1) })
            .unwrap();
        game.refresh_visibility().unwrap();
    }
    let mut physical = other.state.rules.cards.remove(&CardId(2)).unwrap();
    physical.oracle_id = ISLAND.into();
    let mut object = other.state.rules.objects.remove(&CardId(2)).unwrap();
    object.id = ObjectId {
        card: CardId(999),
        incarnation: 42,
    };
    other.state.rules.cards.insert(CardId(999), physical);
    other.state.rules.objects.insert(CardId(999), object);
    other.refresh_visibility().unwrap();
    assert_eq!(
        observation(&original, PlayerId(0)),
        observation(&other, PlayerId(0))
    );
    assert_ne!(
        original.state_digest().unwrap(),
        other.state_digest().unwrap()
    );
    for units in [1, 2, 7, 10000] {
        assert_eq!(
            collect(&original, PlayerId(0), ActionScope::Window, 1, units),
            collect(&other, PlayerId(0), ActionScope::Window, 1, units)
        );
    }
}

#[test]
fn mana_beginnings_validate_initiation_only_and_preserve_exact_cost() {
    let mut game = scenario();
    let cast = check_agreement(&game, PlayerId(0))
        .into_iter()
        .find(|b| matches!(b.initiation, ActionInitiation::Cast { .. }))
        .unwrap();
    answer(&mut game, cast.initiation.response());
    let request = observation(&game, PlayerId(0)).decision.unwrap();
    let PlayerRequest::Targets { candidates, .. } = request else {
        panic!("target window")
    };
    assert_eq!(
        game.player_session(PlayerId(0))
            .action_beginnings(
                token(&game, PlayerId(0)),
                ActionScope::Window,
                64,
                Budget::unlimited()
            )
            .outcome(),
        &ActionQueryOutcome::Unavailable {
            reason: ActionUnavailable::UnsupportedWindow
        }
    );
    answer(
        &mut game,
        PlayerResponse::Targets {
            objects: vec![candidates[0].clone()],
        },
    );
    let PlayerRequest::Mana { cost, .. } = observation(&game, PlayerId(0)).decision.unwrap() else {
        panic!("mana")
    };
    let before = game.snapshot().unwrap();
    let actual = check_agreement(&game, PlayerId(0));
    assert!(
        actual
            .iter()
            .any(|b| matches!(b.initiation, ActionInitiation::ActivateMana { .. }))
    );
    assert_eq!(
        actual
            .iter()
            .filter(|b| matches!(b.initiation, ActionInitiation::FinishMana { .. }))
            .count(),
        2
    );
    let (_, pages) = collect(&game, PlayerId(0), ActionScope::Window, 64, 1);
    let ActionQueryOutcome::Complete { page } = pages.last().unwrap() else {
        unreachable!()
    };
    assert_eq!(
        page.context,
        Some(ActionDecisionContext::Mana {
            cost,
            source: ActionSourceContext::Unavailable
        })
    );
    assert_eq!(game.snapshot().unwrap(), before);
}

fn pod(profile: &str) -> Game {
    let source = match profile {
        "commander-foundations-v1" => {
            include_str!("../../../../tests/fixtures/expected-match-ir.json")
        }
        "commander-attachments-v1" => {
            include_str!("../../../../tests/fixtures/expected-attachments-ir.json")
        }
        _ => unreachable!(),
    };
    Game::new(
        Prepared::admit_pod(
            Catalog::read(source).unwrap(),
            MatchConfig::for_profile(profile, [0; 32], PlayerId(0)).unwrap(),
        )
        .unwrap(),
    )
}
#[test]
fn admitted_hosted_profiles_cover_opening_main_land_cast_and_mana() {
    for profile in ["commander-foundations-v1", "commander-attachments-v1"] {
        let mut game = pod(profile);
        let mut main = false;
        for _ in 0..100 {
            let Advance::AwaitingDecision(request) = game.advance(Budget::unlimited()) else {
                panic!("request")
            };
            match request {
                Request::Mulligan { .. } => {
                    game.respond(
                        request.player(),
                        request.decision_id(),
                        Response::Mulligan { take: false },
                    )
                    .unwrap();
                }
                Request::Priority { .. } => {
                    check_agreement(&game, request.player());
                    if game.turn().step == TurnStep::PrecombatMain {
                        main = true;
                        break;
                    }
                    game.respond(request.player(), request.decision_id(), Response::Pass)
                        .unwrap();
                }
                _ => panic!("unexpected request"),
            }
        }
        assert!(main);
        let actions = check_agreement(&game, PlayerId(0));
        let land = actions
            .iter()
            .find(|b| matches!(b.initiation, ActionInitiation::PlayLand { .. }))
            .unwrap();
        answer(&mut game, land.initiation.response());
        let actions = check_agreement(&game, PlayerId(0));
        assert!(
            !actions
                .iter()
                .any(|b| matches!(b.initiation, ActionInitiation::PlayLand { .. }))
        );
        let cast = actions
            .iter()
            .find(|b| matches!(b.initiation, ActionInitiation::Cast { .. }))
            .unwrap();
        answer(&mut game, cast.initiation.response());
        assert!(matches!(
            observation(&game, PlayerId(0)).decision,
            Some(PlayerRequest::Mana { .. })
        ));
        let actions = check_agreement(&game, PlayerId(0));
        eprintln!("{profile}: mana {} beginnings", actions.len());
        assert!(
            actions
                .iter()
                .any(|b| matches!(b.initiation, ActionInitiation::FinishMana { .. }))
        );
    }
}

#[test]
fn setup_pending_inputs_and_concessions_reuse_existing_query_guards() {
    let game = pod("commander-foundations-v1");
    assert_eq!(
        game.player_session(PlayerId(0))
            .action_beginnings(
                DecisionToken("unknown".into()),
                ActionScope::Window,
                64,
                Budget::new(0)
            )
            .outcome(),
        &ActionQueryOutcome::Unavailable {
            reason: ActionUnavailable::Boundary
        }
    );
    let mut opening = pod("commander-foundations-v1");
    let Advance::AwaitingDecision(request) = opening.advance(Budget::unlimited()) else {
        panic!("opening request")
    };
    assert!(matches!(request, Request::Mulligan { .. }));
    assert_eq!(
        opening
            .player_session(request.player())
            .action_beginnings(
                token(&opening, request.player()),
                ActionScope::Window,
                64,
                Budget::new(1)
            )
            .outcome(),
        &ActionQueryOutcome::Unavailable {
            reason: ActionUnavailable::UnsupportedWindow
        }
    );
    let mut game = scenario();
    let old_token = token(&game, PlayerId(0));
    let QueryResult::Complete(QueryValue::Validated(receipt)) = game
        .player_session(PlayerId(0))
        .check_response(old_token.clone(), PlayerResponse::Pass, Budget::unlimited())
    else {
        panic!("receipt")
    };
    game.submit(receipt).unwrap();
    assert_eq!(
        game.player_session(PlayerId(0))
            .action_beginnings(old_token.clone(), ActionScope::Window, 64, Budget::new(1))
            .outcome(),
        &ActionQueryOutcome::Stale
    );
    game.concede(PlayerId(1)).unwrap();
    assert_eq!(
        game.player_session(PlayerId(0))
            .action_beginnings(old_token, ActionScope::Window, 64, Budget::new(0))
            .outcome(),
        &ActionQueryOutcome::Unavailable {
            reason: ActionUnavailable::Boundary
        }
    );
}

#[test]
fn public_schema_and_outcomes_exclude_trusted_native_state() {
    let schema = schemars::generate::SchemaSettings::draft07()
        .into_generator()
        .into_root_schema_for::<ActionQueryOutcome>();
    let text = serde_json::to_string(&schema).unwrap();
    for forbidden in [
        "Work",
        "Continuation",
        "ValidatedResponse",
        "state_sha256",
        "executor",
        "state_token",
        "CardId",
        "ObjectId",
        "Snapshot",
    ] {
        assert!(!text.contains(forbidden), "schema leaked {forbidden}");
    }
    let game = scenario();
    eprintln!(
        "permitted priority example: {}",
        serde_json::to_string(
            game.player_session(PlayerId(0))
                .action_beginnings(
                    token(&game, PlayerId(0)),
                    ActionScope::Window,
                    64,
                    Budget::unlimited()
                )
                .outcome()
        )
        .unwrap()
    );
    let (_, pages) = collect(&game, PlayerId(0), ActionScope::Window, 64, 1);
    for page in pages {
        let encoded = serde_json::to_string(&page).unwrap();
        assert_eq!(
            serde_json::from_str::<ActionQueryOutcome>(&encoded).unwrap(),
            page
        );
    }
}

// Same exact compiled source as hosted attachments, with a private witness board.
// This bypasses deck construction only in tests; public admission is unchanged.
fn attachment_witness() -> Game {
    let catalog = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-attachments-ir.json"
    ))
    .unwrap();
    let rows = [
        ("Golgari Signet", false),
        ("Forest", false),
        ("Short Sword", false),
        ("Grizzly Bears", false),
        ("Azure Mage", false),
        ("Samite Healer", false),
        ("Sol Ring", false),
        ("Llanowar Elves", false),
        ("Commander's Sphere", false),
        ("Grizzly Bears", true),
    ];
    let mut cards = rows
        .iter()
        .enumerate()
        .map(|(i, (name, hand))| CardSetup {
            id: CardId(i as u32),
            oracle_id: catalog
                .definitions
                .iter()
                .find(|d| d.name == *name)
                .unwrap()
                .oracle_id
                .clone(),
            owner: PlayerId(0),
            controller: PlayerId(0),
            zone: if *hand {
                Zone::Hand { owner: PlayerId(0) }
            } else {
                Zone::Battlefield
            },
        })
        .collect::<Vec<_>>();
    for p in 0..4 {
        for _ in 0..4 {
            cards.push(CardSetup {
                id: CardId(cards.len() as u32),
                oracle_id: FOREST.into(),
                owner: PlayerId(p),
                controller: PlayerId(p),
                zone: Zone::Library { owner: PlayerId(p) },
            });
        }
    }
    let config = ScenarioConfig {
        initial: ScenarioInitial {
            turn_step: TurnStep::Untap,
            ..ScenarioInitial::default()
        },
        mode: Mode::CommanderMatch,
        players: 4,
        profile: "commander-attachments-v1".into(),
        commanders: vec![],
        cards,
    };
    let prepared = Prepared {
        catalog_digest: digest(&catalog).unwrap(),
        profile_digest: digest(&config).unwrap(),
        catalog: Arc::new(catalog),
        config,
        match_config: None,
    };
    let mut game = Game::new(prepared);
    for _ in 0..100 {
        let Advance::AwaitingDecision(request) = game.advance(Budget::unlimited()) else {
            panic!("priority")
        };
        if game.turn().step == TurnStep::PrecombatMain {
            return game;
        }
        game.respond(request.player(), request.decision_id(), Response::Pass)
            .unwrap();
    }
    panic!("main not reached")
}
fn handle(game: &Game, id: u32) -> ObjectHandle {
    game.handle_for(PlayerId(0), game.object_id(CardId(id)).unwrap())
        .unwrap()
}
#[test]
fn attachments_and_nested_mana_have_validated_beginnings_and_permitted_source() {
    let mut game = attachment_witness();
    let all = check_agreement(&game, PlayerId(0));
    for id in [2, 4, 5, 8] {
        assert!(all.iter().any(|b| b.initiation
            == ActionInitiation::ActivateAbility {
                object: handle(&game, id),
                ability: 0
            }));
    }
    for id in [0, 1, 6, 7] {
        assert!(all.iter().any(|b| b.initiation
            == ActionInitiation::ActivateMana {
                object: handle(&game, id)
            }));
    }
    let source = handle(&game, 0);
    answer(
        &mut game,
        PlayerResponse::ActivateMana {
            object: source.clone(),
        },
    );
    let before = game.snapshot().unwrap();
    let actual = check_agreement(&game, PlayerId(0));
    assert!(!actual.iter().any(|b| b.initiation
        == ActionInitiation::ActivateMana {
            object: source.clone()
        }));
    let (_, pages) = collect(&game, PlayerId(0), ActionScope::Window, 1, 1);
    let ActionQueryOutcome::Complete { page } = pages.last().unwrap() else {
        unreachable!()
    };
    assert!(
        matches!(&page.context, Some(ActionDecisionContext::Mana { source: ActionSourceContext::Available { object }, .. }) if object == &source)
    );
    assert_eq!(game.snapshot().unwrap(), before);
    let forest = handle(&game, 1);
    answer(&mut game, PlayerResponse::ActivateMana { object: forest });
    check_agreement(&game, PlayerId(0));
    eprintln!(
        "attachments nested mana: {} beginnings, {} one-unit resumes",
        actual.len(),
        pages.len() - 1
    );
}

#[test]
fn supported_beginnings_survive_restore_but_query_continuations_do_not() {
    let game = attachment_witness();
    let work = game
        .player_session(PlayerId(0))
        .action_beginnings(
            token(&game, PlayerId(0)),
            ActionScope::Window,
            64,
            Budget::new(7),
        )
        .into_parts()
        .1
        .unwrap();
    let restored =
        Game::restore(game.prepared.as_ref().clone(), &game.snapshot().unwrap()).unwrap();
    assert_eq!(
        collect(&game, PlayerId(0), ActionScope::Window, 64, 1),
        collect(&restored, PlayerId(0), ActionScope::Window, 64, 1)
    );
    assert_eq!(
        restored
            .player_session(PlayerId(0))
            .resume_actions(work, Budget::unlimited())
            .outcome(),
        &ActionQueryOutcome::Stale
    );
}

#[test]
fn broken_permitted_handle_binding_is_an_invariant_failure_not_a_partial_menu() {
    let mut game = scenario();
    let current = token(&game, PlayerId(0));
    game.state
        .rules
        .objects
        .get_mut(&CardId(0))
        .unwrap()
        .id
        .incarnation += 1;
    assert_eq!(
        game.player_session(PlayerId(0))
            .action_beginnings(current, ActionScope::Window, 64, Budget::unlimited())
            .outcome(),
        &ActionQueryOutcome::Unavailable {
            reason: ActionUnavailable::Invariant
        }
    );
}
