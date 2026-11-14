//! Private complete-source paid casts, not pool or consumer admission.
//! CR 303.4, 400.7, 608.2b, 608.3a, 613.7, 701.3 and 704.5m/n:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::equipped_indestructible_tests::{answer, both, equip, object, recover, request};
use super::temporary_indestructible_tests::{announce, pass, target, until_player};
use super::*;

const AURAS: &[(&str, &str, &str, usize)] = &[
    (
        "Rancor",
        include_str!("../../../../tests/fixtures/rancor.json"),
        "Forest",
        1,
    ),
    (
        "Pacifism",
        include_str!("../../../../tests/fixtures/pacifism.json"),
        "Plains",
        2,
    ),
    (
        "Unholy Strength",
        include_str!("../../../../tests/fixtures/unholy-strength.json"),
        "Swamp",
        1,
    ),
];

fn prepared(rows: &[(&str, bool)]) -> Prepared {
    let placeholders: Vec<_> = rows
        .iter()
        .map(|(name, hand)| {
            (
                if *name == "Dive Down" {
                    "Giant Growth"
                } else {
                    *name
                },
                *hand,
            )
        })
        .collect();
    let mut p = equipped_indestructible_tests::prepared(&placeholders, false);
    if rows.iter().any(|(name, _)| *name == "Dive Down") {
        let mtg_compiler::CompileResult::Lowered { definition } = mtg_compiler::compile(
            include_str!("../../../../tests/fixtures/archive-manifest.json"),
            include_str!("../../../../tests/fixtures/2f990b54-fbf3-4949-85bb-9ba39710e72a.json"),
        ) else {
            panic!("complete frozen Dive Down source")
        };
        let c = Arc::make_mut(&mut p.catalog);
        for (i, (name, _)) in rows.iter().enumerate() {
            if *name == "Dive Down" {
                p.config.cards[i].oracle_id = definition.oracle_id.clone();
            }
        }
        c.definitions.push(*definition);
        c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
        c.validate().unwrap();
        p.catalog_digest = digest(c).unwrap();
        p.profile_digest = digest(&p.config).unwrap();
    }
    // Compare whole lowered definitions, including Rancor's graveyard trigger;
    // neither a hand-written Aura fragment nor a name-based runtime override.
    for (name, raw, _, _) in AURAS {
        let mtg_compiler::CompileResult::Lowered { definition } = mtg_compiler::compile(
            include_str!("../../../../tests/fixtures/archive-manifest.json"),
            raw,
        ) else {
            panic!("complete frozen Aura source {name}")
        };
        assert_eq!(
            definition.as_ref(),
            p.catalog
                .definitions
                .iter()
                .find(|d| d.name == *name)
                .unwrap()
        );
    }
    p
}

fn batches(g: &Game) -> Vec<&CommittedBatch> {
    g.trace
        .records
        .iter()
        .filter_map(|r| match &r.record {
            EvidenceRecord::Batch { batch } => Some(batch.as_ref()),
            _ => None,
        })
        .collect()
}

fn resolve_inspected(g: &mut Game, q: u64, mut inspect: impl FnMut(&Game)) {
    for _ in 0..2000 {
        let a = g.advance(Budget::new(q));
        inspect(g);
        match a {
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(r) => {
                if matches!(r, Request::Priority { .. })
                    && g.state.rules.stack.is_empty()
                    && g.state.rules.pending_triggers.is_empty()
                {
                    return;
                }
                let response = match &r {
                    Request::OrderTriggers { candidates, .. } => Response::OrderTriggers {
                        order: candidates.clone(),
                    },
                    _ => pass(&r),
                };
                g.respond(r.player(), r.decision_id(), response).unwrap();
            }
            other => panic!("paid resolution {other:?}"),
        }
    }
    panic!("bounded paid resolution")
}

fn cast(g: &mut Game, q: u64, id: u32, targets: Vec<TargetEntity>) {
    announce(g, q, id, targets);
    assert!(g.state.rules.stack.last().unwrap().paid.is_some());
    resolve_inspected(g, q, |_| {});
}

fn assert_visible_attachment(g: &Game, aura: u32, creature: u32) {
    let before = g.state.clone();
    let trace = g.trace.clone();
    for seat in 0..4 {
        let viewer = PlayerId(seat);
        let QueryResult::Complete(QueryValue::Observation(observation)) =
            g.observe(viewer, Budget::unlimited())
        else {
            panic!("post-entry public observation")
        };
        let aura_handle = g.handle_for(viewer, object(g, aura)).unwrap();
        let target_handle = g.handle_for(viewer, object(g, creature)).unwrap();
        let visible_aura = observation
            .view
            .objects
            .iter()
            .find(|o| o.handle == aura_handle)
            .unwrap();
        let visible_target = observation
            .view
            .objects
            .iter()
            .find(|o| o.handle == target_handle)
            .unwrap();
        assert_eq!(visible_aura.attached_to, Some(target_handle));
        assert!(visible_target.attachments.contains(&aura_handle));
        assert_eq!(visible_aura.controller, PlayerId(0));
        assert_eq!(visible_target.controller, PlayerId(1));
        assert_eq!(visible_target.owner, PlayerId(2));
    }
    assert_eq!(g.state, before);
    assert_eq!(g.trace, trace);
}

#[test]
fn paid_aura_entry_is_one_typed_occurrence_before_observations() {
    for (name, _, land, cost) in AURAS {
        let mut rows = vec![(*name, true), ("Grizzly Bears", false)];
        rows.extend(vec![(*land, false); *cost]);
        let mut p = prepared(&rows);
        // Initial ownership/control shape only; no dynamic control effect.
        p.config.cards[1].owner = PlayerId(2);
        p.config.cards[1].controller = PlayerId(1);
        p.profile_digest = digest(&p.config).unwrap();
        both(p, |g, q| {
            assert_eq!(g.state.rules.next_attachment_timestamp, 1);
            let creature = object(g, 1);
            announce(g, q, 0, target(g, 1));
            let stack = g.state.rules.stack.last().unwrap().clone();
            assert!(stack.paid.is_some());
            assert_eq!(g.current(stack.source).unwrap().zone, Zone::Stack);
            recover(g);
            let mut seen_entry = false;
            resolve_inspected(g, q, |g| {
                let aura = g.current(object(g, 0)).unwrap();
                if aura.zone == Zone::Battlefield {
                    seen_entry = true;
                    assert_eq!(aura.attached_to, Some(creature));
                    assert_eq!(aura.attachment_timestamp, Some(1));
                    assert_visible_attachment(g, 0, 1);
                }
            });
            assert!(seen_entry);
            assert_eq!(g.state.rules.next_attachment_timestamp, 2);
            assert_eq!(
                g.state.rules.objects.values().filter(|o| o.tapped).count(),
                *cost
            );
            match *name {
                "Rancor" => {
                    assert_eq!(g.characteristics(creature).unwrap().power, Some(4.into()));
                    assert!(g.has_keyword(creature, KeywordAbility::Trample));
                }
                "Pacifism" => {
                    assert!(g.has_restriction(creature, CreatureRestriction::CantAttackOrBlock))
                }
                "Unholy Strength" => {
                    let c = g.characteristics(creature).unwrap();
                    assert_eq!((c.power, c.toughness), (Some(4.into()), Some(3.into())));
                }
                _ => unreachable!(),
            }
            if g.trace.mode == TraceMode::Detailed {
                let entries: Vec<_> = batches(g)
                    .into_iter()
                    .filter(|b| {
                        matches!(b.cause, BatchCause::SpellCompletion { stack_id, .. }
                        if stack_id == stack.id)
                    })
                    .collect();
                assert_eq!(entries.len(), 1, "entry must not be Move then Attach");
                let batch = entries[0];
                assert_eq!(batch.occurrences.len(), 1);
                let occurrence = &batch.occurrences[0];
                assert!(
                    matches!(occurrence.proposed, ProposedEvent::EnterAttached { object, target }
                    if object == stack.source && target == creature)
                );
                let EventOutcome::EnteredAttached {
                    movement,
                    target,
                    attachment_timestamp,
                } = &occurrence.actual
                else {
                    panic!("movement outcome must already carry the attachment")
                };
                assert_eq!(movement.previous, stack.source);
                assert_eq!(movement.successor, object(g, 0));
                assert_eq!(movement.from, Zone::Stack);
                assert_eq!(movement.destination, Zone::Battlefield);
                assert_eq!(*target, creature);
                assert_eq!(*attachment_timestamp, 1);
                assert_eq!(occurrence.before.next_attachment_timestamp, Some(1));
                let source_attachment = occurrence
                    .before
                    .attachments
                    .iter()
                    .find(|a| a.object == stack.source)
                    .unwrap();
                assert_eq!(source_attachment.target, None);
                assert_eq!(source_attachment.timestamp, None);
                // Consumed source-before facts are intentionally pre-entry;
                // observer state above is the completed attached permanent.
                assert_eq!(
                    occurrence
                        .before
                        .objects
                        .iter()
                        .find(|o| o.object == stack.source)
                        .unwrap()
                        .zone,
                    Zone::Stack
                );
                let before_target = occurrence
                    .before
                    .objects
                    .iter()
                    .find(|o| o.object == creature)
                    .unwrap();
                assert_eq!(before_target.characteristics.power, Some(2.into()));
                assert!(!before_target.keywords.contains(&KeywordAbility::Trample));
                let entry_record = g
                    .trace
                    .records
                    .iter()
                    .position(|r| {
                        matches!(&r.record, EvidenceRecord::Batch { batch: b }
                        if b.batch_id == batch.batch_id)
                    })
                    .unwrap();
                let completed = g.trace.records.iter().position(|r|
                    matches!(&r.record, EvidenceRecord::Event { event }
                        if matches!(event.as_ref(), Event::SpellCompleted { stack_id, resolved: true }
                            if *stack_id == stack.id))).unwrap();
                assert!(entry_record < completed);
                assert!(!batches(g).iter().flat_map(|b| &b.occurrences).any(
                    |o| matches!(o.proposed, ProposedEvent::Attach { object, .. }
                        if object.card == CardId(0))
                ));
            }
        });
    }
}

#[test]
fn paid_attachment_timestamps_follow_chronology_and_same_target_equip_is_unchanged() {
    let mut rows = vec![
        ("Unholy Strength", true),
        ("Unholy Strength", true),
        ("Grizzly Bears", false),
        ("Greenwood Sentinel", false),
        ("Bonesplitter", false),
        ("Murder", true),
    ];
    rows.extend(vec![("Swamp", false); 10]);
    both(prepared(&rows), |g, q| {
        cast(g, q, 1, target(g, 2)); // Higher physical ID attaches first.
        cast(g, q, 0, target(g, 2));
        assert_eq!(
            g.current(object(g, 1)).unwrap().attachment_timestamp,
            Some(1)
        );
        assert_eq!(
            g.current(object(g, 0)).unwrap().attachment_timestamp,
            Some(2)
        );
        equip(g, q, 4, 2);
        assert_eq!(
            g.current(object(g, 4)).unwrap().attachment_timestamp,
            Some(3)
        );
        let next = g.state.rules.next_attachment_timestamp;
        let batch_count = batches(g).len();
        equip(g, q, 4, 2); // Paid ability resolves without a new attach occurrence.
        assert_eq!(
            g.current(object(g, 4)).unwrap().attachment_timestamp,
            Some(3)
        );
        assert_eq!(g.state.rules.next_attachment_timestamp, next);
        assert!(
            !batches(g)
                .iter()
                .skip(batch_count)
                .flat_map(|b| &b.occurrences)
                .any(|o| matches!(o.actual, EventOutcome::AttachmentChanged { .. }))
        );
        equip(g, q, 4, 3);
        assert_eq!(
            g.current(object(g, 4)).unwrap().attachment_timestamp,
            Some(4)
        );
        let old_target = object(g, 3);
        cast(g, q, 5, target(g, 3));
        assert!(g.current(old_target).is_err());
        let equipment = g.current(object(g, 4)).unwrap();
        assert_eq!(
            (equipment.attached_to, equipment.attachment_timestamp),
            (None, None)
        );
        assert_eq!(g.state.rules.next_attachment_timestamp, 5);
        equip(g, q, 4, 2);
        assert_eq!(
            g.current(object(g, 4)).unwrap().attachment_timestamp,
            Some(5)
        );
        assert_eq!(g.state.rules.next_attachment_timestamp, 6);
    });
}

#[test]
fn paid_aura_invalid_stale_and_hexproof_targets_never_enter_battlefield() {
    for interruption in ["Unsummon", "Dive Down"] {
        let mut p = prepared(&[
            ("Unholy Strength", true),
            ("Grizzly Bears", false),
            ("Swamp", false),
            ("Island", false),
            (interruption, true),
        ]);
        for i in [1, 3, 4] {
            p.config.cards[i].owner = PlayerId(1);
            p.config.cards[i].controller = PlayerId(1);
        }
        p.config.cards[4].zone = Zone::Hand { owner: PlayerId(1) };
        p.profile_digest = digest(&p.config).unwrap();
        both(p, |g, q| {
            let creature = object(g, 1);
            let mut proposal = g.fork();
            answer(&mut proposal, q, Response::Cast { card: CardId(0) });
            let r = request(&mut proposal, q);
            assert!(matches!(r, Request::Targets { .. }));
            let before = proposal.state.clone();
            let trace = proposal.trace.clone();
            for invalid in [
                vec![],
                vec![object(g, 2)],
                vec![ObjectId {
                    incarnation: creature.incarnation + 1,
                    ..creature
                }],
            ] {
                assert!(
                    proposal
                        .respond(
                            r.player(),
                            r.decision_id(),
                            Response::Targets { objects: invalid }
                        )
                        .is_err()
                );
                assert_eq!(proposal.state, before);
                assert_eq!(proposal.trace, trace);
            }
            announce(g, q, 0, target(g, 1));
            let aura_stack = g.state.rules.stack.last().unwrap().clone();
            assert!(aura_stack.paid.is_some());
            until_player(g, q, PlayerId(1));
            announce(g, q, 4, target(g, 1));
            assert!(g.state.rules.stack.last().unwrap().paid.is_some());
            resolve_inspected(g, q, |g| {
                let aura = g.current(object(g, 0)).unwrap();
                assert_ne!(aura.zone, Zone::Battlefield);
                assert_eq!((aura.attached_to, aura.attachment_timestamp), (None, None));
            });
            assert_eq!(
                g.current(object(g, 0)).unwrap().zone,
                Zone::Graveyard { owner: PlayerId(0) }
            );
            assert_eq!(g.state.rules.next_attachment_timestamp, 1);
            if interruption == "Unsummon" {
                assert!(g.current(creature).is_err());
            } else {
                assert_eq!(object(g, 1), creature);
                assert!(g.has_keyword(creature, KeywordAbility::Hexproof));
                assert!(!g.targetable_by(creature, PlayerId(0)));
            }
            if g.trace.mode == TraceMode::Detailed {
                assert!(g.events().iter().any(|e| matches!(e,
                    Event::SpellCompleted { stack_id, resolved: false }
                        if *stack_id == aura_stack.id)));
                assert!(
                    !batches(g)
                        .iter()
                        .flat_map(|b| &b.occurrences)
                        .any(|o| matches!(
                            o.actual,
                            EventOutcome::EnteredAttached { .. }
                                | EventOutcome::AttachmentChanged { .. }
                        ))
                );
                assert!(!batches(g).iter().flat_map(|b| &b.occurrences).any(
                    |o| matches!(&o.actual, EventOutcome::Moved { movement }
                        if movement.previous.card == CardId(0)
                            && movement.destination == Zone::Battlefield)
                ));
            }
        });
    }
}

#[test]
fn paid_rancor_departures_clear_timestamps_and_reentry_never_follows_successor() {
    let mut rows = vec![
        ("Rancor", true),
        ("Grizzly Bears", false),
        ("Naturalize", true),
        ("Unsummon", true),
        ("Island", false),
    ];
    rows.extend(vec![("Forest", false); 8]);
    both(prepared(&rows), |g, q| {
        // Keep Island untapped: explicitly fund each green spell as needed.
        answer(
            g,
            q,
            Response::ActivateMana {
                object: object(g, 5),
            },
        );
        cast(g, q, 0, target(g, 1));
        let first_aura = object(g, 0);
        assert_eq!(g.current(first_aura).unwrap().attachment_timestamp, Some(1));
        for id in [6, 7] {
            answer(
                g,
                q,
                Response::ActivateMana {
                    object: object(g, id),
                },
            );
        }
        cast(g, q, 2, target(g, 0));
        assert!(g.current(first_aura).is_err());
        let returned = g.current(object(g, 0)).unwrap();
        assert_eq!(returned.zone, Zone::Hand { owner: PlayerId(0) });
        assert_eq!(
            (returned.attached_to, returned.attachment_timestamp),
            (None, None)
        );
        assert_eq!(g.state.rules.next_attachment_timestamp, 2);
        // No attachments remain, but the consumed allocation history is still
        // semantic and must be proven rather than reset to its sparse default.
        let mut erased: Snapshot = strict_read(&g.snapshot().unwrap()).unwrap();
        erased.state.rules.next_attachment_timestamp = 1;
        erased.state_sha256 = digest(&erased.state).unwrap();
        assert!(Game::restore(g.prepared.as_ref().clone(), &canonical(&erased).unwrap()).is_err());
        answer(
            g,
            q,
            Response::ActivateMana {
                object: object(g, 8),
            },
        );
        cast(g, q, 0, target(g, 1));
        let second_aura = object(g, 0);
        let old_creature = object(g, 1);
        assert_eq!(
            g.current(second_aura).unwrap().attachment_timestamp,
            Some(2)
        );
        answer(
            g,
            q,
            Response::ActivateMana {
                object: object(g, 4),
            },
        );
        announce(g, q, 3, target(g, 1));
        let mut paused_after_departure = false;
        resolve_inspected(g, q, |g| {
            if g.current(old_creature).is_err()
                && g.current(second_aura)
                    .is_ok_and(|o| o.zone == Zone::Battlefield)
            {
                paused_after_departure = true;
                // Reachable stale target until ordinary SBA, not a snapshot
                // forgery. The timestamp clears only with actual detachment.
                let aura = g.current(second_aura).unwrap();
                assert_eq!(aura.attached_to, Some(old_creature));
                assert_eq!(aura.attachment_timestamp, Some(2));
                assert_ne!(aura.attached_to, Some(object(g, 1)));
                recover(g);
            }
        });
        if q == 1 {
            assert!(paused_after_departure);
        }
        let returned = g.current(object(g, 0)).unwrap();
        assert_eq!(returned.zone, Zone::Hand { owner: PlayerId(0) });
        assert_eq!(
            (returned.attached_to, returned.attachment_timestamp),
            (None, None)
        );
        assert_eq!(g.state.rules.next_attachment_timestamp, 3);
        for id in [9, 10] {
            answer(
                g,
                q,
                Response::ActivateMana {
                    object: object(g, id),
                },
            );
        }
        cast(g, q, 1, vec![]);
        assert_ne!(object(g, 1), old_creature);
        assert_eq!(
            g.characteristics(object(g, 1)).unwrap().power,
            Some(2.into())
        );
        assert_eq!(g.current(object(g, 0)).unwrap().attached_to, None);
        answer(
            g,
            q,
            Response::ActivateMana {
                object: object(g, 11),
            },
        );
        cast(g, q, 0, target(g, 1));
        assert_eq!(
            g.current(object(g, 0)).unwrap().attached_to,
            Some(object(g, 1))
        );
        assert_eq!(
            g.current(object(g, 0)).unwrap().attachment_timestamp,
            Some(3)
        );
        assert_eq!(g.state.rules.next_attachment_timestamp, 4);
    });
}

#[test]
fn paid_attachment_recovery_rejects_normalized_timestamp_and_controller_shape_forgeries() {
    let mut rows = vec![
        ("Unholy Strength", true),
        ("Unholy Strength", true),
        ("Grizzly Bears", false),
        ("Greenwood Sentinel", false),
    ];
    rows.extend(vec![("Swamp", false); 2]);
    let mut p = prepared(&rows);
    // Initial shape, never a dynamic controller mutation acceptance claim.
    p.config.cards[2].owner = PlayerId(2);
    p.config.cards[2].controller = PlayerId(1);
    p.profile_digest = digest(&p.config).unwrap();
    both(p, |g, q| {
        cast(g, q, 1, target(g, 2));
        recover(g);
        cast(g, q, 0, target(g, 2));
        recover(g);
        let wire = g.snapshot().unwrap();
        for variant in 0..7 {
            let mut bad: Snapshot = strict_read(&wire).unwrap();
            match variant {
                0 => {
                    bad.state
                        .rules
                        .objects
                        .get_mut(&CardId(0))
                        .unwrap()
                        .attachment_timestamp = None
                }
                1 => {
                    bad.state
                        .rules
                        .objects
                        .get_mut(&CardId(0))
                        .unwrap()
                        .attachment_timestamp = Some(0)
                }
                2 => {
                    bad.state
                        .rules
                        .objects
                        .get_mut(&CardId(0))
                        .unwrap()
                        .attachment_timestamp = Some(1);
                    bad.state
                        .rules
                        .objects
                        .get_mut(&CardId(1))
                        .unwrap()
                        .attachment_timestamp = Some(2);
                }
                3 => bad.state.rules.next_attachment_timestamp += 7,
                4 => {
                    let creature = bad.state.rules.objects.get_mut(&CardId(2)).unwrap();
                    creature.controller = PlayerId(3);
                    creature.default_controller = PlayerId(3);
                }
                5 => {
                    bad.state
                        .rules
                        .objects
                        .get_mut(&CardId(0))
                        .unwrap()
                        .attached_to = Some(object(g, 3))
                }
                _ => {
                    let aura = bad.state.rules.objects.get_mut(&CardId(0)).unwrap();
                    aura.attached_to = None;
                    aura.attachment_timestamp = None;
                }
            }
            if variant >= 2 {
                let mut structural = g.fork();
                structural.state = bad.state.clone();
                structural.validate_state().unwrap();
                assert_ne!(
                    structural.state_digest().unwrap(),
                    g.state_digest().unwrap()
                );
            }
            bad.state_sha256 = digest(&bad.state).unwrap();
            assert!(
                Game::restore(g.prepared.as_ref().clone(), &canonical(&bad).unwrap()).is_err(),
                "normalized attachment forgery {variant}"
            );
        }
    });
}
