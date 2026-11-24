//! Full Royal Assassin source, actual creature casting and accepted responses.
//! Private configuration, not pod admission; no tap/readiness state injection.
//! Pinned CR113.7a,302.6,602.2b,608.2b,701.26a:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_core::testkit::{Feature, ProfileError};

fn prepared() -> Prepared {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    for raw in [
        include_str!(
            "../../../../../tests/fixtures/phase1-r4/9ed6f28f-a3db-48c5-9ab0-b90a7fba5f57.json"
        ),
        include_str!(
            "../../../../../tests/fixtures/phase1-r4/2fa763ad-4d94-4c3a-a099-13ac22c09ce4.json"
        ),
        include_str!(
            "../../../../../tests/fixtures/phase1-r4/b795ecfd-31ac-4a2a-9679-97b088932d93.json"
        ),
    ] {
        let mtg_compiler::CompileResult::Lowered { definition } = mtg_witness::compile(
            include_str!("../../../../../tests/fixtures/archive-manifest-phase1-r4.json"),
            raw,
        ) else {
            panic!("complete tap-source destruction witness must lower");
        };
        if definition.name == "Royal Assassin" {
            assert!(
                definition
                    .capabilities
                    .contains(&Capability::TapSourceDestroyV1)
            );
            assert_eq!(definition.activated[0].cost, ActivationCost::TapSource);
        }
        if let Some(existing) = catalog.definition_by_oracle_id(&definition.oracle_id) {
            assert_eq!(existing, definition.as_ref());
        } else {
            catalog.definitions.push(*definition);
        }
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    let mut board =
        Board::new(catalog, "tap-source-destroy-source-witness").step(TurnStep::PrecombatMain);
    for (name, seat, hand) in [
        ("Royal Assassin", 0, true),
        ("Swamp", 0, false),
        ("Swamp", 0, false),
        ("Swamp", 0, false),
        ("Llanowar Elves", 1, false),
        ("Grizzly Bears", 1, false),
        ("Island", 1, false),
        ("Burst of Energy", 1, true),
        ("Plains", 1, false),
        ("Regress", 2, true),
        ("Island", 2, false),
        ("Island", 2, false),
        ("Island", 2, false),
        ("Llanowar Elves", 1, true),
    ] {
        board = board.permanent_or_hand(name, seat, seat, hand);
    }
    board.libraries("Forest", 4).prepare()
}
fn passive(g: &mut Game, quantum: u64) {
    let r = Driver::request(g, quantum);
    let response = match &r {
        Decision {
            reason: Reason::Priority,
            ..
        } => Answer::Pick {
            picks: vec![vec![0]],
        },
        Decision {
            reason: Reason::Attackers,
            ..
        } => r.attack(vec![]),
        Decision {
            reason: Reason::Blockers,
            ..
        } => r.block(vec![]),
        found @ Decision {
            reason: Reason::Discard,
            ..
        } => {
            let count = &found.required_count();
            let candidates = found.objects_offered();
            r.pick_objects(candidates[..*count].to_vec())
        }
        other => panic!("unexpected aging request {other:?}"),
    };
    g.respond(r.player, r.id, response).unwrap();
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
fn pay_spell(g: &mut Game, quantum: u64, lands: &[u32], color: Color) {
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
                unrestricted: BTreeMap::from([(color, Quantity::from(lands.len() as i64))]),
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
    panic!("spell completion bound");
}
fn reject(g: &mut Game, r: &Decision<Rules>, player: PlayerId, response: Answer) {
    let before = g.snapshot().unwrap();
    assert!(g.respond(player, r.id, response).is_err());
    assert_eq!(g.snapshot().unwrap(), before);
}
fn tap_elf(g: &mut Game, quantum: u64) {
    priority(g, quantum, 1);
    let elf = Driver::object(g, 4);
    assert!(!g.current(elf).unwrap().tapped);
    Driver::answer(g, quantum, g.next_test_decision().activate_mana(elf));
    Driver::request(g, quantum);
    assert!(g.current(elf).unwrap().tapped);
    priority(g, quantum, 0);
}
fn cast_and_age(g: &mut Game, quantum: u64) -> ObjectId {
    Driver::answer(g, quantum, g.next_test_decision().cast(CardId(0)));
    pay_spell(g, quantum, &[1, 2, 3], Color::B);
    assert_eq!(
        g.top().unwrap().paid,
        Some(ManaCost {
            generic: 1.into(),
            colored: vec![Color::B, Color::B]
        })
    );
    finish_to(g, quantum, 0);
    let royal = Driver::object(g, 0);
    assert!(!g.current(royal).unwrap().attack_ready);
    tap_elf(g, quantum);
    let r = Driver::request(g, quantum);
    let before = g.snapshot().unwrap();
    let error = g
        .respond(
            r.player,
            r.id,
            g.next_test_decision().select(Candidate::Action {
                action: Action::ActivateAbility {
                    object: royal,
                    ability: 0,
                },
            }),
        )
        .unwrap_err();
    assert!(error.contains("invalid slot selection"));
    assert_eq!(g.snapshot().unwrap(), before);

    let turn = g.turn().number;
    for _ in 0..300 {
        let r = Driver::request(g, quantum);
        if g.turn().number > turn
            && g.active_player() == Some(PlayerId(0))
            && g.turn().step == TurnStep::PrecombatMain
            && matches!(
                r,
                Decision {
                    reason: Reason::Priority,
                    ..
                }
            )
        {
            break;
        }
        passive(g, quantum);
    }
    assert!(g.turn().number > turn);
    assert!(g.current(royal).unwrap().attack_ready);
    tap_elf(g, quantum);
    royal
}
fn activate(g: &mut Game, quantum: u64, royal: ObjectId, target: ObjectId) -> StackId {
    Driver::answer(
        g,
        quantum,
        g.next_test_decision().select(Candidate::Action {
            action: Action::ActivateAbility {
                object: royal,
                ability: 0,
            },
        }),
    );
    let r = Driver::request(g, quantum);
    assert!(
        matches!(&r,found @ Decision {reason:Reason::Targets{..},..} if found.objects_offered().contains(&target) && !found.objects_offered().contains(&Driver::object(g,5)) && !found.objects_offered().contains(&Driver::object(g,6)) && !found.objects_offered().contains(&Driver::object(g,13)))
    );
    for id in [5, 6, 13] {
        let invalid = Driver::object(g, id);
        reject(
            g,
            &r,
            r.player,
            g.next_test_decision().targets(vec![invalid]),
        );
    }
    let QueryResult::Complete(QueryValue::Decision(Some(Decision {
        id: token,
        reason: Reason::Targets { .. },
        ..
    }))) = g.player_decision_for(PlayerId(0), Budget::unlimited())
    else {
        panic!("public target decision");
    };
    let before = g.snapshot().unwrap();
    assert!(matches!(
        g.check_player_response(
            PlayerId(0),
            token,
            g.next_test_viewer_decision()
                .targets(vec![ObjectHandle("foreign".into())]),
            Budget::unlimited()
        ),
        QueryResult::Rejected(QueryRejection::InvalidResponse)
    ));
    assert_eq!(g.snapshot().unwrap(), before);
    Driver::answer(g, quantum, g.next_test_decision().targets(vec![target]));
    // The generic object-target announcement currently uses a zero-mana
    // continuation. If it is completed directly, the same paid tap is required.
    if matches!(
        Driver::request(g, quantum),
        Decision {
            reason: Reason::Mana { .. },
            ..
        }
    ) {
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
                    unrestricted: BTreeMap::new(),
                },
            },
        );
    }
    assert!(matches!(
        Driver::request(g, quantum),
        Decision {
            reason: Reason::Priority,
            ..
        }
    ));
    assert!(
        g.current(royal).unwrap().tapped,
        "the printed tap cost must actually be paid"
    );
    let stack = g.top().unwrap().id;
    assert_eq!(
        g.top().unwrap().paid,
        Some(ManaCost {
            generic: 0.into(),
            colored: vec![]
        })
    );
    let r = Driver::request(g, quantum);
    reject(
        g,
        &r,
        PlayerId(0),
        g.next_test_decision().select(Candidate::Action {
            action: Action::ActivateAbility {
                object: royal,
                ability: 0,
            },
        }),
    );

    stack
}
fn resolve_start(g: &mut Game) {
    for _ in 0..30 {
        if matches!(&g.state.rules.frame,Frame::Resolve { frame } if frame.cursor==0) {
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
            other => panic!("resolution boundary {other:?}"),
        }
    }
    panic!("resolution boundary bound");
}
#[derive(Clone, Copy)]
enum ResponseCase {
    None,
    UntapTarget,
    BounceSource,
}
fn run(case: ResponseCase, quantum: u64, mode: TraceMode) -> Game {
    let mut g = Game::with_trace(prepared(), mode);
    let royal = cast_and_age(&mut g, quantum);
    let target = Driver::object(&g, 4);
    let ability = activate(&mut g, quantum, royal, target);
    match case {
        ResponseCase::None => (),
        ResponseCase::UntapTarget => {
            priority(&mut g, quantum, 1);
            {
                let selected_answer = g.next_test_decision().cast(CardId(7));
                Driver::answer(&mut g, quantum, selected_answer)
            };
            {
                let selected_answer = g.next_test_decision().targets(vec![target]);
                Driver::answer(&mut g, quantum, selected_answer)
            };
            pay_spell(&mut g, quantum, &[8], Color::W);
            finish_to(&mut g, quantum, 1);
            assert!(!g.current(target).unwrap().tapped);
        }
        ResponseCase::BounceSource => {
            priority(&mut g, quantum, 2);
            {
                let selected_answer = g.next_test_decision().cast(CardId(9));
                Driver::answer(&mut g, quantum, selected_answer)
            };
            {
                let selected_answer = g.next_test_decision().targets(vec![royal]);
                Driver::answer(&mut g, quantum, selected_answer)
            };
            pay_spell(&mut g, quantum, &[10, 11, 12], Color::U);
            finish_to(&mut g, quantum, 1);
            assert!(g.current(royal).is_err());
            assert_eq!(
                g.current(Driver::object(&g, 0)).unwrap().zone,
                Zone::Hand { owner: PlayerId(0) }
            );
        }
    }

    resolve_start(&mut g);
    let legal = !matches!(case, ResponseCase::UntapTarget);
    assert!(
        matches!(&g.state.rules.frame,Frame::Resolve { frame } if frame.stack_id==ability && frame.legal_targets==[legal])
    );

    finish_to(&mut g, quantum, 0);
    if legal {
        assert!(g.current(target).is_err());
        assert_eq!(
            g.current(Driver::object(&g, 4)).unwrap().zone,
            Zone::Graveyard { owner: PlayerId(1) }
        );
    } else {
        assert_eq!(g.current(target).unwrap().zone, Zone::Battlefield);
        assert!(!g.current(target).unwrap().tapped);
    }
    if !matches!(case, ResponseCase::BounceSource) {
        assert!(g.current(royal).unwrap().tapped);
    }
    if mode == TraceMode::Detailed {
        assert_eq!(g.events().iter().filter(|e|matches!(e,Event::Committed { actual:EventOutcome::Tapped { object },.. } if *object==royal)).count(),1);
        assert!(
            g.events()
                .iter()
                .any(|e| matches!(e,Event::AbilityCompleted { stack_id } if *stack_id==ability))
        );
    }

    g
}
fn exercise(case: ResponseCase) {
    run(case, u64::MAX, TraceMode::Detailed);
}
#[test]
fn tap_source_destroy_cast_creature_ages_then_pays_tap_and_destroys_real_tapped_elf() {
    exercise(ResponseCase::None);
}
#[test]
fn tap_source_destroy_real_burst_untaps_target_and_fizzles_with_cost_spent() {
    exercise(ResponseCase::UntapTarget);
}
#[test]
fn tap_source_destroy_paid_ability_survives_real_regress_source_bounce() {
    exercise(ResponseCase::BounceSource);
}
#[test]
fn tap_source_destroy_new_capability_remains_outside_host_and_search() {
    let p = prepared();
    let mut c = p.catalog.as_ref().clone();
    c.definitions.retain(|d| d.name == "Royal Assassin");
    c.validate().unwrap();
    for feature in [Feature::Hosting, Feature::Search] {
        let mut profile = match_profile("commander-foundations-v1").unwrap().clone();
        profile.hosted = false;
        profile.search = false;
        match feature {
            Feature::Hosting => profile.hosted = true,
            Feature::Search => profile.search = true,
        }
        assert!(
            matches!(profile.validate_requested_features(&c),Err(ProfileError::UnreviewedCapabilities { feature:actual,capabilities }) if actual==feature && capabilities.contains(&Capability::TapSourceDestroyV1))
        );
    }
}
