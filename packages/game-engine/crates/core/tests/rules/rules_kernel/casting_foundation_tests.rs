//! M3 foundation only: ordinary paid source proposals plus explicitly synthetic
//! payment-boundary controls. No resolution/exile casting permission or cascade
//! source admission follows from these tests.
use super::*;
use payments::{AnnouncementCompleted, AnnouncementPayment};

fn game(mode: TraceMode) -> Game {
    Game::with_trace(
        FixtureCatalog::Base.admit(ScenarioConfig::stack_response()),
        mode,
    )
}
fn mana_window(mode: TraceMode) -> Game {
    let mut game = game(mode);
    {
        let selected_answer = game.next_test_decision().cast(CardId(1));
        Driver::answer(&mut game, u64::MAX, selected_answer)
    };
    let target = game.object_id(CardId(2)).unwrap();
    {
        let selected_answer = game.next_test_decision().targets(vec![target]);
        Driver::answer(&mut game, u64::MAX, selected_answer)
    };
    assert!(matches!(
        Driver::request(&mut game, u64::MAX),
        Decision {
            reason: Reason::Mana { .. },
            ..
        }
    ));
    game
}
fn casting(game: &Game) -> Casting {
    let Frame::Mana { cast } = &game.state.rules.frame else {
        panic!("not a casting mana window");
    };
    cast.clone()
}

#[test]
fn m3_foundation_unpayable_payment_is_nonmutating_and_keeps_announcement_owner() {
    let mode = TraceMode::Detailed;
    let mut game = mana_window(mode);
    let cast = casting(&game);
    let state = game.state.clone();
    let trace = game.trace.clone();
    assert_eq!(
        game.pay_announcement_costs(&cast).unwrap(),
        AnnouncementPayment::Unpayable
    );
    assert_eq!(game.state, state);
    assert_eq!(game.trace, trace);
    assert!(game.top().unwrap().paid.is_none());
}

#[test]
fn m3_foundation_payment_receipt_defers_resumption_and_cannot_complete_twice() {
    let mode = TraceMode::Detailed;
    let mut game = mana_window(mode);
    let source = game.object_id(CardId(0)).unwrap();
    {
        let selected_answer = game.next_test_decision().activate_mana(source);
        Driver::answer(&mut game, u64::MAX, selected_answer)
    };
    let _ = Driver::request(&mut game, u64::MAX);
    // Direct primitive control: explicit spending is supplied without
    // fabricating an accepted input or claiming the resulting fork replays.
    let mut cast = casting(&game);
    cast.spending = Some(BTreeMap::from([(Color::U, Quantity::from(1))]));
    let frame = game.state.rules.frame.clone();
    let priority = game.state.rules.priority;
    let receipt = game.pay_announcement_costs(&cast).unwrap();
    assert_eq!(
        receipt,
        AnnouncementPayment::Completed(AnnouncementCompleted {
            stack_id: cast.stack_id,
            controller: PlayerId(0),
        })
    );
    assert_eq!(game.state.rules.frame, frame);
    assert_eq!(game.state.rules.priority, priority);
    assert_eq!(
        game.top().unwrap().paid.as_ref(),
        Some(&game.top().unwrap().total_cost)
    );
    assert_eq!(game.mana(PlayerId(0), Color::U), Some(Quantity::zero()));
    let state = game.state.clone();
    let trace = game.trace.clone();
    assert!(
        game.pay_announcement_costs(&cast)
            .unwrap_err()
            .contains("binding")
    );
    assert_eq!(game.state, state);
    assert_eq!(game.trace, trace);
    if mode == TraceMode::Detailed {
        assert_eq!(
            game.events()
                .iter()
                .filter(|event| matches!(event, Event::SpellCast { .. }))
                .count(),
            1
        );
    }
}

#[test]
fn m3_foundation_forged_zero_payment_rejects_without_changing_printed_mana_value() {
    let mut game = mana_window(TraceMode::Detailed);
    let mut cast = casting(&game);
    // A forged total is not a free-casting permission or a valid lock.
    // Genuine alternative/ordinary-zero payments have separate witnesses.
    game.state.rules.stack.last_mut().unwrap().total_cost = zero_mana_cost();
    cast.spending = Some(BTreeMap::new());
    let printed = game
        .definition(CardId(1))
        .unwrap()
        .characteristics
        .mana_cost
        .clone();
    let value = game.stack_mana_value(game.top().unwrap()).unwrap();
    assert!(value > Quantity::zero());
    let frame = game.state.rules.frame.clone();
    assert!(game.pay_announcement_costs(&cast).is_err());
    assert_eq!(game.top().unwrap().paid, None);
    assert_eq!(
        game.definition(CardId(1))
            .unwrap()
            .characteristics
            .mana_cost,
        printed
    );
    assert_eq!(game.stack_mana_value(game.top().unwrap()).unwrap(), value);
    assert_eq!(game.state.rules.frame, frame);
    assert_eq!(
        game.events()
            .iter()
            .filter(|event| matches!(event, Event::SpellCast { .. }))
            .count(),
        0
    );
}

#[test]
fn m3_foundation_payment_rejects_foreign_stack_owner_before_mutation() {
    let mut game = mana_window(TraceMode::Detailed);
    let mut cast = casting(&game);
    cast.stack_id.0 += 1;
    let before = game.snapshot().unwrap();
    assert!(
        game.pay_announcement_costs(&cast)
            .unwrap_err()
            .contains("binding")
    );
    assert_eq!(game.snapshot().unwrap(), before);
}

#[test]
fn m3_foundation_priority_authority_never_grants_exile_or_announcement_casts() {
    let mut game = game(TraceMode::Detailed);
    let _ = Driver::request(&mut game, u64::MAX);
    let before = game.snapshot().unwrap();
    assert!(
        game.announce_priority_spell(PlayerId(1), CardId(1))
            .is_err()
    );
    assert_eq!(game.snapshot().unwrap(), before);
    // Synthetic zone control: an ordinary request must never grant broad exile
    // access even for the caster's own supported spell.
    let mut exile = game.fork();
    exile.state.rules.objects.get_mut(&CardId(1)).unwrap().zone = Zone::Exile;
    let before = exile.snapshot().unwrap();
    assert!(
        exile
            .announce_priority_spell(PlayerId(0), CardId(1))
            .is_err()
    );
    assert_eq!(exile.snapshot().unwrap(), before);
    {
        let selected_answer = game.next_test_decision().cast(CardId(1));
        Driver::answer(&mut game, u64::MAX, selected_answer)
    };
    let _ = Driver::request(&mut game, u64::MAX);
    let before = game.snapshot().unwrap();
    assert!(
        game.announce_priority_spell(PlayerId(0), CardId(1))
            .is_err()
    );
    assert_eq!(game.snapshot().unwrap(), before);
}

#[test]
fn m3_foundation_sampler_covers_every_physical_range_and_exact_rejection_tail() {
    assert_eq!(capacities::OWNED_LIMIT, 100);
    for upper in 2..=100u32 {
        let space = 1u64 << 32;
        let limit = space - space % u64::from(upper);
        // Every accepted complete block has one of each residue, including
        // the final block; every leftover word rejects instead of biasing zero.
        for residue in 0..upper {
            assert_eq!(setup::sample_shuffle_word(residue, upper), Some(residue));
            assert_eq!(
                setup::sample_shuffle_word(
                    (limit - u64::from(upper) + u64::from(residue)) as u32,
                    upper
                ),
                Some(residue)
            );
        }
        for word in limit..space {
            assert_eq!(setup::sample_shuffle_word(word as u32, upper), None);
        }
    }
    assert_eq!(setup::sample_shuffle_word(0, 0), None);
    assert_eq!(setup::sample_shuffle_word(0, 1), None);
    assert_eq!(setup::sample_shuffle_word(0, 101), None);
    assert_eq!(setup::sample_shuffle_word(u32::MAX, 100), None);
    assert_eq!(setup::sample_shuffle_word(4_294_967_199, 100), Some(99));
}
