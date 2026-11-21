//! Bounded private scalar-effect witnesses; no pool/consumer admission.
//! Exact frozen sources; signed controls below are explicitly synthetic IR.
//! New native tests UNRUN here; parent owns the combined native gate.
//! CR20260925 104.3a, 302.6, 611.2c, 613.5–6, 702.15, 800.4a:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

const MUTATION: &str =
    include_str!("../../../../../tests/fixtures/058a8475-649f-433a-b258-42bc03bf40ba.json");

fn stats(g: &Game, recipient: ObjectId, power: Quantity, toughness: Quantity, lifelink: bool) {
    let c = g.characteristics(recipient).unwrap();
    assert_eq!(c.power, Some(power));
    assert_eq!(c.toughness, Some(toughness));
    assert_eq!(g.has_keyword(recipient, KeywordAbility::Lifelink), lifelink);
}

fn complete(g: &Game, recipient: ObjectId, power: Quantity, toughness: Quantity) {
    assert_eq!(g.state.rules.effects.len(), 1);
    let e = &g.state.rules.effects[0];
    assert!(matches!(&e.kind,
        ResolvedEffectKind::GrantKeywords { target, modifier: Some(m), keywords }
        if *target == recipient && m.power == power && m.toughness == toughness
            && keywords == &[KeywordAbility::Lifelink]));
    assert_eq!(e.origin.source_block, 0);
    assert_eq!(e.origin.cursor, 0);
}

// Use the SAME one-unit creation boundary in both outer budget/trace runs.
// A large advance could already finish the buff; it cannot establish the
// earliest point at which an external concession can consume effect facts.
fn first_effect(g: &mut Game, recipient: ObjectId, power: Quantity, toughness: Quantity) {
    let base = g.characteristics(recipient).unwrap();
    for _ in 0..2000 {
        let a = g.advance(Budget::new(1));
        if !g.state.rules.effects.is_empty() {
            complete(g, recipient, power.clone(), toughness.clone());
            let mut p = base.power.clone().unwrap();
            let mut t = base.toughness.clone().unwrap();
            p.add(&power);
            t.add(&toughness);
            stats(g, recipient, p, t, true);
            recover(g);
            return;
        }
        assert_eq!(g.characteristics(recipient).unwrap(), base);
        assert!(!g.has_keyword(recipient, KeywordAbility::Lifelink));
        match a {
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(r) => {
                g.respond(r.player(), r.decision_id(), pass(&r)).unwrap();
            }
            x => panic!("bounded first complete scalar effect {x:?}"),
        }
    }
    panic!("paid scalar effect actually begins");
}

fn queries(g: &Game) {
    let before = g.state.clone();
    let trace = g.trace.clone();
    for seat in 0..4 {
        if !g.is_live(PlayerId(seat)) {
            continue;
        }
        let QueryValue::Observation(view) =
            finish_query(g, g.observe(PlayerId(seat), Budget::new(0)))
        else {
            panic!("resumable observation")
        };
        for e in &g.state.rules.effects {
            let ResolvedEffectKind::GrantKeywords { target, .. } = &e.kind else {
                panic!("one scalar family")
            };
            if g.current(*target).is_err() {
                continue;
            }
            let handle = g.handle_for(PlayerId(seat), *target).unwrap();
            let row = view
                .view
                .objects
                .iter()
                .find(|o| o.handle == handle)
                .unwrap();
            assert_eq!(
                row.characteristics,
                Some(g.characteristics(*target).unwrap())
            );
            assert_eq!(
                row.keywords.contains(&KeywordAbility::Lifelink),
                g.has_keyword(*target, KeywordAbility::Lifelink)
            );
        }
        g.visible_effects(PlayerId(seat)).unwrap();
    }
    assert_eq!(g.state, before);
    assert_eq!(g.trace, trace);
}

#[test]
fn atomic_targeted_keyword_paid_borrowed_ping_owner_departure_lki_and_actual_lifelink() {
    // Baseline RED separately retains the minimal initial owner/controller setup.
    // This witness obtains control through actual paid Aura gameplay, waits for
    // continuous-control readiness, and proves the eventual rules outcome.
    for timing in ["before_creation", "first_creation", "after_resolution"] {
        let mut rows = vec![
            ("Control Magic", 0, 0, true),
            ("Prodigal Sorcerer", 2, 2, false),
            ("Yoked Ox", 1, 1, false),
            ("Grotesque Mutation", 0, 0, true),
        ];
        lands(&mut rows, "Island", 0, 4);
        lands(&mut rows, "Swamp", 0, 2);
        both(prepared_with_sources(&rows, &[MUTATION]), |g, q| {
            cast(g, q, 0, target(g, 1));
            control(g, 1, 0, 2, 2);
            let recipient = object(g, 1);
            assert!(!g.current(recipient).unwrap().attack_ready);
            let turn = g.turn().number;
            own_main(g, q, 0, turn);
            assert!(g.current(recipient).unwrap().attack_ready);
            answer(
                g,
                q,
                Response::ActivateAbility {
                    object: recipient,
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
            request(g, q);
            let ping = g.state.rules.stack.last().unwrap().clone();
            assert!(ping.paid.is_some());
            assert_eq!(ping.controller, PlayerId(0));
            assert_eq!(ping.source, recipient);
            assert!(g.current(recipient).unwrap().tapped);
            // Prefund the exact B/generic cost, avoiding any mixed-land walk.
            fund(g, q, "Swamp", 2);
            announce(g, q, 3, target(g, 1));
            assert_eq!(g.state.rules.stack.len(), 2);
            if timing == "first_creation" {
                first_effect(g, recipient, 3.into(), 1.into());
            } else if timing == "after_resolution" {
                // Stop before resolving the lower paid ping.
                for _ in 0..2000 {
                    let r = request(g, q);
                    if g.state.rules.stack.len() == 1 {
                        break;
                    }
                    answer(g, q, pass(&r));
                }
                assert_eq!(g.state.rules.stack.len(), 1);
                complete(g, recipient, 3.into(), 1.into());
            }
            queries(g);
            recover(g);
            g.concede(PlayerId(2)).unwrap();
            request(g, q);
            assert!(g.object_id(CardId(1)).is_none());
            let pending = g
                .state
                .rules
                .stack
                .iter()
                .find(|s| s.id == ping.id)
                .unwrap();
            assert_eq!(pending.controller, PlayerId(0));
            let lki = pending.source_facts.as_ref().unwrap();
            let buffed = timing != "before_creation";
            assert_eq!(lki.object, recipient);
            assert_eq!(lki.owner, PlayerId(2));
            assert_eq!(lki.controller, PlayerId(0));
            assert_eq!(
                lki.characteristics.power,
                Some((if buffed { 4 } else { 1 }).into())
            );
            assert_eq!(
                lki.characteristics.toughness,
                Some((if buffed { 2 } else { 1 }).into())
            );
            assert_eq!(lki.keywords.contains(&KeywordAbility::Lifelink), buffed);
            if g.trace.mode == TraceMode::Detailed {
                let occurrence = batches(g)
                    .into_iter()
                    .flat_map(|b| &b.occurrences)
                    .find(|o| {
                        matches!(&o.proposed,
                        ProposedEvent::LeaveGame { object, departing_owner }
                        if *object == recipient && *departing_owner == PlayerId(2))
                    })
                    .unwrap();
                let before = occurrence
                    .before
                    .objects
                    .iter()
                    .find(|o| o.object == recipient)
                    .unwrap();
                assert_eq!(
                    before, lki,
                    "typed departure and paid continuation consume the same complete facts"
                );
            }
            recover(g);
            resolve(g, q, |_| {});
            if buffed {
                complete(g, recipient, 3.into(), 1.into());
            } else {
                assert!(g.state.rules.effects.is_empty());
            }
            assert_eq!(g.current(object(g, 2)).unwrap().marked_damage, 1.into());
            assert_eq!(g.current(object(g, 2)).unwrap().zone, Zone::Battlefield);
            assert_eq!(
                g.state.rules.players[0].life,
                (if buffed { 41 } else { 40 }).into()
            );
            assert_eq!(g.state.rules.players[1].life, 40.into());
            if g.trace.mode == TraceMode::Detailed {
                let damage: Vec<_> = g
                    .events()
                    .iter()
                    .filter_map(|e| match e {
                        Event::Committed {
                            actual:
                                EventOutcome::DamageCommitted {
                                    actual, life_gains, ..
                                },
                            ..
                        } => Some((actual, life_gains)),
                        _ => None,
                    })
                    .collect();
                assert_eq!(damage.len(), 1);
                assert_eq!(damage[0].0.len(), 1);
                assert_eq!(damage[0].0[0].amount, 1.into());
                assert_eq!(damage[0].0[0].source.object, recipient);
                assert_eq!(damage[0].0[0].source.controller, PlayerId(0));
                assert_eq!(damage[0].1.len(), usize::from(buffed));
                if buffed {
                    assert_eq!(damage[0].1[0].player, PlayerId(0));
                    assert_eq!(damage[0].1[0].amount, 1.into());
                }
            }
            queries(g);
        });
    }
}

#[test]
fn atomic_targeted_keyword_caster_departure_keeps_all_layers_on_captured_current_controller() {
    for before_creation in [true, false] {
        let mut rows = vec![
            ("Control Magic", 0, 0, true),
            ("Grizzly Bears", 2, 1, false),
            ("Grotesque Mutation", 0, 0, true),
        ];
        lands(&mut rows, "Island", 0, 4);
        lands(&mut rows, "Swamp", 0, 2);
        both(prepared_with_sources(&rows, &[MUTATION]), |g, q| {
            cast(g, q, 0, target(g, 1));
            let recipient = object(g, 1);
            control(g, 1, 0, 2, 1);
            fund(g, q, "Swamp", 2);
            announce(g, q, 2, target(g, 1));
            if !before_creation {
                first_effect(g, recipient, 3.into(), 1.into());
            }
            g.concede(PlayerId(0)).unwrap();
            request(g, q);
            control(g, 1, 1, 2, 1);
            assert!(g.object_id(CardId(0)).is_none());
            assert!(g.object_id(CardId(2)).is_none());
            if before_creation {
                assert!(g.state.rules.effects.is_empty());
                stats(g, recipient, 2.into(), 2.into(), false);
            } else {
                complete(g, recipient, 3.into(), 1.into());
                stats(g, recipient, 5.into(), 3.into(), true);
            }
            queries(g);
            recover(g);
            mtg_witness::temporary::through_cleanup(g, q);
            assert!(g.state.rules.effects.is_empty());
            stats(g, recipient, 2.into(), 2.into(), false);
        });
    }
}

#[test]
fn atomic_targeted_keyword_synthetic_signed_zero_modifiers_and_normalized_forgery() {
    // Parameter controls use one exact compiled source shape but deliberately
    // replace its quantities. These are not claims about that card's Oracle text.
    for (power, toughness) in [(0, 0), (-3, 0), (1, -1)] {
        let rows = [
            ("Grotesque Mutation", 0, 0, true),
            ("Grizzly Bears", 0, 0, false),
            ("Swamp", 0, 0, false),
            ("Swamp", 0, 0, false),
        ];
        let mut p = prepared_with_sources(&rows, &[MUTATION]);
        let mut c = (*p.catalog).clone();
        let d = c
            .definitions
            .iter_mut()
            .find(|d| d.name == "Grotesque Mutation")
            .unwrap();
        let [
            Instruction::GrantKeywords {
                modifier: Some(modifier),
                ..
            },
        ] = d.spell.as_mut().unwrap().instructions.as_mut_slice()
        else {
            panic!("one committed atomic source instruction")
        };
        modifier.power = power.into();
        modifier.toughness = toughness.into();
        c.validate().unwrap();
        p.catalog_digest = digest(&c).unwrap();
        p.catalog = Arc::new(c);
        both(p, |g, q| {
            cast(g, q, 0, target(g, 1));
            let recipient = object(g, 1);
            complete(g, recipient, power.into(), toughness.into());
            stats(
                g,
                recipient,
                (2 + power).into(),
                (2 + toughness).into(),
                true,
            );
            queries(g);
            recover(g);
            let good = g.snapshot().unwrap();
            for variant in 0..3 {
                let mut bad: Snapshot = strict_read(&good).unwrap();
                let ResolvedEffectKind::GrantKeywords { modifier, .. } =
                    &mut bad.state.rules.effects[0].kind
                else {
                    unreachable!()
                };
                if variant == 0 {
                    *modifier = None;
                } else {
                    let m = modifier.as_mut().unwrap();
                    if variant == 1 {
                        m.power.add(&1.into());
                    } else {
                        m.toughness.add(&1.into());
                    }
                }
                // Recompute the ordinary snapshot checksum: it cannot authorize
                // a coherent-looking modifier inconsistent with accepted history.
                bad.state_sha256 = digest(&bad.state).unwrap();
                assert!(
                    Game::restore(g.prepared.as_ref().clone(), &canonical(&bad).unwrap()).is_err()
                );
            }
            mtg_witness::temporary::through_cleanup(g, q);
            assert!(g.state.rules.effects.is_empty());
            stats(g, recipient, 2.into(), 2.into(), false);
        });
    }
}
