//! Frozen CR 702.2b/704.3/704.5g-h/514.2, September 25, 2026:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
//! Accepted-input combat witnesses use exact existing sources, no damage injection.
use super::equipped_indestructible_tests::{
    answer, both, cast, equip, object, prepared, recover, request,
};
use super::*;

fn setup(victim: &str, attacker: &str, fresh: bool) -> Prepared {
    let mut p = prepared(
        &[
            ("Darksteel Plate", false),
            (victim, false),
            (attacker, false),
            ("Disperse", true),
            ("Island", false),
            ("Island", false),
            ("Island", false),
            ("Island", false),
            ("Typhoid Rats", false),
        ],
        false,
    );
    p.config.profile = "deathtouch-window-private".into();
    for (index, seat) in [(2, 1), (8, 2)] {
        p.config.cards[index].owner = PlayerId(seat);
        p.config.cards[index].controller = PlayerId(seat);
    }
    if !fresh {
        // Keep the second Rats out of combat unless explicitly requested.
        p.config.cards[8].zone = Zone::Hand { owner: PlayerId(2) };
    }
    p.profile_digest = digest(&p.config).unwrap();
    p
}
fn pass_response(r: &Request) -> Response {
    match r {
        Request::Priority { .. } => Response::Pass,
        Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
        Request::Blockers { .. } => Response::Blockers { blocks: vec![] },
        x => panic!("unexpected turn request {x:?}"),
    }
}
fn combat(g: &mut Game, q: u64, attacker: u32, expected_damage: i64) {
    let seat = g.current(object(g, attacker)).unwrap().controller;
    for _ in 0..300 {
        let r = request(g, q);
        let response = match &r {
            Request::Attackers { player, .. } if *player == seat => Response::Attackers {
                attacks: vec![(object(g, attacker), PlayerId(0))],
            },
            Request::Blockers { player, .. } if *player == PlayerId(0) => Response::Blockers {
                blocks: vec![(object(g, 1), object(g, attacker))],
            },
            Request::Priority { .. }
                if g.current(object(g, 1)).unwrap().marked_damage == expected_damage.into() =>
            {
                return;
            }
            _ => pass_response(&r),
        };
        answer(g, q, response);
    }
    panic!("combat did not finish")
}
fn bounce_plate(g: &mut Game, q: u64) {
    for _ in 0..8 {
        if request(g, q).player() == PlayerId(0) {
            cast(
                g,
                q,
                3,
                vec![TargetEntity::Object {
                    object: object(g, 0),
                }],
            );
            return;
        }
        answer(g, q, Response::Pass);
    }
    panic!("P0 priority")
}
fn survive_grant_loss(g: &mut Game, q: u64) {
    let turn = g.state.rules.turn.clone();
    bounce_plate(g, q);
    let victim = g.current(object(g, 1)).unwrap();
    assert_eq!(
        victim.zone,
        Zone::Battlefield,
        "old nonlethal deathtouch must not kill after grant loss"
    );
    assert_eq!(victim.marked_damage, 1.into());
    assert!(!victim.deathtouch_damage);
    assert_eq!(
        g.state.rules.turn, turn,
        "grant loss must occur in the same turn"
    );
    assert!(!g.has_keyword(victim.id, KeywordAbility::Indestructible));
    recover(g);
    if g.trace.mode == TraceMode::Detailed {
        let mut snapshot: Snapshot = strict_read(&g.snapshot().unwrap()).unwrap();
        let index = snapshot
            .trace
            .records
            .iter()
            .position(|r| {
                matches!(&r.record,
                    EvidenceRecord::Batch { batch } if batch.cause == BatchCause::DeathtouchCheck
                )
            })
            .expect("recorded deathtouch consumption");
        let EvidenceRecord::Batch { batch: row } = &mut snapshot.trace.records[index].record else {
            unreachable!()
        };
        assert!(
            row.before
                .objects
                .iter()
                .any(|o| o.object.card == CardId(1) && o.deathtouch_damage)
        );
        row.before.objects[0].deathtouch_damage = false;
        assert!(
            Game::restore(g.prepared.as_ref().clone(), &canonical(&snapshot).unwrap()).is_err()
        );
        let mut snapshot: Snapshot = strict_read(&g.snapshot().unwrap()).unwrap();
        snapshot
            .state
            .rules
            .objects
            .get_mut(&CardId(1))
            .unwrap()
            .deathtouch_damage = true;
        snapshot.state_sha256 = digest(&snapshot.state).unwrap();
        assert!(
            Game::restore(g.prepared.as_ref().clone(), &canonical(&snapshot).unwrap()).is_err()
        );
    }
}
#[test]
fn deathtouch_window_no_sba_survivor_grant_loss_cleanup_and_fresh_damage() {
    both(setup("Yoked Ox", "Typhoid Rats", true), |g, q| {
        equip(g, q, 0, 1);
        assert_eq!(
            g.state.rules.objects.values().filter(|o| o.tapped).count(),
            2
        );
        combat(g, q, 2, 1);
        assert_eq!(g.current(object(g, 2)).unwrap().zone, Zone::Battlefield);
        survive_grant_loss(g, q);
        // Ordinary cleanup must remove marks before P2's next actual combat.
        for _ in 0..300 {
            let r = request(g, q);
            if matches!(
                r,
                Request::Attackers {
                    player: PlayerId(2),
                    ..
                }
            ) {
                break;
            }
            answer(g, q, pass_response(&r));
        }
        assert_eq!(
            g.current(object(g, 1)).unwrap().marked_damage,
            Quantity::zero()
        );
        assert!(!g.current(object(g, 1)).unwrap().deathtouch_damage);
        recover(g);
        // Fresh positive deathtouch damage opens a new window, and kills Ox.
        answer(
            g,
            q,
            Response::Attackers {
                attacks: vec![(object(g, 8), PlayerId(0))],
            },
        );
        for _ in 0..100 {
            let r = request(g, q);
            if g.current(object(g, 1)).unwrap().zone != Zone::Battlefield {
                break;
            }
            let response = match &r {
                Request::Blockers {
                    player: PlayerId(0),
                    ..
                } => Response::Blockers {
                    blocks: vec![(object(g, 1), object(g, 8))],
                },
                _ => pass_response(&r),
            };
            answer(g, q, response);
        }
        assert!(matches!(
            g.current(object(g, 1)).unwrap().zone,
            Zone::Graveyard { .. }
        ));
        assert!(
            g.events().is_empty()
                || g.events().iter().any(|e| matches!(
                    e,
                    Event::Committed {
                        proposed: ProposedEvent::StateBasedGraveyard {
                            reason: CreatureLossReason::Deathtouch,
                            ..
                        },
                        ..
                    }
                ))
        );
    });
}
#[test]
fn deathtouch_window_other_sba_wave_survivor_grant_loss() {
    both(setup("Grizzly Bears", "Typhoid Rats", false), |g, q| {
        equip(g, q, 0, 1);
        combat(g, q, 2, 1);
        assert!(matches!(
            g.current(object(g, 2)).unwrap().zone,
            Zone::Graveyard { .. }
        ));
        survive_grant_loss(g, q);
    });
}
#[test]
fn deathtouch_window_ordinary_lethal_marks_still_kill_on_grant_loss() {
    both(setup("Grizzly Bears", "Grizzly Bears", false), |g, q| {
        equip(g, q, 0, 1);
        combat(g, q, 2, 2);
        assert_eq!(g.current(object(g, 1)).unwrap().zone, Zone::Battlefield);
        assert!(!g.current(object(g, 1)).unwrap().deathtouch_damage);
        bounce_plate(g, q);
        assert!(matches!(
            g.current(object(g, 1)).unwrap().zone,
            Zone::Graveyard { .. }
        ));
    });
}

#[test]
fn deathtouch_window_typed_atomic_identity_cleanup_and_bookkeeping_controls() {
    // Direct typed event probes below are kernel tests, not accepted-input gameplay.
    let mut g = Game::with_trace(
        setup("Yoked Ox", "Typhoid Rats", false),
        TraceMode::Detailed,
    );
    request(&mut g, 1);
    equip(&mut g, 1, 0, 1);
    let victim = object(&g, 1);
    let source = object(&g, 2);
    let damage = ProposedEvent::CombatDamage {
        damage: vec![DamageOccurrence {
            source,
            controller: PlayerId(1),
            recipient: DamageRecipient::Creature { object: victim },
            amount: 1.into(),
        }],
    };
    g.commit(damage.clone(), BatchCause::Fixture).unwrap();
    let consume = ProposedEvent::ConsumeDeathtouchWindow {
        objects: vec![victim],
    };
    let stale = ObjectId {
        incarnation: victim.incarnation + 1,
        ..victim
    };
    let before = g.state.clone();
    let trace = g.trace.clone();
    for bad in [
        vec![],
        vec![stale],
        vec![victim, victim],
        vec![source],
        vec![victim, stale],
    ] {
        assert!(
            g.commit(
                ProposedEvent::ConsumeDeathtouchWindow { objects: bad },
                BatchCause::DeathtouchCheck
            )
            .is_err()
        );
        assert_eq!(g.state, before);
        assert_eq!(g.trace, trace);
    }
    assert!(
        g.commit_batch(
            BatchCause::Fixture,
            EventGroup::Simultaneous {
                occurrences: vec![consume.clone(), ProposedEvent::Tap { object: source }],
            }
        )
        .is_err()
    );
    assert_eq!(g.state, before);
    assert!(
        g.commit_batch(
            BatchCause::Fixture,
            EventGroup::Ordered {
                children: vec![
                    EventGroup::Occurrence {
                        proposed: consume.clone()
                    },
                    EventGroup::Occurrence {
                        proposed: ProposedEvent::Tap { object: stale }
                    },
                ]
            }
        )
        .is_err()
    );
    assert_eq!(g.state, before);
    assert_eq!(g.trace, trace);
    // Actual cleanup, including a flag set without a completed intervening SBA check.
    let mut cleaned = g.fork();
    assert!(
        cleaned
            .commit_batch(
                BatchCause::Cleanup,
                EventGroup::Ordered {
                    children: vec![
                        EventGroup::Occurrence {
                            proposed: ProposedEvent::CleanupEffects
                        },
                        EventGroup::Occurrence {
                            proposed: ProposedEvent::Tap { object: stale }
                        },
                    ]
                }
            )
            .is_err()
    );
    assert_eq!(cleaned.state, before);
    assert_eq!(cleaned.trace, trace);
    let batch = cleaned
        .commit_batch(
            BatchCause::Cleanup,
            EventGroup::Occurrence {
                proposed: ProposedEvent::CleanupEffects,
            },
        )
        .unwrap();
    assert!(
        batch
            .before
            .objects
            .iter()
            .any(|o| o.object == victim && o.deathtouch_damage && o.marked_damage == 1.into())
    );
    assert!(!cleaned.current(victim).unwrap().deathtouch_damage);
    assert_eq!(
        cleaned.current(victim).unwrap().marked_damage,
        Quantity::zero()
    );
    // Bookkeeping alone in a cleanup check must not request priority or repeat.
    g.stabilization_wave_with_legends(Frame::Transition, true, false, None)
        .unwrap();
    assert!(matches!(g.state.rules.frame, Frame::Transition));
    assert!(!g.state.rules.cleanup_repeat);
    assert_eq!(g.current(victim).unwrap().marked_damage, 1.into());
    assert!(!g.current(victim).unwrap().deathtouch_damage);
    let batch = g
        .trace
        .records
        .iter()
        .find_map(|r| match &r.record {
            EvidenceRecord::Batch { batch } if batch.cause == BatchCause::DeathtouchCheck => {
                Some(batch)
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(batch.before.objects.len(), 1);
    assert!(batch.before.objects[0].deathtouch_damage);
    assert_eq!(
        batch.occurrences[0].actual,
        EventOutcome::DeathtouchWindowConsumed {
            objects: vec![victim]
        }
    );
    let count = g.state.rules.next_batch_id;
    g.stabilization_wave_with_legends(Frame::Transition, true, false, None)
        .unwrap();
    assert_eq!(
        g.state.rules.next_batch_id, count,
        "no empty bookkeeping events"
    );
    assert!(g.commit(consume, BatchCause::DeathtouchCheck).is_err());
    // A later incarnation cannot consume the old checked identity.
    let movement = g
        .move_object(victim, Zone::Hand { owner: PlayerId(0) })
        .unwrap();
    let successor = g
        .move_object(movement.successor, Zone::Battlefield)
        .unwrap()
        .successor;
    assert_ne!(successor, victim);
    assert!(
        g.commit(
            ProposedEvent::ConsumeDeathtouchWindow {
                objects: vec![victim]
            },
            BatchCause::DeathtouchCheck
        )
        .is_err()
    );
}
