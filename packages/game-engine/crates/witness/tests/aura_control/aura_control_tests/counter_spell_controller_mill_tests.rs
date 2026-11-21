//! CR 608.2b/c/h, 701.6 and 701.17 (frozen 20260925).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

const CASTER: PlayerId = PlayerId(1);
const TARGET_CONTROLLER: PlayerId = PlayerId(0);

fn scenario() -> Prepared {
    let mut rows = vec![
        ("Counterspell", 1, 1, true),
        ("Sol Ring", 0, 0, true),
        ("Volcanic Fallout", 0, 0, true),
    ];
    lands(&mut rows, "Mountain", 0, 3);
    lands(&mut rows, "Island", 1, 3);
    if let Some(path) = std::env::var_os("MTG_COUNTER_SPELL_CONTROLLER_MILL_ARCHIVE_PROBE") {
        let archive = std::fs::read_to_string(path).expect("read frozen counter/mill probe");
        let row = archive
            .lines()
            .find(|line| {
                sha256(line.as_bytes())
                    == "41b1bd452dd2effba27795494f535bfee33a72e09f21ae93a5a19fdacfa94e39"
            })
            .expect("Didn't Say Please, frozen physical row 27840");
        rows[0].0 = "Didn't Say Please";
        return prepared_with_sources(&rows, &[row]);
    }
    let mut pod = prepared(&rows);
    let catalog = Arc::make_mut(&mut pod.catalog);
    let spell = catalog
        .definitions
        .iter_mut()
        .find(|d| d.name == "Counterspell")
        .unwrap();
    spell.name = "Synthetic counter and controller mill".into();
    spell.oracle_id = "synthetic-counter-and-controller-mill".into();
    spell.source_labels = None;
    spell.characteristics.mana_cost.generic = 1.into();
    spell.spell.as_mut().unwrap().instructions = vec![Instruction::CounterSpellAndMillController {
        target_slot: 0,
        count: 3.into(),
        source_block: 0,
    }];
    spell.capabilities = spell.requirements();
    pod.config.cards[0].oracle_id = spell.oracle_id.clone();
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    pod.catalog_digest = digest(catalog).unwrap();
    pod.profile_digest = digest(&pod.config).unwrap();
    pod
}

fn counter_mill_response(g: &mut Game, target: StackId) -> StackInstance {
    until_player(g, 1, CASTER);
    answer(g, 1, Response::Cast { card: CardId(0) });
    assert!(
        matches!(request(g, 1), Request::StackTargets { candidates, .. } if candidates == [target])
    );
    answer(
        g,
        1,
        Response::StackTargets {
            entries: vec![target],
        },
    );
    fund(g, 1, "Island", 3);
    answer(g, 1, Response::FinishMana { undo_mana: false });
    answer(
        g,
        1,
        Response::SpendMana {
            spending: BTreeMap::from([(Color::U, 3.into())]),
        },
    );
    assert!(matches!(request(g, 1), Request::Priority { .. }));
    let spell = g.top().unwrap().clone();
    assert_eq!(spell.stack_targets, [target]);
    assert_eq!(
        spell.paid,
        Some(ManaCost {
            generic: 1.into(),
            colored: vec![Color::U, Color::U]
        })
    );
    spell
}

// Isolated rule-step probes from the paid, legal resolution boundary.
fn probe_counter_mill_prefix(g: &Game, target: &StackInstance) {
    // Ordinary casts here use the owner's hand. Give the exact stack spell a
    // different controller to distinguish pre-counter information from ownership.
    let mut changed = g.fork();
    let controller = PlayerId(2);
    changed
        .state
        .rules
        .stack
        .iter_mut()
        .find(|s| s.id == target.id)
        .unwrap()
        .controller = controller;
    assert!(matches!(
        changed.advance(Budget::new(1)),
        Advance::Suspended { .. }
    ));
    let mut expected = g.state.rules.libraries.clone();
    expected.get_mut(&controller).unwrap().drain(..3);
    assert_eq!(changed.state.rules.libraries, expected);
    assert_eq!(
        changed.zone(object(&changed, target.source.card.0)),
        Some(Zone::Graveyard {
            owner: TARGET_CONTROLLER
        })
    );

    // The counter has one available batch ID. Failing the subsequent mill
    // must restore both the target and all of the first event's trace.
    let mut fault = g.fork();
    fault.state.rules.next_batch_id = u64::MAX - 1;
    assert!(
        matches!(fault.advance(Budget::new(1)), Advance::Fault(reason) if reason == "batch ID resource limit")
    );
    assert_eq!(fault.state.rules.stack, g.state.rules.stack);
    assert_eq!(fault.state.rules.objects, g.state.rules.objects);
    assert_eq!(fault.state.rules.libraries, g.state.rules.libraries);
    assert_eq!(fault.trace(), g.trace());
}

#[test]
fn counter_spell_controller_mill_paid_stack_target_and_prevented_counter() {
    let pod = scenario();
    for (card, protected) in [(1, false), (2, true)] {
        let mut g = Game::with_trace(pod.clone(), TraceMode::Detailed);
        announce(&mut g, 1, card, vec![]);
        let target = g.top().unwrap().clone();
        assert_eq!(target.controller, TARGET_CONTROLLER);
        assert_eq!(target.paid.as_ref(), Some(&target.total_cost));
        let spell = counter_mill_response(&mut g, target.id);
        let cause = BatchCause::Instruction {
            stack_id: spell.id,
            player: CASTER,
            source: spell.source,
            cursor: 0,
            source_block: 0,
        };
        let before = g.state.rules.libraries.clone();
        if !protected {
            let mut departed = g.fork();
            departed.concede(TARGET_CONTROLLER).unwrap();
            resolve(&mut departed, 1, |_| {});
            assert!(departed.events().iter().any(|event| matches!(event, Event::SpellCompleted {stack_id, resolved: false} if *stack_id == spell.id)));
            assert!(batches(&departed).iter().all(|batch| batch.cause != cause));
        }
        let mut completed = false;
        resolve(&mut g, 1, |g| {
            let Frame::Resolve { frame } = &g.state.rules.frame else {
                return;
            };
            if frame.stack_id != spell.id {
                return;
            }
            if frame.cursor == 0 {
                if !protected {
                    probe_counter_mill_prefix(g, &target);
                }
                return;
            }
            completed = true;
            assert_eq!(frame.cursor, 1);
            assert_eq!(
                g.state.rules.stack.iter().any(|e| e.id == target.id),
                protected
            );
            let mut expected = before.clone();
            let top: Vec<_> = expected
                .get_mut(&TARGET_CONTROLLER)
                .unwrap()
                .drain(..3)
                .collect();
            assert_eq!(g.state.rules.libraries, expected);
            let effects: Vec<_> = batches(g)
                .into_iter()
                .filter(|b| b.cause == cause)
                .map(|batch| batch.result(&[]).unwrap())
                .collect();
            let [counter, EventOutcome::Milled { player, movements }] = effects.as_slice() else {
                panic!("expected counter then mill from the resolving instruction: {effects:?}")
            };
            if protected {
                assert!(
                    matches!(counter, EventOutcome::CounterPrevented { target: id } if *id == target.id)
                );
            } else {
                assert!(
                    matches!(counter, EventOutcome::StackObjectCountered { target: id, source, spell: true, .. } if *id == target.id && *source == target.source)
                );
            }
            assert_eq!(*player, TARGET_CONTROLLER);
            assert_eq!(
                movements
                    .iter()
                    .map(|m| m.previous.card)
                    .collect::<Vec<_>>(),
                top
            );
            for movement in movements {
                assert_eq!(
                    g.zone(movement.successor),
                    Some(Zone::Graveyard {
                        owner: TARGET_CONTROLLER
                    })
                );
            }
            recover(g);
        });
        assert!(completed);
    }
}
