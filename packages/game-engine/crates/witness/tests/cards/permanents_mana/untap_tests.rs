//! Genuine archived sources in private witness configurations, not pod/pool
//! admission. All rules changes below use accepted decisions or concessions.
//! Pinned CR506.4b,608.2b,609.3,701.26b,702.11b/18a:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

fn prepared(rows: &[(&str, u8, bool)]) -> Prepared {
    let mut c = FixtureCatalog::Witness.get().clone();
    for (header, raw) in [
        (
            include_str!("../../../../../tests/fixtures/archive-manifest-phase1-r5.json"),
            include_str!(
                "../../../../../tests/fixtures/phase1-r5/b795ecfd-31ac-4a2a-9679-97b088932d93.json"
            ),
        ),
        (
            include_str!("../../../../../tests/fixtures/archive-manifest-phase1-r5.json"),
            include_str!(
                "../../../../../tests/fixtures/phase1-r5/5820d760-8ed7-4abf-a959-c06ed86a3730.json"
            ),
        ),
        (
            include_str!("../../../../../tests/fixtures/archive-manifest-phase1-r5.json"),
            include_str!(
                "../../../../../tests/fixtures/phase1-r5/ba5055ce-6809-47a4-bee6-fb97a5b60f8a.json"
            ),
        ),
    ] {
        let mtg_compiler::CompileResult::Lowered { definition } = mtg_witness::compile(header, raw)
        else {
            panic!("complete archived untap witness source must lower");
        };
        if let Some(existing) = c.definition_by_oracle_id(&definition.oracle_id) {
            assert_eq!(existing, definition.as_ref());
        } else {
            c.definitions.push(*definition);
        }
    }
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    let mut board = Board::new(c, "permanent-untap-source-witness").step(TurnStep::PrecombatMain);
    for &(name, seat, hand) in rows {
        board = board.permanent_or_hand(name, seat, seat, hand);
    }
    board.libraries("Forest", 4).prepare()
}
fn passive(g: &mut Game, quantum: u64) {
    let r = Driver::request(g, quantum);
    let response = match r.clone() {
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
        other => panic!("untap passive {other:?}"),
    };
    g.respond(r.player, r.id, response).unwrap();
}
fn priority(g: &mut Game, quantum: u64, seat: u8) {
    for _ in 0..8 {
        let r = Driver::request(g, quantum);
        assert!(
            matches!(
                r,
                Decision {
                    reason: Reason::Priority,
                    ..
                }
            ),
            "{r:?}"
        );
        if r.player == PlayerId(seat) {
            return;
        }
        passive(g, quantum);
    }
    panic!("untap priority");
}
fn pay(g: &mut Game, quantum: u64, target: ObjectId) {
    for _ in 0..40 {
        let r = Driver::request(g, quantum);
        let response = match &r {
            found @ Decision {
                reason: Reason::Targets { .. },
                ..
            } => {
                let candidates = found.objects_offered();
                {
                    assert!(candidates.contains(&target));
                    r.targets(vec![target])
                }
            }
            Decision {
                shape: Shape::Pay { cost, .. },
                reason: Reason::Mana { .. },
                ..
            } if g.payment_pool(r.player, cost).is_ok() => Answer::Pay {
                pay: PayAnswer::Finish {
                    undo_on_failure: false,
                },
            },
            Decision {
                shape: Shape::Pay { cost, .. },
                reason: Reason::Mana { .. },
                ..
            } => {
                let land = g
                    .state
                    .rules
                    .objects
                    .values()
                    .filter(|o| {
                        o.controller == r.player
                            && o.id != target
                            && o.zone == Zone::Battlefield
                            && !o.tapped
                            && g.definition(o.id.card).unwrap().characteristics.kind
                                == CardKind::Land
                    })
                    .min_by_key(|o| {
                        usize::from(
                            !g.mana_abilities(o.id)
                                .unwrap()
                                .iter()
                                .any(|c| Some(c) == cost.colored.first()),
                        )
                    })
                    .expect("printed mana sources independent of the target");
                r.activate_mana(land.id)
            }
            Decision {
                shape:
                    Shape::Pay {
                        cost,
                        pool:
                            ManaPool {
                                unrestricted: available,
                                ..
                            },
                        ..
                    },
                reason: Reason::SpendMana { .. },
                ..
            } => {
                let mut rest = available.clone();
                let mut spending = BTreeMap::<Color, Quantity>::new();
                for c in &cost.colored {
                    rest.get_mut(c).unwrap().subtract(&1.into());
                    spending.entry(*c).or_default().add(&1.into());
                }
                let mut generic = cost.generic.clone();
                for (c, n) in rest {
                    let used = n.min(generic.clone());
                    generic.subtract(&used);
                    if used > Quantity::zero() {
                        spending.entry(c).or_default().add(&used);
                    }
                }
                assert_eq!(generic, Quantity::zero());
                Answer::Pay {
                    pay: PayAnswer::Spend {
                        unrestricted: spending,
                    },
                }
            }
            Decision {
                reason: Reason::Priority,
                ..
            } => return,
            other => panic!("untap payment {other:?}"),
        };
        g.respond(r.player, r.id, response).unwrap();
    }
    panic!("untap payment bound");
}
fn cast(g: &mut Game, quantum: u64, card: u32, target: ObjectId) -> StackId {
    let seat = g.current(Driver::object(g, card)).unwrap().controller.0;
    priority(g, quantum, seat);
    Driver::answer(g, quantum, g.next_test_decision().cast(CardId(card)));
    pay(g, quantum, target);
    g.state.rules.stack.last().unwrap().id
}
fn resolve_to(g: &mut Game, quantum: u64, len: usize) {
    for _ in 0..80 {
        Driver::request(g, quantum);
        if g.state.rules.stack.len() == len {
            return;
        }
        passive(g, quantum);
    }
    panic!("untap resolution bound");
}
fn resolved(g: &Game, stack: StackId, expected: bool) {
    assert!(
        g.events()
            .iter()
            .any(|e| matches!(e,Event::SpellCompleted {stack_id,resolved}
        if *stack_id==stack && *resolved==expected)),
        "completion {stack:?}={expected}"
    );
}
fn untaps(g: &Game, target: ObjectId) -> usize {
    g.events()
        .iter()
        .filter(|e| {
            matches!(e,Event::Committed {actual:EventOutcome::Untapped {object},..}
        if *object==target)
        })
        .count()
}

#[test]
fn permanent_untap_real_response_preserves_attacking_and_fizzles_swift() {
    let p = prepared(&[
        ("Grizzly Bears", 0, false),
        ("Burst of Energy", 0, true),
        ("Plains", 0, false),
        ("Swift Response", 1, true),
        ("Plains", 1, false),
        ("Plains", 1, false),
    ]);
    let quantum = u64::MAX;
    let mode = TraceMode::Detailed;
    let mut g = Game::with_trace(p.clone(), mode);
    for _ in 0..100 {
        if matches!(
            Driver::request(&mut g, quantum),
            Decision {
                reason: Reason::Attackers,
                ..
            }
        ) {
            break;
        }
        passive(&mut g, quantum);
    }
    let attacker = Driver::object(&g, 0);
    {
        let selected_answer = g.next_test_decision().attack(vec![(attacker, PlayerId(1))]);
        Driver::answer(&mut g, quantum, selected_answer)
    };
    // Responses enqueue input; advance to commit the declaration before
    // inspecting its attack tap.
    Driver::request(&mut g, quantum);
    assert!(g.current(attacker).unwrap().tapped);
    let swift = cast(&mut g, quantum, 3, attacker);
    let burst = cast(&mut g, quantum, 1, attacker);
    assert_eq!(g.state.rules.stack.len(), 2);
    resolve_to(&mut g, quantum, 1);
    assert!(!g.current(attacker).unwrap().tapped);
    assert!(g.current(attacker).unwrap().attack_ready);
    assert!(g.state.rules.combat.iter().any(|a| a.attacker == attacker));
    assert_eq!(g.state.rules.stack[0].id, swift);
    if mode == TraceMode::Detailed {
        resolved(&g, burst, true);
        assert_eq!(untaps(&g, attacker), 1);
    }
    resolve_to(&mut g, quantum, 0);
    assert_eq!(g.current(attacker).unwrap().zone, Zone::Battlefield);
    if mode == TraceMode::Detailed {
        resolved(&g, swift, false);
    }
    for _ in 0..100 {
        let r = Driver::request(&mut g, quantum);
        if g.turn().step == TurnStep::CombatDamage
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
        passive(&mut g, quantum);
    }
    assert_eq!(g.state.rules.players[1].life, 38.into());
    assert!(g.state.rules.combat.iter().any(|a| a.attacker == attacker));
    assert_eq!(
        g.current(Driver::object(&g, 1)).unwrap().zone,
        Zone::Graveyard { owner: PlayerId(0) }
    );
    assert_eq!(
        g.current(Driver::object(&g, 3)).unwrap().zone,
        Zone::Graveyard { owner: PlayerId(1) }
    );
    assert!(g.current(Driver::object(&g, 2)).unwrap().tapped);
    assert!(
        g.current(Driver::object(&g, 4)).unwrap().tapped
            && g.current(Driver::object(&g, 5)).unwrap().tapped
    );
}

#[test]
fn permanent_untap_paid_untapped_land_is_successful_noop_without_false_event() {
    let mut g = Game::with_trace(
        prepared(&[
            ("Plains", 0, false),
            ("Burst of Energy", 0, true),
            ("Plains", 0, false),
        ]),
        TraceMode::Detailed,
    );
    let target = Driver::object(&g, 0);
    let stack = cast(&mut g, 1, 1, target);
    assert!(!g.current(target).unwrap().tapped);
    resolve_to(&mut g, 1, 0);
    assert!(!g.current(target).unwrap().tapped);
    assert!(g.current(Driver::object(&g, 2)).unwrap().tapped);
    assert_eq!(g.mana(PlayerId(0), Color::W), Some(Quantity::zero()));
    resolved(&g, stack, true);
    assert_eq!(untaps(&g, target), 0);
}

#[test]
fn permanent_untap_departure_and_real_bounce_flash_reentry_never_rebind_target() {
    let p = prepared(&[
        ("Research Thief", 1, false),
        ("Burst of Energy", 0, true),
        ("Plains", 0, false),
        ("Unsummon", 1, true),
        ("Island", 1, false),
        ("Island", 1, false),
        ("Island", 1, false),
        ("Island", 1, false),
        ("Island", 1, false),
        ("Island", 1, false),
    ]);
    for departure in [true, false] {
        let mut g = Game::with_trace(p.clone(), TraceMode::Detailed);
        let original = Driver::object(&g, 0);
        let burst = cast(&mut g, 1, 1, original);
        if departure {
            g.concede(PlayerId(1)).unwrap();
            Driver::request(&mut g, 1);
            assert!(g.current(original).is_err());
        } else {
            cast(&mut g, 1, 3, original);
            resolve_to(&mut g, 1, 1);
            assert_eq!(
                g.current(Driver::object(&g, 0)).unwrap().zone,
                Zone::Hand { owner: PlayerId(1) }
            );
            // Research Thief's exact Flash source permits this real reentry
            // while the original Burst remains below it on the stack.
            cast(&mut g, 1, 0, original);
            resolve_to(&mut g, 1, 1);
            assert_ne!(Driver::object(&g, 0), original);
            assert_eq!(
                g.current(Driver::object(&g, 0)).unwrap().zone,
                Zone::Battlefield
            );
            assert!(g.current(original).is_err());
        }
        resolve_to(&mut g, 1, 0);
        resolved(&g, burst, false);
        assert_eq!(untaps(&g, original), 0);
    }
}

#[test]
fn permanent_untap_public_targets_include_untapped_permanents_and_reject_stack_handles() {
    let mut g = Game::new(prepared(&[
        ("Birds of Paradise", 0, false),
        ("Burst of Energy", 0, true),
        ("Plains", 0, false),
    ]));
    let target = Driver::object(&g, 0);
    {
        let selected_answer = g.next_test_decision().activate_mana(target);
        Driver::answer(&mut g, 1, selected_answer)
    };
    {
        let selected_answer = g
            .next_test_decision()
            .select(Candidate::Color { color: Color::G });
        Driver::answer(&mut g, 1, selected_answer)
    };
    {
        let selected_answer = g.next_test_decision().cast(CardId(1));
        Driver::answer(&mut g, 1, selected_answer)
    };
    let r = Driver::request(&mut g, 1);
    let source = Driver::object(&g, 1);
    assert!(
        matches!(&r,found @ Decision {reason:Reason::Targets{..},..} if found.objects_offered().contains(&target)
        && found.objects_offered().contains(&Driver::object(&g,2)) && !found.objects_offered().contains(&source))
    );
    let before = g.snapshot().unwrap();
    assert!(
        g.respond(r.player, r.id, g.next_test_decision().targets(vec![source]))
            .is_err()
    );
    let QueryResult::Complete(QueryValue::Decision(Some(
        found @ Decision {
            reason: Reason::Targets { .. },
            ..
        },
    ))) = g.player_decision_for(PlayerId(0), Budget::unlimited())
    else {
        panic!("untap public targets")
    };
    let token = found.id.clone();
    let candidates = found.objects_offered();
    assert_eq!(candidates.len(), 2);
    for (player, objects) in [
        (PlayerId(1), vec![candidates[0].clone()]),
        (PlayerId(0), vec![ObjectHandle("foreign".into())]),
    ] {
        assert!(matches!(
            g.check_player_response(
                player,
                token.clone(),
                g.next_test_viewer_decision().targets(objects),
                Budget::unlimited()
            ),
            QueryResult::Rejected(QueryRejection::InvalidResponse)
        ));
    }
    assert_eq!(g.snapshot().unwrap(), before);
    pay(&mut g, 1, target);
    resolve_to(&mut g, 1, 0);
    assert!(!g.current(target).unwrap().tapped);
}

#[test]
fn permanent_untap_feature_guards_stay_strict() {
    let p = prepared(&[("Burst of Energy", 0, true)]);
    let mut single = p.catalog.as_ref().clone();
    single.definitions.retain(|d| d.name == "Burst of Energy");
    single.validate().unwrap();
    for (hosted, search) in [(true, false), (false, true)] {
        let mut profile = match_profile("commander-foundations-v1").unwrap().clone();
        profile.hosted = hosted;
        profile.search = search;
        let mtg_core::testkit::ProfileError::UnreviewedCapabilities { capabilities, .. } =
            profile.validate_requested_features(&single).unwrap_err()
        else {
            panic!("new untap capability remains unreviewed")
        };
        assert!(capabilities.contains(&Capability::UntapPermanentV1));
    }
}
