//! Ordinary creature-filter triggers inspect battlefield appearance at the
//! actual event (CR 109.2/603.10). Already assigned combat damage remains real
//! after source departure under the separately reviewed CR 510.2 contract.
use super::*;

fn prepared() -> Prepared {
    let mut c = FixtureCatalog::Witness.get().clone();
    let header = include_str!("../../../../../tests/fixtures/archive-manifest-phase1-r5.json");
    for raw in [
        include_str!(
            "../../../../../tests/fixtures/phase1-r5/ba5055ce-6809-47a4-bee6-fb97a5b60f8a.json"
        ),
        include_str!(
            "../../../../../tests/fixtures/phase1-r5/4caa8e68-e599-416f-b839-3d009569ff29.json"
        ),
    ] {
        let mtg_compiler::CompileResult::Lowered { definition } = mtg_witness::compile(header, raw)
        else {
            panic!("complete Research Thief/Alpha Myr source must lower");
        };
        if let Some(existing) = c.definition_by_oracle_id(&definition.oracle_id) {
            assert_eq!(existing, definition.as_ref());
        } else {
            c.definitions.push(*definition);
        }
    }
    c.schema_version = mtg_contracts::CURRENT_IR_VERSION;
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    let mut board =
        Board::new(c, "damage-match-appearance-domain-witness").step(TurnStep::PrecombatMain);
    board = board
        .card("Research Thief", 2, 0, Zone::Battlefield)
        .card("Alpha Myr", 3, 0, Zone::Battlefield)
        .libraries("Swamp", 4);
    board.prepare()
}
fn passive(g: &mut Game, r: Decision<Rules>) {
    let response = match &r {
        Decision {
            reason: Reason::Priority,
            ..
        } => Answer::Pick {
            picks: vec![vec![0]],
        },
        Decision {
            reason: Reason::Blockers,
            ..
        } => r.block(vec![]),
        found @ Decision {
            reason: Reason::OrderTriggers,
            ..
        } => {
            let candidates = found
                .candidates
                .iter()
                .filter_map(|c| {
                    if let Candidate::Trigger { trigger } = c {
                        Some(*trigger)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            Answer::Order {
                order: (candidates.clone())
                    .into_iter()
                    .map(|trigger| r.test_index(Candidate::Trigger { trigger }))
                    .collect(),
            }
        }
        found @ Decision {
            reason: Reason::DamageEffect { .. },
            ..
        } => {
            let candidates = found
                .candidates
                .iter()
                .filter_map(|c| {
                    if let Candidate::DamageEffect { effect, damage } = c {
                        Some(DamageEffectCandidate {
                            effect: effect.clone(),
                            damage: *damage,
                        })
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            {
                let c = &candidates[0];
                r.select(Candidate::DamageEffect {
                    effect: c.effect.clone(),
                    damage: c.damage,
                })
            }
        }
        x => panic!("unexpected request {x:?}"),
    };
    g.respond(r.player, r.id, response).unwrap();
}
// Pause after assignment and before damage is committed (CR 510.2).
fn assigned_damage_boundary(g: &mut Game) {
    for _ in 0..1000 {
        if let Ok(tx) = g.damage_transaction() {
            assert_eq!(tx.entries.len(), 1);
            assert_eq!(tx.entries[0].source.object.card, CardId(1));
            assert_eq!(tx.entries[0].original_amount, 2.into());
            assert_eq!(g.state.rules.players[1].life, 40.into());
            return;
        }
        match g.advance(Budget::new(1)) {
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(
                r @ Decision {
                    reason: Reason::Attackers,
                    ..
                },
            ) => {
                let alpha = g.object_id(CardId(1)).unwrap();
                g.respond(
                    r.player,
                    r.id,
                    g.next_test_decision().attack(vec![(alpha, PlayerId(1))]),
                )
                .unwrap();
            }
            Advance::AwaitingDecision(r) => passive(g, r),
            other => panic!("expected combat damage, got {other:?}"),
        }
    }
    panic!("missing assigned Alpha Myr combat damage");
}
fn finish_damage(g: &mut Game) {
    for _ in 0..100 {
        let r = Driver::request(g, u64::MAX);
        if g.state.rules.players[1].life < 40.into() {
            assert!(matches!(
                r,
                Decision {
                    reason: Reason::Priority,
                    ..
                }
            ));
            return;
        }
        passive(g, r);
    }
    panic!("missing committed damage");
}
fn actual_alpha(g: &Game) -> ActualDamage {
    g.trace
        .records
        .iter()
        .find_map(|r| {
            let EvidenceRecord::Batch { batch } = &r.record else {
                return None;
            };
            batch.occurrences.iter().find_map(|o| {
                let EventOutcome::DamageCommitted { actual, .. } = &o.actual else {
                    return None;
                };
                actual
                    .iter()
                    .find(|d| d.source.object.card == CardId(1))
                    .cloned()
            })
        })
        .expect("real committed Alpha Myr damage remains in detailed evidence")
}
fn hand(g: &Game) -> usize {
    g.state
        .rules
        .objects
        .values()
        .filter(|o| o.zone == (Zone::Hand { owner: PlayerId(0) }))
        .count()
}

#[test]
fn damage_match_appearance_departed_damager_does_not_trigger_from_prepared_battlefield_facts() {
    let mut g = Game::with_trace(prepared(), TraceMode::Detailed);
    assigned_damage_boundary(&mut g);

    g.concede(PlayerId(3)).unwrap();
    finish_damage(&mut g);
    assert!(g.object_id(CardId(1)).is_none());
    assert!(g.object_id(CardId(0)).is_some());
    let damage = actual_alpha(&g);
    assert_eq!(damage.source.owner, PlayerId(3));
    assert_eq!(damage.source.zone, Zone::Battlefield);
    assert!(damage.amount > Quantity::zero());
    let mut expected_life = Quantity::from(40);
    expected_life.subtract(&damage.amount);
    assert_eq!(g.state.rules.players[1].life, expected_life);
    // CR 510.2 preserves actual assigned damage. CR 109.2/603.10 does not
    // turn the absent historical source into a creature currently controlled.
    assert_eq!(
        g.state.rules.next_trigger_id, 0,
        "departed damager must not satisfy Research Thief's ordinary creature predicate"
    );
    assert!(g.state.rules.pending_triggers.is_empty());
    assert!(g.state.rules.stack.is_empty());
    assert_eq!(hand(&g), 0);
}

#[test]
fn damage_match_appearance_departed_observer_does_not_collect_but_damage_survives() {
    let mut g = Game::with_trace(prepared(), TraceMode::Detailed);
    assigned_damage_boundary(&mut g);
    g.concede(PlayerId(2)).unwrap();
    finish_damage(&mut g);
    assert!(g.object_id(CardId(0)).is_none());
    assert!(g.object_id(CardId(1)).is_some());
    assert!(actual_alpha(&g).amount > Quantity::zero());
    assert_eq!(g.state.rules.next_trigger_id, 0);
    assert!(g.state.rules.stack.is_empty());
    assert_eq!(hand(&g), 0);
}

#[test]
fn damage_match_appearance_damager_departure_after_collection_keeps_frozen_trigger_and_draw() {
    let mut g = Game::with_trace(prepared(), TraceMode::Detailed);
    assigned_damage_boundary(&mut g);
    finish_damage(&mut g);
    assert_eq!(g.state.rules.next_trigger_id, 1);
    assert_eq!(g.state.rules.stack.len(), 1);
    assert!(actual_alpha(&g).amount > Quantity::zero());

    g.concede(PlayerId(3)).unwrap();
    assert!(matches!(
        Driver::request(&mut g, u64::MAX),
        Decision {
            reason: Reason::Priority,
            ..
        }
    ));
    assert!(g.object_id(CardId(1)).is_none());
    assert_eq!(g.state.rules.stack.len(), 1);

    for _ in 0..g.live_players().len() {
        {
            let selected_answer = Answer::Pick {
                picks: vec![vec![0]],
            };
            Driver::answer(&mut g, u64::MAX, selected_answer)
        };
    }
    assert!(matches!(
        Driver::request(&mut g, u64::MAX),
        Decision {
            reason: Reason::Priority,
            ..
        }
    ));
    assert!(g.state.rules.stack.is_empty());
    assert_eq!(hand(&g), 1);
    assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), 3);
}

fn match_source_mut(snapshot: &mut Snapshot) -> &mut Option<Box<ObjectBefore>> {
    let StackKind::Triggered { witness, .. } = &mut snapshot.state.rules.stack[0].kind else {
        panic!("placed Research Thief trigger");
    };
    let TriggerEventWitness::DamageDealt { match_source, .. } = witness else {
        panic!("damage trigger witness");
    };
    match_source
}

#[test]
fn damage_match_appearance_recovery_rejects_invalid_and_coherent_appearance_forgeries() {
    let mut g = Game::with_trace(prepared(), TraceMode::Detailed);
    assigned_damage_boundary(&mut g);
    finish_damage(&mut g);
    assert_eq!(g.state.rules.stack.len(), 1);
    let baseline: Snapshot = strict_read(&g.snapshot().unwrap()).unwrap();
    let mut inspection = baseline.clone();
    let appearance = match_source_mut(&mut inspection).as_ref().unwrap();
    assert_eq!(appearance.object, g.object_id(CardId(1)).unwrap());
    assert_eq!(appearance.owner, PlayerId(3));
    assert_eq!(appearance.controller, PlayerId(0));
    assert_eq!(appearance.zone, Zone::Battlefield);
    assert!(appearance.characteristics.has_type(CardKind::Artifact));
    assert!(appearance.characteristics.has_type(CardKind::Creature));
    assert!(!appearance.characteristics.snow);

    for lie in [
        "missing",
        "incarnation",
        "owner",
        "controller",
        "snow",
        "artifact",
    ] {
        let mut forged = baseline.clone();
        let slot = match_source_mut(&mut forged);
        if lie == "missing" {
            *slot = None;
        } else {
            let source = slot.as_mut().unwrap();
            match lie {
                "incarnation" => source.object.incarnation += 1,
                "owner" => source.owner = PlayerId(0),
                "controller" => source.controller = PlayerId(1),
                "snow" => source.characteristics.snow = true,
                "artifact" => source.characteristics.additional_types.clear(),
                _ => unreachable!(),
            }
        }
        let mut structural = g.fork();
        structural.state = forged.state.clone();
        assert!(structural.validate_trigger_state().is_err(), "{lie}");
        forged.state_sha256 = digest(&forged.state).unwrap();
        assert!(
            Game::restore(
                g.prepared.as_ref().clone(),
                &serde_json::to_string(&forged).unwrap()
            )
            .is_err(),
            "{lie}"
        );
    }

    // Creature-filter matching does not constrain derived power. This lie keeps
    // the source identity, type and controller coherent and still matches the
    // complete Research predicate; accepted-input replay proves the actual P/T.
    let mut coherent = baseline;
    match_source_mut(&mut coherent)
        .as_mut()
        .unwrap()
        .characteristics
        .power = Some(9.into());
    let mut structural = g.fork();
    structural.state = coherent.state.clone();
    structural.validate_trigger_state().unwrap();
    coherent.state_sha256 = digest(&coherent.state).unwrap();
    assert!(
        Game::restore(
            g.prepared.as_ref().clone(),
            &serde_json::to_string(&coherent).unwrap()
        )
        .is_err()
    );
}
