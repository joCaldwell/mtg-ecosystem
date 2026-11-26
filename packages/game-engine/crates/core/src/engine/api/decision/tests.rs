use super::*;
use mtg_contracts::{Color, ManaCost, Quantity};

fn slot(subject: u32) -> Slot {
    Slot {
        role: SlotRole::BlockWith,
        subject: Some(subject),
        allowed: vec![2, 3],
        min: 0,
        max: 1,
        repeat: false,
    }
}

#[test]
fn picks_preserve_slots_and_enforce_joint_cardinality() {
    let mut shape = Shape::Pick {
        slots: vec![slot(0), slot(1)],
        rules: vec![JointRule::PickedAtLeast {
            candidates: vec![2],
            count: 2,
        }],
    };
    for a in [None, Some(2), Some(3), Some(4)] {
        for b in [None, Some(2), Some(3), Some(4)] {
            let picks = vec![a.into_iter().collect(), b.into_iter().collect()];
            assert_eq!(
                shape.check(&Answer::Pick { picks }).is_ok(),
                a != Some(4) && b != Some(4) && (a == Some(2)) == (b == Some(2))
            );
        }
    }
    let Shape::Pick { rules, .. } = &mut shape else {
        unreachable!()
    };
    *rules = vec![
        JointRule::Distinct { slots: vec![0, 1] },
        JointRule::Total { min: 2, max: 2 },
    ];
    for (picks, valid) in [
        (vec![vec![2], vec![3]], true),
        (vec![vec![3], vec![3]], false),
        (vec![vec![], vec![3]], false),
        (vec![vec![2, 3], vec![]], false),
        (vec![vec![2]], false),
    ] {
        assert_eq!(shape.check(&Answer::Pick { picks }).is_ok(), valid);
    }
}

#[test]
fn distribution_requires_lethal_before_damage_to_player() {
    let shape = Shape::Distribute {
        total: 5.into(),
        buckets: [9, 4]
            .map(|candidate| Bucket {
                candidate,
                min: 0.into(),
                max: 5.into(),
            })
            .into(),
        gates: vec![Gate {
            bucket: 1,
            requires: vec![(0, 3.into())],
        }],
    };
    for blocker in -1..=6 {
        for player in -1..=6 {
            let amounts = vec![blocker.into(), player.into()];
            assert_eq!(
                shape.check(&Answer::Distribute { amounts }).is_ok(),
                blocker >= 0
                    && player >= 0
                    && blocker + player == 5
                    && (player == 0 || blocker >= 3)
            );
        }
    }
}

#[test]
fn order_and_number_are_exact() {
    let shape = Shape::Order {
        items: vec![1, 4, 3],
        min: 2,
        max: 2,
    };
    for (order, valid) in [
        (vec![4, 1], true),
        (vec![1, 4], true),
        (vec![3, 3], false),
        (vec![4], false),
        (vec![1, 2], false),
    ] {
        assert_eq!(shape.check(&Answer::Order { order }).is_ok(), valid);
    }
    let huge: Quantity = "999999999999999999999999999999".parse().unwrap();
    let shape = Shape::Number {
        min: huge.clone(),
        max: None,
    };
    assert!(shape.check(&Answer::Number { value: huge }).is_ok());
    assert!(shape.check(&Answer::Number { value: 1.into() }).is_err());
}

#[test]
fn payment_separates_available_sources_from_exact_expenditure() {
    let mut shape = Shape::Pay {
        cost: ManaCost {
            generic: 1.into(),
            colored: vec![Color::G],
        },
        pool: ManaPool {
            unrestricted: [(Color::G, 2.into()), (Color::R, 3.into())].into(),
            restricted: vec![],
        },
        step: PayStep::Produce { sources: vec![3] },
    };
    let answer = |pay| Answer::Pay { pay };
    assert!(
        shape
            .check(&answer(PayAnswer::Activate { source: 3 }))
            .is_ok()
    );
    assert!(
        shape
            .check(&answer(PayAnswer::Activate { source: 0 }))
            .is_err()
    );
    assert!(
        shape
            .check(&answer(PayAnswer::Finish {
                undo_on_failure: false
            }))
            .is_ok()
    );
    let Shape::Pay { step, .. } = &mut shape else {
        unreachable!()
    };
    *step = PayStep::Spend;
    assert!(
        shape
            .check(&answer(PayAnswer::Activate { source: 3 }))
            .is_err()
    );
    for (green, red, valid) in [
        (1, 1, true),
        (2, 0, true),
        (0, 2, false),
        (2, 1, false),
        (3, 0, false),
    ] {
        let unrestricted = [(Color::G, green), (Color::R, red)]
            .into_iter()
            .filter(|(_, n)| *n > 0)
            .map(|(color, n)| (color, n.into()))
            .collect();
        assert_eq!(
            shape
                .check(&answer(PayAnswer::Spend { unrestricted }))
                .is_ok(),
            valid
        );
    }
}

#[test]
fn viewer_schema_has_only_viewer_identities_and_strict_answers() {
    let schema = serde_json::to_string(&schemars::schema_for!(Decision<Viewer>)).unwrap();
    let schema_json: serde_json::Value = serde_json::from_str(&schema).unwrap();
    assert_eq!(schema_json["properties"]["token"]["type"], "string");
    let decision = Decision::<Viewer> {
        id: DecisionToken("v0-d2".into()),
        player: PlayerId(0),
        reason: Reason::Priority,
        candidates: vec![Candidate::Action {
            action: Action::PlayLand {
                object: ObjectHandle("v0-o3".into()),
            },
        }],
        shape: Shape::Pick {
            slots: vec![slot(0)],
            rules: vec![],
        },
    };
    let wire = serde_json::to_value(&decision).unwrap();
    assert_eq!(wire["candidates"][0]["action"]["object"], "v0-o3");
    assert_eq!(
        serde_json::from_value::<Decision<Viewer>>(wire).unwrap(),
        decision
    );
    for private in ["ObjectId", "CardId", "EffectKey", "TriggerId"] {
        assert!(!schema.contains(private));
    }
    let round_trip = serde_json::json!({"answer":"pick","picks":[[0],[]]});
    let answer: Answer = serde_json::from_value(round_trip.clone()).unwrap();
    assert_eq!(serde_json::to_value(answer).unwrap(), round_trip);
    assert!(
        serde_json::from_value::<Answer>(
            serde_json::json!({"answer":"pick","picks":[[0]],"object":"v0-o1"})
        )
        .is_err()
    );
}
