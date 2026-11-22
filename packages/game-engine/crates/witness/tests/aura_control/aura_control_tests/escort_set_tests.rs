//! Paid private source scenarios; no catalog/pool/host admission.
//! Escort record SHA256 0828c19386b8bdbe4f5fa9187e6ccd3ecc4812734cc05875ff9240b439a9af41;
//! source/AST/binding identities are asserted by compiler::escort_set_tests.
//! Pinned CR 113.7a, 400.7, 602.2, 608.2h, 611.2c, 701.21, 702.12,
//! 704.5g/h, 800.4a; rules SHA256
//! 8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_witness::temporary::through_cleanup;
const ESCORT: &str =
    include_str!("../../../../../tests/fixtures/ramos/c0516c2c-d7c9-4763-9180-980f60205a28.json");

fn prepared(rows: &[Row]) -> Prepared {
    let p = prepared_with_sources(rows, &[ESCORT]);
    let d = p
        .catalog
        .definitions
        .iter()
        .find(|d| d.name == "Dauntless Escort")
        .unwrap();
    assert_eq!(
        d.provenance.source_sha256,
        "637bbbc9abd468b8f2cbdd1846cc891608745c0c35f364947be49836bb841713"
    );
    assert!(sacrifice_source_creature_set_keyword_ability(&d.activated[0]).is_some());
    p
}
fn zone(g: &Game, id: u32, expected: Zone) {
    assert_eq!(g.current(object(g, id)).unwrap().zone, expected);
}
fn protected(g: &Game, id: u32, expected: bool) {
    assert_eq!(
        g.has_keyword(object(g, id), KeywordAbility::Indestructible),
        expected,
        "card {id}"
    );
}
fn effect(g: &Game) -> (&ResolvedEffect, &[ObjectId]) {
    assert_eq!(g.state.rules.effects.len(), 1);
    let e = &g.state.rules.effects[0];
    let ResolvedEffectKind::GrantKeywordsSet {
        targets,
        modifier: None,
        keywords,
    } = &e.kind
    else {
        panic!("one pure set effect")
    };
    assert_eq!(keywords, &[KeywordAbility::Indestructible]);
    assert_eq!(e.origin.source_block, 0);
    assert_eq!(e.origin.cursor, 0);
    (e, targets)
}
fn activate(g: &mut Game, q: u64, id: u32) -> ObjectId {
    let source = object(g, id);
    let player = g.current(source).unwrap().controller;
    until_player(g, q, player);
    let old = request(g, q);
    answer(
        g,
        q,
        Response::ActivateAbility {
            object: source,
            ability: 0,
        },
    );
    // No mana/target/choose-sacrifice window for the sole source cost. The
    // actual source has already moved, with its incarnation frozen on stack.
    assert!(matches!(request(g, q), Request::Priority { .. }));
    let top = g.state.rules.stack.last().unwrap();
    assert_eq!(top.source, source);
    assert_eq!(top.controller, player);
    assert!(matches!(top.kind, StackKind::Activated { ability: 0, .. }));
    assert!(top.paid.is_some());
    let facts = top.source_facts.as_ref().unwrap();
    assert_eq!(facts.object, source);
    assert_eq!(facts.controller, player);
    assert_eq!(facts.zone, Zone::Battlefield);
    assert!(top.targets.is_empty() && top.entity_targets.is_empty());
    if g.trace.mode == TraceMode::Detailed {
        assert!(g.events().iter().any(|event| matches!(event,
            Event::AbilityActivated { source: actual, player: activator, ability: 0, .. }
                if *actual == source && *activator == player)));
        assert!(batches(g).iter().flat_map(|b| &b.occurrences).any(|o| matches!(o.proposed,
            ProposedEvent::Sacrifice { object, player: payer } if object == source && payer == player)));
    }
    assert_ne!(source, object(g, id));
    let owner = g.state.rules.cards[&CardId(id)].owner;
    zone(g, id, Zone::Graveyard { owner });
    assert!(g.state.rules.effects.is_empty());
    let before = g.snapshot().unwrap();
    assert!(
        g.respond(old.player(), old.decision_id(), Response::Pass)
            .is_err()
    );
    assert_eq!(g.snapshot().unwrap(), before);
    recover(g);
    source
}
fn queries(g: &Game) {
    let before = g.state.clone();
    let trace = g.trace.clone();
    let (e, captured) = effect(g);
    for viewer in 0..4 {
        let viewer = PlayerId(viewer);
        if !g.is_live(viewer) {
            continue;
        }
        let QueryValue::Observation(o) = finish_query(g, g.observe(viewer, Budget::new(0))) else {
            panic!("observation")
        };
        assert!(
            !o.view
                .objects
                .iter()
                .any(|o| matches!(o.zone, Zone::Library { .. }))
        );
        let visible = g.visible_effects(viewer).unwrap();
        let VisibleEffect::GrantKeywordsSet {
            origin,
            targets,
            keywords,
            modifier,
            ..
        } = visible.last().unwrap()
        else {
            panic!("set projection")
        };
        assert_eq!(origin.controller, e.origin.controller);
        assert_eq!(
            origin.source.announced,
            e.origin.source_handles[viewer.0 as usize]
        );
        assert!(origin.source.current.is_none());
        assert_eq!(keywords, &[KeywordAbility::Indestructible]);
        assert!(modifier.is_none());
        assert_eq!(targets.len(), captured.len());
        for (i, t) in targets.iter().enumerate() {
            assert_eq!(t.announced, e.target_handles[i * 4 + viewer.0 as usize]);
            assert_eq!(t.current, g.handle_for(viewer, captured[i]).ok());
        }
    }
    assert_eq!(g.state, before);
    assert_eq!(g.trace, trace);
}
fn cast_green(g: &mut Game, q: u64, id: u32, targets: Vec<TargetEntity>, green: usize) {
    let owner = g.state.rules.cards[&CardId(id)].owner;
    until_player(g, q, owner);
    fund(g, q, "Forest", green);
    cast(g, q, id, targets);
}

#[test]
fn escort_set_paid_source_capture_resolution_incarnations_removal_cleanup_replay_and_tiny_budget() {
    let mut rows = vec![
        ("Dauntless Escort", 0, 0, true),   // 0: cast for its printed 1GW
        ("Prodigal Sorcerer", 2, 0, false), // 1: control, not ownership
        ("Grizzly Bears", 0, 1, false),     // 2: opposing controller, not buffed
        ("Raise the Alarm", 0, 0, true),    // 3: resolves above Escort
        ("Grizzly Bears", 0, 0, true),      // 4: enters after resolution
        ("Disperse", 0, 0, true),           // 5: leave/reenter incarnation
        ("Lightning Bolt", 0, 0, true),     // 6: actual lethal marks survive
        ("Day of Judgment", 0, 0, true),    // 7: actual destroy
        ("Grizzly Bears", 0, 0, false),     // 8: captured, then leave/reenter
    ];
    lands(&mut rows, "Forest", 0, 6);
    lands(&mut rows, "Plains", 0, 8);
    lands(&mut rows, "Island", 0, 2);
    lands(&mut rows, "Mountain", 0, 1);
    both(prepared(&rows), |g, q| {
        fund(g, q, "Forest", 1);
        fund(g, q, "Plains", 2);
        cast(g, q, 0, vec![]);
        let source = activate(g, q, 0);
        // Raise resolves first via real paid response. Its two new creatures
        // are absent at activation and present for the resolution-time capture.
        fund(g, q, "Plains", 2);
        announce(g, q, 3, vec![]);
        for _ in 0..500 {
            let r = request(g, q);
            if g.state.rules.stack.len() == 1 {
                break;
            }
            answer(g, q, pass(&r));
        }
        assert_eq!(g.state.rules.stack.len(), 1);
        assert!(g.state.rules.effects.is_empty());
        let expected: Vec<_> = g
            .state
            .rules
            .objects
            .values()
            .filter(|o| {
                o.zone == Zone::Battlefield
                    && o.controller == PlayerId(0)
                    && g.is_battlefield_creature(o.id)
            })
            .map(|o| o.id)
            .collect();
        assert_eq!(expected.len(), 4);
        resolve(g, q, |_| {});
        assert_eq!(effect(g).1, expected);
        assert_eq!(effect(g).0.origin.source, source);
        assert_eq!(effect(g).0.origin.controller, PlayerId(0));
        protected(g, 1, true);
        protected(g, 2, false);
        queries(g);
        recover(g);
        // Synthetic normalized snapshot forgery: local shape/handles alone
        // cannot authenticate the historical resolution-time capture.
        let good = g.snapshot().unwrap();
        let mut bad: Snapshot = strict_read(&good).unwrap();
        let mut forged = expected.clone();
        forged[0] = object(g, 2);
        forged.sort();
        let mut handles = Vec::new();
        for id in &forged {
            for viewer in 0..4 {
                handles.push(g.stored_handle_for(PlayerId(viewer), *id).unwrap());
            }
        }
        let e = &mut bad.state.rules.effects[0];
        let ResolvedEffectKind::GrantKeywordsSet { targets, .. } = &mut e.kind else {
            unreachable!()
        };
        *targets = forged;
        e.target_handles = handles;
        let mut structural = g.fork();
        structural.state = bad.state.clone();
        structural.validate_effect_state().unwrap();
        bad.state_sha256 = digest(&bad.state).unwrap();
        assert!(Game::restore(g.prepared.as_ref().clone(), &canonical(&bad).unwrap()).is_err());
        assert_eq!(g.snapshot().unwrap(), good);
        cast_green(g, q, 4, vec![], 2);
        protected(g, 4, false);
        // Recipient leaves and returns by ordinary paid Disperse + creature cast.
        let old = object(g, 8);
        assert!(effect(g).1.contains(&old));
        fund(g, q, "Island", 2);
        cast(g, q, 5, target(g, 8));
        queries(g);
        recover(g);
        cast_green(g, q, 8, vec![], 2);
        assert_ne!(object(g, 8), old);
        assert!(effect(g).1.contains(&old));
        assert!(!effect(g).1.contains(&object(g, 8)));
        protected(g, 8, false);
        // A creature protected solely by this activation deals real damage.
        // Restore at every tiny-step damage frame exercises granted-keyword LKI.
        answer(
            g,
            q,
            Response::ActivateAbility {
                object: object(g, 1),
                ability: 0,
            },
        );
        answer(
            g,
            q,
            Response::EntityTargets {
                entities: target(g, 2),
            },
        );
        let mut damage_frames = 0;
        for _ in 0..2000 {
            let a = g.advance(Budget::new(1));
            if let Frame::DamageEffects { transaction }
            | Frame::DamageAllocation { transaction, .. } = &g.state.rules.frame
            {
                assert!(
                    transaction.entries[0]
                        .source
                        .keywords
                        .contains(&KeywordAbility::Indestructible)
                );
                damage_frames += 1;
                recover(g);
            }
            match a {
                Advance::Suspended { .. } => (),
                Advance::AwaitingDecision(r) => {
                    if g.state.rules.stack.is_empty() {
                        break;
                    }
                    g.respond(r.player(), r.decision_id(), pass(&r)).unwrap();
                }
                other => panic!("bounded protected damage: {other:?}"),
            }
        }
        assert!(g.state.rules.stack.is_empty());
        assert!(damage_frames > 0);
        assert_eq!(g.current(object(g, 2)).unwrap().marked_damage, 1.into());
        fund(g, q, "Mountain", 1);
        cast(g, q, 6, target(g, 1));
        assert_eq!(g.current(object(g, 1)).unwrap().marked_damage, 3.into());
        zone(g, 1, Zone::Battlefield);
        fund(g, q, "Plains", 4);
        cast(g, q, 7, vec![]);
        zone(g, 1, Zone::Battlefield);
        zone(g, 2, Zone::Graveyard { owner: PlayerId(0) });
        zone(g, 4, Zone::Graveyard { owner: PlayerId(0) });
        queries(g);
        recover(g);
        through_cleanup(g, q);
        zone(g, 1, Zone::Battlefield);
        assert_eq!(g.current(object(g, 1)).unwrap().marked_damage, 0.into());
        protected(g, 1, false);
        assert!(g.state.rules.effects.is_empty());
    });
}

#[test]
fn escort_set_paid_borrowed_source_frozen_controller_and_departure() {
    let mut rows = vec![
        ("Dauntless Escort", 1, 1, false),
        ("Control Magic", 0, 0, true),
        ("Grizzly Bears", 2, 0, false),
        ("Grizzly Bears", 1, 1, false),
    ];
    lands(&mut rows, "Island", 0, 4);
    lands(&mut rows, "Island", 1, 4);
    both(prepared(&rows), |g, q| {
        fund(g, q, "Island", 4);
        cast(g, q, 1, target(g, 0));
        control(g, 0, 0, 1, 1);
        let source = activate(g, q, 0);
        assert!(g.state.rules.effects.is_empty());
        // Its owner leaves after payment. The paid invocation belongs to its
        // activator and survives with frozen LKI and controller (CR 800.4a).
        g.concede(PlayerId(1)).unwrap();
        recover(g);
        resolve(g, q, |_| {});
        assert_eq!(effect(g).0.origin.source, source);
        assert_eq!(effect(g).0.origin.controller, PlayerId(0));
        assert_eq!(effect(g).1, &[object(g, 2)]);
        protected(g, 2, true);
        queries(g);
        recover(g);
        through_cleanup(g, q);
        protected(g, 2, false);
    });
}

#[test]
fn escort_set_paid_recipient_control_change_does_not_reselect_capture() {
    let mut rows = vec![
        ("Dauntless Escort", 0, 0, false),
        ("Grizzly Bears", 2, 0, false),
        ("Grizzly Bears", 1, 1, false),
        ("Control Magic", 1, 1, true),
    ];
    lands(&mut rows, "Island", 1, 4);
    both(prepared(&rows), |g, q| {
        // Activate on the Aura caster's natural main phase, so its later
        // sorcery-speed control change occurs before this turn's cleanup.
        own_main(g, q, 1, 0);
        activate(g, q, 0);
        resolve(g, q, |_| {});
        let captured = effect(g).1.to_vec();
        assert_eq!(captured, [object(g, 1)]);
        until_player(g, q, PlayerId(1));
        fund(g, q, "Island", 4);
        cast(g, q, 3, target(g, 1));
        control(g, 1, 1, 2, 0);
        protected(g, 1, true);
        protected(g, 2, false);
        assert_eq!(effect(g).1, captured);
        queries(g);
        recover(g);
        // The Aura controller leaves: its control authority ends, while the
        // captured grant still belongs to the surviving activation controller.
        g.concede(PlayerId(1)).unwrap();
        assert!(!matches!(g.advance(Budget::new(1)), Advance::Fault(_)));
        assert!(!g.is_live(PlayerId(1)));
        control(g, 1, 0, 2, 0);
        protected(g, 1, true);
        queries(g);
        recover(g);
        through_cleanup(g, q);
        protected(g, 1, false);
    });
}
