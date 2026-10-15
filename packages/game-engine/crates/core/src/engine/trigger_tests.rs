use super::*;

fn catalog() -> Catalog {
    Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-trigger-ir.json"
    ))
    .unwrap()
}
fn game() -> Game {
    Game::new(Prepared::admit(catalog(), ScenarioConfig::triggers()).unwrap())
}
fn movement(g: &Game, card: u32, destination: Zone) -> ProposedEvent {
    ProposedEvent::Move {
        object: g.object_id(CardId(card)).unwrap(),
        destination,
    }
}
fn leaf(proposed: ProposedEvent) -> EventGroup {
    EventGroup::Occurrence { proposed }
}
fn grave(g: &Game, card: u32) -> ProposedEvent {
    movement(
        g,
        card,
        Zone::Graveyard {
            owner: g.state.cards[&CardId(card)].owner,
        },
    )
}
fn settle(g: &mut Game) -> Request {
    for _ in 0..1000 {
        match g.advance(Budget::new(1)) {
            Advance::AwaitingDecision(r) => return r,
            Advance::Suspended { .. } => (),
            other => panic!("unexpected boundary {other:?}"),
        }
    }
    panic!("decision budget exhausted")
}
fn answer(g: &mut Game, response: Response) {
    let r = settle(g);
    g.respond(r.player(), r.decision_id(), response).unwrap();
}
fn visionary_placement() -> Game {
    let mut g = game();
    answer(&mut g, Response::Cast { card: CardId(4) });
    for card in [8, 9] {
        let object = g.object_id(CardId(card)).unwrap();
        answer(&mut g, Response::ActivateMana { object });
    }
    answer(&mut g, Response::FinishMana { undo_mana: false });
    answer(
        &mut g,
        Response::SpendMana {
            spending: [(Color::G, 2.into())].into(),
        },
    );
    for _ in 0..4 {
        answer(&mut g, Response::Pass);
    }
    assert!(matches!(settle(&mut g), Request::OrderTriggers { .. }));
    assert_eq!(g.state.pending_triggers.len(), 4);
    g
}
fn recover(g: &Game) {
    let restored = Game::restore(g.prepared.clone(), &g.snapshot().unwrap()).unwrap();
    let replayed = Game::replay(g.prepared.clone(), &g.replay_journal()).unwrap();
    assert_eq!(restored.state, g.state);
    assert_eq!(replayed.state, g.state);
    assert_eq!(restored.trace(), g.trace());
    assert_eq!(replayed.trace(), g.trace());
}

#[test]
fn simultaneous_deaths_share_lookback_observers_including_both_dying_artists() {
    let mut g = game();
    let facts = [1, 16].map(|card| g.object_before(g.object_id(CardId(card)).unwrap()).unwrap());
    let events = [1, 16].map(|card| grave(&g, card)).to_vec();
    let receipt = g
        .commit_batch(
            BatchCause::Fixture,
            EventGroup::Simultaneous {
                occurrences: events,
            },
        )
        .unwrap();
    assert_eq!(g.state.pending_triggers.len(), 10);
    assert_eq!(g.state.next_trigger_id, 10);
    for fact in facts {
        let captured: Vec<_> = g
            .state
            .pending_triggers
            .iter()
            .filter(|t| t.source == fact.object)
            .collect();
        assert_eq!(captured.len(), 2);
        assert!(
            captured
                .iter()
                .all(|t| t.source_facts == fact && t.controller == fact.controller)
        );
    }
    for occurrence in receipt.occurrences {
        let EventOutcome::Moved { movement } = &occurrence.actual else {
            panic!("missing death movement")
        };
        assert!(
            g.events()
                .iter()
                .filter_map(|event| match event {
                    Event::TriggerCollected {
                        occurrence: id,
                        subject,
                        ..
                    } if *id == occurrence.id => Some(subject),
                    _ => None,
                })
                .all(|subject| *subject == movement.previous)
        );
        assert_eq!(g.events().iter().filter(|event| matches!(event, Event::TriggerCollected { occurrence: id, .. } if *id == occurrence.id)).count(), 5);
    }
    g.validate_trigger_state().unwrap();
    // Ordered deaths use their respective before-event views: the first
    // Artist is no longer present to observe the second death (CR 603.10).
    let mut ordered = game();
    let children = [1, 16].map(|card| leaf(grave(&ordered, card))).to_vec();
    ordered
        .commit_batch(BatchCause::Fixture, EventGroup::Ordered { children })
        .unwrap();
    assert_eq!(ordered.state.pending_triggers.len(), 9);
}

#[test]
fn enters_use_resulting_battlefield_and_other_excludes_exact_source() {
    let mut g = game();
    let enter = movement(&g, 4, Zone::Battlefield);
    g.commit_batch(BatchCause::Fixture, leaf(enter)).unwrap();
    let successor = g.object_id(CardId(4)).unwrap();
    assert!(
        g.events()
            .iter()
            .filter_map(|event| match event {
                Event::TriggerCollected { subject, .. } => Some(subject),
                _ => None,
            })
            .all(|subject| *subject == successor)
    );
    let sources: BTreeSet<_> = g
        .state
        .pending_triggers
        .iter()
        .map(|t| t.source.card)
        .collect();
    assert_eq!(
        sources,
        [CardId(3), CardId(4), CardId(17), CardId(29)].into()
    );
    let visionary = g
        .state
        .pending_triggers
        .iter()
        .find(|t| t.source.card == CardId(4))
        .unwrap();
    assert_eq!(visionary.source.incarnation, 1);
    assert_eq!(visionary.source_facts.zone, Zone::Battlefield);
    let mut g = game();
    let leave = movement(&g, 3, Zone::Hand { owner: PlayerId(0) });
    g.commit_batch(BatchCause::Fixture, leaf(leave)).unwrap();
    let enter = movement(&g, 3, Zone::Battlefield);
    g.commit_batch(BatchCause::Fixture, leaf(enter)).unwrap();
    assert_eq!(
        g.state
            .pending_triggers
            .iter()
            .map(|t| t.source.card)
            .collect::<BTreeSet<_>>(),
        [CardId(17), CardId(29)].into()
    );
}

#[test]
fn nondeath_moves_and_noncreatures_do_not_emit_dies_triggers() {
    for (card, destination) in [
        (1, Zone::Exile),
        (1, Zone::Hand { owner: PlayerId(0) }),
        (4, Zone::Graveyard { owner: PlayerId(0) }),
        (8, Zone::Graveyard { owner: PlayerId(0) }),
    ] {
        let mut g = game();
        let proposed = movement(&g, card, destination);
        g.commit_batch(BatchCause::Fixture, leaf(proposed)).unwrap();
        assert!(g.state.pending_triggers.is_empty());
        assert_eq!(g.state.next_trigger_id, 0);
        assert!(
            !g.events()
                .iter()
                .any(|e| matches!(e, Event::TriggerCollected { .. }))
        );
    }
}

#[test]
fn rejected_ordered_batch_and_trigger_resource_overflow_roll_back_collection() {
    let initial = game();
    let enter = movement(&initial, 4, Zone::Battlefield);
    let invalid = ProposedEvent::Move {
        object: ObjectId {
            card: CardId(999),
            incarnation: 0,
        },
        destination: Zone::Exile,
    };
    let mut g = initial.fork();
    let token = g.state_token.clone();
    assert!(
        g.commit_batch(
            BatchCause::Fixture,
            EventGroup::Ordered {
                children: vec![leaf(enter.clone()), leaf(invalid)]
            }
        )
        .is_err()
    );
    assert_eq!(g.state, initial.state);
    assert_eq!(g.trace, initial.trace);
    assert!(Arc::ptr_eq(&token, &g.state_token));
    let mut g = initial.fork();
    g.state.next_trigger_id = u64::MAX;
    let before = g.fork();
    assert!(g.commit_batch(BatchCause::Fixture, leaf(enter)).is_err());
    assert_eq!(g.state, before.state);
    assert_eq!(g.trace, before.trace);
}

#[test]
fn natural_pending_trigger_snapshots_reject_rehashed_provenance_forgeries() {
    let g = visionary_placement();
    recover(&g);
    type Mutation = fn(&mut serde_json::Value);
    let mutations: Vec<Mutation> = vec![
        |t| t["controller"] = serde_json::json!(3),
        |t| t["ability"] = serde_json::json!(99),
        |t| t["id"] = serde_json::json!(99),
        |t| t["source"]["card"] = serde_json::json!(999),
        |t| t["source_facts"]["owner"] = serde_json::json!(3),
        |t| t["source_facts"]["controller"] = serde_json::json!(3),
        |t| t["source_facts"]["default_controller"] = serde_json::json!(3),
        |t| t["source_facts"]["tapped"] = serde_json::json!(true),
        |t| t["source_facts"]["marked_damage"] = serde_json::json!("1"),
        |t| t["source_handles"] = serde_json::json!([]),
        |t| t["event"]["batch"] = serde_json::json!(999),
        |t| t["event"]["path"] = serde_json::json!([99]),
        |t| t["movement"]["previous"]["incarnation"] = serde_json::json!(99),
        |t| t["movement"]["destination"] = serde_json::json!({"zone":"exile"}),
    ];
    for mutate in mutations {
        let mut value = serde_json::to_value(&g.state).unwrap();
        mutate(&mut value["pending_triggers"][0]);
        let mut saved: Snapshot = strict_read(&g.snapshot().unwrap()).unwrap();
        saved.state = serde_json::from_value(value).unwrap();
        saved.state_sha256 = digest(&saved.state).unwrap();
        assert!(Game::restore(g.prepared.clone(), &canonical(&saved).unwrap()).is_err());
    }
}

#[test]
fn triggered_stack_targets_are_abilities_and_countering_moves_no_source_card() {
    let mut g = visionary_placement();
    loop {
        match settle(&mut g) {
            Request::OrderTriggers { candidates, .. } => {
                answer(&mut g, Response::OrderTriggers { order: candidates })
            }
            Request::Priority { .. } => break,
            other => panic!("unexpected placement decision {other:?}"),
        }
    }
    assert_eq!(g.state.stack.len(), 4);
    assert!(
        g.state
            .stack
            .iter()
            .all(|s| matches!(s.kind, StackKind::Triggered { .. }) && s.paid.is_some())
    );
    recover(&g);
    let target = g.state.stack[0].id;
    let countering = g.state.stack[3].id;
    let source = g.state.stack[0].source;
    let fact = g.current(source).unwrap().clone();
    let initial = g.fork();
    assert!(
        g.commit_batch(
            BatchCause::Fixture,
            leaf(ProposedEvent::CounterStackObject {
                target,
                countering,
                domain: TargetDomain::StackSpell
            })
        )
        .is_err()
    );
    assert_eq!(g.state, initial.state);
    let receipt = g
        .commit_batch(
            BatchCause::Fixture,
            leaf(ProposedEvent::CounterStackObject {
                target,
                countering,
                domain: TargetDomain::StackActivatedOrTriggeredAbility,
            }),
        )
        .unwrap();
    assert!(matches!(
        receipt.result(&[]),
        Some(EventOutcome::StackObjectCountered {
            spell: false,
            movement: None,
            ..
        })
    ));
    assert_eq!(g.current(source).unwrap(), &fact);
    assert_eq!(g.state.stack.len(), 3);
    assert!(!g.state.stack.iter().any(|s| s.id == target));
}

#[test]
fn trigger_admission_requires_exact_catalog_and_all_five_closed_profiles() {
    for c in [
        ScenarioConfig::triggers(),
        ScenarioConfig::triggers_low(),
        ScenarioConfig::triggers_borrowed(),
        ScenarioConfig::triggers_empty_library(),
        ScenarioConfig::triggers_nonactive(),
    ] {
        Prepared::admit(catalog(), c).unwrap();
    }
    for mutation in 0..5 {
        let mut c = catalog();
        match mutation {
            0 => c.schema_version = 7,
            1 => {
                c.definitions.pop();
            }
            2 => {
                c.definitions
                    .iter_mut()
                    .find(|d| d.name == "Blood Artist")
                    .unwrap()
                    .triggered[0]
                    .condition = TriggerCondition::SelfEntersBattlefield
            }
            3 => {
                c.definitions
                    .iter_mut()
                    .find(|d| d.name == "Essence Warden")
                    .unwrap()
                    .characteristics
                    .mana_cost
                    .generic = 1.into()
            }
            _ => {
                let mut extra = c.definitions[0].clone();
                extra.oracle_id = "extra".into();
                c.definitions.push(extra);
            }
        }
        assert!(Prepared::admit(c, ScenarioConfig::triggers()).is_err());
    }
    for mutation in 0..5 {
        let mut config = ScenarioConfig::triggers();
        match mutation {
            0 => config.cards[1].controller = PlayerId(3),
            1 => config.cards[4].zone = Zone::Battlefield,
            2 => config.commanders.push(CardId(1)),
            3 => config.cards[3].oracle_id = config.cards[1].oracle_id.clone(),
            _ => {
                let mut extra = config.cards[1].clone();
                extra.id = CardId(999);
                config.cards.push(extra);
            }
        }
        assert!(Prepared::admit(catalog(), config).is_err());
    }
}
