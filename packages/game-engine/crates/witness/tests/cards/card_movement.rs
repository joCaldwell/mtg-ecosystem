//! Frozen R4 card movement, discard and private library procedures.
//! CR 101.4, 400.2, 400.7, 601.2c, 608.2b-d, 701.9, 701.20, 701.23, 701.24b:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use mtg_contracts::kir::{Cell, definition_cells};
use mtg_core::testkit::*;

macro_rules! record {
    ($id:literal) => {
        include_str!(concat!(
            "../../../../tests/fixtures/phase1-r4/",
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
const HYMN: &str = record!("992d9a72-d34d-47d7-96fb-37f0efb515ef");
const INDEX: &str = record!("613b824e-39d1-4c6e-a19c-c915abdc860b");
const RESURRECTION: &str = record!("837417d8-8260-486d-a3ed-3b5711eaf34a");
const VAMPIRIC_TUTOR: &str = record!("ededbdae-d9dc-4206-9335-d7158f2d7700");
const WORLDLY_TUTOR: &str = record!("e8863518-0bfa-49c3-8c6e-6c9116a81051");

fn board(cards: &[(&str, Cell)]) -> Board {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    for &(raw, cell) in cards {
        let result = mtg_witness::compile(mtg_witness::R4_MANIFEST, raw);
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
        board(&[(CAREFUL_STUDY, Cell::DiscardController)])
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
    let mut g = Game::new(
        board(&[(BURGLAR_RAT, Cell::DiscardGroup)])
            .seats(3)
            .card("Burglar Rat", 0, 0, hand(0))
            .card("Swamp", 0, 0, Zone::Battlefield)
            .card("Swamp", 0, 0, Zone::Battlefield)
            .card("Forest", 1, 1, hand(1))
            .card("Island", 2, 2, hand(2))
            .prepare(),
    );
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1, 2]);
    for (seat, id, other) in [(1, 3, 2), (2, 4, 1)] {
        let d = next_choice(&mut g);
        assert_eq!(
            (d.player, &d.reason),
            (PlayerId(seat), &Reason::ResolveDiscard)
        );
        assert_eq!(objects(&d), [Driver::object(&g, id)]);
        assert_hidden(&g, other, hand(seat));
        assert!(
            view(&g, seat)
                .view
                .objects
                .iter()
                .all(|row| row.zone != hand(other))
        );
        assert_eq!(g.zone(Driver::object(&g, 3)), Some(hand(1)));
        Driver::pick_object(&mut g, objects(&d)[0]);
    }
    settle(&mut g);
    for (seat, id) in [(1, 3), (2, 4)] {
        assert_eq!(
            g.zone(Driver::object(&g, id)),
            Some(Zone::Graveyard {
                owner: PlayerId(seat)
            })
        );
    }
}

#[test]
fn hymn_discards_two_random_distinct_cards_and_replays() {
    let opponent = PlayerId(1);
    let hand = Zone::Hand { owner: opponent };
    let graveyard = Zone::Graveyard { owner: opponent };
    let mut g = Game::new(
        board(&[(HYMN, Cell::DiscardTargetRandom)])
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
    settle(&mut g);
    let zones: Vec<_> = (3..6).map(|id| g.zone(Driver::object(&g, id))).collect();
    assert_eq!(zones.iter().filter(|z| **z == Some(graveyard)).count(), 2);
    assert_eq!(zones.iter().filter(|z| **z == Some(hand)).count(), 1);
}

#[test]
fn eternal_witness_announces_one_target_then_offers_the_return() {
    let you = PlayerId(0);
    let graveyard = Zone::Graveyard { owner: you };
    let hand = Zone::Hand { owner: you };
    let prepared = board(&[(ETERNAL_WITNESS, Cell::MoveOwnGraveyardTargetToHand)])
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
            (ARCHAEOMANCER, Cell::MoveOwnGraveyardTargetToHand),
            (DISENTOMB, Cell::MoveOwnGraveyardTargetToHand),
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
        let mut setup = board(&[(raw, Cell::MoveOwnGraveyardCreatureToBattlefield)])
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
