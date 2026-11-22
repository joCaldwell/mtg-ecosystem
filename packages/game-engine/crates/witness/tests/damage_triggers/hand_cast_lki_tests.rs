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

/// Every invocation the frame holds has exactly one owner: its stack entry,
/// or the detached entry once it has left the stack.
fn assert_single_owner(g: &Game) {
    let rules = &g.state.rules;
    rules.frame.for_each_held(&rules.frames, &mut |held| {
        if let frame::Held::Invocation { id, .. } = held {
            let stacked = rules.stack.iter().filter(|s| s.id == id).count();
            let detached = usize::from(rules.detached.contains_key(&id));
            assert_eq!(
                stacked + detached,
                1,
                "owners of {id:?} at {:?}",
                rules.frame
            );
            assert_eq!(g.entry(id).unwrap().id, id);
        }
    });
    assert!(
        rules
            .detached
            .keys()
            .all(|id| rules.frame.holds(&rules.frames, *id))
    );
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
                assert_single_owner(&g);
                recover(&g);
                if let Frame::HandCast {
                    frame,
                    child: Some(child),
                } = &g.state.rules.frame
                {
                    owned_child = true;
                    // CR 608.2h/113.7a: the owner now resolves with its source's
                    // last-known information. The frame only names it.
                    assert_eq!(frame.stack_id, parent(&g).id);
                    assert_eq!(parent(&g).source_facts.as_ref(), Some(&departing));
                    let paid = g.top().unwrap();
                    assert_eq!(paid.id, *child);
                    // CR 400.7: a later object of the same card is not this
                    // source. Facts are keyed by the exact incarnation, so the
                    // successor's do not reach the owner.
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
    assert_single_owner(&cancelled);
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
                assert_single_owner(&g);
                recover(&g);
                if let Frame::HandCast { frame, child: None } = &g.state.rules.frame {
                    returned = true;
                    // The owner resolves on with the source's last-known facts.
                    assert!(g.current(source).is_err());
                    let facts = g
                        .entry(frame.stack_id)
                        .unwrap()
                        .source_facts
                        .as_ref()
                        .unwrap();
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
fn every_cast_frame_suspending_a_hand_cast_owner_names_its_single_owner() {
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
                resume: FrameLink(0),
                cleanup: false,
                changed: false,
            },
            Frame::LegendRule {
                resume: FrameLink(0),
                groups: vec![],
                kept: vec![],
            },
            Frame::ManaSourceChoice {
                player: PlayerId(0),
                source,
                scope: ManaSourceScope::Intrinsic,
                resume: FrameLink(0),
            },
            Frame::CommanderReturn {
                object: source,
                destination: Zone::Command { owner: PlayerId(0) },
                continuation: CommanderContinuation::StateBased {
                    resume: FrameLink(0),
                },
            },
        ]
    };
    let held = |frame: &Frame, parents: &[Frame]| {
        let mut ids = vec![];
        frame.for_each_held(parents, &mut |held| {
            if let frame::Held::Invocation { hold, id } = held {
                ids.push((hold, id));
            }
        });
        ids
    };
    let owner = parent(&g).id;
    let mut frames = 0;
    for cast_frame in wrap(&cast) {
        for frame in nest(cast_frame.clone()) {
            let parents = if frame.parent().is_some() {
                vec![cast_frame.clone()]
            } else {
                vec![]
            };
            frames += 1;
            // The owner waits beneath its child: suspended, never resolving.
            let [(hold, id)] = held(&frame, &parents).try_into().unwrap();
            assert_eq!(hold, frame::Hold::Suspended);
            assert_eq!(id, owner);
            assert!(frame.holds(&parents, owner));
            assert!(frame.resolving_invocation(&parents).is_none());
            // A departure reaches the one owner. The frame holds no facts
            // that a rule step could write back stale.
            let mut pushed = g.fork();
            pushed.state.rules.frames = parents;
            pushed.state.rules.frame = frame.clone();
            assert_eq!(parent(&pushed).source_facts.as_ref(), Some(&at_trigger));
            pushed.retain_source_facts(&departing);
            assert!(pushed.state.rules.frame == frame);
            assert_eq!(parent(&pushed).source_facts.as_ref(), Some(&departing));
            // CR 608.2m: once its entry leaves the stack, the owner is the
            // detached entry, for exactly as long as the frame resumes it.
            let index = pushed
                .state
                .rules
                .stack
                .iter()
                .position(|entry| entry.id == owner)
                .unwrap();
            pushed.unstack(index).unwrap();
            pushed.release_detached();
            assert_single_owner(&pushed);
            assert_eq!(
                pushed.entry(owner).unwrap().source_facts.as_ref(),
                Some(&departing)
            );
            pushed.state.rules.frame = Frame::Priority;
            pushed.release_detached();
            assert!(pushed.entry(owner).is_err());
            assert!(pushed.state.rules.detached.is_empty());
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
    assert!(resolving.resolving_invocation(&[]) == Some(owner));
}
