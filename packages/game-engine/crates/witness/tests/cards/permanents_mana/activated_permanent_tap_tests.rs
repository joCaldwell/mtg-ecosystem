//! Complete Icy Manipulator/Regress sources in private configurations, not pods.
//! All tap states, costs and zone changes below use accepted runtime inputs.
//! Pinned CR113.7a,602.1a/2b,608.2b,609.3,701.26a:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

fn prepared(rows: &[(&str, u8, bool)]) -> Prepared {
    prepared_with_catalog(rows, tap_catalog())
}

fn tap_catalog() -> Catalog {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    for raw in [
        include_str!(
            "../../../../../tests/fixtures/phase1-r5/3608f1f7-8dc5-4dd1-ae91-c830e1de9529.json"
        ),
        include_str!(
            "../../../../../tests/fixtures/phase1-r5/2fa763ad-4d94-4c3a-a099-13ac22c09ce4.json"
        ),
    ] {
        let mtg_compiler::CompileResult::Lowered { definition } = mtg_witness::compile(
            include_str!("../../../../../tests/fixtures/archive-manifest-phase1-r5.json"),
            raw,
        ) else {
            panic!("complete activated tap witness source must lower");
        };
        if definition.name == "Icy Manipulator" {
            assert!(
                definition
                    .capabilities
                    .contains(&Capability::ActivatedTapPermanentV1)
            );
            assert!(
                definition
                    .capabilities
                    .contains(&Capability::TapPermanentV1)
            );
            assert_eq!(definition.activated.len(), 1);
        }
        if let Some(existing) = catalog.definition_by_oracle_id(&definition.oracle_id) {
            assert_eq!(existing, definition.as_ref());
        } else {
            catalog.definitions.push(*definition);
        }
    }
    catalog
}

fn prepared_with_catalog(rows: &[(&str, u8, bool)], mut catalog: Catalog) -> Prepared {
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    let mut board =
        Board::new(catalog, "activated-permanent-tap-source-witness").step(TurnStep::PrecombatMain);
    for &(name, seat, hand) in rows {
        board = board.permanent_or_hand(name, seat, seat, hand);
    }
    board.libraries("Forest", 4).prepare()
}
fn priority(g: &mut Game, quantum: u64, seat: u8) {
    for _ in 0..8 {
        let r = Driver::request(g, quantum);
        assert!(matches!(
            r,
            Decision {
                reason: Reason::Priority,
                ..
            }
        ));
        if r.player == PlayerId(seat) {
            return;
        }
        Driver::answer(
            g,
            quantum,
            Answer::Pick {
                picks: vec![vec![0]],
            },
        );
    }
    panic!("priority bound");
}
fn pay(g: &mut Game, quantum: u64, target: ObjectId, lands: &[u32]) -> StackId {
    Driver::answer(g, quantum, g.next_test_decision().targets(vec![target]));
    for id in lands {
        let land = Driver::object(g, *id);
        Driver::answer(g, quantum, g.next_test_decision().activate_mana(land));
    }
    Driver::answer(
        g,
        quantum,
        Answer::Pay {
            pay: PayAnswer::Finish {
                undo_on_failure: false,
            },
        },
    );
    assert!(matches!(
        Driver::request(g, quantum),
        Decision {
            reason: Reason::SpendMana { .. },
            ..
        }
    ));
    Driver::answer(
        g,
        quantum,
        Answer::Pay {
            pay: PayAnswer::Spend {
                unrestricted: BTreeMap::from([(Color::U, Quantity::from(lands.len() as i64))]),
            },
        },
    );
    assert!(matches!(
        Driver::request(g, quantum),
        Decision {
            reason: Reason::Priority,
            ..
        }
    ));
    g.top().unwrap().id
}
fn activate(g: &mut Game, quantum: u64, target: ObjectId) -> StackId {
    let source = Driver::object(g, 0);
    priority(g, quantum, 0);
    Driver::answer(
        g,
        quantum,
        g.next_test_decision().select(Candidate::Action {
            action: Action::ActivateAbility {
                object: source,
                ability: 0,
            },
        }),
    );
    assert!(matches!(
        Driver::request(g, quantum),
        Decision {
            reason: Reason::Targets { .. },
            ..
        }
    ));
    let stack = pay(g, quantum, target, &[1]);
    assert_eq!(
        g.top().unwrap().paid,
        Some(ManaCost {
            generic: 1.into(),
            colored: vec![]
        })
    );
    assert!(g.current(source).unwrap().tapped);
    assert!(g.current(Driver::object(g, 1)).unwrap().tapped);
    assert_eq!(g.mana(PlayerId(0), Color::U), Some(Quantity::zero()));
    stack
}
fn finish_to(g: &mut Game, quantum: u64, remaining: usize) {
    for _ in 0..30 {
        assert!(matches!(
            Driver::request(g, quantum),
            Decision {
                reason: Reason::Priority,
                ..
            }
        ));
        if g.state.rules.stack.len() == remaining {
            return;
        }
        Driver::answer(
            g,
            quantum,
            Answer::Pick {
                picks: vec![vec![0]],
            },
        );
    }
    panic!("activated tap completion bound");
}
fn resolve_start(g: &mut Game) {
    for _ in 0..30 {
        if matches!(&g.state.rules.frame,Frame::Resolve { frame } if frame.cursor == 0) {
            return;
        }
        match g.advance(Budget::new(1)) {
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(r)
                if matches!(
                    r,
                    Decision {
                        reason: Reason::Priority,
                        ..
                    }
                ) =>
            {
                g.respond(
                    r.player,
                    r.id,
                    Answer::Pick {
                        picks: vec![vec![0]],
                    },
                )
                .unwrap();
            }
            other => panic!("activation resolution boundary {other:?}"),
        }
    }
    panic!("activation resolution boundary work bound");
}
fn reject(g: &mut Game, r: &Decision<Rules>, player: PlayerId, response: Answer) {
    let before = g.snapshot().unwrap();
    assert!(g.respond(player, r.id, response).is_err());
    assert_eq!(g.snapshot().unwrap(), before);
}
fn taps(g: &Game, target: ObjectId) -> usize {
    g.events().iter().filter(|e|matches!(e,Event::Committed { actual:EventOutcome::Tapped { object },.. } if *object == target)).count()
}
fn completed(g: &Game, stack: StackId) {
    if g.trace.mode == TraceMode::Detailed {
        assert!(
            g.events()
                .iter()
                .any(|e| matches!(e,Event::AbilityCompleted { stack_id } if *stack_id == stack))
        );
    }
}
#[test]
fn activated_permanent_tap_remains_outside_reviewed_hosting_and_search() {
    use mtg_core::testkit::{Feature, ProfileError};
    let p = prepared(&[("Icy Manipulator", 0, false), ("Island", 0, false)]);
    let profile = mtg_core::testkit::match_profile("commander-foundations-v1").unwrap();
    for feature in [Feature::Hosting, Feature::Search] {
        assert!(matches!(
            profile.require_feature(&p.catalog, feature),
            Err(ProfileError::UnreviewedCapabilities { feature: actual, capabilities })
                if actual == feature && capabilities.contains(&Capability::ActivatedTapPermanentV1)
        ));
    }
}
fn foreign_handle_rejected(g: &Game) {
    let QueryResult::Complete(QueryValue::Decision(Some(Decision {
        id: token,
        reason: Reason::Targets { .. },
        ..
    }))) = g.player_decision_for(PlayerId(0), Budget::unlimited())
    else {
        panic!("own target decision");
    };
    let QueryResult::Complete(QueryValue::Observation(foreign)) =
        g.observe(PlayerId(1), Budget::unlimited())
    else {
        panic!("foreign observation");
    };
    let icy = g.definition(CardId(0)).unwrap().oracle_id.clone();
    let handle = foreign
        .view
        .objects
        .iter()
        .find(|o| o.oracle_id == icy && o.zone == Zone::Battlefield)
        .unwrap()
        .handle
        .clone();
    let before = g.snapshot().unwrap();
    assert!(matches!(
        g.check_player_response(
            PlayerId(0),
            token,
            g.next_test_viewer_decision().targets(vec![handle]),
            Budget::unlimited()
        ),
        QueryResult::Rejected(QueryRejection::InvalidResponse)
    ));
    assert_eq!(g.snapshot().unwrap(), before);
}

#[test]
fn activated_permanent_tap_union_targets_pay_cost_and_reject_ineligible_objects_atomically() {
    for name in ["Sol Ring", "Grizzly Bears", "Island"] {
        let p = prepared(&[
            ("Icy Manipulator", 0, false),
            ("Island", 0, false),
            (name, 2, false),
            ("Glorious Anthem", 2, false),
            ("Sol Ring", 1, true),
        ]);
        let quantum = u64::MAX;
        let mode = TraceMode::Detailed;
        let mut g = Game::with_trace(p.clone(), mode);
        let source = Driver::object(&g, 0);
        let target = Driver::object(&g, 2);
        {
            let selected_answer = g.next_test_decision().select(Candidate::Action {
                action: Action::ActivateAbility {
                    object: source,
                    ability: 0,
                },
            });
            Driver::answer(&mut g, quantum, selected_answer)
        };
        let r = Driver::request(&mut g, quantum);
        assert!(
            matches!(&r,found @ Decision {reason:Reason::Targets{..},..} if found.objects_offered().contains(&source) && found.objects_offered().contains(&Driver::object(&g,1)) && found.objects_offered().contains(&target) && !found.objects_offered().contains(&Driver::object(&g,3)) && !found.objects_offered().contains(&Driver::object(&g,4)))
        );
        for id in [3, 4] {
            let invalid = Driver::object(&g, id);
            {
                let selected = g.next_test_decision().targets(vec![invalid]);
                reject(&mut g, &r, r.player, selected)
            };
        }
        {
            let selected = g.next_test_decision().targets(vec![target]);
            reject(&mut g, &r, PlayerId(1), selected)
        };
        foreign_handle_rejected(&g);

        let stack = pay(&mut g, quantum, target, &[1]);
        assert!(g.current(source).unwrap().tapped);
        assert_eq!(
            g.top().unwrap().paid,
            Some(ManaCost {
                generic: 1.into(),
                colored: vec![]
            })
        );

        let r = Driver::request(&mut g, quantum);
        {
            let selected = g.next_test_decision().select(Candidate::Action {
                action: Action::ActivateAbility {
                    object: source,
                    ability: 0,
                },
            });
            reject(&mut g, &r, PlayerId(0), selected)
        };
        finish_to(&mut g, quantum, 0);
        completed(&g, stack);
        assert!(g.current(target).unwrap().tapped);
        if mode == TraceMode::Detailed {
            assert_eq!(taps(&g, source), 1);
            assert_eq!(taps(&g, target), 1);
        }
    }
}
#[test]
fn paid_creature_source_tap_checks_target_and_pays_before_resolution() {
    let record = include_str!(
        "../../../../../tests/fixtures/phase1-r5/00d11662-9d10-4915-b80c-c1ad54b4e71b.json"
    );
    let manifest = include_str!("../../../../../tests/fixtures/archive-manifest-phase1-r5.json");
    assert_eq!(
        sha256(record.trim_end().as_bytes()),
        "5ce182cb8d8d5fa9a0d967a7de818bd7dc275fcb911aea86f150a42e4299fcd7"
    );
    let mtg_compiler::CompileResult::Lowered { definition } =
        mtg_witness::compile(manifest, record)
    else {
        panic!("complete Half-Elf Monk source must lower");
    };
    assert!(
        definition
            .capabilities
            .contains(&Capability::ActivatedCreatureTapV1)
    );
    assert!(
        !definition
            .capabilities
            .contains(&Capability::ActivatedTapPermanentV1)
    );
    let mut catalog = tap_catalog();
    if let Some(existing) = catalog.definition_by_oracle_id(&definition.oracle_id) {
        assert_eq!(existing, definition.as_ref());
    } else {
        catalog.definitions.push(*definition);
    }
    let p = prepared_with_catalog(
        &[
            ("Half-Elf Monk", 0, false),
            ("Plains", 0, false),
            ("Plains", 0, false),
            ("Grizzly Bears", 2, false),
            ("Island", 2, false),
        ],
        catalog,
    );
    let mut game = Game::with_trace(p, TraceMode::Detailed);
    let source = Driver::object(&game, 0);
    let target = Driver::object(&game, 3);
    let noncreature = Driver::object(&game, 4);
    const WORK: u64 = 4096;
    {
        let selected_answer = game.next_test_decision().select(Candidate::Action {
            action: Action::ActivateAbility {
                object: source,
                ability: 0,
            },
        });
        Driver::answer(&mut game, WORK, selected_answer)
    };
    let targets = Driver::request(&mut game, WORK);
    assert!(
        matches!(&targets,found @ Decision {reason:Reason::Targets{..},..}
        if found.objects_offered().contains(&target) && !found.objects_offered().contains(&noncreature))
    );
    {
        let selected = game.next_test_decision().targets(vec![noncreature]);
        reject(&mut game, &targets, PlayerId(0), selected)
    };
    {
        let selected_answer = game.next_test_decision().targets(vec![target]);
        Driver::answer(&mut game, WORK, selected_answer)
    };
    for card in [1, 2] {
        let land = Driver::object(&game, card);
        {
            let selected_answer = game.next_test_decision().activate_mana(land);
            Driver::answer(&mut game, WORK, selected_answer)
        };
    }
    {
        let selected_answer = Answer::Pay {
            pay: PayAnswer::Finish {
                undo_on_failure: false,
            },
        };
        Driver::answer(&mut game, WORK, selected_answer)
    };
    {
        let selected_answer = Answer::Pay {
            pay: PayAnswer::Spend {
                unrestricted: BTreeMap::from([(Color::W, 2.into())]),
            },
        };
        Driver::answer(&mut game, WORK, selected_answer)
    };
    assert!(matches!(
        Driver::request(&mut game, WORK),
        Decision {
            reason: Reason::Priority,
            ..
        }
    ));
    assert_eq!(
        game.top().unwrap().paid,
        Some(ManaCost {
            generic: 1.into(),
            colored: vec![Color::W]
        })
    );
    assert!(game.current(source).unwrap().tapped);
    assert!(!game.current(target).unwrap().tapped);
    let stack = game.top().unwrap().id;
    finish_to(&mut game, WORK, 0);
    completed(&game, stack);
    assert!(game.current(target).unwrap().tapped);
    assert_eq!(taps(&game, source), 1);
    assert_eq!(taps(&game, target), 1);
}

#[test]
fn activated_permanent_tap_self_target_cost_taps_once_and_resolution_is_noop() {
    let p = prepared(&[("Icy Manipulator", 0, false), ("Island", 0, false)]);
    let quantum = u64::MAX;
    let mode = TraceMode::Detailed;
    let mut g = Game::with_trace(p.clone(), mode);
    let source = Driver::object(&g, 0);
    let stack = activate(&mut g, quantum, source);

    finish_to(&mut g, quantum, 0);
    completed(&g, stack);
    assert!(g.current(source).unwrap().tapped);
    if mode == TraceMode::Detailed {
        assert_eq!(taps(&g, source), 1);
    }
}
fn bounce_case(source_bounce: bool, quantum: u64, mode: TraceMode) -> Game {
    let mut g = Game::with_trace(
        prepared(&[
            ("Icy Manipulator", 0, false),
            ("Island", 0, false),
            ("Grizzly Bears", 2, false),
            ("Regress", 1, true),
            ("Island", 1, false),
            ("Island", 1, false),
            ("Island", 1, false),
        ]),
        mode,
    );
    let source = Driver::object(&g, 0);
    let target = Driver::object(&g, 2);
    let ability = activate(&mut g, quantum, target);

    priority(&mut g, quantum, 1);
    {
        let selected_answer = g.next_test_decision().cast(CardId(3));
        Driver::answer(&mut g, quantum, selected_answer)
    };
    let bounced = if source_bounce { source } else { target };
    pay(&mut g, quantum, bounced, &[4, 5, 6]);
    finish_to(&mut g, quantum, 1);
    assert!(g.current(bounced).is_err());
    let successor = Driver::object(&g, bounced.card.0);
    assert_eq!(
        g.current(successor).unwrap().zone,
        Zone::Hand {
            owner: if source_bounce {
                PlayerId(0)
            } else {
                PlayerId(2)
            }
        }
    );

    resolve_start(&mut g);
    assert!(
        matches!(&g.state.rules.frame,Frame::Resolve { frame } if frame.stack_id == ability && frame.legal_targets == [source_bounce])
    );

    finish_to(&mut g, quantum, 0);
    completed(&g, ability);
    if source_bounce {
        assert!(g.current(target).unwrap().tapped);
    } else {
        assert!(g.current(source).unwrap().tapped);
    }
    if mode == TraceMode::Detailed {
        assert_eq!(taps(&g, source), 1);
        assert_eq!(taps(&g, target), usize::from(source_bounce));
    }

    g
}
#[test]
fn activated_permanent_tap_paid_ability_survives_real_regress_source_bounce() {
    let a = bounce_case(true, 1, TraceMode::Detailed);
    let b = bounce_case(true, 4096, TraceMode::Off);
    assert_eq!(a.state, b.state);
    assert_eq!(a.replay_journal(), b.replay_journal());
}
#[test]
fn activated_permanent_tap_real_target_bounce_fizzles_without_refunding_source_cost() {
    let a = bounce_case(false, 1, TraceMode::Detailed);
    let b = bounce_case(false, 4096, TraceMode::Off);
    assert_eq!(a.state, b.state);
    assert_eq!(a.replay_journal(), b.replay_journal());
}
