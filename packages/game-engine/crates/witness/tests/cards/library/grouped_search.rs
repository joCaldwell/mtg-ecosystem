//! CR 201.2b, 608.2c, 701.23a–b, h: one private search, a legal group,
//! and one simultaneous move and shuffle.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

const R6: &str = include_str!(
    "../../../../../tests/fixtures/archive-manifest-r6-candidate-6a4226a5e7c5ed8d0044770ac4ededc8c2e5586217b80e53b454dd4a9f952b37.json"
);

macro_rules! records {
    ($id:literal) => {
        (
            include_str!(concat!(
                "../../../../../tests/fixtures/phase1-r5/",
                $id,
                ".json"
            )),
            include_str!(concat!(
                "../../../../../tests/fixtures/phase1-r6/",
                $id,
                ".json"
            )),
        )
    };
}

const KROSAN_VERGE: (&str, &str) = records!("d9a10971-f32b-4978-952d-fed0a5bc9e36");
const LOTUSLIGHT_DANCERS: (&str, &str) = records!("83c8f1ca-194e-4cb6-9d5f-029133c1edbc");
const SHARED_SUMMONS: (&str, &str) = records!("c2c9dbd0-2062-4ee1-b32e-8eeacd95589c");
const DEATHBELLOW_WAR_CRY: (&str, &str) = records!("947835a2-a72f-44e2-a183-99dcb019d6ff");
const THREE_DREAMS: (&str, &str) = records!("b3f9da94-c822-49b8-a1fc-be82189206af");
const UNCAGE_THE_MENAGERIE: (&str, &str) = records!("d2043685-f091-444f-9c35-d238afa24d7b");
const DARKSTEEL_MUTATION: &str = records!("05a4f8ff-49da-42af-add5-6248c4b0644b").0;

const TRACE_MODES: [TraceMode; 4] = [
    TraceMode::Off,
    TraceMode::Compact,
    TraceMode::Stream,
    TraceMode::Detailed,
];

fn grouped_board((r5, r6): (&str, &str), companions: &[(&str, &[Cell])]) -> Board {
    let mut catalog = catalog(companions);
    let legacy = mtg_witness::compile(mtg_witness::R5_MANIFEST, r5);
    let printed = mtg_witness::compile(R6, r6);
    let (control, definition) = match (legacy, printed) {
        (
            mtg_compiler::CompileResult::Lowered {
                definition: control,
            },
            mtg_compiler::CompileResult::Lowered { definition },
        ) => (control, definition),
        (legacy, printed) => {
            panic!("grouped search did not compile on both cuts: {legacy:?}; {printed:?}");
        }
    };
    assert!(definition.provenance.printed_sha256.is_some());
    assert_eq!(definition_tiers(&control), definition_tiers(&definition));
    assert_eq!(definition_tiers(&definition).unwrap(), Tiers::EXECUTES);
    catalog.definitions.push(*definition);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    Board::new(catalog, "grouped-search")
        .step(TurnStep::PrecombatMain)
        .seats(2)
        .seed(SEED)
}

fn search(game: &mut Game, quantum: u64, candidates: &[u32], selected: Option<u32>) {
    let request = loop {
        let request = Driver::request(game, quantum);
        if request.reason != Reason::Priority {
            break request;
        }
        let pass = request
            .pick_one(&Candidate::Action {
                action: Action::Pass,
            })
            .unwrap();
        game.respond(request.player, request.id, pass).unwrap();
    };
    assert_eq!(
        (request.player, &request.reason),
        (PlayerId(0), &Reason::SearchLibrary)
    );
    assert_eq!(
        request
            .candidates
            .iter()
            .map(|candidate| {
                let Candidate::Object { object } = candidate else {
                    panic!("unexpected search candidate {candidate:?}");
                };
                *object
            })
            .collect::<std::collections::BTreeSet<_>>(),
        objects(game, candidates).into_iter().collect()
    );
    let Shape::Pick { slots, .. } = &request.shape else {
        panic!("{request:?}")
    };
    assert_eq!(
        (slots[0].min, slots[0].max),
        (0, u32::from(!candidates.is_empty()))
    );
    assert_recoverable((&*game).into());
    private_to(game, 0, library(game, 0).len());
    let picks = selected
        .into_iter()
        .map(|id| {
            request
                .index(&Candidate::Object {
                    object: Driver::object(game, id),
                })
                .unwrap()
        })
        .collect();
    game.respond(
        request.player,
        request.id,
        Answer::Pick { picks: vec![picks] },
    )
    .unwrap();
    assert_recoverable((&*game).into());
}

fn finish(game: &mut Game, quantum: u64) {
    for _ in 0..16 {
        let request = Driver::request(game, quantum);
        assert_eq!(request.reason, Reason::Priority, "{request:?}");
        if game.state.rules.stack.is_empty() {
            assert_recoverable((&*game).into());
            assert_eq!(shuffles(game, 0), 1);
            assert_eq!(shuffles(game, 1), 0);
            return;
        }
        let pass = request
            .pick_one(&Candidate::Action {
                action: Action::Pass,
            })
            .unwrap();
        game.respond(request.player, request.id, pass).unwrap();
    }
    panic!("search did not finish");
}

#[test]
fn grouped_search_allocates_distinct_cards_to_overlapping_qualities() {
    for mode in TRACE_MODES {
        for quantum in [1, u64::MAX] {
            let mut game = Game::with_trace(
                grouped_board(KROSAN_VERGE, &[(TEMPLE_GARDEN, &[])])
                    .permanent_or_hand("Krosan Verge", 0, 0, false)
                    .permanent_or_hand("Island", 0, 0, false)
                    .permanent_or_hand("Island", 0, 0, false)
                    .library_card("Temple Garden", 0)
                    .library_card("Forest", 0)
                    .library_card("Island", 0)
                    .library_card("Forest", 1)
                    .prepare(),
                mode,
            );
            let object = Driver::object(&game, 0);
            Driver::action(&mut game, Action::ActivateAbility { object, ability: 0 });
            Driver::pay(&mut game, &[1, 2]);
            search(&mut game, quantum, &[3, 4], Some(3));
            assert_eq!(library(&game, 0).len(), 3, "the first card has not moved");
            // The dual land can fill either slot, leaving Forest legal.
            search(&mut game, quantum, &[4], Some(4));
            let entry = Driver::request(&mut game, quantum);
            assert_eq!(entry.reason, Reason::OptionalEffect);
            assert_recoverable((&game).into());
            let decline = entry
                .pick_one(&Candidate::Flag {
                    flag: Flag::Decline,
                })
                .unwrap();
            game.respond(entry.player, entry.id, decline).unwrap();
            finish(&mut game, quantum);
            assert_eq!(
                game.zone(Driver::object(&game, 0)),
                Some(Zone::Graveyard { owner: PlayerId(0) })
            );
            for id in [3, 4] {
                let land = game.current(Driver::object(&game, id)).unwrap();
                assert_eq!((land.zone, land.tapped), (Zone::Battlefield, true));
            }
            assert_eq!(library(&game, 0), [CardId(5)]);

            let mut game = Game::with_trace(
                grouped_board(LOTUSLIGHT_DANCERS, &[])
                    .permanent_or_hand("Lotuslight Dancers", 0, 0, true)
                    .permanent_or_hand("Swamp", 0, 0, false)
                    .permanent_or_hand("Forest", 0, 0, false)
                    .permanent_or_hand("Island", 0, 0, false)
                    .permanent_or_hand("Island", 0, 0, false)
                    .permanent_or_hand("Island", 0, 0, false)
                    .library_card("Yargle and Multani", 0)
                    .library_card("Murder", 0)
                    .library_card("Storm Crow", 0)
                    .library_card("Forest", 0)
                    .library_card("Forest", 1)
                    .prepare(),
                mode,
            );
            Driver::cast(&mut game, CardId(0));
            Driver::pay(&mut game, &[1, 2, 3, 4, 5]);
            search(&mut game, quantum, &[6, 7, 8], Some(6));
            search(&mut game, quantum, &[7, 8], Some(7));
            search(&mut game, quantum, &[8], Some(8));
            finish(&mut game, quantum);
            for id in [6, 7, 8] {
                assert_eq!(
                    game.zone(Driver::object(&game, id)),
                    Some(Zone::Graveyard { owner: PlayerId(0) })
                );
            }
            assert_eq!(library(&game, 0), [CardId(9)]);
        }
    }
}

#[test]
fn grouped_search_filters_duplicate_names_and_allows_stopping() {
    for mode in TRACE_MODES {
        for quantum in [1, u64::MAX] {
            for (card, name, mana, first, second, companions, take_both) in [
                (
                    SHARED_SUMMONS,
                    "Shared Summons",
                    "Forest",
                    "Grizzly Bears",
                    "Storm Crow",
                    &[][..],
                    true,
                ),
                (
                    THREE_DREAMS,
                    "Three Dreams",
                    "Plains",
                    "Pacifism",
                    "Darksteel Mutation",
                    &[(PACIFISM, &[][..]), (DARKSTEEL_MUTATION, &[][..])][..],
                    false,
                ),
            ] {
                let mut board = grouped_board(card, companions).permanent_or_hand(name, 0, 0, true);
                for _ in 0..5 {
                    board = board.permanent_or_hand(mana, 0, 0, false);
                }
                let prepared = board
                    .library_card(first, 0)
                    .library_card(first, 0)
                    .library_card(second, 0)
                    .library_card("Mountain", 0)
                    .library_card("Forest", 1)
                    .prepare();
                let mut game = Game::with_trace(prepared, mode);
                Driver::cast(&mut game, CardId(0));
                Driver::pay(&mut game, &[1, 2, 3, 4, 5]);
                search(&mut game, quantum, &[6, 7, 8], Some(7));
                search(&mut game, quantum, &[8], take_both.then_some(8));
                finish(&mut game, quantum);
                assert_eq!(
                    game.zone(Driver::object(&game, 7)),
                    Some(Zone::Hand { owner: PlayerId(0) })
                );
                assert_eq!(library(&game, 0).len(), if take_both { 2 } else { 3 });
                assert!(library(&game, 0).contains(&CardId(6)));
                assert_eq!(library(&game, 0).contains(&CardId(8)), !take_both);
            }
        }
    }
}

#[test]
fn grouped_search_rejects_unsupported_counts_and_scopes_at_their_source() {
    grouped_board(DEATHBELLOW_WAR_CRY, &[]);
    let (r5, r6) = UNCAGE_THE_MENAGERIE;
    for (manifest, record) in [(mtg_witness::R5_MANIFEST, r5), (R6, r6)] {
        let result = mtg_witness::compile(manifest, record);
        assert!(
            matches!(&result, mtg_compiler::CompileResult::Unsupported { path, reason }
            if !path.is_empty() && !reason.is_empty()),
            "{result:?}"
        );
    }
}
