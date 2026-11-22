//! Frozen shock sources in private legal100 setups declared BEFORE construction.
//! No admission or executed qualification. Raw counter/authority tampering below
//! is explicitly synthetic and never supplies the paid/public runtime witnesses.
use super::*;
const POOL: &str = include_str!(
    "../../../../../../tests/fixtures/ramos/20283c4a-f1f0-42f0-bc08-6da87474426b.json"
);
const GROUND: &str = include_str!(
    "../../../../../../tests/fixtures/ramos/16052b52-ade1-406f-a06b-ce7ea607fb63.json"
);
const FOUNTAIN: &str = include_str!(
    "../../../../../../tests/fixtures/ramos/f1750962-a87c-49f6-b731-02ae971ac6ea.json"
);
fn reseal(p: &mut Prepared) {
    let mut c = (*p.catalog).clone();
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    p.catalog_digest = digest(&c).unwrap();
    p.catalog = Arc::new(c);
    if let Some(m) = &mut p.match_config {
        for seat in 0..4 {
            let mut counts = BTreeMap::<String, u32>::new();
            let mut names = BTreeSet::new();
            for card in p.config.cards.iter().filter(|c| c.owner == PlayerId(seat)) {
                let d = p.catalog.definition_by_oracle_id(&card.oracle_id).unwrap();
                assert!(d.characteristics.basic || names.insert(d.name.clone()));
                *counts.entry(card.oracle_id.clone()).or_default() += 1;
            }
            assert_eq!(counts.values().sum::<u32>(), 100);
            m.decks[seat as usize].cards = counts
                .into_iter()
                .map(|(oracle_id, count)| DeckEntry { oracle_id, count })
                .collect();
        }
    }
    p.profile_digest = if p.match_config.is_some() {
        digest(&(&p.config, &p.match_config)).unwrap()
    } else {
        digest(&p.config).unwrap()
    };
}
fn add_sources(p: &mut Prepared, sources: &[&str]) {
    let c = Arc::make_mut(&mut p.catalog);
    for raw in sources {
        let d = lower(raw);
        c.definitions.retain(|old| old.oracle_id != d.oracle_id);
        c.definitions.push(d);
    }
}
fn land_setup(life: &str, controller: u8) -> Prepared {
    let mut p = midgame(&[("Cinder Glade", 0, controller, true)]);
    add_sources(&mut p, &[POOL]);
    let old = lower(CINDER).oracle_id;
    for card in &mut p.config.cards {
        if card.oracle_id == old {
            card.oracle_id = lower(POOL).oracle_id;
        }
    }
    p.config.initial.life[0] = life.parse().unwrap();
    reseal(&mut p);
    p
}
fn paid_setup(raw: &str, life: &str, shocks: &[&str]) -> Prepared {
    let mut p = natural_prepared(raw);
    let mut probe = Game::new(p.clone());
    opening(&mut probe);
    // Replace deepest ordinary Forests, preserving CardId-based randomness and
    // opening/development/payment sources. This inspects a setup probe only.
    let cards: Vec<_> = probe.state.rules.libraries[&PlayerId(0)]
        .iter()
        .rev()
        .filter(|id| probe.definition(**id).unwrap().name == "Forest")
        .take(shocks.len())
        .copied()
        .collect();
    assert_eq!(cards.len(), shocks.len());
    add_sources(&mut p, shocks);
    for (card, raw) in cards.iter().zip(shocks) {
        p.config
            .cards
            .iter_mut()
            .find(|c| c.id == *card)
            .unwrap()
            .oracle_id = lower(raw).oracle_id;
    }
    p.config.initial.life[0] = life.parse().unwrap();
    reseal(&mut p);
    p
}
fn play(g: &mut Game) -> ObjectId {
    let _ = request(g);
    let object = named(g, "Breeding Pool");
    let handle = g.handle_for(PlayerId(0), object).unwrap();
    public(g, PlayerResponse::PlayLand { object: handle });
    let r = request(g);
    assert!(matches!(
        r,
        Request::OptionalEffect {
            player: PlayerId(0),
            ..
        }
    ));
    assert_eq!(g.zone(object), Some(Zone::Hand { owner: PlayerId(0) }));
    assert_eq!(g.turn().lands_played, 0);
    object
}
fn boolean(g: &mut Game, paid: bool) {
    assert!(matches!(request(g), Request::OptionalEffect { .. }));
    public(g, PlayerResponse::OptionalEffect { perform: paid });
}
fn select(g: &mut Game, sources: &[&str], reverse: bool) -> Vec<ObjectId> {
    let mut objects: Vec<_> = sources
        .iter()
        .map(|raw| named(g, &lower(raw).name))
        .collect();
    if reverse {
        objects.reverse();
    }
    let r = observation(g, 0).decision.unwrap();
    let PlayerRequest::SearchLands { candidates, .. } = r else {
        panic!("search request");
    };
    let handles = objects
        .iter()
        .map(|o| {
            candidates
                .iter()
                .find(|c| g.resolve_search_handle(PlayerId(0), &c.handle).unwrap() == *o)
                .unwrap()
                .handle
                .clone()
        })
        .collect();
    public(g, PlayerResponse::SearchLands { objects: handles });
    let _ = request(g);
    objects.sort();
    assert!(
        matches!(&g.state.rules.frame, Frame::LandEntry { objects: selected, choices, .. } if selected == &objects && choices.is_empty())
    );
    objects
}
fn paid_choices(g: &mut Game, mask: u8, count: usize) {
    for i in 0..count {
        boolean(g, mask & (1 << i) != 0);
        assert!(matches!(
            g.advance(Budget::new(1)),
            Advance::Suspended { .. } | Advance::AwaitingDecision(_)
        ));
    }
    assert!(
        matches!(&g.state.rules.frame, Frame::LandEntry { choices, .. } if choices.len() == count)
    );
}
fn commit_entry(g: &mut Game) {
    assert!(matches!(
        g.advance(Budget::new(1)),
        Advance::Suspended { .. }
    ));
    assert!(!matches!(g.state.rules.frame, Frame::LandEntry { .. }));
}
#[test]
fn optional_entry_public_landplay_life_decline_pay_equality_and_intended_controller() {
    for (life, pay) in [
        ("1", false),
        ("2", false),
        ("3", false),
        ("2", true),
        ("3", true),
    ] {
        let mut g = Game::new(land_setup(life, 1));
        let old = play(&mut g);
        assert_eq!(g.state.rules.players[0].life, life.parse().unwrap());
        // Intended controller is player0, despite this synthetic initial hand
        // card's controller1. Ownership/control never substitute for the payer.
        assert_eq!(g.current(old).unwrap().controller, PlayerId(1));
        boolean(&mut g, pay);
        let _ = g.advance(Budget::new(1));
        assert_eq!(g.zone(old), Some(Zone::Hand { owner: PlayerId(0) }));
        assert_eq!(g.turn().lands_played, 0);
        commit_entry(&mut g);
        let new = g.object_id(old.card).unwrap();
        assert_ne!(new, old);
        assert_eq!(g.current(new).unwrap().controller, PlayerId(0));
        assert_eq!(g.current(new).unwrap().tapped, !pay);
        assert_eq!(g.turn().lands_played, 1);
        let mut expected: Quantity = life.parse().unwrap();
        if pay {
            expected.subtract(&2.into());
        }
        assert_eq!(g.state.rules.players[0].life, expected);
        let _ = request(&mut g);
        assert_eq!(g.is_live(PlayerId(0)), !(pay && life == "2"));
    }
    let mut g = Game::new(land_setup("1", 0));
    play(&mut g);
    let before = g.state.clone();
    let trace = g.trace.clone();
    let r = request(&mut g);
    assert!(
        g.check_response(
            PlayerId(0),
            r.decision_id(),
            Response::OptionalEffect { perform: true }
        )
        .is_err()
    );
    assert_eq!(g.state, before);
    assert_eq!(g.trace, trace);
}
#[test]
fn optional_entry_real_paid_claim_subsets_orders_combined_reservation_and_common_prior() {
    for (life, mask, reverse) in [
        (1, 0, false),
        (2, 0, true),
        (2, 1, false),
        (2, 2, true),
        (3, 0, false),
        (3, 1, true),
        (3, 2, false),
        (4, 0, false),
        (4, 1, true),
        (4, 2, false),
        (4, 3, false),
        (5, 0, true),
        (5, 1, true),
        (5, 2, false),
        (5, 3, true),
    ] {
        let mut g = Game::new(paid_setup(CLAIM, &life.to_string(), &[POOL, GROUND]));
        let _ = natural_search(&mut g, "Skyshroud Claim", 1);
        let objects = select(&mut g, &[POOL, GROUND], reverse);
        let before_life = g.state.rules.players[0].life.clone();
        paid_choices(&mut g, mask, 2);
        assert_eq!(g.state.rules.players[0].life, before_life);
        assert!(
            objects
                .iter()
                .all(|o| g.zone(*o) == Some(Zone::Library { owner: PlayerId(0) }))
        );
        commit_entry(&mut g);
        for (i, old) in objects.iter().enumerate() {
            let new = g.object_id(old.card).unwrap();
            assert_ne!(new, *old);
            assert_eq!(g.current(new).unwrap().tapped, mask & (1 << i) == 0);
        }
        assert_eq!(
            g.state.rules.players[0].life,
            Quantity::from(i64::from(life) - 2 * i64::from(mask.count_ones()))
        );
        // One group, one prior view and per-payment occurrences. The normal
        // shuffle/SBA remains AFTER this committed entry boundary.
        let batch = g
            .trace
            .records
            .iter()
            .rev()
            .find_map(|r| match &r.record {
                EvidenceRecord::Batch { batch } => Some(batch),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            batch
                .occurrences
                .iter()
                .filter(|o| matches!(o.proposed, ProposedEvent::SearchLandEntry { .. }))
                .count(),
            2
        );
        assert_eq!(
            batch
                .occurrences
                .iter()
                .filter(|o| matches!(o.proposed, ProposedEvent::LoseLife { .. }))
                .count(),
            mask.count_ones() as usize
        );
        let EventGroup::Ordered { children } = &batch.group else {
            panic!("payments and entry must have an ordered outer transaction");
        };
        let payments = mask.count_ones() as usize;
        assert_eq!(children.len(), payments + 1);
        for child in &children[..payments] {
            assert!(matches!(child, EventGroup::Occurrence {
                proposed: ProposedEvent::LoseLife { player: PlayerId(0), amount }
            } if *amount == Quantity::from(2)));
        }
        let EventGroup::Simultaneous { occurrences } = &children[payments] else {
            panic!("all selected entries must share one simultaneous group");
        };
        assert_eq!(occurrences.len(), 2);
        for (index, old) in objects.iter().enumerate() {
            assert!(
                matches!(occurrences[index], ProposedEvent::SearchLandEntry { object, player: PlayerId(0), .. } if object == *old)
            );
        }
        let entries: Vec<_> = batch
            .occurrences
            .iter()
            .filter(|o| matches!(o.proposed, ProposedEvent::SearchLandEntry { .. }))
            .collect();
        assert_eq!(
            entries[0].before.entry_basic_lands,
            entries[1].before.entry_basic_lands
        );
    }
    for life in [2, 3] {
        let mut g = Game::new(paid_setup(CLAIM, &life.to_string(), &[POOL, GROUND]));
        let _ = natural_search(&mut g, "Skyshroud Claim", 1);
        select(&mut g, &[POOL, GROUND], false);
        boolean(&mut g, true);
        let _ = request(&mut g);
        let r = request(&mut g);
        let before = g.state.clone();
        assert!(
            g.check_response(
                PlayerId(0),
                r.decision_id(),
                Response::OptionalEffect { perform: true }
            )
            .is_err()
        );
        assert_eq!(g.state, before);
        boolean(&mut g, false);
        let _ = g.advance(Budget::new(1));
        commit_entry(&mut g);
    }
}
#[test]
fn optional_entry_paid_lore_and_farseek_keep_forced_tapping_and_shuffle() {
    for (raw, name, shock, forced) in [
        (LORE, "Nature's Lore", POOL, false),
        (FARSEEK, "Farseek", FOUNTAIN, true),
    ] {
        for pay in [false, true] {
            let mut g = Game::new(paid_setup(raw, "5", &[shock]));
            let _ = natural_search(&mut g, name, 0);
            let objects = select(&mut g, &[shock], false);
            paid_choices(&mut g, u8::from(pay), 1);
            commit_entry(&mut g);
            assert_eq!(
                g.current(g.object_id(objects[0].card).unwrap())
                    .unwrap()
                    .tapped,
                forced || !pay
            );
            assert_eq!(
                g.state.rules.players[0].life,
                Quantity::from(if pay { 3 } else { 5 })
            );
            let _ = request(&mut g);
            check(&g);
            assert_eq!(
                g.state.rules.match_state.as_ref().unwrap().library_shuffles[0],
                1
            );
        }
    }
}
#[test]
fn optional_entry_claim_mixed_cinder_keeps_prior_basic_land_counts() {
    for basics in [1, 2] {
        let mut g = Game::new(paid_setup(CLAIM, "5", &[POOL]));
        let _ = natural_search(&mut g, "Skyshroud Claim", basics);
        let objects = select(&mut g, &[CINDER, POOL], basics == 2);
        paid_choices(&mut g, 1, 1);
        commit_entry(&mut g);
        let cinder = named(&g, "Cinder Glade");
        assert_eq!(g.current(cinder).unwrap().tapped, basics < 2);
        assert_eq!(objects.len(), 2);
    }
}
#[test]
fn optional_entry_public_pending_work_restore_replay_privacy_and_tokens() {
    for mode in [TraceMode::Off, TraceMode::Detailed] {
        let p = land_setup("3", 0);
        let mut g = Game::with_trace(p.clone(), mode);
        let old = play(&mut g);
        check(&g);
        let state = g.state.clone();
        let trace = g.trace.clone();
        let own = observation(&g, 0);
        let context = own.decision_context.as_ref().unwrap();
        assert_eq!(
            context.source.as_ref().unwrap().kind,
            DecisionSourceKind::Effect
        );
        assert_eq!(
            context.source.as_ref().unwrap().oracle_id,
            lower(POOL).oracle_id
        );
        assert!(
            context
                .instruction
                .as_ref()
                .unwrap()
                .text
                .contains("pay 2 life")
        );
        observation_work(&g, PlayerId(0));
        for seat in 1..4 {
            let o = observation(&g, seat);
            assert!(o.decision.is_none());
            assert!(o.decision_context.is_none());
            assert!(
                o.view
                    .objects
                    .iter()
                    .all(|o| o.oracle_id != lower(POOL).oracle_id)
            );
        }
        assert_eq!(g.state, state);
        assert_eq!(g.trace, trace);
        let token = own.decision.unwrap().token().clone();
        let mut foreign = g.fork();
        assert!(
            foreign
                .submit(validate(
                    &g,
                    PlayerResponse::OptionalEffect { perform: true }
                ))
                .is_err()
        );
        let receipt = validate(&g, PlayerResponse::OptionalEffect { perform: true });
        g.submit(receipt).unwrap();
        check(&g); // accepted but unconsumed bool
        assert!(matches!(
            g.check_player_response(
                PlayerId(0),
                token,
                PlayerResponse::OptionalEffect { perform: false },
                Budget::unlimited()
            ),
            QueryResult::Rejected(_)
        ));
        let snap = g.snapshot().unwrap();
        let mut tiny = Game::restore(p.clone(), &snap).unwrap();
        let _ = request(&mut g);
        for _ in 0..30 {
            if !matches!(tiny.advance(Budget::new(1)), Advance::Suspended { .. }) {
                break;
            }
        }
        assert_eq!(tiny.state, g.state);
        assert_eq!(tiny.trace, g.trace);
        assert_ne!(g.object_id(old.card).unwrap(), old);
        check(&g);
        let replay =
            Game::replay_to(p, &g.replay_journal(), Some(g.rule_boundary()), mode).unwrap();
        assert_eq!(replay.state, g.state);
    }
}
#[test]
fn optional_entry_zero_and_huge_exact_costs_are_not_printed_two_dispatch() {
    // Explicit synthetic IR quantities and initial life, never source admission.
    for (cost, life) in [
        ("0", "-1"),
        ("0", "1"),
        ("1", "1"),
        ("3", "3"),
        (
            "999999999999999999999999999999999999999",
            "999999999999999999999999999999999999999",
        ),
    ] {
        let mut p = land_setup(life, 0);
        let d = Arc::make_mut(&mut p.catalog)
            .definitions
            .iter_mut()
            .find(|d| d.name == "Breeding Pool")
            .unwrap();
        d.source_labels = None;
        d.static_abilities[0].effect = StaticEffect::EntersTappedUnlessPayLife {
            amount: cost.parse().unwrap(),
        };
        reseal(&mut p);
        let mut g = Game::new(p);
        play(&mut g);
        boolean(&mut g, true);
        let _ = g.advance(Budget::new(1));
        commit_entry(&mut g);
        let mut expected: Quantity = life.parse().unwrap();
        expected.subtract(&cost.parse().unwrap());
        assert_eq!(g.state.rules.players[0].life, expected);
        let payments = g
            .events()
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Event::Committed {
                        proposed: ProposedEvent::LoseLife { .. },
                        ..
                    }
                )
            })
            .count();
        assert_eq!(payments, usize::from(cost != "0"));
    }
}
#[test]
fn optional_entry_concessions_preserve_or_invalidate_pending_both_boolean_values() {
    for perform in [false, true] {
        for pending in [false, true] {
            for departing in [0, 2] {
                let mut g = Game::new(land_setup("3", 0));
                play(&mut g);
                if pending {
                    boolean(&mut g, perform);
                }
                let input = g.state.rules.pending.clone();
                let journal = g.journal().len();
                g.concede(PlayerId(departing)).unwrap();
                assert!(matches!(
                    g.advance(Budget::new(1)),
                    Advance::Suspended { .. } | Advance::AwaitingDecision(_)
                ));
                assert_eq!(g.journal().len(), journal);
                if departing == 0 {
                    assert!(!matches!(g.state.rules.frame, Frame::LandEntry { .. }));
                    assert!(g.state.rules.pending.is_none());
                    assert_eq!(g.state.rules.players[0].life, Quantity::from(3));
                    assert_eq!(g.turn().lands_played, 0);
                } else {
                    assert!(matches!(g.state.rules.frame, Frame::LandEntry { .. }));
                    assert_eq!(g.state.rules.pending, input);
                    check(&g);
                }
            }
        }
    }
    // A personal paid search belonging to a departed chooser must not shuffle.
    let mut g = Game::new(paid_setup(CLAIM, "5", &[POOL, GROUND]));
    let _ = natural_search(&mut g, "Skyshroud Claim", 1);
    select(&mut g, &[POOL, GROUND], false);
    boolean(&mut g, false);
    let _ = request(&mut g);
    boolean(&mut g, true);
    let random = g
        .state
        .rules
        .match_state
        .as_ref()
        .unwrap()
        .random
        .word_position;
    let shuffles = g
        .state
        .rules
        .match_state
        .as_ref()
        .unwrap()
        .library_shuffles
        .clone();
    g.concede(PlayerId(0)).unwrap();
    let _ = request(&mut g);
    assert_eq!(
        g.state
            .rules
            .match_state
            .as_ref()
            .unwrap()
            .random
            .word_position,
        random
    );
    assert_eq!(
        g.state.rules.match_state.as_ref().unwrap().library_shuffles,
        shuffles
    );
    assert!(g.state.rules.pending.is_none());
    check(&g);
}
#[test]
fn optional_entry_paid_spell_source_owner_departure_keeps_the_frozen_root() {
    // Explicit synthetic source-owner mismatch probe after a real paid search:
    // current complete ordinary search producers cannot cast another owner's
    // hand card. No replay/admission is claimed for this ownership tampering.
    for perform in [false, true] {
        let mut g = Game::new(paid_setup(CLAIM, "5", &[POOL, GROUND]));
        let _ = natural_search(&mut g, "Skyshroud Claim", 1);
        select(&mut g, &[POOL, GROUND], false);
        boolean(&mut g, perform);
        let pending = g.state.rules.pending.clone();
        let source = match &g.state.rules.frame {
            Frame::LandEntry {
                origin: EntryOrigin::Search { frame },
                ..
            } => g.entry(frame.stack_id).unwrap().source,
            _ => panic!(),
        };
        Arc::make_mut(&mut g.state.rules.cards)
            .get_mut(&source.card)
            .unwrap()
            .owner = PlayerId(1);
        g.concede(PlayerId(1)).unwrap();
        let _ = g.advance(Budget::new(1));
        assert!(matches!(g.state.rules.frame, Frame::LandEntry { .. }));
        assert_eq!(g.state.rules.pending, pending);
        assert!(g.current(source).is_err());
        let _ = request(&mut g);
        boolean(&mut g, false);
        let _ = g.advance(Budget::new(1));
        commit_entry(&mut g);
        assert_eq!(
            g.state.rules.players[0].life,
            Quantity::from(if perform { 3 } else { 5 })
        );
    }
}
#[test]
fn optional_entry_exact_cause_group_authority_rejects_forgery_and_rolls_back_resources() {
    let mut g = Game::new(land_setup("3", 0));
    let old = play(&mut g);
    boolean(&mut g, true);
    let _ = g.advance(Budget::new(1));
    let cause = g.entry_cause_for_test();
    let group = g.entry_group_for_test();
    let before = g.state.clone();
    let trace = g.trace.clone();
    // Every omission/duplicate/foreign payment or move with the originating
    // cause must reject BEFORE any leaf or evidence publication.
    let EventGroup::Ordered { children } = group.clone() else {
        panic!()
    };
    for case in 0..7 {
        let mut children = children.clone();
        match case {
            0 => {
                children.remove(0);
            }
            1 => {
                children.remove(1);
            }
            2 => {
                children.insert(0, children[0].clone());
            }
            3 => {
                children[0] = EventGroup::Occurrence {
                    proposed: ProposedEvent::LoseLife {
                        player: PlayerId(1),
                        amount: 2.into(),
                    },
                };
            }
            4 => {
                children[1] = EventGroup::Simultaneous {
                    occurrences: vec![ProposedEvent::Move {
                        object: old,
                        destination: Zone::Graveyard { owner: PlayerId(0) },
                    }],
                };
            }
            5 => {
                children.pop();
            }
            _ => {
                children[0] = EventGroup::Occurrence {
                    proposed: ProposedEvent::LoseLife {
                        player: PlayerId(0),
                        amount: 3.into(),
                    },
                };
            }
        }
        assert!(
            g.commit_batch(cause.clone(), EventGroup::Ordered { children })
                .is_err()
        );
        assert_eq!(g.state, before);
        assert_eq!(g.trace, trace);
    }
    let mut foreign = cause.clone();
    if let BatchCause::LandPlay { decision_id, .. } = &mut foreign {
        *decision_id += 1;
    }
    assert!(g.commit_batch(foreign, group.clone()).is_err());
    assert_eq!(g.state, before);
    // Explicit synthetic resource exhaustion on a complete, unpaid frame.
    for fault in 0..3 {
        let mut probe = g.fork();
        match fault {
            0 => probe.state.rules.next_batch_id = u64::MAX,
            1 => {
                probe.state.rules.next_event_sequence = u64::MAX;
            }
            _ => {
                let successor = ObjectId {
                    card: old.card,
                    incarnation: u32::MAX,
                };
                probe.state.rules.objects.get_mut(&old.card).unwrap().id = successor;
                if let Frame::LandEntry { objects, .. } = &mut probe.state.rules.frame {
                    objects[0] = successor;
                }
            }
        }
        let state = probe.state.clone();
        let trace = probe.trace.clone();
        assert!(probe.land_entry_step().is_err());
        assert_eq!(probe.state, state);
        assert_eq!(probe.trace, trace);
    }
}
#[test]
fn optional_entry_retention_charge_is_the_exact_frame_payload_and_owners_charge_once() {
    let mut g = Game::new(paid_setup(CLAIM, "5", &[POOL, GROUND]));
    let _ = natural_search(&mut g, "Skyshroud Claim", 1);
    select(&mut g, &[POOL, GROUND], false);
    let Frame::LandEntry {
        origin: EntryOrigin::Search { frame },
        objects,
        ..
    } = &g.state.rules.frame
    else {
        panic!()
    };
    let frame = frame.clone();
    let objects = objects.clone();
    // The frame names its invocation. The invocation's one owner is charged
    // for its cost lock and grant payload; the frame is charged for the rest.
    let owner = g.entry(frame.stack_id).unwrap();
    assert_eq!(g.cost_lock_charge(), costs::entry_cost_lock_charge(owner));
    assert_eq!(
        g.land_entry_charge(),
        stored_history::payload_charge(&g.state.rules.frame)
    );
    let mut previous = g.fork();
    previous.state.rules.frame = Frame::ResolutionChoice {
        frame: frame.clone(),
        choice: ResolutionChoice::Search {
            player: PlayerId(0),
            maximum: 2,
            split: false,
            source_block: g.entry_search_block(&frame).unwrap(),
        },
    };
    assert_eq!(g.cost_lock_charge(), previous.cost_lock_charge());
    assert_eq!(g.grant_payload_charge(), previous.grant_payload_charge());
    assert_eq!(
        g.history_charge() - previous.history_charge(),
        g.land_entry_charge()
    );
    let mut wrapped = g.fork();
    wrapped.state.rules.frame = Frame::Stabilize {
        resume: Box::new(g.state.rules.frame.clone()),
        cleanup: false,
        changed: false,
    };
    assert_eq!(wrapped.land_entry_charge(), g.land_entry_charge());
    assert_eq!(wrapped.cost_lock_charge(), g.cost_lock_charge());
    assert_eq!(wrapped.grant_payload_charge(), g.grant_payload_charge());
    // Explicit synthetic history-limit boundary, independent of trace retention.
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        let mut probe = g.fork();
        probe.trace.mode = mode;
        let retained = probe.state.rules.retained_auxiliary_charge;
        let extra = capacities::HISTORY_LIMIT - probe.history_charge();
        probe.state.rules.retained_auxiliary_charge = retained + extra;
        assert!(probe.check_history_capacity().is_ok());
        probe.state.rules.retained_auxiliary_charge += 1;
        assert!(probe.check_history_capacity().is_err());
    }
    assert_eq!(objects.len(), 2);
}
#[test]
fn optional_entry_synthetic_zero_prior_cinder_and_large_unaffordable_cost() {
    let mut g = Game::new(paid_setup(CLAIM, "5", &[POOL]));
    let _ = natural_search(&mut g, "Skyshroud Claim", 1);
    // Explicit synthetic generic movement probe, not a source-backed removal.
    // This occurs BEFORE choice preparation; the subsequent life-only prefix
    // must preserve this zero common-prior Basic+Land count for both entries.
    let basics: Vec<_> = g
        .state
        .rules
        .objects
        .values()
        .filter(|o| o.zone == Zone::Battlefield && g.characteristics(o.id).unwrap().basic)
        .map(|o| o.id)
        .collect();
    for basic in basics {
        g.move_object(
            basic,
            Zone::Graveyard {
                owner: g.state.rules.cards[&basic.card].owner,
            },
        )
        .unwrap();
    }
    // This raw synthetic movement bypasses the ordinary rule-step publication.
    g.refresh_visibility().unwrap();
    select(&mut g, &[CINDER, POOL], false);
    paid_choices(&mut g, 1, 1);
    commit_entry(&mut g);
    assert!(g.current(named(&g, "Cinder Glade")).unwrap().tapped);
    let mut p = land_setup("3", 0);
    let d = Arc::make_mut(&mut p.catalog)
        .definitions
        .iter_mut()
        .find(|d| d.name == "Breeding Pool")
        .unwrap();
    d.source_labels = None;
    d.static_abilities[0].effect = StaticEffect::EntersTappedUnlessPayLife {
        amount: "999999999999999999999999999999999".parse().unwrap(),
    };
    reseal(&mut p);
    let mut g = Game::new(p);
    play(&mut g);
    let before = g.state.clone();
    let token = observation(&g, 0).decision.unwrap().token().clone();
    assert!(matches!(
        g.check_player_response(
            PlayerId(0),
            token,
            PlayerResponse::OptionalEffect { perform: true },
            Budget::unlimited()
        ),
        QueryResult::Rejected(_)
    ));
    assert_eq!(g.state, before);
}
#[test]
fn optional_entry_private_restore_rejects_foreign_prefix_source_origin_and_duplicate_authority() {
    let p = land_setup("3", 0);
    let mut g = Game::new(p.clone());
    play(&mut g);
    let snapshot = g.snapshot().unwrap();
    for case in 0..6 {
        let mut s: Snapshot = strict_read(&snapshot).unwrap();
        let Frame::LandEntry {
            origin,
            objects,
            choices,
        } = &mut s.state.rules.frame
        else {
            panic!()
        };
        match case {
            0 => choices.extend([
                land_entry::EntryChoice::Life(true),
                land_entry::EntryChoice::Life(true),
            ]),
            1 => {
                objects.push(objects[0]);
            }
            2 => objects[0].incarnation += 1,
            3 => {
                if let EntryOrigin::LandPlay { player, .. } = origin {
                    *player = PlayerId(1);
                }
            }
            4 => {
                if let EntryOrigin::LandPlay { decision_id, .. } = origin {
                    *decision_id = u64::MAX;
                }
            }
            _ => choices.push(land_entry::EntryChoice::Life(true)), // structurally valid but absent accepted input
        }
        s.state_sha256 = digest(&s.state).unwrap();
        assert!(Game::restore(p.clone(), &canonical(&s).unwrap()).is_err());
    }
    let mut value: serde_json::Value = serde_json::from_str(&snapshot).unwrap();
    value["state"]["rules"]["frame"]["authority"] = serde_json::json!({"paid":true});
    assert!(Game::restore(p, &value.to_string()).is_err());
}
#[test]
fn optional_entry_selected_library_context_between_offers_and_tiny_replay() {
    let p = paid_setup(CLAIM, "5", &[POOL, GROUND]);
    let mut g = Game::with_trace(p.clone(), TraceMode::Compact);
    let _ = natural_search(&mut g, "Skyshroud Claim", 1);
    let objects = select(&mut g, &[POOL, GROUND], true);
    let before = g.state.clone();
    let trace = g.trace.clone();
    let own = observation(&g, 0);
    assert!(
        own.decision_context
            .as_ref()
            .unwrap()
            .source
            .as_ref()
            .unwrap()
            .object
            .is_some()
    );
    assert!(objects.iter().all(|o| {
        own.view.objects.iter().any(|v| {
            v.oracle_id == g.definition(o.card).unwrap().oracle_id
                && v.zone == Zone::Library { owner: PlayerId(0) }
        })
    }));
    for seat in 1..4 {
        let view = observation(&g, seat);
        assert!(view.decision.is_none() && view.decision_context.is_none());
        assert!(objects.iter().all(|o| {
            view.view
                .objects
                .iter()
                .all(|v| v.oracle_id != g.definition(o.card).unwrap().oracle_id)
        }));
    }
    let old_token = own.decision.as_ref().unwrap().token().clone();
    let serialized = canonical(&own).unwrap();
    assert!(
        !serialized.contains("\"incarnation\"")
            && !serialized.contains("\"card\":")
            && !serialized.contains("library_order")
    );
    observation_work(&g, PlayerId(0));
    assert_eq!(g.state, before);
    assert_eq!(g.trace, trace);
    let r = request(&mut g);
    assert!(
        g.check_response(
            PlayerId(1),
            r.decision_id(),
            Response::OptionalEffect { perform: false }
        )
        .is_err()
    );
    assert!(
        g.check_response(PlayerId(0), r.decision_id(), Response::Pass)
            .is_err()
    );
    boolean(&mut g, false);
    check(&g); // accepted false remains private until consumed
    let _ = request(&mut g);
    check(&g); // between offers, still no life debit/entry
    assert!(matches!(
        g.check_player_response(
            PlayerId(0),
            old_token,
            PlayerResponse::OptionalEffect { perform: true },
            Budget::unlimited()
        ),
        QueryResult::Rejected(_)
    ));
    assert_eq!(g.state.rules.players[0].life, Quantity::from(5));
    assert!(
        objects
            .iter()
            .all(|o| g.zone(*o) == Some(Zone::Library { owner: PlayerId(0) }))
    );
    boolean(&mut g, true);
    check(&g);
    let mut tiny = Game::restore(p.clone(), &g.snapshot().unwrap()).unwrap();
    let _ = request(&mut g);
    for _ in 0..2000 {
        if !matches!(tiny.advance(Budget::new(1)), Advance::Suspended { .. }) {
            break;
        }
    }
    assert_eq!(tiny.state, g.state);
    assert_eq!(tiny.trace, g.trace);
    check(&g);
    let replay = Game::replay_to(
        p,
        &g.replay_journal(),
        Some(g.rule_boundary()),
        TraceMode::Compact,
    )
    .unwrap();
    assert_eq!(replay.state, g.state);
}
#[test]
fn optional_entry_synthetic_nonempty_grant_history_and_commander_wrapper_charge_once() {
    // Accounting-only typed controls: no triggered-search or commander-wrapper
    // source producer is admitted. Keep the real paid-root case separate above.
    let mut g = Game::new(paid_setup(CLAIM, "5", &[POOL, GROUND]));
    let _ = natural_search(&mut g, "Skyshroud Claim", 1);
    select(&mut g, &[POOL, GROUND], false);
    let Frame::LandEntry {
        origin: EntryOrigin::Search { frame },
        objects,
        choices,
    } = &g.state.rules.frame
    else {
        panic!()
    };
    let root = frame.clone();
    let objects = objects.clone();
    let choices = choices.clone();
    let mut owner = g.entry(root.stack_id).unwrap().clone();
    let fact = g.object_before(owner.source).unwrap();
    let handles = (0..4)
        .map(|s| g.handle_for(PlayerId(s), owner.source).unwrap())
        .collect::<Vec<_>>();
    owner.cost_lock = None;
    owner.kind = StackKind::Triggered {
        match_condition: None,
        ability: EffectiveTriggerAbility::Granted {
            grant_source: owner.source,
            static_ability_index: 0,
        },
        grant: Some(CascadeGrantProvenance {
            source_facts: fact.clone(),
            source_handles: handles.clone(),
        }),
        source_handles: handles,
        trigger_id: TriggerId(0),
        event: TriggerOccurrenceId::Cast(CastOccurrenceId {
            cast: root.stack_id,
        }),
        witness: TriggerEventWitness::SpellCast {
            cast: Box::new(CastOccurrence {
                priority_paid_exile: false,
                history: triggers::CastHistoryFacts {
                    turn_number: g.turn().number,
                    first_overall: true,
                    first_from_exile: false,
                },
                x: None,
                stack_id: root.stack_id,
                caster: PlayerId(0),
                cast_from: Zone::Hand { owner: PlayerId(0) },
                spell: fact,
            }),
        },
    };
    let grant = spell_cascade_grants::entry_grant_charge(&owner);
    assert!(grant > 0);
    let entry = Frame::LandEntry {
        origin: EntryOrigin::Search { frame: root },
        objects: objects.clone(),
        choices,
    };
    let payload = stored_history::payload_charge(&entry);
    let mut probe = g.fork();
    probe.state.rules.frame = entry.clone();
    // The grant payload is charged once, at the invocation's single owner.
    let before = probe.grant_payload_charge();
    let id = owner.id;
    *probe
        .state
        .rules
        .stack
        .iter_mut()
        .find(|s| s.id == id)
        .unwrap() = owner;
    assert_eq!(probe.grant_payload_charge(), before + grant);
    assert_eq!(probe.land_entry_charge(), payload);
    let existing = probe.grant_payload_charge();
    probe.state.rules.frame = Frame::CommanderReturn {
        object: objects[0],
        destination: Zone::Graveyard { owner: PlayerId(0) },
        continuation: Box::new(CommanderContinuation::StateBased { resume: entry }),
    };
    assert_eq!(probe.land_entry_charge(), payload);
    assert_eq!(probe.grant_payload_charge(), existing);
    assert_eq!(probe.cascade_charge(), 0);
}
#[test]
fn optional_entry_paid_real_pool_uses_both_existing_intrinsic_mana_choices() {
    for color in [Color::G, Color::U] {
        let mut g = Game::new(land_setup("3", 0));
        let old = play(&mut g);
        boolean(&mut g, true);
        let _ = request(&mut g);
        let source = g.object_id(old.card).unwrap();
        assert_ne!(source, old);
        assert!(g.validate_mana_activation(PlayerId(0), old).is_err());
        activate_public(&mut g, source);
        choose(&mut g, color);
        assert_eq!(g.mana(PlayerId(0), color), Some(Quantity::from(1)));
        assert!(g.current(source).unwrap().tapped);
        check(&g);
    }
}
#[test]
fn optional_entry_complete_paid_frame_allows_unrelated_departure_but_not_false_instruction_cause() {
    let mut g = Game::new(paid_setup(CLAIM, "5", &[POOL, GROUND]));
    let _ = natural_search(&mut g, "Skyshroud Claim", 1);
    select(&mut g, &[POOL, GROUND], false);
    paid_choices(&mut g, 3, 2);
    let cause = g.entry_cause_for_test();
    let group = g.entry_group_for_test();
    let EventGroup::Ordered { children } = group.clone() else {
        panic!()
    };
    let truncated = EventGroup::Ordered {
        children: vec![children[0].clone()],
    };
    for wrong_cursor in [false, true] {
        let mut forged = cause.clone();
        if let BatchCause::Instruction {
            cursor,
            source_block,
            ..
        } = &mut forged
        {
            if wrong_cursor {
                *cursor += 1;
            } else {
                *source_block += 1;
            }
        }
        let before = g.state.clone();
        assert!(g.commit_batch(forged, truncated.clone()).is_err());
        assert_eq!(g.state, before);
    }
    let frame = g.state.rules.frame.clone();
    g.concede(PlayerId(2)).unwrap();
    let _ = g.advance(Budget::new(1));
    assert_eq!(g.state.rules.frame, frame);
    assert_eq!(g.state.rules.players[0].life, Quantity::from(5));
    commit_entry(&mut g);
    assert_eq!(g.state.rules.players[0].life, Quantity::from(1));
    assert_eq!(
        g.state.rules.match_state.as_ref().unwrap().library_shuffles[0],
        0
    );
    let _ = request(&mut g);
    assert_eq!(
        g.state.rules.match_state.as_ref().unwrap().library_shuffles[0],
        1
    );
    check(&g);
}
