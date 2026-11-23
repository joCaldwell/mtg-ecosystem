//! Exact-source V2 combat/recovery witnesses and explicitly private event
//! witnesses for noncombat occurrences (the source has no damage activation).
use super::*;
fn catalog() -> Catalog {
    let mut c = FixtureCatalog::Witness.get().clone();
    let mtg_compiler::CompileResult::Lowered { definition } = mtg_witness::compile(
        include_str!("../../../../../tests/fixtures/archive-manifest-phase1-r4.json"),
        include_str!(
            "../../../../../tests/fixtures/phase1-r4/5dee663f-9e2e-4503-8086-fc51664fdf29.json"
        ),
    ) else {
        panic!("complete Thieving Otter source must lower")
    };
    if let Some(existing) = c.definition_by_oracle_id(&definition.oracle_id) {
        assert_eq!(existing, definition.as_ref());
    } else {
        c.definitions.push(*definition);
    }
    c.schema_version = mtg_contracts::CURRENT_IR_VERSION;
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    c
}
fn prepared(sources: &[(&str, u8, u8, Zone)]) -> Prepared {
    let c = catalog();
    let mut board = Board::new(c, "step-program-domain-witness").step(TurnStep::PrecombatMain);
    for &(name, owner, controller, zone) in sources {
        board = board.card(name, owner, controller, zone);
    }
    board = board.libraries("Swamp", 8);
    for seat in 0..4 {
        for name in [
            "Swamp", "Swamp", "Swamp", "Swamp", "Swamp", "Swamp", "Plains", "Forest", "Mountain",
            "Island", "Island",
        ] {
            board = board.card(name, seat, seat, Zone::Battlefield);
        }
    }
    board.prepare()
}
fn passive(g: &mut Game) {
    let r = Driver::request(g, u64::MAX);
    let (player, decision_id) = (r.player(), r.decision_id());
    let response = match r {
        Request::Priority { .. } => Response::Pass,
        Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
        Request::Blockers { .. } => Response::Blockers { blocks: vec![] },
        Request::OrderTriggers { candidates, .. } => Response::OrderTriggers { order: candidates },
        Request::Discard {
            count, candidates, ..
        } => Response::Discard {
            objects: candidates.into_iter().take(count).collect(),
        },
        other => panic!("unexpected passive request {other:?}"),
    };
    g.respond(player, decision_id, response).unwrap();
}
fn to_step(g: &mut Game, number: u64, step: TurnStep) {
    for _ in 0..500 {
        let _ = Driver::request(g, u64::MAX);
        if g.state.rules.turn.number == number && g.state.rules.turn.step == step {
            return;
        }
        passive(g);
    }
    panic!("missing step {number}/{step:?}")
}
fn placed(g: &mut Game) {
    loop {
        match Driver::request(g, u64::MAX) {
            Request::Priority { .. } => return,
            Request::OrderTriggers { candidates, .. } => {
                Driver::answer(g, u64::MAX, Response::OrderTriggers { order: candidates })
            }
            r => panic!("placement {r:?}"),
        }
    }
}
fn resolve(g: &mut Game) {
    for _ in 0..g.live_players().len() {
        Driver::answer(g, u64::MAX, Response::Pass);
    }
    let _ = Driver::request(g, u64::MAX);
}

fn game(mode: TraceMode) -> Game {
    Game::with_trace(
        prepared(&[("Thieving Otter", 2, 0, Zone::Battlefield)]),
        mode,
    )
}
fn pending(g: &mut Game) {
    for _ in 0..500 {
        if !g.state.rules.pending_triggers.is_empty() {
            return;
        }
        match g.advance(Budget::new(1)) {
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(_) => passive(g),
            other => panic!("missing damage trigger: {other:?}"),
        }
    }
    panic!("missing damage trigger")
}
fn witness(g: &Game) -> TriggerEventWitness {
    serde_json::from_value(
        serde_json::to_value(&g.state.rules.pending_triggers[0]).unwrap()["witness"].clone(),
    )
    .unwrap()
}
fn legacy(g: &mut Game, recipient: DamageRecipient, amount: i64) -> Result<EventOutcome, String> {
    let source = g.object_before(Driver::object(g, 0)).unwrap();
    g.commit(
        ProposedEvent::NoncombatDamage {
            damage: Box::new(NoncombatDamageOccurrence {
                controller: source.controller,
                source,
                recipient,
                amount: amount.into(),
            }),
        },
        BatchCause::Fixture,
    )
}
#[test]
fn opponent_damage_exact_source_combat_and_departures() {
    let mode = TraceMode::Off;
    for departing in [None, Some(0), Some(1), Some(2)] {
        let mut g = game(mode);
        to_step(&mut g, 1, TurnStep::DeclareAttackers);
        let source = Driver::object(&g, 0);
        Driver::answer(
            &mut g,
            u64::MAX,
            Response::Attackers {
                attacks: vec![(source, PlayerId(1))],
            },
        );
        pending(&mut g);
        let frozen = witness(&g);
        assert!(
            matches!(&frozen, TriggerEventWitness::DamageDealt { kind: DamageKind::Combat, damage, transaction: Some(_), .. }
                if damage.source.object == source && damage.source.owner == PlayerId(2) && damage.controller == PlayerId(0) && damage.amount == 2.into())
        );
        assert_eq!(g.state.rules.pending_triggers.len(), 1);

        if let Some(player) = departing {
            g.concede(PlayerId(player)).unwrap();
        }
        placed(&mut g);

        if departing == Some(0) {
            assert!(g.state.rules.stack.is_empty());
            continue;
        }
        assert!(
            matches!(&g.top().unwrap().kind, StackKind::Triggered { witness, .. } if witness == &frozen)
        );
        let n = g.state.rules.libraries[&PlayerId(0)].len();
        resolve(&mut g);
        assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), n - 1);
    }
}
#[test]
fn opponent_damage_private_noncombat_leaf_multiplicity_and_negative_recipients() {
    let mode = TraceMode::Detailed;
    let mut g = game(mode);
    let source = Driver::object(&g, 0);
    for recipient in [
        DamageRecipient::Player {
            player: PlayerId(0),
        },
        DamageRecipient::Creature { object: source },
    ] {
        legacy(&mut g, recipient, 1).unwrap();
        assert!(g.state.rules.pending_triggers.is_empty());
    }
    g.commit(
        ProposedEvent::LoseLife {
            player: PlayerId(1),
            amount: 1.into(),
        },
        BatchCause::Fixture,
    )
    .unwrap();
    assert!(g.state.rules.pending_triggers.is_empty());
    let before = g.state.clone();
    assert!(
        legacy(
            &mut g,
            DamageRecipient::Player {
                player: PlayerId(1)
            },
            0
        )
        .is_err()
    );
    assert_eq!(g.state, before);
    for _ in 0..2 {
        legacy(
            &mut g,
            DamageRecipient::Player {
                player: PlayerId(1),
            },
            1,
        )
        .unwrap();
    }
    assert_eq!(g.state.rules.pending_triggers.len(), 2);
    assert!(
        matches!(witness(&g), TriggerEventWitness::DamageDealt { kind: DamageKind::Noncombat, transaction: None, damage, .. } if damage.amount == 1.into())
    );
    g.validate_trigger_state().unwrap();
    // These direct typed-event fixtures do not claim accepted-input replay.
}
#[test]
fn opponent_damage_private_collection_uses_observer_controller_and_frozen_restore_relation() {
    for player in [0, 2, 3] {
        let mut g = game(TraceMode::Detailed);
        let before = g.fork();
        let source = Driver::object(&g, 0);
        let damage = NoncombatDamageOccurrence {
            source: g.object_before(source).unwrap(),
            controller: PlayerId(0),
            recipient: DamageRecipient::Player {
                player: PlayerId(player),
            },
            amount: 1.into(),
        };
        let batch = g
            .commit_batch(
                BatchCause::Fixture,
                EventGroup::Occurrence {
                    proposed: ProposedEvent::NoncombatDamage {
                        damage: Box::new(damage),
                    },
                },
            )
            .unwrap();
        // Private collector-only control change: assignment facts and invocation
        // controller originally remain P0, while the source's event observer is
        // P2. Vary the invocation controller independently to P1 below.
        // This is not a reachable pending-transaction or replay claim.
        g.state.rules.pending_triggers.clear();
        g.state
            .rules
            .objects
            .get_mut(&CardId(0))
            .unwrap()
            .controller = PlayerId(2);
        let mut occurrence = batch.occurrences[0].clone();
        if let EventOutcome::NoncombatDamageDealt { damage, .. } = &mut occurrence.actual {
            damage.controller = PlayerId(1);
        }
        g.collect_triggers(&BatchCause::Fixture, &before, &[occurrence])
            .unwrap();
        assert_eq!(
            g.state.rules.pending_triggers.len(),
            usize::from(player != 2)
        );
        if player != 2 {
            assert_eq!(g.state.rules.pending_triggers[0].controller, PlayerId(2));
            g.validate_trigger_state().unwrap();
            // A later controller does not reinterpret the frozen opponent test.
            g.state
                .rules
                .objects
                .get_mut(&CardId(0))
                .unwrap()
                .controller = PlayerId(player);
            g.validate_trigger_state().unwrap();
        }
    }
}

#[test]
fn opponent_damage_hosting_and_search_remain_excluded() {
    let c = catalog();
    for (search, hosted, feature) in [
        (true, false, Feature::Search),
        (false, true, Feature::Hosting),
    ] {
        let mut p = match_profile("commander-foundations-v1").unwrap().clone();
        p.search = search;
        p.hosted = hosted;
        assert!(
            matches!(p.validate_requested_features(&c), Err(ProfileError::UnreviewedCapabilities {feature:f,capabilities})
            if f == feature && capabilities.contains(&Capability::SourceDamageTriggerV2))
        );
    }
}
#[test]
fn opponent_damage_private_transaction_collection_checks_actual_member_proof() {
    let mut g = game(TraceMode::Off);
    let source = Driver::object(&g, 0);
    g.prepare_combat_damage(vec![DamageOccurrence {
        source,
        controller: PlayerId(0),
        recipient: DamageRecipient::Player {
            player: PlayerId(1),
        },
        amount: 2.into(),
    }])
    .unwrap();
    let mut before = g.fork();
    let id = g.damage_transaction().unwrap().id;
    let batch = g
        .commit_batch(
            BatchCause::Fixture,
            EventGroup::Occurrence {
                proposed: ProposedEvent::CommitDamage { transaction: id },
            },
        )
        .unwrap();
    // Private collector boundary: change the frozen transaction kind only to
    // prove noncombat collection uses the same actual-member checks. This is
    // not a validated noncombat continuation or accepted-input replay witness.
    if let Frame::DamageEffects { transaction } = &mut before.state.rules.frame {
        transaction.kind = DamageKind::Noncombat;
    } else {
        panic!("missing transaction")
    }
    g.state.rules.pending_triggers.clear();
    g.collect_triggers(&batch.cause, &before, &batch.occurrences)
        .unwrap();
    assert!(matches!(witness(&g), TriggerEventWitness::DamageDealt {
        kind: DamageKind::Noncombat, transaction: Some(tx), .. } if tx == id));
    g.validate_trigger_state().unwrap();
    for field in [
        "transaction",
        "member",
        "amount",
        "controller",
        "recipient",
        "source",
        "duplicate",
    ] {
        let mut changed = batch.occurrences.clone();
        let EventOutcome::DamageCommitted {
            transaction,
            actual,
            ..
        } = &mut changed[0].actual
        else {
            panic!("actual")
        };
        match field {
            "transaction" => transaction.0 += 1,
            "member" => actual[0].id.0 += 1,
            "amount" => actual[0].amount = 1.into(),
            "controller" => actual[0].controller = PlayerId(3),
            "recipient" => {
                actual[0].recipient = DamageRecipient::Player {
                    player: PlayerId(3),
                }
            }
            "source" => actual[0].source.controller = PlayerId(3),
            "duplicate" => actual.push(actual[0].clone()),
            _ => unreachable!(),
        }
        let mut fork = g.fork();
        let state = fork.state.clone();
        assert!(
            fork.collect_triggers(&batch.cause, &before, &changed)
                .is_err(),
            "{field}"
        );
        assert_eq!(fork.state, state, "{field}");
    }
    let mut prevented = batch.occurrences;
    let EventOutcome::DamageCommitted { actual, .. } = &mut prevented[0].actual else {
        panic!("actual")
    };
    actual.clear();
    g.state.rules.pending_triggers.clear();
    g.collect_triggers(&batch.cause, &before, &prevented)
        .unwrap();
    assert!(g.state.rules.pending_triggers.is_empty());
}
