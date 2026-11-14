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
                for variant in 0..2 {
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
                    } else {
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
