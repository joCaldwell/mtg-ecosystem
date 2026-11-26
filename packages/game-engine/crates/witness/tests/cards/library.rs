//! Track 3A waves W4 and W6: library search, shuffle, scry and surveil from
//! exact frozen R4 source, on seeded private boards. Every choice is checked
//! for state validity, an exact snapshot round trip and per-seat privacy;
//! every finished resolution replays from its journal.
//! Pinned CR 401.2, 608.2d, 608.2h, 701.22a-d, 701.23a-e, 701.24a,
//! 701.25a-d, 800.4a, 903.9b:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use mtg_contracts::kir::{Cell, Tiers, definition_cells, definition_tiers};
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

const RAMPANT_GROWTH: &str = record!("8539f295-5d58-4436-a73a-b9277c4c7795");
const DEMONIC_TUTOR: &str = record!("82004860-e589-4e38-8d61-8c0210e4ea39");
const EVOLVING_WILDS: &str = record!("a75445d3-1303-4bb5-89ad-26ea93fecd48");
const TERRAMORPHIC_EXPANSE: &str = record!("1bd3e453-aa21-4ee6-95c2-d6d920ee8e7a");
const FLOODED_STRAND: &str = record!("f3c7af78-a77d-4134-82a2-a5ce84285a84");
const PATH_TO_EXILE: &str = record!("d683d985-9888-4d21-8b5f-69e69ce4a03b");
const OPT: &str = record!("713332c1-5bd8-400f-bfff-c1ca0697a043");
const PREORDAIN: &str = record!("ac641490-ca14-48d7-8cc4-b69ce984befa");
const CONSIDER: &str = record!("4c9bcba6-87b5-4fb3-97ee-6fe5b739337d");
const CULTIVATE: &str = record!("8b755881-a72d-4e21-a369-d2924eb4585a");
const ARMILLARY_SPHERE: &str = record!("3963140c-da67-43e6-9514-fe9dc0a43c4d");
const CRYSTAL_BALL: &str = record!("bd85fe4d-1d62-416f-ac2d-e287911c84e3");
const SINISTER_STARFISH: &str = record!("58321467-41d6-4419-bcb2-cd3e60f677b3");
const TIME_OF_NEED: &str = record!("4f2adfd0-c8ab-4bcc-ae55-cb0e798aec7f");
const FARSEEK: &str = record!("495e52e6-4c2b-4574-9474-eadbdcc8b4ac");
const SKYSHROUD_CLAIM: &str = record!("376c9d3f-21d3-4251-bb6a-026fa9e1b0e1");
const HALLOWED_FOUNTAIN: &str = record!("f1750962-a87c-49f6-b731-02ae971ac6ea");
const TEMPLE_GARDEN: &str = record!("f413a83d-a40d-434c-b20a-4c707c0527fa");
const BREEDING_POOL: &str = record!("20283c4a-f1f0-42f0-bc08-6da87474426b");
const ZUR_THE_ENCHANTER: &str = record!("d7950018-d744-48a8-81aa-0d8384703f48");
const PACIFISM: &str = record!("5f5e0b10-c8cf-450c-bfd3-bcb0528ec330");

const SEED: [u8; 32] = [7; 32];

/// A seeded board whose catalog adds `cards`, each compiled from frozen
/// source to `Executes` only and occupying the named cells.
fn board(cards: &[(&str, &[Cell])]) -> Board {
    Board::new(catalog(cards), "r4-library-witness")
        .step(TurnStep::PrecombatMain)
        .seats(2)
        .seed(SEED)
}

/// The witness catalog with `cards` added; see [`board`].
fn catalog(cards: &[(&str, &[Cell])]) -> Catalog {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    for &(raw, cells) in cards {
        let result = mtg_witness::compile(mtg_witness::R4_MANIFEST, raw);
        let mtg_compiler::CompileResult::Lowered { definition } = result else {
            panic!("frozen library witness failed: {result:?}");
        };
        assert_eq!(definition_tiers(&definition).unwrap(), Tiers::EXECUTES);
        let occupied = definition_cells(&definition).unwrap();
        for cell in cells {
            assert!(
                occupied.contains(cell),
                "{} lacks {cell:?}",
                definition.name
            );
        }
        catalog.definitions.push(*definition);
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog
}

fn identity(name: &str) -> String {
    identity_in(FixtureCatalog::Witness.get(), name)
}

fn identity_in(catalog: &Catalog, name: &str) -> String {
    oracle_id(catalog, name).to_owned()
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

/// CR 401.2, 701.23a: at the next decision, the chooser's observation holds
/// exactly the library cards it looks at (a searcher's whole library);
/// every other seat sees no library card and no decision.
fn private_to(g: &mut Game, chooser: u8, cards: usize) {
    let d = Driver::request(g, u64::MAX);
    assert_eq!(d.player, PlayerId(chooser));
    for seat in 0..2 {
        let o = observation(g, seat);
        let library = o
            .view
            .objects
            .iter()
            .filter(|object| matches!(object.zone, Zone::Library { .. }))
            .count();
        if seat == chooser {
            assert_eq!(library, cards, "chooser sees its choice");
            assert!(o.decision.is_some());
        } else {
            assert_eq!(library, 0, "seat {seat} learns no library card");
            assert!(o.decision.is_none());
        }
    }
}

/// Pass priority until the next decision that is not priority.
fn next_choice(g: &mut Game) -> Decision<Rules> {
    for _ in 0..16 {
        let d = Driver::request(g, 1);
        if d.reason != Reason::Priority {
            return d;
        }
        let pass = d
            .pick_one(&Candidate::Action {
                action: Action::Pass,
            })
            .unwrap();
        g.respond(d.player, d.id, pass).unwrap();
    }
    panic!("no choice within 16 priority passes")
}

/// The pending decision is `seat`'s `reason`; answer it with `objects`.
fn answer(g: &mut Game, seat: u8, reason: Reason<Rules>, objects: &[ObjectId]) -> Decision<Rules> {
    let d = next_choice(g);
    assert_eq!((d.player, &d.reason), (PlayerId(seat), &reason), "{d:?}");
    checked(g);
    let indices = objects
        .iter()
        .map(|object| d.index(&Candidate::Object { object: *object }).unwrap())
        .collect();
    let response = match d.shape {
        Shape::Order { .. } => Answer::Order { order: indices },
        _ => Answer::Pick {
            picks: vec![indices],
        },
    };
    g.respond(d.player, d.id, response).unwrap();
    d
}

fn optional(g: &mut Game, seat: u8, accept: bool) {
    let d = next_choice(g);
    assert_eq!(
        (d.player, &d.reason),
        (PlayerId(seat), &Reason::OptionalEffect)
    );
    checked(g);
    let flag = if accept { Flag::Accept } else { Flag::Decline };
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
        let pass = d
            .pick_one(&Candidate::Action {
                action: Action::Pass,
            })
            .unwrap();
        g.respond(d.player, d.id, pass).unwrap();
    }
    panic!("no empty-stack priority");
}

fn library(g: &Game, seat: u8) -> Vec<CardId> {
    g.state.rules.libraries[&PlayerId(seat)].clone()
}

fn shuffles(g: &Game, seat: u8) -> u64 {
    g.state.rules.match_state.as_ref().unwrap().library_shuffles[usize::from(seat)]
}

fn objects(g: &Game, ids: &[u32]) -> Vec<ObjectId> {
    ids.iter().map(|id| Driver::object(g, *id)).collect()
}

/// Rampant Growth: "Search your library for a basic land card, put that card
/// onto the battlefield tapped, then shuffle." Only basic lands are offered;
/// finding none is allowed and the library is still shuffled (CR 701.23b).
#[test]
fn rampant_growth_offers_basic_lands_and_always_shuffles() {
    for find in [true, false] {
        let prepared = board(&[(
            RAMPANT_GROWTH,
            &[Cell::SearchController, Cell::ShuffleController],
        )])
        .permanent_or_hand("Rampant Growth", 0, 0, true)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .library_card("Grizzly Bears", 0)
        .library_card("Island", 0)
        .library_card("Grizzly Bears", 0)
        .library_card("Plains", 0)
        .library_card("Forest", 1)
        .prepare();
        let mut g = Game::with_trace(prepared, TraceMode::Detailed);
        Driver::cast(&mut g, CardId(0));
        Driver::pay(&mut g, &[1, 2]);
        let lands = objects(&g, &[4, 6]);
        let d = next_choice(&mut g);
        assert_eq!(d.reason, Reason::SearchLibrary);
        let offered: Vec<_> = d
            .candidates
            .iter()
            .map(|c| match c {
                Candidate::Object { object } => *object,
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(offered, lands);
        let Shape::Pick { slots, .. } = &d.shape else {
            panic!("{d:?}")
        };
        assert_eq!((slots[0].min, slots[0].max), (0, 1));
        // CR 701.23a: the searcher looks at every card, not only matches.
        private_to(&mut g, 0, 4);
        // Candidates follow object order and view rows handle order (handles
        // are minted in object order); neither follows library position.
        assert!(offered.windows(2).all(|w| w[0] < w[1]));
        let rows: Vec<_> = observation(&g, 0)
            .view
            .objects
            .into_iter()
            .filter(|o| matches!(o.zone, Zone::Library { .. }))
            .map(|o| o.handle.0)
            .collect();
        assert_eq!(rows.len(), 4);
        assert!(rows.windows(2).all(|w| w[0] < w[1]));
        let island = lands[0];
        answer(
            &mut g,
            0,
            Reason::SearchLibrary,
            &if find { vec![island] } else { vec![] },
        );
        settle(&mut g);
        assert_eq!(shuffles(&g, 0), 1);
        assert_eq!(shuffles(&g, 1), 0);
        let island = g.current(Driver::object(&g, 4)).unwrap().clone();
        if find {
            assert_eq!(island.zone, Zone::Battlefield);
            assert!(island.tapped);
            assert_eq!(island.controller, PlayerId(0));
            assert_eq!(library(&g, 0).len(), 3);
        } else {
            assert_eq!(island.zone, Zone::Library { owner: PlayerId(0) });
            assert_eq!(library(&g, 0).len(), 4);
        }
        // The searched library holds exactly its remaining cards.
        let mut remaining = library(&g, 0);
        remaining.sort();
        let expected: Vec<_> = [3, 4, 5, 6]
            .into_iter()
            .filter(|id| !find || *id != 4)
            .map(CardId)
            .collect();
        assert_eq!(remaining, expected);
        // The card left the choice: nobody keeps a library handle.
        for seat in 0..2 {
            assert!(
                observation(&g, seat)
                    .view
                    .objects
                    .iter()
                    .all(|o| !matches!(o.zone, Zone::Library { .. }))
            );
        }
    }
}

/// Demonic Tutor: "Search your library for a card, put that card into your
/// hand, then shuffle." With no stated quality one card must be found
/// (CR 701.23d); it is not revealed (CR 701.23e).
#[test]
fn demonic_tutor_must_find_one_card_and_keeps_it_hidden() {
    let prepared = board(&[(DEMONIC_TUTOR, &[Cell::SearchController])])
        .permanent_or_hand("Demonic Tutor", 0, 0, true)
        .permanent_or_hand("Swamp", 0, 0, false)
        .permanent_or_hand("Swamp", 0, 0, false)
        .library_card("Grizzly Bears", 0)
        .library_card("Island", 0)
        .library_card("Lightning Bolt", 0)
        .library_card("Forest", 1)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1, 2]);
    let d = next_choice(&mut g);
    assert_eq!(d.reason, Reason::SearchLibrary);
    assert_eq!(d.candidates.len(), 3);
    let Shape::Pick { slots, .. } = &d.shape else {
        panic!("{d:?}")
    };
    assert_eq!((slots[0].min, slots[0].max), (1, 1));
    private_to(&mut g, 0, 3);
    let bolt = Driver::object(&g, 5);
    answer(&mut g, 0, Reason::SearchLibrary, &[bolt]);
    settle(&mut g);
    assert_eq!(
        g.zone(Driver::object(&g, 5)),
        Some(Zone::Hand { owner: PlayerId(0) })
    );
    assert!(observation(&g, 1).view.known_searched_hand.is_empty());
    assert_eq!(shuffles(&g, 0), 1);
}

/// Evolving Wilds, Terramorphic Expanse and Flooded Strand: the paid
/// sacrifice survives the source's departure; the found land enters tapped
/// for the first two and untapped for the Strand, which pays 1 life.
#[test]
fn fetch_lands_find_their_lands_after_sacrificing_themselves() {
    for (name, raw, tapped, life) in [
        ("Evolving Wilds", EVOLVING_WILDS, true, 0),
        ("Terramorphic Expanse", TERRAMORPHIC_EXPANSE, true, 0),
        ("Flooded Strand", FLOODED_STRAND, false, 1),
    ] {
        let prepared = board(&[(raw, &[Cell::SearchController, Cell::ShuffleController])])
            .permanent_or_hand(name, 0, 0, false)
            .library_card("Swamp", 0)
            .library_card("Island", 0)
            .library_card("Mountain", 0)
            .library_card("Forest", 1)
            .prepare();
        let mut g = Game::with_trace(prepared, TraceMode::Detailed);
        let before = g.state.rules.players[0].life.clone();
        let fetch = Driver::object(&g, 0);
        Driver::action(
            &mut g,
            Action::ActivateAbility {
                object: fetch,
                ability: 0,
            },
        );
        let island = Driver::object(&g, 2);
        let d = answer(&mut g, 0, Reason::SearchLibrary, &[island]);
        // The Strand finds a Plains or an Island; the others any basic land.
        assert_eq!(d.candidates.len(), if life == 1 { 1 } else { 3 });
        settle(&mut g);
        assert_eq!(
            g.zone(Driver::object(&g, 0)),
            Some(Zone::Graveyard { owner: PlayerId(0) })
        );
        let island = g.current(Driver::object(&g, 2)).unwrap().clone();
        assert_eq!((island.zone, island.tapped), (Zone::Battlefield, tapped));
        let mut expected = before;
        expected.add(&Quantity::from(-life));
        assert_eq!(g.state.rules.players[0].life, expected);
        assert_eq!(shuffles(&g, 0), 1);
    }
}

/// Path to Exile: "Exile target creature. Its controller may search their
/// library for a basic land card, put that card onto the battlefield
/// tapped, then shuffle." The exiled creature's last-known controller makes
/// the choice and searches their own library (CR 608.2d, 608.2h).
#[test]
fn path_to_exile_offers_the_exiled_creatures_controller_a_basic_land() {
    for accept in [true, false] {
        let prepared = board(&[(
            PATH_TO_EXILE,
            &[
                Cell::Control,
                Cell::SearchTargetController,
                Cell::ShuffleTargetController,
            ],
        )])
        .permanent_or_hand("Path to Exile", 0, 0, true)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 1, 1, false)
        .library_card("Forest", 0)
        .library_card("Grizzly Bears", 1)
        .library_card("Swamp", 1)
        .prepare();
        let mut g = Game::with_trace(prepared, TraceMode::Detailed);
        let target = Driver::object(&g, 2);
        Driver::cast(&mut g, CardId(0));
        Driver::pick_object(&mut g, target);
        Driver::pay(&mut g, &[1]);
        optional(&mut g, 1, accept);
        if accept {
            private_to(&mut g, 1, 2);
            let swamp = Driver::object(&g, 5);
            answer(&mut g, 1, Reason::SearchLibrary, &[swamp]);
        }
        settle(&mut g);
        assert_eq!(g.state.rules.objects[&target.card].zone, Zone::Exile);
        let swamp = g.current(Driver::object(&g, 5)).unwrap().clone();
        if accept {
            assert_eq!(swamp.zone, Zone::Battlefield);
            assert!(swamp.tapped);
            assert_eq!(swamp.controller, PlayerId(1));
        } else {
            assert_eq!(swamp.zone, Zone::Library { owner: PlayerId(1) });
        }
        assert_eq!(shuffles(&g, 1), u64::from(accept));
        assert_eq!(shuffles(&g, 0), 0);
    }
}

/// CR 800.4a: when the exiled creature's controller leaves the game while
/// their choice waits, the choice is withdrawn and nothing is searched.
#[test]
fn path_to_exile_choice_is_withdrawn_when_its_chooser_concedes() {
    let prepared = board(&[(PATH_TO_EXILE, &[Cell::SearchTargetController])])
        .seats(3)
        .permanent_or_hand("Path to Exile", 0, 0, true)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 1, 1, false)
        .library_card("Swamp", 1)
        .library_card("Forest", 2)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let target = Driver::object(&g, 2);
    Driver::cast(&mut g, CardId(0));
    Driver::pick_object(&mut g, target);
    Driver::pay(&mut g, &[1]);
    let d = next_choice(&mut g);
    assert_eq!(
        (d.player, &d.reason),
        (PlayerId(1), &Reason::OptionalEffect)
    );
    g.concede(PlayerId(1)).unwrap();
    settle(&mut g);
    assert!(!g.is_live(PlayerId(1)));
    assert_eq!(shuffles(&g, 1), 0);

    // Conceding during the search itself skips it as well.
    let prepared = board(&[(PATH_TO_EXILE, &[Cell::SearchTargetController])])
        .seats(3)
        .permanent_or_hand("Path to Exile", 0, 0, true)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 1, 1, false)
        .library_card("Swamp", 1)
        .library_card("Forest", 2)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    Driver::cast(&mut g, CardId(0));
    let target = Driver::object(&g, 2);
    Driver::pick_object(&mut g, target);
    Driver::pay(&mut g, &[1]);
    optional(&mut g, 1, true);
    let d = next_choice(&mut g);
    assert_eq!(d.reason, Reason::SearchLibrary);
    g.concede(PlayerId(1)).unwrap();
    settle(&mut g);
    assert_eq!(shuffles(&g, 1), 0);
}

/// Opt: "Scry 1. Draw a card." Bottoming the top card draws the next one.
#[test]
fn opt_scries_one_then_draws() {
    for bottom in [true, false] {
        let prepared = board(&[(OPT, &[Cell::ScryController])])
            .permanent_or_hand("Opt", 0, 0, true)
            .permanent_or_hand("Island", 0, 0, false)
            .library_card("Grizzly Bears", 0)
            .library_card("Island", 0)
            .library_card("Swamp", 0)
            .library_card("Forest", 1)
            .prepare();
        let mut g = Game::with_trace(prepared, TraceMode::Detailed);
        Driver::cast(&mut g, CardId(0));
        Driver::pay(&mut g, &[1]);
        let bears = Driver::object(&g, 2);
        private_to(&mut g, 0, 0);
        let d = answer(
            &mut g,
            0,
            Reason::ScryBottom,
            &if bottom { vec![bears] } else { vec![] },
        );
        assert_eq!(d.candidates.len(), 1);
        settle(&mut g);
        let drawn = if bottom { 3 } else { 2 };
        assert_eq!(
            g.zone(Driver::object(&g, drawn)),
            Some(Zone::Hand { owner: PlayerId(0) })
        );
        let expected = if bottom {
            vec![CardId(4), CardId(2)]
        } else {
            vec![CardId(3), CardId(4)]
        };
        assert_eq!(library(&g, 0), expected);
    }
}

/// Preordain: "Scry 2, then draw a card." Each partition asks only for the
/// orders it needs; the final library is exactly the chosen order.
#[test]
fn preordain_orders_top_and_bottom_cards() {
    let setup = || {
        let prepared = board(&[(PREORDAIN, &[Cell::ScryController])])
            .permanent_or_hand("Preordain", 0, 0, true)
            .permanent_or_hand("Island", 0, 0, false)
            .library_card("Grizzly Bears", 0)
            .library_card("Lightning Bolt", 0)
            .library_card("Swamp", 0)
            .library_card("Forest", 1)
            .prepare();
        let mut g = Game::with_trace(prepared, TraceMode::Detailed);
        Driver::cast(&mut g, CardId(0));
        Driver::pay(&mut g, &[1]);
        g
    };
    // Both stay on top, Bolt first: Bolt is drawn, Bears remains on top.
    let mut g = setup();
    let [bears, bolt] = [Driver::object(&g, 2), Driver::object(&g, 3)];
    let d = answer(&mut g, 0, Reason::ScryBottom, &[]);
    assert_eq!(d.candidates.len(), 2);
    private_to(&mut g, 0, 2);
    answer(&mut g, 0, Reason::OrderLibraryTop, &[bolt, bears]);
    settle(&mut g);
    assert_eq!(
        g.zone(Driver::object(&g, 3)),
        Some(Zone::Hand { owner: PlayerId(0) })
    );
    assert_eq!(library(&g, 0), vec![CardId(2), CardId(4)]);

    // Both to the bottom, Bears above Bolt: the Swamp is drawn.
    let mut g = setup();
    answer(&mut g, 0, Reason::ScryBottom, &[bolt, bears]);
    answer(&mut g, 0, Reason::OrderLibraryBottom, &[bears, bolt]);
    settle(&mut g);
    assert_eq!(
        g.zone(Driver::object(&g, 4)),
        Some(Zone::Hand { owner: PlayerId(0) })
    );
    assert_eq!(library(&g, 0), vec![CardId(2), CardId(3)]);

    // One of each needs no order.
    let mut g = setup();
    answer(&mut g, 0, Reason::ScryBottom, &[bears]);
    settle(&mut g);
    assert_eq!(library(&g, 0), vec![CardId(4), CardId(2)]);
}

/// Consider: "Surveil 1. Draw a card." The surveilled card goes to the
/// graveyard as a public move.
#[test]
fn consider_surveils_into_the_graveyard_then_draws() {
    for graveyard in [true, false] {
        let prepared = board(&[(CONSIDER, &[Cell::SurveilController])])
            .permanent_or_hand("Consider", 0, 0, true)
            .permanent_or_hand("Island", 0, 0, false)
            .library_card("Grizzly Bears", 0)
            .library_card("Swamp", 0)
            .library_card("Forest", 1)
            .prepare();
        let mut g = Game::with_trace(prepared, TraceMode::Detailed);
        Driver::cast(&mut g, CardId(0));
        Driver::pay(&mut g, &[1]);
        let bears = Driver::object(&g, 2);
        answer(
            &mut g,
            0,
            Reason::SurveilGraveyard,
            &if graveyard { vec![bears] } else { vec![] },
        );
        settle(&mut g);
        let bears = g.current(Driver::object(&g, 2)).unwrap().zone;
        if graveyard {
            assert_eq!(bears, Zone::Graveyard { owner: PlayerId(0) });
            assert_eq!(
                g.zone(Driver::object(&g, 3)),
                Some(Zone::Hand { owner: PlayerId(0) })
            );
        } else {
            assert_eq!(bears, Zone::Hand { owner: PlayerId(0) });
        }
    }
}

/// Cultivate splits its found cards between two zones. That split has no
/// executor, so the card rejects explicitly.
#[test]
fn cultivate_split_destination_rejects() {
    let result = mtg_witness::compile(mtg_witness::R4_MANIFEST, CULTIVATE);
    let mtg_compiler::CompileResult::Unsupported { reason, .. } = result else {
        panic!("split search must reject: {result:?}");
    };
    assert_eq!(reason, "verb:search:found_cards");
}

/// The chooser's library objects in their observation, in published order.
fn library_view(g: &Game, seat: u8) -> Vec<String> {
    observation(g, seat)
        .view
        .objects
        .iter()
        .filter(|o| matches!(o.zone, Zone::Library { .. }))
        .map(|o| o.oracle_id.clone())
        .collect()
}

/// CR 701.23a, b: with no matching card the searcher still looks at the
/// whole library and finds nothing; the library is still shuffled.
#[test]
fn rampant_growth_inspects_a_library_without_basic_lands() {
    let prepared = board(&[(
        RAMPANT_GROWTH,
        &[Cell::SearchController, Cell::ShuffleController],
    )])
    .permanent_or_hand("Rampant Growth", 0, 0, true)
    .permanent_or_hand("Forest", 0, 0, false)
    .permanent_or_hand("Forest", 0, 0, false)
    .library_card("Lightning Bolt", 0)
    .library_card("Grizzly Bears", 0)
    .library_card("Forest", 1)
    .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1, 2]);
    let d = next_choice(&mut g);
    assert_eq!((d.player, &d.reason), (PlayerId(0), &Reason::SearchLibrary));
    assert!(d.candidates.is_empty());
    let Shape::Pick { slots, .. } = &d.shape else {
        panic!("{d:?}")
    };
    assert_eq!((slots[0].min, slots[0].max), (0, 0));
    private_to(&mut g, 0, 2);
    // Object order, never library order.
    let mut seen = library_view(&g, 0);
    seen.sort();
    let mut expected = ["Lightning Bolt", "Grizzly Bears"].map(identity).to_vec();
    expected.sort();
    assert_eq!(seen, expected);
    answer(&mut g, 0, Reason::SearchLibrary, &[]);
    settle(&mut g);
    assert_eq!(shuffles(&g, 0), 1);
    assert_eq!(library(&g, 0).len(), 2);
}

/// Demonic Tutor finding its owner's commander: CR 903.9b offers the command
/// zone instead of the hand, then the library is shuffled. Both answers
/// validate, round-trip and replay.
#[test]
fn demonic_tutor_finding_a_commander_offers_the_command_zone() {
    for to_command in [true, false] {
        let prepared = board(&[(DEMONIC_TUTOR, &[Cell::SearchController])])
            .permanent_or_hand("Demonic Tutor", 0, 0, true)
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Swamp", 0, 0, false)
            .library_card("Isamaru, Hound of Konda", 0)
            .library_card("Grizzly Bears", 0)
            .library_card("Forest", 1)
            .commander(CardId(3))
            .prepare();
        let mut g = Game::with_trace(prepared, TraceMode::Detailed);
        Driver::cast(&mut g, CardId(0));
        Driver::pay(&mut g, &[1, 2]);
        let isamaru = Driver::object(&g, 3);
        answer(&mut g, 0, Reason::SearchLibrary, &[isamaru]);
        let d = next_choice(&mut g);
        assert_eq!(d.player, PlayerId(0));
        assert!(
            matches!(&d.reason, Reason::CommanderReturn { destination, .. }
                if *destination == Zone::Hand { owner: PlayerId(0) }),
            "{d:?}"
        );
        checked(&g);
        assert_recoverable((&g).into());
        let flag = if to_command {
            Flag::CommandZone
        } else {
            Flag::Destination
        };
        g.respond(
            d.player,
            d.id,
            d.pick_one(&Candidate::Flag { flag }).unwrap(),
        )
        .unwrap();
        settle(&mut g);
        let zone = g.state.rules.objects[&CardId(3)].zone;
        if to_command {
            assert_eq!(zone, Zone::Command { owner: PlayerId(0) });
        } else {
            assert_eq!(zone, Zone::Hand { owner: PlayerId(0) });
        }
        assert_eq!(library(&g, 0), vec![CardId(4)]);
        assert_eq!(shuffles(&g, 0), 1);
    }
}

const SPHERE: &[(&str, &[Cell])] = &[(
    ARMILLARY_SPHERE,
    &[Cell::SearchController, Cell::ShuffleController],
)];

/// Armillary Sphere: "Search your library for up to two basic land cards,
/// reveal them, put them into your hand, then shuffle."
fn sphere_board() -> Board {
    board(SPHERE)
        .permanent_or_hand("Armillary Sphere", 0, 0, false)
        .permanent_or_hand("Swamp", 0, 0, false)
        .permanent_or_hand("Swamp", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, true)
        .library_card("Forest", 0)
        .library_card("Island", 0)
        .library_card("Grizzly Bears", 0)
        .library_card("Forest", 1)
}

/// Activate the Sphere and find `found` (card ids), resolving to an empty
/// stack with seat 0 holding priority.
fn sphere(found: &[u32]) -> Game {
    let mut g = Game::with_trace(sphere_board().prepare(), TraceMode::Detailed);
    let sphere = Driver::object(&g, 0);
    Driver::action(
        &mut g,
        Action::ActivateAbility {
            object: sphere,
            ability: 0,
        },
    );
    Driver::pay(&mut g, &[1, 2]);
    let found = objects(&g, found);
    answer(&mut g, 0, Reason::SearchLibrary, &found);
    settle(&mut g);
    g
}

/// Seat 1's permitted view and its encoded observation tensors, at seat 1's
/// next priority after seat 0 passes.
fn opponent_tensors(g: &mut Game) -> (String, mtg_observe::Row) {
    Driver::pass(g);
    assert_eq!(Driver::request(g, u64::MAX).player, PlayerId(1));
    let o = observation(g, 1);
    let d = o.decision.as_ref().expect("seat 1 decides");
    let catalog = mtg_observe::catalog::ObservationCatalog::new(catalog(SPHERE).into());
    let mtg_observe::action::Step::Choose(choices) =
        mtg_observe::action::Composer::compose(d, &[], None).unwrap()
    else {
        panic!("seat 1 has a choice")
    };
    let row = mtg_observe::encode(&catalog, &o, 0, &choices, false).unwrap();
    (serde_json::to_string(&o.view).unwrap(), row)
}

/// CR 701.23e: the found cards are revealed in the library, before they
/// move to the hand, and each seat keeps a count of the revealed identities.
#[test]
fn armillary_sphere_reveals_found_lands_before_they_move() {
    let g = sphere(&[4, 5]);
    let events = g.events();
    let revealed = events
        .iter()
        .position(|e| {
            matches!(e, Event::Committed { actual: EventOutcome::FoundRevealed { objects, .. }, .. }
                if objects.iter().all(|o| o.incarnation == 0 && matches!(o.card, CardId(4) | CardId(5))))
        })
        .expect("found lands revealed in the library");
    let moved = events
        .iter()
        .position(|e| {
            matches!(e, Event::Committed { actual: EventOutcome::Moved { movement }, .. }
                if movement.previous.card == CardId(4)
                    && movement.destination == Zone::Hand { owner: PlayerId(0) })
        })
        .expect("found land moved");
    assert!(revealed < moved);
    let mut known = ["Forest", "Island"]
        .map(|n| (PlayerId(0), identity(n)))
        .to_vec();
    known.sort();
    for seat in 0..2 {
        assert_eq!(observation(&g, seat).view.known_searched_hand, known);
    }
    assert_eq!(shuffles(&g, 0), 1);
}

/// CR 400.2, 701.23e: revealed hand knowledge counts identities, not
/// copies. Playing the hidden Forest or the revealed one leaves seat 1 the
/// same permitted view and the same encoded observation, while revealing a
/// different land is distinguishable in both.
#[test]
fn revealed_hand_knowledge_never_identifies_a_copy() {
    let mut played = vec![];
    for land in [3, 4] {
        let mut g = sphere(&[4]);
        let forest = identity("Forest");
        assert_eq!(
            observation(&g, 1).view.known_searched_hand,
            vec![(PlayerId(0), forest)]
        );
        let object = Driver::object(&g, land);
        Driver::action(&mut g, Action::PlayLand { object });
        assert_eq!(Driver::request(&mut g, u64::MAX).player, PlayerId(0));
        assert_eq!(g.state.rules.objects[&object.card].zone, Zone::Battlefield);
        checked(&g);
        assert!(observation(&g, 1).view.known_searched_hand.is_empty());
        played.push(opponent_tensors(&mut g));
    }
    assert_eq!(played[0].0, played[1].0, "permitted views differ");
    assert!(played[0].1 == played[1].1, "observation tensors differ");

    let mut forest = sphere(&[4]);
    let mut island = sphere(&[5]);
    let forest = opponent_tensors(&mut forest);
    let island = opponent_tensors(&mut island);
    assert_ne!(forest.0, island.0);
    assert!(forest.1 != island.1, "revealed identities are encoded");
}

/// CR 701.22d, 701.25d: a positive scry or surveil of an empty library is
/// still that event, with no choice and nothing moved.
#[test]
fn looks_at_an_empty_library_are_still_events() {
    for (name, raw, cell, mana) in [
        ("Crystal Ball", CRYSTAL_BALL, Cell::ScryController, true),
        (
            "Sinister Starfish",
            SINISTER_STARFISH,
            Cell::SurveilController,
            false,
        ),
    ] {
        let prepared = board(&[(raw, &[cell])])
            .permanent_or_hand(name, 0, 0, false)
            .permanent_or_hand("Island", 0, 0, false)
            .library_card("Forest", 1)
            .prepare();
        let mut g = Game::with_trace(prepared, TraceMode::Detailed);
        let source = Driver::object(&g, 0);
        Driver::action(
            &mut g,
            Action::ActivateAbility {
                object: source,
                ability: 0,
            },
        );
        if mana {
            Driver::pay(&mut g, &[1]);
        }
        settle(&mut g);
        let looks = g
            .events()
            .iter()
            .filter(|e| match e {
                Event::Committed {
                    actual:
                        EventOutcome::Scried {
                            player,
                            top,
                            bottom,
                        },
                    ..
                } => *player == PlayerId(0) && top.is_empty() && bottom.is_empty(),
                Event::Committed {
                    actual:
                        EventOutcome::Surveilled {
                            player,
                            top,
                            movements,
                        },
                    ..
                } => *player == PlayerId(0) && top.is_empty() && movements.is_empty(),
                _ => false,
            })
            .count();
        assert_eq!(looks, 1, "{name}");
    }
}

/// `validate_state` rejects a departed chooser's choice of another verb, a
/// saved search table whose unselected candidates changed, and a seeded
/// board without its randomness.
#[test]
fn library_state_rejects_forged_choices_and_lost_randomness() {
    // A departed searcher's choice must still be the search's own variant.
    let prepared = board(&[(PATH_TO_EXILE, &[Cell::SearchTargetController])])
        .seats(3)
        .permanent_or_hand("Path to Exile", 0, 0, true)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 1, 1, false)
        .library_card("Swamp", 1)
        .library_card("Forest", 2)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    Driver::cast(&mut g, CardId(0));
    let target = Driver::object(&g, 2);
    Driver::pick_object(&mut g, target);
    Driver::pay(&mut g, &[1]);
    optional(&mut g, 1, true);
    assert_eq!(next_choice(&mut g).reason, Reason::SearchLibrary);
    g.concede(PlayerId(1)).unwrap();
    let Frame::ResolutionChoice { choice, .. } = &g.state.rules.frame else {
        panic!("{:?}", g.state.rules.frame)
    };
    let ResolutionChoice::Search { source_block, .. } = choice.clone() else {
        panic!("{choice:?}")
    };
    let surveil = ResolutionChoice::Surveil {
        player: PlayerId(1),
        looked: vec![Driver::object(&g, 3)],
        graveyard: None,
        source_block,
    };
    // Queued concession, then after the departure itself.
    forged_choice_rejected(&mut g, &surveil, "queued forged choice variant");
    through_departure(&mut g, 1);
    forged_choice_rejected(&mut g, &surveil, "departed forged choice variant");

    // A saved search answer keeps its whole offered table.
    let prepared = board(&[(RAMPANT_GROWTH, &[Cell::SearchController])])
        .permanent_or_hand("Rampant Growth", 0, 0, true)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .library_card("Island", 0)
        .library_card("Plains", 0)
        .library_card("Forest", 1)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1, 2]);
    let d = next_choice(&mut g);
    let island = Candidate::Object {
        object: Driver::object(&g, 3),
    };
    let picked = d.index(&island).unwrap();
    g.respond(
        d.player,
        d.id,
        Answer::Pick {
            picks: vec![vec![picked]],
        },
    )
    .unwrap();
    g.validate_state().unwrap();
    let pending = g.state.rules.pending.clone();
    let hidden = Driver::object(&g, 5);
    let forged = g.state.rules.pending.as_mut().unwrap();
    let unselected = forged
        .decision
        .candidates
        .iter()
        .position(|c| *c != island)
        .unwrap();
    // Seat 1's hidden library card in place of the unselected Plains.
    forged.decision.candidates[unselected] = Candidate::Object { object: hidden };
    assert!(g.validate_state().is_err(), "forged unselected candidate");
    g.state.rules.pending = pending;
    g.validate_state().unwrap();

    // A seeded board's shuffles need its match randomness.
    g.state.rules.match_state = None;
    assert!(g.validate_state().is_err(), "seed without randomness");
}

/// Advance one rules step at a time until `seat` has left; its library
/// choice is still the suspended frame.
fn through_departure(g: &mut Game, seat: u8) {
    for _ in 0..64 {
        if !g.is_live(PlayerId(seat)) {
            assert!(
                matches!(g.state.rules.frame, Frame::ResolutionChoice { .. }),
                "{:?}",
                g.state.rules.frame
            );
            return;
        }
        let _ = g.advance(Budget::new(1));
    }
    panic!("seat {seat} never departed")
}

/// The current state validates; with `choice` substituted it does not.
fn forged_choice_rejected(g: &mut Game, choice: &ResolutionChoice, what: &str) {
    g.validate_state().unwrap();
    let original = g.state.rules.frame.clone();
    if let Frame::ResolutionChoice {
        choice: current, ..
    } = &mut g.state.rules.frame
    {
        *current = choice.clone();
    }
    assert!(g.validate_state().is_err(), "{what}");
    g.state.rules.frame = original;
    g.validate_state().unwrap();
}

/// CR 701.22a, 800.4a: a departed scrier's saved `looked` cards stay
/// distinct cards of their own library within the printed amount, both
/// while the concession is queued and after the departure.
#[test]
fn departed_scry_rejects_impossible_looked_cards() {
    let prepared = board(&[(OPT, &[Cell::ScryController])])
        .seats(3)
        .permanent_or_hand("Grizzly Bears", 0, 0, false)
        .permanent_or_hand("Opt", 1, 1, true)
        .permanent_or_hand("Island", 1, 1, false)
        .library_card("Swamp", 1)
        .library_card("Island", 1)
        .library_card("Forest", 2)
        .library_card("Forest", 0)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    // Seat 0 passes to seat 1, who casts Opt at instant speed.
    Driver::pass(&mut g);
    Driver::cast(&mut g, CardId(1));
    Driver::pay(&mut g, &[2]);
    let d = next_choice(&mut g);
    assert_eq!((d.player, &d.reason), (PlayerId(1), &Reason::ScryBottom));
    let Frame::ResolutionChoice { choice, .. } = g.state.rules.frame.clone() else {
        panic!()
    };
    let ResolutionChoice::Scry {
        looked,
        source_block,
        ..
    } = choice
    else {
        panic!("{choice:?}")
    };
    let [own, other_own, theirs] = [3, 4, 5].map(|id| Driver::object(&g, id));
    assert_eq!(looked, vec![own]);
    let forged = |looked: Vec<ObjectId>| ResolutionChoice::Scry {
        player: PlayerId(1),
        looked,
        bottom: None,
        top: None,
        source_block,
    };
    let cases = [
        (forged(vec![own, own]), "duplicate looked card"),
        (forged(vec![theirs]), "opponent-owned looked card"),
        (forged(vec![own, other_own]), "more than the printed amount"),
    ];
    g.concede(PlayerId(1)).unwrap();
    for (choice, what) in &cases {
        forged_choice_rejected(&mut g, choice, what);
    }
    through_departure(&mut g, 1);
    for (choice, what) in &cases {
        forged_choice_rejected(&mut g, choice, what);
    }
}

/// CR 400.7, 701.22a: a departed scrier's scry 2 cannot have looked at two
/// incarnations of one physical card, though two is within the amount.
#[test]
fn departed_scry_two_rejects_two_incarnations_of_one_card() {
    let prepared = board(&[(CRYSTAL_BALL, &[Cell::ScryController])])
        .seats(3)
        .permanent_or_hand("Grizzly Bears", 0, 0, false)
        .permanent_or_hand("Crystal Ball", 1, 1, false)
        .permanent_or_hand("Island", 1, 1, false)
        .library_card("Swamp", 1)
        .library_card("Island", 1)
        .library_card("Forest", 2)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    Driver::pass(&mut g);
    let ball = Driver::object(&g, 1);
    Driver::action(
        &mut g,
        Action::ActivateAbility {
            object: ball,
            ability: 0,
        },
    );
    Driver::pay(&mut g, &[2]);
    let d = next_choice(&mut g);
    assert_eq!((d.player, &d.reason), (PlayerId(1), &Reason::ScryBottom));
    let Frame::ResolutionChoice { choice, .. } = g.state.rules.frame.clone() else {
        panic!()
    };
    let ResolutionChoice::Scry {
        looked,
        source_block,
        ..
    } = choice
    else {
        panic!("{choice:?}")
    };
    let swamp = Driver::object(&g, 3);
    assert_eq!(looked, vec![swamp, Driver::object(&g, 4)]);
    let later = ObjectId {
        face: Default::default(),
        card: swamp.card,
        incarnation: swamp.incarnation + 1,
    };
    let forged = ResolutionChoice::Scry {
        player: PlayerId(1),
        looked: vec![swamp, later],
        bottom: None,
        top: None,
        source_block,
    };
    g.concede(PlayerId(1)).unwrap();
    forged_choice_rejected(&mut g, &forged, "queued: two incarnations of one card");
    through_departure(&mut g, 1);
    forged_choice_rejected(&mut g, &forged, "departed: two incarnations of one card");
}

/// The Sphere board with two Forests on top of the library.
fn two_forest_sphere() -> Game {
    let prepared = board(SPHERE)
        .permanent_or_hand("Armillary Sphere", 0, 0, false)
        .permanent_or_hand("Swamp", 0, 0, false)
        .permanent_or_hand("Swamp", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, true)
        .library_card("Forest", 0)
        .library_card("Forest", 0)
        .library_card("Island", 0)
        .library_card("Forest", 1)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let sphere = Driver::object(&g, 0);
    Driver::action(
        &mut g,
        Action::ActivateAbility {
            object: sphere,
            ability: 0,
        },
    );
    Driver::pay(&mut g, &[1, 2]);
    let found = objects(&g, &[4, 5]);
    answer(&mut g, 0, Reason::SearchLibrary, &found);
    settle(&mut g);
    g
}

fn known_forests(g: &Game) -> usize {
    let forest = identity("Forest");
    g.publicly_searched_hand(PlayerId(1))
        .unwrap()
        .iter()
        .filter(|(owner, id)| *owner == PlayerId(0) && *id == forest)
        .count()
}

/// CR 400.7, 701.23e: a discard is one departure from the hand, whether
/// applied alone or inside a batch, so it consumes exactly one known copy.
#[test]
fn one_discard_consumes_one_known_copy() {
    for batch in [false, true] {
        let mut g = two_forest_sphere();
        assert_eq!(known_forests(&g), 2);
        let discard = ProposedEvent::DiscardCard {
            object: Driver::object(&g, 4),
            player: PlayerId(0),
        };
        if batch {
            g.commit_batch(
                BatchCause::Cleanup,
                EventGroup::Simultaneous {
                    occurrences: vec![discard],
                },
            )
            .unwrap();
        } else {
            g.apply_occurrence(&discard).unwrap();
        }
        assert_eq!(known_forests(&g), 1, "batch {batch}");
        g.validate_state().unwrap();
    }
}

/// Time of Need: "Search your library for a legendary creature card, reveal
/// it, put it into your hand, then shuffle." Seat 0 also holds a hidden
/// Isamaru.
fn time_of_need(commander: bool, seats: u8) -> Game {
    let mut board = board(&[(
        TIME_OF_NEED,
        &[Cell::SearchController, Cell::ShuffleController],
    )])
    .seats(seats)
    .permanent_or_hand("Time of Need", 0, 0, true)
    .permanent_or_hand("Forest", 0, 0, false)
    .permanent_or_hand("Forest", 0, 0, false)
    .permanent_or_hand("Isamaru, Hound of Konda", 0, 0, true)
    .library_card("Isamaru, Hound of Konda", 0)
    .library_card("Grizzly Bears", 0)
    .library_card("Forest", 1);
    if commander {
        board = board.commander(CardId(4));
    }
    let mut g = Game::with_trace(board.prepare(), TraceMode::Detailed);
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1, 2]);
    let found = Driver::object(&g, 4);
    answer(&mut g, 0, Reason::SearchLibrary, &[found]);
    g
}

fn revealed_in_library(g: &Game, card: u32) -> bool {
    g.events().iter().any(|e| {
        matches!(e, Event::Committed { actual: EventOutcome::FoundRevealed { objects, .. }, .. }
            if objects.iter().any(|o| o.card == CardId(card) && o.incarnation == 0))
    })
}

/// CR 601.2i, 701.23e: an unpaid Isamaru returns to the hand it was cast
/// from, restoring the known copy its announcement consumed; casting the
/// hidden or the revealed copy leaves seat 1 the same view.
#[test]
fn reversed_cast_restores_a_known_hand_copy() {
    let mut views = vec![];
    for card in [3, 4] {
        let mut g = time_of_need(false, 2);
        settle(&mut g);
        let isamaru = (PlayerId(0), identity("Isamaru, Hound of Konda"));
        assert_eq!(
            observation(&g, 1).view.known_searched_hand,
            vec![isamaru.clone()]
        );
        Driver::cast(&mut g, CardId(card));
        let d = Driver::request(&mut g, u64::MAX);
        assert!(observation(&g, 1).view.known_searched_hand.is_empty());
        g.respond(
            d.player,
            d.id,
            Answer::Pay {
                pay: PayAnswer::Finish {
                    undo_on_failure: true,
                },
            },
        )
        .unwrap();
        assert_eq!(Driver::request(&mut g, u64::MAX).reason, Reason::Priority);
        assert_eq!(
            g.zone(Driver::object(&g, card)),
            Some(Zone::Hand { owner: PlayerId(0) })
        );
        assert_eq!(observation(&g, 1).view.known_searched_hand, vec![isamaru]);
        checked(&g);
        views.push(serde_json::to_string(&observation(&g, 1).view).unwrap());
    }
    assert_eq!(views[0], views[1]);
}

/// CR 701.23e, 903.9b: Time of Need reveals the found commander while it is
/// still in the library, before its owner chooses a zone; conceding at
/// that prompt cannot skip the reveal.
#[test]
fn time_of_need_reveals_a_commander_before_its_zone_choice() {
    for concede in [false, true] {
        let mut g = time_of_need(true, 3);
        let d = next_choice(&mut g);
        assert!(matches!(d.reason, Reason::CommanderReturn { .. }), "{d:?}");
        assert!(revealed_in_library(&g, 4));
        checked(&g);
        if concede {
            g.concede(PlayerId(0)).unwrap();
            for _ in 0..64 {
                if let Advance::AwaitingDecision(_) = g.advance(Budget::new(1)) {
                    break;
                }
            }
            assert!(!g.is_live(PlayerId(0)));
            g.validate_state().unwrap();
        } else {
            g.respond(
                d.player,
                d.id,
                d.pick_one(&Candidate::Flag {
                    flag: Flag::Destination,
                })
                .unwrap(),
            )
            .unwrap();
            settle(&mut g);
            assert_eq!(
                g.state.rules.objects[&CardId(4)].zone,
                Zone::Hand { owner: PlayerId(0) }
            );
        }
        let reveals = g
            .events()
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Event::Committed {
                        actual: EventOutcome::FoundRevealed { .. },
                        ..
                    }
                )
            })
            .count();
        assert_eq!(reveals, 1);
    }
}

/// CR 701.23e: a printed reveal of cards bound for the battlefield happens
/// in the library too; nothing stays counted in a hand. Rampant Growth's IR
/// is edited to reveal (no frozen card prints this; the cell admits it).
#[test]
fn a_revealed_search_onto_the_battlefield_reveals_in_the_library() {
    let mut catalog = catalog(&[(
        RAMPANT_GROWTH,
        &[Cell::SearchController, Cell::ShuffleController],
    )]);
    let growth = identity_in(&catalog, "Rampant Growth");
    let definition = catalog
        .definitions
        .iter_mut()
        .find(|d| d.oracle_id == growth)
        .unwrap();
    let mut value = serde_json::to_value(&*definition).unwrap();
    fn reveal(value: &mut serde_json::Value) -> usize {
        match value {
            serde_json::Value::Object(fields) => {
                let here = match fields.get_mut("search") {
                    Some(serde_json::Value::Object(search)) if search.contains_key("to") => {
                        search.insert("reveal".into(), true.into());
                        1
                    }
                    _ => 0,
                };
                here + fields.values_mut().map(reveal).sum::<usize>()
            }
            serde_json::Value::Array(items) => items.iter_mut().map(reveal).sum(),
            _ => 0,
        }
    }
    assert_eq!(reveal(&mut value), 1);
    *definition = serde_json::from_value(value).unwrap();
    let prepared = Board::new(catalog, "r4-library-witness")
        .step(TurnStep::PrecombatMain)
        .seats(2)
        .seed(SEED)
        .permanent_or_hand("Rampant Growth", 0, 0, true)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .library_card("Island", 0)
        .library_card("Forest", 1)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1, 2]);
    let island = Driver::object(&g, 3);
    answer(&mut g, 0, Reason::SearchLibrary, &[island]);
    settle(&mut g);
    assert!(revealed_in_library(&g, 3));
    let events = g.events();
    let reveal = events
        .iter()
        .position(|e| {
            matches!(
                e,
                Event::Committed {
                    actual: EventOutcome::FoundRevealed { .. },
                    ..
                }
            )
        })
        .unwrap();
    let entry = events
        .iter()
        .position(|e| {
            matches!(e, Event::Committed { actual: EventOutcome::Moved { movement }, .. }
            if movement.previous == island && movement.destination == Zone::Battlefield)
        })
        .unwrap();
    assert!(reveal < entry);
    assert!(observation(&g, 1).view.known_searched_hand.is_empty());
}

/// CR 903.9b: once the owner answered the found commander's zone choice,
/// the saved flag rows cannot be swapped to change what that answer means.
#[test]
fn search_commander_choice_keeps_its_saved_flag_rows() {
    let mut g = time_of_need(true, 2);
    let d = next_choice(&mut g);
    g.respond(
        d.player,
        d.id,
        d.pick_one(&Candidate::Flag {
            flag: Flag::Destination,
        })
        .unwrap(),
    )
    .unwrap();
    g.validate_state().unwrap();
    let pending = g.state.rules.pending.clone();
    g.state
        .rules
        .pending
        .as_mut()
        .unwrap()
        .decision
        .candidates
        .swap(0, 1);
    assert!(g.validate_state().is_err(), "swapped commander flag rows");
    g.state.rules.pending = pending;
    g.validate_state().unwrap();
}

fn life(g: &Game, seat: u8) -> Quantity {
    g.state.rules.players[usize::from(seat)].life.clone()
}

fn minus(mut life: Quantity, amount: i64) -> Quantity {
    life.add(&Quantity::from(-amount));
    life
}

/// Answer seat 0's pending entry life payment (CR 614.1c), checking the
/// suspended state first.
fn entry_payment(g: &mut Game, pay: bool) {
    let d = next_choice(g);
    assert_eq!(
        (d.player, &d.reason),
        (PlayerId(0), &Reason::OptionalEffect),
        "{d:?}"
    );
    let Shape::Pick { slots, .. } = &d.shape else {
        panic!("{d:?}")
    };
    assert!(matches!(
        slots[0].subject.map(|i| &d.candidates[i as usize]),
        Some(Candidate::Object { .. })
    ));
    checked(g);
    assert_recoverable((&*g).into());
    let flag = if pay { Flag::Accept } else { Flag::Decline };
    g.respond(
        d.player,
        d.id,
        d.pick_one(&Candidate::Flag { flag }).unwrap(),
    )
    .unwrap();
}

/// Farseek: "Search your library for a Plains, Island, Swamp, or Mountain
/// card, put it onto the battlefield tapped, then shuffle." CR 614.1c,
/// 614.12 and the shockland ruling ("If an effect puts this land onto the
/// battlefield tapped, you may pay 2 life, but it still enters tapped.",
/// Lorwyn Eclipsed release notes): Hallowed Fountain still offers its
/// payment and enters tapped either way.
#[test]
fn farseek_offers_a_shockland_payment_and_enters_it_tapped_either_way() {
    for pay in [true, false] {
        let prepared = board(&[
            (FARSEEK, &[Cell::SearchController, Cell::ShuffleController]),
            (HALLOWED_FOUNTAIN, &[]),
        ])
        .permanent_or_hand("Farseek", 0, 0, true)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .library_card("Hallowed Fountain", 0)
        .library_card("Forest", 1)
        .prepare();
        let mut g = Game::with_trace(prepared, TraceMode::Detailed);
        let before = life(&g, 0);
        Driver::cast(&mut g, CardId(0));
        Driver::pay(&mut g, &[1, 2]);
        let fountain = Driver::object(&g, 3);
        answer(&mut g, 0, Reason::SearchLibrary, &[fountain]);
        entry_payment(&mut g, pay);
        settle(&mut g);
        let fountain = g.current(Driver::object(&g, 3)).unwrap().clone();
        assert_eq!((fountain.zone, fountain.tapped), (Zone::Battlefield, true));
        assert_eq!(life(&g, 0), minus(before, if pay { 2 } else { 0 }));
        assert_eq!(shuffles(&g, 0), 1);
    }
}

/// Flooded Strand finding Hallowed Fountain onto the battlefield untapped:
/// CR 614.1c offers 2 life; paying enters untapped, declining tapped, and
/// the search still shuffles.
#[test]
fn flooded_strand_finding_a_shockland_offers_its_life_payment() {
    for pay in [true, false] {
        let prepared = board(&[
            (
                FLOODED_STRAND,
                &[Cell::SearchController, Cell::ShuffleController],
            ),
            (HALLOWED_FOUNTAIN, &[]),
        ])
        .permanent_or_hand("Flooded Strand", 0, 0, false)
        .library_card("Hallowed Fountain", 0)
        .library_card("Island", 0)
        .library_card("Forest", 1)
        .prepare();
        let mut g = Game::with_trace(prepared, TraceMode::Detailed);
        let before = life(&g, 0);
        let strand = Driver::object(&g, 0);
        Driver::action(
            &mut g,
            Action::ActivateAbility {
                object: strand,
                ability: 0,
            },
        );
        let fountain = Driver::object(&g, 1);
        answer(&mut g, 0, Reason::SearchLibrary, &[fountain]);
        // The searcher still sees the found card while deciding its entry.
        assert!(library_view(&g, 0).contains(&identity_in(
            &catalog(&[(HALLOWED_FOUNTAIN, &[])]),
            "Hallowed Fountain"
        )));
        assert!(library_view(&g, 1).is_empty());
        entry_payment(&mut g, pay);
        settle(&mut g);
        let fountain = g.current(Driver::object(&g, 1)).unwrap().clone();
        assert_eq!((fountain.zone, fountain.tapped), (Zone::Battlefield, !pay));
        assert_eq!(life(&g, 0), minus(before, if pay { 3 } else { 1 }));
        assert_eq!(shuffles(&g, 0), 1);
    }
}

/// Skyshroud Claim finding Temple Garden and Breeding Pool: CR 614.12 the
/// two enter simultaneously; each payment is asked in object order, and a
/// payment the remaining life no longer covers is not offered.
#[test]
fn skyshroud_claim_collects_each_shockland_payment_before_one_entry() {
    for (starting, answers, untapped) in [
        (40, &[true, true][..], [true, true]),
        (40, &[true, false][..], [true, false]),
        (40, &[false, true][..], [false, true]),
        // 3 life covers one payment; the second is never asked.
        (3, &[true][..], [true, false]),
    ] {
        let prepared = board(&[
            (
                SKYSHROUD_CLAIM,
                &[Cell::SearchController, Cell::ShuffleController],
            ),
            (TEMPLE_GARDEN, &[]),
            (BREEDING_POOL, &[]),
        ])
        .life(PlayerId(0), Quantity::from(starting))
        .permanent_or_hand("Skyshroud Claim", 0, 0, true)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .library_card("Temple Garden", 0)
        .library_card("Breeding Pool", 0)
        .library_card("Grizzly Bears", 0)
        .library_card("Forest", 1)
        .prepare();
        let mut g = Game::with_trace(prepared, TraceMode::Detailed);
        Driver::cast(&mut g, CardId(0));
        Driver::pay(&mut g, &[1, 2, 3, 4]);
        let found = objects(&g, &[5, 6]);
        answer(&mut g, 0, Reason::SearchLibrary, &found);
        for pay in answers {
            entry_payment(&mut g, *pay);
        }
        settle(&mut g);
        let paid = answers.iter().filter(|p| **p).count() as i64;
        assert_eq!(life(&g, 0), minus(Quantity::from(starting), 2 * paid));
        let mut batches = vec![];
        for (card, untapped) in [5, 6].into_iter().zip(untapped) {
            let land = g.current(Driver::object(&g, card)).unwrap().clone();
            assert_eq!((land.zone, land.tapped), (Zone::Battlefield, !untapped));
            batches.push(
                g.events()
                    .iter()
                    .position(|e| matches!(e, Event::Committed { actual: EventOutcome::Moved { movement }, .. }
                        if movement.previous.card == CardId(card) && movement.destination == Zone::Battlefield))
                    .unwrap(),
            );
        }
        // One entry batch: no event separates the two entries.
        assert_eq!(batches[1].abs_diff(batches[0]), 1);
        assert_eq!(shuffles(&g, 0), 1);
    }
}

/// Skyshroud Claim finding Temple Garden and Breeding Pool, the latter given
/// Gilt-Leaf Palace's entry reveal of an Elf card in place of its payment
/// (no frozen Forest land prints a reveal): CR 614.12, 701.20a. The
/// payment is asked first, then the reveal, which offers only the matching
/// hand card; both enter in one batch, and a reveal publishes the card.
#[test]
fn skyshroud_claim_asks_each_entry_reveal_after_the_payments() {
    use mtg_contracts::kir::{Atom, Pred, StaticEffect};
    for (hand, pay, reveal) in [
        ("Llanowar Elves", true, Some(true)),
        ("Llanowar Elves", false, Some(false)),
        ("Island", true, None),
    ] {
        let mut catalog = catalog(&[
            (
                SKYSHROUD_CLAIM,
                &[Cell::SearchController, Cell::ShuffleController],
            ),
            (TEMPLE_GARDEN, &[]),
            (BREEDING_POOL, &[]),
        ]);
        let pool = catalog
            .definitions
            .iter_mut()
            .find(|d| d.name == "Breeding Pool")
            .unwrap();
        for ability in &mut pool.static_abilities {
            if matches!(ability.effect, StaticEffect::PayLifeOrEnterTapped { .. }) {
                ability.effect = StaticEffect::RevealOrEnterTapped {
                    filter: Pred(vec![Atom::Subtype {
                        subtype: "Elf".into(),
                    }]),
                };
            }
        }
        pool.source_labels = None;
        pool.capabilities = pool.requirements();
        let elves = identity_in(&catalog, hand);
        let prepared = Board::new(catalog, "r4-library-witness")
            .step(TurnStep::PrecombatMain)
            .seats(2)
            .seed(SEED)
            .permanent_or_hand("Skyshroud Claim", 0, 0, true)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand(hand, 0, 0, true)
            .library_card("Temple Garden", 0)
            .library_card("Breeding Pool", 0)
            .library_card("Forest", 1)
            .prepare();
        let mut g = Game::with_trace(prepared, TraceMode::Detailed);
        Driver::cast(&mut g, CardId(0));
        Driver::pay(&mut g, &[1, 2, 3, 4]);
        let [garden, pool] = [6, 7].map(|id| Driver::object(&g, id));
        let card = Driver::object(&g, 5);
        answer(&mut g, 0, Reason::SearchLibrary, &[garden, pool]);
        entry_payment(&mut g, pay);
        if let Some(reveal) = reveal {
            let d = next_choice(&mut g);
            assert_eq!(d.reason, Reason::OptionalEffect);
            assert_eq!(offered_entrant(&d), pool);
            assert_eq!(
                d.candidates,
                vec![
                    Candidate::Flag {
                        flag: Flag::Decline
                    },
                    Candidate::Object { object: pool },
                    Candidate::Object { object: card },
                ]
            );
            checked(&g);
            assert_recoverable((&g).into());
            // A forged reveal of a found card, or out of order, rejects.
            let Frame::ResolutionChoice {
                choice: ResolutionChoice::SearchEntry { source_block, .. },
                ..
            } = g.state.rules.frame.clone()
            else {
                panic!()
            };
            for (revealed, what) in [
                (vec![(pool, Some(garden))], "a found card revealed"),
                (vec![(garden, Some(card))], "a payment land revealed"),
            ] {
                let forged = ResolutionChoice::SearchEntry {
                    player: PlayerId(0),
                    found: vec![garden, pool],
                    hosts: vec![],
                    paid: vec![(garden, pay)],
                    revealed,
                    source_block,
                };
                forged_choice_rejected(&mut g, &forged, what);
            }
            let pick = if reveal {
                Candidate::Object { object: card }
            } else {
                Candidate::Flag {
                    flag: Flag::Decline,
                }
            };
            g.respond(d.player, d.id, d.pick_one(&pick).unwrap())
                .unwrap();
        }
        settle(&mut g);
        let revealed = reveal == Some(true);
        for (id, untapped) in [(6, pay), (7, revealed)] {
            let land = g.current(Driver::object(&g, id)).unwrap().clone();
            assert_eq!((land.zone, land.tapped), (Zone::Battlefield, !untapped));
        }
        let known = observation(&g, 1).view.known_searched_hand;
        let expected: Vec<_> = revealed
            .then(|| (PlayerId(0), elves.clone()))
            .into_iter()
            .collect();
        assert_eq!(known, expected);
        assert_eq!(shuffles(&g, 0), 1);
    }
}

/// A suspended entry payment keeps its found set and asked prefix.
#[test]
fn search_entry_state_rejects_forged_entrants_and_answers() {
    let prepared = board(&[
        (
            SKYSHROUD_CLAIM,
            &[Cell::SearchController, Cell::ShuffleController],
        ),
        (TEMPLE_GARDEN, &[]),
        (BREEDING_POOL, &[]),
    ])
    .permanent_or_hand("Skyshroud Claim", 0, 0, true)
    .permanent_or_hand("Forest", 0, 0, false)
    .permanent_or_hand("Forest", 0, 0, false)
    .permanent_or_hand("Forest", 0, 0, false)
    .permanent_or_hand("Forest", 0, 0, false)
    .library_card("Temple Garden", 0)
    .library_card("Breeding Pool", 0)
    .library_card("Forest", 1)
    .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1, 2, 3, 4]);
    let [garden, pool, theirs] = [5, 6, 7].map(|id| Driver::object(&g, id));
    answer(&mut g, 0, Reason::SearchLibrary, &[garden, pool]);
    assert_eq!(next_choice(&mut g).reason, Reason::OptionalEffect);
    let Frame::ResolutionChoice { choice, .. } = g.state.rules.frame.clone() else {
        panic!()
    };
    let ResolutionChoice::SearchEntry { source_block, .. } = choice else {
        panic!("{choice:?}")
    };
    let forged =
        |found: Vec<ObjectId>, paid: Vec<(ObjectId, bool)>| ResolutionChoice::SearchEntry {
            player: PlayerId(0),
            found,
            hosts: vec![],
            paid,
            revealed: vec![],
            source_block,
        };
    for (choice, what) in [
        (
            forged(vec![garden, theirs], vec![]),
            "opponent's card found",
        ),
        (forged(vec![garden, garden], vec![]), "a card found twice"),
        (
            forged(vec![garden, pool], vec![(pool, true)]),
            "answered out of order",
        ),
        (
            forged(vec![garden, pool], vec![(garden, true), (pool, false)]),
            "nothing left to ask",
        ),
    ] {
        forged_choice_rejected(&mut g, &choice, what);
    }
}

fn claim(starting: i64, seats: u8) -> (Game, [ObjectId; 2]) {
    let prepared = board(&[
        (
            SKYSHROUD_CLAIM,
            &[Cell::SearchController, Cell::ShuffleController],
        ),
        (TEMPLE_GARDEN, &[]),
        (BREEDING_POOL, &[]),
    ])
    .seats(seats)
    .life(PlayerId(0), Quantity::from(starting))
    .permanent_or_hand("Skyshroud Claim", 0, 0, true)
    .permanent_or_hand("Forest", 0, 0, false)
    .permanent_or_hand("Forest", 0, 0, false)
    .permanent_or_hand("Forest", 0, 0, false)
    .permanent_or_hand("Forest", 0, 0, false)
    .library_card("Temple Garden", 0)
    .library_card("Breeding Pool", 0)
    .library_card("Forest", 1)
    .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1, 2, 3, 4]);
    let found = [5, 6].map(|id| Driver::object(&g, id));
    (g, found)
}

fn offered_entrant(d: &Decision<Rules>) -> ObjectId {
    let Shape::Pick { slots, .. } = &d.shape else {
        panic!("{d:?}")
    };
    match slots[0].subject.map(|i| &d.candidates[i as usize]) {
        Some(Candidate::Object { object }) => *object,
        other => panic!("{other:?}"),
    }
}

/// The found set is saved in object order whatever order the searcher
/// picked it in, so at 3 life the lower object is the one offered.
#[test]
fn search_entry_found_cards_are_in_object_order() {
    let (mut g, [garden, pool]) = claim(3, 2);
    answer(&mut g, 0, Reason::SearchLibrary, &[pool, garden]);
    let d = next_choice(&mut g);
    assert_eq!(d.reason, Reason::OptionalEffect);
    assert_eq!(offered_entrant(&d), garden);
    let Frame::ResolutionChoice {
        choice: ResolutionChoice::SearchEntry { found, .. },
        ..
    } = &g.state.rules.frame
    else {
        panic!()
    };
    assert_eq!(found, &vec![garden, pool]);
}

/// The pending payment binds to its entrant: replacing the found set under
/// an accepted answer changes the offer, which the saved table rejects.
#[test]
fn a_pending_entry_payment_binds_to_its_entrant() {
    let (mut g, [garden, pool]) = claim(40, 2);
    answer(&mut g, 0, Reason::SearchLibrary, &[garden, pool]);
    let d = next_choice(&mut g);
    assert_eq!(offered_entrant(&d), garden);
    g.respond(
        d.player,
        d.id,
        d.pick_one(&Candidate::Flag { flag: Flag::Accept }).unwrap(),
    )
    .unwrap();
    g.validate_state().unwrap();
    let original = g.state.rules.frame.clone();
    if let Frame::ResolutionChoice {
        choice: ResolutionChoice::SearchEntry { found, .. },
        ..
    } = &mut g.state.rules.frame
    {
        *found = vec![pool];
    }
    assert!(
        g.validate_state().is_err(),
        "payment moved to another entrant"
    );
    g.state.rules.frame = original;
    g.validate_state().unwrap();
}

/// CR 800.4a: after the searcher departs mid-entry, its saved payments must
/// still answer the found entrants in order; skipping a payable entrant is
/// rejected both while the concession is queued and after it.
#[test]
fn departed_search_entry_rejects_skipped_payments() {
    let (mut g, [garden, pool]) = claim(40, 3);
    answer(&mut g, 0, Reason::SearchLibrary, &[garden, pool]);
    assert_eq!(next_choice(&mut g).reason, Reason::OptionalEffect);
    let Frame::ResolutionChoice {
        choice: ResolutionChoice::SearchEntry { source_block, .. },
        ..
    } = g.state.rules.frame.clone()
    else {
        panic!()
    };
    let forged = ResolutionChoice::SearchEntry {
        player: PlayerId(0),
        found: vec![garden, pool],
        hosts: vec![],
        paid: vec![(pool, true)],
        revealed: vec![],
        source_block,
    };
    let unsorted = ResolutionChoice::SearchEntry {
        player: PlayerId(0),
        found: vec![pool, garden],
        hosts: vec![],
        paid: vec![],
        revealed: vec![],
        source_block,
    };
    g.concede(PlayerId(0)).unwrap();
    forged_choice_rejected(&mut g, &forged, "queued: skipped payable entrant");
    forged_choice_rejected(&mut g, &unsorted, "queued: unsorted found set");
    through_departure(&mut g, 0);
    forged_choice_rejected(&mut g, &forged, "departed: skipped payable entrant");
    forged_choice_rejected(&mut g, &unsorted, "departed: unsorted found set");
}

/// Zur the Enchanter attacks; seat 1 controls Grizzly Bears (card 8)
/// unless it answers with Murder on Zur instead. Returns the game at the
/// trigger's search.
fn zur(murder: bool) -> Game {
    zur_with(2, murder, (!murder).then_some(1))
}

/// Zur in a `seats`-player game; Grizzly Bears (card 8) under `bears`.
fn zur_with(seats: u8, murder: bool, bears: Option<u8>) -> Game {
    let mut board = board(&[
        (
            ZUR_THE_ENCHANTER,
            &[Cell::SearchController, Cell::ShuffleController],
        ),
        (PACIFISM, &[]),
    ])
    .permanent_or_hand("Zur the Enchanter", 0, 0, false)
    .permanent_or_hand("Murder", 1, 1, true)
    .permanent_or_hand("Swamp", 1, 1, false)
    .permanent_or_hand("Swamp", 1, 1, false)
    .permanent_or_hand("Swamp", 1, 1, false)
    .library_card("Pacifism", 0)
    .library_card("Grizzly Bears", 0)
    .library_card("Forest", 1)
    .seats(seats);
    if let Some(seat) = bears {
        board = board.permanent_or_hand("Grizzly Bears", seat, seat, false);
    }
    let mut g = Game::with_trace(board.prepare(), TraceMode::Detailed);
    let zur = Driver::object(&g, 0);
    let mut responded = !murder;
    for _ in 0..64 {
        let r = Driver::request(&mut g, u64::MAX);
        let answer = match &r.reason {
            Reason::Priority
                if !responded && r.player == PlayerId(1) && !g.state.rules.stack.is_empty() =>
            {
                responded = true;
                Driver::cast(&mut g, CardId(1));
                Driver::pick_object(&mut g, zur);
                Driver::pay(&mut g, &[2, 3, 4]);
                continue;
            }
            Reason::Priority => mtg_witness::temporary::pass(&r),
            Reason::Attackers => r.attack(vec![(zur, PlayerId(1))]),
            Reason::OptionalEffect if r.player == PlayerId(0) => {
                r.pick_one(&Candidate::Flag { flag: Flag::Accept }).unwrap()
            }
            Reason::SearchLibrary => {
                assert_eq!(g.zone(zur) == Some(Zone::Battlefield), !murder);
                return g;
            }
            other => panic!("unexpected decision {other:?}"),
        };
        g.respond(r.player, r.id, answer).unwrap();
    }
    panic!("Zur's trigger never searched")
}

/// Pass to the postcombat main phase, declaring no blockers.
fn finish_combat(g: &mut Game) {
    for _ in 0..64 {
        let r = Driver::request(g, u64::MAX);
        if g.state.rules.turn.step == TurnStep::PostcombatMain {
            return;
        }
        let answer = match &r.reason {
            Reason::Priority => mtg_witness::temporary::pass(&r),
            Reason::Blockers => r.block(vec![]),
            other => panic!("unexpected decision {other:?}"),
        };
        g.respond(r.player, r.id, answer).unwrap();
    }
    panic!("combat never ended")
}

/// "Whenever Zur attacks, you may search your library for an enchantment
/// card with mana value 3 or less, put it onto the battlefield, then
/// shuffle." CR 303.4f: Pacifism, put onto the battlefield without being
/// cast, enchants a creature its controller chooses (not a target).
#[test]
fn zur_puts_pacifism_onto_a_chosen_creature() {
    let mut g = zur(false);
    let [zur, pacifism, bears] = [0, 5, 8].map(|id| Driver::object(&g, id));
    answer(&mut g, 0, Reason::SearchLibrary, &[pacifism]);
    let d = next_choice(&mut g);
    assert_eq!(
        (d.player, &d.reason),
        (PlayerId(0), &Reason::EntryAttachment)
    );
    let Shape::Pick { slots, .. } = &d.shape else {
        panic!("{d:?}")
    };
    assert_eq!(
        slots[0].subject.map(|i| d.candidates[i as usize].clone()),
        Some(Candidate::Object { object: pacifism })
    );
    let hosts: std::collections::BTreeSet<_> = slots[0]
        .allowed
        .iter()
        .map(|i| match &d.candidates[*i as usize] {
            Candidate::Object { object } => *object,
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(
        hosts,
        std::collections::BTreeSet::from([zur, bears]),
        "both creatures"
    );
    assert!(library_view(&g, 1).is_empty());
    checked(&g);
    assert_recoverable((&g).into());
    g.respond(
        d.player,
        d.id,
        d.pick_one(&Candidate::Object { object: bears }).unwrap(),
    )
    .unwrap();
    finish_combat(&mut g);
    let pacifism = g.current(Driver::object(&g, 5)).unwrap().clone();
    assert_eq!(pacifism.zone, Zone::Battlefield);
    assert_eq!(pacifism.controller, PlayerId(0));
    assert_eq!(pacifism.attached_to, Some(bears));
    assert_eq!(shuffles(&g, 0), 1);
    checked(&g);
    assert_recoverable((&g).into());
}

/// CR 303.4g: with Zur destroyed in response and no other creature,
/// Pacifism has nothing to enchant, so it stays in the library, which is
/// still shuffled.
#[test]
fn zur_leaves_pacifism_in_the_library_without_a_creature_to_enchant() {
    let mut g = zur(true);
    let pacifism = Driver::object(&g, 5);
    answer(&mut g, 0, Reason::SearchLibrary, &[pacifism]);
    finish_combat(&mut g);
    assert_eq!(
        g.zone(Driver::object(&g, 5)),
        Some(Zone::Library { owner: PlayerId(0) })
    );
    assert_eq!(shuffles(&g, 0), 1);
    checked(&g);
    assert_recoverable((&g).into());
}

/// CR 304.4, 307.4: an unrestricted search onto the battlefield (Rampant
/// Growth's IR edited to "two cards, untapped"; no printed card finds an
/// instant or sorcery for the battlefield) puts the shockland there while
/// the sorcery it found stays in the library. A sorcery on the battlefield
/// never validates.
#[test]
fn a_battlefield_search_leaves_a_found_sorcery_in_the_library() {
    let catalog = unrestricted_growth(&[(HALLOWED_FOUNTAIN, &[])]);
    let prepared = Board::new(catalog, "r4-library-witness")
        .step(TurnStep::PrecombatMain)
        .seats(2)
        .seed(SEED)
        .permanent_or_hand("Rampant Growth", 0, 0, true)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .library_card("Hallowed Fountain", 0)
        .library_card("Rampant Growth", 0)
        .library_card("Grizzly Bears", 0)
        .library_card("Forest", 1)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1, 2]);
    let found = objects(&g, &[3, 4]);
    let d = answer(&mut g, 0, Reason::SearchLibrary, &found);
    let Shape::Pick { slots, .. } = &d.shape else {
        panic!("{d:?}")
    };
    assert_eq!((slots[0].min, slots[0].max), (2, 2));
    entry_payment(&mut g, true);
    settle(&mut g);
    assert_eq!(g.zone(Driver::object(&g, 3)), Some(Zone::Battlefield));
    assert_eq!(
        g.zone(Driver::object(&g, 4)),
        Some(Zone::Library { owner: PlayerId(0) })
    );
    assert_eq!(shuffles(&g, 0), 1);

    // A forged sorcery permanent: the found Rampant Growth on the battlefield.
    let sorcery = Driver::object(&g, 4);
    let object = g.state.rules.objects.get_mut(&sorcery.card).unwrap();
    object.zone = Zone::Battlefield;
    for library in g.state.rules.libraries.values_mut() {
        library.retain(|card| *card != sorcery.card);
    }
    assert!(g.validate_state().is_err(), "sorcery on the battlefield");
}

/// CR 303.4g, 800.4a: Zur is Murdered in response, and while its controller
/// chooses Pacifism's host, the only remaining creature's controller
/// concedes. No host is left, so Pacifism stays in the library and the
/// library is still shuffled; nothing faults.
#[test]
fn pacifism_stays_in_the_library_when_its_last_host_leaves_during_the_choice() {
    let mut g = zur_with(3, true, Some(2));
    let pacifism = Driver::object(&g, 5);
    answer(&mut g, 0, Reason::SearchLibrary, &[pacifism]);
    let d = next_choice(&mut g);
    assert_eq!(d.reason, Reason::EntryAttachment);
    g.concede(PlayerId(2)).unwrap();
    g.validate_state().unwrap();
    finish_combat(&mut g);
    assert!(!g.is_live(PlayerId(2)));
    assert_eq!(
        g.zone(Driver::object(&g, 5)),
        Some(Zone::Library { owner: PlayerId(0) })
    );
    assert_eq!(shuffles(&g, 0), 1);
    checked(&g);
    assert_recoverable((&g).into());
}

/// Saved hosts name only Auras, and saved payments are only ever affordable
/// in sequence (CR 119.4), both while the searcher is in the game and after
/// it departs.
#[test]
fn search_entry_answers_name_only_auras_and_affordable_payments() {
    for (life, case) in [(40, 0), (40, 1), (3, 2)] {
        let (mut g, [garden, pool]) = claim(life, 3);
        answer(&mut g, 0, Reason::SearchLibrary, &[garden, pool]);
        assert_eq!(next_choice(&mut g).reason, Reason::OptionalEffect);
        let Frame::ResolutionChoice {
            choice: ResolutionChoice::SearchEntry { source_block, .. },
            ..
        } = g.state.rules.frame.clone()
        else {
            panic!()
        };
        let forest = Driver::object(&g, 1);
        let entry = |hosts: Vec<(ObjectId, ObjectId)>, paid: Vec<(ObjectId, bool)>| {
            ResolutionChoice::SearchEntry {
                player: PlayerId(0),
                found: vec![garden, pool],
                hosts,
                paid,
                revealed: vec![],
                source_block,
            }
        };
        let (forged, what) = match case {
            // Temple Garden posing as an Aura, skipping its payment.
            0 => (
                entry(vec![(garden, forest)], vec![(pool, true)]),
                "non-Aura host entry skipping a payment",
            ),
            // Temple Garden posing as an Aura beside its own payment.
            1 => (
                entry(vec![(garden, forest)], vec![(garden, true)]),
                "non-Aura host entry",
            ),
            // Two 2-life payments from 3 life.
            _ => (
                entry(vec![], vec![(garden, true), (pool, true)]),
                "unaffordable payments",
            ),
        };
        forged_choice_rejected(&mut g, &forged, what);
        g.concede(PlayerId(0)).unwrap();
        through_departure(&mut g, 0);
        forged_choice_rejected(&mut g, &forged, what);
    }
}

/// The witness catalog with `cards` and Rampant Growth's IR edited to an
/// unrestricted "two cards, untapped" search onto the battlefield, which no
/// printed card has; it reaches entry paths printed cards cannot combine.
fn unrestricted_growth(cards: &[(&str, &[Cell])]) -> Catalog {
    let mut all = vec![(
        RAMPANT_GROWTH,
        &[Cell::SearchController, Cell::ShuffleController][..],
    )];
    all.extend_from_slice(cards);
    let mut catalog = catalog(&all);
    let growth = identity_in(&catalog, "Rampant Growth");
    let definition = catalog
        .definitions
        .iter_mut()
        .find(|d| d.oracle_id == growth)
        .unwrap();
    let mut value = serde_json::to_value(&*definition).unwrap();
    fn unrestrict(value: &mut serde_json::Value) -> usize {
        match value {
            serde_json::Value::Object(fields) => {
                let here = match fields.get_mut("search") {
                    Some(serde_json::Value::Object(search)) if search.contains_key("to") => {
                        search.insert("filter".into(), serde_json::json!([]));
                        search.insert(
                            "maximum".into(),
                            serde_json::json!({"amount": "fixed", "value": "2"}),
                        );
                        search["to"].as_object_mut().unwrap().remove("tapped");
                        1
                    }
                    _ => 0,
                };
                here + fields.values_mut().map(unrestrict).sum::<usize>()
            }
            serde_json::Value::Array(items) => items.iter_mut().map(unrestrict).sum(),
            _ => 0,
        }
    }
    assert_eq!(unrestrict(&mut value), 1);
    *definition = serde_json::from_value(value).unwrap();
    catalog
}

/// Rampant Growth (edited IR, see [`unrestricted_growth`]) finds Pacifism
/// and Hallowed Fountain; Pacifism goes on seat 1's Bears and the
/// Fountain's payment is open. Seat 2 controls a Forest (card 8).
fn pacifism_then_fountain() -> (Game, [ObjectId; 5], u32) {
    let prepared = Board::new(
        unrestricted_growth(&[(HALLOWED_FOUNTAIN, &[]), (PACIFISM, &[])]),
        "r4-library-witness",
    )
    .step(TurnStep::PrecombatMain)
    .seats(3)
    .seed(SEED)
    .permanent_or_hand("Rampant Growth", 0, 0, true)
    .permanent_or_hand("Forest", 0, 0, false)
    .permanent_or_hand("Forest", 0, 0, false)
    .permanent_or_hand("Grizzly Bears", 1, 1, false)
    .library_card("Pacifism", 0)
    .library_card("Hallowed Fountain", 0)
    .library_card("Grizzly Bears", 0)
    .library_card("Forest", 1)
    .permanent_or_hand("Forest", 2, 2, false)
    .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1, 2]);
    let ids = [1, 3, 4, 5, 8].map(|id| Driver::object(&g, id));
    let [_, bears, pacifism, fountain, _] = ids;
    answer(&mut g, 0, Reason::SearchLibrary, &[pacifism, fountain]);
    let d = next_choice(&mut g);
    assert_eq!(d.reason, Reason::EntryAttachment);
    g.respond(
        d.player,
        d.id,
        d.pick_one(&Candidate::Object { object: bears }).unwrap(),
    )
    .unwrap();
    let d = next_choice(&mut g);
    assert_eq!(offered_entrant(&d), fountain);
    let Frame::ResolutionChoice {
        choice: ResolutionChoice::SearchEntry { source_block, .. },
        ..
    } = g.state.rules.frame.clone()
    else {
        panic!()
    };
    (g, ids, source_block)
}

/// CR 303.4f-g: a saved host is the exact legal object chosen. A Forest
/// substituted for the Bears never validates; the Bears leaving with its
/// conceding owner does, and Pacifism then stays in the library while the
/// Fountain enters.
#[test]
fn a_saved_aura_host_is_the_chosen_legal_object_or_departed() {
    let (mut g, [forest, _, pacifism, fountain, _], source_block) = pacifism_then_fountain();
    let before = life(&g, 0);
    let swapped = ResolutionChoice::SearchEntry {
        player: PlayerId(0),
        found: vec![pacifism, fountain],
        hosts: vec![(pacifism, forest)],
        paid: vec![],
        revealed: vec![],
        source_block,
    };
    forged_choice_rejected(&mut g, &swapped, "a Forest substituted for the Bears");
    g.concede(PlayerId(1)).unwrap();
    through_departure(&mut g, 1);
    g.validate_state().unwrap();
    entry_payment(&mut g, true);
    settle(&mut g);
    assert_eq!(
        g.zone(Driver::object(&g, 4)),
        Some(Zone::Library { owner: PlayerId(0) })
    );
    let fountain = g.current(Driver::object(&g, 5)).unwrap().clone();
    assert_eq!((fountain.zone, fountain.tapped), (Zone::Battlefield, false));
    assert_eq!(life(&g, 0), minus(before, 2));
    assert_eq!(shuffles(&g, 0), 1);
}

/// CR 800.4a: after the searcher itself concedes, a saved host substituted
/// by seat 2's Forest (still on the battlefield) or by the searcher's own
/// Forest (gone with it) never validates, while the true Bears does, both
/// while the concession is queued and after the departure.
#[test]
fn a_departed_searchers_saved_host_stays_the_chosen_object() {
    let (mut g, [own_forest, _, pacifism, fountain, other_forest], source_block) =
        pacifism_then_fountain();
    let host = |host: ObjectId| ResolutionChoice::SearchEntry {
        player: PlayerId(0),
        found: vec![pacifism, fountain],
        hosts: vec![(pacifism, host)],
        paid: vec![],
        revealed: vec![],
        source_block,
    };
    g.concede(PlayerId(0)).unwrap();
    for departed in [false, true] {
        if departed {
            through_departure(&mut g, 0);
            assert!(g.state.rules.objects.get(&own_forest.card).is_none());
        }
        g.validate_state().unwrap();
        forged_choice_rejected(&mut g, &host(other_forest), "a present Forest as host");
        forged_choice_rejected(
            &mut g,
            &host(own_forest),
            "the searcher's own Forest as host",
        );
    }
}

/// A search entry never waits without a question: with Zur Murdered and no
/// creature left, and no player departed, an unanswered Pacifism entry is
/// a forged state.
#[test]
fn a_search_entry_without_a_question_never_validates() {
    let mut g = zur(true);
    let pacifism = Driver::object(&g, 5);
    let Frame::ResolutionChoice {
        choice: ResolutionChoice::Search { source_block, .. },
        ..
    } = g.state.rules.frame.clone()
    else {
        panic!("{:?}", g.state.rules.frame)
    };
    let pause = ResolutionChoice::SearchEntry {
        player: PlayerId(0),
        found: vec![pacifism],
        hosts: vec![],
        paid: vec![],
        revealed: vec![],
        source_block,
    };
    forged_choice_rejected(&mut g, &pause, "question-less search entry");
}
