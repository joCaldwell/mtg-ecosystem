//! Genuine archived sources in private witness configurations, not pod/pool
//! admission. All rules changes below use accepted decisions or concessions.
//! Pinned CR506.4b,608.2b,609.3,701.26b,702.11b/18a:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

fn prepared(rows: &[(&str, u8, bool)]) -> Prepared {
    let mut c = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap();
    for (header, raw) in [
        (
            include_str!("../../../../tests/fixtures/archive-manifest-current.json"),
            include_str!("../../../../tests/fixtures/b795ecfd-31ac-4a2a-9679-97b088932d93.json"),
        ),
        (
            include_str!("../../../../tests/fixtures/archive-manifest-current.json"),
            include_str!("../../../../tests/fixtures/5820d760-8ed7-4abf-a959-c06ed86a3730.json"),
        ),
        (
            include_str!(
                "../../../../tests/fixtures/cards-data-d2ce/archive-manifest-current.json"
            ),
            include_str!(
                "../../../../tests/fixtures/cards-data-d2ce/ba5055ce-6809-47a4-bee6-fb97a5b60f8a.json"
            ),
        ),
    ] {
        let mtg_compiler::CompileResult::Lowered { definition } =
            mtg_compiler::compile(header, raw)
        else {
            panic!("complete archived untap witness source must lower");
        };
        c.definitions.push(*definition);
    }
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    let mut config = ScenarioConfig::turns_permanents();
    config.profile = "permanent-untap-source-witness".into();
    config.initial.turn_step = TurnStep::PrecombatMain;
    config.cards = rows
        .iter()
        .enumerate()
        .map(|(id, (name, seat, hand))| CardSetup {
            id: CardId(id as u32),
            oracle_id: c
                .definitions
                .iter()
                .find(|d| d.name == *name)
                .unwrap()
                .oracle_id
                .clone(),
            owner: PlayerId(*seat),
            controller: PlayerId(*seat),
            zone: if *hand {
                Zone::Hand {
                    owner: PlayerId(*seat),
                }
            } else {
                Zone::Battlefield
            },
        })
        .collect();
    for seat in 0..4 {
        for _ in 0..4 {
            config.cards.push(CardSetup {
                id: CardId(config.cards.len() as u32),
                oracle_id: FOREST.into(),
                owner: PlayerId(seat),
                controller: PlayerId(seat),
                zone: Zone::Library {
                    owner: PlayerId(seat),
                },
            });
        }
    }
    Prepared {
        catalog_digest: digest(&c).unwrap(),
        catalog: Arc::new(c),
        profile_digest: digest(&config).unwrap(),
        config,
        match_config: None,
    }
}
fn object(g: &Game, id: u32) -> ObjectId {
    g.object_id(CardId(id)).unwrap()
}
fn request(g: &mut Game, quantum: u64) -> Request {
    for _ in 0..1000 {
        match g.advance(Budget::new(quantum)) {
            Advance::AwaitingDecision(r) => return r,
            Advance::Suspended { .. } => (),
            other => panic!("untap {:?}: {other:?}", g.state.rules.frame),
        }
    }
    panic!("untap work bound");
}
fn answer(g: &mut Game, quantum: u64, response: Response) {
    let r = request(g, quantum);
    g.respond(r.player(), r.decision_id(), response).unwrap();
}
fn passive(g: &mut Game, quantum: u64) {
    let r = request(g, quantum);
    let response = match r {
        Request::Priority { .. } => Response::Pass,
        Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
        Request::Blockers { .. } => Response::Blockers { blocks: vec![] },
        other => panic!("untap passive {other:?}"),
    };
    g.respond(r.player(), r.decision_id(), response).unwrap();
}
fn priority(g: &mut Game, quantum: u64, seat: u8) {
    for _ in 0..8 {
        let r = request(g, quantum);
        assert!(matches!(r, Request::Priority { .. }), "{r:?}");
        if r.player() == PlayerId(seat) {
            return;
        }
        passive(g, quantum);
    }
    panic!("untap priority");
}
fn pay(g: &mut Game, quantum: u64, target: ObjectId) {
    for _ in 0..40 {
        let r = request(g, quantum);
        let response = match &r {
            Request::Targets { candidates, .. } => {
                assert!(candidates.contains(&target));
                Response::Targets {
                    objects: vec![target],
                }
            }
            Request::Mana { cost, .. } if g.payment_pool(r.player(), cost).is_ok() => {
                Response::FinishMana { undo_mana: false }
            }
            Request::Mana { cost, .. } => {
                let land = g
                    .state
                    .rules
                    .objects
                    .values()
                    .filter(|o| {
                        o.controller == r.player()
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
                Response::ActivateMana { object: land.id }
            }
            Request::SpendMana {
                cost, available, ..
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
                Response::SpendMana { spending }
            }
            Request::Priority { .. } => return,
            other => panic!("untap payment {other:?}"),
        };
        g.respond(r.player(), r.decision_id(), response).unwrap();
    }
    panic!("untap payment bound");
}
fn cast(g: &mut Game, quantum: u64, card: u32, target: ObjectId) -> StackId {
    let seat = g.current(object(g, card)).unwrap().controller.0;
    priority(g, quantum, seat);
    answer(g, quantum, Response::Cast { card: CardId(card) });
    pay(g, quantum, target);
    g.state.rules.stack.last().unwrap().id
}
fn resolve_to(g: &mut Game, quantum: u64, len: usize) {
    for _ in 0..80 {
        request(g, quantum);
        if g.state.rules.stack.len() == len {
            return;
        }
        passive(g, quantum);
    }
    panic!("untap resolution bound");
}
fn check(g: &Game) {
    let p = g.prepared.as_ref().clone();
    let restored = Game::restore(p.clone(), &g.snapshot().unwrap()).unwrap();
    assert_eq!(restored.state, g.state);
    assert_eq!(restored.trace(), g.trace());
    let replay = Game::replay_to(
        p,
        &g.replay_journal(),
        Some(g.rule_boundary()),
        g.trace.mode,
    )
    .unwrap();
    assert_eq!(replay.state, g.state);
    assert_eq!(replay.trace(), g.trace());
    assert_eq!(g.fork().state, g.state);
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
fn permanent_untap_real_response_preserves_attacking_and_fizzles_swift_with_replay() {
    let p = prepared(&[
        ("Grizzly Bears", 0, false),
        ("Burst of Energy", 0, true),
        ("Plains", 0, false),
        ("Swift Response", 1, true),
        ("Plains", 1, false),
        ("Plains", 1, false),
    ]);
    let mut baseline = None;
    for (quantum, mode) in [(1, TraceMode::Detailed), (4096, TraceMode::Off)] {
        let mut g = Game::with_trace(p.clone(), mode);
        for _ in 0..100 {
            if matches!(request(&mut g, quantum), Request::Attackers { .. }) {
                break;
            }
            passive(&mut g, quantum);
        }
        let attacker = object(&g, 0);
        answer(
            &mut g,
            quantum,
            Response::Attackers {
                attacks: vec![(attacker, PlayerId(1))],
            },
        );
        // Responses enqueue input; advance to commit the declaration before
        // inspecting its attack tap.
        request(&mut g, quantum);
        assert!(g.current(attacker).unwrap().tapped);
        let swift = cast(&mut g, quantum, 3, attacker);
        let burst = cast(&mut g, quantum, 1, attacker);
        assert_eq!(g.state.rules.stack.len(), 2);
        check(&g);
        resolve_to(&mut g, quantum, 1);
        assert!(!g.current(attacker).unwrap().tapped);
        assert!(g.current(attacker).unwrap().attack_ready);
        assert!(g.state.rules.combat.iter().any(|a| a.attacker == attacker));
        assert_eq!(g.state.rules.stack[0].id, swift);
        if mode == TraceMode::Detailed {
            resolved(&g, burst, true);
            assert_eq!(untaps(&g, attacker), 1);
        }
        check(&g);
        resolve_to(&mut g, quantum, 0);
        assert_eq!(g.current(attacker).unwrap().zone, Zone::Battlefield);
        if mode == TraceMode::Detailed {
            resolved(&g, swift, false);
        }
        check(&g);
        for _ in 0..100 {
            let r = request(&mut g, quantum);
            if g.turn().step == TurnStep::CombatDamage && matches!(r, Request::Priority { .. }) {
                break;
            }
            passive(&mut g, quantum);
        }
        assert_eq!(g.state.rules.players[1].life, 38.into());
        assert!(g.state.rules.combat.iter().any(|a| a.attacker == attacker));
        assert_eq!(
            g.current(object(&g, 1)).unwrap().zone,
            Zone::Graveyard { owner: PlayerId(0) }
        );
        assert_eq!(
            g.current(object(&g, 3)).unwrap().zone,
            Zone::Graveyard { owner: PlayerId(1) }
        );
        assert!(g.current(object(&g, 2)).unwrap().tapped);
        assert!(
            g.current(object(&g, 4)).unwrap().tapped && g.current(object(&g, 5)).unwrap().tapped
        );
        check(&g);
        if let Some(old) = &baseline {
            assert_eq!(old, &g.state);
        } else {
            baseline = Some(g.state.clone());
        }
    }
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
    let target = object(&g, 0);
    let stack = cast(&mut g, 1, 1, target);
    assert!(!g.current(target).unwrap().tapped);
    check(&g);
    resolve_to(&mut g, 1, 0);
    assert!(!g.current(target).unwrap().tapped);
    assert!(g.current(object(&g, 2)).unwrap().tapped);
    assert_eq!(g.mana(PlayerId(0), Color::W), Some(Quantity::zero()));
    resolved(&g, stack, true);
    assert_eq!(untaps(&g, target), 0);
    check(&g);
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
        let original = object(&g, 0);
        let burst = cast(&mut g, 1, 1, original);
        check(&g);
        if departure {
            g.concede(PlayerId(1)).unwrap();
            request(&mut g, 1);
            assert!(g.current(original).is_err());
        } else {
            cast(&mut g, 1, 3, original);
            resolve_to(&mut g, 1, 1);
            assert_eq!(
                g.current(object(&g, 0)).unwrap().zone,
                Zone::Hand { owner: PlayerId(1) }
            );
            // Research Thief's exact Flash source permits this real reentry
            // while the original Burst remains below it on the stack.
            cast(&mut g, 1, 0, original);
            resolve_to(&mut g, 1, 1);
            assert_ne!(object(&g, 0), original);
            assert_eq!(g.current(object(&g, 0)).unwrap().zone, Zone::Battlefield);
            assert!(g.current(original).is_err());
        }
        check(&g);
        resolve_to(&mut g, 1, 0);
        resolved(&g, burst, false);
        assert_eq!(untaps(&g, original), 0);
        check(&g);
    }
}

#[test]
fn permanent_untap_public_targets_include_untapped_permanents_and_reject_stack_handles() {
    let mut g = Game::new(prepared(&[
        ("Birds of Paradise", 0, false),
        ("Burst of Energy", 0, true),
        ("Plains", 0, false),
    ]));
    let target = object(&g, 0);
    answer(&mut g, 1, Response::ActivateMana { object: target });
    answer(&mut g, 1, Response::ManaColor { color: Color::G });
    answer(&mut g, 1, Response::Cast { card: CardId(1) });
    let r = request(&mut g, 1);
    let source = object(&g, 1);
    assert!(
        matches!(&r,Request::Targets {candidates,..} if candidates.contains(&target)
        && candidates.contains(&object(&g,2)) && !candidates.contains(&source))
    );
    let before = g.snapshot().unwrap();
    assert!(
        g.respond(
            r.player(),
            r.decision_id(),
            Response::Targets {
                objects: vec![source]
            }
        )
        .is_err()
    );
    let QueryResult::Complete(QueryValue::Decision(Some(PlayerRequest::Targets {
        token,
        candidates,
        ..
    }))) = g.player_decision_for(PlayerId(0), Budget::unlimited())
    else {
        panic!("untap public targets")
    };
    assert_eq!(candidates.len(), 2);
    for (player, objects) in [
        (PlayerId(1), vec![candidates[0].clone()]),
        (PlayerId(0), vec![ObjectHandle("foreign".into())]),
    ] {
        assert!(matches!(
            g.check_player_response(
                player,
                token.clone(),
                PlayerResponse::Targets { objects },
                Budget::unlimited()
            ),
            QueryResult::Rejected(QueryRejection::InvalidResponse)
        ));
    }
    assert_eq!(g.snapshot().unwrap(), before);
    check(&g);
    pay(&mut g, 1, target);
    resolve_to(&mut g, 1, 0);
    assert!(!g.current(target).unwrap().tapped);
    check(&g);
}

#[test]
fn permanent_untap_actual_equipment_targetability_and_feature_guards_stay_strict() {
    for equipment in ["Lightning Greaves", "Swiftfoot Boots"] {
        let mut g = Game::new(prepared(&[
            ("Birds of Paradise", 0, false),
            (equipment, 0, false),
            ("Burst of Energy", 1, true),
            ("Plains", 0, false),
            ("Plains", 1, false),
        ]));
        let target = object(&g, 0);
        let source = object(&g, 1);
        answer(
            &mut g,
            1,
            Response::ActivateAbility {
                object: source,
                ability: 0,
            },
        );
        pay(&mut g, 1, target);
        resolve_to(&mut g, 1, 0);
        assert_eq!(g.current(source).unwrap().attached_to, Some(target));
        priority(&mut g, 1, 1);
        answer(&mut g, 1, Response::Cast { card: CardId(2) });
        let r = request(&mut g, 1);
        assert!(matches!(&r,Request::Targets {candidates,..} if !candidates.contains(&target)));
        let before = g.snapshot().unwrap();
        assert!(
            g.respond(
                r.player(),
                r.decision_id(),
                Response::Targets {
                    objects: vec![target]
                }
            )
            .is_err()
        );
        assert_eq!(g.snapshot().unwrap(), before);
        check(&g);
    }
    let p = prepared(&[("Burst of Energy", 0, true)]);
    let mut single = p.catalog.as_ref().clone();
    single.definitions.retain(|d| d.name == "Burst of Energy");
    single.validate().unwrap();
    for (hosted, search) in [(true, false), (false, true)] {
        let mut profile = match_profile("commander-foundations-v1").unwrap().clone();
        profile.hosted = hosted;
        profile.search = search;
        let crate::profile_data::ProfileError::UnreviewedCapabilities { capabilities, .. } =
            profile.validate_requested_features(&single).unwrap_err()
        else {
            panic!("new untap capability remains unreviewed")
        };
        assert!(capabilities.contains(&Capability::UntapPermanentV1));
    }
}
