//! CR 608.2h/113.7a: a hand-cast trigger keeps its source's last-known
//! information when the child it offers moves that source. The archived
//! Maelstrom Archangel trigger, Giant Growth and Bone Splinters are witnesses;
//! no rule names them.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::damage_controller_discard_tests::{
    answer, begin_resolution, placed, prepared, recover, settle,
};
use super::grouped_damage_trigger_tests::{FOREST, GROWTH};
use super::*;

const SPLINTERS: &str = "0c936582-d4c0-4da8-bc7e-49da5a433939";
const CROW: &str = "000d5588-5a4c-434e-988d-396632ade42c";
const ANGEL: CardId = CardId(0);
const CHILD: CardId = CardId(1);
const TARGET: CardId = CardId(2);

/// The trigger's controller owns its source, a Forest and Giant Growth; the
/// offered child is Bone Splinters and an opponent controls its target.
fn scenario(owner: u8) -> (Game, CardId, CardId) {
    let mut p = prepared(false, false);
    let mtg_compiler::CompileResult::Lowered { definition } = mtg_witness::compile(
        include_str!("../../../../tests/fixtures/ramos/archive-manifest-current.json"),
        include_str!("../../../../tests/fixtures/ramos/ac50ef98-1791-4dd4-9c1f-8cea7db7ade5.json"),
    ) else {
        panic!("complete combat hand cast source must lower")
    };
    p.config.cards[0].oracle_id = definition.oracle_id.clone();
    p.config.cards[0].owner = PlayerId(owner);
    let catalog = Arc::make_mut(&mut p.catalog);
    catalog.definitions.push(*definition);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    p.catalog_digest = digest(catalog).unwrap();
    p.config.cards[1].oracle_id = SPLINTERS.into();
    p.config.cards[2].zone = Zone::Battlefield;
    p.config.cards[2].oracle_id = CROW.into();
    let forest = CardId(p.config.cards.len() as u32);
    p.config.cards.push(CardSetup {
        id: forest,
        oracle_id: FOREST.into(),
        owner: PlayerId(0),
        controller: PlayerId(0),
        zone: Zone::Battlefield,
    });
    let growth = CardId(p.config.cards.len() as u32);
    p.config.cards.push(CardSetup {
        id: growth,
        oracle_id: GROWTH.into(),
        owner: PlayerId(0),
        controller: PlayerId(0),
        zone: Zone::Hand { owner: PlayerId(0) },
    });
    p.profile_digest = digest(&p.config).unwrap();
    (Game::new(p), forest, growth)
}

fn pass_round(g: &mut Game) {
    for _ in 0..g.live_players().len() {
        answer(g, Response::Pass);
    }
}

/// Puts the combat trigger on the stack, then changes its source's
/// characteristics before it resolves so the departure facts are new.
fn offered(owner: u8) -> (Game, ObjectId, ObjectBefore) {
    let (mut g, forest, growth) = scenario(owner);
    placed(&mut g);
    let source = g.object_id(ANGEL).unwrap();
    let at_trigger = g.top().unwrap().source_facts.clone().unwrap();
    answer(&mut g, Response::Cast { card: growth });
    assert!(matches!(settle(&mut g), Request::Targets { .. }));
    answer(
        &mut g,
        Response::Targets {
            objects: vec![source],
        },
    );
    let land = g.object_id(forest).unwrap();
    answer(&mut g, Response::ActivateMana { object: land });
    answer(&mut g, Response::FinishMana { undo_mana: false });
    assert!(matches!(settle(&mut g), Request::SpendMana { .. }));
    answer(
        &mut g,
        Response::SpendMana {
            spending: BTreeMap::from([(Color::G, Quantity::from(1))]),
        },
    );
    assert!(matches!(settle(&mut g), Request::Priority { .. }));
    assert_eq!(g.state.rules.stack.len(), 2);
    pass_round(&mut g);
    assert!(matches!(settle(&mut g), Request::Priority { .. }));
    assert_eq!(g.state.rules.stack.len(), 1);
    // The live trigger still holds its trigger-time facts; its source grew.
    assert_eq!(g.top().unwrap().source_facts.as_ref(), Some(&at_trigger));
    assert_ne!(g.object_before(source).unwrap(), at_trigger);
    let request = begin_resolution(&mut g);
    let Request::HandCast { candidates, .. } = request else {
        panic!("expected the hand cast offer, got {request:?}")
    };
    assert_eq!(candidates, [g.object_id(CHILD).unwrap()]);
    (g, source, at_trigger)
}

fn parent(g: &Game) -> &StackInstance {
    g.state
        .rules
        .stack
        .iter()
        .find(|entry| !entry.kind.is_spell())
        .expect("live hand cast trigger")
}

/// Announces the child through its target; the sacrifice cost is next.
fn announced(g: &mut Game) {
    answer(g, Response::Cast { card: CHILD });
    assert!(matches!(settle(g), Request::Targets { .. }));
    let target = g.object_id(TARGET).unwrap();
    answer(
        g,
        Response::Targets {
            objects: vec![target],
        },
    );
    assert!(matches!(settle(g), Request::SacrificeCost { .. }));
}

/// Every frame copy of the trigger equals its live stack entry.
fn assert_copies_current(g: &Game) {
    g.state.rules.frame.for_each_held(&mut |held| {
        if let frame::Held::Invocation { entry, .. } = held
            && let Some(owner) = g.state.rules.stack.iter().find(|s| s.id == entry.id)
        {
            assert!(
                entry == owner,
                "stale frame copy at {:?}",
                g.state.rules.frame
            );
        }
    });
}

#[test]
fn hand_cast_child_sacrificing_the_trigger_source_completes_restores_and_replays() {
    let (mut g, source, at_trigger) = offered(0);
    recover(&g);
    announced(&mut g);
    // Mid-payment: the announced child and its owning trigger restore and replay.
    recover(&g);
    let departing = g.object_before(source).unwrap();
    assert_ne!(departing, at_trigger);
    let r = settle(&mut g);
    g.respond(
        r.player(),
        r.decision_id(),
        Response::SacrificeCost { object: source },
    )
    .unwrap();
    // Each rule-step boundary after the payment restores and replays, including
    // the one where the paid child sits above its still-resolving owner.
    let mut owned_child = false;
    let after = loop {
        match g.advance(Budget::new(1)) {
            Advance::AwaitingDecision(r) => break r,
            Advance::Suspended { .. } => {
                assert_copies_current(&g);
                recover(&g);
                if let Frame::HandCast {
                    frame,
                    child: Some(child),
                } = &g.state.rules.frame
                {
                    owned_child = true;
                    // CR 608.2h/113.7a: the owner now resolves with its source's
                    // last-known information, in the frame and on the stack.
                    assert_eq!(frame.spell.source_facts.as_ref(), Some(&departing));
                    assert_eq!(parent(&g).source_facts.as_ref(), Some(&departing));
                    let paid = g.top().unwrap();
                    assert_eq!(paid.id, *child);
                    // CR 400.7: a later object of the same card is not this
                    // source. Facts are keyed by the exact incarnation, so the
                    // successor's reach neither the owner nor the frame.
                    let successor = g.object_id(ANGEL).unwrap();
                    assert_eq!(successor.incarnation, source.incarnation + 1);
                    let mut probe = g.fork();
                    let later = probe.object_before(successor).unwrap();
                    probe.retain_source_facts(&later);
                    assert!(probe.state == g.state);
                }
            }
            other => panic!("owning trigger at {:?}: {other:?}", g.state.rules.frame),
        }
    };
    assert!(
        owned_child,
        "the paid child must return to its owning trigger"
    );
    assert!(matches!(after, Request::Priority { .. }));
    recover(&g);
    let successor = g.object_id(ANGEL).unwrap();
    assert_eq!(
        g.current(successor).unwrap().zone,
        Zone::Graveyard { owner: PlayerId(0) }
    );
    assert_eq!(g.state.rules.stack.len(), 1);
    let child = g.top().unwrap();
    assert_eq!(child.source.card, CHILD);
    assert_eq!(child.paid.as_ref(), Some(&child.total_cost));
    pass_round(&mut g);
    assert!(matches!(settle(&mut g), Request::Priority { .. }));
    assert_eq!(
        g.current(g.object_id(TARGET).unwrap()).unwrap().zone,
        Zone::Graveyard { owner: PlayerId(1) }
    );
    recover(&g);
}

#[test]
fn hand_cast_offer_declined_or_cancelled_keeps_one_current_owner() {
    let (g, source, at_trigger) = offered(0);
    // Declining completes the trigger with its source still on the battlefield.
    let mut declined = g.fork();
    answer(&mut declined, Response::Pass);
    assert!(matches!(settle(&mut declined), Request::Priority { .. }));
    assert!(declined.state.rules.stack.is_empty());
    assert_eq!(declined.object_id(ANGEL).unwrap(), source);
    recover(&declined);
    // A cancelled proposal returns to the same offer and may be declined.
    // The target's controller leaves before the proposal is complete.
    let mut cancelled = g.fork();
    answer(&mut cancelled, Response::Cast { card: CHILD });
    assert!(matches!(settle(&mut cancelled), Request::Targets { .. }));
    let target = cancelled.object_id(TARGET).unwrap();
    answer(
        &mut cancelled,
        Response::Targets {
            objects: vec![target],
        },
    );
    cancelled.concede(PlayerId(1)).unwrap();
    let Request::HandCast { candidates, .. } = settle(&mut cancelled) else {
        panic!("the failed proposal must return to its owning hand offer")
    };
    assert_eq!(candidates, [cancelled.object_id(CHILD).unwrap()]);
    assert_copies_current(&cancelled);
    assert_eq!(parent(&cancelled).source_facts.as_ref(), Some(&at_trigger));
    recover(&cancelled);
    answer(&mut cancelled, Response::Pass);
    assert!(matches!(settle(&mut cancelled), Request::Priority { .. }));
    assert!(cancelled.state.rules.stack.is_empty());
    recover(&cancelled);
}

#[test]
fn hand_cast_source_leaving_during_the_child_announcement_cancels_with_last_known_facts() {
    // Another player owns the trigger's source. Their departure removes it
    // while the child is being announced (CR 800.4a), so the sacrifice cost can
    // no longer be paid and the proposal is cancelled.
    let (mut g, source, at_trigger) = offered(2);
    announced(&mut g);
    let departing = g.object_before(source).unwrap();
    assert_ne!(departing, at_trigger);
    g.concede(PlayerId(2)).unwrap();
    let mut returned = false;
    let after = loop {
        match g.advance(Budget::new(1)) {
            Advance::AwaitingDecision(r) => break r,
            Advance::Suspended { .. } => {
                assert_copies_current(&g);
                recover(&g);
                if let Frame::HandCast { frame, child: None } = &g.state.rules.frame {
                    returned = true;
                    // The owner resolves on with the source's last-known facts.
                    assert!(g.current(source).is_err());
                    let facts = frame.spell.source_facts.as_ref().unwrap();
                    assert_eq!(facts.object, source);
                    assert_eq!(facts.characteristics, departing.characteristics);
                    assert_ne!(*facts, at_trigger);
                    assert_eq!(parent(&g).source_facts.as_ref(), Some(facts));
                }
            }
            other => panic!("owning trigger at {:?}: {other:?}", g.state.rules.frame),
        }
    };
    assert!(
        returned,
        "the cancelled child must return to its owning trigger"
    );
    // No creature remains to sacrifice, so the offer has no candidate left.
    assert!(matches!(after, Request::Priority { .. }), "{after:?}");
    assert!(g.state.rules.stack.is_empty());
    assert_eq!(
        g.current(g.object_id(CHILD).unwrap()).unwrap().zone,
        Zone::Hand { owner: PlayerId(0) }
    );
    recover(&g);
}

#[test]
fn every_cast_frame_suspending_a_hand_cast_owner_carries_its_last_known_facts() {
    let (mut g, source, at_trigger) = offered(0);
    answer(&mut g, Response::Cast { card: CHILD });
    assert!(matches!(settle(&mut g), Request::Targets { .. }));
    let Frame::Targets { cast } = g.state.rules.frame.clone() else {
        panic!("the child is being announced")
    };
    assert!(matches!(
        cast.completion,
        CastCompletion::HandTrigger { .. }
    ));
    let departing = g.object_before(source).unwrap();
    assert_ne!(departing, at_trigger);
    let wrap = |cast: &Casting| -> Vec<Frame> {
        let cast = || cast.clone();
        vec![
            Frame::AdditionalCosts { cast: cast() },
            Frame::AnnounceX { cast: cast() },
            Frame::HybridCost { cast: cast() },
            Frame::SacrificeCost { cast: cast() },
            Frame::Modes { cast: cast() },
            Frame::EntityTargets { cast: cast() },
            Frame::StackTargets { cast: cast() },
            Frame::PlayerTargets { cast: cast() },
            Frame::SpendMana { cast: cast() },
            Frame::Targets { cast: cast() },
            Frame::Mana { cast: cast() },
            Frame::Payment { cast: cast() },
        ]
    };
    let nest = |inner: Frame| -> Vec<Frame> {
        vec![
            inner.clone(),
            Frame::Stabilize {
                resume: Box::new(inner.clone()),
                cleanup: false,
                changed: false,
            },
            Frame::LegendRule {
                resume: Box::new(inner.clone()),
                groups: vec![],
                kept: vec![],
            },
            Frame::ManaSourceChoice {
                player: PlayerId(0),
                source,
                scope: ManaSourceScope::Intrinsic,
                resume: Box::new(inner.clone()),
            },
            Frame::CommanderReturn {
                object: source,
                destination: Zone::Command { owner: PlayerId(0) },
                continuation: Box::new(CommanderContinuation::StateBased { resume: inner }),
            },
        ]
    };
    let held = |frame: &Frame| {
        let mut entries = vec![];
        frame.for_each_held(&mut |held| {
            if let frame::Held::Invocation { hold, entry } = held {
                entries.push((hold, entry.clone()));
            }
        });
        entries
    };
    let mut frames = 0;
    for cast_frame in wrap(&cast) {
        for frame in nest(cast_frame) {
            frames += 1;
            // The owner waits beneath its child: suspended, never resolving.
            let [(hold, before)] = held(&frame).try_into().unwrap();
            assert_eq!(hold, frame::Hold::Suspended);
            assert_eq!(before.source_facts.as_ref(), Some(&at_trigger));
            assert!(frame.resolving_invocation().is_none());
            // A departure reaches the copy wherever the frame holds it.
            let mut pushed = g.fork();
            pushed.state.rules.frame = frame.clone();
            pushed.retain_source_facts(&departing);
            let [(_, after)] = held(&pushed.state.rules.frame).try_into().unwrap();
            assert_eq!(after.source_facts.as_ref(), Some(&departing));
            assert!(after == *parent(&pushed));
            // A stale copy written back by a step is reconciled at commit.
            let (stale, lease) = pushed.lease_frame();
            assert!(stale == pushed.state.rules.frame);
            pushed.state.rules.frame = frame;
            pushed.commit_frame(lease);
            let [(_, committed)] = held(&pushed.state.rules.frame).try_into().unwrap();
            assert!(committed == *parent(&pushed));
        }
    }
    assert_eq!(frames, 60);
    // The same owner is resolving once its child no longer suspends it.
    let Frame::Targets { cast } = &g.state.rules.frame else {
        unreachable!()
    };
    let CastCompletion::HandTrigger { frame } = &cast.completion else {
        unreachable!()
    };
    let resolving = Frame::HandCast {
        frame: frame.clone(),
        child: None,
    };
    assert!(resolving.resolving_invocation() == Some(parent(&g)));
}
