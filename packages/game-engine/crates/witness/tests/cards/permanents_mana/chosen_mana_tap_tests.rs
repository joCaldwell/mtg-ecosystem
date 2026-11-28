//! CR 302.6, 601.2h, 605.3b, 733.1: the tap symbol and the written tap
//! cost pay together, with exact-incarnation reversal on a failed payment.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_compiler::CompileResult;
use mtg_contracts::ChosenCostAction;

const R5: &str = include_str!("../../../../../tests/fixtures/archive-manifest-phase1-r5.json");
const SENTINEL: &str = include_str!(
    "../../../../../tests/fixtures/phase1-r5/e2f98813-4f39-4135-b05b-407e9eb797f6.json"
);
const SETTLEMENT: &str = include_str!(
    "../../../../../tests/fixtures/phase1-r5/e6b77545-de5c-4f4a-b7ea-83498fb33ba8.json"
);
const DRUM: &str = include_str!(
    "../../../../../tests/fixtures/phase1-r5/bdeb440d-a714-40e8-9038-f651e6ae45fb.json"
);

fn board(record: &str, name: &str) -> Board {
    let result = mtg_witness::compile(R5, record);
    let CompileResult::Lowered { definition } = result else {
        panic!("{name} rejected: {result:?}");
    };
    let mut catalog = FixtureCatalog::Witness.get().clone();
    catalog.definitions.push(*definition);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    Board::new(catalog, "chosen-mana-tap-witness")
        .step(TurnStep::PrecombatMain)
        .permanent_or_hand(name, 0, 0, false)
}

fn begin(game: &mut Game, payment: bool) {
    let object = Driver::object(game, 0);
    let request = Driver::request(game, u64::MAX);
    let answer = if payment {
        Answer::Pay {
            pay: PayAnswer::Activate {
                source: request
                    .index(&Candidate::Action {
                        action: Action::ActivateMana { object },
                    })
                    .unwrap(),
            },
        }
    } else {
        request.activate_mana(object)
    };
    Driver::answer(game, u64::MAX, answer);
}

fn choose_and_produce(game: &mut Game, helper: u32) {
    let choice = Driver::request(game, u64::MAX);
    assert_eq!(
        choice.reason,
        Reason::ChosenObjectCost {
            action: ChosenCostAction::Tap
        }
    );
    let object = Driver::object(game, helper);
    assert_eq!(choice.candidates, [Candidate::Object { object }]);
    assert!(!game.state.rules.objects[&CardId(0)].tapped);
    assert!(!game.state.rules.objects[&CardId(helper)].tapped);
    assert_recoverable((&*game).into());
    Driver::pick_object(game, object);
    let choice = Driver::request(game, u64::MAX);
    assert!(matches!(choice.reason, Reason::ManaColor { .. }));
    assert!(game.state.rules.objects[&CardId(0)].tapped);
    assert!(game.state.rules.objects[&CardId(helper)].tapped);
    assert_recoverable((&*game).into());
    let answer = choice
        .pick_one(&Candidate::Color { color: Color::G })
        .unwrap();
    Driver::answer(game, u64::MAX, answer);
    Driver::request(game, u64::MAX);
    assert_eq!(
        game.state.rules.players[0].mana.get(&Color::G),
        Some(&1.into())
    );
}

#[test]
fn chosen_mana_tap_accepts_a_summoning_sick_helper_and_can_reverse_both_costs() {
    for (record, name) in [(SENTINEL, "Jaspera Sentinel"), (DRUM, "Springleaf Drum")] {
        let mut game = Game::new(
            board(record, name)
                .permanent_or_hand("Llanowar Elves", 0, 0, true)
                .permanent_or_hand("Forest", 0, 0, false)
                .permanent_or_hand("Llanowar Elves", 1, 1, false)
                .permanent_or_hand("Colossal Dreadmaw", 0, 0, true)
                .libraries("Forest", 4)
                .prepare(),
        );
        let source = Driver::object(&game, 0);
        let decision = Driver::request(&mut game, u64::MAX);
        assert!(
            decision
                .index(&Candidate::Action {
                    action: Action::ActivateMana { object: source },
                })
                .is_none()
        );
        Driver::cast(&mut game, CardId(1));
        Driver::pay(&mut game, &[2]);
        for _ in 0..game.live_players().len() {
            Driver::pass(&mut game);
        }
        assert!(!game.state.rules.objects[&CardId(1)].attack_ready);
        let pool = game.state.rules.players[0].mana.clone();
        Driver::cast(&mut game, CardId(4));
        begin(&mut game, true);
        choose_and_produce(&mut game, 1);
        Driver::answer(
            &mut game,
            u64::MAX,
            Answer::Pay {
                pay: PayAnswer::Finish {
                    undo_on_failure: true,
                },
            },
        );
        Driver::request(&mut game, u64::MAX);
        assert!(!game.state.rules.objects[&CardId(0)].tapped);
        assert!(!game.state.rules.objects[&CardId(1)].tapped);
        assert!(game.state.rules.objects[&CardId(2)].tapped);
        assert!(game.state.rules.stack.is_empty());
        assert_eq!(game.state.rules.players[0].mana, pool);
        assert_recoverable((&game).into());
    }
}

#[test]
fn chosen_mana_tap_keeps_an_independent_fixed_color_ability() {
    let prepare = || board(SETTLEMENT, "Holdout Settlement");
    for helper in [false, true] {
        let board = prepare();
        let board = if helper {
            board.permanent_or_hand("Llanowar Elves", 0, 0, false)
        } else {
            board
        };
        let mut game = Game::new(board.libraries("Forest", 4).prepare());
        begin(&mut game, false);
        let choice = Driver::request(&mut game, u64::MAX);
        let ability = if helper { 1 } else { 0 };
        assert_eq!(choice.candidates.len(), if helper { 2 } else { 1 });
        let answer = choice
            .pick_one(&Candidate::ManaAbility {
                ability: mtg_core::ManaAbilityIdentity::Printed { index: ability },
            })
            .unwrap();
        Driver::answer(&mut game, u64::MAX, answer);
        if helper {
            choose_and_produce(&mut game, 1);
        } else {
            Driver::request(&mut game, u64::MAX);
            assert_eq!(
                game.state.rules.players[0].mana.get(&Color::C),
                Some(&1.into())
            );
        }
        assert_recoverable((&game).into());
    }
}
