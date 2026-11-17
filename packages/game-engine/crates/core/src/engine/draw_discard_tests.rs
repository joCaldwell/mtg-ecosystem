//! Private seeded 100-card setup, paid Careful Study, no profile/consumer admission.
//! CR401.4/608.2c/701.24/903.9b, frozen20260925.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
const STUDY: &str =
    include_str!("../../../../tests/fixtures/32e9aa23-fc0a-4b82-82f9-1659c304428c.json");
fn seed() -> [u8; 32] {
    // Discovery only. Actual tests run the real setup and verify the opening.
    static SEED: std::sync::OnceLock<[u8; 32]> = std::sync::OnceLock::new();
    *SEED.get_or_init(|| {
        for byte in 0..=u8::MAX {
            let mut random = MatchRandom {
                version: 1,
                seed: [byte; 32],
                stream: 0,
                word_position: RngPosition(0),
            };
            let mut cards = (1..100).collect::<Vec<u32>>();
            for i in (1..99).rev() {
                loop {
                    if let Some(j) =
                        setup::sample_shuffle_word(random.word().unwrap(), i as u32 + 1)
                    {
                        cards.swap(i, j as usize);
                        break;
                    }
                }
            }
            if cards[..7].contains(&1) {
                return [byte; 32];
            }
        }
        panic!("seed discovery")
    })
}
fn prepared() -> Prepared {
    let mut p = prepared_with_sources(&[], &[STUDY]);
    let commander = p
        .catalog
        .definitions
        .iter()
        .find(|d| d.name == "Caelorna, Coral Tyrant")
        .unwrap()
        .oracle_id
        .clone();
    let names = [
        "Careful Study",
        "Storm Crow",
        "Counterspell",
        "Unsummon",
        "Swan Song",
    ];
    p.config.profile = "controller-draw-discard-private-seeded".into();
    p.config.mode = Mode::CommanderMatch;
    p.config.initial.turn_step = TurnStep::Untap;
    p.config.initial.libraries = None;
    p.config.cards.clear();
    p.config.commanders.clear();
    for seat in 0..4 {
        let owner = PlayerId(seat);
        let id = CardId(u32::from(seat) * 100);
        p.config.commanders.push(id);
        p.config.cards.push(CardSetup {
            id,
            oracle_id: commander.clone(),
            owner,
            controller: owner,
            zone: Zone::Command { owner },
        });
        for i in 1..100 {
            let name = if seat == 0 && i <= 5 {
                names[i as usize - 1]
            } else {
                "Island"
            };
            let oracle = p
                .catalog
                .definitions
                .iter()
                .find(|d| d.name == name)
                .unwrap()
                .oracle_id
                .clone();
            p.config.cards.push(CardSetup {
                id: CardId(id.0 + i),
                oracle_id: oracle,
                owner,
                controller: owner,
                zone: Zone::Library { owner },
            });
        }
    }
    let decks = std::array::from_fn(|seat| {
        let mut counts = BTreeMap::<String, u32>::new();
        for c in p
            .config
            .cards
            .iter()
            .filter(|c| c.owner == PlayerId(seat as u8))
        {
            *counts.entry(c.oracle_id.clone()).or_default() += 1;
        }
        Decklist {
            commander: commander.clone(),
            cards: counts
                .into_iter()
                .map(|(oracle_id, count)| DeckEntry { oracle_id, count })
                .collect(),
        }
    });
    p.match_config = Some(MatchConfig {
        profile: p.config.profile.clone(),
        decks,
        starting_player: PlayerId(0),
        seed: seed(),
    });
    p.profile_digest = digest(&(&p.config, &p.match_config)).unwrap();
    p
}
fn library(g: &Game, seat: u8) -> Vec<ObjectId> {
    g.state
        .rules
        .libraries
        .get(&PlayerId(seat))
        .into_iter()
        .flatten()
        .map(|c| g.object_id(*c).unwrap())
        .collect()
}
fn develop(g: &mut Game, q: u64) -> CardId {
    for _ in 0..1000 {
        let r = request(g, q);
        if matches!(r, Request::Mulligan { .. }) {
            g.respond(
                r.player(),
                r.decision_id(),
                Response::Mulligan { take: false },
            )
            .unwrap();
            continue;
        }
        if matches!(
            r,
            Request::Priority {
                player: PlayerId(0),
                ..
            }
        ) && g.active_player() == Some(PlayerId(0))
            && matches!(
                g.turn().step,
                TurnStep::PrecombatMain | TurnStep::PostcombatMain
            )
        {
            assert_eq!(
                g.current(object(g, 1)).unwrap().zone,
                Zone::Hand { owner: PlayerId(0) },
                "real seeded opening"
            );
            if !g
                .state
                .rules
                .objects
                .values()
                .any(|o| o.zone == Zone::Battlefield && o.controller == PlayerId(0))
            {
                let land = g
                    .state
                    .rules
                    .objects
                    .values()
                    .find(|o| {
                        o.zone == (Zone::Hand { owner: PlayerId(0) })
                            && g.definition(o.id.card).unwrap().name == "Island"
                    })
                    .unwrap()
                    .id
                    .card;
                g.respond(
                    r.player(),
                    r.decision_id(),
                    Response::PlayLand { card: land },
                )
                .unwrap();
                continue;
            }
            recover(g);
            return CardId(1);
        }
        answer(g, q, pass(&r));
    }
    panic!("develop bound")
}
fn paid(g: &mut Game, q: u64) -> Request {
    let card = develop(g, q);
    fund(g, q, "Island", 1);
    request(g, q);
    announce(g, q, card.0, vec![]);
    assert_eq!(
        g.state
            .rules
            .stack
            .last()
            .unwrap()
            .paid
            .as_ref()
            .unwrap()
            .colored,
        vec![Color::U]
    );
    for _ in 0..1000 {
        let r = request(g, q);
        if matches!(r, Request::ResolveDiscard { .. }) {
            recover(g);
            return r;
        }
        assert!(!g.state.rules.stack.is_empty());
        answer(g, q, pass(&r));
    }
    panic!("order bound")
}
fn finish(g: &mut Game, q: u64) {
    for _ in 0..1000 {
        let r = request(g, q);
        if g.state.rules.stack.is_empty() && matches!(r, Request::Priority { .. }) {
            recover(g);
            return;
        }
        answer(g, q, pass(&r));
    }
    panic!("finish bound")
}
fn choose(g: &mut Game, r: &Request, objects: Vec<ObjectId>) {
    g.respond(
        r.player(),
        r.decision_id(),
        Response::ResolveDiscard { objects },
    )
    .unwrap();
    recover(g);
}

#[test]
fn draw_discard_paid_whole_hand_and_one_common_prior_batch() {
    for old in [false, true] {
        both(prepared(), |g, q| {
            develop(g, q);
            let old_hand = g
                .action_hand(PlayerId(0))
                .into_iter()
                .filter(|o| o.card != CardId(1))
                .collect::<Vec<_>>();
            let r = paid(g, q);
            let Request::ResolveDiscard {
                candidates, count, ..
            } = &r
            else {
                panic!("discard")
            };
            assert_eq!(*count, 2);
            assert_eq!(candidates.len(), old_hand.len() + 2);
            let selected = if old {
                old_hand[..2].to_vec()
            } else {
                candidates
                    .iter()
                    .filter(|o| !old_hand.contains(o))
                    .copied()
                    .collect()
            };
            assert_eq!(selected.len(), 2);
            let library_before = library(g, 0);
            choose(g, &r, selected.clone());
            finish(g, q);
            assert_eq!(library(g, 0), library_before);
            assert_eq!(g.action_hand(PlayerId(0)).len(), old_hand.len());
            for id in &selected {
                let now = g.object_id(id.card).unwrap();
                assert_eq!(now.incarnation, id.incarnation + 1);
                assert_eq!(
                    g.current(now).unwrap().zone,
                    Zone::Graveyard { owner: PlayerId(0) }
                );
            }
            if q == 1 {
                let batches=g.trace.records.iter().filter_map(|r|if let EvidenceRecord::Batch{batch}=&r.record{Some(batch)}else{None}).filter(|b|matches!(&b.group,EventGroup::Simultaneous{occurrences} if occurrences.len()==2&&occurrences.iter().all(|e|matches!(e,ProposedEvent::DiscardCard{..})))).collect::<Vec<_>>();
                assert_eq!(batches.len(), 1);
                let b = batches[0];
                for occurrence in &b.occurrences {
                    for object in &occurrence.before.objects {
                        assert!(
                            b.before.objects.contains(object),
                            "leaf consumed the shared prior object facts"
                        );
                    }
                    for player in &occurrence.before.players {
                        assert!(
                            b.before.players.contains(player),
                            "leaf consumed the shared prior player facts"
                        );
                    }
                }
                for id in selected {
                    assert!(
                        b.before.objects.iter().any(
                            |o| o.object == id && o.zone == (Zone::Hand { owner: PlayerId(0) })
                        )
                    );
                }
            }
        });
    }
}
#[test]
fn draw_discard_public_api_work_receipts_privacy_and_atomic_negatives() {
    both(prepared(), |g, q| {
        let r = paid(g, q);
        let Request::ResolveDiscard { candidates, .. } = &r else {
            panic!("discard")
        };
        let state = g.state.clone();
        let trace = g.trace.clone();
        for seat in 1..4 {
            let QueryValue::Observation(o) =
                finish_query(g, g.observe(PlayerId(seat), Budget::new(0)))
            else {
                panic!("view")
            };
            assert!(o.decision.is_none());
        }
        for objects in [
            vec![],
            vec![candidates[0]],
            vec![candidates[0]; 2],
            vec![candidates[0], object(g, 100)],
        ] {
            assert!(
                g.respond(
                    r.player(),
                    r.decision_id(),
                    Response::ResolveDiscard { objects }
                )
                .is_err()
            );
            assert_eq!(g.state, state);
            assert_eq!(g.trace, trace);
        }
        let QueryValue::Decision(Some(PlayerRequest::ResolveDiscard {
            token,
            candidates: handles,
            count,
            ..
        })) = finish_query(g, g.player_decision_for(PlayerId(0), Budget::new(0)))
        else {
            panic!("public discard")
        };
        assert_eq!(count, 2);
        let QueryResult::Incomplete { work, .. } =
            g.player_decision_for(PlayerId(0), Budget::new(1))
        else {
            panic!("work")
        };
        let saved = g.save_work(&work).unwrap();
        let work = g.restore_work(&saved).unwrap();
        assert!(matches!(
            finish_query(g, g.resume_work(work, Budget::unlimited())),
            QueryValue::Decision(Some(PlayerRequest::ResolveDiscard { count: 2, .. }))
        ));
        let response = PlayerResponse::ResolveDiscard {
            objects: handles[..2].to_vec(),
        };
        let QueryValue::Validated(receipt) = finish_query(
            g,
            g.check_player_response(PlayerId(0), token.clone(), response.clone(), Budget::new(0)),
        ) else {
            panic!("receipt")
        };
        let QueryValue::Validated(duplicate) = finish_query(
            g,
            g.check_player_response(PlayerId(0), token, response, Budget::new(0)),
        ) else {
            panic!("duplicate")
        };
        g.submit(receipt).unwrap();
        recover(g);
        assert!(g.submit(duplicate).is_err());
        assert!(g.restore_work(&saved).is_err());
        let mut bad: Snapshot = strict_read(&g.snapshot().unwrap()).unwrap();
        let Response::ResolveDiscard { objects } =
            &mut bad.state.rules.pending.as_mut().unwrap().response
        else {
            panic!("accepted discard")
        };
        objects[1] = candidates[2];
        bad.state_sha256 = digest(&bad.state).unwrap();
        assert!(Game::restore(g.prepared.as_ref().clone(), &canonical(&bad).unwrap()).is_err());
        finish(g, q);
    });
}
#[test]
fn draw_discard_departure_consumes_only_owned_pending_choice() {
    for accepted in [false, true] {
        both(prepared(), |g, q| {
            let r = paid(g, q);
            let Request::ResolveDiscard { candidates, .. } = &r else {
                panic!("discard")
            };
            if accepted {
                choose(g, &r, candidates[..2].to_vec());
            }
            g.concede(PlayerId(0)).unwrap();
            recover(g);
            finish(g, q);
            assert!(!g.is_live(PlayerId(0)));
            assert!(g.state.rules.pending.is_none());
        });
    }
}
#[test]
fn draw_discard_typed_zero_and_large_count_use_available_hand() {
    for draw in [0, 2] {
        for discard in ["0", "99999999999999999999999"] {
            let mut p = prepared();
            let c = Arc::make_mut(&mut p.catalog);
            let d = c
                .definitions
                .iter_mut()
                .find(|d| d.name == "Careful Study")
                .unwrap();
            d.spell.as_mut().unwrap().instructions = vec![
                Instruction::DrawControllerCards {
                    count: draw.into(),
                    source_block: 0,
                },
                Instruction::DiscardCards {
                    recipient: None,
                    count: discard.parse().unwrap(),
                    source_block: 0,
                },
            ];
            d.capabilities = d.requirements();
            d.validate().unwrap();
            p.catalog_digest = digest(c).unwrap();
            both(p, |g, q| {
                let card = develop(g, q);
                fund(g, q, "Island", 1);
                request(g, q);
                announce(g, q, card.0, vec![]);
                for _ in 0..1000 {
                    let r = request(g, q);
                    if let Request::ResolveDiscard {
                        candidates, count, ..
                    } = &r
                    {
                        assert_eq!(*count, candidates.len());
                        choose(g, &r, candidates.clone());
                        break;
                    }
                    if g.state.rules.stack.is_empty() {
                        break;
                    }
                    answer(g, q, pass(&r));
                }
                finish(g, q);
                if discard != "0" {
                    assert!(g.action_hand(PlayerId(0)).is_empty());
                }
            });
        }
    }
}
#[test]
fn draw_discard_commander_waits_for_sba_and_failed_draw_does_not_interrupt_discard() {
    for to_command in [false, true] {
        let mut p = prepared_with_sources(
            &[
                ("Careful Study", 0, 0, true),
                ("Island", 0, 0, false),
                ("Caelorna, Coral Tyrant", 0, 0, true),
            ],
            &[STUDY],
        );
        p.config.commanders = vec![CardId(2)];
        p.profile_digest = digest(&p.config).unwrap();
        both(p, |g, q| {
            fund(g, q, "Island", 1);
            request(g, q);
            announce(g, q, 0, vec![]);
            let r = loop {
                let r = request(g, q);
                if matches!(r, Request::ResolveDiscard { .. }) {
                    break r;
                }
                answer(g, q, pass(&r));
            };
            let Request::ResolveDiscard { candidates, .. } = &r else {
                panic!("discard")
            };
            let commander = object(g, 2);
            let selected = vec![
                commander,
                *candidates
                    .iter()
                    .find(|o| o.card != commander.card)
                    .unwrap(),
            ];
            choose(g, &r, selected);
            let r = request(g, q);
            assert!(matches!(r, Request::CommanderReturn { .. }));
            assert_eq!(
                g.current(g.object_id(commander.card).unwrap())
                    .unwrap()
                    .zone,
                Zone::Graveyard { owner: PlayerId(0) }
            );
            recover(g);
            g.respond(
                r.player(),
                r.decision_id(),
                Response::CommanderReturn { to_command },
            )
            .unwrap();
            finish(g, q);
            assert_eq!(
                g.current(g.object_id(commander.card).unwrap())
                    .unwrap()
                    .zone,
                if to_command {
                    Zone::Command { owner: PlayerId(0) }
                } else {
                    Zone::Graveyard { owner: PlayerId(0) }
                }
            );
        });
    }
    let mut p = prepared_with_sources(
        &[("Careful Study", 0, 0, true), ("Island", 0, 0, false)],
        &[STUDY],
    );
    let keep = p.config.initial_libraries()[&PlayerId(0)][0];
    p.config
        .cards
        .retain(|c| c.zone != (Zone::Library { owner: PlayerId(0) }) || c.id == keep);
    p.config.initial.libraries = None;
    p.profile_digest = digest(&p.config).unwrap();
    both(p, |g, q| {
        fund(g, q, "Island", 1);
        request(g, q);
        announce(g, q, 0, vec![]);
        let r = loop {
            let r = request(g, q);
            if matches!(r, Request::ResolveDiscard { .. }) {
                break r;
            }
            answer(g, q, pass(&r));
        };
        let Request::ResolveDiscard {
            candidates, count, ..
        } = &r
        else {
            panic!("discard")
        };
        assert_eq!(*count, 1);
        let Frame::ResolutionChoice { frame, .. } = &g.state.rules.frame else {
            panic!("frame")
        };
        assert!(
            matches!(&frame.outcomes[0],InstructionOutcome::DrawAttempts{attempted,failed,..}if *attempted==2.into()&&*failed==1.into())
        );
        assert!(g.is_live(PlayerId(0)));
        choose(g, &r, candidates.clone());
        finish(g, q);
        assert!(!g.is_live(PlayerId(0)));
    });
}
