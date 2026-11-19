use super::*;

#[test]
fn synthetic_cast_entry_gates_draw_on_the_paid_predecessor() {
    let source = include_str!("../../../../tests/fixtures/synthetic-cast-entry.json");
    let prepared = prepared_with_sources(
        &[
            ("Cinder Archive Surveyor", 0, 0, true),
            ("Forest", 0, 0, false),
            ("Forest", 0, 0, false),
            ("Unsummon", 0, 0, true),
            ("Island", 0, 0, false),
        ],
        &[source],
    );
    let controller = PlayerId(0);
    let mut game = Game::with_trace(prepared, TraceMode::Detailed);
    let mut direct = game.fork();
    let initial = object(&direct, 0);
    direct
        .move_object_for(initial, Zone::Battlefield, BatchCause::Fixture)
        .unwrap();
    assert!(direct.state.rules.pending_triggers.is_empty());
    assert!(direct.events().iter().any(|event| matches!(
        event,
        Event::TriggerConditionChecked {
            check: InterveningCheck::SourceCastByController { cast: None, .. },
            ..
        }
    )));

    announce(&mut game, 1, 0, vec![]);
    let paid = game.top().unwrap().clone();
    assert_eq!(paid.paid.as_ref(), Some(&paid.total_cost));
    for _ in 0..4 {
        answer(&mut game, 1, Response::Pass);
    }
    assert!(matches!(request(&mut game, 1), Request::Priority { .. }));
    let entrant = object(&game, 0);
    let evidence = InterveningCheck::SourceCastByController {
        entrant,
        controller,
        cast: Some(EntryCast {
            spell: paid.source,
            stack_id: paid.id,
            caster: controller,
        }),
    };
    let trigger = game.top().unwrap();
    assert_eq!(trigger.source, entrant);
    assert!(
        matches!(&trigger.kind, StackKind::Triggered {match_condition:Some(check), ..} if check == &evidence)
    );
    recover(&game);
    let mut forged = game.fork();
    let StackKind::Triggered {
        match_condition:
            Some(InterveningCheck::SourceCastByController {
                cast: Some(cast), ..
            }),
        ..
    } = &mut forged.state.rules.stack.last_mut().unwrap().kind
    else {
        unreachable!()
    };
    cast.spell.incarnation = entrant.incarnation;
    assert!(
        Game::restore(
            forged.prepared.as_ref().clone(),
            &forged.snapshot().unwrap()
        )
        .is_err()
    );

    // A response moves the exact entrant away before the independent trigger resolves.
    announce(
        &mut game,
        1,
        3,
        vec![TargetEntity::Object { object: entrant }],
    );
    for _ in 0..4 {
        answer(&mut game, 1, Response::Pass);
    }
    assert!(matches!(request(&mut game, 1), Request::Priority { .. }));
    assert_eq!(
        game.zone(object(&game, 0)),
        Some(Zone::Hand { owner: controller })
    );
    for _ in 0..4 {
        answer(&mut game, 1, Response::Pass);
    }
    assert!(
        matches!(request(&mut game,1), Request::OptionalEffect {player, ..} if player == controller)
    );
    assert!(
        game.events()
            .iter()
            .any(|event| matches!(event,Event::TriggerConditionChecked {
        stage:InterveningStage::ResolutionStart { .. },check, ..
    } if check == &evidence))
    );
    recover(&game);
    let hand = game.action_hand(controller).len();
    answer(&mut game, 1, Response::OptionalEffect { perform: true });
    assert!(matches!(request(&mut game, 1), Request::Priority { .. }));
    assert_eq!(game.action_hand(controller).len(), hand + 1);
    recover(&game);

    // The same physical card enters again without a cast. An earlier payment
    // or surviving history for that card cannot qualify this new incarnation.
    let returned = object(&game, 0);
    let trigger_count = game.state.rules.next_trigger_id;
    game.move_object_for(returned, Zone::Battlefield, BatchCause::Fixture)
        .unwrap();
    assert_eq!(game.state.rules.next_trigger_id, trigger_count);
    assert!(game.state.rules.pending_triggers.is_empty());
}

#[test]
fn paid_entry_untaps_only_the_trigger_controllers_current_tapped_lands() {
    let source = include_str!("../../../../tests/fixtures/synthetic-cast-land-untap.json");
    let prepared = prepared_with_sources(
        &[
            ("Tidelock Surveyor", 0, 0, true),
            ("Forest", 1, 0, false),
            ("Forest", 0, 0, false),
            ("Forest", 0, 0, false),
            ("Forest", 0, 1, false),
        ],
        &[source],
    );
    let mut game = Game::with_trace(prepared, TraceMode::Detailed);
    announce(&mut game, 1, 0, vec![]);
    for _ in 0..4 {
        answer(&mut game, 1, Response::Pass);
    }
    assert!(matches!(request(&mut game, 1), Request::Priority { .. }));
    let trigger = game.top().unwrap().id;
    recover(&game);
    let controlled = [object(&game, 1), object(&game, 2)];
    assert!(
        controlled
            .iter()
            .all(|id| game.current(*id).unwrap().tapped)
    );
    let opposed = object(&game, 4);
    game.commit(ProposedEvent::Tap { object: opposed }, BatchCause::Fixture)
        .unwrap();
    for _ in 0..4 {
        answer(&mut game, 1, Response::Pass);
    }
    assert!(matches!(request(&mut game, 1), Request::Priority { .. }));
    let batches = game.trace().records.iter().filter_map(|r| match &r.record {
        EvidenceRecord::Batch {batch} if matches!(batch.cause,BatchCause::Instruction {stack_id,source_block:0,..} if stack_id==trigger) => Some(batch),
        _ => None
    }).collect::<Vec<_>>();
    assert_eq!(batches.len(), 1);
    assert_eq!(
        batches[0].group,
        EventGroup::Simultaneous {
            occurrences: controlled
                .map(|object| ProposedEvent::Untap { object })
                .to_vec()
        }
    );
    assert!(
        controlled
            .iter()
            .all(|id| !game.current(*id).unwrap().tapped)
    );
    assert!(game.current(opposed).unwrap().tapped);
}
