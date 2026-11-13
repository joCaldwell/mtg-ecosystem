//! Ordinary creature-filter triggers inspect battlefield appearance at the
//! actual event (CR 109.2/603.10). Already assigned combat damage remains real
//! after source departure under the separately reviewed CR 510.2 contract.
use super::*;

const RESEARCH: &str = "ba5055ce-6809-47a4-bee6-fb97a5b60f8a";
const ALPHA: &str = "4caa8e68-e599-416f-b839-3d009569ff29";
const FURNACE: &str = "68715465-6cf9-4006-87e9-31f227fe9ed3";
const HEALER: &str = "95a0ca48-d924-47f4-86ed-42c673ee778c";
const SWAMP: &str = "56719f6a-1a6c-4c0a-8d21-18f7d7350b68";

fn prepared() -> Prepared {
    let mut c = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap();
    let header = include_str!(
        "../../../../tests/fixtures/cards-data-filtered-damage/archive-manifest-current.json"
    );
    for raw in [
        include_str!(
            "../../../../tests/fixtures/cards-data-d2ce/ba5055ce-6809-47a4-bee6-fb97a5b60f8a.json"
        ),
        include_str!(
            "../../../../tests/fixtures/cards-data-filtered-damage/4caa8e68-e599-416f-b839-3d009569ff29.json"
        ),
    ] {
        let mtg_compiler::CompileResult::Lowered { definition } =
            mtg_compiler::compile(header, raw)
        else {
            panic!("complete Research Thief/Alpha Myr source must lower");
        };
        c.definitions.push(*definition);
    }
    c.schema_version = mtg_contracts::CURRENT_IR_VERSION;
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    let mut config = ScenarioConfig::turns_permanents();
    config.profile = "damage-match-appearance-domain-witness".into();
    config.initial.turn_step = TurnStep::PrecombatMain;
    config.cards = [
        (RESEARCH, 2, 0),
        (ALPHA, 3, 0),
        (FURNACE, 0, 0),
        (HEALER, 0, 0),
    ]
    .into_iter()
    .enumerate()
    .map(|(id, (oracle, owner, controller))| CardSetup {
        id: CardId(id as u32),
        oracle_id: oracle.into(),
        owner: PlayerId(owner),
        controller: PlayerId(controller),
        zone: Zone::Battlefield,
    })
    .collect();
    for seat in 0..4 {
        for _ in 0..4 {
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
    Prepared {
        catalog_digest: digest(&c).unwrap(),
        catalog: Arc::new(c),
        profile_digest: digest(&config).unwrap(),
        config,
        match_config: None,
    }
}
fn request(g: &mut Game) -> Request {
    for _ in 0..500 {
        match g.advance(Budget::new(1)) {
            Advance::AwaitingDecision(r) => return r,
            Advance::Suspended { .. } => (),
            x => panic!(
                "damage match appearance at {:?}: {x:?}",
                g.state.rules.frame
            ),
        }
    }
    panic!("damage match appearance budget");
}
fn answer(g: &mut Game, response: Response) {
    let r = request(g);
    g.respond(r.player(), r.decision_id(), response).unwrap();
}
fn passive(g: &mut Game, r: Request) {
    let response = match &r {
        Request::Priority { .. } => Response::Pass,
        Request::Blockers { .. } => Response::Blockers { blocks: vec![] },
        Request::OrderTriggers { candidates, .. } => Response::OrderTriggers {
            order: candidates.clone(),
        },
        Request::DamageEffect { candidates, .. } => {
            let c = &candidates[0];
            Response::DamageEffect {
                effect: c.effect.clone(),
                damage: c.damage,
            }
        }
        x => panic!("unexpected request {x:?}"),
    };
    g.respond(r.player(), r.decision_id(), response).unwrap();
}
fn recover(g: &Game) {
    let restored = Game::restore(g.prepared.as_ref().clone(), &g.snapshot().unwrap()).unwrap();
    assert_eq!(restored.state, g.state);
    assert_eq!(restored.trace, g.trace);
    let replayed = Game::replay_to(
        g.prepared.as_ref().clone(),
        &g.replay_journal(),
        Some(g.state.rules.rule_steps),
        g.trace.mode,
    )
    .unwrap();
    assert_eq!(replayed.state, g.state);
    assert_eq!(replayed.trace, g.trace);
}
fn ordering_boundary(g: &mut Game) {
    let object = g.object_id(CardId(3)).unwrap();
    answer(g, Response::ActivateAbility { object, ability: 0 });
    answer(
        g,
        Response::EntityTargets {
            entities: vec![TargetEntity::Player {
                player: PlayerId(1),
            }],
        },
    );
    for _ in 0..g.live_players().len() {
        answer(g, Response::Pass);
    }
    assert!(matches!(request(g), Request::Priority { .. }));
    for _ in 0..100 {
        let r = request(g);
        if matches!(r, Request::Attackers { .. }) {
            let alpha = g.object_id(CardId(1)).unwrap();
            g.respond(
                r.player(),
                r.decision_id(),
                Response::Attackers {
                    attacks: vec![(alpha, PlayerId(1))],
                },
            )
            .unwrap();
            break;
        }
        passive(g, r);
    }
    for _ in 0..100 {
        let r = request(g);
        if let Request::DamageEffect {
            player, candidates, ..
        } = &r
        {
            assert_eq!(*player, PlayerId(1));
            assert_eq!(candidates.len(), 2);
            let tx = g.damage_transaction().unwrap();
            assert_eq!(tx.entries.len(), 1);
            assert_eq!(tx.entries[0].source.object.card, CardId(1));
            assert_eq!(tx.entries[0].original_amount, 2.into());
            assert_eq!(g.state.rules.players[1].life, 40.into());
            return;
        }
        passive(g, r);
    }
    panic!("missing real Furnace/Healer ordering choice");
}
fn finish_damage(g: &mut Game) {
    for _ in 0..100 {
        let r = request(g);
        if g.state.rules.players[1].life < 40.into() {
            assert!(matches!(r, Request::Priority { .. }));
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
    ordering_boundary(&mut g);
    recover(&g);
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
    recover(&g);
}

#[test]
fn damage_match_appearance_departed_observer_does_not_collect_but_damage_survives() {
    let mut g = Game::with_trace(prepared(), TraceMode::Detailed);
    ordering_boundary(&mut g);
    g.concede(PlayerId(2)).unwrap();
    finish_damage(&mut g);
    assert!(g.object_id(CardId(0)).is_none());
    assert!(g.object_id(CardId(1)).is_some());
    assert!(actual_alpha(&g).amount > Quantity::zero());
    assert_eq!(g.state.rules.next_trigger_id, 0);
    assert!(g.state.rules.stack.is_empty());
    assert_eq!(hand(&g), 0);
    recover(&g);
}

#[test]
fn damage_match_appearance_damager_departure_after_collection_keeps_frozen_trigger_and_draw() {
    let mut g = Game::with_trace(prepared(), TraceMode::Detailed);
    ordering_boundary(&mut g);
    finish_damage(&mut g);
    assert_eq!(g.state.rules.next_trigger_id, 1);
    assert_eq!(g.state.rules.stack.len(), 1);
    assert!(actual_alpha(&g).amount > Quantity::zero());
    recover(&g);
    g.concede(PlayerId(3)).unwrap();
    assert!(matches!(request(&mut g), Request::Priority { .. }));
    assert!(g.object_id(CardId(1)).is_none());
    assert_eq!(g.state.rules.stack.len(), 1);
    recover(&g);
    for _ in 0..g.live_players().len() {
        answer(&mut g, Response::Pass);
    }
    assert!(matches!(request(&mut g), Request::Priority { .. }));
    assert!(g.state.rules.stack.is_empty());
    assert_eq!(hand(&g), 1);
    assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), 3);
    recover(&g);
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
    ordering_boundary(&mut g);
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
    recover(&g);

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
