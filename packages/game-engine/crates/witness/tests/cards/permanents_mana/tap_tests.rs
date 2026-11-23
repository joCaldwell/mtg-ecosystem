//! Complete archived Pressure Point in private source configurations, not pods.
//! Accepted mana activations, spells and concessions; no injected tap state.
//! Pinned CR104.3a,608.2b/c,609.3,701.26a,800.4a:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

fn prepared(already_tapped: bool) -> Prepared {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    let mtg_compiler::CompileResult::Lowered { definition } = mtg_witness::compile(
        include_str!("../../../../../tests/fixtures/archive-manifest-phase1-r4.json"),
        include_str!(
            "../../../../../tests/fixtures/phase1-r4/d4bbdd0c-e578-49ed-b2f7-b1decb04ee87.json"
        ),
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
            kernel!(Tap { what: target_object!(0) } @ 0),
            kernel!(Draw { player: you!(), amount: fixed!(count) } @ 1)
        ] if *count == Quantity::from(1)
    ));
    if let Some(existing) = catalog.definition_by_oracle_id(&definition.oracle_id) {
        assert_eq!(existing, definition.as_ref());
    } else {
        catalog.definitions.push(*definition);
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    let mut board = Board::new(catalog, "tap-cantrip-source-witness").step(TurnStep::PrecombatMain);
    for (name, seat, hand) in [
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
    ] {
        board = board.permanent_or_hand(name, seat, seat, hand);
    }
    board.libraries("Forest", 4).prepare()
}
fn cast(g: &mut Game, quantum: u64, target: ObjectId, already: bool) -> StackId {
    if already {
        Driver::answer(g, quantum, Response::ActivateMana { object: target });
        assert!(matches!(
            Driver::request(g, quantum),
            Request::Priority { .. }
        ));
        assert!(g.current(target).unwrap().tapped);
        assert_eq!(g.mana(PlayerId(0), Color::G), Some(1.into()));
    }
    Driver::answer(g, quantum, Response::Cast { card: CardId(0) });
    let r = Driver::request(g, quantum);
    let Request::Targets { candidates, .. } = &r else {
        panic!("tap targets {r:?}");
    };
    let land = Driver::object(g, 2);
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
    Driver::answer(
        g,
        quantum,
        Response::Targets {
            objects: vec![target],
        },
    );
    let plains = Driver::object(g, 2);
    Driver::answer(g, quantum, Response::ActivateMana { object: plains });
    if !already {
        let other = Driver::object(g, 3);
        Driver::answer(g, quantum, Response::ActivateMana { object: other });
    }
    Driver::answer(g, quantum, Response::FinishMana { undo_mana: false });
    assert!(matches!(
        Driver::request(g, quantum),
        Request::SpendMana { .. }
    ));
    let spending = if already {
        BTreeMap::from([(Color::G, 1.into()), (Color::W, 1.into())])
    } else {
        BTreeMap::from([(Color::W, 2.into())])
    };
    Driver::answer(g, quantum, Response::SpendMana { spending });
    assert!(matches!(
        Driver::request(g, quantum),
        Request::Priority { .. }
    ));
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
        assert!(matches!(
            Driver::request(g, quantum),
            Request::Priority { .. }
        ));
        if g.state.rules.stack.is_empty() {
            return;
        }
        Driver::answer(g, quantum, Response::Pass);
    }
    panic!("tap completion bound");
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
    let target = Driver::object(&g, 1);
    let stack = cast(&mut g, quantum, target, already);
    let library = g.state.rules.libraries[&PlayerId(0)].len();
    let old_taps = if mode == TraceMode::Detailed {
        taps(&g, target)
    } else {
        0
    };

    match case {
        Case::GoneBefore => {
            g.concede(PlayerId(1)).unwrap();
            assert!(matches!(
                Driver::request(&mut g, quantum),
                Request::Priority { .. }
            ));
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
        g.current(Driver::object(&g, 0)).unwrap().zone,
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

    g
}
fn exercise(case: Case) {
    run(case, u64::MAX, TraceMode::Detailed);
}
#[test]
fn tap_cantrip_paid_untapped_creature_taps_then_draws_with_replay() {
    exercise(Case::Untapped);
}
#[test]
fn tap_cantrip_already_tapped_by_real_mana_activation_is_noop_then_draws() {
    exercise(Case::Tapped);
}
#[test]
fn tap_cantrip_sole_target_gone_before_resolution_fizzles_without_draw() {
    exercise(Case::GoneBefore);
}
#[test]
fn tap_cantrip_target_gone_after_resolution_start_skips_tap_but_draws() {
    exercise(Case::GoneAfter);
}
