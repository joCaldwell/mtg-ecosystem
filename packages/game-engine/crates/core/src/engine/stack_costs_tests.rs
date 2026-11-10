use super::*;

fn prepared() -> Prepared {
    Prepared::admit(
        Catalog::read(include_str!(
            "../../../../tests/fixtures/expected-stack-ir.json"
        ))
        .unwrap(),
        ScenarioConfig::stack_costs(),
    )
    .unwrap()
}
fn game() -> Game {
    Game::new(prepared())
}
fn settle(game: &mut Game) -> Request {
    for _ in 0..500 {
        match game.advance(Budget::new(1)) {
            Advance::AwaitingDecision(request) => return request,
            Advance::Suspended { .. } => (),
            other => panic!("unexpected boundary: {other:?}"),
        }
    }
    panic!("decision budget exhausted")
}
fn answer(game: &mut Game, response: Response) {
    let request = settle(game);
    game.respond(request.player(), request.decision_id(), response)
        .unwrap();
}
fn activate_mana(game: &mut Game, card: u32) {
    let object = game.object_id(CardId(card)).unwrap();
    answer(game, Response::ActivateMana { object });
}
fn ability(game: &mut Game, card: u32) {
    let object = game.object_id(CardId(card)).unwrap();
    answer(game, Response::ActivateAbility { object, ability: 0 });
}
fn bears_on_stack() -> Game {
    let mut game = game();
    answer(&mut game, Response::Cast { card: CardId(10) });
    activate_mana(&mut game, 1);
    activate_mana(&mut game, 6);
    answer(&mut game, Response::FinishMana { undo_mana: false });
    answer(
        &mut game,
        Response::SpendMana {
            spending: [(Color::U, 1.into()), (Color::G, 1.into())].into(),
        },
    );
    assert!(matches!(settle(&mut game), Request::Priority { .. }));
    game
}
fn with_counter(mut game: Game, card: u32) -> Game {
    let target = game.state.rules.stack[0].id;
    answer(&mut game, Response::Pass);
    answer(&mut game, Response::Cast { card: CardId(card) });
    answer(
        &mut game,
        Response::StackTargets {
            entries: vec![target],
        },
    );
    let count = if card == 11 { 2 } else { 1 };
    for id in 15..15 + count {
        activate_mana(&mut game, id);
    }
    answer(&mut game, Response::FinishMana { undo_mana: false });
    answer(
        &mut game,
        Response::SpendMana {
            spending: BTreeMap::from([(Color::U, Quantity::from(i64::from(count)))]),
        },
    );
    settle(&mut game);
    assert_eq!(game.state.rules.stack.len(), 2);
    assert_eq!(game.state.rules.stack[1].id, StackId(1));
    game
}
fn counter(target: StackId, domain: TargetDomain) -> ProposedEvent {
    ProposedEvent::CounterStackObject {
        target,
        countering: StackId(1),
        domain,
    }
}
fn recover(game: &Game) {
    let restored =
        Game::restore(game.prepared.as_ref().clone(), &game.snapshot().unwrap()).unwrap();
    let replayed = Game::replay(game.prepared.as_ref().clone(), &game.replay_journal()).unwrap();
    assert_eq!(
        restored.state_digest().unwrap(),
        game.state_digest().unwrap()
    );
    assert_eq!(
        replayed.state_digest().unwrap(),
        game.state_digest().unwrap()
    );
    assert_eq!(restored.trace(), game.trace());
    assert_eq!(replayed.trace(), game.trace());
}

#[test]
fn counter_event_consumes_exact_spell_membership_and_actual_physical_successor() {
    let mut game = with_counter(bears_on_stack(), 11);
    let entry = game.state.rules.stack[0].clone();
    let mana = game.state.rules.players[0].mana.clone();
    let receipt = game
        .commit_batch(
            BatchCause::Fixture,
            EventGroup::Occurrence {
                proposed: counter(entry.id, TargetDomain::StackSpell),
            },
        )
        .unwrap();
    assert_eq!(receipt.before.stack.len(), 1);
    assert_eq!(receipt.before.stack[0].id, entry.id);
    assert_eq!(receipt.before.stack[0].source, entry.source);
    assert_eq!(receipt.before.stack[0].paid, entry.paid);
    assert!(receipt.before.stack[0].spell);
    assert_eq!(receipt.before.objects[0].zone, Zone::Stack);
    let Some(EventOutcome::StackObjectCountered {
        target,
        spell,
        movement: Some(movement),
        ..
    }) = receipt.result(&[])
    else {
        panic!("missing actual spell counter movement")
    };
    assert_eq!(*target, entry.id);
    assert!(*spell);
    assert_eq!(movement.previous, entry.source);
    assert_eq!(movement.successor.incarnation, entry.source.incarnation + 1);
    assert_eq!(movement.destination, Zone::Graveyard { owner: PlayerId(0) });
    assert_eq!(game.object_id(CardId(10)), Some(movement.successor));
    assert_eq!(game.state.rules.stack.len(), 1);
    assert_eq!(game.state.rules.stack[0].id, StackId(1));
    assert_eq!(game.state.rules.players[0].mana, mana);
}

#[test]
fn countered_noncard_invocation_retains_paid_tap_and_current_source() {
    let mut game = game();
    ability(&mut game, 7);
    settle(&mut game);
    let mut game = with_counter(game, 12);
    let entry = game.state.rules.stack[0].clone();
    assert!(game.current(entry.source).unwrap().tapped);
    let receipt = game
        .commit_batch(
            BatchCause::Fixture,
            EventGroup::Occurrence {
                proposed: counter(entry.id, TargetDomain::StackActivatedOrTriggeredAbility),
            },
        )
        .unwrap();
    assert_eq!(receipt.before.stack.len(), 1);
    assert!(!receipt.before.stack[0].spell);
    assert!(receipt.before.objects.is_empty());
    assert!(matches!(
        receipt.result(&[]),
        Some(EventOutcome::StackObjectCountered {
            spell: false,
            movement: None,
            ..
        })
    ));
    assert_eq!(game.state.rules.stack.len(), 1);
    assert_eq!(game.state.rules.stack[0].id, StackId(1));
    assert_eq!(game.object_id(CardId(7)), Some(entry.source));
    assert_eq!(game.current(entry.source).unwrap().zone, Zone::Battlefield);
    assert!(game.current(entry.source).unwrap().tapped);
}

#[test]
fn counter_event_failures_and_aggregate_conflicts_roll_back_every_write_and_receipt() {
    let initial = with_counter(bears_on_stack(), 11);
    let target = initial.state.rules.stack[0].id;
    let land = initial.object_id(CardId(2)).unwrap();
    for group in [
        EventGroup::Ordered {
            children: vec![
                EventGroup::Occurrence {
                    proposed: ProposedEvent::Move {
                        object: land,
                        destination: Zone::Exile,
                    },
                },
                EventGroup::Occurrence {
                    proposed: counter(target, TargetDomain::StackActivatedOrTriggeredAbility),
                },
            ],
        },
        EventGroup::Ordered {
            children: vec![
                EventGroup::Occurrence {
                    proposed: counter(target, TargetDomain::StackSpell),
                },
                EventGroup::Occurrence {
                    proposed: counter(target, TargetDomain::StackSpell),
                },
            ],
        },
        EventGroup::Simultaneous {
            occurrences: vec![
                counter(target, TargetDomain::StackSpell),
                ProposedEvent::Tap { object: land },
            ],
        },
        EventGroup::Occurrence {
            proposed: counter(target, TargetDomain::StackActivatedOrTriggeredAbility),
        },
        EventGroup::Occurrence {
            proposed: ProposedEvent::CounterStackObject {
                target,
                countering: target,
                domain: TargetDomain::StackSpell,
            },
        },
    ] {
        let mut branch = initial.fork();
        let token = branch.state_token.clone();
        assert!(branch.commit_batch(BatchCause::Fixture, group).is_err());
        assert_eq!(branch.state, initial.state);
        assert_eq!(branch.trace, initial.trace);
        assert!(Arc::ptr_eq(&token, &branch.state_token));
    }
}

#[test]
fn failed_mana_activation_reverses_only_its_receipts_and_preserves_preexisting_pool() {
    let mut initial = game();
    activate_mana(&mut initial, 1);
    let source = initial.object_id(CardId(8)).unwrap();
    ability(&mut initial, 8);
    assert!(matches!(settle(&mut initial), Request::Mana { .. }));
    activate_mana(&mut initial, 2);
    for undo in [false, true] {
        let mut branch = initial.fork();
        answer(&mut branch, Response::FinishMana { undo_mana: undo });
        assert!(matches!(settle(&mut branch), Request::Priority { .. }));
        assert!(branch.state.rules.stack.is_empty());
        assert_eq!(branch.object_id(CardId(8)), Some(source));
        assert!(!branch.current(source).unwrap().tapped);
        assert!(
            branch
                .current(branch.object_id(CardId(1)).unwrap())
                .unwrap()
                .tapped
        );
        assert_eq!(
            branch
                .current(branch.object_id(CardId(2)).unwrap())
                .unwrap()
                .tapped,
            !undo
        );
        assert_eq!(
            branch.mana(PlayerId(0), Color::U),
            Some(Quantity::from(if undo { 1 } else { 2 }))
        );
        assert!(
            !branch
                .events()
                .iter()
                .any(|event| matches!(event, Event::AbilityActivated { .. }))
        );
        recover(&branch);
    }
}

#[test]
fn reversing_mana_activation_preserves_naturally_tapped_attacking_source() {
    let mut game = game();
    let source = game.object_id(CardId(8)).unwrap();
    let mut declared = false;
    for _ in 0..32 {
        let request = settle(&mut game);
        match request {
            Request::Attackers { .. } => {
                answer(
                    &mut game,
                    Response::Attackers {
                        attacks: vec![(source, PlayerId(1))],
                    },
                );
                declared = true;
                break;
            }
            Request::Priority { .. } => answer(&mut game, Response::Pass),
            other => panic!("unexpected precombat boundary: {other:?}"),
        }
    }
    assert!(declared);
    settle(&mut game);
    assert!(game.current(source).unwrap().tapped);
    let combat = game.state.rules.combat.clone();
    ability(&mut game, 8);
    activate_mana(&mut game, 1);
    answer(&mut game, Response::FinishMana { undo_mana: true });
    settle(&mut game);
    assert!(game.state.rules.stack.is_empty());
    assert_eq!(game.object_id(CardId(8)), Some(source));
    assert!(game.current(source).unwrap().tapped);
    assert_eq!(game.state.rules.combat, combat);
    assert!(
        !game
            .current(game.object_id(CardId(1)).unwrap())
            .unwrap()
            .tapped
    );
    assert_eq!(game.mana(PlayerId(0), Color::U), Some(Quantity::zero()));
    recover(&game);
}

#[test]
fn successful_mana_activation_pays_exact_selected_cost_without_moving_or_tapping_source() {
    let mut game = game();
    let source = game.object_id(CardId(8)).unwrap();
    ability(&mut game, 8);
    for card in 1..=4 {
        activate_mana(&mut game, card);
    }
    answer(&mut game, Response::FinishMana { undo_mana: true });
    assert!(matches!(settle(&mut game), Request::SpendMana { .. }));
    let spending = BTreeMap::from([(Color::U, Quantity::from(4))]);
    answer(
        &mut game,
        Response::SpendMana {
            spending: spending.clone(),
        },
    );
    settle(&mut game);
    assert_eq!(game.state.rules.stack.len(), 1);
    assert_eq!(
        game.state.rules.stack[0].paid,
        Some(ManaCost {
            generic: 3.into(),
            colored: vec![Color::U]
        })
    );
    assert_eq!(game.state.rules.stack[0].source, source);
    assert_eq!(game.current(source).unwrap().zone, Zone::Battlefield);
    assert!(!game.current(source).unwrap().tapped);
    assert_eq!(game.mana(PlayerId(0), Color::U), Some(Quantity::zero()));
    assert!(
        game.events()
            .iter()
            .any(|event| matches!(event, Event::Committed {
        actual: EventOutcome::ManaPaidSelected {spending: actual, ..}, ..
    } if actual == &spending))
    );
    assert_eq!(
        game.events()
            .iter()
            .filter(|event| matches!(event, Event::AbilityActivated { .. }))
            .count(),
        1
    );
    recover(&game);
}

#[test]
fn rehashed_stack_bindings_and_activation_origin_cannot_forge_reachable_snapshots() {
    let mut game = bears_on_stack();
    answer(&mut game, Response::Pass);
    answer(&mut game, Response::Cast { card: CardId(11) });
    let target = game.state.rules.stack[0].id;
    answer(
        &mut game,
        Response::StackTargets {
            entries: vec![target],
        },
    );
    assert!(matches!(settle(&mut game), Request::Mana { .. }));
    recover(&game);
    for mutation in 0..6 {
        let mut snapshot: Snapshot = strict_read(&game.snapshot().unwrap()).unwrap();
        let own_id = snapshot.state.rules.stack[1].id;
        match mutation {
            0 => snapshot.state.rules.stack[1].stack_targets[0] = own_id,
            1 => snapshot.state.rules.stack[1].stack_targets[0] = StackId(999),
            2 => snapshot.state.rules.stack[1].announced_stack_targets[0].clear(),
            3 => snapshot.state.rules.stack[1].cast_from = Zone::Battlefield,
            4 => {
                if let Frame::Mana { cast } = &mut snapshot.state.rules.frame {
                    cast.origin = stack_costs::AnnouncementOrigin::Activated;
                }
            }
            _ => {
                if let Frame::Mana { cast } = &mut snapshot.state.rules.frame {
                    cast.original_object.card = CardId(8);
                }
            }
        }
        snapshot.state_sha256 = digest(&snapshot.state).unwrap();
        assert!(
            Game::restore(
                game.prepared.as_ref().clone(),
                &canonical(&snapshot).unwrap()
            )
            .is_err(),
            "mutation {mutation}"
        );
    }
    let mut game = Game::new(prepared());
    ability(&mut game, 8);
    settle(&mut game);
    let mut snapshot: Snapshot = strict_read(&game.snapshot().unwrap()).unwrap();
    if let Frame::Mana { cast } = &mut snapshot.state.rules.frame {
        cast.origin = stack_costs::AnnouncementOrigin::Spell;
    }
    snapshot.state_sha256 = digest(&snapshot.state).unwrap();
    assert!(
        Game::restore(
            game.prepared.as_ref().clone(),
            &canonical(&snapshot).unwrap()
        )
        .is_err()
    );
}

#[test]
fn exact_stack_profile_admission_rejects_missing_old_extra_and_changed_semantics() {
    let catalog = (*prepared().catalog).clone();
    for config in [
        ScenarioConfig::stack_costs(),
        ScenarioConfig::stack_costs_borrowed(),
        ScenarioConfig::stack_costs_empty_library(),
        ScenarioConfig::stack_costs_library(),
    ] {
        Prepared::admit(catalog.clone(), config).unwrap();
    }
    let old = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-action-ir.json"
    ))
    .unwrap();
    assert!(Prepared::admit(old, ScenarioConfig::stack_costs()).is_err());
    for mutation in 0..5 {
        let mut bad = catalog.clone();
        match mutation {
            0 => bad.schema_version = 5,
            1 => {
                bad.definitions.pop();
            }
            2 => {
                bad.definitions
                    .iter_mut()
                    .find(|d| d.name == "Azure Mage")
                    .unwrap()
                    .characteristics
                    .mana_cost
                    .generic = 2.into()
            }
            3 => {
                bad.definitions
                    .iter_mut()
                    .find(|d| d.name == "Stifle")
                    .unwrap()
                    .spell
                    .as_mut()
                    .unwrap()
                    .targets[0]
                    .domain = TargetDomain::StackSpell
            }
            _ => {
                let mut extra = bad.definitions[0].clone();
                extra.oracle_id = "zz-extra-definition".into();
                bad.definitions.push(extra);
            }
        }
        assert!(Prepared::admit(bad, ScenarioConfig::stack_costs()).is_err());
    }
    for mutation in 0..5 {
        let mut bad = ScenarioConfig::stack_costs();
        match mutation {
            0 => bad.cards[7].controller = PlayerId(1),
            1 => bad.cards[8].zone = Zone::Hand { owner: PlayerId(0) },
            2 => bad.commanders = vec![CardId(7)],
            3 => bad.cards[10].oracle_id = bad.cards[11].oracle_id.clone(),
            _ => {
                let mut extra = bad.cards[1].clone();
                extra.id = CardId(100);
                bad.cards.push(extra);
            }
        }
        assert!(Prepared::admit(catalog.clone(), bad).is_err());
    }
}
