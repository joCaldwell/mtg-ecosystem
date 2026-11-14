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
    let p = equipped_indestructible_tests::prepared(rows, false);
    // Compare whole lowered definitions, including Rancor's graveyard trigger;
    // neither a hand-written Aura fragment nor a name-based runtime override.
    for (name, raw, _, _) in AURAS {
        let mtg_compiler::CompileResult::Lowered { definition } = mtg_compiler::compile(
            include_str!("../../../../tests/fixtures/archive-manifest-current.json"),
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
