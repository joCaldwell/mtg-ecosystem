//! CR 614.15 and 605.3b: the mana ability pays its tap and replaces its
//! original production only when its public battlefield condition holds.
use super::*;
use mtg_compiler::CompileResult;

const R5: &str = include_str!("../../../../../tests/fixtures/archive-manifest-phase1-r5.json");
const LEAFKIN: &str = include_str!(
    "../../../../../tests/fixtures/phase1-r5/55eb9310-f24b-410b-850b-a7d0d5117946.json"
);
const AUDIENCE: &str = include_str!(
    "../../../../../tests/fixtures/phase1-r5/5be6f245-31cf-4294-ace0-ca6ce3463fed.json"
);
const CARYATID: &str = include_str!(
    "../../../../../tests/fixtures/phase1-r5/5f2be3c2-060a-43e1-b63b-9cd3c78ffcb0.json"
);
const RIVER: &str = include_str!(
    "../../../../../tests/fixtures/phase1-r5/8a83d284-75a0-4901-b7d9-c4b7586ee327.json"
);

fn board(record: &str, name: &str) -> Board {
    let CompileResult::Lowered { definition } = mtg_witness::compile(R5, record) else {
        panic!("{} rejected: {:?}", name, mtg_witness::compile(R5, record));
    };
    let mut catalog = FixtureCatalog::Witness.get().clone();
    catalog.definitions.push(*definition);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    Board::new(catalog, "conditional-mana-witness")
        .step(TurnStep::PrecombatMain)
        .permanent_or_hand(name, 0, 0, false)
        .libraries("Forest", 4)
}

fn activate(board: Board, color: Option<Color>, expected: i64) {
    let mut game = Game::new(board.prepare());
    let source = Driver::object(&game, 0);
    let answer = game.next_test_decision().activate_mana(source);
    Driver::answer(&mut game, u64::MAX, answer);
    let decision = Driver::request(&mut game, u64::MAX);
    if let Some(color) = color {
        assert!(matches!(decision.reason, Reason::ManaColor { .. }));
        assert_recoverable((&game).into());
        let answer = decision.pick_one(&Candidate::Color { color }).unwrap();
        Driver::answer(&mut game, u64::MAX, answer);
        Driver::request(&mut game, u64::MAX);
    }
    assert_eq!(
        game.state.rules.players[0].mana,
        BTreeMap::from([(color.unwrap_or(Color::G), Quantity::from(expected))])
    );
    assert!(game.state.rules.stack.is_empty());
    assert_recoverable((&game).into());
}

#[test]
fn conditional_mana_counts_the_activator_creatures_including_the_source() {
    for own_creatures in [2, 3] {
        let mut board = board(LEAFKIN, "Leafkin Druid")
            .permanent_or_hand("Colossal Dreadmaw", 1, 1, false)
            .permanent_or_hand("Forest", 0, 0, false);
        for _ in 0..own_creatures {
            board = board.permanent_or_hand("Llanowar Elves", 0, 0, false);
        }
        activate(board, None, if own_creatures == 2 { 1 } else { 2 });
    }
}

#[test]
fn conditional_mana_reads_control_and_current_power_before_choosing_its_output() {
    for (record, name, color) in [
        (AUDIENCE, "Raucous Audience", None),
        (CARYATID, "Ilysian Caryatid", Some(Color::U)),
    ] {
        let build = || {
            board(record, name)
                .permanent_or_hand("Colossal Dreadmaw", 1, 1, false)
                .permanent_or_hand("Llanowar Elves", 0, 0, false)
        };
        activate(build(), color, 1);
        activate(
            build().permanent_or_hand("Colossal Dreadmaw", 0, 0, false),
            color,
            2,
        );
    }
}

#[test]
fn conditional_mana_with_turn_history_stays_explicitly_unsupported() {
    let result = mtg_witness::compile(R5, RIVER);
    assert!(
        matches!(
            &result, CompileResult::Unsupported { path, .. } if path.contains("compose.mana-replacement")
        ),
        "{result:?}"
    );
}
