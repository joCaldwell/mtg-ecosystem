//! Exact retained tapped-creature destruction. Natural witnesses use accepted
//! inputs; the explicit untap/attachment kernel probes make no replay claim.
//! Pinned CR110.5,601.2c,601.6,608.2b,702.11b/18a:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

fn prepared(rows: &[(&str, u8, bool)]) -> Prepared {
    let mut c = FixtureCatalog::Witness.get().clone();
    for raw in [
        include_str!(
            "../../../../../tests/fixtures/phase1-r4/5820d760-8ed7-4abf-a959-c06ed86a3730.json"
        ),
        include_str!(
            "../../../../../tests/fixtures/phase1-r4/ce979f36-84f2-4419-b400-971263494cc6.json"
        ),
    ] {
        let mtg_compiler::CompileResult::Lowered { definition } = mtg_witness::compile(
            include_str!("../../../../../tests/fixtures/archive-manifest-phase1-r4.json"),
            raw,
        ) else {
            panic!("complete retained tapped-creature spell must lower");
        };
        assert!(
            definition
                .capabilities
                .contains(&Capability::TappedPermanentTargetV1)
        );
        assert!(
            definition.spell.as_ref().unwrap().targets[0]
                .filter
                .as_ref()
                .unwrap()
                .tapped
        );
        if let Some(existing) = c.definition_by_oracle_id(&definition.oracle_id) {
            assert_eq!(existing, definition.as_ref());
        } else {
            c.definitions.push(*definition);
        }
    }
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    let mut board =
        Board::new(c, "tapped-creature-target-source-witness").step(TurnStep::PrecombatMain);
    for &(name, player, hand) in rows {
        board = board.permanent_or_hand(name, player, player, hand);
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
        other => panic!("unexpected passive {other:?}"),
    };
    g.respond(r.player, r.id, response).unwrap();
}
fn priority(g: &mut Game, quantum: u64, player: u8) {
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
        if r.player == PlayerId(player) {
            return;
        }
        passive(g, quantum);
    }
    panic!("priority seat {player}");
}
fn pay(g: &mut Game, quantum: u64, target: ObjectId) {
    for _ in 0..30 {
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
                let source = g
                    .state
                    .rules
                    .objects
                    .values()
                    .filter(|o| {
                        o.controller == r.player && o.zone == Zone::Battlefield && !o.tapped
                    })
                    .filter(|o| {
                        g.definition(o.id.card).unwrap().characteristics.kind == CardKind::Land
                    })
                    .min_by_key(|o| {
                        usize::from(
                            !g.mana_abilities(o.id)
                                .unwrap()
                                .iter()
                                .any(|c| Some(c) == cost.colored.first()),
                        )
                    })
                    .expect("enough printed land sources");
                r.activate_mana(source.id)
            }
            Decision {
                shape:
                    Shape::Pay {
                        cost,
                        pool:
                            ManaPool {
                                unrestricted: available,
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
            other => panic!("tapped payment {other:?}"),
        };
        g.respond(r.player, r.id, response).unwrap();
    }
    panic!("payment bound");
}
fn cast(g: &mut Game, quantum: u64, card: u32, target: ObjectId) {
    let caster = g.current(Driver::object(g, card)).unwrap().controller.0;
    priority(g, quantum, caster);
    Driver::answer(g, quantum, g.next_test_decision().cast(CardId(card)));
    pay(g, quantum, target);
}
fn resolve(g: &mut Game, quantum: u64) {
    for _ in 0..30 {
        Driver::request(g, quantum);
        if g.state.rules.stack.is_empty() {
            return;
        }
        passive(g, quantum);
    }
    panic!("resolution bound");
}
fn offers_cast(g: &Game, player: PlayerId, card: CardId) -> bool {
    let before = g.snapshot().unwrap();
    let QueryResult::Complete(QueryValue::Decision(Some(decision))) =
        g.player_decision_for(player, Budget::unlimited())
    else {
        panic!("own priority decision");
    };
    assert!(matches!(
        decision,
        Decision {
            reason: Reason::Priority,
            ..
        }
    ));
    let QueryResult::Complete(QueryValue::Observation(observation)) =
        g.observe(player, Budget::unlimited())
    else {
        panic!("permitted acting-player observation");
    };
    let oracle = &g.definition(card).unwrap().oracle_id;
    let handle = observation
        .view
        .objects
        .iter()
        .find(|o| &o.oracle_id == oracle)
        .unwrap()
        .handle
        .clone();
    let offered = decision
        .index(&Candidate::Action {
            action: Action::Cast { card: handle },
        })
        .is_some();
    assert_eq!(g.snapshot().unwrap(), before);
    offered
}

#[test]
fn tapped_target_swift_response_destroys_actual_attacker() {
    let p = prepared(&[
        ("Grizzly Bears", 0, false),
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
    let target = Driver::object(&g, 0);
    {
        let selected_answer = g.next_test_decision().attack(vec![(target, PlayerId(1))]);
        Driver::answer(&mut g, quantum, selected_answer)
    };
    priority(&mut g, quantum, 1);
    assert!(g.current(target).unwrap().tapped);
    cast(&mut g, quantum, 1, target);
    resolve(&mut g, quantum);
    assert_eq!(
        g.current(Driver::object(&g, 0)).unwrap().zone,
        Zone::Graveyard { owner: PlayerId(0) }
    );
    assert!(
        g.current(Driver::object(&g, 2)).unwrap().tapped
            && g.current(Driver::object(&g, 3)).unwrap().tapped
    );
}

#[test]
fn tapped_target_assassinate_uses_actual_mana_tap_and_sorcery_timing() {
    // A genuinely eligible tapped opponent creature does not grant permission
    // to cast a sorcery during another player's main phase.
    let mut opponent = Game::new(prepared(&[
        ("Birds of Paradise", 0, false),
        ("Assassinate", 1, true),
    ]));
    let target = Driver::object(&opponent, 0);
    {
        let selected_answer = opponent.next_test_decision().activate_mana(target);
        Driver::answer(&mut opponent, 1, selected_answer)
    };
    {
        let selected_answer = opponent
            .next_test_decision()
            .select(Candidate::Color { color: Color::G });
        Driver::answer(&mut opponent, 1, selected_answer)
    };
    priority(&mut opponent, 1, 1);
    assert!(opponent.current(target).unwrap().tapped);
    let r = Driver::request(&mut opponent, 1);
    let before = opponent.snapshot().unwrap();
    let error = opponent
        .respond(
            r.player,
            r.id,
            opponent.next_test_decision().cast(CardId(1)),
        )
        .unwrap_err();
    assert!(error.contains("invalid slot selection"), "{error}");
    assert_eq!(opponent.snapshot().unwrap(), before);

    let p = prepared(&[
        ("Birds of Paradise", 1, false),
        ("Assassinate", 0, true),
        ("Swamp", 0, false),
        ("Swamp", 0, false),
        ("Swamp", 0, false),
    ]);
    let mut g = Game::new(p);
    priority(&mut g, 1, 1);
    let target = Driver::object(&g, 0);
    {
        let selected_answer = g.next_test_decision().activate_mana(target);
        Driver::answer(&mut g, 1, selected_answer)
    };
    assert!(matches!(
        Driver::request(&mut g, 1),
        Decision {
            reason: Reason::ManaColor { .. },
            ..
        }
    ));
    {
        let selected_answer = g
            .next_test_decision()
            .select(Candidate::Color { color: Color::G });
        Driver::answer(&mut g, 1, selected_answer)
    };
    priority(&mut g, 1, 0);
    assert_eq!(g.turn().step, TurnStep::PrecombatMain);
    assert!(g.current(target).unwrap().tapped);
    cast(&mut g, 1, 1, target);
    resolve(&mut g, 1);
    assert_eq!(
        g.current(Driver::object(&g, 0)).unwrap().zone,
        Zone::Graveyard { owner: PlayerId(1) }
    );
}

#[test]
fn tapped_target_candidates_reject_untapped_and_noncreature_choices_atomically() {
    let p = prepared(&[
        ("Birds of Paradise", 1, false),
        ("Grizzly Bears", 0, false),
        ("Sol Ring", 1, false),
        ("Swift Response", 1, true),
        ("Plains", 1, false),
        ("Plains", 1, false),
    ]);
    let mut g = Game::new(p);
    priority(&mut g, 1, 1);
    let r = Driver::request(&mut g, 1);
    let before = g.snapshot().unwrap();
    assert!(!offers_cast(&g, PlayerId(1), CardId(3)));
    assert!(
        g.respond(r.player, r.id, g.next_test_decision().cast(CardId(3)))
            .is_err()
    );
    assert_eq!(g.snapshot().unwrap(), before);
    let bird = Driver::object(&g, 0);
    {
        let selected_answer = g.next_test_decision().activate_mana(bird);
        Driver::answer(&mut g, 1, selected_answer)
    };
    {
        let selected_answer = g
            .next_test_decision()
            .select(Candidate::Color { color: Color::G });
        Driver::answer(&mut g, 1, selected_answer)
    };
    let ring = Driver::object(&g, 2);
    {
        let selected_answer = g.next_test_decision().activate_mana(ring);
        Driver::answer(&mut g, 1, selected_answer)
    };
    priority(&mut g, 1, 1);
    assert!(offers_cast(&g, PlayerId(1), CardId(3)));
    {
        let selected_answer = g.next_test_decision().cast(CardId(3));
        Driver::answer(&mut g, 1, selected_answer)
    };
    let r = Driver::request(&mut g, 1);
    assert!(
        matches!(&r,found @ Decision {reason:Reason::Targets{..},..} if found.objects_offered()==vec![bird])
    );
    let before = g.snapshot().unwrap();
    for target in [Driver::object(&g, 1), ring] {
        assert!(
            g.respond(r.player, r.id, g.next_test_decision().targets(vec![target]))
                .is_err()
        );
        assert_eq!(g.snapshot().unwrap(), before);
    }
    let QueryResult::Complete(QueryValue::Decision(Some(
        found @ Decision {
            reason: Reason::Targets { .. },
            ..
        },
    ))) = g.player_decision_for(PlayerId(1), Budget::unlimited())
    else {
        panic!("public target handles")
    };
    let token = found.id.clone();
    let candidates = found.objects_offered();
    assert_eq!(candidates.len(), 1);
    assert!(matches!(
        g.check_player_response(
            PlayerId(0),
            token.clone(),
            g.next_test_viewer_decision().targets(candidates.clone()),
            Budget::unlimited()
        ),
        QueryResult::Rejected(QueryRejection::InvalidResponse)
    ));
    assert!(matches!(
        g.check_player_response(
            PlayerId(1),
            token,
            g.next_test_viewer_decision()
                .targets(vec![ObjectHandle("foreign".into())]),
            Budget::unlimited()
        ),
        QueryResult::Rejected(QueryRejection::InvalidResponse)
    ));
    assert_eq!(g.snapshot().unwrap(), before);
    pay(&mut g, 1, bird);
    resolve(&mut g, 1);
}

#[test]
fn tapped_target_private_kernel_untap_during_mana_preserves_cast_then_fizzles() {
    // No untap spell is added to admission. This typed-event kernel probe is
    // distinct from the accepted-input recovery witnesses above.
    let p = prepared(&[
        ("Grizzly Bears", 0, false),
        ("Swift Response", 1, true),
        ("Plains", 1, false),
        ("Plains", 1, false),
    ]);
    let mut g = Game::new(p);
    Driver::request(&mut g, 1);
    let target = Driver::object(&g, 0);
    g.commit(ProposedEvent::Tap { object: target }, BatchCause::Fixture)
        .unwrap();
    priority(&mut g, 1, 1);
    {
        let selected_answer = g.next_test_decision().cast(CardId(1));
        Driver::answer(&mut g, 1, selected_answer)
    };
    {
        let selected_answer = g.next_test_decision().targets(vec![target]);
        Driver::answer(&mut g, 1, selected_answer)
    };
    assert!(matches!(
        Driver::request(&mut g, 1),
        Decision {
            reason: Reason::Mana { .. },
            ..
        }
    ));
    g.commit(ProposedEvent::Untap { object: target }, BatchCause::Fixture)
        .unwrap();
    pay(&mut g, 1, target);
    assert_eq!(g.current(Driver::object(&g, 1)).unwrap().zone, Zone::Stack);
    resolve(&mut g, 1);
    assert_eq!(g.current(target).unwrap().zone, Zone::Battlefield);
    assert_eq!(
        g.current(Driver::object(&g, 1)).unwrap().zone,
        Zone::Graveyard { owner: PlayerId(1) }
    );
}

#[test]
fn tapped_target_feature_admission_remains_strict() {
    let p = prepared(&[("Swift Response", 0, true)]);
    let mut single = p.catalog.as_ref().clone();
    single.definitions.retain(|d| d.name == "Swift Response");
    single.validate().unwrap();
    for (hosted, search) in [(true, false), (false, true)] {
        let mut profile = match_profile("commander-foundations-v1").unwrap().clone();
        profile.hosted = hosted;
        profile.search = search;
        let mtg_core::testkit::ProfileError::UnreviewedCapabilities { capabilities, .. } =
            profile.validate_requested_features(&single).unwrap_err()
        else {
            panic!("unreviewed tapped capability")
        };
        assert!(capabilities.contains(&Capability::TappedPermanentTargetV1));
    }
}
