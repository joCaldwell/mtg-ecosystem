//! Exercise audited continuations and the interpreter's interactive choice path.
use super::*;
use mtg_contracts::kir::{
    Amount, Cond, Context, Domain, Node, Payment, PlayerExpr, Pred, Ref, SetExpr, Verb,
};

fn controller() -> PlayerExpr {
    PlayerExpr::you(Context::Spell)
}

fn gain(player: PlayerExpr, amount: Amount) -> Node {
    Node::Do(Verb::GainLife { player, amount })
}

fn scenario() -> Prepared {
    let mut prepared = super::formats_tests::actions_scenario();
    let catalog = Arc::make_mut(&mut prepared.catalog);
    let def = catalog
        .definitions
        .iter_mut()
        .find(|def| def.name == "Cruel Edict")
        .unwrap();
    let each = Node::ForEach {
        var: 9,
        over: Domain::Players {
            players: PlayerExpr::EachOpponent,
        },
        body: Box::new(gain(
            PlayerExpr::Is {
                who: Ref::BoundVar { id: 9 },
            },
            Amount::fixed(2),
        )),
    };
    let chosen = SetExpr::Pool {
        pool: Ref::BoundVar { id: 3 },
    };
    let candidates = SetExpr::battlefield(Pred::creatures());
    let choose = Node::Choose {
        var: 3,
        chooser: controller(),
        from: candidates.clone(),
        minimum: Amount::fixed(1),
        maximum: Amount::fixed(1),
        body: Box::new(gain(controller(), Amount::Count { set: chosen })),
    };
    let unless = |payment| Node::UnlessPays {
        payer: controller(),
        payment,
        node: Box::new(gain(controller(), Amount::fixed(10))),
    };
    let node = Node::Seq(vec![
        Node::If {
            condition: Cond::Exists { set: candidates },
            then: Box::new(Node::Optional {
                chooser: controller(),
                node: Box::new(each),
            }),
            otherwise: None,
        },
        choose,
        unless(Payment::Life {
            amount: Amount::fixed(3),
        }),
        unless(Payment::Generic {
            amount: Amount::fixed(0),
        }),
    ]);
    def.spell.as_mut().unwrap().instructions = vec![Instruction::Kernel {
        node,
        source_block: 0,
    }];
    prepared.catalog_digest = mtg_contracts::digest(catalog).unwrap();
    prepared
}

pub(super) fn suspended_control() -> Game {
    let mut game = super::formats_tests::cast_edict(scenario());
    let request = game.advance(Budget::unlimited());
    assert!(
        matches!(
            request,
            Advance::AwaitingDecision(Request::OptionalEffect {
                player: PlayerId(0),
                ..
            })
        ),
        "{request:?}"
    );
    assert!(game.state.rules.frames.len() > 1);
    game.validate_state().unwrap();
    game
}

fn answer(game: &mut Game, response: Response) {
    let next = game.advance(Budget::unlimited());
    let Advance::AwaitingDecision(request) = next else {
        panic!("expected a decision, got {next:?}")
    };
    game.respond(request.player(), request.decision_id(), response)
        .unwrap();
}

#[test]
fn nested_controls_restore_and_replay_at_live_choices() {
    let mut original = suspended_control();
    let before = original
        .state
        .rules
        .players
        .iter()
        .map(|p| p.life.clone())
        .collect::<Vec<_>>();
    let prepared = original.prepared.as_ref().clone();
    let mut restored = Game::restore(prepared.clone(), &original.snapshot().unwrap()).unwrap();
    for game in [&mut original, &mut restored] {
        for _ in 0..4 {
            answer(game, Response::OptionalEffect { perform: true });
            game.validate_state().unwrap();
        }
        assert!(matches!(
            game.advance(Budget::unlimited()),
            Advance::AwaitingDecision(Request::Mana { .. })
        ));
        assert!(
            game.state
                .rules
                .frames
                .iter()
                .any(|f| matches!(f, Frame::Kernel { .. }))
        );
        let payment_charge = game
            .state
            .rules
            .frames
            .iter()
            .filter(|f| !matches!(f, Frame::Kernel { .. }))
            .chain(std::iter::once(&game.state.rules.frame))
            .map(stored_history::payload_charge)
            .sum::<usize>();
        assert_eq!(game.mana_continuation_charge(), payment_charge);
        answer(game, Response::FinishMana { undo_mana: false });
        answer(
            game,
            Response::SpendMana {
                spending: BTreeMap::new(),
            },
        );
        assert!(matches!(
            game.advance(Budget::unlimited()),
            Advance::AwaitingDecision(Request::Priority { .. })
        ));
        game.validate_state().unwrap();
        for (seat, player) in game.state.rules.players.iter().enumerate() {
            let delta = if seat == 0 { -2 } else { 2 };
            let mut expected = before[seat].clone();
            expected.add(&Quantity::from(delta));
            assert_eq!(player.life, expected);
        }
        assert!(game.state.rules.frames.is_empty());
    }
    assert_eq!(original.snapshot().unwrap(), restored.snapshot().unwrap());
    let mut replay = Game::replay(prepared, &original.replay_journal()).unwrap();
    replay.advance(Budget::unlimited());
    assert_eq!(
        original.state_digest().unwrap(),
        replay.state_digest().unwrap()
    );
}

#[test]
fn declining_optional_and_unless_payments_follows_only_selected_branches() {
    let mut g = suspended_control();
    let mut expected = g.state.rules.players[0].life.clone();
    for perform in [false, true, false, false] {
        answer(&mut g, Response::OptionalEffect { perform });
    }
    assert!(matches!(
        g.advance(Budget::unlimited()),
        Advance::AwaitingDecision(Request::Priority { .. })
    ));
    expected.add(&Quantity::from(21));
    assert_eq!(g.state.rules.players[0].life, expected);
    g.validate_state().unwrap();
}

#[test]
fn a_nested_verb_keeps_its_own_player_choice_and_returns_to_the_sequence() {
    let mut prepared = super::formats_tests::actions_scenario();
    let catalog = Arc::make_mut(&mut prepared.catalog);
    let program = catalog
        .definitions
        .iter_mut()
        .find(|d| d.name == "Cruel Edict")
        .unwrap()
        .spell
        .as_mut()
        .unwrap();
    let Instruction::Kernel { node, .. } = &mut program.instructions[0] else {
        panic!("expected the admitted sacrifice verb")
    };
    let candidates = SetExpr::battlefield(Pred::creatures());
    let size = Amount::Count {
        set: candidates.clone(),
    };
    *node = Node::Seq(vec![
        node.clone(),
        Node::Choose {
            var: 3,
            chooser: controller(),
            from: candidates,
            minimum: size.clone(),
            maximum: size,
            body: Box::new(gain(
                PlayerExpr::OwnerOf {
                    object: Ref::Source,
                },
                Amount::Count {
                    set: SetExpr::Pool {
                        pool: Ref::BoundVar { id: 3 },
                    },
                },
            )),
        },
    ]);
    mtg_contracts::kir::check(
        node,
        &mtg_contracts::kir::Env {
            context: Context::Spell,
            targets: &program.targets,
            event: None,
            announced_x: false,
        },
    )
    .unwrap();
    prepared.catalog_digest = mtg_contracts::digest(catalog).unwrap();
    let mut game = super::formats_tests::cast_edict(prepared.clone());
    let next = game.advance(Budget::unlimited());
    let Advance::AwaitingDecision(Request::Sacrifice {
        player, candidates, ..
    }) = next
    else {
        panic!("expected nested sacrifice, got {next:?}")
    };
    assert_eq!(player, PlayerId(1));
    game.validate_state().unwrap();
    let before = game.state.rules.players[0].life.clone();
    let mut restored = Game::restore(prepared.clone(), &game.snapshot().unwrap()).unwrap();
    for g in [&mut game, &mut restored] {
        answer(
            g,
            Response::Sacrifice {
                object: candidates[0],
            },
        );
        // Inspect the instruction boundary before resolving the rest of the spell.
        // The moving object may be a commander whose owner must choose a zone.
        for _ in 0..64 {
            match g.advance(Budget::new(1)) {
                Advance::AwaitingDecision(Request::CommanderReturn { .. }) => {
                    answer(g, Response::CommanderReturn { to_command: false });
                }
                Advance::Suspended { .. } => {}
                other => panic!("missing completed control instruction: {other:?}"),
            }
            if let Frame::Resolve { frame } = &g.state.rules.frame
                && frame.cursor == 1
                && g.state.rules.frames.is_empty()
            {
                break;
            }
        }
        let Frame::Resolve { frame } = &g.state.rules.frame else {
            panic!("missing completed instruction")
        };
        let [InstructionOutcome::Kernel { outcomes }] = frame.outcomes.as_slice() else {
            panic!("lost control outcomes")
        };
        assert!(matches!(outcomes.as_slice(), [
            InstructionOutcome::Sacrificed { player: p, movement: Some(movement) },
            InstructionOutcome::Life { actual: Some(EventOutcome::LifeGained { player: PlayerId(0), amount, .. }) }
        ] if *p == player && movement.previous == candidates[0] && *amount == Quantity::from(2)));
        g.validate_state().unwrap();
        let roundtrip = Game::restore(prepared.clone(), &g.snapshot().unwrap()).unwrap();
        assert_eq!(roundtrip.state_digest().unwrap(), g.state_digest().unwrap());
        assert!(g.state.rules.frames.is_empty());
        let mut expected = before.clone();
        expected.add(&2.into());
        assert_eq!(g.state.rules.players[0].life, expected);
        g.validate_state().unwrap();
    }
    assert_eq!(
        game.state_digest().unwrap(),
        restored.state_digest().unwrap()
    );
}

#[test]
fn missing_kernel_parent_rejects_the_step() {
    let mut game = suspended_control();
    game.state.rules.frames.clear();
    assert!(game.validate_state().is_err());
    game.state.rules.frame = Frame::Stabilize {
        resume: FrameLink(usize::MAX),
        cleanup: false,
        changed: false,
    };
    assert!(
        matches!(game.advance(Budget::new(1)), Advance::Fault(reason) if reason.contains("invalid continuation stack link"))
    );
}
