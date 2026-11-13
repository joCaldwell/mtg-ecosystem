//! Exact-source V2 combat/recovery witnesses and explicitly private event
//! witnesses for noncombat occurrences (the source has no damage activation).
use super::*;
const OTTER: &str = "5dee663f-9e2e-4503-8086-fc51664fdf29";
const SWAMP: &str = "56719f6a-1a6c-4c0a-8d21-18f7d7350b68";
const PLAINS: &str = "bc71ebf6-2056-41f7-be35-b2e5c34afa99";
const FOREST: &str = "b34bb2dc-c1af-4d77-b0b3-a0fb342a5fc6";
const MOUNTAIN: &str = "a3fb7228-e76b-4e96-a40e-20b5fed75685";
const ISLAND: &str = "b2c6aa39-2d2a-459c-a555-fb48ba993373";
fn catalog() -> Catalog {
    let mut c = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap();
    let mtg_compiler::CompileResult::Lowered { definition } = mtg_compiler::compile(
        include_str!("../../../../tests/fixtures/cards-data-d2ce/archive-manifest-current.json"),
        include_str!(
            "../../../../tests/fixtures/cards-data-d2ce/5dee663f-9e2e-4503-8086-fc51664fdf29.json"
        ),
    ) else {
        panic!("complete Thieving Otter source must lower")
    };
    c.definitions.push(*definition);
    c.schema_version = mtg_contracts::CURRENT_IR_VERSION;
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    c
}
fn prepared(sources: &[(&str, u8, u8, Zone)]) -> Prepared {
    let c = catalog();
    let mut config = ScenarioConfig::turns_permanents();
    config.profile = "step-program-domain-witness".into();
    config.initial.turn_step = TurnStep::PrecombatMain;
    config.cards.clear();
    for (id, (oracle, owner, controller, zone)) in sources.iter().enumerate() {
        config.cards.push(CardSetup {
            id: CardId(id as u32),
            oracle_id: (*oracle).into(),
            owner: PlayerId(*owner),
            controller: PlayerId(*controller),
            zone: *zone,
        });
    }
    for seat in 0..4 {
        for _ in 0..8 {
            config.cards.push(CardSetup {
                id: CardId(config.cards.len() as u32),
                oracle_id: SWAMP.into(),
                owner: PlayerId(seat),
                controller: PlayerId(seat),
                zone: Zone::Library {
                    owner: PlayerId(seat),
                },
            });
        }
    }
    for seat in 0..4 {
        for oracle in [
            SWAMP, SWAMP, SWAMP, SWAMP, SWAMP, SWAMP, PLAINS, FOREST, MOUNTAIN, ISLAND, ISLAND,
        ] {
            config.cards.push(CardSetup {
                id: CardId(config.cards.len() as u32),
                oracle_id: oracle.into(),
                owner: PlayerId(seat),
                controller: PlayerId(seat),
                zone: Zone::Battlefield,
            });
        }
    }
    Prepared {
        catalog_digest: digest(&c).unwrap(),
        catalog: Arc::new(c),
        profile_digest: digest(&config).unwrap(),
        config,
        match_config: None,
    }
}
fn settle(g: &mut Game) -> Request {
    for _ in 0..500 {
        match g.advance(Budget::new(1)) {
            Advance::AwaitingDecision(r) => return r,
            Advance::Suspended { .. } => (),
            other => panic!("step witness at {:?}: {other:?}", g.state.rules.frame),
        }
    }
    panic!("step witness budget")
}
fn answer(g: &mut Game, response: Response) {
    let r = settle(g);
    g.respond(r.player(), r.decision_id(), response).unwrap();
}
fn passive(g: &mut Game) {
    let r = settle(g);
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
fn recover(g: &Game) {
    let restored = Game::restore(g.prepared.as_ref().clone(), &g.snapshot().unwrap()).unwrap();
    assert!(restored.state == g.state);
    assert_eq!(restored.trace, g.trace);
    let replayed = Game::replay_to(
        g.prepared.as_ref().clone(),
        &g.replay_journal(),
        Some(g.state.rules.rule_steps),
        g.trace.mode,
    )
    .unwrap();
    assert!(replayed.state == g.state);
    assert_eq!(replayed.trace, g.trace);
}
fn to_step(g: &mut Game, number: u64, step: TurnStep) {
    for _ in 0..500 {
        let _ = settle(g);
        if g.state.rules.turn.number == number && g.state.rules.turn.step == step {
            return;
        }
        passive(g);
    }
    panic!("missing step {number}/{step:?}")
}
fn placed(g: &mut Game) {
    loop {
        match settle(g) {
            Request::Priority { .. } => return,
            Request::OrderTriggers { candidates, .. } => {
                answer(g, Response::OrderTriggers { order: candidates })
            }
            r => panic!("placement {r:?}"),
        }
    }
}
fn resolve(g: &mut Game) {
    for _ in 0..g.live_players().len() {
        answer(g, Response::Pass);
    }
    let _ = settle(g);
}

fn game(mode: TraceMode) -> Game {
    Game::with_trace(prepared(&[(OTTER, 2, 0, Zone::Battlefield)]), mode)
}
fn modes() -> [TraceMode; 3] {
    [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed]
}
fn object(g: &Game) -> ObjectId {
    g.object_id(CardId(0)).unwrap()
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
    let source = g.object_before(object(g)).unwrap();
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
fn opponent_damage_exact_source_combat_replay_and_departures() {
    for mode in modes() {
        for departing in [None, Some(0), Some(1), Some(2)] {
            let mut g = game(mode);
            to_step(&mut g, 1, TurnStep::DeclareAttackers);
            let source = object(&g);
            answer(
                &mut g,
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
            recover(&g);
            if let Some(player) = departing {
                g.concede(PlayerId(player)).unwrap();
            }
            placed(&mut g);
            recover(&g);
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
            recover(&g);
        }
    }
}
#[test]
fn opponent_damage_private_noncombat_leaf_multiplicity_and_negative_recipients() {
    for mode in modes() {
        let mut g = game(mode);
        let source = object(&g);
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
}
#[test]
fn opponent_damage_private_collection_uses_observer_controller_and_frozen_restore_relation() {
    for player in [0, 2, 3] {
        let mut g = game(TraceMode::Detailed);
        let before = g.fork();
        let source = object(&g);
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
        g.collect_triggers(&before, &[occurrence]).unwrap();
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

fn pay_input(g: &mut Game) {
    let Request::SpendMana {
        player,
        cost,
        available,
        ..
    } = settle(g)
    else {
        panic!("spending")
    };
    let remaining = g.payment_pool(player, &cost).unwrap();
    let spending = available
        .into_iter()
        .filter_map(|(color, mut amount)| {
            amount.subtract(&remaining.get(&color).cloned().unwrap_or_default());
            (amount > Quantity::zero()).then_some((color, amount))
        })
        .collect();
    answer(g, Response::SpendMana { spending });
}
fn fund(g: &mut Game) {
    let player = settle(g).player();
    let lands = g
        .state
        .rules
        .objects
        .values()
        .filter(|o| {
            o.zone == Zone::Battlefield
                && o.controller == player
                && !o.tapped
                && g.definition(o.id.card).unwrap().characteristics.kind == CardKind::Land
        })
        .map(|o| o.id)
        .collect::<Vec<_>>();
    for object in lands {
        answer(g, Response::ActivateMana { object });
    }
    answer(g, Response::FinishMana { undo_mana: false });
    pay_input(g);
}
fn cast(g: &mut Game, card: u32) {
    answer(g, Response::Cast { card: CardId(card) });
    fund(g);
    placed(g);
}

#[test]
fn opponent_damage_exact_source_prevented_combat_has_no_trigger_and_replays() {
    let fog = catalog()
        .definitions
        .iter()
        .find(|d| d.name == "Fog")
        .unwrap()
        .oracle_id
        .clone();
    for mode in modes() {
        let mut g = Game::with_trace(
            prepared(&[
                (OTTER, 2, 0, Zone::Battlefield),
                (&fog, 0, 0, Zone::Hand { owner: PlayerId(0) }),
            ]),
            mode,
        );
        cast(&mut g, 1);
        resolve(&mut g);
        to_step(&mut g, 1, TurnStep::DeclareAttackers);
        let source = object(&g);
        answer(
            &mut g,
            Response::Attackers {
                attacks: vec![(source, PlayerId(1))],
            },
        );
        to_step(&mut g, 1, TurnStep::PostcombatMain);
        assert_eq!(g.state.rules.players[1].life, 40.into());
        assert_eq!(g.state.rules.next_trigger_id, 0);
        recover(&g);
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
    let source = object(&g);
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
    g.collect_triggers(&before, &batch.occurrences).unwrap();
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
        assert!(fork.collect_triggers(&before, &changed).is_err(), "{field}");
        assert_eq!(fork.state, state, "{field}");
    }
    let mut prevented = batch.occurrences;
    let EventOutcome::DamageCommitted { actual, .. } = &mut prevented[0].actual else {
        panic!("actual")
    };
    actual.clear();
    g.state.rules.pending_triggers.clear();
    g.collect_triggers(&before, &prevented).unwrap();
    assert!(g.state.rules.pending_triggers.is_empty());
}
