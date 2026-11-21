//! CR 601.2c-d, 608.2b and 616.1. The choices bind before costs, and a
//! surviving creature keeps its original share when another target leaves.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

#[test]
fn divided_plus_one_counters_paid_charm_keeps_frozen_shares() {
    let charm = include_str!(
        "../../../../../tests/fixtures/ramos/4137a22c-f793-4e27-a9b2-72740a6e2121.json"
    );
    let scales = include_str!(
        "../../../../../tests/fixtures/ramos/a1f3da21-af6d-450e-bf0b-985d158418e6.json"
    );
    let mut rows = vec![
        ("Abzan Charm", 0, 0, true),
        ("Grizzly Bears", 0, 0, false),
        ("Grizzly Bears", 1, 1, false),
        ("Hardened Scales", 0, 0, false),
        ("Disperse", 1, 1, true),
    ];
    for land in ["Plains", "Swamp", "Forest"] {
        lands(&mut rows, land, 0, 1);
    }
    lands(&mut rows, "Island", 1, 2);
    let prepared = prepared_with_sources(&rows, &[charm, scales]);
    let mut game = Game::with_trace(prepared.clone(), TraceMode::Detailed);
    let caster = PlayerId(0);
    let first = object(&game, 1);
    let second = object(&game, 2);
    answer(&mut game, 1, Response::Cast { card: CardId(0) });
    assert!(matches!(request(&mut game, 1), Request::Modes { .. }));
    answer(&mut game, 1, Response::Modes { mode: 2 });
    let Request::Targets {
        decision_id,
        candidates,
        counter_division: Some(division),
        ..
    } = request(&mut game, 1)
    else {
        panic!("the chosen mode announces its divided creature targets");
    };
    assert_eq!(candidates, [first, second]);
    assert_eq!(
        division,
        CounterDivision {
            count: 2.into(),
            minimum_targets: 1,
            maximum_targets: 2,
        }
    );
    for invalid in [vec![], vec![first, first]] {
        assert!(
            game.check_response(caster, decision_id, Response::Targets { objects: invalid })
                .is_err()
        );
    }
    let QueryResult::Complete(QueryValue::Decision(Some(PlayerRequest::Targets {
        token,
        counter_division,
        candidates,
        ..
    }))) = game.player_decision_for(caster, Budget::unlimited())
    else {
        panic!("expected the public divided-target offer");
    };
    assert_eq!(counter_division, Some(division));
    let QueryResult::Complete(QueryValue::Validated(receipt)) = game.check_player_response(
        caster,
        token,
        PlayerResponse::Targets {
            objects: candidates,
        },
        Budget::unlimited(),
    ) else {
        panic!("the two distinct public targets fix one share each");
    };
    game.submit(receipt).unwrap();
    for land in ["Plains", "Swamp", "Forest"] {
        fund(&mut game, 1, land, 1);
    }
    answer(&mut game, 1, Response::FinishMana { undo_mana: false });
    answer(
        &mut game,
        1,
        Response::SpendMana {
            spending: BTreeMap::from([
                (Color::W, 1.into()),
                (Color::B, 1.into()),
                (Color::G, 1.into()),
            ]),
        },
    );
    assert!(matches!(request(&mut game, 1), Request::Priority { .. }));
    let stack_id = game.top().unwrap().id;
    let mut lost_target = Game::restore(prepared, &game.snapshot().unwrap()).unwrap();

    resolve(&mut game, 1, |game| {
        if matches!(game.state.rules.frame, Frame::CounterPlacement { .. }) {
            assert_eq!(game.current(first).unwrap().plus_one_counters, 0.into());
            assert_eq!(game.current(second).unwrap().plus_one_counters, 0.into());
        }
    });
    assert_eq!(game.current(first).unwrap().plus_one_counters, 2.into());
    assert_eq!(game.current(second).unwrap().plus_one_counters, 1.into());
    let placements = batches(&game)
        .into_iter()
        .filter(|batch| {
            matches!(
                batch.cause,
                BatchCause::Instruction { stack_id: id, source_block: 0, .. } if id == stack_id
            ) && matches!(batch.group, EventGroup::Simultaneous { .. })
        })
        .collect::<Vec<_>>();
    let [batch] = placements.as_slice() else {
        panic!("both counter placements commit in one occurrence group");
    };
    assert_eq!(
        batch.group,
        EventGroup::Simultaneous {
            occurrences: vec![
                ProposedEvent::PutPlusOneCounters {
                    object: first,
                    amount: 2.into()
                },
                ProposedEvent::PutPlusOneCounters {
                    object: second,
                    amount: 1.into()
                },
            ],
        }
    );
    recover(&game);

    until_player(&mut lost_target, 1, PlayerId(1));
    announce(
        &mut lost_target,
        1,
        4,
        vec![TargetEntity::Object { object: second }],
    );
    resolve(&mut lost_target, 1, |_| {});
    assert_eq!(
        lost_target.current(first).unwrap().plus_one_counters,
        2.into()
    );
    assert_eq!(
        lost_target.zone(object(&lost_target, 2)),
        Some(Zone::Hand { owner: PlayerId(1) })
    );
    recover(&lost_target);
}
