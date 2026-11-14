//! Paid complete-source composition; no pool, policy or consumer admission.
//! Frozen d705222b280a71cb; CR20260925 109.5, 302.6, 303.4, 400.7,
//! 506.4, 608.2b, 608.3c, 613.1b/f/g, 613.7e, 704.5g:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

const SPIRIT: &str =
    include_str!("../../../../tests/fixtures/15b618f0-281f-4745-8504-bd70e7b4a267.json");
const EMBRACE: &str =
    include_str!("../../../../tests/fixtures/d5cb0370-a6f0-40c0-8b8f-553fca23778d.json");
const SOURCES: [&str; 2] = [SPIRIT, EMBRACE];
const WITNESSES: [(&str, KeywordAbility); 2] = [
    ("Spirit Away", KeywordAbility::Flying),
    ("Yavimaya's Embrace", KeywordAbility::Trample),
];

fn stats(g: &Game, id: u32, power: i64, toughness: i64, keywords: &[KeywordAbility]) {
    let creature = object(g, id);
    let c = g.characteristics(creature).unwrap();
    assert_eq!(c.power, Some(power.into()));
    assert_eq!(c.toughness, Some(toughness.into()));
    for keyword in [KeywordAbility::Flying, KeywordAbility::Trample] {
        assert_eq!(
            g.has_keyword(creature, keyword),
            keywords.contains(&keyword)
        );
    }
}

fn public_stats(g: &Game, aura: u32, creature: u32, controller: u8) {
    visible(g, aura, creature, controller);
    let before = g.state.clone();
    let trace = g.trace.clone();
    for seat in 0..4 {
        let QueryResult::Complete(QueryValue::Observation(view)) =
            g.observe(PlayerId(seat), Budget::unlimited())
        else {
            panic!("public characteristics")
        };
        let handle = g.handle_for(PlayerId(seat), object(g, creature)).unwrap();
        let row = view
            .view
            .objects
            .iter()
            .find(|o| o.handle == handle)
            .unwrap();
        assert_eq!(
            row.characteristics,
            Some(g.characteristics(object(g, creature)).unwrap())
        );
        for keyword in [KeywordAbility::Flying, KeywordAbility::Trample] {
            assert_eq!(
                row.keywords.contains(&keyword),
                g.has_keyword(object(g, creature), keyword)
            );
        }
    }
    assert_eq!(g.state, before);
    assert_eq!(g.trace, trace);
}

// Pay the actual frozen mana cost; the shared helper chooses exact mixed-color
// spending through SpendMana. Prefund G so its generic land walk cannot starve it.
fn announce_modified(g: &mut Game, q: u64, aura: u32, creature: u32) {
    let green = g.definition(CardId(aura)).unwrap().name == "Yavimaya's Embrace";
    let tapped = g.state.rules.objects.values().filter(|o| o.tapped).count();
    if green {
        fund(g, q, "Forest", 1);
    }
    announce(g, q, aura, target(g, creature));
    let paid = g.state.rules.stack.last().unwrap().paid.as_ref().unwrap();
    assert_eq!(paid.generic, 5.into());
    assert_eq!(
        paid.colored,
        if green {
            vec![Color::G, Color::U, Color::U]
        } else {
            vec![Color::U, Color::U]
        }
    );
    assert_eq!(
        g.state.rules.objects.values().filter(|o| o.tapped).count() - tapped,
        if green { 8 } else { 7 }
    );
}

#[test]
fn paid_composed_aura_control_atomic_entry_union_and_same_controller_combat() {
    let mut rows = vec![
        ("Spirit Away", 0, 0, true),
        ("Yavimaya's Embrace", 0, 0, true),
        ("Grizzly Bears", 2, 1, false),
        ("Bonesplitter", 0, 0, false),
        ("Battlegrowth", 0, 0, true),
        ("Giant Growth", 0, 0, true),
        ("Naturalize", 0, 0, true),
        ("Naturalize", 0, 0, true),
    ];
    lands(&mut rows, "Island", 0, 16);
    lands(&mut rows, "Forest", 0, 6);
    both(prepared_with_sources(&rows, &SOURCES), |g, q| {
        let creature = object(g, 2);
        announce_modified(g, q, 0, 2);
        let stack = g.state.rules.stack.last().unwrap().clone();
        recover(g);
        let mut entered = false;
        resolve(g, q, |g| {
            if g.current(object(g, 0)).unwrap().zone == Zone::Battlefield {
                entered = true;
                control(g, 2, 0, 2, 1);
                stats(g, 2, 4, 4, &[KeywordAbility::Flying]);
                public_stats(g, 0, 2, 0);
            }
        });
        assert!(entered);
        assert!(
            g.state.rules.effects.is_empty(),
            "catalog-backed static grants need no resolved effect rows"
        );
        if g.trace.mode == TraceMode::Detailed {
            let entries: Vec<_> = batches(g).into_iter().filter(|b| matches!(b.cause, BatchCause::SpellCompletion { stack_id, .. } if stack_id == stack.id)).collect();
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].occurrences.len(), 1);
            assert!(
                matches!(&entries[0].occurrences[0].actual, EventOutcome::EnteredAttached { movement, target, attachment_timestamp: 1 } if movement.previous == stack.source && movement.successor == object(g, 0) && *target == creature)
            );
        }
        let turn = g.turn().number;
        own_main(g, q, 0, turn);
        assert!(g.current(creature).unwrap().attack_ready);
        equip(g, q, 3, 2);
        fund(g, q, "Forest", 1);
        cast(g, q, 4, target(g, 2));
        fund(g, q, "Forest", 1);
        cast(g, q, 5, target(g, 2));
        let before = g.current(creature).unwrap().clone();
        let effects = g.state.rules.effects.clone();
        announce_modified(g, q, 1, 2);
        let second_stack = g.state.rules.stack.last().unwrap().clone();
        resolve(g, q, |g| {
            if g.current(object(g, 1)).unwrap().zone == Zone::Battlefield {
                assert_eq!(g.current(creature).unwrap(), &before);
                stats(
                    g,
                    2,
                    12,
                    10,
                    &[KeywordAbility::Flying, KeywordAbility::Trample],
                );
                public_stats(g, 1, 2, 0);
            }
        });
        if g.trace.mode == TraceMode::Detailed {
            let entries: Vec<_> = batches(g).into_iter().filter(|b| matches!(b.cause, BatchCause::SpellCompletion { stack_id, .. } if stack_id == second_stack.id)).collect();
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].occurrences.len(), 1);
            assert!(
                matches!(&entries[0].occurrences[0].actual, EventOutcome::EnteredAttached { movement, target, attachment_timestamp: 2 } if movement.previous == second_stack.source && movement.successor == object(g, 1) && *target == creature)
            );
        }
        assert_eq!(g.state.rules.effects, effects);
        assert_eq!(g.current(creature).unwrap().plus_one_counters, 1.into());
        let mut attacked = false;
        for _ in 0..100 {
            let r = request(g, q);
            if let Request::Attackers { candidates, .. } = &r {
                assert!(candidates.contains(&creature));
                answer(
                    g,
                    q,
                    Response::Attackers {
                        attacks: vec![(creature, PlayerId(1))],
                    },
                );
                attacked = true;
                break;
            }
            answer(g, q, pass(&r));
        }
        assert!(attacked);
        until_player(g, q, PlayerId(0));
        let before = g.current(creature).unwrap().clone();
        fund(g, q, "Forest", 1);
        cast(g, q, 6, target(g, 1));
        assert_eq!(g.current(creature).unwrap(), &before);
        assert!(g.state.rules.combat.iter().any(|a| a.attacker == creature));
        stats(g, 2, 10, 8, &[KeywordAbility::Flying]);
        assert_eq!(g.state.rules.effects, effects);
        recover(g);
        fund(g, q, "Forest", 1);
        cast(g, q, 7, target(g, 0));
        control(g, 2, 1, 2, 1);
        stats(g, 2, 8, 6, &[]);
        assert!(!g.current(creature).unwrap().attack_ready);
        assert!(g.current(creature).unwrap().tapped);
        assert!(!g.state.rules.combat.iter().any(|a| a.attacker == creature));
        assert_eq!(g.current(object(g, 3)).unwrap().attached_to, Some(creature));
        assert_eq!(g.state.rules.effects, effects);
    });
}

#[test]
fn paid_composed_aura_control_opposing_chronology_removal_and_source_forgery() {
    for (name, keyword) in WITNESSES {
        for modified_first in [true, false] {
            let (first, second) = if modified_first { (0, 1) } else { (1, 0) };
            let mut rows = vec![
                (name, 0, 0, true),
                ("Control Magic", 1, 1, true),
                ("Grizzly Bears", 3, 2, false),
                ("Disperse", 1, 1, true),
                ("Disperse", 1, 1, true),
                ("Grizzly Bears", 3, 2, false),
            ];
            rows[first as usize].1 = 0;
            rows[first as usize].2 = 0;
            rows[second as usize].1 = 1;
            rows[second as usize].2 = 1;
            lands(&mut rows, "Island", 0, 10);
            lands(&mut rows, "Forest", 0, 1);
            lands(&mut rows, "Island", 1, 12);
            lands(&mut rows, "Forest", 1, 1);
            both(prepared_with_sources(&rows, &SOURCES), |g, q| {
                if first == 0 {
                    announce_modified(g, q, first, 2);
                    resolve(g, q, |_| {});
                } else {
                    cast(g, q, first, target(g, 2));
                }
                control(g, 2, 0, 3, 2);
                let turn = g.turn().number;
                own_main(g, q, 1, turn);
                if second == 0 {
                    announce_modified(g, q, second, 2);
                    resolve(g, q, |_| {});
                } else {
                    cast(g, q, second, target(g, 2));
                }
                control(g, 2, 1, 3, 2);
                stats(g, 2, 4, 4, &[keyword]);
                public_stats(g, 0, 2, 1);
                assert!(g.state.rules.effects.is_empty());
                recover(g);
                let wire = g.snapshot().unwrap();
                for variant in 0..3 {
                    let mut bad: Snapshot = strict_read(&wire).unwrap();
                    if variant == 0 {
                        bad.state
                            .rules
                            .objects
                            .get_mut(&CardId(first))
                            .unwrap()
                            .attachment_timestamp = Some(2);
                        bad.state
                            .rules
                            .objects
                            .get_mut(&CardId(second))
                            .unwrap()
                            .attachment_timestamp = Some(1);
                        bad.state
                            .rules
                            .objects
                            .get_mut(&CardId(2))
                            .unwrap()
                            .controller = PlayerId(0);
                    } else if variant == 1 {
                        // A shape-valid forged modifier source redirects the actual
                        // attached Aura; modifiers themselves remain immutable catalog data.
                        let decoy = object(g, 5);
                        bad.state
                            .rules
                            .objects
                            .get_mut(&CardId(0))
                            .unwrap()
                            .attached_to = Some(decoy);
                        bad.state
                            .rules
                            .objects
                            .get_mut(&CardId(2))
                            .unwrap()
                            .controller = PlayerId(if modified_first { 1 } else { 0 });
                        let d = bad.state.rules.objects.get_mut(&CardId(5)).unwrap();
                        d.controller = PlayerId(if modified_first { 0 } else { 1 });
                        d.attack_ready = false;
                    } else {
                        // Current Aura control masks this invented default;
                        // only replay proves the underlying control authority.
                        bad.state
                            .rules
                            .objects
                            .get_mut(&CardId(2))
                            .unwrap()
                            .default_controller = PlayerId(0);
                    }
                    let mut structural = g.fork();
                    structural.state = bad.state.clone();
                    structural.validate_state().unwrap();
                    if variant == 1 {
                        stats(&structural, 2, 2, 2, &[]);
                        stats(&structural, 5, 4, 4, &[keyword]);
                    }
                    bad.state_sha256 = digest(&bad.state).unwrap();
                    assert!(
                        Game::restore(g.prepared.as_ref().clone(), &canonical(&bad).unwrap())
                            .is_err(),
                        "normalized historical forgery {variant}"
                    );
                }
                // Removing the older modified source leaves newer plain control;
                // removing older plain control leaves the modified source intact.
                let mut older_removed = g.fork();
                cast(&mut older_removed, q, 3, target(g, first));
                control(&older_removed, 2, 1, 3, 2);
                if modified_first {
                    stats(&older_removed, 2, 2, 2, &[]);
                } else {
                    stats(&older_removed, 2, 4, 4, &[keyword]);
                }
                recover(&older_removed);
                cast(g, q, 3, target(g, second));
                control(g, 2, 0, 3, 2);
                if modified_first {
                    stats(g, 2, 4, 4, &[keyword]);
                } else {
                    stats(g, 2, 2, 2, &[]);
                }
                cast(g, q, 4, target(g, first));
                control(g, 2, 2, 3, 2);
                stats(g, 2, 2, 2, &[]);
                assert_eq!(g.state.rules.next_attachment_timestamp, 3);
            });
        }
    }
}

#[test]
fn paid_composed_aura_control_bounce_reentry_invalid_targets_and_atomic_inputs() {
    for (name, keyword) in WITNESSES {
        let mut rows = vec![
            (name, 0, 0, true),
            ("Grizzly Bears", 0, 1, false),
            ("Disperse", 0, 0, true),
            ("Unsummon", 0, 0, true),
            ("Unsummon", 1, 1, true),
        ];
        lands(&mut rows, "Island", 0, 20);
        lands(&mut rows, "Forest", 0, 4);
        lands(&mut rows, "Island", 1, 1);
        both(prepared_with_sources(&rows, &SOURCES), |g, q| {
            let creature = object(g, 1);
            let r = request(g, q);
            let before = g.state.clone();
            let trace = g.trace.clone();
            assert!(
                g.respond(
                    PlayerId(1),
                    r.decision_id(),
                    Response::Cast { card: CardId(0) }
                )
                .is_err()
            );
            assert_eq!(g.state, before);
            assert_eq!(g.trace, trace);
            let mut proposal = g.fork();
            answer(&mut proposal, q, Response::Cast { card: CardId(0) });
            let r = request(&mut proposal, q);
            assert!(matches!(r, Request::Targets { .. }));
            let before = proposal.state.clone();
            let trace = proposal.trace.clone();
            let land = object(g, 5);
            for objects in [
                vec![],
                vec![land],
                vec![creature, creature],
                vec![ObjectId {
                    incarnation: creature.incarnation + 1,
                    ..creature
                }],
            ] {
                assert!(
                    proposal
                        .respond(r.player(), r.decision_id(), Response::Targets { objects })
                        .is_err()
                );
                assert_eq!(proposal.state, before);
                assert_eq!(proposal.trace, trace);
            }
            announce_modified(g, q, 0, 1);
            resolve(g, q, |_| {});
            let aura = object(g, 0);
            stats(g, 1, 4, 4, &[keyword]);
            cast(g, q, 2, target(g, 0));
            control(g, 1, 1, 0, 1);
            stats(g, 1, 2, 2, &[]);
            assert!(g.current(aura).is_err());
            assert_eq!(
                g.current(object(g, 0)).unwrap().zone,
                Zone::Hand { owner: PlayerId(0) }
            );
            assert_eq!(g.current(object(g, 0)).unwrap().attachment_timestamp, None);
            recover(g);

            // The target becomes illegal after accepted payment. Resolution may
            // not create a transient grant, attach to its hand successor, or enter.
            let mut invalid = g.fork();
            announce_modified(&mut invalid, q, 0, 1);
            let stack = invalid.state.rules.stack.last().unwrap().id;
            let event_count = invalid.events().len();
            until_player(&mut invalid, q, PlayerId(1));
            cast(&mut invalid, q, 4, target(g, 1));
            assert!(invalid.current(creature).is_err());
            assert_eq!(
                invalid.current(object(&invalid, 0)).unwrap().zone,
                Zone::Graveyard { owner: PlayerId(0) }
            );
            assert_eq!(
                invalid.current(object(&invalid, 1)).unwrap().zone,
                Zone::Hand { owner: PlayerId(0) }
            );
            assert_eq!(invalid.state.rules.next_attachment_timestamp, 2);
            if invalid.trace.mode == TraceMode::Detailed {
                assert!(invalid.events().iter().any(|e| matches!(e, Event::SpellCompleted { stack_id, resolved: false } if *stack_id == stack)));
                assert!(!invalid.events().iter().skip(event_count).any(
                    |e| matches!(e, Event::ControlChanged { object, .. } if *object == creature)
                ));
            }
            recover(&invalid);

            announce_modified(g, q, 0, 1);
            resolve(g, q, |_| {});
            let second_aura = object(g, 0);
            assert_ne!(second_aura, aura);
            assert_eq!(
                g.current(second_aura).unwrap().attachment_timestamp,
                Some(2)
            );
            stats(g, 1, 4, 4, &[keyword]);
            announce(g, q, 3, target(g, 1));
            let mut paused = false;
            resolve(g, q, |g| {
                if !paused
                    && g.current(creature).is_err()
                    && g.current(second_aura)
                        .is_ok_and(|o| o.zone == Zone::Battlefield)
                {
                    paused = true;
                    recover(g);
                    assert_eq!(g.current(second_aura).unwrap().attached_to, Some(creature));
                    assert_ne!(
                        g.current(second_aura).unwrap().attached_to,
                        Some(object(g, 1))
                    );
                }
            });
            if q == 1 {
                assert!(paused);
            }
            assert_eq!(
                g.current(object(g, 0)).unwrap().zone,
                Zone::Graveyard { owner: PlayerId(0) }
            );
            assert_eq!(
                g.current(object(g, 1)).unwrap().zone,
                Zone::Hand { owner: PlayerId(0) }
            );
            fund(g, q, "Forest", 1);
            cast(g, q, 1, vec![]);
            assert_ne!(object(g, 1), creature);
            control(g, 1, 0, 0, 0);
            stats(g, 1, 2, 2, &[]);
            assert_eq!(g.current(object(g, 0)).unwrap().attached_to, None);
            assert_eq!(g.state.rules.next_attachment_timestamp, 3);
            assert!(g.state.rules.effects.is_empty());
        });
    }
}

#[test]
fn paid_composed_aura_control_toughness_loss_restores_older_control_before_lethal_sba() {
    // Exact existing frozen sources: Lightning Bolt deals 3; Naturalize destroys
    // the Aura. A 2/2 survives as 4/4, then dies at the next SBA after losing +2/+2.
    for (name, keyword) in WITNESSES {
        let mut rows = vec![
            ("Mind Control", 0, 0, true),
            (name, 1, 1, true),
            ("Grizzly Bears", 3, 2, false),
            ("Lightning Bolt", 1, 1, true),
            ("Naturalize", 1, 1, true),
        ];
        lands(&mut rows, "Island", 0, 5);
        lands(&mut rows, "Island", 1, 9);
        lands(&mut rows, "Forest", 1, 3);
        lands(&mut rows, "Mountain", 1, 1);
        both(prepared_with_sources(&rows, &SOURCES), |g, q| {
            cast(g, q, 0, target(g, 2));
            let turn = g.turn().number;
            own_main(g, q, 1, turn);
            announce_modified(g, q, 1, 2);
            resolve(g, q, |_| {});
            control(g, 2, 1, 3, 2);
            stats(g, 2, 4, 4, &[keyword]);
            let creature = object(g, 2);
            fund(g, q, "Mountain", 1);
            cast(g, q, 3, target(g, 2));
            assert_eq!(g.current(creature).unwrap().marked_damage, 3.into());
            assert_eq!(g.current(creature).unwrap().zone, Zone::Battlefield);
            recover(g);
            fund(g, q, "Forest", 1);
            announce(g, q, 4, target(g, 1));
            let mut between = false;
            resolve(g, q, |g| {
                if !between
                    && g.current(object(g, 1)).unwrap().zone != Zone::Battlefield
                    && g.current(creature).is_ok()
                {
                    between = true;
                    control(g, 2, 0, 3, 2);
                    stats(g, 2, 2, 2, &[]);
                    assert_eq!(g.current(creature).unwrap().marked_damage, 3.into());
                    recover(g);
                }
            });
            if q == 1 {
                assert!(
                    between,
                    "recoverable boundary after Aura loss, before lethal SBA"
                );
            }
            assert!(g.current(creature).is_err());
            assert_eq!(
                g.current(object(g, 2)).unwrap().zone,
                Zone::Graveyard { owner: PlayerId(3) }
            );
            assert_eq!(
                g.current(object(g, 1)).unwrap().zone,
                Zone::Graveyard { owner: PlayerId(1) }
            );
            assert_eq!(
                g.current(object(g, 0)).unwrap().zone,
                Zone::Graveyard { owner: PlayerId(0) }
            );
            if g.trace.mode == TraceMode::Detailed {
                let deaths: Vec<_> = batches(g).into_iter().filter(|b| matches!(b.cause, BatchCause::StateBased)).flat_map(|b| &b.occurrences).filter(|o| matches!(&o.actual, EventOutcome::StateBasedMoved { movement, reason: CreatureLossReason::LethalDamage } if movement.previous == creature)).collect();
                assert_eq!(deaths.len(), 1);
                let fact = deaths[0]
                    .before
                    .objects
                    .iter()
                    .find(|o| o.object == creature)
                    .unwrap();
                assert_eq!(fact.controller, PlayerId(0));
                assert_eq!(fact.owner, PlayerId(3));
                assert_eq!(fact.default_controller, PlayerId(2));
                assert_eq!(fact.marked_damage, 3.into());
                assert_eq!(fact.characteristics.power, Some(2.into()));
                assert_eq!(fact.characteristics.toughness, Some(2.into()));
                assert!(!fact.keywords.contains(&keyword));
                assert!(g.events().iter().any(|e| matches!(e, Event::ControlChanged { object, previous, controller } if *object == creature && *previous == PlayerId(1) && *controller == PlayerId(0))));
            }
        });
    }
}
