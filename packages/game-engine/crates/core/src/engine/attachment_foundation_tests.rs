//! Low-level event laws using exact attachment fixtures, without extending
//! admission or synthesizing new card semantics. Natural cast/replay witnesses
//! belong to the attachment integration suite.
use super::*;

fn game() -> Game {
    let catalog = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-attachments-ir.json"
    ))
    .unwrap();
    let rows = [
        ("Bonesplitter", 0, Zone::Battlefield),
        ("Short Sword", 0, Zone::Battlefield),
        ("Grizzly Bears", 0, Zone::Battlefield),
        ("Storm Crow", 1, Zone::Battlefield),
        ("Rancor", 0, Zone::Hand { owner: PlayerId(0) }),
        ("Unholy Strength", 0, Zone::Hand { owner: PlayerId(0) }),
        ("Forest", 0, Zone::Battlefield),
    ];
    let config = ScenarioConfig {
        initial: ScenarioInitial::default(),
        mode: Mode::CommanderMatch,
        players: 4,
        profile: "commander-attachments-v1".into(),
        commanders: vec![],
        cards: rows
            .iter()
            .enumerate()
            .map(|(i, (name, owner, zone))| CardSetup {
                id: CardId(i as u32),
                oracle_id: catalog
                    .definitions
                    .iter()
                    .find(|d| d.name == *name)
                    .unwrap()
                    .oracle_id
                    .clone(),
                owner: PlayerId(*owner),
                controller: PlayerId(*owner),
                zone: *zone,
            })
            .collect(),
    };
    Game::new(Prepared {
        catalog_digest: digest(&catalog).unwrap(),
        profile_digest: digest(&config).unwrap(),
        catalog: Arc::new(catalog),
        config,
        match_config: None,
    })
}
fn object(g: &Game, card: u32) -> ObjectId {
    g.object_id(CardId(card)).unwrap()
}
fn attach(g: &mut Game, card: u32, target: Option<ObjectId>) -> EventOutcome {
    g.commit(
        ProposedEvent::Attach {
            object: object(g, card),
            target,
        },
        BatchCause::Fixture,
    )
    .unwrap()
}
fn move_card(g: &mut Game, card: u32, destination: Zone) -> ObjectId {
    g.move_object_for(object(g, card), destination, BatchCause::Fixture)
        .unwrap()
        .successor
}
fn timestamp(g: &Game, card: u32) -> Option<u64> {
    g.state.rules.objects[&CardId(card)].attachment_timestamp
}
fn unchanged_after_rejection(g: &mut Game, group: EventGroup) {
    let before = g.fork();
    assert!(g.commit_batch(BatchCause::Fixture, group).is_err());
    assert_eq!(g.state, before.state);
    assert_eq!(g.trace, before.trace);
    assert_eq!(g.state_digest().unwrap(), before.state_digest().unwrap());
}
fn leaf(proposed: ProposedEvent) -> EventGroup {
    EventGroup::Occurrence { proposed }
}

#[test]
fn allocation_noop_detach_retarget_reattach_and_incarnation_reset() {
    let mut g = game();
    let bear = object(&g, 2);
    let crow = object(&g, 3);
    assert_eq!(g.state.rules.next_attachment_timestamp, 1);
    // Allocate the larger physical ID first: object ordering is irrelevant.
    attach(&mut g, 1, Some(bear));
    attach(&mut g, 0, Some(bear));
    assert_eq!((timestamp(&g, 1), timestamp(&g, 0)), (Some(1), Some(2)));
    attach(&mut g, 0, Some(bear));
    assert_eq!(timestamp(&g, 0), Some(2));
    assert_eq!(g.state.rules.next_attachment_timestamp, 3);
    attach(&mut g, 0, Some(crow));
    assert_eq!(timestamp(&g, 0), Some(3));
    attach(&mut g, 0, None);
    assert_eq!(timestamp(&g, 0), None);
    assert_eq!(g.state.rules.next_attachment_timestamp, 4);
    attach(&mut g, 0, Some(bear));
    assert_eq!(timestamp(&g, 0), Some(4));
    let old = object(&g, 0);
    move_card(&mut g, 0, Zone::Exile);
    assert_eq!(timestamp(&g, 0), None);
    assert_eq!(g.state.rules.objects[&CardId(0)].attached_to, None);
    let returned = move_card(&mut g, 0, Zone::Battlefield);
    assert_ne!(old, returned);
    attach(&mut g, 0, Some(bear));
    assert_eq!(timestamp(&g, 0), Some(5));
    assert_eq!(g.state.rules.next_attachment_timestamp, 6);
    g.validate_attachment_state().unwrap();
}

#[test]
fn exhaustion_rejects_attachment_and_entry_without_writes_but_allows_noop_detach() {
    let mut g = game();
    let bear = object(&g, 2);
    let crow = object(&g, 3);
    attach(&mut g, 0, Some(bear));
    let aura = move_card(&mut g, 4, Zone::Stack);
    g.state.rules.next_attachment_timestamp = u64::MAX;
    for proposal in [
        ProposedEvent::Attach {
            object: object(&g, 0),
            target: Some(crow),
        },
        ProposedEvent::Attach {
            object: object(&g, 1),
            target: Some(bear),
        },
        ProposedEvent::EnterAttached {
            object: aura,
            target: bear,
        },
    ] {
        unchanged_after_rejection(&mut g, leaf(proposal));
    }
    attach(&mut g, 0, Some(bear));
    assert_eq!(timestamp(&g, 0), Some(1));
    attach(&mut g, 0, None);
    assert_eq!(timestamp(&g, 0), None);
    assert_eq!(g.state.rules.next_attachment_timestamp, u64::MAX);
}

#[test]
fn invalid_stale_and_failed_later_children_roll_back_the_complete_batch() {
    let mut g = game();
    let equipment = object(&g, 0);
    let bear = object(&g, 2);
    let forest = object(&g, 6);
    let stale = object(&g, 3);
    move_card(&mut g, 3, Zone::Exile);
    let crow = move_card(&mut g, 3, Zone::Battlefield);
    let stale_source = object(&g, 1);
    move_card(&mut g, 1, Zone::Exile);
    for proposal in [
        ProposedEvent::Attach {
            object: equipment,
            target: Some(equipment),
        },
        ProposedEvent::Attach {
            object: equipment,
            target: Some(forest),
        },
        ProposedEvent::Attach {
            object: equipment,
            target: Some(stale),
        },
        ProposedEvent::Attach {
            object: bear,
            target: Some(crow),
        },
        ProposedEvent::Attach {
            object: stale_source,
            target: Some(bear),
        },
        ProposedEvent::EnterAttached {
            object: object(&g, 4),
            target: bear,
        },
    ] {
        unchanged_after_rejection(&mut g, leaf(proposal));
    }
    unchanged_after_rejection(
        &mut g,
        EventGroup::Ordered {
            children: vec![
                leaf(ProposedEvent::Attach {
                    object: equipment,
                    target: Some(bear),
                }),
                leaf(ProposedEvent::Attach {
                    object: equipment,
                    target: Some(stale),
                }),
            ],
        },
    );
    assert_eq!(g.state.rules.next_attachment_timestamp, 1);
}

#[test]
fn attached_entry_is_one_occurrence_with_consumed_facts_and_no_intermediate_event() {
    let mut g = game();
    let bear = object(&g, 2);
    let aura = move_card(&mut g, 4, Zone::Stack);
    let events_before = g.events().len();
    let receipt = g
        .commit_batch(
            BatchCause::Fixture,
            leaf(ProposedEvent::EnterAttached {
                object: aura,
                target: bear,
            }),
        )
        .unwrap();
    let [occurrence] = receipt.occurrences.as_slice() else {
        panic!()
    };
    assert_eq!(occurrence.before.next_attachment_timestamp, Some(1));
    assert_eq!(
        occurrence.before.attachments,
        vec![events::AttachmentBefore {
            object: aura,
            target: None,
            timestamp: None,
        }]
    );
    let EventOutcome::EnteredAttached {
        movement,
        target,
        attachment_timestamp,
    } = &occurrence.actual
    else {
        panic!()
    };
    assert_eq!(*target, bear);
    assert_eq!(*attachment_timestamp, 1);
    assert_eq!(movement.from, Zone::Stack);
    assert_eq!(movement.destination, Zone::Battlefield);
    assert_eq!(movement.previous, aura);
    assert_eq!(g.events().len(), events_before + 1);
    assert!(matches!(
        &g.events()[events_before],
        Event::Committed {
            proposed: ProposedEvent::EnterAttached { .. },
            actual: EventOutcome::EnteredAttached { .. },
        }
    ));
    let entered = g.current(movement.successor).unwrap();
    assert_eq!(entered.attached_to, Some(bear));
    assert_eq!(entered.attachment_timestamp, Some(1));
    assert_eq!(g.state.rules.next_attachment_timestamp, 2);
    assert!(g.current(aura).is_err());
    g.validate_attachment_state().unwrap();
    let unrelated = g
        .commit_batch(
            BatchCause::Fixture,
            leaf(ProposedEvent::Tap {
                object: object(&g, 6),
            }),
        )
        .unwrap();
    assert!(unrelated.before.attachments.is_empty());
    assert_eq!(unrelated.before.next_attachment_timestamp, None);
}

#[test]
fn simultaneous_allocation_and_dependent_entry_reject_but_independent_detaches_commute() {
    let mut g = game();
    let bear = object(&g, 2);
    let crow = object(&g, 3);
    let a = object(&g, 0);
    let b = object(&g, 1);
    let aura = move_card(&mut g, 4, Zone::Stack);
    for occurrences in [
        vec![
            ProposedEvent::Attach {
                object: a,
                target: Some(bear),
            },
            ProposedEvent::Attach {
                object: b,
                target: Some(crow),
            },
        ],
        vec![
            ProposedEvent::Attach {
                object: a,
                target: Some(bear),
            },
            ProposedEvent::Move {
                object: bear,
                destination: Zone::Exile,
            },
        ],
        vec![
            ProposedEvent::EnterAttached {
                object: aura,
                target: bear,
            },
            ProposedEvent::Tap { object: crow },
        ],
    ] {
        unchanged_after_rejection(&mut g, EventGroup::Simultaneous { occurrences });
    }
    attach(&mut g, 0, Some(bear));
    attach(&mut g, 1, Some(crow));
    g.commit_batch(
        BatchCause::Fixture,
        EventGroup::Simultaneous {
            occurrences: vec![
                ProposedEvent::Attach {
                    object: a,
                    target: None,
                },
                ProposedEvent::Attach {
                    object: b,
                    target: None,
                },
            ],
        },
    )
    .unwrap();
    assert_eq!((timestamp(&g, 0), timestamp(&g, 1)), (None, None));
    assert_eq!(g.state.rules.next_attachment_timestamp, 3);
    attach(&mut g, 0, Some(bear));
    attach(&mut g, 1, Some(crow));
    let moved = g
        .commit_batch(
            BatchCause::Fixture,
            EventGroup::Simultaneous {
                occurrences: vec![
                    ProposedEvent::Move {
                        object: a,
                        destination: Zone::Exile,
                    },
                    ProposedEvent::Move {
                        object: b,
                        destination: Zone::Exile,
                    },
                ],
            },
        )
        .unwrap();
    assert_eq!(moved.before.attachments.len(), 2);
    assert_eq!((timestamp(&g, 0), timestamp(&g, 1)), (None, None));
    assert_eq!(g.state.rules.next_attachment_timestamp, 5);
}

#[test]
fn stale_target_waits_for_sba_without_following_successor_and_movement_captures_timestamp() {
    let mut g = game();
    let bear = object(&g, 2);
    attach(&mut g, 0, Some(bear));
    let successor = move_card(&mut g, 2, Zone::Exile);
    assert_eq!(g.state.rules.objects[&CardId(0)].attached_to, Some(bear));
    assert_eq!(timestamp(&g, 0), Some(1));
    g.validate_attachment_state().unwrap();
    assert!(g.static_modifications(successor).is_empty());
    let mut occurrences = vec![];
    g.attachment_sbas(&mut occurrences).unwrap();
    g.commit_batch(
        BatchCause::StateBased,
        EventGroup::Simultaneous { occurrences },
    )
    .unwrap();
    assert_eq!(timestamp(&g, 0), None);
    let crow = object(&g, 3);
    attach(&mut g, 0, Some(crow));
    let equipment = object(&g, 0);
    let receipt = g
        .commit_batch(
            BatchCause::Fixture,
            leaf(ProposedEvent::Move {
                object: equipment,
                destination: Zone::Exile,
            }),
        )
        .unwrap();
    assert_eq!(
        receipt.before.attachments,
        vec![events::AttachmentBefore {
            object: equipment,
            target: Some(crow),
            timestamp: Some(2)
        }]
    );
    assert_eq!(timestamp(&g, 0), None);
    assert_eq!(g.state.rules.next_attachment_timestamp, 3);
}

#[test]
fn timestamp_validation_and_canonical_digest_bind_the_new_private_fields() {
    let mut g = game();
    let initial = canonical(&g.state).unwrap();
    assert!(!initial.contains("next_attachment_timestamp"));
    let decoded: State = strict_read(&initial).unwrap();
    assert_eq!(decoded.rules.next_attachment_timestamp, 1);
    assert_eq!(decoded, g.state);
    let bear = object(&g, 2);
    attach(&mut g, 0, Some(bear));
    attach(&mut g, 1, Some(bear));
    let baseline = g.state_digest().unwrap();
    assert_eq!(baseline, digest(&g.state).unwrap());
    for (timestamp, next) in [
        (None, 3),
        (Some(0), 3),
        (Some(3), 3),
        (Some(2), 3),
        (Some(1), 0),
    ] {
        let mut forged = g.fork();
        forged
            .state
            .rules
            .objects
            .get_mut(&CardId(0))
            .unwrap()
            .attachment_timestamp = timestamp;
        forged.state.rules.next_attachment_timestamp = next;
        assert!(forged.validate_attachment_state().is_err());
        assert_ne!(baseline, forged.state_digest().unwrap());
        assert_eq!(
            forged.state_digest().unwrap(),
            digest(&forged.state).unwrap()
        );
    }
    let mut future = g.fork();
    future
        .state
        .rules
        .objects
        .get_mut(&CardId(0))
        .unwrap()
        .attached_to = Some(ObjectId {
        incarnation: bear.incarnation + 1,
        ..bear
    });
    assert!(future.validate_attachment_state().is_err());
    let mut forged_counter = g.fork();
    forged_counter.state.rules.next_attachment_timestamp += 1;
    // Structurally plausible history still requires the snapshot replay proof.
    forged_counter.validate_attachment_state().unwrap();
    assert_ne!(baseline, forged_counter.state_digest().unwrap());
    assert_eq!(
        forged_counter.state_digest().unwrap(),
        digest(&forged_counter.state).unwrap()
    );
}

#[test]
fn bestowed_host_departure_restores_same_chromanticore() {
    let mut p = super::equipped_indestructible_tests::prepared(
        &[("Grizzly Bears", false), ("Grizzly Bears", false)],
        false,
    );
    let mtg_compiler::CompileResult::Lowered { definition } = mtg_compiler::compile(
        include_str!("../../../../tests/fixtures/ramos/archive-manifest-current.json"),
        include_str!("../../../../tests/fixtures/ramos/695ee4fe-356d-47fb-9442-d171b618151c.json"),
    ) else {
        panic!("frozen Chromanticore must lower")
    };
    let printed = definition.characteristics.clone();
    p.config.cards[0].oracle_id = definition.oracle_id.clone();
    let c = Arc::make_mut(&mut p.catalog);
    c.definitions.push(*definition);
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    p.catalog_digest = digest(c).unwrap();
    p.profile_digest = digest(&p.config).unwrap();
    let mut g = Game::new(p);
    let source = object(&g, 0);
    let host = object(&g, 1);
    // Seed only the established attachment. The casting lane witnesses entry.
    let permanent = g.state.rules.objects.get_mut(&source.card).unwrap();
    permanent.attached_to = Some(host);
    permanent.attachment_timestamp = Some(1);
    g.state.rules.next_attachment_timestamp = 2;
    g.validate_attachment_state().unwrap();
    let aura = g.characteristics(source).unwrap();
    assert_eq!(aura.subtypes, ["Aura"]);
    assert_eq!((aura.power, aura.toughness), (None, None));
    assert_eq!(g.characteristics(host).unwrap().power, Some(6.into()));
    move_card(&mut g, 1, Zone::Exile);
    assert_eq!(g.zone(source), Some(Zone::Battlefield));
    assert_eq!(g.current(source).unwrap().attached_to, None);
    assert_eq!(g.characteristics(source).unwrap(), printed);
    g.validate_attachment_state().unwrap();
}
