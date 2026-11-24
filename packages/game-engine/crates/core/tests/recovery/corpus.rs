//! Each sample ends at a named rules boundary, including mid-step continuations.
use mtg_core::testkit::*;

fn game(catalog: FixtureCatalog, config: ScenarioConfig) -> Game {
    Game::new(Prepared::admit(catalog.get().clone(), config).unwrap())
}

#[test]
fn partial_setup() {
    let prepared = Prepared::admit_pod(
        FixtureCatalog::Match.get().clone(),
        MatchConfig::for_profile("commander-foundations-v1", [0; 32], PlayerId(0)).unwrap(),
    )
    .unwrap();
    let mut game = Game::new(prepared);
    assert!(matches!(
        game.advance(Budget::new(2)),
        Advance::Suspended { .. }
    ));
    // Snapshot 85 recovery anchor: two paid words in seat zero's first
    // opening shuffle, before any completed shuffle or seat-local effect.
    let snapshot: serde_json::Value = serde_json::from_str(&game.snapshot().unwrap()).unwrap();
    assert_eq!(
        snapshot["state"]["match_state"]["random"],
        serde_json::json!({
            "version":2, "seed": vec![0; 32], "table":0, "seat_random":[0,0,0,0],
            "open":{"stream":1u64<<56,"word":2}
        })
    );
    assert_recoverable((&game).into());
}

#[test]
fn mana_payment_and_priority_over_stack() {
    let mut game = game(FixtureCatalog::Base, ScenarioConfig::land_return());
    let raw: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/payment-inputs.json"
    ))
    .unwrap();
    let inputs: Vec<AcceptedInput> =
        serde_json::from_value(raw["accepted_inputs"].clone()).unwrap();
    for input in inputs.into_iter().take(5) {
        let request = Driver::request(&mut game, u64::MAX);
        if input.decision_id == 4 {
            assert!(matches!(
                request,
                Decision {
                    reason: Reason::SpendMana { .. },
                    ..
                }
            ));
            assert_recoverable((&game).into());
        }
        game.respond(input.player, input.decision_id, input.answer)
            .unwrap();
    }
    assert!(matches!(
        Driver::request(&mut game, u64::MAX),
        Decision {
            reason: Reason::Priority,
            ..
        }
    ));
    assert_eq!(game.state.rules.stack.len(), 1);
    assert_recoverable((&game).into());
}

#[test]
fn kernel_continuation() {
    let game = super::state_recovery::suspended_control();
    assert!(
        game.state
            .rules
            .frames
            .iter()
            .any(|frame| matches!(frame, Frame::Kernel { .. }))
    );
    assert_recoverable((&game).into());
}

#[test]
fn combat_declaration() {
    let mut game = game(FixtureCatalog::Base, ScenarioConfig::combat_interactions());
    for _ in 0..32 {
        match Driver::request(&mut game, u64::MAX) {
            Decision {
                reason: Reason::Attackers,
                ..
            } => {
                assert_recoverable((&game).into());
                return;
            }
            Decision {
                reason: Reason::Priority,
                ..
            } => Driver::pass(&mut game),
            other => panic!("unexpected {other:?}"),
        }
    }
    panic!("journal never reached combat");
}

#[test]
fn simultaneous_trigger_ordering() {
    let mut game = Game::new(witness_scenario(
        FixtureCatalog::Match.get().clone(),
        ScenarioConfig::triggers(),
    ));
    Driver::cast(&mut game, CardId(4));
    Driver::pay(&mut game, &[8, 9]);
    for _ in 0..4 {
        Driver::pass(&mut game);
    }
    assert!(matches!(
        Driver::request(&mut game, u64::MAX),
        Decision {
            reason: Reason::OrderTriggers,
            ..
        }
    ));
    assert_recoverable((&game).into());
}

#[test]
fn commander_replacement() {
    let mut game = game(FixtureCatalog::Action, ScenarioConfig::command_casting());
    Driver::cast(&mut game, CardId(0));
    Driver::pay(&mut game, &[1]);
    for _ in 0..5 {
        Driver::pass(&mut game);
    }
    Driver::cast(&mut game, CardId(7));
    let commander = Driver::object(&game, 0);
    {
        let selected_answer = game.next_test_decision().targets(vec![commander]);
        Driver::answer(&mut game, u64::MAX, selected_answer)
    };
    Driver::pay(&mut game, &[6]);
    for _ in 0..4 {
        Driver::pass(&mut game);
    }
    assert!(matches!(
        Driver::request(&mut game, u64::MAX),
        Decision {
            reason: Reason::CommanderReturn { .. },
            ..
        }
    ));
    assert_recoverable((&game).into());
}

#[test]
fn concession_advances_to_surviving_priority() {
    let mut game = game(FixtureCatalog::Base, ScenarioConfig::commander_foundation());
    Driver::pass(&mut game);
    let departing = Driver::request(&mut game, u64::MAX).player;
    let before = game.rule_boundary();
    game.concede(departing).unwrap();
    let next = Driver::request(&mut game, u64::MAX);
    assert!(matches!(
        next,
        Decision {
            reason: Reason::Priority,
            ..
        }
    ));
    assert_ne!(next.player, departing);
    assert_eq!(game.live_players().len(), 3);
    assert!(!game.live_players().contains(&departing));
    assert!(game.rule_boundary() > before);
    assert_recoverable((&game).into());
}

#[test]
fn terminal() {
    let mut game = game(FixtureCatalog::Base, ScenarioConfig::commander_foundation());
    Driver::pass(&mut game);
    for seat in [1, 2, 3] {
        game.concede(PlayerId(seat)).unwrap();
    }
    assert!(matches!(
        Driver::settle(&mut game, u64::MAX),
        Advance::Finished {
            winner: PlayerId(0)
        }
    ));
    assert_recoverable((&game).into());
}
