use super::*;
fn catalog() -> Catalog {
    Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-choices-ir.json"
    ))
    .unwrap()
}
fn game(config: ScenarioConfig) -> Game {
    Game::new(Prepared::admit(catalog(), config).unwrap())
}
fn object(g: &Game, id: u32) -> ObjectId {
    g.object_id(CardId(id)).unwrap()
}
fn seed(
    g: &mut Game,
    source: u32,
    mode: Option<u32>,
    cursor: u32,
    kind: ResolvedEffectKind,
) -> EffectId {
    let id = EffectId(g.state.next_effect_id);
    let source = object(g, source);
    let source_handles = g.capture_trigger_source_handles(source).unwrap();
    let target_handles = match &kind {
        ResolvedEffectKind::ModifyPowerToughness { target, .. } => {
            g.capture_trigger_source_handles(*target).unwrap()
        }
        _ => vec![],
    };
    g.state.next_effect_id += 1;
    g.state.next_stack_id = 1;
    g.state.effects.push(ResolvedEffect {
        id,
        origin: EffectOrigin {
            mode,
            stack_id: StackId(0),
            source,
            controller: PlayerId(0),
            source_handles,
            cursor,
            source_block: cursor,
        },
        expires_in_cleanup_of_turn: g.state.turn.number,
        kind,
        target_handles,
    });
    id
}
fn allocation() -> (Game, EffectKey) {
    let mut g = game(ScenarioConfig::choices());
    for (cursor, id) in [(0, 16), (1, 17)] {
        let target = object(&g, id);
        seed(
            &mut g,
            4,
            None,
            cursor,
            ResolvedEffectKind::ModifyPowerToughness {
                target,
                power: 1.into(),
                toughness: 1.into(),
            },
        );
    }
    let shield = seed(
        &mut g,
        5,
        Some(1),
        0,
        ResolvedEffectKind::PreventNextDamage {
            target: TargetEntity::Player {
                player: PlayerId(0),
            },
            remaining: 3.into(),
        },
    );
    let effect = EffectKey::Resolved { id: shield };
    let damage = [16, 17]
        .into_iter()
        .map(|id| DamageOccurrence {
            source: object(&g, id),
            controller: PlayerId(1),
            recipient: DamageRecipient::Player {
                player: PlayerId(0),
            },
            amount: 3.into(),
        })
        .collect();
    g.prepare_combat_damage(damage).unwrap();
    g.damage_effect_step().unwrap();
    assert!(matches!(g.state.frame, Frame::DamageAllocation { .. }));
    (g, effect)
}
#[test]
fn six_choice_profiles_bind_complete33_catalog_and_owned_limits() {
    for config in [
        ScenarioConfig::choices(),
        ScenarioConfig::choices_furnace(),
        ScenarioConfig::conditions(),
        ScenarioConfig::conditions_nonempty(),
        ScenarioConfig::conditions_borrowed(),
        ScenarioConfig::conditions_no_targets(),
    ] {
        let p = Prepared::admit(catalog(), config.clone()).unwrap();
        assert_eq!(p.catalog.definitions.len(), 33);
        for seat in 0..4 {
            assert!(
                config
                    .cards
                    .iter()
                    .filter(|c| c.owner == PlayerId(seat))
                    .count()
                    <= 16
            )
        }
        for version in 3..10 {
            let mut c = catalog();
            c.schema_version = version;
            assert!(Prepared::admit(c, config.clone()).is_err())
        }
        let mut changed = config;
        changed.cards[0].zone = Zone::Exile;
        assert!(Prepared::admit(catalog(), changed).is_err());
    }
    assert!(Prepared::admit(catalog(), ScenarioConfig::effects()).is_err());
}
#[test]
fn modal_origin_selects_only_its_source_program_and_exact_budget() {
    let mut g = game(ScenarioConfig::choices());
    let id = seed(
        &mut g,
        5,
        Some(1),
        0,
        ResolvedEffectKind::PreventNextDamage {
            target: TargetEntity::Player {
                player: PlayerId(0),
            },
            remaining: 3.into(),
        },
    );
    let e = g.state.effects.iter().find(|e| e.id == id).unwrap();
    assert_eq!(g.shield_limit(e).unwrap(), 3.into());
    let mut wrong = e.clone();
    wrong.origin.mode = Some(0);
    assert!(g.shield_limit(&wrong).is_err());
    wrong.origin.mode = None;
    assert!(g.shield_limit(&wrong).is_err());
    wrong.origin.mode = Some(2);
    assert!(g.shield_limit(&wrong).is_err());
    let root = g.definition(CardId(5)).unwrap().spell.as_ref().unwrap();
    assert_eq!(root.modes.len(), 2);
    assert!(root.instructions.is_empty());
    assert!(matches!(
        g.selected_program(root, Some(0)).unwrap().instructions[0],
        Instruction::GainLife { .. }
    ));
    assert!(matches!(
        g.selected_program(root, Some(1)).unwrap().instructions[0],
        Instruction::PreventNextDamage { .. }
    ));
}
#[test]
fn intervening_match_uses_common_lookback_and_never_retroactively_triggers() {
    let g = game(ScenarioConfig::conditions_nonempty());
    let bouncer = object(&g, 2);
    let salve = object(&g, 5);
    let death = ProposedEvent::Move {
        object: bouncer,
        destination: Zone::Graveyard { owner: PlayerId(0) },
    };
    let discard = ProposedEvent::DiscardCard {
        object: salve,
        player: PlayerId(0),
    };
    let mut simultaneous = g.fork();
    simultaneous
        .commit_batch(
            BatchCause::Fixture,
            EventGroup::Simultaneous {
                occurrences: vec![discard.clone(), death.clone()],
            },
        )
        .unwrap();
    assert!(
        !simultaneous
            .state
            .pending_triggers
            .iter()
            .any(|t| t.source == bouncer)
    );
    assert_eq!(simultaneous.state.next_trigger_id, 2);
    assert!(simultaneous.events().iter().any(|e|matches!(e,Event::TriggerConditionChecked{source,stage:InterveningStage::Match{..},check,..} if *source==bouncer&&check.hand_count==1&&!check.passed)));
    let mut ordered = g;
    ordered
        .commit_batch(
            BatchCause::Fixture,
            EventGroup::Ordered {
                children: vec![
                    EventGroup::Occurrence { proposed: discard },
                    EventGroup::Occurrence { proposed: death },
                ],
            },
        )
        .unwrap();
    let trigger = ordered
        .state
        .pending_triggers
        .iter()
        .find(|t| t.source == bouncer)
        .unwrap();
    assert!(trigger.match_condition.as_ref().unwrap().passed);
    assert_eq!(ordered.state.next_trigger_id, 3);
}
#[test]
fn allocation_requires_complete_forced_vector_and_rejects_atomically() {
    let (mut g, effect) = allocation();
    let id = g.damage_transaction().unwrap().id;
    let baseline = g.state.clone();
    for vector in [
        vec![],
        vec![(DamageId(0), 3.into())],
        vec![(DamageId(0), 1.into()), (DamageId(0), 2.into())],
        vec![(DamageId(0), (-1).into()), (DamageId(1), 4.into())],
        vec![(DamageId(0), 1.into()), (DamageId(1), 1.into())],
        vec![(DamageId(0), 3.into()), (DamageId(1), 1.into())],
        vec![(DamageId(0), 1.into()), (DamageId(9), 2.into())],
    ] {
        assert!(
            g.commit(
                ProposedEvent::AllocateDamageEffect {
                    transaction: id,
                    effect: effect.clone(),
                    allocations: vector
                },
                BatchCause::Fixture
            )
            .is_err()
        );
        assert_eq!(g.state, baseline);
    }
    assert!(
        g.commit(
            ProposedEvent::ApplyDamageEffect {
                transaction: id,
                effect: effect.clone(),
                damage: DamageId(0)
            },
            BatchCause::Fixture
        )
        .is_err()
    );
    assert_eq!(g.state, baseline);
    assert!(
        g.commit(
            ProposedEvent::CommitDamage { transaction: id },
            BatchCause::Fixture
        )
        .is_err()
    );
    assert_eq!(g.state, baseline);
    let out = g
        .commit(
            ProposedEvent::AllocateDamageEffect {
                transaction: id,
                effect,
                allocations: vec![(DamageId(0), 1.into()), (DamageId(1), 2.into())],
            },
            BatchCause::Fixture,
        )
        .unwrap();
    assert!(
        matches!(out,EventOutcome::DamageEffectAllocated{applications,..} if applications.iter().map(|a|a.prevented.to_string()).collect::<Vec<_>>()==["1","2"])
    );
    assert_eq!(g.state.players[0].life, 40.into());
    g.validate_damage_transaction(g.damage_transaction().unwrap())
        .unwrap();
}
#[test]
fn every_integral_split_is_supported_and_prefix_vector_is_proved() {
    for first in 0..=3 {
        let (mut g, effect) = allocation();
        let id = g.damage_transaction().unwrap().id;
        g.commit(
            ProposedEvent::AllocateDamageEffect {
                transaction: id,
                effect,
                allocations: vec![
                    (DamageId(0), first.into()),
                    (DamageId(1), (3 - first).into()),
                ],
            },
            BatchCause::Fixture,
        )
        .unwrap();
        let Frame::DamageAllocation { transaction, .. } = g.state.frame.clone() else {
            panic!()
        };
        g.state.frame = Frame::DamageEffects { transaction };
        let expected = [Quantity::from(3 - first), Quantity::from(first)];
        assert_eq!(
            g.damage_transaction()
                .unwrap()
                .entries
                .iter()
                .map(|d| d.amount.clone())
                .collect::<Vec<_>>(),
            expected
        );
        let baseline = g.state.clone();
        for mutant in 0..5 {
            g.state = baseline.clone();
            let Frame::DamageEffects { transaction } = &mut g.state.frame else {
                panic!()
            };
            let record = &mut transaction.allocation_groups[0];
            match mutant {
                0 => record.allocations[0].1 = 9.into(),
                1 => record.budget_after = 1.into(),
                2 => record.application_count += 1,
                3 => record.application_start = 99,
                _ => transaction.applications[0].prevented = 9.into(),
            };
            let before = g.state.clone();
            assert!(
                g.commit(
                    ProposedEvent::CommitDamage { transaction: id },
                    BatchCause::Fixture
                )
                .is_err()
            );
            assert_eq!(g.state, before)
        }
        g.state = baseline;
        let out = g
            .commit(
                ProposedEvent::CommitDamage { transaction: id },
                BatchCause::Fixture,
            )
            .unwrap();
        assert!(
            matches!(out,EventOutcome::DamageCommitted{results,..} if results[0].current==37.into())
        );
    }
}
#[test]
fn allocation_late_failure_rolls_back_budget_and_every_application() {
    let (mut g, effect) = allocation();
    let id = g.damage_transaction().unwrap().id;
    let before = g.state.clone();
    assert!(
        g.commit_batch(
            BatchCause::Fixture,
            EventGroup::Ordered {
                children: vec![
                    EventGroup::Occurrence {
                        proposed: ProposedEvent::AllocateDamageEffect {
                            transaction: id,
                            effect,
                            allocations: vec![(DamageId(0), 1.into()), (DamageId(1), 2.into())]
                        }
                    },
                    EventGroup::Occurrence {
                        proposed: ProposedEvent::CommitDamage { transaction: id }
                    }
                ]
            }
        )
        .is_err()
    );
    assert_eq!(g.state, before);
}
fn answer(g: &mut Game, response: Response) {
    let Advance::AwaitingDecision(request) = g.advance(Budget::unlimited()) else {
        panic!("missing decision at {:?}", g.state.frame)
    };
    g.respond(request.player(), request.decision_id(), response)
        .unwrap();
}
fn context_with_prefix_recovery(g: &Game) -> SearchContext {
    let before = g.state_digest().unwrap();
    let QueryValue::SearchContext(expected) =
        (match g.search_context(PlayerId(0), "fixed-seed".into(), Budget::unlimited()) {
            QueryResult::Complete(v) => v,
            _ => panic!("context"),
        })
    else {
        panic!()
    };
    let restored = Game::restore(g.prepared.clone(), &g.snapshot().unwrap()).unwrap();
    let mut result = restored.search_context(PlayerId(0), "fixed-seed".into(), Budget::new(0));
    let mut steps = 0;
    loop {
        result = match result {
            QueryResult::Complete(QueryValue::SearchContext(actual)) => {
                assert_eq!(actual, expected);
                assert_eq!(g.state_digest().unwrap(), before);
                return actual;
            }
            QueryResult::Incomplete { work, .. } => {
                let saved = restored.save_work(&work).unwrap();
                let recovered = restored.restore_work(&saved).unwrap();
                steps += 1;
                assert!(steps < 1000);
                restored.resume_work(recovered, Budget::new(1))
            }
            _ => panic!("context recovery rejected"),
        };
    }
}
#[test]
fn natural_effect_context_and_modal_observation_recover_every_work_prefix() {
    let mut g = game(ScenarioConfig::choices_furnace());
    let ox = object(&g, 1);
    answer(&mut g, Response::Cast { card: CardId(4) });
    answer(
        &mut g,
        Response::Targets {
            objects: vec![ox, ox, ox],
        },
    );
    for id in [7, 8] {
        let source = object(&g, id);
        answer(&mut g, Response::ActivateMana { object: source });
    }
    answer(&mut g, Response::FinishMana { undo_mana: false });
    answer(
        &mut g,
        Response::SpendMana {
            spending: [(Color::G, 1.into()), (Color::W, 1.into())].into(),
        },
    );
    for _ in 0..4 {
        answer(&mut g, Response::Pass);
    }
    assert!(matches!(
        g.advance(Budget::unlimited()),
        Advance::AwaitingDecision(Request::Priority { .. })
    ));
    let context = context_with_prefix_recovery(&g);
    assert_eq!(context.observation.view.effects.len(), 4);
    assert_eq!(g.characteristics(ox).unwrap().power, Some(3.into()));
    answer(&mut g, Response::Cast { card: CardId(5) });
    assert!(matches!(
        g.advance(Budget::unlimited()),
        Advance::AwaitingDecision(Request::Modes { .. })
    ));
    let before = g.state_digest().unwrap();
    let restored = Game::restore(g.prepared.clone(), &g.snapshot().unwrap()).unwrap();
    let mut result = restored.observe(PlayerId(0), Budget::new(0));
    let observed = loop {
        result = match result {
            QueryResult::Complete(QueryValue::Observation(o)) => break o,
            QueryResult::Incomplete { work, .. } => {
                let saved = restored.save_work(&work).unwrap();
                restored.resume_work(restored.restore_work(&saved).unwrap(), Budget::new(1))
            }
            _ => panic!("modal observation recovery"),
        }
    };
    assert!(
        matches!(observed.decision,Some(PlayerRequest::Modes{candidates,..}) if candidates==[0,1])
    );
    assert_eq!(before, g.state_digest().unwrap());
}
#[test]
fn allocation_input_conversion_is_per_entry_and_recovered_scratch_is_proved() {
    let (mut g, _) = allocation();
    g.refresh_visibility().unwrap();
    let QueryResult::Complete(QueryValue::Decision(Some(PlayerRequest::DamageAllocation {
        token,
        candidates,
        ..
    }))) = g.player_decision_for(PlayerId(0), Budget::unlimited())
    else {
        panic!()
    };
    let response = PlayerResponse::DamageAllocation {
        allocations: vec![
            (candidates[0].damage.clone(), 1.into()),
            (candidates[1].damage.clone(), 2.into()),
        ],
    };
    let before = g.state_digest().unwrap();
    let mut result = g.check_player_response(PlayerId(0), token, response, Budget::new(0));
    let mut units = 0;
    loop {
        result = match result {
            QueryResult::Complete(QueryValue::Validated(_)) => break,
            QueryResult::Incomplete { work, .. } => {
                let saved = g.save_work(&work).unwrap();
                let value: serde_json::Value = serde_json::from_str(&saved).unwrap();
                if units == 2 {
                    assert_eq!(
                        value["data"]["task"]["converted_allocations"]
                            .as_array()
                            .unwrap()
                            .len(),
                        1
                    );
                    let mut forged = value.clone();
                    forged["data"]["task"]["converted_allocations"][0][1] = "999".into();
                    assert!(g.restore_work(&canonical(&forged).unwrap()).is_err());
                }
                let restored = g.restore_work(&saved).unwrap();
                units += 1;
                g.resume_work(restored, Budget::new(1))
            }
            _ => panic!("allocation conversion recovery"),
        };
    }
    assert_eq!(units, 5);
    assert_eq!(g.state_digest().unwrap(), before);
}
