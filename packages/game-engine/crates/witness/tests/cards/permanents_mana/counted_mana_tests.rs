//! CR 605.3b and 608.2h distinguish the count after costs from the count on
//! spell resolution. CR 106.4 credits the resolving player's mana pool.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

const MANIFEST: &str =
    include_str!("../../../../../tests/fixtures/archive-manifest-phase1-r5.json");

fn board() -> Board {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    for raw in [
        include_str!(
            "../../../../../tests/fixtures/phase1-r5/7c427c3d-ecd8-45ef-bebd-8f10f4a311db.json"
        ),
        include_str!(
            "../../../../../tests/fixtures/phase1-r5/6e2c2423-d854-4478-99e6-64f29851f026.json"
        ),
        include_str!(
            "../../../../../tests/fixtures/phase1-r5/7358e164-5704-4e78-9b21-6a9bf2a968ce.json"
        ),
        include_str!(
            "../../../../../tests/fixtures/phase1-r5/a8dba58b-2956-492e-ae30-49db2ae68e53.json"
        ),
    ] {
        match mtg_witness::compile(MANIFEST, raw) {
            mtg_compiler::CompileResult::Lowered { definition } => {
                catalog.definitions.push(*definition)
            }
            other => panic!("counted mana record rejected: {other:?}"),
        }
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    Board::new(catalog, "counted-mana-witness").step(TurnStep::PrecombatMain)
}

fn activate(game: &mut Game, id: u32) {
    let object = Driver::object(game, id);
    let answer = game.next_test_decision().activate_mana(object);
    Driver::answer(game, u64::MAX, answer);
    Driver::request(game, u64::MAX);
}

fn restored(game: &Game) -> Game {
    let snapshot = game.snapshot().unwrap();
    let restored = Game::restore(game.prepared.as_ref().clone(), &snapshot).unwrap();
    assert_eq!(restored.snapshot().unwrap(), snapshot);
    restored
}

#[test]
fn cradle_reads_zero_then_counts_a_creature_on_its_next_activation() {
    let mut game = Game::new(
        board()
            .permanent_or_hand("Gaea's Cradle", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Llanowar Elves", 0, 0, true)
            .libraries("Forest", 4)
            .prepare(),
    );
    activate(&mut game, 0);
    assert!(game.state.rules.players[0].mana.is_empty());
    Driver::cast(&mut game, CardId(2));
    Driver::pay(&mut game, &[1]);
    let mut game = restored(&game);
    for _ in 0..256 {
        let decision = Driver::request(&mut game, u64::MAX);
        let object = Driver::object(&game, 0);
        if decision
            .index(&Candidate::Action {
                action: Action::ActivateMana { object },
            })
            .is_some()
        {
            activate(&mut game, 0);
            assert_eq!(
                game.state.rules.players[0].mana.get(&Color::G),
                Some(&1.into())
            );
            restored(&game);
            return;
        }
        let answer = match decision.reason {
            Reason::Priority => decision
                .pick_one(&Candidate::Action {
                    action: Action::Pass,
                })
                .unwrap(),
            Reason::Attackers => decision.attack(vec![]),
            Reason::Blockers => decision.block(vec![]),
            _ => panic!("unexpected decision {decision:?}"),
        };
        game.respond(decision.player, decision.id, answer).unwrap();
    }
    panic!("Cradle did not become available on the following turn");
}

#[test]
fn geyser_counts_opponents_lands_tapped_in_response_after_restore() {
    let mut board = board().permanent_or_hand("Mana Geyser", 0, 0, true);
    for _ in 0..5 {
        board = board.permanent_or_hand("Mountain", 0, 0, false);
    }
    let mut game = Game::new(
        board
            .permanent_or_hand("Forest", 1, 1, false)
            .permanent_or_hand("Forest", 1, 1, false)
            .libraries("Forest", 4)
            .prepare(),
    );
    Driver::cast(&mut game, CardId(0));
    Driver::pay(&mut game, &[1, 2, 3, 4, 5]);
    Driver::pass(&mut game);
    activate(&mut game, 6);
    let mut game = restored(&game);
    for _ in 0..4 {
        Driver::pass(&mut game);
    }
    Driver::request(&mut game, u64::MAX);
    assert!(game.state.rules.stack.is_empty());
    assert_eq!(
        game.state.rules.players[0].mana.get(&Color::R),
        Some(&1.into())
    );
}

#[test]
fn coffers_restores_its_pending_activation_then_counts_tapped_swamps() {
    let mut game = Game::new(
        board()
            .permanent_or_hand("Cabal Coffers", 0, 0, false)
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Swamp", 0, 0, false)
            .libraries("Forest", 4)
            .prepare(),
    );
    activate(&mut game, 0);
    assert!(matches!(
        Driver::request(&mut game, u64::MAX).reason,
        Reason::Mana { .. }
    ));
    let mut game = restored(&game);
    activate(&mut game, 1);
    activate(&mut game, 2);
    Driver::answer(
        &mut game,
        u64::MAX,
        Answer::Pay {
            pay: PayAnswer::Finish {
                undo_on_failure: true,
            },
        },
    );
    Driver::answer(
        &mut game,
        u64::MAX,
        Answer::Pay {
            pay: PayAnswer::Spend {
                unrestricted: BTreeMap::from([(Color::B, 2.into())]),
            },
        },
    );
    Driver::request(&mut game, u64::MAX);
    assert_eq!(
        game.state.rules.players[0].mana.get(&Color::B),
        Some(&3.into())
    );
}
