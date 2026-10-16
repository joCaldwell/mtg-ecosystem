//! Independent permitted-capture and hypothetical-origin adversarial proofs.
//! CR103.5/.5c,117.3/.4,302.6,400.2/.7,401.2/.3,402.3,701.24a,733.1/.2.
//! Constructors below read capture DTOs only; trusted real snapshots are used
//! separately to verify that paired test roots actually differ in hidden state.
use mtg_contracts::{CardKind, Catalog, canonical, digest, sha256};
use mtg_core::*;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

fn catalog() -> Catalog {
    Catalog::read(include_str!(
        "../../../tests/fixtures/expected-match-ir.json"
    ))
    .unwrap()
}
fn oracle(name: &str) -> String {
    catalog()
        .definitions
        .into_iter()
        .find(|d| d.name == name)
        .unwrap()
        .oracle_id
}
fn source_config(index: u16) -> MatchConfig {
    let seed = sha256(format!("p7-independent-capture/{index}").as_bytes());
    let bytes = std::array::from_fn(|i| u8::from_str_radix(&seed[2 * i..2 * i + 2], 16).unwrap());
    MatchConfig::foundations(bytes, PlayerId(0))
}
fn all_plains() -> Decklist {
    let commander = oracle("Isamaru, Hound of Konda");
    let mut cards = vec![
        DeckEntry {
            oracle_id: commander.clone(),
            count: 1,
        },
        DeckEntry {
            oracle_id: oracle("Plains"),
            count: 99,
        },
    ];
    cards.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    Decklist { commander, cards }
}
fn finish_query(g: &Game, mut q: QueryResult) -> QueryValue {
    for _ in 0..100_000 {
        match q {
            QueryResult::Complete(v) => return v,
            QueryResult::Incomplete { work, .. } => q = g.resume_work(work, Budget::new(16)),
            QueryResult::Rejected(e) => panic!("query rejected {e:?}"),
        }
    }
    panic!("query bound")
}
fn observe(g: &Game, p: PlayerId) -> Observation {
    let QueryValue::Observation(o) = finish_query(g, g.observe(p, Budget::new(100_000))) else {
        panic!("observation")
    };
    o
}
fn request(g: &Game) -> (PlayerId, PlayerRequest) {
    for p in g.live_players() {
        let QueryValue::Decision(r) =
            finish_query(g, g.player_decision_for(p, Budget::new(100_000)))
        else {
            panic!("decision")
        };
        if let Some(r) = r {
            return (p, r);
        }
    }
    panic!("no player request")
}
fn settle(g: &mut Game) {
    for _ in 0..100_000 {
        match g.advance(Budget::new(1024)) {
            Advance::Suspended { .. } => {}
            Advance::AwaitingDecision(_) => return,
            _ => panic!("root preparation terminated"),
        }
    }
    panic!("advance bound")
}
fn submit(g: &mut Game, response: PlayerResponse) {
    let (p, r) = request(g);
    let QueryValue::Validated(receipt) = finish_query(
        g,
        g.check_player_response(p, r.token().clone(), response, Budget::new(100_000)),
    ) else {
        panic!("receipt")
    };
    g.submit(receipt).unwrap();
    settle(g);
}
fn root(
    config: MatchConfig,
    mulligans: [u8; 4],
    own_turn: usize,
    departures: &[PlayerId],
) -> (Prepared, Game) {
    let p = Prepared::admit_pod(catalog(), config).unwrap();
    let mut g = Game::with_trace(p.clone(), TraceMode::Off);
    for player in departures {
        g.concede(*player).unwrap();
    }
    settle(&mut g);
    let mut seen = BTreeSet::new();
    for _ in 0..20_000 {
        let (player, r) = request(&g);
        let response = match r {
            PlayerRequest::Mulligan {
                mulligans_taken, ..
            } => PlayerResponse::Mulligan {
                take: mulligans_taken < mulligans[player.0 as usize],
            },
            PlayerRequest::BottomOpening {
                count, candidates, ..
            } => PlayerResponse::BottomOpening {
                objects: candidates.into_iter().rev().take(count).collect(),
            },
            PlayerRequest::Priority { .. } => {
                let view = observe(&g, player).view;
                if player == PlayerId(0)
                    && view.active == Some(player)
                    && view.turn.step == TurnStep::PrecombatMain
                {
                    seen.insert(view.turn.number);
                    if seen.len() == own_turn && view.turn.lands_played == 0 {
                        return (p, g);
                    }
                }
                let land = view.objects.iter().find(|o| {
                    o.zone == Zone::Hand { owner: player }
                        && o.characteristics
                            .as_ref()
                            .is_some_and(|c| c.kind == CardKind::Land)
                });
                if view.active == Some(player)
                    && view.turn.step == TurnStep::PrecombatMain
                    && view.turn.lands_played == 0
                {
                    if let Some(land) = land {
                        PlayerResponse::PlayLand {
                            object: land.handle.clone(),
                        }
                    } else {
                        PlayerResponse::Pass
                    }
                } else {
                    PlayerResponse::Pass
                }
            }
            PlayerRequest::Attackers { .. } => PlayerResponse::Attackers { attacks: vec![] },
            PlayerRequest::Blockers { .. } => PlayerResponse::Blockers { blocks: vec![] },
            PlayerRequest::Discard {
                count, candidates, ..
            } => PlayerResponse::Discard {
                objects: candidates.into_iter().take(count).collect(),
            },
            _ => panic!("unexpected root preparation request"),
        };
        submit(&mut g, response);
    }
    panic!("root preparation decision bound")
}
fn capture(g: &Game, quantum: u64) -> SearchCapture {
    let mut result = g.capture_search(PlayerId(0), Budget::new(quantum));
    for _ in 0..1_000_000 {
        match result {
            CaptureOutcome::Complete(c) => return c,
            CaptureOutcome::Incomplete {
                work,
                required_minimum,
            } => result = g.resume_search_capture(work, Budget::new(quantum.max(required_minimum))),
            CaptureOutcome::Rejected(e) => panic!("capture rejected {e:?}"),
        }
    }
    panic!("capture work bound")
}
fn take(pool: &mut BTreeMap<String, u32>, id: &str) {
    let n = pool
        .get_mut(id)
        .unwrap_or_else(|| panic!("missing remaining definition {id}"));
    assert!(*n > 0);
    *n -= 1;
}
/// A deterministic test constructor, deliberately separate from the production
/// sampler. All arguments are permitted capture data; no actual Game is read.
fn proposal(capture: &SearchCapture, reverse: bool) -> WorldProposal {
    let data = capture.data();
    let view = &data.context.observation.view;
    let mut serial = 0u64;
    let mut card = |owner: usize, id: String| {
        let c = HypotheticalCard {
            symbol: format!("hypothesis-{owner}-{serial}"),
            oracle_id: id,
        };
        serial += 1;
        c
    };
    let seats = std::array::from_fn(|seat| {
        let player = PlayerId(seat as u8);
        if view.players[seat].status != PlayerStatus::Live {
            return SeatHypothesis {
                hidden_hand: vec![],
                ordered_library: vec![],
            };
        }
        let mut pool: BTreeMap<String, u32> = data.profile.decks[seat]
            .cards
            .iter()
            .map(|r| (r.oracle_id.clone(), r.count))
            .collect();
        for object in view.objects.iter().filter(|o| o.owner == player) {
            take(&mut pool, &object.oracle_id)
        }
        let mut hand = vec![];
        if player != view.viewer {
            for memory in data.known_hands.iter().filter(|m| m.player == player) {
                take(&mut pool, &memory.oracle_id);
                hand.push(card(seat, memory.oracle_id.clone()));
            }
        }
        let bottom = if player == view.viewer {
            data.known_bottom
                .iter()
                .map(|m| m.oracle_id.clone())
                .collect::<Vec<_>>()
        } else {
            vec![]
        };
        for id in &bottom {
            take(&mut pool, id)
        }
        let mut remaining = pool
            .into_iter()
            .flat_map(|(id, n)| std::iter::repeat_n(id, n as usize))
            .collect::<Vec<_>>();
        if reverse {
            remaining.reverse();
        }
        let hand_size = if player == view.viewer {
            0
        } else {
            view.players[seat].hand_count as usize
        };
        while hand.len() < hand_size {
            hand.push(card(seat, remaining.remove(0)));
        }
        let library = remaining
            .into_iter()
            .chain(bottom)
            .map(|id| card(seat, id))
            .collect::<Vec<_>>();
        assert_eq!(library.len(), view.players[seat].library_count as usize);
        SeatHypothesis {
            hidden_hand: hand,
            ordered_library: library,
        }
    });
    WorldProposal {
        proposal_version: 1,
        context_id: capture.context_id().into(),
        sampler_version: mtg_core::SEARCH_SAMPLER.into(),
        world_index: 0,
        seats,
        future_seed: [73; 32],
    }
}
fn build(c: &SearchCapture, p: WorldProposal, quantum: u64) -> (SampledGame, RootHandleMap) {
    let mut result = SampledGame::build(catalog(), c, p, Budget::new(quantum));
    for _ in 0..1_000_000 {
        match result {
            SampleBuildOutcome::Complete { game, root_map } => return (game, root_map),
            SampleBuildOutcome::Incomplete {
                work,
                required_minimum,
            } => result = work.resume(Budget::new(quantum.max(required_minimum))),
            SampleBuildOutcome::Rejected(e) => panic!("sample rejected {e:?}"),
        }
    }
    panic!("sample build work bound")
}
fn rejects(c: &SearchCapture, p: WorldProposal) {
    let mut result = SampledGame::build(catalog(), c, p, Budget::new(1));
    for _ in 0..1_000_000 {
        match result {
            SampleBuildOutcome::Rejected(_) => return,
            SampleBuildOutcome::Incomplete {
                work,
                required_minimum,
            } => result = work.resume(Budget::new(16.max(required_minimum))),
            SampleBuildOutcome::Complete { .. } => panic!("invalid proposal admitted"),
        }
    }
    panic!("rejection work bound")
}
fn sample_query(g: &SampledGame, mut q: QueryResult) -> QueryValue {
    for _ in 0..100_000 {
        match q {
            QueryResult::Complete(v) => return v,
            QueryResult::Incomplete { work, .. } => q = g.resume_work(work, Budget::new(16)),
            QueryResult::Rejected(e) => panic!("sample query rejected {e:?}"),
        }
    }
    panic!("sample query work bound")
}
fn sample_observe(g: &SampledGame, p: PlayerId) -> Observation {
    let QueryValue::Observation(o) = sample_query(g, g.observe(p, Budget::new(100_000))) else {
        panic!("sample observation")
    };
    o
}

#[test]
fn capture_is_permitted_seed_free_and_budget_partitions_preserve_full_history() {
    let (_, g) = root(source_config(0), [2, 0, 0, 0], 2, &[]);
    let before = g.state_digest().unwrap();
    let a = capture(&g, 1);
    let b = capture(&g, 100_000);
    assert_eq!(canonical(a.data()).unwrap(), canonical(b.data()).unwrap());
    assert_eq!(a.context_id(), b.context_id());
    let data = a.data();
    let history_digest = digest(&data.context.permitted_history).unwrap();
    assert_eq!(
        a.context_id(),
        digest(&(
            "capture-identity-v1",
            data.capture_version,
            &data.profile,
            &data.context.observation,
            &data.own_knowledge,
            &data.known_bottom,
            &data.known_hands,
            history_digest
        ))
        .unwrap()
    );
    assert_eq!(before, g.state_digest().unwrap());
    assert!(a.data().context.permitted_history.len() > 4);
    assert!(!a.data().own_knowledge.is_empty());
    let value = serde_json::to_value(a.data()).unwrap();
    fn no_hidden_fields(value: &Value) {
        match value {
            Value::Object(map) => {
                for (k, v) in map {
                    assert!(
                        ![
                            "state_sha256",
                            "profile_digest",
                            "match_state",
                            "word_position",
                            "seed",
                            "source_state"
                        ]
                        .contains(&k.as_str()),
                        "forbidden field {k}"
                    );
                    no_hidden_fields(v)
                }
            }
            Value::Array(values) => {
                for v in values {
                    no_hidden_fields(v)
                }
            }
            _ => {}
        }
    }
    no_hidden_fields(&value);
    assert_eq!(
        a.data().profile.reconstruction_schema,
        "foundations-fresh-precombat-v1"
    );
    assert_eq!(
        a.data().profile.information_policy,
        "foundations-open-decklists-v1"
    );
}

#[test]
fn natural_indistinguishable_roots_produce_identical_proposals_and_mapped_samples() {
    let mut previous: BTreeMap<String, (u16, SearchCapture, Game)> = BTreeMap::new();
    // Named setup-only discovery domain, separate from the frozen workload.
    for index in 0..64 {
        let (_, g) = root(source_config(index), [0; 4], 1, &[]);
        let c = capture(&g, 100_000);
        let key = canonical(c.data()).unwrap();
        if let Some((prior_index, other, real)) = previous.remove(&key) {
            eprintln!(
                "natural permitted pair indices {prior_index}/{index} in p7-independent-capture domain"
            );
            assert_eq!(other.context_id(), c.context_id());
            assert_ne!(g.state_digest().unwrap(), real.state_digest().unwrap());
            // Test-only evidence that the true unseen definition order differs.
            let x: Value = serde_json::from_str(&g.snapshot().unwrap()).unwrap();
            let y: Value = serde_json::from_str(&real.snapshot().unwrap()).unwrap();
            let order = |s: &Value| {
                s["state"]["libraries"]["1"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|id| s["state"]["cards"][id.to_string()]["oracle_id"].clone())
                    .collect::<Vec<_>>()
            };
            assert_ne!(order(&x), order(&y));
            let pa = proposal(&other, false);
            let pb = proposal(&c, false);
            assert_eq!(canonical(&pa).unwrap(), canonical(&pb).unwrap());
            let (sa, ma) = build(&other, pa, 1);
            let (sb, mb) = build(&c, pb, 100_000);
            assert_eq!(sa.state_digest().unwrap(), sb.state_digest().unwrap());
            assert_eq!(
                ma.mapped_observation(&sample_observe(&sa, PlayerId(0)))
                    .unwrap(),
                other.data().context.observation
            );
            assert_eq!(
                mb.mapped_observation(&sample_observe(&sb, PlayerId(0)))
                    .unwrap(),
                c.data().context.observation
            );
            return;
        }
        previous.insert(key, (index, c, g));
        // Do not retain a large collection of authoritative games during search.
        if previous.len() > 32 {
            let key = previous.keys().next().unwrap().clone();
            previous.remove(&key);
        }
    }
    panic!("no natural paired root in fixed setup-only search domain");
}

#[test]
fn complete_proposals_reject_counts_duplicates_foreign_context_and_own_hand_overrides() {
    let (_, g) = root(source_config(1), [0; 4], 2, &[]);
    let c = capture(&g, 100_000);
    let base = proposal(&c, false);
    let _ = build(&c, base.clone(), 1);
    let mut p = base.clone();
    p.proposal_version = 2;
    rejects(&c, p);
    let mut p = base.clone();
    p.context_id = "another-root".into();
    rejects(&c, p);
    let mut p = base.clone();
    p.sampler_version = "unreviewed-prior".into();
    rejects(&c, p);
    let mut p = base.clone();
    let duplicate = p.seats[0].ordered_library[0].clone();
    p.seats[0].hidden_hand.push(duplicate);
    rejects(&c, p);
    let mut p = base.clone();
    p.seats[1].ordered_library.pop();
    rejects(&c, p);
    let mut p = base.clone();
    p.seats[1].ordered_library[0].symbol = p.seats[2].ordered_library[0].symbol.clone();
    rejects(&c, p);
    let mut p = base.clone();
    p.seats[1].ordered_library[0].oracle_id = oracle("Lightning Bolt");
    rejects(&c, p);
    let mut raw = serde_json::to_value(&base).unwrap();
    raw["actual_seed"] = json!(vec![17u8; 32]);
    assert!(serde_json::from_value::<WorldProposal>(raw).is_err());
}

#[test]
fn world_identity_ignores_labels_hand_order_and_rng_but_preserves_library_definitions() {
    let (_, g) = root(source_config(2), [0; 4], 2, &[]);
    let c = capture(&g, 100_000);
    let a = proposal(&c, false);
    let mut b = a.clone();
    b.world_index = 999;
    b.future_seed = [12; 32];
    for (seat, s) in b.seats.iter_mut().enumerate() {
        s.hidden_hand.reverse();
        for (index, card) in s
            .hidden_hand
            .iter_mut()
            .chain(s.ordered_library.iter_mut())
            .enumerate()
        {
            card.symbol = format!("renamed-{seat}-{index}");
        }
    }
    assert_eq!(
        a.assignment_digest().unwrap(),
        b.assignment_digest().unwrap()
    );
    let alt = proposal(&c, true);
    assert_ne!(
        a.assignment_digest().unwrap(),
        alt.assignment_digest().unwrap()
    );
    let _ = build(&c, b, 100_000);
}

#[test]
fn projected_root_and_each_chooser_keep_own_hands_private_with_no_hypothesis_handles() {
    let (_, g) = root(source_config(3), [2, 0, 0, 0], 2, &[]);
    let c = capture(&g, 100_000);
    let p = proposal(&c, false);
    let (game, map) = build(&c, p.clone(), 1);
    assert_eq!(
        map.mapped_observation(&sample_observe(&game, PlayerId(0)))
            .unwrap(),
        c.data().context.observation
    );
    let context = &c.data().context.observation;
    for object in &context.view.objects {
        assert!(map.object(&object.handle).is_ok());
    }
    assert!(
        map.object(&ObjectHandle(p.seats[1].hidden_hand[0].symbol.clone()))
            .is_err()
    );
    for viewer in 0..4 {
        let view = sample_observe(&game, PlayerId(viewer)).view;
        for object in &view.objects {
            if let Zone::Hand { owner } = object.zone {
                assert_eq!(owner, PlayerId(viewer));
            }
        }
        let encoded = canonical(&view).unwrap();
        for seat in &p.seats {
            for card in seat.hidden_hand.iter().chain(&seat.ordered_library) {
                assert!(!encoded.contains(&card.symbol));
            }
        }
    }
}

#[test]
fn hidden_library_order_and_future_rng_do_not_fingerprint_any_current_chooser_view() {
    let (_, g) = root(source_config(3), [2, 0, 0, 0], 2, &[]);
    let c = capture(&g, 100_000);
    let a = proposal(&c, false);
    let mut b = a.clone();
    let library = &mut b.seats[1].ordered_library;
    let different = library
        .iter()
        .position(|card| card.oracle_id != library[0].oracle_id)
        .expect("the black-green deck has distinct remaining definitions");
    library.swap(0, different);
    b.future_seed[0] ^= 1;
    assert_ne!(
        a.assignment_digest().unwrap(),
        b.assignment_digest().unwrap()
    );
    let (mut first, first_map) = build(&c, a, 100_000);
    let (mut second, second_map) = build(&c, b, 100_000);
    assert_ne!(
        first.state_digest().unwrap(),
        second.state_digest().unwrap()
    );
    for viewer in 0..4 {
        assert_eq!(
            sample_observe(&first, PlayerId(viewer)),
            sample_observe(&second, PlayerId(viewer)),
            "hidden assignment fingerprints viewer {viewer} before it is learned"
        );
    }
    assert_eq!(
        first_map
            .mapped_observation(&sample_observe(&first, PlayerId(0)))
            .unwrap(),
        second_map
            .mapped_observation(&sample_observe(&second, PlayerId(0)))
            .unwrap()
    );
    // Even a subsequent public transition may not expose an origin fingerprint.
    sample_pass(&mut first);
    sample_pass(&mut second);
    for viewer in 0..4 {
        assert_eq!(
            sample_observe(&first, PlayerId(viewer)),
            sample_observe(&second, PlayerId(viewer))
        );
    }
}

#[test]
fn departed_seats_stay_absent_and_original_seating_survives_three_and_two_seat_samples() {
    for absent in [vec![PlayerId(2)], vec![PlayerId(1), PlayerId(3)]] {
        let (_, g) = root(source_config(4), [2, 0, 0, 0], 2, &absent);
        let c = capture(&g, 100_000);
        let p = proposal(&c, false);
        for seat in &absent {
            assert!(p.seats[seat.0 as usize].hidden_hand.is_empty());
            assert!(p.seats[seat.0 as usize].ordered_library.is_empty());
        }
        let (game, map) = build(&c, p.clone(), 1);
        assert_eq!(
            map.mapped_observation(&sample_observe(&game, PlayerId(0)))
                .unwrap(),
            c.data().context.observation
        );
        let mut bad = p;
        bad.seats[absent[0].0 as usize]
            .ordered_library
            .push(HypotheticalCard {
                symbol: "resurrected-card".into(),
                oracle_id: oracle("Plains"),
            });
        rejects(&c, bad);
    }
}

fn paid_bottom() -> (Game, ObjectHandle, ObjectHandle) {
    for index in 0..32 {
        let p = Prepared::admit_pod(catalog(), source_config(index + 200)).unwrap();
        let mut g = Game::with_trace(p, TraceMode::Off);
        settle(&mut g);
        for _ in 0..100 {
            let (player, r) = request(&g);
            let response = match r {
                PlayerRequest::Mulligan {
                    mulligans_taken, ..
                } => PlayerResponse::Mulligan {
                    take: player == PlayerId(0) && mulligans_taken < 3,
                },
                PlayerRequest::BottomOpening {
                    count, candidates, ..
                } => {
                    if player == PlayerId(0) && count == 2 {
                        let view = observe(&g, player).view;
                        for a in &candidates {
                            for b in &candidates {
                                let aid = &view
                                    .objects
                                    .iter()
                                    .find(|o| o.handle == *a)
                                    .unwrap()
                                    .oracle_id;
                                let bid = &view
                                    .objects
                                    .iter()
                                    .find(|o| o.handle == *b)
                                    .unwrap()
                                    .oracle_id;
                                if aid != bid {
                                    return (g, a.clone(), b.clone());
                                }
                            }
                        }
                        break;
                    }
                    PlayerResponse::BottomOpening {
                        objects: candidates.into_iter().take(count).collect(),
                    }
                }
                _ => panic!("unexpected paid-bottom preparation stage"),
            };
            submit(&mut g, response);
        }
    }
    panic!("fixed seed domain lacks two distinct paid-bottom definitions")
}
fn opening_to_main(g: &mut Game, final_takes: u8) {
    for _ in 0..1000 {
        let (player, r) = request(g);
        let response = match r {
            PlayerRequest::Mulligan {
                mulligans_taken, ..
            } => PlayerResponse::Mulligan {
                take: player == PlayerId(0) && mulligans_taken < final_takes,
            },
            PlayerRequest::BottomOpening {
                count, candidates, ..
            } => PlayerResponse::BottomOpening {
                objects: candidates.into_iter().rev().take(count).collect(),
            },
            PlayerRequest::Priority { .. } => {
                let o = observe(g, player);
                if player == PlayerId(0) && o.view.turn.step == TurnStep::PrecombatMain {
                    return;
                }
                PlayerResponse::Pass
            }
            _ => panic!("unexpected opening completion stage"),
        };
        submit(g, response);
    }
    panic!("opening completion work bound")
}

#[test]
fn applied_opposite_bottom_orders_change_knowledge_without_changing_old_permitted_views() {
    let (source, a, b) = paid_bottom();
    let mut left = source.fork();
    let mut right = source.fork();
    submit(
        &mut left,
        PlayerResponse::BottomOpening {
            objects: vec![a.clone(), b.clone()],
        },
    );
    submit(
        &mut right,
        PlayerResponse::BottomOpening {
            objects: vec![b, a],
        },
    );
    opening_to_main(&mut left, 3);
    opening_to_main(&mut right, 3);
    let lc = capture(&left, 1);
    let rc = capture(&right, 100_000);
    assert_eq!(lc.data().context, rc.data().context);
    assert_ne!(lc.data().known_bottom, rc.data().known_bottom);
    assert_ne!(lc.context_id(), rc.context_id());
    assert_eq!(
        lc.data()
            .known_bottom
            .iter()
            .map(|c| &c.oracle_id)
            .collect::<Vec<_>>(),
        rc.data()
            .known_bottom
            .iter()
            .rev()
            .map(|c| &c.oracle_id)
            .collect::<Vec<_>>()
    );
    for c in [&lc, &rc] {
        let good = proposal(c, false);
        let _ = build(c, good.clone(), 1);
        let mut bad = good;
        let len = bad.seats[0].ordered_library.len();
        bad.seats[0].ordered_library.swap(len - 1, len - 2);
        rejects(c, bad);
    }
}

#[test]
fn later_applied_redraw_keeps_memory_but_replaces_old_position_constraints() {
    let (mut game, a, b) = paid_bottom();
    submit(
        &mut game,
        PlayerResponse::BottomOpening {
            objects: vec![a, b],
        },
    );
    // Take another mulligan after the earlier ordered two-card bottom.
    opening_to_main(&mut game, 4);
    let c = capture(&game, 100_000);
    let bottoms = c
        .data()
        .own_knowledge
        .iter()
        .filter_map(|action| match action {
            OwnKnowledgeAction::OpeningBottom { cards } => Some(cards),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(bottoms.iter().any(|cards| cards.len() == 2));
    assert_eq!(bottoms.last().unwrap().len(), 3);
    assert_eq!(&c.data().known_bottom, *bottoms.last().unwrap());
    let (game, map) = build(&c, proposal(&c, false), 1);
    for memory in &c.data().known_bottom {
        assert!(
            map.object(&ObjectHandle(memory.memory_ref.clone()))
                .is_err()
        );
    }
    assert_eq!(
        map.mapped_observation(&sample_observe(&game, PlayerId(0)))
            .unwrap(),
        c.data().context.observation
    );
}

fn sample_restore(
    c: &SearchCapture,
    json: &str,
    quantum: u64,
) -> Result<SampledGame, SearchRejection> {
    let mut result = SampledGame::restore(catalog(), c, json, Budget::new(quantum));
    for _ in 0..1_000_000 {
        match result {
            SampleRestoreOutcome::Complete(g) => return Ok(g),
            SampleRestoreOutcome::Rejected(e) => return Err(e),
            SampleRestoreOutcome::Incomplete {
                work,
                required_minimum,
            } => result = work.resume(Budget::new(quantum.max(required_minimum))),
        }
    }
    panic!("restore work bound")
}
fn sample_pass(g: &mut SampledGame) {
    let o = sample_observe(g, PlayerId(0));
    let r = o.decision.expect("root priority");
    let QueryValue::Validated(receipt) = sample_query(
        g,
        g.check_player_response(
            PlayerId(0),
            r.token().clone(),
            PlayerResponse::Pass,
            Budget::new(100_000),
        ),
    ) else {
        panic!("sample receipt")
    };
    g.submit_validated(receipt).unwrap();
    for _ in 0..10000 {
        if !matches!(g.advance(Budget::new(1)), Advance::Suspended { .. }) {
            return;
        }
    }
    panic!("sample pass advance bound")
}

#[test]
fn sample_origin_restores_by_its_own_future_journal_and_cannot_be_a_real_snapshot() {
    let (prepared, g) = root(source_config(5), [2, 0, 0, 0], 2, &[]);
    let c = capture(&g, 100_000);
    let (mut sample, _) = build(&c, proposal(&c, false), 1);
    let before = sample.state_digest().unwrap();
    let fork = sample.fork();
    sample_pass(&mut sample);
    assert_eq!(fork.state_digest().unwrap(), before);
    assert_ne!(sample.state_digest().unwrap(), before);
    let raw = sample.snapshot().unwrap();
    let restored = sample_restore(&c, &raw, 1).unwrap();
    assert_eq!(
        restored.state_digest().unwrap(),
        sample.state_digest().unwrap()
    );
    assert!(Game::restore(prepared, &raw).is_err());

    // A valid alternative proposal and correctly recomputed public origin hash
    // still cannot authorize the old world's claimed final state.
    let mut value: Value = serde_json::from_str(&raw).unwrap();
    let mut changed: WorldProposal = serde_json::from_value(value["proposal"].clone()).unwrap();
    changed.future_seed[0] ^= 1;
    let (alternative, _) = build(&c, changed.clone(), 100_000);
    assert_ne!(alternative.state_digest().unwrap(), before);
    value["proposal"] = serde_json::to_value(&changed).unwrap();
    value["origin_id"] = digest(&(c.context_id(), &changed)).unwrap().into();
    assert!(sample_restore(&c, &canonical(&value).unwrap(), 1).is_err());

    // The future journal and rule boundary are proved rather than trusted.
    let mut value: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(value["journal"]["decisions"].as_array().unwrap().len(), 1);
    value["journal"]["decisions"][0]["player"] = 1.into();
    assert!(sample_restore(&c, &canonical(&value).unwrap(), 1).is_err());
    let mut value: Value = serde_json::from_str(&raw).unwrap();
    value["journal"]["decisions"] = json!([]);
    assert!(sample_restore(&c, &canonical(&value).unwrap(), 1).is_err());
    let mut value: Value = serde_json::from_str(&raw).unwrap();
    value["boundary"] = (value["boundary"].as_u64().unwrap() - 1).into();
    assert!(sample_restore(&c, &canonical(&value).unwrap(), 1).is_err());

    let mut value: Value = serde_json::from_str(&raw).unwrap();
    value["sample_snapshot_version"] = 2.into();
    assert!(sample_restore(&c, &canonical(&value).unwrap(), 1).is_err());
    let mut value: Value = serde_json::from_str(&raw).unwrap();
    value["unreviewed_origin"] = json!({"take_real_hidden_state":true});
    assert!(sample_restore(&c, &canonical(&value).unwrap(), 1).is_err());
    let (_, other) = root(source_config(6), [0; 4], 2, &[]);
    let foreign = capture(&other, 100_000);
    assert!(sample_restore(&foreign, &raw, 1).is_err());
}

#[test]
fn hypothetical_query_work_and_receipts_are_foreign_stale_and_recovery_prefix_scoped() {
    let (_, g) = root(source_config(7), [0; 4], 2, &[]);
    let c = capture(&g, 100_000);
    let (mut a, _) = build(&c, proposal(&c, false), 1);
    let (b, _) = build(&c, proposal(&c, true), 1);
    let o = sample_observe(&a, PlayerId(0));
    let token = o.decision.unwrap().token().clone();
    let QueryResult::Incomplete { work, .. } = a.check_player_response(
        PlayerId(0),
        token.clone(),
        PlayerResponse::Pass,
        Budget::new(0),
    ) else {
        panic!("zero-budget check")
    };
    let sidecar = a.save_work(&work).unwrap();
    assert!(b.restore_work(&sidecar).is_err());
    let mut bad: Value = serde_json::from_str(&sidecar).unwrap();
    bad["data"]["units_done"] = json!(999999999u64);
    assert!(a.restore_work(&canonical(&bad).unwrap()).is_err());
    let recovered = a.restore_work(&sidecar).unwrap();
    let QueryValue::Validated(receipt) =
        sample_query(&a, a.resume_work(recovered, Budget::new(100_000)))
    else {
        panic!("recovered receipt")
    };
    let mut fork = a.fork();
    assert!(fork.submit_validated(receipt).is_err());
    let QueryValue::Validated(receipt) = sample_query(
        &a,
        a.check_player_response(
            PlayerId(0),
            token,
            PlayerResponse::Pass,
            Budget::new(100_000),
        ),
    ) else {
        panic!("receipt")
    };
    a.submit_validated(receipt).unwrap();
    assert!(a.restore_work(&sidecar).is_err());
    assert!(matches!(
        a.resume_work(work, Budget::new(1)),
        QueryResult::Rejected(_)
    ));
}

#[test]
fn capture_work_is_pure_and_rejects_foreign_or_mutated_actual_roots() {
    let (_, mut g) = root(source_config(8), [0; 4], 2, &[]);
    let CaptureOutcome::Incomplete { work, .. } = g.capture_search(PlayerId(0), Budget::new(0))
    else {
        panic!("zero-budget capture")
    };
    assert!(matches!(
        g.fork().resume_search_capture(work, Budget::new(1)),
        CaptureOutcome::Rejected(SearchRejection::StaleWork)
    ));
    let CaptureOutcome::Incomplete { work, .. } = g.capture_search(PlayerId(0), Budget::new(0))
    else {
        panic!("zero-budget capture")
    };
    submit(&mut g, PlayerResponse::Pass);
    assert!(matches!(
        g.resume_search_capture(work, Budget::new(1)),
        CaptureOutcome::Rejected(SearchRejection::StaleWork)
    ));
    assert!(matches!(
        g.capture_search(PlayerId(0), Budget::new(1)),
        CaptureOutcome::Rejected(SearchRejection::UnsupportedRoot(_))
    ));
    assert!(matches!(
        g.capture_search(PlayerId(1), Budget::new(1)),
        CaptureOutcome::Rejected(SearchRejection::UnsupportedRoot(_))
    ));
}

fn sample_step(g: &mut SampledGame) -> (PlayerId, PlayerRequest) {
    for _ in 0..10000 {
        match g.advance(Budget::new(1024)) {
            Advance::Suspended { .. } => {}
            Advance::AwaitingDecision(r) => {
                let p = r.player();
                let o = sample_observe(g, p);
                return (p, o.decision.expect("sample acting decision"));
            }
            _ => panic!("sample rollout unexpectedly terminated"),
        }
    }
    panic!("sample automatic work bound")
}
fn sample_submit(g: &mut SampledGame, p: PlayerId, r: &PlayerRequest, response: PlayerResponse) {
    let QueryValue::Validated(receipt) = sample_query(
        g,
        g.check_player_response(p, r.token().clone(), response, Budget::new(100_000)),
    ) else {
        panic!("sample receipt")
    };
    g.submit_validated(receipt).unwrap();
}
fn sample_to_own_draw(g: &mut SampledGame, library: u32) -> Observation {
    for _ in 0..1000 {
        let (p, r) = sample_step(g);
        let own = sample_observe(g, PlayerId(0));
        if own.view.players[0].library_count == library - 1 {
            return own;
        }
        let view = sample_observe(g, p).view;
        for object in &view.objects {
            if let Zone::Hand { owner } = object.zone {
                assert_eq!(owner, p);
            }
        }
        let response = match &r {
            PlayerRequest::Priority { .. } => PlayerResponse::Pass,
            PlayerRequest::Attackers { .. } => PlayerResponse::Attackers { attacks: vec![] },
            PlayerRequest::Blockers { .. } => PlayerResponse::Blockers { blocks: vec![] },
            PlayerRequest::Discard {
                count, candidates, ..
            } => PlayerResponse::Discard {
                objects: candidates.iter().take(*count).cloned().collect(),
            },
            _ => panic!("unexpected no-action sample rollout choice"),
        };
        sample_submit(g, p, &r, response);
    }
    panic!("next own draw decision bound")
}
#[test]
fn future_draw_uses_hypothesized_library_without_touching_true_game_or_other_sample() {
    let (_, real) = root(source_config(9), [0; 4], 2, &[]);
    let before = real.state_digest().unwrap();
    let c = capture(&real, 100_000);
    let mut a = proposal(&c, false);
    let different = a.seats[0]
        .ordered_library
        .iter()
        .position(|card| card.oracle_id != a.seats[0].ordered_library[0].oracle_id)
        .expect("two future own definitions");
    let mut b = a.clone();
    b.seats[0].ordered_library.swap(0, different);
    a.future_seed = [43; 32];
    b.future_seed = [44; 32];
    let a_top = a.seats[0].ordered_library[0].oracle_id.clone();
    let b_top = b.seats[0].ordered_library[0].oracle_id.clone();
    assert_ne!(a_top, b_top);
    let (mut ga, ma) = build(&c, a, 1);
    let (mut gb, mb) = build(&c, b, 1);
    assert_eq!(
        ma.mapped_observation(&sample_observe(&ga, PlayerId(0)))
            .unwrap(),
        c.data().context.observation
    );
    assert_eq!(
        mb.mapped_observation(&sample_observe(&gb, PlayerId(0)))
            .unwrap(),
        c.data().context.observation
    );
    let independent = gb.state_digest().unwrap();
    let library = c.data().context.observation.view.players[0].library_count;
    let oa = sample_to_own_draw(&mut ga, library);
    assert_eq!(gb.state_digest().unwrap(), independent);
    let ob = sample_to_own_draw(&mut gb, library);
    let hand = |o: &Observation| {
        let mut counts = BTreeMap::<String, usize>::new();
        for object in &o.view.objects {
            if object.zone == (Zone::Hand { owner: PlayerId(0) }) {
                *counts.entry(object.oracle_id.clone()).or_default() += 1;
            }
        }
        counts
    };
    let mut ac = hand(&oa);
    let mut bc = hand(&ob);
    *ac.get_mut(&a_top).unwrap() -= 1;
    *bc.get_mut(&b_top).unwrap() -= 1;
    ac.retain(|_, n| *n > 0);
    bc.retain(|_, n| *n > 0);
    assert_eq!(ac, bc);
    assert_eq!(before, real.state_digest().unwrap());
}

#[test]
fn interchangeable_basic_copies_and_rng_labels_cannot_manufacture_eight_worlds() {
    let mut config = source_config(10);
    config.decks = std::array::from_fn(|_| all_plains());
    let (_, g) = root(config, [0; 4], 2, &[]);
    let c = capture(&g, 100_000);
    let base = proposal(&c, false);
    let mut assignments = BTreeSet::new();
    for index in 0..8 {
        let mut p = base.clone();
        p.world_index = index;
        p.future_seed = [index as u8; 32];
        for (seat, s) in p.seats.iter_mut().enumerate() {
            s.hidden_hand.reverse();
            s.ordered_library.reverse();
            for (i, card) in s
                .hidden_hand
                .iter_mut()
                .chain(s.ordered_library.iter_mut())
                .enumerate()
            {
                card.symbol = format!("copy-label-{index}-{seat}-{i}");
            }
        }
        assignments.insert(p.assignment_digest().unwrap());
    }
    assert_eq!(
        assignments.len(),
        1,
        "this domain cannot honestly satisfy the eight-world requirement"
    );
}

#[test]
fn completed_command_cast_preserves_public_tax_mana_and_newborn_readiness() {
    let (_, mut g) = root(source_config(11), [0; 4], 2, &[]);
    let view = observe(&g, PlayerId(0)).view;
    let commander = view
        .objects
        .iter()
        .find(|o| o.owner == PlayerId(0) && o.zone == (Zone::Command { owner: PlayerId(0) }))
        .unwrap()
        .handle
        .clone();
    let land = view
        .objects
        .iter()
        .find(|o| {
            o.owner == PlayerId(0)
                && o.zone == Zone::Battlefield
                && o.oracle_id == oracle("Plains")
                && !o.tapped
        })
        .unwrap()
        .handle
        .clone();
    submit(&mut g, PlayerResponse::Cast { card: commander });
    assert!(matches!(
        g.capture_search(PlayerId(0), Budget::new(1)),
        CaptureOutcome::Rejected(SearchRejection::UnsupportedRoot(_))
    ));
    submit(&mut g, PlayerResponse::ActivateMana { object: land });
    submit(&mut g, PlayerResponse::FinishMana { undo_mana: false });
    submit(
        &mut g,
        PlayerResponse::SpendMana {
            spending: BTreeMap::from([(mtg_contracts::Color::W, mtg_contracts::Quantity::from(1))]),
        },
    );
    for _ in 0..4 {
        submit(&mut g, PlayerResponse::Pass);
    }
    let c = capture(&g, 100_000);
    let obj = c
        .data()
        .context
        .observation
        .view
        .objects
        .iter()
        .find(|o| {
            o.owner == PlayerId(0)
                && o.zone == Zone::Battlefield
                && o.oracle_id == oracle("Isamaru, Hound of Konda")
        })
        .unwrap();
    assert!(!obj.attack_ready);
    let history = &c
        .data()
        .context
        .observation
        .view
        .commanders
        .iter()
        .find(|c| c.history.owner == PlayerId(0))
        .unwrap()
        .history;
    assert_eq!(
        history.command_casts[&PlayerId(0)],
        mtg_contracts::Quantity::from(1)
    );
    let (sample, map) = build(&c, proposal(&c, false), 1);
    assert_eq!(
        map.mapped_observation(&sample_observe(&sample, PlayerId(0)))
            .unwrap(),
        c.data().context.observation
    );
}

fn root_after_failed_opponent_announcement() -> (Game, String) {
    for index in 300..332 {
        let (_, mut g) = root(source_config(index), [0; 4], 1, &[]);
        let mut revealed: Option<String> = None;
        for _ in 0..1000 {
            let (player, r) = request(&g);
            let view = observe(&g, player).view;
            if let Some(oracle_id) = revealed.as_ref()
                && player == PlayerId(0)
                && view.active == Some(player)
                && view.turn.step == TurnStep::PrecombatMain
            {
                return (g, oracle_id.clone());
            }
            let response = match r {
                PlayerRequest::Priority { .. } => {
                    if revealed.is_none()
                        && player == PlayerId(1)
                        && view.active == Some(player)
                        && view.turn.step == TurnStep::PrecombatMain
                    {
                        let creature = view.objects.iter().find(|o| {
                            o.zone == (Zone::Hand { owner: player })
                                && o.characteristics
                                    .as_ref()
                                    .is_some_and(|c| c.kind == CardKind::Creature)
                        });
                        if let Some(creature) = creature {
                            revealed = Some(creature.oracle_id.clone());
                            PlayerResponse::Cast {
                                card: creature.handle.clone(),
                            }
                        } else {
                            break;
                        }
                    } else {
                        PlayerResponse::Pass
                    }
                }
                PlayerRequest::Mana { .. } => PlayerResponse::FinishMana { undo_mana: true },
                PlayerRequest::Discard {
                    count, candidates, ..
                } => PlayerResponse::Discard {
                    objects: candidates
                        .into_iter()
                        .filter(|h| {
                            view.objects
                                .iter()
                                .find(|o| o.handle == *h)
                                .is_some_and(|o| Some(&o.oracle_id) != revealed.as_ref())
                        })
                        .take(count)
                        .collect(),
                },
                PlayerRequest::Attackers { .. } => PlayerResponse::Attackers { attacks: vec![] },
                PlayerRequest::Blockers { .. } => PlayerResponse::Blockers { blocks: vec![] },
                _ => panic!("unexpected failed-announcement fixture choice"),
            };
            submit(&mut g, response);
        }
    }
    panic!("fixed fixture domain lacks an opponent creature to reveal")
}
#[test]
fn reversed_public_announcement_constrains_the_opponent_hand_without_granting_a_handle() {
    let (g, revealed) = root_after_failed_opponent_announcement();
    let c = capture(&g, 100_000);
    assert!(
        c.data()
            .known_hands
            .iter()
            .any(|m| m.player == PlayerId(1) && m.oracle_id == revealed)
    );
    assert!(
        !c.data()
            .context
            .observation
            .view
            .objects
            .iter()
            .any(|o| o.owner == PlayerId(1) && o.oracle_id == revealed)
    );
    let good = proposal(&c, false);
    let (sample, _) = build(&c, good.clone(), 1);
    assert!(
        !sample_observe(&sample, PlayerId(0))
            .view
            .objects
            .iter()
            .any(|o| o.owner == PlayerId(1) && o.oracle_id == revealed)
    );
    let mut bad = good;
    let h = bad.seats[1]
        .hidden_hand
        .iter()
        .position(|card| card.oracle_id == revealed)
        .unwrap();
    let l = bad.seats[1]
        .ordered_library
        .iter()
        .position(|card| card.oracle_id != revealed)
        .unwrap();
    let replacement = bad.seats[1].ordered_library[l].clone();
    bad.seats[1].ordered_library[l] = bad.seats[1].hidden_hand[h].clone();
    bad.seats[1].hidden_hand[h] = replacement;
    rejects(&c, bad);
}
