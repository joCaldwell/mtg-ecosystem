//! Complete-source Research Thief and Alpha Myr combat/replay witnesses.
//! Standalone predicate probes below use real source facts and explicitly typed
//! patterns; they do not modify or claim new semantics for a card definition.
use super::*;
const RESEARCH: &str = "ba5055ce-6809-47a4-bee6-fb97a5b60f8a";
const ALPHA: &str = "4caa8e68-e599-416f-b839-3d009569ff29";
const BEARS: &str = "14c8f55d-d177-4c25-a931-ebeb9e6062a0";
const RING: &str = "6ad8011d-3471-4369-9d68-b264cc027487";
const SWAMP: &str = "56719f6a-1a6c-4c0a-8d21-18f7d7350b68";
fn catalog() -> Catalog {
    let mut c = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap();
    let manifest = include_str!(
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
            mtg_compiler::compile(manifest, raw)
        else {
            panic!("complete filtered-damage source must lower");
        };
        c.definitions.push(*definition);
    }
    c.schema_version = 30;
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

fn object(g: &Game, id: u32) -> ObjectId {
    g.object_id(CardId(id)).unwrap()
}
fn game() -> Game {
    Game::new(prepared(&[
        (RESEARCH, 2, 0, Zone::Battlefield),
        (ALPHA, 3, 0, Zone::Battlefield),
    ]))
}
fn attack(g: &mut Game, turn: u64, attacks: &[(u32, u8)]) {
    to_step(g, turn, TurnStep::DeclareAttackers);
    let attacks = attacks
        .iter()
        .map(|(id, p)| (object(g, *id), PlayerId(*p)))
        .collect();
    answer(g, Response::Attackers { attacks });
}
fn pending(g: &mut Game) {
    for _ in 0..500 {
        if !g.state.rules.pending_triggers.is_empty() {
            return;
        }
        match g.advance(Budget::new(1)) {
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(_) => passive(g),
            other => panic!("missing filtered trigger: {other:?}"),
        }
    }
    panic!("missing filtered trigger");
}
fn witness(t: &PendingTrigger) -> TriggerEventWitness {
    serde_json::from_value(serde_json::to_value(t).unwrap()["witness"].clone()).unwrap()
}
#[test]
fn filtered_damage_exact_sources_separate_observer_damager_owner_and_replay() {
    for mode in [TraceMode::Off, TraceMode::Detailed] {
        let mut g = Game::with_trace(
            prepared(&[
                (RESEARCH, 2, 0, Zone::Battlefield),
                (ALPHA, 3, 0, Zone::Battlefield),
            ]),
            mode,
        );
        attack(&mut g, 1, &[(1, 1)]);
        pending(&mut g);
        assert_eq!(g.state.rules.pending_triggers.len(), 1);
        let trigger = &g.state.rules.pending_triggers[0];
        assert_eq!(trigger.source, object(&g, 0));
        assert_eq!(trigger.controller, PlayerId(0));
        assert_eq!(trigger.source_facts.owner, PlayerId(2));
        assert!(
            matches!(witness(trigger),TriggerEventWitness::DamageDealt {damage,..} if damage.source.object==object(&g,1) && damage.source.owner==PlayerId(3) && damage.source.controller==PlayerId(0))
        );
        recover(&g);
        placed(&mut g);
        recover(&g);
        let count = g.state.rules.libraries[&PlayerId(0)].len();
        resolve(&mut g);
        assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), count - 1);
        recover(&g);
    }
}
#[test]
fn filtered_damage_creature_type_conjunction_control_and_actual_occurrence_negatives() {
    // Standalone typed predicate probes retain exact source facts. In
    // particular the invocation controller cannot replace the damager's.
    let mut g = Game::new(prepared(&[
        (RESEARCH, 2, 0, Zone::Battlefield),
        (ALPHA, 3, 0, Zone::Battlefield),
        (RING, 0, 0, Zone::Battlefield),
        (ALPHA, 0, 1, Zone::Battlefield),
        (BEARS, 0, 0, Zone::Battlefield),
    ]));
    let original = g.definition(CardId(0)).unwrap().triggered[0]
        .condition
        .clone();
    for (id, expected) in [(1, true), (2, false), (3, false), (4, false)] {
        let mut actual = ActualDamage {
            id: DamageId(0),
            source: g.object_before(object(&g, id)).unwrap(),
            controller: PlayerId(1),
            recipient: DamageRecipient::Player {
                player: PlayerId(2),
            },
            amount: 1.into(),
        };
        assert_eq!(
            triggers::damage_matches(
                &original,
                object(&g, 0),
                PlayerId(0),
                DamageKind::Combat,
                &actual
            ),
            expected
        );
        assert!(!triggers::damage_matches(
            &original,
            object(&g, 0),
            PlayerId(0),
            DamageKind::Noncombat,
            &actual
        ));
        if id == 1 || id == 3 {
            let mut pattern = original.clone();
            let TriggerCondition::DamageDealt { kind, .. } = &mut pattern else {
                panic!("filter");
            };
            *kind = TriggerDamageKind::Any;
            assert_eq!(
                triggers::damage_matches(
                    &pattern,
                    object(&g, 0),
                    PlayerId(0),
                    DamageKind::Noncombat,
                    &actual
                ),
                id == 1
            );
        }
        // A permanent pattern never matches a creature card in another zone,
        // while the original frozen battlefield fact remains usable as LKI.
        let battlefield_fact = actual.source.clone();
        actual.source.zone = Zone::Hand {
            owner: actual.source.owner,
        };
        assert!(!triggers::damage_matches(
            &original,
            object(&g, 0),
            PlayerId(0),
            DamageKind::Combat,
            &actual
        ));
        actual.source = battlefield_fact;
        actual.amount = 0.into();
        assert!(!triggers::damage_matches(
            &original,
            object(&g, 0),
            PlayerId(0),
            DamageKind::Combat,
            &actual
        ));
    }
    let ring = object(&g, 2);
    let state = g.state.clone();
    assert!(
        g.commit(
            ProposedEvent::CombatDamage {
                damage: vec![DamageOccurrence {
                    source: ring,
                    controller: PlayerId(0),
                    recipient: DamageRecipient::Player {
                        player: PlayerId(1)
                    },
                    amount: 1.into()
                }]
            },
            BatchCause::Fixture
        )
        .is_err()
    );
    assert_eq!(g.state, state);
    // Accepted combat negatives: a nonartifact creature and an artifact
    // creature owned by the observer's controller but controlled by an opponent.
    for enemy in [false, true] {
        let mut g = Game::new(prepared(&[
            (RESEARCH, 2, 0, Zone::Battlefield),
            (
                if enemy { ALPHA } else { BEARS },
                0,
                u8::from(enemy),
                Zone::Battlefield,
            ),
        ]));
        let turn = if enemy { 2 } else { 1 };
        attack(&mut g, turn, &[(1, if enemy { 0 } else { 1 })]);
        to_step(&mut g, turn, TurnStep::PostcombatMain);
        assert_eq!(g.state.rules.next_trigger_id, 0);
        recover(&g);
    }
}
#[test]
fn filtered_damage_multiple_observers_and_members_retain_occurrence_multiplicity() {
    let mut g = Game::new(prepared(&[
        (RESEARCH, 2, 0, Zone::Battlefield),
        (RESEARCH, 1, 0, Zone::Battlefield),
        (ALPHA, 3, 0, Zone::Battlefield),
        (ALPHA, 3, 0, Zone::Battlefield),
    ]));
    attack(&mut g, 1, &[(2, 1), (3, 1)]);
    pending(&mut g);
    assert_eq!(g.state.rules.pending_triggers.len(), 4);
    let pairs = g
        .state
        .rules
        .pending_triggers
        .iter()
        .map(|t| {
            let TriggerEventWitness::DamageDealt { damage, .. } = witness(t) else {
                panic!("damage");
            };
            (t.source, damage.source.object)
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(pairs.len(), 4);
    recover(&g);
    placed(&mut g);
    let count = g.state.rules.libraries[&PlayerId(0)].len();
    while !g.state.rules.stack.is_empty() {
        resolve(&mut g);
    }
    assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), count - 4);
    recover(&g);
}
#[test]
fn filtered_damage_observer_and_damager_departure_preserve_frozen_witness() {
    for departing in [vec![2], vec![3], vec![2, 3]] {
        let mut g = game();
        attack(&mut g, 1, &[(1, 1)]);
        pending(&mut g);
        let frozen = witness(&g.state.rules.pending_triggers[0]);
        for player in departing {
            g.concede(PlayerId(player)).unwrap();
        }
        placed(&mut g);
        recover(&g);
        assert!(
            matches!(&g.top().unwrap().kind,StackKind::Triggered {witness,..} if witness==&frozen)
        );
        let count = g.state.rules.libraries[&PlayerId(0)].len();
        resolve(&mut g);
        assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), count - 1);
        recover(&g);
    }
}
#[test]
fn filtered_damage_coherent_damager_and_recipient_lies_cannot_authorize_recovery() {
    let mut g = Game::new(prepared(&[
        (RESEARCH, 2, 0, Zone::Battlefield),
        (ALPHA, 3, 0, Zone::Battlefield),
        (ALPHA, 3, 0, Zone::Battlefield),
    ]));
    attack(&mut g, 1, &[(1, 1)]);
    pending(&mut g);
    recover(&g);
    for lie in ["damager", "recipient"] {
        let mut forged: serde_json::Value = serde_json::from_str(&g.snapshot().unwrap()).unwrap();
        let damage = &mut forged["state"]["pending_triggers"][0]["witness"]["damage"];
        if lie == "damager" {
            damage["source"] =
                serde_json::to_value(g.object_before(object(&g, 2)).unwrap()).unwrap();
        } else {
            damage["recipient"]["player"] = 3.into();
        }
        let decoded: Snapshot = serde_json::from_value(forged.clone()).unwrap();
        let mut structural = g.fork();
        structural.state = decoded.state;
        structural.validate_trigger_state().unwrap();
        forged["state_sha256"] = digest(&forged["state"]).unwrap().into();
        assert!(
            Game::restore(g.prepared.as_ref().clone(), &forged.to_string()).is_err(),
            "{lie}"
        );
    }
}
#[test]
fn filtered_damage_hosting_search_remain_excluded() {
    let c = catalog();
    for (search, hosted, feature) in [
        (true, false, Feature::Search),
        (false, true, Feature::Hosting),
    ] {
        let mut profile = match_profile("commander-foundations-v1").unwrap().clone();
        profile.search = search;
        profile.hosted = hosted;
        assert!(
            matches!(profile.validate_requested_features(&c),Err(ProfileError::UnreviewedCapabilities {feature:f,capabilities}) if f==feature&&capabilities.contains(&Capability::FilteredDamageTriggerV1))
        );
    }
}
