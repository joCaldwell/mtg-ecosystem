//! Frozen R5 card movement, discard and private library procedures.
//! CR 101.4, 102.2, 400.2, 400.7, 601.2c, 608.2b-d, 701.9, 701.20, 701.23, 701.24b, 806.1, 903.2:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use mtg_contracts::kir::{Cell, EvaluatedVerb, definition_cells};
use mtg_core::testkit::*;

macro_rules! record {
    ($id:literal) => {
        include_str!(concat!(
            "../../../../tests/fixtures/phase1-r5/",
            $id,
            ".json"
        ))
    };
}

const ARCHAEOMANCER: &str = record!("a91a3266-cadd-47a0-9b20-160307f14c07");
const BURGLAR_RAT: &str = record!("2f807301-37df-4724-871a-08e3512b07b3");
const CAREFUL_STUDY: &str = record!("32e9aa23-fc0a-4b82-82f9-1659c304428c");
const DISENTOMB: &str = record!("e4ab986d-662c-4879-b6f1-6887a2ec4d7c");
const ETERNAL_WITNESS: &str = record!("30b24e8e-3b0e-4d8e-90f3-f66eb7c1858c");
const GRAVEWAKER: &str = record!("7ceec1c7-c1f9-4d47-a203-6ee98c5d30d2");
const GORGING_VULTURE: &str = record!("67330f45-d08c-4fe2-8ae3-8d18939c512a");
const SIBSIG_HOST: &str = record!("11776f73-f504-4f19-b5b6-2f58279f8b15");
const WHEEL_OF_FORTUNE: &str = record!("a8abd966-de7b-46a3-8ac7-8747ab35653a");
const HYMN: &str = record!("992d9a72-d34d-47d7-96fb-37f0efb515ef");
const SYPHON_MIND: &str = record!("abc37d6c-6300-47b5-a679-9db5b83eb54f");
const GERRARDS_VERDICT: &str = record!("670b5dfb-ffdf-43ed-acc3-855006e352fc");
const INDEX: &str = record!("613b824e-39d1-4c6e-a19c-c915abdc860b");
const RECKLESS_HANDLING: &str = record!("f1ea7dc5-01cb-4780-947c-08ea9324c52f");
const RESURRECTION: &str = record!("837417d8-8260-486d-a3ed-3b5711eaf34a");
const VAMPIRIC_TUTOR: &str = record!("ededbdae-d9dc-4206-9335-d7158f2d7700");
const WORLDLY_TUTOR: &str = record!("e8863518-0bfa-49c3-8c6e-6c9116a81051");

fn board(cards: &[(&str, Cell)]) -> Board {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    for &(raw, cell) in cards {
        let result = mtg_witness::compile(mtg_witness::R5_MANIFEST, raw);
        let mtg_compiler::CompileResult::Lowered { definition } = result else {
            panic!("frozen movement witness failed: {result:?}");
        };
        assert!(definition_cells(&definition).unwrap().contains(&cell));
        catalog.definitions.push(*definition);
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    Board::new(catalog, "r4-card-movement")
        .step(TurnStep::PrecombatMain)
        .seats(2)
        .seed([29; 32])
}

fn checked(g: &Game) {
    g.validate_state().unwrap();
    let snapshot = g.snapshot().unwrap();
    assert_eq!(
        Game::restore(g.prepared.as_ref().clone(), &snapshot)
            .unwrap()
            .snapshot()
            .unwrap(),
        snapshot,
    );
}

fn view(g: &Game, seat: u8) -> Observation {
    let QueryResult::Complete(QueryValue::Observation(observation)) =
        g.observe(PlayerId(seat), Budget::unlimited())
    else {
        panic!("expected observation");
    };
    observation
}

fn next_choice(g: &mut Game) -> Decision<Rules> {
    for _ in 0..24 {
        let d = Driver::request(g, 1);
        if d.reason != Reason::Priority {
            checked(g);
            return d;
        }
        assert!(
            !g.state.rules.stack.is_empty(),
            "resolution finished before a choice"
        );
        Driver::pass(g);
    }
    panic!("no resolution choice");
}

fn settle(g: &mut Game) {
    for _ in 0..24 {
        let d = Driver::request(g, 1);
        assert_eq!(d.reason, Reason::Priority, "{d:?}");
        if g.state.rules.stack.is_empty() {
            checked(g);
            assert_recoverable((&*g).into());
            return;
        }
        Driver::pass(g);
    }
    panic!("no empty-stack priority");
}

fn objects(d: &Decision<Rules>) -> Vec<ObjectId> {
    d.candidates
        .iter()
        .filter_map(|candidate| match candidate {
            Candidate::Object { object } => Some(*object),
            _ => None,
        })
        .collect()
}

fn order(g: &mut Game, d: &Decision<Rules>, order: Vec<u32>) {
    g.respond(d.player, d.id, Answer::Order { order }).unwrap();
}

fn assert_hidden(g: &Game, viewer: u8, zone: Zone) {
    let observed = view(g, viewer);
    assert!(observed.decision.is_none());
    assert!(!observed.view.objects.iter().any(|row| row.zone == zone));
}

#[test]
fn careful_study_draws_before_the_controller_discards() {
    let you = PlayerId(0);
    let hand = Zone::Hand { owner: you };
    let graveyard = Zone::Graveyard { owner: you };
    let mut g = Game::new(
        board(&[(CAREFUL_STUDY, Cell::Evaluated(EvaluatedVerb::Discard))])
            .card("Careful Study", 0, 0, hand)
            .card("Island", 0, 0, Zone::Battlefield)
            .library_card("Forest", 0)
            .library_card("Swamp", 0)
            .prepare(),
    );
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1]);
    let d = next_choice(&mut g);
    assert_eq!((d.player, &d.reason), (you, &Reason::ResolveDiscard));
    assert_eq!(objects(&d), [Driver::object(&g, 2), Driver::object(&g, 3)]);
    assert_hidden(&g, 1, hand);
    assert_recoverable((&g).into());
    g = Game::restore(g.prepared.as_ref().clone(), &g.snapshot().unwrap()).unwrap();
    g.respond(
        d.player,
        d.id,
        Answer::Pick {
            picks: vec![vec![0, 1]],
        },
    )
    .unwrap();
    settle(&mut g);
    assert_eq!(g.zone(Driver::object(&g, 2)), Some(graveyard));
    assert_eq!(g.zone(Driver::object(&g, 3)), Some(graveyard));
}

#[test]
fn burglar_rat_keeps_apnap_discard_choices_private_until_the_group_moves() {
    let hand = |seat| Zone::Hand {
        owner: PlayerId(seat),
    };
    let mut first = Game::new(
        board(&[(BURGLAR_RAT, Cell::Evaluated(EvaluatedVerb::Discard))])
            .seats(4)
            .card("Burglar Rat", 0, 0, hand(0))
            .card("Swamp", 0, 0, Zone::Battlefield)
            .card("Swamp", 0, 0, Zone::Battlefield)
            .card("Forest", 1, 1, hand(1))
            .card("Plains", 1, 1, hand(1))
            .card("Island", 2, 2, hand(2))
            .prepare(),
    );
    Driver::cast(&mut first, CardId(0));
    Driver::pay(&mut first, &[1, 2]);
    let choice = next_choice(&mut first);
    assert_eq!(
        (choice.player, &choice.reason),
        (PlayerId(1), &Reason::ResolveDiscard)
    );
    let mut second = first.fork();
    let forest = Driver::object(&first, 3);
    let plains = Driver::object(&second, 4);
    Driver::pick_object(&mut first, forest);
    Driver::pick_object(&mut second, plains);
    let left = next_choice(&mut first);
    let right = next_choice(&mut second);
    assert_eq!(
        (left.player, &left.reason),
        (PlayerId(2), &Reason::ResolveDiscard)
    );
    assert_eq!(
        (right.player, &right.reason),
        (PlayerId(2), &Reason::ResolveDiscard)
    );
    for seat in [0, 2, 3] {
        assert_eq!(view(&first, seat), view(&second, seat));
    }
    assert_eq!(first.zone(forest), Some(hand(1)));
    assert_eq!(second.zone(plains), Some(hand(1)));
    assert_hidden(&first, 3, hand(1));
    for game in [&mut first, &mut second] {
        assert_recoverable((&*game).into());
        let island = Driver::object(game, 5);
        Driver::pick_object(game, island);
        // Seat 3 has an empty hand. It receives no choice and the other picks
        // still commit simultaneously.
        settle(game);
        assert_eq!(
            game.zone(Driver::object(game, 5)),
            Some(Zone::Graveyard { owner: PlayerId(2) })
        );
    }
    assert_eq!(
        first.zone(Driver::object(&first, 3)),
        Some(Zone::Graveyard { owner: PlayerId(1) })
    );
    assert_eq!(
        second.zone(Driver::object(&second, 4)),
        Some(Zone::Graveyard { owner: PlayerId(1) })
    );
}

#[test]
fn hymn_discards_two_random_distinct_cards_and_replays() {
    let opponent = PlayerId(1);
    let hand = Zone::Hand { owner: opponent };
    let graveyard = Zone::Graveyard { owner: opponent };
    let mut g = Game::new(
        board(&[(HYMN, Cell::Evaluated(EvaluatedVerb::Discard))])
            .permanent_or_hand("Hymn to Tourach", 0, 0, true)
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Swamp", 0, 0, false)
            .card("Forest", 1, 1, hand)
            .card("Island", 1, 1, hand)
            .card("Plains", 1, 1, hand)
            .prepare(),
    );
    Driver::cast(&mut g, CardId(0));
    let targets = Driver::request(&mut g, 1);
    let answer = targets
        .pick_one(&Candidate::Player { player: opponent })
        .unwrap();
    g.respond(targets.player, targets.id, answer).unwrap();
    Driver::pay(&mut g, &[1, 2]);
    let mut duplicate = g.fork();
    settle(&mut duplicate);
    settle(&mut g);
    let zones: Vec<_> = (3..6).map(|id| g.zone(Driver::object(&g, id))).collect();
    assert_eq!(zones.iter().filter(|z| **z == Some(graveyard)).count(), 2);
    assert_eq!(zones.iter().filter(|z| **z == Some(hand)).count(), 1);
    assert_eq!(
        zones,
        (3..6)
            .map(|id| duplicate.zone(Driver::object(&duplicate, id)))
            .collect::<Vec<_>>()
    );
}

#[test]
fn gorging_vulture_counts_creatures_actually_milled_from_a_short_library() {
    let graveyard = Zone::Graveyard { owner: PlayerId(0) };
    let mut g = Game::new(
        board(&[(GORGING_VULTURE, Cell::Evaluated(EvaluatedVerb::Mill))])
            .permanent_or_hand("Gorging Vulture", 0, 0, true)
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Swamp", 0, 0, false)
            .library_card("Forest", 0)
            .library_card("Grizzly Bears", 0)
            .prepare(),
    );
    let life = g.state.rules.players[0].life.clone();
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1, 2, 3]);
    settle(&mut g);
    assert_eq!(g.zone(Driver::object(&g, 4)), Some(graveyard));
    assert_eq!(g.zone(Driver::object(&g, 5)), Some(graveyard));
    let mut expected = life;
    expected.add(&1.into());
    assert_eq!(g.state.rules.players[0].life, expected);
}

#[test]
fn sibsig_host_mills_each_players_own_library() {
    let mut g = Game::new(
        board(&[(SIBSIG_HOST, Cell::Evaluated(EvaluatedVerb::Mill))])
            .seats(4)
            .permanent_or_hand("Sibsig Host", 0, 0, true)
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Swamp", 0, 0, false)
            .library_card("Forest", 0)
            .library_card("Plains", 1)
            .library_card("Island", 2)
            .prepare(),
    );
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1, 2, 3, 4, 5]);
    settle(&mut g);
    for seat in 0..3 {
        assert_eq!(
            g.zone(Driver::object(&g, 6 + u32::from(seat))),
            Some(Zone::Graveyard {
                owner: PlayerId(seat)
            })
        );
    }
}

#[test]
fn wheel_of_fortune_discards_the_whole_hand_without_a_choice() {
    let hand = |seat| Zone::Hand {
        owner: PlayerId(seat),
    };
    let mut setup = board(&[(WHEEL_OF_FORTUNE, Cell::Evaluated(EvaluatedVerb::Discard))])
        .card("Wheel of Fortune", 0, 0, hand(0))
        .permanent_or_hand("Mountain", 0, 0, false)
        .permanent_or_hand("Mountain", 0, 0, false)
        .permanent_or_hand("Mountain", 0, 0, false)
        .card("Plains", 0, 0, hand(0))
        .card("Forest", 1, 1, hand(1))
        .card("Island", 1, 1, hand(1));
    for seat in 0..2 {
        for _ in 0..7 {
            setup = setup.library_card("Swamp", seat);
        }
    }
    let mut g = Game::new(setup.prepare());
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1, 2, 3]);
    settle(&mut g);
    for (id, owner) in [(4, 0), (5, 1), (6, 1)] {
        assert_eq!(
            g.zone(Driver::object(&g, id)),
            Some(Zone::Graveyard {
                owner: PlayerId(owner)
            })
        );
    }
}

#[test]
fn reckless_handling_reads_the_type_actually_discarded_this_way() {
    let hand = Zone::Hand { owner: PlayerId(0) };
    let graveyard = Zone::Graveyard { owner: PlayerId(0) };
    for (card, damage) in [("Sol Ring", 2), ("Forest", 0)] {
        let mut g = Game::new(
            board(&[(RECKLESS_HANDLING, Cell::Evaluated(EvaluatedVerb::Discard))])
                .card("Reckless Handling", 0, 0, hand)
                .permanent_or_hand("Mountain", 0, 0, false)
                .permanent_or_hand("Mountain", 0, 0, false)
                .card(card, 0, 0, hand)
                .prepare(),
        );
        let mut expected = g.state.rules.players[1].life.clone();
        expected.subtract(&damage.into());
        Driver::cast(&mut g, CardId(0));
        Driver::pay(&mut g, &[1, 2]);
        settle(&mut g);
        assert_eq!(g.zone(Driver::object(&g, 3)), Some(graveyard));
        assert_eq!(g.state.rules.players[1].life, expected);
    }
}

#[test]
fn eternal_witness_announces_one_target_then_offers_the_return() {
    let you = PlayerId(0);
    let graveyard = Zone::Graveyard { owner: you };
    let hand = Zone::Hand { owner: you };
    let prepared = board(&[(
        ETERNAL_WITNESS,
        Cell::Evaluated(mtg_contracts::kir::EvaluatedVerb::Move),
    )])
    .card("Eternal Witness", 0, 0, hand)
    .permanent_or_hand("Forest", 0, 0, false)
    .permanent_or_hand("Forest", 0, 0, false)
    .permanent_or_hand("Forest", 0, 0, false)
    .card("Grizzly Bears", 0, 0, graveyard)
    .card("Island", 1, 1, Zone::Graveyard { owner: PlayerId(1) })
    .prepare();
    for flag in [Flag::Decline, Flag::Accept] {
        let mut g = Game::new(prepared.clone());
        Driver::cast(&mut g, CardId(0));
        Driver::pay(&mut g, &[1, 2, 3]);
        let targets = next_choice(&mut g);
        let target = Driver::object(&g, 4);
        assert!(matches!(targets.reason, Reason::Targets { .. }));
        assert_eq!(objects(&targets), [target]);
        Driver::pick_object(&mut g, target);
        let d = next_choice(&mut g);
        assert_eq!((d.player, &d.reason), (you, &Reason::OptionalEffect));
        let answer = d.pick_one(&Candidate::Flag { flag }).unwrap();
        g.respond(d.player, d.id, answer).unwrap();
        settle(&mut g);
        let current = Driver::object(&g, 4);
        if flag == Flag::Accept {
            assert_ne!(current, target);
            assert_eq!(g.zone(current), Some(hand));
        } else {
            assert_eq!(current, target);
            assert_eq!(g.zone(current), Some(graveyard));
        }
    }
}

#[test]
fn archaeomancer_targets_either_spell_type_from_its_own_graveyard() {
    let graveyard = Zone::Graveyard { owner: PlayerId(0) };
    let mut g = Game::new(
        board(&[
            (
                ARCHAEOMANCER,
                Cell::Evaluated(mtg_contracts::kir::EvaluatedVerb::Move),
            ),
            (
                DISENTOMB,
                Cell::Evaluated(mtg_contracts::kir::EvaluatedVerb::Move),
            ),
        ])
        .permanent_or_hand("Archaeomancer", 0, 0, true)
        .permanent_or_hand("Island", 0, 0, false)
        .permanent_or_hand("Island", 0, 0, false)
        .permanent_or_hand("Island", 0, 0, false)
        .permanent_or_hand("Island", 0, 0, false)
        .card("Unsummon", 0, 0, graveyard)
        .card("Disentomb", 0, 0, graveyard)
        .card("Grizzly Bears", 0, 0, graveyard)
        .prepare(),
    );
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1, 2, 3, 4]);
    let targets = next_choice(&mut g);
    let target = Driver::object(&g, 5);
    assert_eq!(objects(&targets), [target, Driver::object(&g, 6)]);
    Driver::pick_object(&mut g, target);
    settle(&mut g);
    assert_eq!(g.zone(target), None);
    assert_eq!(
        g.zone(Driver::object(&g, 5)),
        Some(Zone::Hand { owner: PlayerId(0) })
    );
}

#[test]
fn graveyard_creature_can_enter_untapped_from_a_spell_or_tapped_from_an_ability() {
    for tapped in [false, true] {
        let graveyard = Zone::Graveyard { owner: PlayerId(0) };
        let (raw, name, land) = if tapped {
            (GRAVEWAKER, "Gravewaker", "Swamp")
        } else {
            (RESURRECTION, "Resurrection", "Plains")
        };
        let mut setup = board(&[(
            raw,
            Cell::Evaluated(mtg_contracts::kir::EvaluatedVerb::Move),
        )])
        .permanent_or_hand(name, 0, 0, !tapped)
        .card("Grizzly Bears", 0, 0, graveyard)
        .card("Forest", 0, 0, graveyard);
        let lands: Vec<u32> = (3..if tapped { 10 } else { 7 }).collect();
        for _ in &lands {
            setup = setup.permanent_or_hand(land, 0, 0, false);
        }
        let mut g = Game::new(setup.prepare());
        if tapped {
            let object = Driver::object(&g, 0);
            Driver::action(&mut g, Action::ActivateAbility { object, ability: 0 });
        } else {
            Driver::cast(&mut g, CardId(0));
        }
        let targets = Driver::request(&mut g, 1);
        checked(&g);
        let target = Driver::object(&g, 1);
        assert_eq!(objects(&targets), [target]);
        Driver::pick_object(&mut g, target);
        Driver::pay(&mut g, &lands);
        settle(&mut g);
        let new = Driver::object(&g, 1);
        assert_ne!(new, target);
        assert_eq!(g.zone(new), Some(Zone::Battlefield));
        let object = &g.state.rules.objects[&new.card];
        assert_eq!(object.controller, PlayerId(0));
        assert_eq!(object.tapped, tapped);
    }
}

#[test]
fn index_looks_privately_orders_a_short_prefix_and_rejects_forged_state() {
    let you = PlayerId(0);
    let library = Zone::Library { owner: you };
    let mut g = Game::new(
        board(&[(INDEX, Cell::ReorderLibraryTop)])
            .permanent_or_hand("Index", 0, 0, true)
            .permanent_or_hand("Island", 0, 0, false)
            .library_card("Plains", 0)
            .library_card("Swamp", 0)
            .library_card("Forest", 0)
            .prepare(),
    );
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1]);
    let d = next_choice(&mut g);
    assert_eq!((d.player, &d.reason), (you, &Reason::OrderLibraryTop));
    assert_eq!(
        objects(&d),
        (2..5).map(|id| Driver::object(&g, id)).collect::<Vec<_>>()
    );
    assert_eq!(
        view(&g, 0)
            .view
            .objects
            .iter()
            .filter(|row| row.zone == library)
            .count(),
        3
    );
    assert_hidden(&g, 1, library);
    let old = g.state.rules.frame.clone();
    let Frame::ResolutionChoice {
        choice: ResolutionChoice::ReorderLibraryTop { looked, .. },
        ..
    } = &mut g.state.rules.frame
    else {
        panic!("expected private reorder")
    };
    looked.swap(0, 1);
    assert!(g.validate_state().is_err(), "forged prefix order");
    g.state.rules.frame = old;
    let before = g.state_digest().unwrap();
    assert!(
        g.respond(
            d.player,
            d.id,
            Answer::Order {
                order: vec![0, 0, 2]
            }
        )
        .is_err()
    );
    assert_eq!(g.state_digest().unwrap(), before);
    order(&mut g, &d, vec![2, 0, 1]);
    settle(&mut g);
    assert_eq!(
        g.state.rules.libraries[&you],
        [CardId(4), CardId(2), CardId(3)]
    );
    assert!(view(&g, 1).view.known_library_positions.is_empty());
}

#[test]
fn top_search_shuffles_the_remainder_and_reveals_only_when_printed() {
    let you = PlayerId(0);
    let library = Zone::Library { owner: you };
    for (raw, tutor, mana, reveal, choose) in [
        (VAMPIRIC_TUTOR, "Vampiric Tutor", "Swamp", false, true),
        (WORLDLY_TUTOR, "Worldly Tutor", "Forest", true, true),
        (WORLDLY_TUTOR, "Worldly Tutor", "Forest", true, false),
    ] {
        let mut g = Game::with_trace(
            board(&[(raw, Cell::SearchLibraryTop)])
                .permanent_or_hand(tutor, 0, 0, true)
                .permanent_or_hand(mana, 0, 0, false)
                .library_card("Island", 0)
                .library_card("Grizzly Bears", 0)
                .library_card("Swamp", 0)
                .prepare(),
            TraceMode::Detailed,
        );
        Driver::cast(&mut g, CardId(0));
        Driver::pay(&mut g, &[1]);
        let d = next_choice(&mut g);
        assert_eq!((d.player, &d.reason), (you, &Reason::SearchLibrary));
        assert_eq!(objects(&d).len(), if reveal { 1 } else { 3 });
        assert_hidden(&g, 1, library);
        let target = Driver::object(&g, 3);
        let picks = if choose {
            vec![d.index(&Candidate::Object { object: target }).unwrap()]
        } else {
            vec![]
        };
        g.respond(d.player, d.id, Answer::Pick { picks: vec![picks] })
            .unwrap();
        settle(&mut g);
        let mut revealed = false;
        let mut shuffled = false;
        for event in g.events() {
            match event {
                Event::Committed {
                    proposed: ProposedEvent::RevealFound { objects, .. },
                    ..
                } => {
                    assert_eq!(objects, &[target]);
                    assert!(!shuffled);
                    revealed = true;
                }
                Event::Committed {
                    proposed: ProposedEvent::ShuffleLibraryToTop { found, .. },
                    actual: EventOutcome::LibraryShuffled { count, .. },
                } => {
                    assert!(!shuffled);
                    assert_eq!(*count, 1);
                    assert_eq!(*found, choose.then_some(target));
                    shuffled = true;
                }
                _ => {}
            }
        }
        assert!(shuffled);
        assert_eq!(revealed, reveal && choose);
        let library = &g.state.rules.libraries[&you];
        if choose {
            assert_eq!(library[0], target.card);
            assert_eq!(g.zone(target), Some(Zone::Library { owner: you }));
        }
        assert_eq!(library.len(), 3);
        for seat in 0..2 {
            let known = view(&g, seat).view.known_library_positions;
            assert_eq!(known.len(), usize::from(reveal && choose));
            if let [position] = known.as_slice() {
                assert_eq!(position.position, 1);
                assert_eq!(position.library_owner, you);
            }
        }
    }
}

#[test]
fn probe_rejects_a_target_whose_announcement_requires_kicker() {
    let probe = record!("f8d227b6-1627-4ea1-b815-887094497abf");
    let result = mtg_witness::compile(mtg_witness::R5_MANIFEST, probe);
    let mtg_compiler::CompileResult::Unsupported { reason, .. } = result else {
        panic!("expected rejection of Probe's conditional target: {result:?}");
    };
    assert_eq!(reason, "cell:target:cost_conditional_announcement");
}

#[test]
fn ponder_rejects_invalid_completed_reorders_while_waiting_to_shuffle() {
    let ponder = record!("02090581-61aa-4348-ad57-451be8ee91c2");
    let mut g = Game::new(
        board(&[(ponder, Cell::ReorderLibraryTop)])
            .permanent_or_hand("Ponder", 0, 0, true)
            .permanent_or_hand("Island", 0, 0, false)
            .library_card("Plains", 0)
            .library_card("Forest", 0)
            .library_card("Swamp", 0)
            .library_card("Island", 0)
            .library_card("Forest", 1)
            .prepare(),
    );
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1]);
    let decision = next_choice(&mut g);
    assert_eq!(decision.reason, Reason::OrderLibraryTop);
    order(&mut g, &decision, vec![2, 0, 1]);
    let shuffle = next_choice(&mut g);
    assert_eq!(shuffle.reason, Reason::OptionalEffect);
    let original = g.state.rules.frames[0].clone();
    let outside_prefix = Driver::object(&g, 5);
    let other_library = Driver::object(&g, 6);
    let battlefield = Driver::object(&g, 1);
    for forged in [
        "duplicate",
        "different pool",
        "other library",
        "battlefield",
    ] {
        let Frame::Resolve { frame } = &mut g.state.rules.frames[0] else {
            panic!("expected the saved invocation");
        };
        let [
            InstructionOutcome::Effect {
                actual: Some(EventOutcome::LibraryTopReordered { looked, top, .. }),
            },
        ] = frame.outcomes.as_mut_slice()
        else {
            panic!("expected the completed reorder");
        };
        match forged {
            "duplicate" => top[1] = top[0],
            "different pool" => top[1] = outside_prefix,
            "other library" | "battlefield" => {
                let replacement = if forged == "other library" {
                    other_library
                } else {
                    battlefield
                };
                let original = top[1];
                top[1] = replacement;
                let looked = looked.iter_mut().find(|card| **card == original).unwrap();
                *looked = replacement;
            }
            _ => unreachable!(),
        }
        let error = g.validate_state().expect_err(forged);
        assert!(
            error.contains("completed library reorder"),
            "{forged}: {error}"
        );
        g.state.rules.frames[0] = original.clone();
    }
    let decline = shuffle
        .pick_one(&Candidate::Flag {
            flag: Flag::Decline,
        })
        .unwrap();
    g.respond(shuffle.player, shuffle.id, decline).unwrap();
    settle(&mut g);
    assert_eq!(
        g.zone(Driver::object(&g, 4)),
        Some(Zone::Hand { owner: PlayerId(0) }),
    );
}

fn hand_size(g: &Game, seat: u8) -> usize {
    let hand = Zone::Hand {
        owner: PlayerId(seat),
    };
    g.state
        .rules
        .objects
        .values()
        .filter(|object| object.zone == hand)
        .count()
}

/// CR 806.1, 903.2, 608.2c, 701.9c: each other player discards, and the caster
/// draws one card for each card actually discarded; an empty hand adds none.
#[test]
fn syphon_mind_draws_for_each_card_the_other_players_discarded() {
    let hand = |seat| Zone::Hand {
        owner: PlayerId(seat),
    };
    let graveyard = |seat| Zone::Graveyard {
        owner: PlayerId(seat),
    };
    let mut setup = board(&[(SYPHON_MIND, Cell::Evaluated(EvaluatedVerb::Discard))])
        .seats(4)
        .card("Syphon Mind", 0, 0, hand(0));
    for _ in 0..4 {
        setup = setup.card("Swamp", 0, 0, Zone::Battlefield);
    }
    let mut first = Game::new(
        setup
            .card("Forest", 1, 1, hand(1))
            .card("Plains", 1, 1, hand(1))
            .card("Island", 2, 2, hand(2))
            .library_card("Forest", 0)
            .library_card("Plains", 0)
            .library_card("Island", 0)
            .prepare(),
    );
    Driver::cast(&mut first, CardId(0));
    Driver::pay(&mut first, &[1, 2, 3, 4]);
    let choice = next_choice(&mut first);
    assert_eq!(
        (choice.player, &choice.reason),
        (PlayerId(1), &Reason::ResolveDiscard)
    );
    // The caster is never asked to discard.
    assert_eq!(hand_size(&first, 0), 0);
    assert_recoverable((&first).into());
    let mut second = first.fork();
    let (forest, plains) = (Driver::object(&first, 5), Driver::object(&second, 6));
    Driver::pick_object(&mut first, forest);
    Driver::pick_object(&mut second, plains);
    for game in [&mut first, &mut second] {
        let island = next_choice(game);
        assert_eq!(
            (island.player, &island.reason),
            (PlayerId(2), &Reason::ResolveDiscard)
        );
        assert_recoverable((&*game).into());
        let card = Driver::object(game, 7);
        Driver::pick_object(game, card);
        // Seat 3's empty hand discards nothing and asks nothing.
        settle(game);
        assert_eq!(game.zone(Driver::object(game, 7)), Some(graveyard(2)));
        assert_eq!(hand_size(game, 0), 2);
        assert_eq!(hand_size(game, 1), 1);
        assert_eq!(hand_size(game, 3), 0);
        assert_eq!(
            game.zone(Driver::object(game, 10)),
            Some(Zone::Library { owner: PlayerId(0) })
        );
    }
    assert_eq!(first.zone(Driver::object(&first, 5)), Some(graveyard(1)));
    assert_eq!(second.zone(Driver::object(&second, 6)), Some(graveyard(1)));
}

/// CR 608.2c, 701.9c: the gain counts only land cards among the target
/// player's actual discards.
#[test]
fn gerrards_verdict_gains_three_for_each_land_card_discarded() {
    let opponent = PlayerId(1);
    let hand = Zone::Hand { owner: opponent };
    let mut g = Game::new(
        board(&[(GERRARDS_VERDICT, Cell::Evaluated(EvaluatedVerb::Discard))])
            .card("Gerrard's Verdict", 0, 0, Zone::Hand { owner: PlayerId(0) })
            .card("Plains", 0, 0, Zone::Battlefield)
            .card("Swamp", 0, 0, Zone::Battlefield)
            .card("Forest", 1, 1, hand)
            .card("Grizzly Bears", 1, 1, hand)
            .card("Island", 1, 1, hand)
            .prepare(),
    );
    let life = g.state.rules.players[0].life.clone();
    Driver::cast(&mut g, CardId(0));
    let targets = Driver::request(&mut g, 1);
    let answer = targets
        .pick_one(&Candidate::Player { player: opponent })
        .unwrap();
    g.respond(targets.player, targets.id, answer).unwrap();
    Driver::pay(&mut g, &[1, 2]);
    let d = next_choice(&mut g);
    assert_eq!((d.player, &d.reason), (opponent, &Reason::ResolveDiscard));
    assert_recoverable((&g).into());
    let forest = Driver::object(&g, 3);
    let bears = Driver::object(&g, 4);
    let picks = objects(&d)
        .iter()
        .enumerate()
        .filter(|(_, object)| [forest, bears].contains(object))
        .map(|(index, _)| index as u32)
        .collect();
    g.respond(d.player, d.id, Answer::Pick { picks: vec![picks] })
        .unwrap();
    settle(&mut g);
    let graveyard = Some(Zone::Graveyard { owner: opponent });
    assert_eq!(g.zone(Driver::object(&g, 3)), graveyard);
    assert_eq!(g.zone(Driver::object(&g, 4)), graveyard);
    assert_eq!(g.zone(Driver::object(&g, 5)), Some(hand));
    let mut expected = life;
    expected.add(&3.into());
    assert_eq!(g.state.rules.players[0].life, expected);
}
