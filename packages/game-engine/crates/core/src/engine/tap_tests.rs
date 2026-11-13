//! Complete archived Pressure Point in private source configurations, not pods.
//! Accepted mana activations, spells and concessions; no injected tap state.
//! Pinned CR104.3a,608.2b/c,609.3,701.26a,800.4a:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

fn prepared(already_tapped: bool) -> Prepared {
    let mut catalog = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap();
    let mtg_compiler::CompileResult::Lowered { definition } = mtg_compiler::compile(
        include_str!("../../../../tests/fixtures/archive-manifest-current.json"),
        include_str!("../../../../tests/fixtures/d4bbdd0c-e578-49ed-b2f7-b1decb04ee87.json"),
    ) else {
        panic!("complete Pressure Point source must lower");
    };
    assert!(
        definition
            .capabilities
            .contains(&Capability::TapPermanentV1)
    );
    assert!(matches!(
        definition.spell.as_ref().unwrap().instructions.as_slice(),
        [
            Instruction::TapPermanent {
                target_slot: 0,
                source_block: 0
            },
            Instruction::DrawCards {
                recipient: None,
                count,
                source_block: 1
            }
        ] if *count == Quantity::from(1)
    ));
    catalog.definitions.push(*definition);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    let mut config = ScenarioConfig::turns_permanents();
    config.profile = "tap-cantrip-source-witness".into();
    config.initial.turn_step = TurnStep::PrecombatMain;
    config.cards = [
        ("Pressure Point", 0, true),
        (
            if already_tapped {
                "Llanowar Elves"
            } else {
                "Grizzly Bears"
            },
            if already_tapped { 0 } else { 1 },
            false,
        ),
        ("Plains", 0, false),
        ("Plains", 0, false),
    ]
    .into_iter()
    .enumerate()
    .map(|(id, (name, seat, hand))| CardSetup {
        id: CardId(id as u32),
        oracle_id: catalog
            .definitions
            .iter()
            .find(|d| d.name == name)
            .unwrap()
            .oracle_id
            .clone(),
        owner: PlayerId(seat),
        controller: PlayerId(seat),
        zone: if hand {
            Zone::Hand {
                owner: PlayerId(seat),
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
        catalog_digest: digest(&catalog).unwrap(),
        catalog: Arc::new(catalog),
        profile_digest: digest(&config).unwrap(),
        config,
        match_config: None,
    }
}
fn object(g: &Game, card: u32) -> ObjectId {
    g.object_id(CardId(card)).unwrap()
}
fn request(g: &mut Game, quantum: u64) -> Request {
    for _ in 0..500 {
        match g.advance(Budget::new(quantum)) {
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(r) => return r,
            other => panic!("tap {:?}: {other:?}", g.state.rules.frame),
        }
    }
    panic!("tap work bound");
}
fn answer(g: &mut Game, quantum: u64, response: Response) {
    let r = request(g, quantum);
    g.respond(r.player(), r.decision_id(), response).unwrap();
}
fn cast(g: &mut Game, quantum: u64, target: ObjectId, already: bool) -> StackId {
    if already {
        answer(g, quantum, Response::ActivateMana { object: target });
        assert!(matches!(request(g, quantum), Request::Priority { .. }));
        assert!(g.current(target).unwrap().tapped);
        assert_eq!(g.mana(PlayerId(0), Color::G), Some(1.into()));
    }
    answer(g, quantum, Response::Cast { card: CardId(0) });
    let r = request(g, quantum);
    let Request::Targets { candidates, .. } = &r else {
        panic!("tap targets {r:?}");
    };
    let land = object(g, 2);
    assert!(candidates.contains(&target));
    assert!(!candidates.contains(&land));
    let before = g.snapshot().unwrap();
    assert!(
        g.respond(
            r.player(),
            r.decision_id(),
            Response::Targets {
                objects: vec![land]
            }
        )
        .is_err()
    );
    assert_eq!(g.snapshot().unwrap(), before);
    answer(
        g,
        quantum,
        Response::Targets {
            objects: vec![target],
        },
    );
    let plains = object(g, 2);
    answer(g, quantum, Response::ActivateMana { object: plains });
    if !already {
        let other = object(g, 3);
        answer(g, quantum, Response::ActivateMana { object: other });
    }
    answer(g, quantum, Response::FinishMana { undo_mana: false });
    assert!(matches!(request(g, quantum), Request::SpendMana { .. }));
    let spending = if already {
        BTreeMap::from([(Color::G, 1.into()), (Color::W, 1.into())])
    } else {
        BTreeMap::from([(Color::W, 2.into())])
    };
    answer(g, quantum, Response::SpendMana { spending });
    assert!(matches!(request(g, quantum), Request::Priority { .. }));
    assert_eq!(
        g.top().unwrap().paid,
        Some(ManaCost {
            generic: 1.into(),
            colored: vec![Color::W]
        })
    );
    g.top().unwrap().id
}
fn resolve_start(g: &mut Game) {
    for _ in 0..40 {
        if matches!(&g.state.rules.frame, Frame::Resolve { frame } if frame.cursor == 0) {
            return;
        }
        match g.advance(Budget::new(1)) {
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(r) if matches!(r, Request::Priority { .. }) => {
                g.respond(r.player(), r.decision_id(), Response::Pass)
                    .unwrap();
            }
            other => panic!("tap resolution boundary {other:?}"),
        }
    }
    panic!("tap resolution boundary work bound");
}
fn finish(g: &mut Game, quantum: u64) {
    for _ in 0..20 {
        assert!(matches!(request(g, quantum), Request::Priority { .. }));
        if g.state.rules.stack.is_empty() {
            return;
        }
        answer(g, quantum, Response::Pass);
    }
    panic!("tap completion bound");
}
fn recover(g: &Game) {
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
}
#[derive(Clone, Copy)]
enum Case {
    Untapped,
    Tapped,
    GoneBefore,
    GoneAfter,
}
fn taps(g: &Game, target: ObjectId) -> usize {
    g.events().iter().filter(|e| matches!(e, Event::Committed { actual: EventOutcome::Tapped { object }, .. } if *object == target)).count()
}
fn run(case: Case, quantum: u64, mode: TraceMode) -> Game {
    let already = matches!(case, Case::Tapped);
    let mut g = Game::with_trace(prepared(already), mode);
    let target = object(&g, 1);
    let stack = cast(&mut g, quantum, target, already);
    let library = g.state.rules.libraries[&PlayerId(0)].len();
    let old_taps = if mode == TraceMode::Detailed {
        taps(&g, target)
    } else {
        0
    };
    recover(&g);
    match case {
        Case::GoneBefore => {
            g.concede(PlayerId(1)).unwrap();
            assert!(matches!(request(&mut g, quantum), Request::Priority { .. }));
            assert!(g.current(target).is_err());
        }
        Case::GoneAfter => {
            resolve_start(&mut g);
            assert!(
                matches!(&g.state.rules.frame, Frame::Resolve { frame } if frame.legal_targets == [true] && frame.cursor == 0)
            );
            g.concede(PlayerId(1)).unwrap();
            assert!(matches!(
                g.advance(Budget::new(1)),
                Advance::Suspended { .. }
            ));
            assert!(
                matches!(&g.state.rules.frame, Frame::Resolve { frame } if frame.legal_targets == [true] && frame.cursor == 0)
            );
            assert!(g.current(target).is_err());
            recover(&g);
        }
        _ => (),
    }
    finish(&mut g, quantum);
    let drew = !matches!(case, Case::GoneBefore);
    assert_eq!(
        g.state.rules.libraries[&PlayerId(0)].len(),
        library - usize::from(drew)
    );
    assert_eq!(
        g.state
            .rules
            .objects
            .values()
            .filter(|o| o.zone == (Zone::Hand { owner: PlayerId(0) }))
            .count(),
        usize::from(drew)
    );
    assert_eq!(
        g.current(object(&g, 0)).unwrap().zone,
        Zone::Graveyard { owner: PlayerId(0) }
    );
    if !matches!(case, Case::GoneBefore | Case::GoneAfter) {
        assert!(g.current(target).unwrap().tapped);
    }
    if mode == TraceMode::Detailed {
        assert_eq!(
            taps(&g, target) - old_taps,
            usize::from(matches!(case, Case::Untapped))
        );
        assert!(g.events().iter().any(|e| matches!(e, Event::SpellCompleted { stack_id, resolved } if *stack_id == stack && *resolved == drew)));
    }
    recover(&g);
    g
}
fn quanta(case: Case) {
    let a = run(case, 1, TraceMode::Detailed);
    let b = run(case, 4096, TraceMode::Off);
    assert_eq!(a.state, b.state);
    assert_eq!(a.replay_journal(), b.replay_journal());
}
#[test]
fn tap_cantrip_paid_untapped_creature_taps_then_draws_with_replay() {
    quanta(Case::Untapped);
}
#[test]
fn tap_cantrip_already_tapped_by_real_mana_activation_is_noop_then_draws() {
    quanta(Case::Tapped);
}
#[test]
fn tap_cantrip_sole_target_gone_before_resolution_fizzles_without_draw() {
    quanta(Case::GoneBefore);
}
#[test]
fn tap_cantrip_target_gone_after_resolution_start_skips_tap_but_draws() {
    quanta(Case::GoneAfter);
}
