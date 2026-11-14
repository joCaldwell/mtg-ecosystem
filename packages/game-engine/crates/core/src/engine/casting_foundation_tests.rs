//! M3 foundation only: ordinary paid source proposals plus explicitly synthetic
//! payment-boundary controls. No resolution/exile casting permission or cascade
//! source admission follows from these tests.
use super::*;
use payments::{AnnouncementCompleted, AnnouncementPayment};

fn game(mode: TraceMode) -> Game {
    Game::with_trace(
        Prepared::admit(
            Catalog::read(include_str!("../../../../tests/fixtures/expected-ir.json")).unwrap(),
            ScenarioConfig::stack_response(),
        )
        .unwrap(),
        mode,
    )
}
fn request(game: &mut Game) -> Request {
    loop {
        match game.advance(Budget::new(1)) {
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(request) => return request,
            other => panic!("unexpected boundary {other:?}"),
        }
    }
}
fn answer(game: &mut Game, response: Response) {
    let request = request(game);
    game.respond(request.player(), request.decision_id(), response)
        .unwrap();
}
fn mana_window(mode: TraceMode) -> Game {
    let mut game = game(mode);
    answer(&mut game, Response::Cast { card: CardId(1) });
    let target = game.object_id(CardId(2)).unwrap();
    answer(
        &mut game,
        Response::Targets {
            objects: vec![target],
        },
    );
    assert!(matches!(request(&mut game), Request::Mana { .. }));
    game
}
fn casting(game: &Game) -> Casting {
    let Frame::Mana { cast } = &game.state.rules.frame else {
        panic!("not a casting mana window");
    };
    cast.clone()
}
fn prove(game: &Game) {
    let restored =
        Game::restore(game.prepared.as_ref().clone(), &game.snapshot().unwrap()).unwrap();
    assert_eq!(restored.state, game.state);
    let replayed = Game::replay_to(
        game.prepared.as_ref().clone(),
        &game.replay_journal(),
        Some(game.rule_boundary()),
        game.trace.mode,
    )
    .unwrap();
    assert_eq!(replayed.state, game.state);
    assert_eq!(replayed.trace, game.trace);
}

#[test]
fn m3_foundation_unpayable_payment_is_nonmutating_and_keeps_announcement_owner() {
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
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
        prove(&game);
    }
}

#[test]
fn m3_foundation_payment_receipt_defers_resumption_and_cannot_complete_twice() {
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        let mut game = mana_window(mode);
        let source = game.object_id(CardId(0)).unwrap();
        answer(&mut game, Response::ActivateMana { object: source });
        let _ = request(&mut game);
        prove(&game);
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
}

#[test]
fn m3_foundation_empty_payment_completes_without_changing_printed_mana_value() {
    let mut game = mana_window(TraceMode::Detailed);
    let mut cast = casting(&game);
    // Synthetic locked-zero-cost control, not a free-casting permission or a
    // source-backed alternative-cost witness. Printed cost remains untouched.
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
    assert!(matches!(
        game.pay_announcement_costs(&cast).unwrap(),
        AnnouncementPayment::Completed(_)
    ));
    assert_eq!(game.top().unwrap().paid, Some(zero_mana_cost()));
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
        1
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
    prove(&game);
}

#[test]
fn m3_foundation_priority_authority_never_grants_exile_or_announcement_casts() {
    let mut game = game(TraceMode::Detailed);
    let _ = request(&mut game);
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
    answer(&mut game, Response::Cast { card: CardId(1) });
    let _ = request(&mut game);
    let before = game.snapshot().unwrap();
    assert!(
        game.announce_priority_spell(PlayerId(0), CardId(1))
            .is_err()
    );
    assert_eq!(game.snapshot().unwrap(), before);
    prove(&game);
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
