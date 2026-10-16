//! Independent legal-pod, private setup, deterministic-shuffle and full-match proofs.
//! Pinned CR103.2c/103.3/103.5/103.5c/103.8c,702.3,903.5–.7.
use mtg_contracts::{CardKind, Catalog, Color, Quantity};
use mtg_core::*;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

fn catalog() -> Catalog {
    Catalog::read(include_str!(
        "../../../tests/fixtures/expected-match-ir.json"
    ))
    .unwrap()
}
fn config() -> MatchConfig {
    MatchConfig::foundations([17; 32], PlayerId(2))
}
fn oracle(name: &str) -> String {
    catalog()
        .definitions
        .into_iter()
        .find(|d| d.name == name)
        .unwrap()
        .oracle_id
}
fn replace_basic(c: &mut MatchConfig, seat: usize, name: &str) {
    let land = if seat.is_multiple_of(2) {
        "Plains"
    } else {
        "Forest"
    };
    let land = oracle(land);
    c.decks[seat]
        .cards
        .iter_mut()
        .find(|r| r.oracle_id == land)
        .unwrap()
        .count -= 1;
    c.decks[seat].cards.push(DeckEntry {
        oracle_id: oracle(name),
        count: 1,
    });
}

#[test]
fn pod_normalization_constructs_exact_four_hundred_cards_without_scenario_overrides() {
    let cat = catalog();
    let original = config();
    let p = validate_pod(&cat, &original).unwrap();
    let mut permuted = original.clone();
    for (seat, deck) in permuted.decks.iter_mut().enumerate() {
        let basic = oracle(if seat.is_multiple_of(2) {
            "Plains"
        } else {
            "Forest"
        });
        let row = deck
            .cards
            .iter_mut()
            .find(|r| r.oracle_id == basic)
            .unwrap();
        row.count -= 10;
        deck.cards.push(DeckEntry {
            oracle_id: basic,
            count: 10,
        });
        deck.cards.reverse();
    }
    let q = validate_pod(&cat, &permuted).unwrap();
    assert_eq!(p.normalized_config(), q.normalized_config());
    assert_eq!(p.identity_digest().unwrap(), q.identity_digest().unwrap());
    assert_eq!(p.physical_config(), q.physical_config());
    let physical = p.physical_config();
    assert_eq!(physical.cards.len(), 400);
    assert_eq!(
        physical.commanders,
        vec![CardId(0), CardId(100), CardId(200), CardId(300)]
    );
    let ids = physical.cards.iter().map(|c| c.id).collect::<BTreeSet<_>>();
    assert_eq!(ids.len(), 400);
    for seat in 0..4 {
        let owner = PlayerId(seat);
        let cards = physical
            .cards
            .iter()
            .filter(|c| c.owner == owner)
            .collect::<Vec<_>>();
        assert_eq!(cards.len(), 100);
        assert!(cards.iter().all(|c| c.controller == owner));
        assert_eq!(
            cards
                .iter()
                .filter(|c| c.zone == Zone::Command { owner })
                .count(),
            1
        );
        assert_eq!(
            cards
                .iter()
                .filter(|c| c.zone == Zone::Library { owner })
                .count(),
            99
        );
    }
    assert!(
        Prepared::admit(cat, physical.clone()).is_err(),
        "valid layouts cannot bypass sealed pod admission"
    );
}

#[test]
fn all_white_and_all_black_green_are_distinct_valid_legal_compositions() {
    let cat = catalog();
    for deck in [white_deck(), black_green_deck()] {
        let mut c = config();
        c.decks = std::array::from_fn(|_| deck.clone());
        let p = validate_pod(&cat, &c).unwrap();
        assert_eq!(p.physical_config().cards.len(), 400);
        let mut different = c.clone();
        different.seed[0] ^= 1;
        assert_ne!(
            p.identity_digest().unwrap(),
            validate_pod(&cat, &different)
                .unwrap()
                .identity_digest()
                .unwrap()
        );
        different = c.clone();
        different.starting_player = PlayerId(3);
        assert_ne!(
            p.identity_digest().unwrap(),
            validate_pod(&cat, &different)
                .unwrap()
                .identity_digest()
                .unwrap()
        );
    }
}

#[test]
fn count_aggregation_never_bypasses_singleton_or_commander_cardinality() {
    let cat = catalog();
    let mut c = config();
    replace_basic(&mut c, 0, "Yoked Ox");
    assert!(matches!(
        validate_pod(&cat, &c),
        Err(MatchAdmissionError::SingletonName {
            player: PlayerId(0),
            ..
        })
    ));
    c = config();
    let id = c.decks[0].commander.clone();
    c.decks[0]
        .cards
        .iter_mut()
        .find(|r| r.oracle_id == id)
        .unwrap()
        .count = 0;
    assert!(matches!(
        validate_pod(&cat, &c),
        Err(MatchAdmissionError::InvalidCount { .. })
    ));
    c = config();
    c.decks[0].cards.retain(|r| r.oracle_id != id);
    let plains = oracle("Plains");
    c.decks[0]
        .cards
        .iter_mut()
        .find(|r| r.oracle_id == plains)
        .unwrap()
        .count += 1;
    assert!(matches!(
        validate_pod(&cat, &c),
        Err(MatchAdmissionError::CommanderCount { count: 0, .. })
    ));
    c = config();
    c.decks[0]
        .cards
        .iter_mut()
        .find(|r| r.oracle_id == plains)
        .unwrap()
        .count -= 1;
    c.decks[0].cards.push(DeckEntry {
        oracle_id: id,
        count: 1,
    });
    assert!(matches!(
        validate_pod(&cat, &c),
        Err(MatchAdmissionError::CommanderCount { count: 2, .. })
    ));
}

#[test]
fn oversize_underflow_unknown_and_incompatible_cards_fail_before_physical_allocation() {
    let cat = catalog();
    let mut c = config();
    c.decks[0].cards[0].count = u32::MAX;
    assert!(matches!(
        validate_pod(&cat, &c),
        Err(MatchAdmissionError::InvalidDeckSize { .. })
    ));
    c = config();
    c.decks[0].cards.pop();
    assert!(matches!(
        validate_pod(&cat, &c),
        Err(MatchAdmissionError::InvalidDeckSize { .. })
    ));
    c = config();
    replace_basic(&mut c, 0, "Lightning Bolt");
    assert!(matches!(
        validate_pod(&cat, &c),
        Err(MatchAdmissionError::ColorIdentity { .. })
    ));
    c = config();
    replace_basic(&mut c, 1, "Seeds of Strength");
    assert!(matches!(
        validate_pod(&cat, &c),
        Err(MatchAdmissionError::ColorIdentity { .. })
    ));
    c = config();
    replace_basic(&mut c, 0, "Island");
    assert!(matches!(
        validate_pod(&cat, &c),
        Err(MatchAdmissionError::ColorIdentity { .. })
            | Err(MatchAdmissionError::BasicLandColor { .. })
    ));
    c = config();
    let land = oracle("Plains");
    let r = c.decks[0]
        .cards
        .iter_mut()
        .find(|r| r.oracle_id == land)
        .unwrap();
    r.count -= 1;
    c.decks[0].cards.push(DeckEntry {
        oracle_id: "not-an-authenticated-oracle-id".into(),
        count: 1,
    });
    assert!(matches!(
        validate_pod(&cat, &c),
        Err(MatchAdmissionError::UnsupportedCard { .. })
    ));
}

#[test]
fn current_known_ban_classification_precedes_generic_unsupported_pool_errors() {
    let cat = catalog();
    let policy: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/commander-policy.json"
    ))
    .unwrap();
    let ban = &policy["named_bans"].as_array().unwrap()[0];
    let id = ban["oracle_id"].as_str().unwrap();
    let mut c = config();
    let land = oracle("Plains");
    c.decks[0]
        .cards
        .iter_mut()
        .find(|r| r.oracle_id == land)
        .unwrap()
        .count -= 1;
    c.decks[0].cards.push(DeckEntry {
        oracle_id: id.into(),
        count: 1,
    });
    assert!(matches!(
        validate_pod(&cat, &c),
        Err(MatchAdmissionError::BannedCard {
            player: PlayerId(0),
            ..
        })
    ));
    c = config();
    c.decks[0].commander = id.into();
    assert!(matches!(
        validate_pod(&cat, &c),
        Err(MatchAdmissionError::BannedCard { .. })
    ));
    c = config();
    c.decks[0].commander = oracle("Elvish Visionary");
    assert!(matches!(
        validate_pod(&cat, &c),
        Err(MatchAdmissionError::UnsupportedCommander { .. })
    ));
}

#[test]
fn setup_configuration_requires_explicit_seed_start_and_schema_without_hidden_overrides() {
    let c = config();
    let base = serde_json::to_value(&c).unwrap();
    for field in ["seed", "starting_player", "decks"] {
        let mut forged = base.clone();
        forged.as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<MatchConfig>(forged).is_err());
    }
    for field in ["life", "library_order", "hand", "mana", "sideboard"] {
        let mut forged = base.clone();
        forged[field] = json!([]);
        assert!(serde_json::from_value::<MatchConfig>(forged).is_err());
    }
    let mut forged = base.clone();
    forged["seed"] = json!(vec![17; 31]);
    assert!(serde_json::from_value::<MatchConfig>(forged).is_err());
    forged = base.clone();
    forged["decks"][0]["cards"][0]["count"] = json!(-1);
    assert!(serde_json::from_value::<MatchConfig>(forged).is_err());
    forged = base;
    forged["decks"][0]["sideboard"] = json!([]);
    assert!(serde_json::from_value::<MatchConfig>(forged).is_err());
    let cat = catalog();
    let mut c = config();
    c.starting_player = PlayerId(4);
    assert!(matches!(
        validate_pod(&cat, &c),
        Err(MatchAdmissionError::InvalidStartingPlayer)
    ));
    c = config();
    c.profile = "arbitrary-commander".into();
    assert!(matches!(
        validate_pod(&cat, &c),
        Err(MatchAdmissionError::UnsupportedProfile)
    ));
}

fn prepared_match(c: MatchConfig) -> Prepared {
    Prepared::admit_pod(catalog(), c).unwrap()
}
fn finish_query(g: &Game, mut result: QueryResult) -> QueryValue {
    for _ in 0..100000 {
        match result {
            QueryResult::Complete(value) => return value,
            QueryResult::Incomplete { work, .. } => result = g.resume_work(work, Budget::new(16)),
            QueryResult::Rejected(e) => panic!("query rejected {e:?}"),
        }
    }
    panic!("query work bound")
}
fn settle_match(g: &mut Game, budget: u64) -> Advance {
    for _ in 0..20000 {
        let a = g.advance(Budget::new(budget));
        if !matches!(a, Advance::Suspended { .. }) {
            return a;
        }
    }
    panic!("automatic work bound")
}
fn observation(g: &Game, player: PlayerId) -> Observation {
    let QueryValue::Observation(o) = finish_query(g, g.observe(player, Budget::new(100000))) else {
        panic!("observation")
    };
    o
}
fn request(g: &Game) -> (PlayerId, PlayerRequest) {
    let p = g
        .live_players()
        .into_iter()
        .find(|p| g.decision_for(*p).unwrap().is_some())
        .expect("acting player");
    let QueryValue::Decision(Some(r)) =
        finish_query(g, g.player_decision_for(p, Budget::new(100000)))
    else {
        panic!("request")
    };
    (p, r)
}
fn submit_match(g: &mut Game, response: PlayerResponse) {
    let (p, r) = request(g);
    let QueryValue::Validated(receipt) = finish_query(
        g,
        g.check_player_response(p, r.token().clone(), response, Budget::new(100000)),
    ) else {
        panic!("receipt")
    };
    g.submit(receipt).unwrap();
}
fn input_match(g: &mut Game, response: PlayerResponse) {
    submit_match(g, response);
    assert!(matches!(settle_match(g, 1), Advance::AwaitingDecision(_)));
}
fn assert_restore(g: &Game, p: &Prepared) {
    let restored = Game::restore(p.clone(), &g.snapshot().unwrap()).unwrap();
    assert_eq!(restored.state_digest().unwrap(), g.state_digest().unwrap());
}
fn assert_replay(g: &Game, p: &Prepared) {
    let replay = Game::replay_with_trace(p.clone(), &g.replay_journal(), TraceMode::Off).unwrap();
    assert_eq!(replay.state_digest().unwrap(), g.state_digest().unwrap());
}
fn state(g: &Game) -> Value {
    serde_json::from_str::<Value>(&g.snapshot().unwrap()).unwrap()["state"].clone()
}
fn reject_input(g: &Game, p: PlayerId, token: DecisionToken, response: PlayerResponse) {
    let before = g.snapshot().unwrap();
    let mut r = g.check_player_response(p, token, response, Budget::new(0));
    for _ in 0..10000 {
        match r {
            QueryResult::Rejected(_) => {
                assert_eq!(before, g.snapshot().unwrap());
                return;
            }
            QueryResult::Incomplete { work, .. } => r = g.resume_work(work, Budget::new(16)),
            _ => panic!("illegal response accepted"),
        }
    }
    panic!("rejection work bound")
}
fn initial_match(c: MatchConfig, budget: u64) -> (Prepared, Game) {
    let p = prepared_match(c);
    let mut g = Game::with_trace(p.clone(), TraceMode::Off);
    assert!(matches!(
        settle_match(&mut g, budget),
        Advance::AwaitingDecision(_)
    ));
    (p, g)
}
fn keep_all(g: &mut Game) {
    while matches!(request(g).1, PlayerRequest::Mulligan { .. }) {
        input_match(g, PlayerResponse::Mulligan { take: false });
    }
}
fn hand_count(g: &Game, player: PlayerId) -> u32 {
    observation(g, player)
        .view
        .players
        .into_iter()
        .find(|p| p.player == player)
        .unwrap()
        .hand_count
}

#[test]
fn seed_shuffle_budget_partition_and_private_setup_views_are_deterministic() {
    let p = prepared_match(config());
    let mut a = Game::with_trace(p.clone(), TraceMode::Off);
    assert!(matches!(
        a.observe(PlayerId(0), Budget::new(100)),
        QueryResult::Rejected(QueryRejection::UnavailableDuringSetup)
    ));
    assert!(matches!(
        a.search_context(PlayerId(0), "public-sampling-seed".into(), Budget::new(100)),
        QueryResult::Rejected(QueryRejection::UnavailableDuringSetup)
    ));
    assert!(matches!(
        settle_match(&mut a, 1),
        Advance::AwaitingDecision(_)
    ));
    let mut b = Game::with_trace(p.clone(), TraceMode::Off);
    assert!(matches!(
        settle_match(&mut b, 4096),
        Advance::AwaitingDecision(_)
    ));
    assert_eq!(a.state_digest().unwrap(), b.state_digest().unwrap());
    assert_eq!(request(&a).0, PlayerId(2));
    assert_restore(&a, &p);
    assert_replay(&a, &p);
    for seat in 0..4 {
        let view = observation(&a, PlayerId(seat));
        assert_eq!(view.observation_version, 15);
        assert!(view.view.priority.is_none());
        assert_eq!(view.view.commanders.len(), 4);
        assert!(
            view.view
                .objects
                .iter()
                .all(|o| o.zone == Zone::Command { owner: o.owner }
                    || o.zone
                        == Zone::Hand {
                            owner: PlayerId(seat)
                        })
        );
        assert_eq!(
            view.view
                .objects
                .iter()
                .filter(|o| o.zone
                    == Zone::Hand {
                        owner: PlayerId(seat)
                    })
                .count(),
            7
        );
        assert!(
            view.view
                .players
                .iter()
                .all(|p| p.hand_count == 7 && p.library_count == 92)
        );
        let json = serde_json::to_string(&view).unwrap();
        for secret in [
            "word_position",
            "match_state",
            "random",
            "seed",
            "libraries",
        ] {
            assert!(!json.contains(&format!("\"{secret}\"")));
        }
    }
    let original = a.snapshot().unwrap();
    let mut fork = a.fork();
    input_match(&mut fork, PlayerResponse::Mulligan { take: true });
    assert_eq!(a.snapshot().unwrap(), original);
    let mut other = config();
    other.seed[0] ^= 1;
    let (_, different) = initial_match(other, 4096);
    assert_ne!(state(&a)["libraries"], state(&different)["libraries"]);
}

#[test]
fn declarations_finish_before_any_redraw_and_kept_players_never_rejoin() {
    let (p, mut g) = initial_match(config(), 4096);
    let before = state(&g);
    let libraries = before["libraries"].clone();
    let objects = before["objects"].clone();
    for seat in [2, 3, 0] {
        assert_eq!(request(&g).0, PlayerId(seat));
        input_match(
            &mut g,
            PlayerResponse::Mulligan {
                take: seat == 2 || seat == 0,
            },
        );
        assert_eq!(state(&g)["libraries"], libraries);
        assert_eq!(
            state(&g)["objects"],
            objects,
            "declarations cannot redraw early"
        );
    }
    assert_eq!(request(&g).0, PlayerId(1));
    submit_match(&mut g, PlayerResponse::Mulligan { take: false });
    assert!(matches!(
        g.advance(Budget::new(1)),
        Advance::Suspended { .. }
    ));
    assert!(matches!(
        g.observe(PlayerId(0), Budget::new(1000)),
        QueryResult::Rejected(QueryRejection::UnavailableDuringSetup)
    ));
    assert!(matches!(
        settle_match(&mut g, 1),
        Advance::AwaitingDecision(_)
    ));
    let (actor, r) = request(&g);
    assert_eq!(actor, PlayerId(2));
    assert!(matches!(
        r,
        PlayerRequest::Mulligan {
            mulligans_taken: 1,
            bottom_if_mulligan: 1,
            ..
        }
    ));
    assert_eq!(hand_count(&g, PlayerId(2)), 7);
    assert_eq!(hand_count(&g, PlayerId(0)), 7);
    assert_restore(&g, &p);
    input_match(&mut g, PlayerResponse::Mulligan { take: false });
    assert_eq!(request(&g).0, PlayerId(0));
    input_match(&mut g, PlayerResponse::Mulligan { take: false });
    assert!(matches!(request(&g).1, PlayerRequest::Priority { .. }));
    assert_replay(&g, &p);
}

#[test]
fn multiplayer_free_paid_mulligans_and_explicit_bottom_order_survive_restore() {
    let (p, mut g) = initial_match(config(), 4096);
    for i in 0..4 {
        input_match(&mut g, PlayerResponse::Mulligan { take: i == 0 });
    }
    assert_eq!(request(&g).0, PlayerId(2));
    input_match(&mut g, PlayerResponse::Mulligan { take: true });
    let (actor, req) = request(&g);
    let PlayerRequest::BottomOpening {
        count, candidates, ..
    } = &req
    else {
        panic!("paid bottom")
    };
    assert_eq!(actor, PlayerId(2));
    assert_eq!(*count, 1);
    assert_eq!(candidates.len(), 7);
    reject_input(
        &g,
        PlayerId(3),
        req.token().clone(),
        PlayerResponse::BottomOpening {
            objects: vec![candidates[0].clone()],
        },
    );
    reject_input(
        &g,
        actor,
        req.token().clone(),
        PlayerResponse::BottomOpening { objects: vec![] },
    );
    input_match(
        &mut g,
        PlayerResponse::BottomOpening {
            objects: vec![candidates[0].clone()],
        },
    );
    assert_eq!(hand_count(&g, actor), 6);
    input_match(&mut g, PlayerResponse::Mulligan { take: true });
    let (
        _,
        PlayerRequest::BottomOpening {
            count, candidates, ..
        },
    ) = request(&g)
    else {
        panic!("second paid bottom")
    };
    assert_eq!(count, 2);
    let mut a = g.fork();
    let mut b = g.fork();
    input_match(
        &mut a,
        PlayerResponse::BottomOpening {
            objects: candidates[..2].to_vec(),
        },
    );
    input_match(
        &mut b,
        PlayerResponse::BottomOpening {
            objects: vec![candidates[1].clone(), candidates[0].clone()],
        },
    );
    assert_eq!(hand_count(&a, actor), 5);
    assert_eq!(hand_count(&b, actor), 5);
    let sa = state(&a);
    let sb = state(&b);
    let la = sa["libraries"]["2"].as_array().unwrap();
    let lb = sb["libraries"]["2"].as_array().unwrap();
    assert_eq!(&la[..la.len() - 2], &lb[..lb.len() - 2]);
    assert_eq!(la[la.len() - 2], lb[lb.len() - 1]);
    assert_eq!(la[la.len() - 1], lb[lb.len() - 2]);
    assert_restore(&a, &p);
    assert_restore(&b, &p);
    input_match(&mut a, PlayerResponse::Mulligan { take: false });
    input_match(&mut b, PlayerResponse::Mulligan { take: false });
    assert_replay(&a, &p);
    assert_replay(&b, &p);
}

#[test]
fn maximum_eight_mulligans_forces_empty_keep_and_starting_player_still_draws() {
    let mut c = config();
    c.starting_player = PlayerId(0);
    let (p, mut g) = initial_match(c, 4096);
    for seat in 0..4 {
        assert_eq!(request(&g).0, PlayerId(seat));
        input_match(&mut g, PlayerResponse::Mulligan { take: seat == 0 });
    }
    for taken in 1..8 {
        let (actor, r) = request(&g);
        assert_eq!(actor, PlayerId(0));
        assert!(
            matches!(r,PlayerRequest::Mulligan{mulligans_taken:n,can_mulligan:true,..} if n==taken)
        );
        input_match(&mut g, PlayerResponse::Mulligan { take: true });
        let (
            _,
            PlayerRequest::BottomOpening {
                count, candidates, ..
            },
        ) = request(&g)
        else {
            panic!("bottom")
        };
        assert_eq!(count, usize::from(taken));
        input_match(
            &mut g,
            PlayerResponse::BottomOpening {
                objects: candidates[..count].to_vec(),
            },
        );
    }
    assert_eq!(hand_count(&g, PlayerId(0)), 0);
    assert!(matches!(request(&g).1, PlayerRequest::Priority { .. }));
    assert_restore(&g, &p);
    for _ in 0..20 {
        if hand_count(&g, PlayerId(0)) == 1 {
            break;
        }
        input_match(&mut g, PlayerResponse::Pass);
    }
    assert_eq!(
        hand_count(&g, PlayerId(0)),
        1,
        "multiplayer starting player gets first ordinary draw"
    );
    assert_restore(&g, &p);
    assert_replay(&g, &p);
}

#[test]
fn setup_restore_rejects_rehashed_random_order_and_declaration_forgery() {
    let p = prepared_match(config());
    let mut g = Game::with_trace(p.clone(), TraceMode::Off);
    assert!(matches!(
        g.advance(Budget::new(1)),
        Advance::Suspended { .. }
    ));
    assert_restore(&g, &p);
    assert!(matches!(
        settle_match(&mut g, 4096),
        Advance::AwaitingDecision(_)
    ));
    let original: Value = serde_json::from_str(&g.snapshot().unwrap()).unwrap();
    for change in 0..6 {
        let mut forged = original.clone();
        match change {
            0 => forged["state"]["match_state"]["random"]["seed"][0] = json!(99),
            1 => forged["state"]["match_state"]["random"]["stream"] = json!(1),
            2 => forged["state"]["match_state"]["random"]["word_position"] = json!("393"),
            3 => forged["state"]["match_state"]["setup"]["players"][2]["kept"] = json!(true),
            4 => forged["state"]["libraries"]["2"]
                .as_array_mut()
                .unwrap()
                .swap(0, 1),
            _ => forged["state"]["match_state"]["random"]["word_position"] = json!("0392"),
        }
        forged["state_sha256"] = json!(mtg_contracts::digest(&forged["state"]).unwrap());
        assert!(
            Game::restore(p.clone(), &serde_json::to_string(&forged).unwrap()).is_err(),
            "forgery{change}"
        );
    }
    assert_restore(&g, &p);
    assert_replay(&g, &p);
}

#[test]
fn bottom_query_recovery_receipts_and_handles_remain_executor_and_state_scoped() {
    let (p, mut g) = initial_match(config(), 4096);
    for i in 0..4 {
        input_match(&mut g, PlayerResponse::Mulligan { take: i == 0 });
    }
    input_match(&mut g, PlayerResponse::Mulligan { take: true });
    let (actor, req) = request(&g);
    let PlayerRequest::BottomOpening { candidates, .. } = &req else {
        panic!("bottom")
    };
    let response = PlayerResponse::BottomOpening {
        objects: vec![candidates[0].clone()],
    };
    let before = g.snapshot().unwrap();
    let QueryResult::Incomplete { work, .. } =
        g.check_player_response(actor, req.token().clone(), response.clone(), Budget::new(0))
    else {
        panic!("work")
    };
    let saved = g.save_work(&work).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&saved).unwrap()["work_version"],
        14
    );
    let mut restored = Game::restore(p.clone(), &before).unwrap();
    let work = restored.restore_work(&saved).unwrap();
    let QueryValue::Validated(receipt) =
        finish_query(&restored, restored.resume_work(work, Budget::new(1)))
    else {
        panic!("receipt")
    };
    assert!(g.submit(receipt).is_err());
    assert_eq!(g.snapshot().unwrap(), before);
    let work = restored.restore_work(&saved).unwrap();
    let QueryValue::Validated(receipt) =
        finish_query(&restored, restored.resume_work(work, Budget::new(1)))
    else {
        panic!("local receipt")
    };
    restored.submit(receipt).unwrap();
    assert!(matches!(
        settle_match(&mut restored, 1),
        Advance::AwaitingDecision(_)
    ));
    reject_input(&restored, actor, req.token().clone(), response);
    assert!(restored.restore_work(&saved).is_err());
    input_match(&mut restored, PlayerResponse::Mulligan { take: true });
    let (actor2, new_req) = request(&restored);
    reject_input(
        &restored,
        actor2,
        new_req.token().clone(),
        PlayerResponse::BottomOpening {
            objects: vec![candidates[0].clone(), candidates[1].clone()],
        },
    );
    let PlayerRequest::BottomOpening {
        candidates: new, ..
    } = new_req
    else {
        panic!("new bottom")
    };
    reject_input(
        &restored,
        actor2,
        request(&restored).1.token().clone(),
        PlayerResponse::BottomOpening {
            objects: vec![new[0].clone(), new[0].clone()],
        },
    );
    assert_restore(&restored, &p);
}

#[test]
fn concessions_before_setup_and_after_accepted_round_input_keep_original_seating() {
    let p = prepared_match(config());
    let mut g = Game::with_trace(p.clone(), TraceMode::Off);
    g.concede(PlayerId(2)).unwrap();
    assert!(matches!(
        settle_match(&mut g, 1),
        Advance::AwaitingDecision(_)
    ));
    assert_eq!(request(&g).0, PlayerId(3));
    assert_eq!(
        observation(&g, PlayerId(3)).view.live_turn_order,
        vec![PlayerId(0), PlayerId(1), PlayerId(3)]
    );
    assert_restore(&g, &p);
    keep_all(&mut g);
    assert_eq!(observation(&g, PlayerId(3)).view.active, Some(PlayerId(3)));
    assert_replay(&g, &p);
    let (p, mut g) = initial_match(config(), 4096);
    submit_match(&mut g, PlayerResponse::Mulligan { take: true });
    g.concede(PlayerId(2)).unwrap();
    assert!(matches!(
        settle_match(&mut g, 1),
        Advance::AwaitingDecision(_)
    ));
    assert_eq!(request(&g).0, PlayerId(3));
    keep_all(&mut g);
    assert_eq!(observation(&g, PlayerId(3)).view.active, Some(PlayerId(3)));
    assert_restore(&g, &p);
    assert_replay(&g, &p);
}

#[test]
fn partial_simultaneous_redraw_and_accepted_bottom_departures_never_short_draw_survivors() {
    let (p, mut g) = initial_match(config(), 4096);
    for seat in [2, 3, 0] {
        assert_eq!(request(&g).0, PlayerId(seat));
        input_match(
            &mut g,
            PlayerResponse::Mulligan {
                take: seat == 2 || seat == 0,
            },
        );
    }
    submit_match(&mut g, PlayerResponse::Mulligan { take: false });
    let mut partial = false;
    for _ in 0..2000 {
        let _ = g.advance(Budget::new(1));
        let s = state(&g);
        let phase = &s["match_state"]["setup"]["phase"];
        if phase["phase"] == "draw" && phase["players"][0] == 2 && phase["remaining"] == 3 {
            partial = true;
            break;
        }
    }
    assert!(
        partial,
        "pause inside first participant's seven-card simultaneous redraw"
    );
    assert!(matches!(
        g.observe(PlayerId(0), Budget::new(1000)),
        QueryResult::Rejected(QueryRejection::UnavailableDuringSetup)
    ));
    g.concede(PlayerId(2)).unwrap();
    assert!(matches!(
        settle_match(&mut g, 1),
        Advance::AwaitingDecision(_)
    ));
    assert_eq!(request(&g).0, PlayerId(0));
    assert_eq!(hand_count(&g, PlayerId(0)), 7);
    assert_eq!(hand_count(&g, PlayerId(1)), 7);
    assert_eq!(hand_count(&g, PlayerId(3)), 7);
    assert_restore(&g, &p);
    keep_all(&mut g);
    assert_eq!(observation(&g, PlayerId(3)).view.active, Some(PlayerId(3)));
    assert_replay(&g, &p);
    let (p, mut g) = initial_match(config(), 4096);
    for i in 0..4 {
        input_match(&mut g, PlayerResponse::Mulligan { take: i == 0 });
    }
    input_match(&mut g, PlayerResponse::Mulligan { take: true });
    let (_, PlayerRequest::BottomOpening { candidates, .. }) = request(&g) else {
        panic!("bottom")
    };
    submit_match(
        &mut g,
        PlayerResponse::BottomOpening {
            objects: vec![candidates[0].clone()],
        },
    );
    g.concede(PlayerId(2)).unwrap();
    assert!(matches!(
        settle_match(&mut g, 1),
        Advance::AwaitingDecision(_)
    ));
    assert!(matches!(request(&g).1, PlayerRequest::Priority { .. }));
    assert_eq!(observation(&g, PlayerId(3)).view.active, Some(PlayerId(3)));
    assert_restore(&g, &p);
    assert_replay(&g, &p);
}

fn exact_spending(
    cost: &mtg_contracts::ManaCost,
    available: &BTreeMap<Color, Quantity>,
) -> BTreeMap<Color, Quantity> {
    let mut rest = available.clone();
    let mut spent = BTreeMap::<Color, Quantity>::new();
    let one = Quantity::from(1);
    for color in &cost.colored {
        rest.get_mut(color).expect("colored mana").subtract(&one);
        spent.entry(*color).or_default().add(&one);
    }
    let mut generic = cost.generic.clone();
    for (color, n) in &mut rest {
        while *n > Quantity::zero() && generic > Quantity::zero() {
            n.subtract(&one);
            generic.subtract(&one);
            spent.entry(*color).or_default().add(&one);
        }
    }
    assert_eq!(generic, Quantity::zero());
    spent
}
fn dual_yargle_returns() -> (Prepared, Game) {
    let mut c = config();
    c.starting_player = PlayerId(1);
    let bg = Decklist {
        commander: oracle("Yargle and Multani"),
        cards: vec![
            DeckEntry {
                oracle_id: oracle("Yargle and Multani"),
                count: 1,
            },
            DeckEntry {
                oracle_id: oracle("Forest"),
                count: 49,
            },
            DeckEntry {
                oracle_id: oracle("Swamp"),
                count: 50,
            },
        ],
    };
    let w = Decklist {
        commander: oracle("Isamaru, Hound of Konda"),
        cards: vec![
            DeckEntry {
                oracle_id: oracle("Isamaru, Hound of Konda"),
                count: 1,
            },
            DeckEntry {
                oracle_id: oracle("Plains"),
                count: 99,
            },
        ],
    };
    c.decks = [bg.clone(), bg, w.clone(), w];
    let (p, mut g) = initial_match(c, 4096);
    keep_all(&mut g);
    let yargle = oracle("Yargle and Multani");
    let swamp = oracle("Swamp");
    let forest = oracle("Forest");
    for _ in 0..3000 {
        let (actor, r) = request(&g);
        let view = observation(&g, actor).view;
        let response = match r {
            PlayerRequest::CommanderReturn { .. } => return (p, g),
            PlayerRequest::Priority { .. } => {
                let main = view.active == Some(actor)
                    && matches!(
                        view.turn.step,
                        TurnStep::PrecombatMain | TurnStep::PostcombatMain
                    )
                    && view.stack.is_empty();
                let lands = view
                    .objects
                    .iter()
                    .filter(|o| {
                        o.controller == actor
                            && o.zone == Zone::Battlefield
                            && o.characteristics
                                .as_ref()
                                .is_some_and(|c| c.kind == CardKind::Land)
                    })
                    .collect::<Vec<_>>();
                if main && view.turn.lands_played == 0 {
                    let need = if lands.iter().filter(|o| o.oracle_id == swamp).count() < 2 {
                        &swamp
                    } else {
                        &forest
                    };
                    let land = view
                        .objects
                        .iter()
                        .filter(|o| {
                            o.zone == (Zone::Hand { owner: actor })
                                && o.characteristics
                                    .as_ref()
                                    .is_some_and(|c| c.kind == CardKind::Land)
                        })
                        .min_by_key(|o| o.oracle_id != *need);
                    if let Some(land) = land {
                        input_match(
                            &mut g,
                            PlayerResponse::PlayLand {
                                object: land.handle.clone(),
                            },
                        );
                        continue;
                    }
                }
                let ready = lands
                    .iter()
                    .filter(|o| !o.tapped)
                    .copied()
                    .collect::<Vec<_>>();
                if main
                    && actor.0 < 2
                    && ready.len() >= 6
                    && ready.iter().filter(|o| o.oracle_id == swamp).count() >= 2
                    && ready.iter().any(|o| o.oracle_id == forest)
                {
                    if let Some(cmd) = view.objects.iter().find(|o| {
                        o.owner == actor
                            && o.oracle_id == yargle
                            && o.zone == (Zone::Command { owner: actor })
                    }) {
                        PlayerResponse::Cast {
                            card: cmd.handle.clone(),
                        }
                    } else {
                        PlayerResponse::Pass
                    }
                } else {
                    PlayerResponse::Pass
                }
            }
            PlayerRequest::Mana { .. } => {
                if let Some(o) = view.objects.iter().find(|o| {
                    o.controller == actor
                        && o.zone == Zone::Battlefield
                        && !o.tapped
                        && o.characteristics
                            .as_ref()
                            .is_some_and(|c| c.kind == CardKind::Land)
                }) {
                    PlayerResponse::ActivateMana {
                        object: o.handle.clone(),
                    }
                } else {
                    PlayerResponse::FinishMana { undo_mana: false }
                }
            }
            PlayerRequest::SpendMana {
                cost, available, ..
            } => PlayerResponse::SpendMana {
                spending: exact_spending(&cost, &available),
            },
            PlayerRequest::Attackers { candidates, .. } => {
                let own = view.objects.iter().find(|o| {
                    o.controller == actor
                        && o.oracle_id == yargle
                        && o.zone == Zone::Battlefield
                        && o.attack_ready
                });
                let opposing = view.objects.iter().any(|o| {
                    o.controller == PlayerId(0)
                        && o.oracle_id == yargle
                        && o.zone == Zone::Battlefield
                });
                PlayerResponse::Attackers {
                    attacks: if actor == PlayerId(1) && opposing {
                        own.filter(|o| candidates.contains(&o.handle))
                            .map(|o| vec![(o.handle.clone(), PlayerId(0))])
                            .unwrap_or_default()
                    } else {
                        vec![]
                    },
                }
            }
            PlayerRequest::Blockers {
                candidates,
                attackers,
                ..
            } => PlayerResponse::Blockers {
                blocks: if actor == PlayerId(0) && !candidates.is_empty() && !attackers.is_empty() {
                    vec![(candidates[0].clone(), attackers[0].clone())]
                } else {
                    vec![]
                },
            },
            PlayerRequest::Damage {
                power, recipients, ..
            } => {
                assert_eq!(recipients.len(), 1);
                PlayerResponse::Damage {
                    assignments: vec![(recipients[0].clone(), power)],
                }
            }
            PlayerRequest::Discard {
                count, candidates, ..
            } => PlayerResponse::Discard {
                objects: candidates[..count].to_vec(),
            },
            _ => panic!("unexpected dual-commander request"),
        };
        input_match(&mut g, response);
    }
    panic!("dual Yargle natural development bound")
}
fn commander_zone(g: &Game, card: u32) -> String {
    let owner = PlayerId((card / 100) as u8);
    let view = observation(g, g.live_players()[0]).view;
    let id = &view
        .commanders
        .iter()
        .find(|c| c.history.owner == owner)
        .unwrap()
        .oracle_id;
    let object = view
        .objects
        .iter()
        .find(|o| o.owner == owner && o.oracle_id == *id)
        .unwrap();
    match object.zone {
        Zone::Graveyard { .. } => "graveyard",
        Zone::Command { .. } => "command",
        _ => panic!("unexpected commander zone"),
    }
    .into()
}

#[test]
fn simultaneous_commander_returns_choose_apnap_before_atomic_wave_and_survive_departures() {
    let (p, g) = dual_yargle_returns();
    let (actor, r) = request(&g);
    assert_eq!(
        actor,
        PlayerId(1),
        "active owner chooses before lower physicalID"
    );
    assert!(
        matches!(r,PlayerRequest::CommanderReturn{previous_choices,..} if previous_choices.is_empty())
    );
    assert_eq!(commander_zone(&g, 0), "graveyard");
    assert_eq!(commander_zone(&g, 100), "graveyard");
    let mut verifier = SnapshotVerifier::new(p.clone(), TraceMode::Off);
    verifier.verify(&g.snapshot().unwrap()).unwrap();
    let mut second = g.fork();
    input_match(
        &mut second,
        PlayerResponse::CommanderReturn { to_command: true },
    );
    let (actor, r) = request(&second);
    assert_eq!(actor, PlayerId(0));
    let PlayerRequest::CommanderReturn {
        previous_choices, ..
    } = r
    else {
        panic!("second owner")
    };
    assert_eq!(previous_choices.len(), 1);
    assert_eq!(previous_choices[0].owner, PlayerId(1));
    assert!(previous_choices[0].to_command);
    assert!(!previous_choices[0].designation.is_empty());
    assert_eq!(commander_zone(&second, 0), "graveyard");
    assert_eq!(
        commander_zone(&second, 100),
        "graveyard",
        "first accepted return must wait for simultaneous wave"
    );
    verifier.verify(&second.snapshot().unwrap()).unwrap();
    let mut yes = second.fork();
    submit_match(
        &mut yes,
        PlayerResponse::CommanderReturn { to_command: true },
    );
    for _ in 0..20 {
        let a = yes.advance(Budget::new(1));
        let left = commander_zone(&yes, 0);
        let right = commander_zone(&yes, 100);
        assert_eq!(left, right, "no single-return zone prefix may exist");
        if matches!(a, Advance::AwaitingDecision(_)) {
            break;
        }
    }
    assert_eq!(commander_zone(&yes, 0), "command");
    assert_eq!(commander_zone(&yes, 100), "command");
    verifier.verify(&yes.snapshot().unwrap()).unwrap();
    for (card, owner) in [(0, 0), (100, 1)] {
        let h = yes.commander_ledger().history(CardId(card)).unwrap();
        assert_eq!(h.command_casts[&PlayerId(owner)], Quantity::from(1));
        assert!(h.combat_damage.is_empty());
    }
    let mut no = second.fork();
    input_match(
        &mut no,
        PlayerResponse::CommanderReturn { to_command: false },
    );
    assert_eq!(commander_zone(&no, 0), "graveyard");
    assert_eq!(commander_zone(&no, 100), "command");
    input_match(&mut no, PlayerResponse::Pass);
    assert!(matches!(request(&no).1, PlayerRequest::Priority { .. }));
    let mut left = second.fork();
    left.concede(PlayerId(1)).unwrap();
    assert!(matches!(
        settle_match(&mut left, 1),
        Advance::AwaitingDecision(_)
    ));
    let (
        _,
        PlayerRequest::CommanderReturn {
            previous_choices, ..
        },
    ) = request(&left)
    else {
        panic!("surviving second owner")
    };
    assert_eq!(previous_choices.len(), 1);
    input_match(
        &mut left,
        PlayerResponse::CommanderReturn { to_command: true },
    );
    assert_eq!(commander_zone(&left, 0), "command");
    assert!(left.object_id(CardId(100)).is_none());
}

fn defender_seed(index: u16) -> [u8; 32] {
    let hex =
        mtg_contracts::sha256(format!("mtg-engine-p5-defender-adversarial-v1:{index}").as_bytes());
    std::array::from_fn(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap())
}

#[test]
fn naturally_cast_pumped_ready_defender_cannot_attack_and_later_blocks_normally() {
    // Separate witness, not a substituted completion-batch seed. Setup-only
    // deterministic search over0..255 first found index5/P3 withWall/Growth/Forest.
    let c = MatchConfig::foundations(defender_seed(5), PlayerId(0));
    assert_eq!(
        c.seed,
        [
            218, 242, 173, 204, 195, 32, 115, 232, 20, 238, 116, 21, 194, 222, 15, 79, 126, 111,
            50, 66, 12, 16, 130, 135, 143, 4, 248, 16, 154, 238, 167, 142
        ]
    );
    let (p, mut g) = initial_match(c, 4096);
    let wall = oracle("Wall of Wood");
    let growth = oracle("Giant Growth");
    let forest = oracle("Forest");
    let plains = oracle("Plains");
    let commander = oracle("Isamaru, Hound of Konda");
    let initial = observation(&g, PlayerId(3));
    for id in [&wall, &growth, &forest] {
        assert!(
            initial
                .view
                .objects
                .iter()
                .any(|o| o.zone == (Zone::Hand { owner: PlayerId(3) }) && o.oracle_id == *id)
        );
    }
    keep_all(&mut g);
    let mut pumped = false;
    let mut blocked = false;
    let mut verifier = SnapshotVerifier::new(p.clone(), TraceMode::Off);
    for _ in 0..1500 {
        let (actor, r) = request(&g);
        let view = observation(&g, actor).view;
        if blocked
            && view.objects.iter().any(|o| {
                o.oracle_id == wall
                    && o.owner == PlayerId(3)
                    && o.marked_damage == Quantity::from(2)
            })
        {
            let w = view
                .objects
                .iter()
                .find(|o| o.oracle_id == wall && o.owner == PlayerId(3))
                .unwrap();
            assert_eq!(w.zone, Zone::Battlefield);
            assert!(!w.tapped);
            assert_eq!(
                w.characteristics.as_ref().unwrap().power,
                Some(Quantity::zero())
            );
            assert_eq!(
                w.characteristics.as_ref().unwrap().toughness,
                Some(Quantity::from(3))
            );
            assert!(
                view.effects.is_empty(),
                "Growth expired in the preceding cleanup"
            );
            assert_eq!(
                view.players
                    .iter()
                    .find(|p| p.player == PlayerId(3))
                    .unwrap()
                    .life,
                Quantity::from(40)
            );
            assert!(
                g.commander_ledger()
                    .history(CardId(0))
                    .unwrap()
                    .combat_damage
                    .is_empty(),
                "blocking prevents commander player damage"
            );
            verifier.verify(&g.snapshot().unwrap()).unwrap();
            return;
        }
        let response = match r {
            PlayerRequest::Priority { .. } => {
                let main = view.active == Some(actor)
                    && matches!(
                        view.turn.step,
                        TurnStep::PrecombatMain | TurnStep::PostcombatMain
                    )
                    && view.stack.is_empty();
                let mana = if actor == PlayerId(3) {
                    &forest
                } else {
                    &plains
                };
                let lands = view
                    .objects
                    .iter()
                    .filter(|o| {
                        o.controller == actor && o.zone == Zone::Battlefield && o.oracle_id == *mana
                    })
                    .collect::<Vec<_>>();
                if main && matches!(actor, PlayerId(0) | PlayerId(3)) {
                    if lands.is_empty()
                        && view.turn.lands_played == 0
                        && let Some(o) = view.objects.iter().find(|o| {
                            o.zone == (Zone::Hand { owner: actor }) && o.oracle_id == *mana
                        })
                    {
                        input_match(
                            &mut g,
                            PlayerResponse::PlayLand {
                                object: o.handle.clone(),
                            },
                        );
                        continue;
                    }
                    if lands.iter().any(|o| !o.tapped) {
                        let spell = if actor == PlayerId(0) {
                            view.objects.iter().find(|o| {
                                o.oracle_id == commander
                                    && o.zone == (Zone::Command { owner: actor })
                            })
                        } else if !view.objects.iter().any(|o| {
                            o.oracle_id == wall
                                && o.controller == actor
                                && o.zone == Zone::Battlefield
                        }) {
                            view.objects.iter().find(|o| {
                                o.oracle_id == wall && o.zone == (Zone::Hand { owner: actor })
                            })
                        } else if view.objects.iter().any(|o| {
                            o.oracle_id == wall
                                && o.controller == actor
                                && o.zone == Zone::Battlefield
                                && o.attack_ready
                        }) {
                            view.objects.iter().find(|o| {
                                o.oracle_id == growth && o.zone == (Zone::Hand { owner: actor })
                            })
                        } else {
                            None
                        };
                        if let Some(o) = spell {
                            input_match(
                                &mut g,
                                PlayerResponse::Cast {
                                    card: o.handle.clone(),
                                },
                            );
                            continue;
                        }
                    }
                }
                PlayerResponse::Pass
            }
            PlayerRequest::Targets { candidates, .. } => {
                assert_eq!(actor, PlayerId(3));
                let w = view
                    .objects
                    .iter()
                    .find(|o| {
                        o.oracle_id == wall && o.controller == actor && o.zone == Zone::Battlefield
                    })
                    .unwrap();
                assert!(candidates.contains(&w.handle));
                PlayerResponse::Targets {
                    objects: vec![w.handle.clone()],
                }
            }
            PlayerRequest::Mana { .. } => {
                let mana = if actor == PlayerId(3) {
                    &forest
                } else {
                    &plains
                };
                if let Some(o) = view.objects.iter().find(|o| {
                    o.controller == actor
                        && o.zone == Zone::Battlefield
                        && o.oracle_id == *mana
                        && !o.tapped
                }) {
                    PlayerResponse::ActivateMana {
                        object: o.handle.clone(),
                    }
                } else {
                    PlayerResponse::FinishMana { undo_mana: false }
                }
            }
            PlayerRequest::SpendMana {
                cost, available, ..
            } => PlayerResponse::SpendMana {
                spending: exact_spending(&cost, &available),
            },
            PlayerRequest::Attackers {
                token, candidates, ..
            } => {
                if actor == PlayerId(3) && !pumped {
                    let w = view
                        .objects
                        .iter()
                        .find(|o| {
                            o.oracle_id == wall
                                && o.controller == actor
                                && o.zone == Zone::Battlefield
                        })
                        .unwrap();
                    if w.characteristics.as_ref().unwrap().power == Some(Quantity::from(3)) {
                        assert!(
                            w.attack_ready && !w.tapped,
                            "restriction proof excludes summoning sickness and tapping"
                        );
                        assert!(!candidates.contains(&w.handle));
                        verifier.verify(&g.snapshot().unwrap()).unwrap();
                        reject_input(
                            &g,
                            actor,
                            token,
                            PlayerResponse::Attackers {
                                attacks: vec![(w.handle.clone(), PlayerId(0))],
                            },
                        );
                        pumped = true;
                    }
                }
                PlayerResponse::Attackers {
                    attacks: if actor == PlayerId(0) && pumped {
                        view.objects
                            .iter()
                            .find(|o| {
                                o.oracle_id == commander
                                    && o.controller == actor
                                    && o.zone == Zone::Battlefield
                                    && candidates.contains(&o.handle)
                            })
                            .map(|o| vec![(o.handle.clone(), PlayerId(3))])
                            .unwrap_or_default()
                    } else {
                        vec![]
                    },
                }
            }
            PlayerRequest::Blockers {
                candidates,
                attackers,
                ..
            } => {
                if actor == PlayerId(3) && pumped && !attackers.is_empty() {
                    let w = view
                        .objects
                        .iter()
                        .find(|o| {
                            o.oracle_id == wall
                                && o.controller == actor
                                && o.zone == Zone::Battlefield
                        })
                        .unwrap();
                    assert!(candidates.contains(&w.handle));
                    assert_eq!(attackers.len(), 1);
                    blocked = true;
                    PlayerResponse::Blockers {
                        blocks: vec![(w.handle.clone(), attackers[0].clone())],
                    }
                } else {
                    PlayerResponse::Blockers { blocks: vec![] }
                }
            }
            PlayerRequest::Damage {
                power, recipients, ..
            } => {
                assert_eq!(recipients.len(), 1);
                PlayerResponse::Damage {
                    assignments: vec![(recipients[0].clone(), power)],
                }
            }
            PlayerRequest::Discard {
                count, candidates, ..
            } => PlayerResponse::Discard {
                objects: candidates[..count].to_vec(),
            },
            _ => panic!("unexpected Defender witness choice"),
        };
        input_match(&mut g, response);
    }
    panic!("predeclared focused Defender development bound1500")
}
