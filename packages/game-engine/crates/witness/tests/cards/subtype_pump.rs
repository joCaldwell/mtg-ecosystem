//! CR 205.3m, 611.2c, 613.4c: a resolving P/T modifier captures creatures by
//! their current subtypes when it begins, after earlier layers are applied.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use mtg_compiler::CompileResult;
use mtg_core::testkit::*;

const GHOUL: &str =
    include_str!("../../../../tests/fixtures/phase1-r5/602e421e-afab-4488-8a52-af66b4fdce4b.json");
const DEMON: &str =
    include_str!("../../../../tests/fixtures/phase1-r5/2cee861f-d28c-47e6-baee-2da9b7fa89d8.json");
const CHOSEN: &str =
    include_str!("../../../../tests/fixtures/phase1-r5/e7a7ba65-ad14-41d5-889b-9bbeae9ab3f7.json");

fn board(record: &str, name: &str) -> Board {
    let result = mtg_witness::compile(mtg_witness::R5_MANIFEST, record);
    let CompileResult::Lowered { definition } = result else {
        panic!("{name} rejected: {result:?}");
    };
    let mut catalog = FixtureCatalog::Witness.get().clone();
    catalog.definitions.push(*definition);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    let mut board = Board::new(catalog, "subtype-pump-witness")
        .step(TurnStep::PrecombatMain)
        .seats(2)
        .permanent_or_hand(name, 0, 0, true)
        .permanent_or_hand("Colossal Dreadmaw", 0, 0, false)
        .permanent_or_hand("Colossal Dreadmaw", 1, 1, false)
        .permanent_or_hand("Llanowar Elves", 1, 1, false);
    for _ in 0..5 {
        board = board.permanent_or_hand("Swamp", 0, 0, false);
    }
    board.libraries("Swamp", 4)
}

fn settle_spell_and_trigger(game: &mut Game) {
    for _ in 0..2 * game.live_players().len() {
        Driver::pass(game);
    }
    Driver::request(game, u64::MAX);
    assert!(game.state.rules.stack.is_empty());
    assert_eq!(
        game.state.rules.objects[&CardId(3)].zone,
        Zone::Graveyard { owner: PlayerId(1) }
    );
    assert_recoverable((&*game).into());
}

fn power(game: &Game, card: u32) -> Quantity {
    game.characteristics(Driver::object(game, card))
        .unwrap()
        .power
        .unwrap()
}

#[test]
fn subtype_pump_excludes_the_entering_zombie_and_captures_both_boards() {
    let mut game = Game::new(board(GHOUL, "Noxious Ghoul").prepare());
    Driver::cast(&mut game, CardId(0));
    Driver::pay(&mut game, &[4, 5, 6, 7, 8]);
    settle_spell_and_trigger(&mut game);
    assert_eq!(power(&game, 0), 3.into());
    assert_eq!(power(&game, 1), 5.into());
    assert_eq!(power(&game, 2), 5.into());
}

#[test]
fn subtype_pump_reads_the_demon_subtype_the_aura_granted_on_entry() {
    let mut game = Game::new(board(DEMON, "Inner Demon").prepare());
    Driver::cast(&mut game, CardId(0));
    let target = Driver::object(&game, 1);
    Driver::pick_object(&mut game, target);
    Driver::pay(&mut game, &[4, 5, 6, 7]);
    settle_spell_and_trigger(&mut game);
    assert_eq!(power(&game, 1), 8.into());
    assert_eq!(power(&game, 2), 4.into());
}

#[test]
fn subtype_pump_with_an_unbound_chosen_type_stays_unsupported() {
    let result = mtg_witness::compile(mtg_witness::R5_MANIFEST, CHOSEN);
    assert!(
        matches!(&result, CompileResult::Unsupported { reason, .. } if reason.contains("not_chosen_type")),
        "{result:?}"
    );
}
