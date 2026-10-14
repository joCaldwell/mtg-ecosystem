use super::*;

fn game() -> Game {
    Game::new(
        Prepared::admit(
            Catalog::read(include_str!("../../../../tests/fixtures/expected-ir.json")).unwrap(),
            ScenarioConfig::stack_response(),
        )
        .unwrap(),
    )
}

fn reversal(source: Option<ObjectId>, amount: i64, remove_zero: bool) -> ProposedEvent {
    ProposedEvent::ReverseMana {
        player: PlayerId(0),
        color: Color::U,
        amount: Quantity::from(amount),
        source,
        remove_zero,
    }
}

fn cause() -> BatchCause {
    BatchCause::CastReversal {
        stack_id: StackId(0),
        player: PlayerId(0),
    }
}

#[test]
fn ordered_reversal_restores_sources_and_exact_pool_shape() {
    let mut game = game();
    let first = game.object_id(CardId(0)).unwrap();
    let second = game.object_id(CardId(3)).unwrap();
    for object in [first, second] {
        game.state.objects.get_mut(&object.card).unwrap().tapped = true;
    }
    // Ownership/control may differ when another player departs during casting.
    game.state.objects.get_mut(&second.card).unwrap().controller = PlayerId(2);
    game.state.players[0]
        .mana
        .insert(Color::U, Quantity::from(2));
    let batch = game
        .commit_batch(
            cause(),
            EventGroup::Ordered {
                children: vec![
                    EventGroup::Occurrence {
                        proposed: reversal(Some(second), 1, false),
                    },
                    EventGroup::Occurrence {
                        proposed: reversal(Some(first), 1, true),
                    },
                ],
            },
        )
        .unwrap();
    assert_eq!(batch.rules, ["733.1", "733.2"]);
    assert_eq!(batch.before.objects.len(), 2);
    assert_eq!(batch.before.players[0].mana[&Color::U], Quantity::from(2));
    assert_eq!(
        batch.occurrences[1].before.players[0].mana[&Color::U],
        Quantity::from(1)
    );
    assert!(matches!(
        batch.result(&[0]),
        Some(EventOutcome::ManaReversed {
            removed_entry: false,
            ..
        })
    ));
    assert!(matches!(
        batch.result(&[1]),
        Some(EventOutcome::ManaReversed {
            removed_entry: true,
            ..
        })
    ));
    assert!(game.state.players[0].mana.is_empty());
    assert!(!game.current(first).unwrap().tapped);
    assert!(!game.current(second).unwrap().tapped);
    assert_eq!(game.current(second).unwrap().controller, PlayerId(2));

    // A zero-valued key predating activation must survive reversal.
    game.state.objects.get_mut(&first.card).unwrap().tapped = true;
    game.state.players[0]
        .mana
        .insert(Color::U, Quantity::from(1));
    game.commit(reversal(Some(first), 1, false), cause())
        .unwrap();
    assert_eq!(game.state.players[0].mana[&Color::U], Quantity::zero());
}

#[test]
fn absent_source_reversal_never_resurrects_departed_owned_objects() {
    let mut game = game();
    game.state.objects.remove(&CardId(3));
    game.state.players[0]
        .mana
        .insert(Color::U, Quantity::from(1));
    let batch = game
        .commit_batch(
            cause(),
            EventGroup::Occurrence {
                proposed: reversal(None, 1, true),
            },
        )
        .unwrap();
    assert!(batch.before.objects.is_empty());
    assert_eq!(batch.before.players.len(), 1);
    assert!(matches!(
        batch.result(&[]),
        Some(EventOutcome::ManaReversed {
            source: None,
            removed_entry: true,
            ..
        })
    ));
    assert!(!game.state.objects.contains_key(&CardId(3)));
    assert!(game.state.players[0].mana.is_empty());
}

#[test]
fn invalid_reversal_or_conflicting_group_preserves_state_trace_and_tokens() {
    let mut initial = game();
    let land = initial.object_id(CardId(0)).unwrap();
    let bear = initial.object_id(CardId(2)).unwrap();
    initial.state.objects.get_mut(&land.card).unwrap().tapped = true;
    initial.state.objects.get_mut(&bear.card).unwrap().tapped = true;
    initial.state.players[0]
        .mana
        .insert(Color::U, Quantity::from(1));
    let stale = ObjectId {
        incarnation: land.incarnation + 1,
        ..land
    };
    for proposed in [
        reversal(Some(land), -1, true),
        reversal(Some(land), 0, true),
        reversal(Some(land), 2, true),
        reversal(Some(stale), 1, true),
        reversal(Some(bear), 1, true),
    ] {
        let mut game = initial.fork();
        let token = game.state_token.clone();
        assert!(game.commit(proposed, cause()).is_err());
        assert_eq!(game.state, initial.state);
        assert_eq!(game.trace, initial.trace);
        assert!(Arc::ptr_eq(&token, &game.state_token));
    }
    for group in [
        EventGroup::Ordered {
            children: vec![
                EventGroup::Occurrence {
                    proposed: reversal(Some(land), 1, true),
                },
                EventGroup::Occurrence {
                    proposed: reversal(None, 1, true),
                },
            ],
        },
        EventGroup::Simultaneous {
            occurrences: vec![reversal(None, 1, false), reversal(None, 1, false)],
        },
    ] {
        let mut game = initial.fork();
        let token = game.state_token.clone();
        assert!(game.commit_batch(cause(), group).is_err());
        assert_eq!(game.state, initial.state);
        assert_eq!(game.trace, initial.trace);
        assert!(Arc::ptr_eq(&token, &game.state_token));
    }

    for unavailable in ["untapped", "wrong_zone", "departed"] {
        let mut game = initial.fork();
        match unavailable {
            "untapped" => game.state.objects.get_mut(&land.card).unwrap().tapped = false,
            "wrong_zone" => game.state.objects.get_mut(&land.card).unwrap().zone = Zone::Exile,
            "departed" => game.state.players[0].status = PlayerStatus::Left,
            _ => unreachable!(),
        }
        let state = game.state.clone();
        let trace = game.trace.clone();
        assert!(game.commit(reversal(Some(land), 1, true), cause()).is_err());
        assert_eq!(game.state, state);
        assert_eq!(game.trace, trace);
    }
}
