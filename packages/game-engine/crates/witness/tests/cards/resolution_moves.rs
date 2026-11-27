//! RESOLVE-MOVE: cards of the controller's own hand or graveyard chosen while
//! an instruction resolves and moved together, from exact frozen R5 source.
//! These choices never target (CR 115.10). A hand choice is private to its
//! chooser and is asked whenever the hand could hide a match, so no other
//! seat learns whether a card matched (CR 402.3). A "may" or "up to" choice
//! may choose none (CR 608.2d); otherwise as many as possible are chosen.
//! "At random" from the graveyard is a seeded selection nobody makes.
//! Every decision is checked for state validity and an exact snapshot round
//! trip; every finished resolution replays from its journal.
//! Pinned CR 110.2, 115.10, 400.7, 402.3, 404.2, 608.2c-d, 614.1c, 903.9b:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use mtg_contracts::kir::{Cell, Tiers, definition_cells, definition_tiers};
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

const GROWTH_SPIRAL: &str = record!("34bcc217-dd91-45a0-90d7-a94d02f1f317");
const ARBOREAL_GRAZER: &str = record!("d18a0815-59d3-4667-b52b-9acda741215e");
const ELVISH_PIPER: &str = record!("872ef617-7cdb-4a7a-85f5-efc9e12bece5");
const GRAPPLE_WITH_THE_PAST: &str = record!("d14c633d-977a-4f78-85c2-88582b3c3670");
const INSPIRATION_FROM_BEYOND: &str = record!("be87e410-4302-43ee-b880-56eedf2b3648");
const PORT_OF_KARFELL: &str = record!("f500a8a4-6135-448c-9116-bd2695a72229");
const ANOTHER_CHANCE: &str = record!("df9cdc11-cb92-46ce-a83d-eb26e3cd004a");
const HAUNTED_FENGRAF: &str = record!("7c6143f3-ad2c-4d7f-9041-aa59f01d8fb7");
const MAKE_A_WISH: &str = record!("888ccda6-896f-40f3-93a1-efc48254af34");
const TOMB_TYRANT: &str = record!("0fb2c61a-9e74-4ee2-810a-b77a3b45f446");
const GHOULRAISER: &str = record!("614f748b-5d31-440d-a7c0-3ba12dc63c24");
const KONA: &str = record!("76372327-41ec-4516-abff-3b612e073629");
const PACIFISM: &str = record!("5f5e0b10-c8cf-450c-bfd3-bcb0528ec330");
const BREEDING_POOL: &str = record!("20283c4a-f1f0-42f0-bc08-6da87474426b");

const SEED: [u8; 32] = [11; 32];

/// A seeded two-seat main-phase board whose catalog adds `cards`, each
/// compiled from frozen source to `Executes` and occupying the named cells.
fn board(cards: &[(&str, &[Cell])]) -> Board {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    for &(raw, cells) in cards {
        let result = mtg_witness::compile(mtg_witness::R5_MANIFEST, raw);
        let mtg_compiler::CompileResult::Lowered { definition } = result else {
            panic!("frozen resolution move witness failed: {result:?}");
        };
        assert_eq!(definition_tiers(&definition).unwrap(), Tiers::EXECUTES);
        let occupied = definition_cells(&definition).unwrap();
        for cell in cells {
            assert!(
                occupied.contains(cell),
                "{} lacks {cell:?}: {occupied:?}",
                definition.name
            );
        }
        catalog.definitions.push(*definition);
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    Board::new(catalog, "r5-resolution-moves")
        .step(TurnStep::PrecombatMain)
        .seats(2)
        .seed(SEED)
}

/// State validation and an exact snapshot round trip.
fn checked(g: &Game) {
    g.validate_state().unwrap();
    let snapshot = g.snapshot().unwrap();
    let restored = Game::restore(g.prepared.as_ref().clone(), &snapshot).unwrap();
    assert_eq!(restored.snapshot().unwrap(), snapshot);
}

fn observation(g: &Game, seat: u8) -> Observation {
    let QueryResult::Complete(QueryValue::Observation(o)) =
        g.observe(PlayerId(seat), Budget::unlimited())
    else {
        panic!("missing observation for seat {seat}");
    };
    o
}

/// Pass priority until the next decision that is not priority.
fn next_choice(g: &mut Game) -> Decision<Rules> {
    for _ in 0..16 {
        let d = Driver::request(g, 1);
        if d.reason != Reason::Priority {
            return d;
        }
        Driver::pass(g);
    }
    panic!("no choice within 16 priority passes")
}

/// The pending card choice: seat 0's, with exactly `offered` and the given
/// bounds; seat 1 has no decision.
fn card_choice(g: &mut Game, offered: &[u32], min: u32, max: u32) -> Decision<Rules> {
    let d = next_choice(g);
    assert_eq!(
        (d.player, &d.reason),
        (PlayerId(0), &Reason::ResolveCardChoice),
        "{d:?}"
    );
    let expected: Vec<_> = offered
        .iter()
        .map(|id| Candidate::Object {
            object: Driver::object(g, *id),
        })
        .collect();
    assert_eq!(d.candidates, expected);
    let Shape::Pick { slots, .. } = &d.shape else {
        panic!("{:?}", d.shape)
    };
    assert_eq!((slots.len(), slots[0].min, slots[0].max), (1, min, max));
    assert!(observation(g, 1).decision.is_none());
    checked(g);
    d
}

fn choose(g: &mut Game, d: &Decision<Rules>, ids: &[u32]) {
    let picks = ids
        .iter()
        .map(|id| {
            d.index(&Candidate::Object {
                object: Driver::object(g, *id),
            })
            .unwrap()
        })
        .collect();
    g.respond(d.player, d.id, Answer::Pick { picks: vec![picks] })
        .unwrap();
}

fn flag(g: &mut Game, reason: Reason<Rules>, flag: Flag) {
    let d = next_choice(g);
    assert_eq!((d.player, &d.reason), (PlayerId(0), &reason), "{d:?}");
    checked(g);
    g.respond(
        d.player,
        d.id,
        d.pick_one(&Candidate::Flag { flag }).unwrap(),
    )
    .unwrap();
}

/// Resolve to an empty stack, then prove replay from the journal.
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

fn object(g: &Game, card: u32) -> &Object {
    &g.state.rules.objects[&CardId(card)]
}

fn zone(g: &Game, card: u32) -> Zone {
    object(g, card).zone
}

const HAND: Zone = Zone::Hand { owner: PlayerId(0) };
const GRAVEYARD: Zone = Zone::Graveyard { owner: PlayerId(0) };

/// Growth Spiral with seat 0's hand card 3 named `hidden`; card 5 is drawn.
fn growth_spiral(hidden: &str) -> Game {
    let prepared = board(&[
        (GROWTH_SPIRAL, &[Cell::ChooseHandCards]),
        (BREEDING_POOL, &[]),
    ])
    .permanent_or_hand("Growth Spiral", 0, 0, true)
    .permanent_or_hand("Forest", 0, 0, false)
    .permanent_or_hand("Island", 0, 0, false)
    .permanent_or_hand(hidden, 0, 0, true)
    .permanent_or_hand("Grizzly Bears", 0, 0, true)
    .library_card("Grizzly Bears", 0)
    .library_card("Island", 1)
    .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1, 2]);
    g
}

/// Growth Spiral: "Draw a card. You may put a land card from your hand onto
/// the battlefield." One private choice offers only the hand's lands and
/// may choose none; the land enters untapped under its owner's control.
#[test]
fn growth_spiral_puts_a_chosen_hand_land_onto_the_battlefield() {
    for take in [true, false] {
        let mut g = growth_spiral("Forest");
        let d = card_choice(&mut g, &[3], 0, 1);
        assert_eq!(zone(&g, 5), HAND, "the draw comes first");
        choose(&mut g, &d, if take { &[3] } else { &[] });
        settle(&mut g);
        if take {
            assert_eq!(zone(&g, 3), Zone::Battlefield);
            assert!(!object(&g, 3).tapped);
            assert_eq!(object(&g, 3).controller, PlayerId(0));
        } else {
            assert_eq!(zone(&g, 3), HAND);
        }
        assert_eq!(zone(&g, 4), HAND);
    }
}

/// CR 402.3: seat 0 is asked even with no land in hand, and seat 1's
/// permitted observation is identical whether or not a hidden card matched,
/// both while seat 0 chooses and after it chooses none.
#[test]
fn growth_spiral_hand_choice_reveals_nothing_to_other_seats() {
    let mut views = vec![];
    for (hidden, offered) in [("Forest", &[3][..]), ("Lightning Bolt", &[][..])] {
        let mut g = growth_spiral(hidden);
        let d = card_choice(&mut g, offered, 0, u32::from(!offered.is_empty()));
        let waiting = serde_json::to_string(&observation(&g, 1)).unwrap();
        choose(&mut g, &d, &[]);
        settle(&mut g);
        assert_eq!(zone(&g, 3), HAND);
        views.push((waiting, serde_json::to_string(&observation(&g, 1)).unwrap()));
    }
    assert_eq!(views[0].0, views[1].0, "seat 1 learns which card matched");
    assert_eq!(views[0].1, views[1].1, "seat 1 learns the declined card");
}

/// CR 614.1c: a chosen shockland still asks its entry payment, then enters
/// untapped for 2 life.
#[test]
fn growth_spiral_entry_keeps_the_lands_entry_payment() {
    let mut g = growth_spiral("Breeding Pool");
    let life = g.state.rules.players[0].life.clone();
    let d = card_choice(&mut g, &[3], 0, 1);
    choose(&mut g, &d, &[3]);
    flag(&mut g, Reason::OptionalEffect, Flag::Accept);
    settle(&mut g);
    assert_eq!(zone(&g, 3), Zone::Battlefield);
    assert!(!object(&g, 3).tapped);
    let mut paid = g.state.rules.players[0].life.clone();
    paid.add(&2.into());
    assert_eq!(paid, life);
}

/// The suspended hand choice belongs to its controller; a forged chooser
/// fails validation, and a departed chooser's choice is skipped (CR 800.4a).
#[test]
fn hand_choice_rejects_a_forged_chooser_and_skips_a_departure() {
    let mut g = growth_spiral("Forest");
    card_choice(&mut g, &[3], 0, 1);
    let original = g.state.rules.frame.clone();
    let Frame::ResolutionChoice {
        choice: ResolutionChoice::Search { player, .. },
        ..
    } = &mut g.state.rules.frame
    else {
        panic!("{original:?}")
    };
    *player = PlayerId(1);
    assert!(g.validate_state().is_err());
    g.state.rules.frame = original;
    checked(&g);
    let mut departed = g.fork();
    departed.concede(PlayerId(0)).unwrap();
    assert_eq!(Driver::request(&mut departed, 1).reason, Reason::Priority);
    assert!(departed.state.rules.stack.is_empty());
    assert_recoverable((&departed).into());
}

/// Arboreal Grazer: "When this creature enters, you may put a land card from
/// your hand onto the battlefield tapped."
#[test]
fn arboreal_grazer_puts_the_chosen_land_onto_the_battlefield_tapped() {
    let prepared = board(&[(ARBOREAL_GRAZER, &[Cell::ChooseHandCards])])
        .permanent_or_hand("Arboreal Grazer", 0, 0, true)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Island", 0, 0, true)
        .permanent_or_hand("Grizzly Bears", 0, 0, true)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1]);
    let d = card_choice(&mut g, &[2], 0, 1);
    choose(&mut g, &d, &[2]);
    settle(&mut g);
    assert_eq!(zone(&g, 2), Zone::Battlefield);
    assert!(object(&g, 2).tapped);
    assert_eq!(zone(&g, 3), HAND);
}

/// Elvish Piper: "{G}, {T}: You may put a creature card from your hand onto
/// the battlefield." Only the creature card is offered.
#[test]
fn elvish_piper_offers_only_creature_cards() {
    let prepared = board(&[(ELVISH_PIPER, &[Cell::ChooseHandCards])])
        .permanent_or_hand("Elvish Piper", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Colossal Dreadmaw", 0, 0, true)
        .permanent_or_hand("Forest", 0, 0, true)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let piper = Driver::object(&g, 0);
    Driver::action(
        &mut g,
        Action::ActivateAbility {
            object: piper,
            ability: 0,
        },
    );
    Driver::pay(&mut g, &[1]);
    let d = card_choice(&mut g, &[2], 0, 1);
    choose(&mut g, &d, &[2]);
    settle(&mut g);
    assert_eq!(zone(&g, 2), Zone::Battlefield);
    assert!(!object(&g, 2).tapped);
    assert_eq!(zone(&g, 3), HAND);
}

/// Grapple with the Past: "Mill three cards, then you may return a creature
/// or land card from your graveyard to your hand." The milled instant is
/// not offered; a returned commander offers its owner the command zone
/// (CR 903.9b); choosing none returns nothing.
#[test]
fn grapple_with_the_past_returns_a_creature_or_land_after_milling() {
    for pick in [None, Some(4), Some(6)] {
        let prepared = board(&[(GRAPPLE_WITH_THE_PAST, &[Cell::ChooseGraveyardCards])])
            .permanent_or_hand("Grapple with the Past", 0, 0, true)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .library_card("Lightning Bolt", 0)
            .library_card("Grizzly Bears", 0)
            .library_card("Forest", 0)
            .card("Isamaru, Hound of Konda", 0, 0, GRAVEYARD)
            .library_card("Island", 1)
            .commander(CardId(6))
            .prepare();
        let mut g = Game::with_trace(prepared, TraceMode::Detailed);
        Driver::cast(&mut g, CardId(0));
        Driver::pay(&mut g, &[1, 2]);
        let d = card_choice(&mut g, &[4, 5, 6], 0, 1);
        assert_eq!(zone(&g, 3), GRAVEYARD);
        choose(&mut g, &d, pick.as_slice());
        if pick == Some(6) {
            let d = next_choice(&mut g);
            assert!(matches!(d.reason, Reason::CommanderReturn { .. }), "{d:?}");
            checked(&g);
            g.respond(
                d.player,
                d.id,
                d.pick_one(&Candidate::Flag {
                    flag: Flag::CommandZone,
                })
                .unwrap(),
            )
            .unwrap();
        }
        settle(&mut g);
        for card in [3, 4, 5, 6] {
            let expected = match pick {
                Some(6) if card == 6 => Zone::Command { owner: PlayerId(0) },
                Some(chosen) if chosen == card => HAND,
                _ => GRAVEYARD,
            };
            assert_eq!(zone(&g, card), expected, "card {card} after {pick:?}");
        }
    }
}

/// Inspiration from Beyond: "Mill three cards, then return an instant or
/// sorcery card from your graveyard to your hand." The return is
/// mandatory: two matches ask for exactly one, a single match moves
/// without a question and none moves nothing.
#[test]
fn inspiration_from_beyond_must_return_an_instant_or_sorcery() {
    for (milled, offered) in [
        (
            ["Lightning Bolt", "Disfigure", "Grizzly Bears"],
            &[3, 4][..],
        ),
        (["Lightning Bolt", "Grizzly Bears", "Forest"], &[3][..]),
        (["Forest", "Grizzly Bears", "Island"], &[][..]),
    ] {
        let mut board = board(&[(INSPIRATION_FROM_BEYOND, &[Cell::ChooseGraveyardCards])])
            .permanent_or_hand("Inspiration from Beyond", 0, 0, true)
            .permanent_or_hand("Island", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, false);
        board = board.permanent_or_hand("Island", 0, 0, false);
        for name in milled {
            board = board.library_card(name, 0);
        }
        let prepared = board.library_card("Island", 1).prepare();
        let mut g = Game::with_trace(prepared, TraceMode::Detailed);
        // Cards: 0 spell, 1-3 lands, 4-6 milled.
        Driver::cast(&mut g, CardId(0));
        Driver::pay(&mut g, &[1, 2, 3]);
        let offered: Vec<u32> = offered.iter().map(|i| i + 1).collect();
        if offered.len() > 1 {
            let d = card_choice(&mut g, &offered, 1, 1);
            choose(&mut g, &d, &offered[1..]);
            settle(&mut g);
            assert_eq!(zone(&g, offered[1]), HAND);
            assert_eq!(zone(&g, offered[0]), GRAVEYARD);
        } else {
            settle(&mut g);
            for card in 4..7 {
                let expected = if offered.contains(&card) {
                    HAND
                } else {
                    GRAVEYARD
                };
                assert_eq!(zone(&g, card), expected, "{milled:?}");
            }
        }
    }
}

/// Port of Karfell: "{3}{U}{B}{B}, {T}, Sacrifice this land: Mill four
/// cards, then return a creature card from your graveyard to the
/// battlefield tapped." The chosen creature enters tapped under its owner's
/// control; the sacrificed land itself is no creature card.
#[test]
fn port_of_karfell_returns_a_milled_creature_tapped() {
    let prepared = board(&[(PORT_OF_KARFELL, &[Cell::ChooseGraveyardCards])])
        .permanent_or_hand("Port of Karfell", 0, 0, false)
        .permanent_or_hand("Island", 0, 0, false)
        .permanent_or_hand("Swamp", 0, 0, false)
        .permanent_or_hand("Swamp", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .library_card("Grizzly Bears", 0)
        .library_card("Lightning Bolt", 0)
        .library_card("Colossal Dreadmaw", 0)
        .library_card("Forest", 0)
        .library_card("Island", 0)
        .library_card("Island", 1)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let port = Driver::object(&g, 0);
    Driver::action(
        &mut g,
        Action::ActivateAbility {
            object: port,
            ability: 0,
        },
    );
    Driver::pay(&mut g, &[1, 2, 3, 4, 5, 6]);
    assert_eq!(zone(&g, 0), GRAVEYARD);
    let d = card_choice(&mut g, &[7, 9], 1, 1);
    choose(&mut g, &d, &[9]);
    settle(&mut g);
    assert_eq!(zone(&g, 9), Zone::Battlefield);
    assert!(object(&g, 9).tapped);
    assert_eq!(object(&g, 9).controller, PlayerId(0));
    assert_eq!(zone(&g, 7), GRAVEYARD);
    assert_eq!(zone(&g, 11), Zone::Library { owner: PlayerId(0) });
}

/// Another Chance: "You may mill two cards. Then return up to two creature
/// cards from your graveyard to your hand." Up to two may be chosen
/// together, including none. The declined mill leaves the library intact.
#[test]
fn another_chance_returns_up_to_two_creature_cards() {
    for picks in [&[4, 6][..], &[5][..], &[][..]] {
        let prepared = board(&[(ANOTHER_CHANCE, &[Cell::ChooseGraveyardCards])])
            .permanent_or_hand("Another Chance", 0, 0, true)
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Swamp", 0, 0, false)
            .card("Grizzly Bears", 0, 0, GRAVEYARD)
            .card("Colossal Dreadmaw", 0, 0, GRAVEYARD)
            .card("Typhoid Rats", 0, 0, GRAVEYARD)
            .card("Forest", 0, 0, GRAVEYARD)
            .library_card("Island", 0)
            .library_card("Island", 0)
            .library_card("Island", 0)
            .library_card("Island", 1)
            .prepare();
        let mut g = Game::with_trace(prepared, TraceMode::Detailed);
        Driver::cast(&mut g, CardId(0));
        Driver::pay(&mut g, &[1, 2, 3]);
        flag(&mut g, Reason::OptionalEffect, Flag::Decline);
        let d = card_choice(&mut g, &[4, 5, 6], 0, 2);
        choose(&mut g, &d, picks);
        settle(&mut g);
        for card in 4..8 {
            let expected = if picks.contains(&card) {
                HAND
            } else {
                GRAVEYARD
            };
            assert_eq!(zone(&g, card), expected, "card {card} after {picks:?}");
        }
        assert_eq!(zone(&g, 8), Zone::Library { owner: PlayerId(0) });
    }
}

/// Seat 0's consumed random-stream ordinal.
fn random_ordinal(g: &Game) -> u64 {
    g.state
        .rules
        .match_state
        .as_ref()
        .unwrap()
        .random
        .seat_random[0]
}

/// The number of `cards` in `zone`.
fn count_in(g: &Game, cards: &[u32], zone: Zone) -> usize {
    cards
        .iter()
        .filter(|card| self::zone(g, **card) == zone)
        .count()
}

/// Haunted Fengraf: "{3}, {T}, Sacrifice this land: Return a creature card
/// at random from your graveyard to your hand." Nobody is asked; one
/// seeded draw picks between two creature cards, none is drawn for one,
/// and the sacrificed land and the instant are never selected.
#[test]
fn haunted_fengraf_returns_a_creature_card_at_random() {
    for (second, creatures) in [("Typhoid Rats", &[4, 5][..]), ("Forest", &[4][..])] {
        let prepared = board(&[(HAUNTED_FENGRAF, &[Cell::RandomGraveyardCards])])
            .permanent_or_hand("Haunted Fengraf", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .card("Grizzly Bears", 0, 0, GRAVEYARD)
            .card(second, 0, 0, GRAVEYARD)
            .card("Lightning Bolt", 0, 0, GRAVEYARD)
            .library_card("Island", 0)
            .library_card("Island", 1)
            .prepare();
        let mut g = Game::with_trace(prepared, TraceMode::Detailed);
        let before = random_ordinal(&g);
        let fengraf = Driver::object(&g, 0);
        Driver::action(
            &mut g,
            Action::ActivateAbility {
                object: fengraf,
                ability: 0,
            },
        );
        Driver::pay(&mut g, &[1, 2, 3]);
        settle(&mut g);
        assert_eq!(count_in(&g, creatures, HAND), 1, "{second}");
        assert_eq!(count_in(&g, &[0, 4, 5, 6], HAND), 1, "{second}");
        assert_eq!(
            random_ordinal(&g) - before,
            u64::from(creatures.len() > 1),
            "{second}"
        );
    }
}

/// Make a Wish: "Return two cards at random from your graveyard to your
/// hand." Two of three cards move together; with two cards both move.
#[test]
fn make_a_wish_returns_two_cards_at_random() {
    for graveyard in [
        &["Grizzly Bears", "Lightning Bolt", "Island"][..],
        &["Grizzly Bears", "Island"][..],
    ] {
        let mut board = board(&[(MAKE_A_WISH, &[Cell::RandomGraveyardCards])]).permanent_or_hand(
            "Make a Wish",
            0,
            0,
            true,
        );
        for _ in 0..4 {
            board = board.permanent_or_hand("Forest", 0, 0, false);
        }
        for name in graveyard {
            board = board.card(name, 0, 0, GRAVEYARD);
        }
        let prepared = board
            .library_card("Island", 0)
            .library_card("Island", 1)
            .prepare();
        let mut g = Game::with_trace(prepared, TraceMode::Detailed);
        Driver::cast(&mut g, CardId(0));
        Driver::pay(&mut g, &[1, 2, 3, 4]);
        settle(&mut g);
        let cards: Vec<u32> = (5..5 + graveyard.len() as u32).collect();
        assert_eq!(count_in(&g, &cards, HAND), 2, "{graveyard:?}");
        assert_eq!(zone(&g, 0), GRAVEYARD);
    }
}

/// Tomb Tyrant: "... Sacrifice a creature: Return a Zombie creature card at
/// random from your graveyard to the battlefield." The entering
/// Ghoulraiser ("When this creature enters, return a Zombie card at random
/// from your graveyard to your hand") then returns one of the other two.
#[test]
fn tomb_tyrant_reanimates_a_random_zombie_whose_trigger_returns_another() {
    let prepared = board(&[
        (TOMB_TYRANT, &[Cell::RandomGraveyardCards]),
        (GHOULRAISER, &[Cell::RandomGraveyardCards]),
    ])
    .permanent_or_hand("Tomb Tyrant", 0, 0, false)
    .permanent_or_hand("Grizzly Bears", 0, 0, false)
    .permanent_or_hand("Swamp", 0, 0, false)
    .permanent_or_hand("Swamp", 0, 0, false)
    .permanent_or_hand("Swamp", 0, 0, false)
    .card("Ghoulraiser", 0, 0, GRAVEYARD)
    .card("Ghoulraiser", 0, 0, GRAVEYARD)
    .card("Ghoulraiser", 0, 0, GRAVEYARD)
    .library_card("Island", 0)
    .library_card("Island", 1)
    .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let tyrant = Driver::object(&g, 0);
    Driver::action(
        &mut g,
        Action::ActivateAbility {
            object: tyrant,
            ability: 0,
        },
    );
    Driver::pay(&mut g, &[2, 3, 4]);
    let d = Driver::request(&mut g, u64::MAX);
    assert_eq!(d.reason, Reason::SacrificeCost, "{d:?}");
    let bears = Driver::object(&g, 1);
    Driver::pick_object(&mut g, bears);
    settle(&mut g);
    let zombies = [5, 6, 7];
    assert_eq!(count_in(&g, &zombies, Zone::Battlefield), 1);
    assert_eq!(count_in(&g, &zombies, HAND), 1);
    assert_eq!(count_in(&g, &zombies, GRAVEYARD), 1);
    let entered = zombies
        .into_iter()
        .find(|card| zone(&g, *card) == Zone::Battlefield)
        .unwrap();
    assert!(!object(&g, entered).tapped);
    assert_eq!(object(&g, entered).controller, PlayerId(0));
    assert_eq!(zone(&g, 1), GRAVEYARD);
}

/// Kona, Rescue Beastie: "At the beginning of your second main phase, if
/// Kona is tapped, you may put a permanent card from your hand onto the
/// battlefield." A chosen Aura enchants an object its controller chooses
/// without targeting, already attached when it enters (CR 303.4f, 603.6a).
#[test]
fn kona_puts_an_aura_from_hand_onto_a_chosen_creature() {
    let prepared = board(&[(KONA, &[Cell::ChooseHandCards]), (PACIFISM, &[])])
        .step(TurnStep::BeginningCombat)
        .permanent_or_hand("Kona, Rescue Beastie", 0, 0, false)
        .permanent_or_hand("Pacifism", 0, 0, true)
        .permanent_or_hand("Grizzly Bears", 0, 0, true)
        .permanent_or_hand("Lightning Bolt", 0, 0, true)
        .permanent_or_hand("Grizzly Bears", 1, 1, false)
        .library_card("Island", 0)
        .library_card("Island", 1)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    // Kona attacks, so it is tapped when the second main phase begins.
    let kona = Driver::object(&g, 0);
    for _ in 0..64 {
        let r = Driver::request(&mut g, u64::MAX);
        let answer = match &r.reason {
            Reason::Priority => mtg_witness::temporary::pass(&r),
            Reason::Attackers => r.attack(vec![(kona, PlayerId(1))]),
            Reason::Blockers => r.block(vec![]),
            Reason::ResolveCardChoice => break,
            other => panic!("unexpected decision {other:?}"),
        };
        g.respond(r.player, r.id, answer).unwrap();
    }
    assert_eq!(g.state.rules.turn.step, TurnStep::PostcombatMain);
    let d = card_choice(&mut g, &[1, 2], 0, 1);
    choose(&mut g, &d, &[1]);
    let [pacifism, bears] = [1, 4].map(|id| Driver::object(&g, id));
    let d = next_choice(&mut g);
    assert_eq!(
        (d.player, &d.reason),
        (PlayerId(0), &Reason::EntryAttachment),
        "{d:?}"
    );
    let Shape::Pick { slots, .. } = &d.shape else {
        panic!("{d:?}")
    };
    assert_eq!(
        slots[0].subject.map(|i| d.candidates[i as usize].clone()),
        Some(Candidate::Object { object: pacifism })
    );
    let hosts: Vec<_> = slots[0]
        .allowed
        .iter()
        .map(|i| d.candidates[*i as usize].clone())
        .collect();
    assert_eq!(
        hosts,
        [kona, bears].map(|object| Candidate::Object { object })
    );
    checked(&g);
    g.respond(
        d.player,
        d.id,
        d.pick_one(&Candidate::Object { object: bears }).unwrap(),
    )
    .unwrap();
    settle(&mut g);
    let entered = g.current(Driver::object(&g, 1)).unwrap().clone();
    assert_eq!(entered.zone, Zone::Battlefield);
    assert_ne!(entered.id, pacifism, "a new object (CR 400.7)");
    assert_eq!(entered.controller, PlayerId(0));
    assert_eq!(entered.attached_to, Some(bears));
    assert!(g.events().iter().any(|e| matches!(e,
        Event::Committed { actual: EventOutcome::EnteredAttached { movement, target, .. }, .. }
            if movement.previous == pacifism && movement.from == HAND && *target == bears)));
    assert_eq!(zone(&g, 2), HAND);
}

/// Growth Spiral with its IR edited to put an enchantment card from hand:
/// no printed card in the lane puts an Aura from hand with no possible
/// host. CR 303.4g: with no creature to enchant, the chosen Pacifism is
/// never asked about and stays in its owner's hand.
#[test]
fn a_chosen_aura_without_a_host_stays_in_hand() {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    for raw in [GROWTH_SPIRAL, PACIFISM] {
        let mtg_compiler::CompileResult::Lowered { definition } =
            mtg_witness::compile(mtg_witness::R5_MANIFEST, raw)
        else {
            panic!("frozen witness failed")
        };
        catalog.definitions.push(*definition);
    }
    let spiral = catalog
        .definitions
        .iter_mut()
        .find(|d| d.name == "Growth Spiral")
        .unwrap();
    let mut value = serde_json::to_value(&*spiral).unwrap();
    fn enchantments(value: &mut serde_json::Value) -> usize {
        match value {
            serde_json::Value::Object(fields) => {
                let here = match fields.get_mut("search") {
                    Some(serde_json::Value::Object(search)) if search.get("from").is_some() => {
                        search.insert(
                            "filter".into(),
                            serde_json::to_value(mtg_contracts::kir::Pred(vec![
                                mtg_contracts::kir::Atom::Type {
                                    kind: CardKind::Enchantment,
                                },
                            ]))
                            .unwrap(),
                        );
                        1
                    }
                    _ => 0,
                };
                here + fields.values_mut().map(enchantments).sum::<usize>()
            }
            serde_json::Value::Array(items) => items.iter_mut().map(enchantments).sum(),
            _ => 0,
        }
    }
    assert_eq!(enchantments(&mut value), 1);
    *spiral = serde_json::from_value(value).unwrap();
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    let prepared = Board::new(catalog, "r5-resolution-moves")
        .step(TurnStep::PrecombatMain)
        .seats(2)
        .seed(SEED)
        .permanent_or_hand("Growth Spiral", 0, 0, true)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Island", 0, 0, false)
        .permanent_or_hand("Pacifism", 0, 0, true)
        .library_card("Island", 0)
        .library_card("Island", 1)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1, 2]);
    let d = card_choice(&mut g, &[3], 0, 1);
    choose(&mut g, &d, &[3]);
    settle(&mut g);
    assert_eq!(zone(&g, 3), HAND);
}

/// CR 608.2d: a random selection never waits on a player. A suspended
/// frame forged as Haunted Fengraf's controller choosing between its two
/// creature cards fails validation.
#[test]
fn a_random_selection_rejects_a_forged_player_choice() {
    let prepared = board(&[(HAUNTED_FENGRAF, &[Cell::RandomGraveyardCards])])
        .permanent_or_hand("Haunted Fengraf", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .card("Grizzly Bears", 0, 0, GRAVEYARD)
        .card("Typhoid Rats", 0, 0, GRAVEYARD)
        .library_card("Island", 0)
        .library_card("Island", 1)
        .prepare();
    let source_block = g_source_block(&prepared);
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let fengraf = Driver::object(&g, 0);
    Driver::action(
        &mut g,
        Action::ActivateAbility {
            object: fengraf,
            ability: 0,
        },
    );
    Driver::pay(&mut g, &[1, 2, 3]);
    for _ in 0..4096 {
        if let Frame::Resolve { frame } = &g.state.rules.frame
            && frame.random_discard.is_none()
            && !g.state.rules.stack.is_empty()
        {
            let resolving = frame.clone();
            g.validate_state().unwrap();
            let original = g.state.rules.frame.clone();
            g.state.rules.frame = Frame::ResolutionChoice {
                frame: resolving,
                choice: ResolutionChoice::Search {
                    player: PlayerId(0),
                    source_block,
                },
            };
            assert!(
                g.validate_state().is_err(),
                "a player chooses a random card"
            );
            g.state.rules.frame = original;
            settle(&mut g);
            assert_eq!(count_in(&g, &[4, 5], HAND), 1);
            return;
        }
        match g.advance(Budget::new(1)) {
            Advance::Suspended { .. } => {}
            Advance::AwaitingDecision(d)
                if d.reason == Reason::Priority && !g.state.rules.stack.is_empty() =>
            {
                g.respond(d.player, d.id, mtg_witness::temporary::pass(&d))
                    .unwrap();
            }
            other => panic!("{other:?}"),
        }
    }
    panic!("the random return never resolved");
}

/// The block of Haunted Fengraf's random return.
fn g_source_block(prepared: &Prepared) -> u32 {
    let fengraf = prepared
        .catalog
        .definitions
        .iter()
        .find(|d| d.name == "Haunted Fengraf")
        .unwrap();
    let [Instruction::Kernel { source_block, .. }] =
        fengraf.activated[0].program.instructions.as_slice()
    else {
        panic!("one random instruction")
    };
    *source_block
}
