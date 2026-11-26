//! Exercise audited continuations and the interpreter's interactive choice path.
use super::*;
use mtg_contracts::kir::{
    Amount, Cond, Context, Domain, Node, Payment, PlayerExpr, Pred, Ref, SetExpr, Verb, ZoneExpr,
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

pub(crate) fn suspended_control() -> Game {
    let mut game = super::formats_tests::cast_edict(scenario());
    let request = game.advance(Budget::unlimited());
    assert!(
        matches!(
            request,
            Advance::AwaitingDecision(Decision {
                player: PlayerId(0),
                reason: Reason::OptionalEffect,
                ..
            })
        ),
        "{request:?}"
    );
    assert!(game.state.rules.frames.len() > 1);
    game.validate_state().unwrap();
    game
}

fn answer(game: &mut Game, response: Answer) {
    let next = game.advance(Budget::unlimited());
    let Advance::AwaitingDecision(request) = next else {
        panic!("expected a decision, got {next:?}")
    };
    game.respond(request.player, request.id, response).unwrap();
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
            {
                let selected = game
                    .next_test_decision()
                    .select(Candidate::Flag { flag: Flag::Accept });
                answer(game, selected)
            };
            game.validate_state().unwrap();
        }
        assert!(matches!(
            game.advance(Budget::unlimited()),
            Advance::AwaitingDecision(Decision {
                reason: Reason::Mana { .. },
                ..
            })
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
        answer(
            game,
            Answer::Pay {
                pay: PayAnswer::Finish {
                    undo_on_failure: false,
                },
            },
        );
        answer(
            game,
            Answer::Pay {
                pay: PayAnswer::Spend {
                    unrestricted: BTreeMap::new(),
                },
            },
        );
        assert!(matches!(
            game.advance(Budget::unlimited()),
            Advance::AwaitingDecision(Decision {
                reason: Reason::Priority,
                ..
            })
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
        {
            let selected_answer = g.next_test_decision().select(Candidate::Flag {
                flag: if perform { Flag::Accept } else { Flag::Decline },
            });
            answer(&mut g, selected_answer)
        };
    }
    assert!(matches!(
        g.advance(Budget::unlimited()),
        Advance::AwaitingDecision(Decision {
            reason: Reason::Priority,
            ..
        })
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
    let Instruction::Kernel { node, .. } = &mut program.instructions[0];
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
                controller(),
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
    let Advance::AwaitingDecision(
        found @ Decision {
            reason: Reason::Sacrifice,
            ..
        },
    ) = next
    else {
        panic!("expected nested sacrifice, got {next:?}")
    };
    let player = found.player;
    let candidates = found.objects_offered();
    assert_eq!(player, PlayerId(1));
    game.validate_state().unwrap();
    let before = game.state.rules.players[0].life.clone();
    let mut restored = Game::restore(prepared.clone(), &game.snapshot().unwrap()).unwrap();
    for g in [&mut game, &mut restored] {
        {
            let selected = g.next_test_decision().pick_object(candidates[0]);
            answer(g, selected)
        };
        // Inspect the instruction boundary before resolving the rest of the spell.
        // The moving object may be a commander whose owner must choose a zone.
        for _ in 0..64 {
            match g.advance(Budget::new(1)) {
                Advance::AwaitingDecision(Decision {
                    reason: Reason::CommanderReturn { .. },
                    ..
                }) => {
                    {
                        let selected = g.next_test_decision().select(Candidate::Flag {
                            flag: Flag::Destination,
                        });
                        answer(g, selected)
                    };
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
fn a_nested_single_draw_restores_before_drawing() {
    let prepared = edict_with_kernel(
        Node::Seq(vec![Node::Do(Verb::Draw {
            player: PlayerExpr::Is {
                who: Ref::Target { slot: 0 },
            },
            amount: Amount::fixed(1),
        })]),
        TargetDomain::AnyPlayer,
    );
    let mut game = super::formats_tests::cast_edict(prepared.clone());
    let owner = PlayerId(1);
    let library = game.state.rules.libraries[&owner].clone();
    let mut leaf_started = false;
    for _ in 0..32 {
        let next = game.advance(Budget::new(1));
        assert!(matches!(next, Advance::Suspended { .. }), "{next:?}");
        if matches!(&game.state.rules.frame, Frame::Resolve { frame } if frame.cursor == 0)
            && matches!(game.state.rules.frames.last(), Some(Frame::Kernel { .. }))
        {
            leaf_started = true;
            break;
        }
    }
    assert!(leaf_started, "expected the nested draw's resolving frame");
    assert_eq!(game.state.rules.libraries[&owner], library);
    let mut restored = Game::restore(prepared, &game.snapshot().unwrap()).unwrap();
    for g in [&mut game, &mut restored] {
        let next = g.advance(Budget::unlimited());
        assert!(
            matches!(
                next,
                Advance::AwaitingDecision(Decision {
                    reason: Reason::Priority,
                    ..
                })
            ),
            "{next:?}"
        );
        assert_eq!(g.state.rules.libraries[&owner].as_slice(), &library[1..]);
        let drawn = g.object_id(library[0]).unwrap();
        assert_eq!(g.current(drawn).unwrap().zone, Zone::Hand { owner });
        g.validate_state().unwrap();
    }
    assert_eq!(game.snapshot().unwrap(), restored.snapshot().unwrap());
}

#[test]
fn a_forced_graveyard_choice_iterates_only_matching_cards() {
    // CR 109.2a, 404.1: this set describes creature cards in graveyards.
    let from = SetExpr::Matching {
        zone: ZoneExpr::Graveyards,
        filter: Pred::creatures(),
    };
    let size = Amount::Count { set: from.clone() };
    let node = Node::Choose {
        var: 0,
        chooser: controller(),
        from,
        minimum: size.clone(),
        maximum: size,
        body: Box::new(Node::ForEach {
            var: 1,
            over: Domain::Objects {
                set: SetExpr::Pool {
                    pool: Ref::BoundVar { id: 0 },
                },
            },
            body: Box::new(gain(
                PlayerExpr::OwnerOf {
                    object: Ref::BoundVar { id: 1 },
                },
                Amount::fixed(1),
            )),
        }),
    };
    let prepared = edict_with_kernel(node, TargetDomain::OpponentPlayer);
    let mut game = super::formats_tests::cast_edict(prepared);
    // Add a creature and a land to different graveyards. Other creatures
    // remain on the battlefield, including more owned by the matching owner.
    for (card, owner) in [(CardId(1), PlayerId(1)), (CardId(12), PlayerId(2))] {
        let object = game.object_id(card).unwrap();
        game.move_object(object, Zone::Graveyard { owner }).unwrap();
    }
    let mut expected: Vec<_> = game
        .state
        .rules
        .players
        .iter()
        .map(|player| player.life.clone())
        .collect();
    expected[1].add(&Quantity::from(1));
    let next = game.advance(Budget::unlimited());
    assert!(
        matches!(
            next,
            Advance::AwaitingDecision(Decision {
                reason: Reason::Priority,
                ..
            })
        ),
        "{next:?}"
    );
    assert!(
        game.state
            .rules
            .players
            .iter()
            .map(|p| &p.life)
            .eq(&expected)
    );
    game.validate_state().unwrap();
}

fn edict_with_kernel(node: Node, target_domain: TargetDomain) -> Prepared {
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
    program.targets[0].domain = target_domain;
    mtg_contracts::kir::check(
        &node,
        &mtg_contracts::kir::Env {
            context: Context::Spell,
            targets: &program.targets,
            event: None,
            announced_x: false,
        },
    )
    .unwrap();
    program.instructions = vec![Instruction::Kernel {
        node,
        source_block: 0,
    }];
    prepared.catalog_digest = mtg_contracts::digest(catalog).unwrap();
    prepared
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

/// CR 118.12, 608.2c: "you may pay N life. If you do, ..." and "you may pay
/// {0}. If you do, ...": the node runs only after the payer has paid.
fn if_pays_scenario() -> Prepared {
    let mut prepared = super::formats_tests::actions_scenario();
    let catalog = Arc::make_mut(&mut prepared.catalog);
    let program = catalog
        .definitions
        .iter_mut()
        .find(|def| def.name == "Cruel Edict")
        .unwrap()
        .spell
        .as_mut()
        .unwrap();
    let if_pays = |payment, amount| Node::IfPays {
        payer: controller(),
        payment,
        node: Box::new(gain(controller(), Amount::fixed(amount))),
    };
    let node = Node::Seq(vec![
        if_pays(
            Payment::Life {
                amount: Amount::fixed(3),
            },
            10,
        ),
        if_pays(
            Payment::Generic {
                amount: Amount::fixed(0),
            },
            5,
        ),
    ]);
    let derived = mtg_contracts::kir::check(
        &node,
        &mtg_contracts::kir::Env {
            context: Context::Spell,
            targets: &program.targets,
            event: None,
            announced_x: false,
        },
    )
    .unwrap();
    // A payment node is control: it earns no tier above Executes.
    assert!(derived.cells.contains(&mtg_contracts::kir::Cell::Control));
    program.instructions = vec![Instruction::Kernel {
        node,
        source_block: 0,
    }];
    prepared.catalog_digest = mtg_contracts::digest(catalog).unwrap();
    prepared
}

fn pay_generic_zero(game: &mut Game) {
    answer(
        game,
        Answer::Pay {
            pay: PayAnswer::Finish {
                undo_on_failure: false,
            },
        },
    );
    answer(
        game,
        Answer::Pay {
            pay: PayAnswer::Spend {
                unrestricted: BTreeMap::new(),
            },
        },
    );
}

#[test]
fn if_pays_performs_only_once_paid_and_restores_at_the_offer() {
    let mut original = super::formats_tests::cast_edict(if_pays_scenario());
    assert!(matches!(
        original.advance(Budget::unlimited()),
        Advance::AwaitingDecision(Decision {
            player: PlayerId(0),
            reason: Reason::OptionalEffect,
            ..
        })
    ));
    original.validate_state().unwrap();
    let before = original.state.rules.players[0].life.clone();
    let prepared = original.prepared.as_ref().clone();
    let mut restored = Game::restore(prepared.clone(), &original.snapshot().unwrap()).unwrap();
    for game in [&mut original, &mut restored] {
        // Pay 3 life, then {0}: both nodes run.
        for _ in 0..2 {
            let accept = game
                .next_test_decision()
                .select(Candidate::Flag { flag: Flag::Accept });
            answer(game, accept);
            game.validate_state().unwrap();
        }
        pay_generic_zero(game);
        assert!(matches!(
            game.advance(Budget::unlimited()),
            Advance::AwaitingDecision(Decision {
                reason: Reason::Priority,
                ..
            })
        ));
        game.validate_state().unwrap();
        let mut expected = before.clone();
        expected.add(&Quantity::from(-3 + 10 + 5));
        assert_eq!(game.state.rules.players[0].life, expected);
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
fn declining_an_if_pays_offer_skips_its_node() {
    let mut game = super::formats_tests::cast_edict(if_pays_scenario());
    let before = game.state.rules.players[0].life.clone();
    for _ in 0..2 {
        let decline = game.next_test_decision().select(Candidate::Flag {
            flag: Flag::Decline,
        });
        answer(&mut game, decline);
        game.validate_state().unwrap();
    }
    assert!(matches!(
        game.advance(Budget::unlimited()),
        Advance::AwaitingDecision(Decision {
            reason: Reason::Priority,
            ..
        })
    ));
    assert_eq!(game.state.rules.players[0].life, before);
    game.validate_state().unwrap();
}

/// CR 118.12: a suspended life payment is exactly its admitted terms. A
/// snapshot whose saved amount or payer differs from the node's is rejected.
#[test]
fn a_suspended_life_payment_must_match_its_admitted_terms() {
    let mut game = super::formats_tests::cast_edict(if_pays_scenario());
    assert!(matches!(
        game.advance(Budget::unlimited()),
        Advance::AwaitingDecision(Decision {
            reason: Reason::OptionalEffect,
            ..
        })
    ));
    game.validate_state().unwrap();
    // The suspended stage lives in the current frame; edit it in place.
    let frame = serde_json::to_value(&game.state.rules.frame).unwrap();
    fn stages(value: &mut serde_json::Value, edit: &dyn Fn(&mut serde_json::Value)) -> usize {
        match value {
            serde_json::Value::Object(fields) => {
                if fields.get("stage") == Some(&serde_json::json!("life")) {
                    edit(value);
                    return 1;
                }
                fields.values_mut().map(|child| stages(child, edit)).sum()
            }
            serde_json::Value::Array(items) => {
                items.iter_mut().map(|item| stages(item, edit)).sum()
            }
            _ => 0,
        }
    }
    let original = game.state.rules.frame.clone();
    let mut with = |edit: &dyn Fn(&mut serde_json::Value)| {
        let mut changed = frame.clone();
        assert_eq!(stages(&mut changed, edit), 1);
        game.state.rules.frame = serde_json::from_value(changed).unwrap();
        let result = game.validate_state();
        game.state.rules.frame = original.clone();
        result
    };
    // The same terms validate.
    with(&|_| {}).unwrap();
    // A different amount or payer than the admitted node's does not.
    let amount = with(&|stage| {
        stage["amount"] = match &stage["amount"] {
            serde_json::Value::String(_) => serde_json::json!("4"),
            _ => serde_json::json!(4),
        }
    });
    assert!(amount.unwrap_err().contains("life payment differs"));
    let payer = with(&|stage| stage["payer"] = serde_json::json!(1));
    assert!(payer.unwrap_err().contains("payer differs"));
}

/// The Actions edict, cast with `target` and resolved to its first boundary.
fn cast_edict_at(prepared: Prepared, target: TargetEntity) -> Game {
    let mut game = Game::new(prepared);
    let mut answer = |response: &dyn Fn(&Decision<Rules>) -> Answer| {
        let Advance::AwaitingDecision(request) = game.advance(Budget::unlimited()) else {
            panic!("expected a decision")
        };
        game.respond(request.player, request.id, response(&request))
            .unwrap();
    };
    answer(&|d| d.cast(CardId(4)));
    answer(&|d| d.entity_targets(vec![target]));
    let lands = [2, 3].map(|land| Driver::object(&game, land));
    for land in lands {
        let mut answer = |response: &dyn Fn(&Decision<Rules>) -> Answer| {
            let Advance::AwaitingDecision(request) = game.advance(Budget::unlimited()) else {
                panic!("expected a decision")
            };
            game.respond(request.player, request.id, response(&request))
                .unwrap();
        };
        answer(&|d| d.activate_mana(land));
    }
    for response in [
        Answer::Pay {
            pay: PayAnswer::Finish {
                undo_on_failure: true,
            },
        },
        Answer::Pay {
            pay: PayAnswer::Spend {
                unrestricted: BTreeMap::from([
                    (Color::B, Quantity::from(1)),
                    (Color::G, Quantity::from(1)),
                ]),
            },
        },
    ] {
        let Advance::AwaitingDecision(request) = game.advance(Budget::unlimited()) else {
            panic!("expected a payment decision")
        };
        game.respond(request.player, request.id, response).unwrap();
    }
    for _ in 0..game.live_players().len() {
        let Advance::AwaitingDecision(request) = game.advance(Budget::unlimited()) else {
            panic!("expected priority")
        };
        game.respond(
            request.player,
            request.id,
            Answer::Pick {
                picks: vec![vec![0]],
            },
        )
        .unwrap();
    }
    game
}

fn next_reason(game: &mut Game) -> Reason<Rules> {
    let Advance::AwaitingDecision(request) = game.advance(Budget::unlimited()) else {
        panic!("expected a decision")
    };
    request.reason
}

/// CR 400.7, 608.2d: once its target has changed zones, an optional move
/// of that target is impossible, so no choice is offered.
#[test]
fn an_optional_move_of_a_departed_target_offers_no_choice() {
    let target = SetExpr::One {
        object: Ref::Target { slot: 0 },
    };
    let bounce = Node::Do(Verb::Move {
        what: target.clone(),
        to: mtg_contracts::kir::Destination::OwnerHand,
    });
    let optional = Node::Optional {
        chooser: controller(),
        node: Box::new(bounce.clone()),
    };
    for departed in [false, true] {
        let node = if departed {
            Node::Seq(vec![bounce.clone(), optional.clone()])
        } else {
            optional.clone()
        };
        let prepared = edict_with_kernel(node, TargetDomain::BattlefieldCreature);
        let creature = Game::new(prepared.clone()).object_id(CardId(1)).unwrap();
        let mut game = cast_edict_at(prepared, TargetEntity::Object { object: creature });
        game.validate_state().unwrap();
        let reason = next_reason(&mut game);
        if departed {
            assert_eq!(reason, Reason::Priority);
        } else {
            assert_eq!(reason, Reason::OptionalEffect);
        }
    }
}

fn each_opponent_gains(amount: i64) -> Node {
    Node::ForEach {
        var: 5,
        over: Domain::Players {
            players: PlayerExpr::EachOpponent,
        },
        body: Box::new(Node::Optional {
            chooser: controller(),
            node: Box::new(gain(
                PlayerExpr::Is {
                    who: Ref::BoundVar { id: 5 },
                },
                Amount::fixed(amount),
            )),
        }),
    }
}

/// CR 608.2d, 800.4a: an optional life change of a player who has left the
/// game, or of zero life, offers no choice; live players' iterations do.
#[test]
fn an_optional_life_change_needs_a_live_player_and_an_amount() {
    let each = each_opponent_gains;
    let mut game =
        super::formats_tests::cast_edict(edict_with_kernel(each(1), TargetDomain::OpponentPlayer));
    assert!(game.live_players().len() >= 4);
    let before: Vec<_> = game
        .state
        .rules
        .players
        .iter()
        .map(|p| p.life.clone())
        .collect();
    // Player 1's iteration is offered; player 2 concedes meanwhile.
    assert_eq!(next_reason(&mut game), Reason::OptionalEffect);
    game.concede(PlayerId(2)).unwrap();
    let mut accepted = 0;
    loop {
        let Advance::AwaitingDecision(request) = game.advance(Budget::unlimited()) else {
            panic!("expected a decision")
        };
        if request.reason != Reason::OptionalEffect {
            break;
        }
        accepted += 1;
        let accept = request
            .pick_one(&Candidate::Flag { flag: Flag::Accept })
            .unwrap();
        game.respond(request.player, request.id, accept).unwrap();
        game.validate_state().unwrap();
    }
    // Players 1 and 3 only.
    assert_eq!(accepted, 2);
    for seat in [1usize, 3] {
        let mut expected = before[seat].clone();
        expected.add(&Quantity::from(1));
        assert_eq!(game.state.rules.players[seat].life, expected);
    }
    // Gaining zero life is no choice at all.
    let mut zero =
        super::formats_tests::cast_edict(edict_with_kernel(each(0), TargetDomain::OpponentPlayer));
    assert_eq!(next_reason(&mut zero), Reason::Priority);
}

/// CR 118.12: a suspended generic payment is exactly its admitted amount.
#[test]
fn a_suspended_generic_payment_must_match_its_admitted_amount() {
    let mut game = super::formats_tests::cast_edict(if_pays_scenario());
    // Pay the 3 life; the {0} payment is then offered.
    let accept = game
        .next_test_decision()
        .select(Candidate::Flag { flag: Flag::Accept });
    answer(&mut game, accept);
    assert_eq!(next_reason(&mut game), Reason::OptionalEffect);
    assert!(matches!(
        game.state.rules.frame,
        Frame::ResolutionPayment { .. }
    ));
    game.validate_state().unwrap();
    let frame = serde_json::to_value(&game.state.rules.frame).unwrap();
    fn generic(value: &mut serde_json::Value) -> usize {
        match value {
            serde_json::Value::Object(fields) => {
                if let Some(cost) = fields.get_mut("cost")
                    && cost.get("generic").is_some()
                {
                    cost["generic"] = match &cost["generic"] {
                        serde_json::Value::String(_) => serde_json::json!("9"),
                        _ => serde_json::json!(9),
                    };
                    return 1;
                }
                fields.values_mut().map(generic).sum()
            }
            serde_json::Value::Array(items) => items.iter_mut().map(generic).sum(),
            _ => 0,
        }
    }
    let mut changed = frame;
    assert_eq!(generic(&mut changed), 1);
    game.state.rules.frame = serde_json::from_value(changed).unwrap();
    assert!(
        game.validate_state()
            .unwrap_err()
            .contains("resolution payment binding invariant")
    );
}

/// CR 608.2d, 800.4a: a pending optional choice is withdrawn when it
/// becomes impossible while it waits. Player 1 concedes while the offer to
/// gain them life is pending; only players 2 and 3 are then offered.
#[test]
fn a_pending_optional_choice_is_withdrawn_once_it_is_impossible() {
    let mut game = super::formats_tests::cast_edict(edict_with_kernel(
        each_opponent_gains(1),
        TargetDomain::OpponentPlayer,
    ));
    let before: Vec<_> = game
        .state
        .rules
        .players
        .iter()
        .map(|p| p.life.clone())
        .collect();
    assert_eq!(next_reason(&mut game), Reason::OptionalEffect);
    game.concede(PlayerId(1)).unwrap();
    game.validate_state().unwrap();
    let mut offered = 0;
    loop {
        let Advance::AwaitingDecision(request) = game.advance(Budget::unlimited()) else {
            panic!("expected a decision")
        };
        if request.reason != Reason::OptionalEffect {
            break;
        }
        offered += 1;
        let accept = request
            .pick_one(&Candidate::Flag { flag: Flag::Accept })
            .unwrap();
        game.respond(request.player, request.id, accept).unwrap();
        game.validate_state().unwrap();
    }
    assert_eq!(offered, 2);
    assert_eq!(game.state.rules.players[1].life, before[1]);
    for seat in [2usize, 3] {
        let mut expected = before[seat].clone();
        expected.add(&Quantity::from(1));
        assert_eq!(game.state.rules.players[seat].life, expected);
    }
}

fn creature_payment_game(has_creature: bool) -> Game {
    let mut prepared = super::formats_tests::actions_scenario();
    if has_creature {
        prepared.config.cards[1].controller = PlayerId(0);
    }
    let catalog = Arc::make_mut(&mut prepared.catalog);
    let program = catalog
        .definitions
        .iter_mut()
        .find(|d| d.name == "Cruel Edict")
        .unwrap()
        .spell
        .as_mut()
        .unwrap();
    program.instructions = vec![Instruction::Kernel {
        node: Node::IfPays {
            payer: controller(),
            payment: Payment::SacrificeCreature {},
            node: Box::new(gain(controller(), Amount::fixed(5))),
        },
        source_block: 0,
    }];
    prepared.catalog_digest = mtg_contracts::digest(catalog).unwrap();
    super::formats_tests::cast_edict(prepared)
}

pub(super) fn suspended_creature_payment() -> Game {
    let mut game = creature_payment_game(true);
    assert_eq!(next_reason(&mut game), Reason::OptionalEffect);
    game.validate_state().unwrap();
    game
}

#[test]
fn creature_payment_checks_feasibility_and_restores_the_exact_choice() {
    let mut unavailable = creature_payment_game(false);
    let starting_life = unavailable.state.rules.players[0].life.clone();
    assert_eq!(next_reason(&mut unavailable), Reason::Priority);
    assert_eq!(unavailable.state.rules.players[0].life, starting_life);

    for pay in [true, false] {
        let original = suspended_creature_payment();
        let mut game = Game::restore(
            original.prepared.as_ref().clone(),
            &original.snapshot().unwrap(),
        )
        .unwrap();
        let creature = game.object_id(CardId(1)).unwrap();
        let choice = if pay {
            Candidate::Object { object: creature }
        } else {
            Candidate::Flag {
                flag: Flag::Decline,
            }
        };
        let response = game.next_test_decision().select(choice);
        answer(&mut game, response);
        assert_eq!(next_reason(&mut game), Reason::Priority);
        game.validate_state().unwrap();
        let mut expected = starting_life.clone();
        if pay {
            expected.add(&Quantity::from(5));
        }
        assert_eq!(game.state.rules.players[0].life, expected);
    }
}
