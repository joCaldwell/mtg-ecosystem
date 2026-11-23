//! Complete-source Research Thief and Alpha Myr combat/replay witnesses.
//! Standalone predicate probes below use real source facts and explicitly typed
//! patterns; they do not modify or claim new semantics for a card definition.
use super::*;
fn catalog() -> Catalog {
    let mut c = FixtureCatalog::Witness.get().clone();
    let manifest = include_str!("../../../../../tests/fixtures/archive-manifest-phase1-r4.json");
    for raw in [
        include_str!(
            "../../../../../tests/fixtures/phase1-r4/ba5055ce-6809-47a4-bee6-fb97a5b60f8a.json"
        ),
        include_str!(
            "../../../../../tests/fixtures/phase1-r4/4caa8e68-e599-416f-b839-3d009569ff29.json"
        ),
    ] {
        let mtg_compiler::CompileResult::Lowered { definition } =
            mtg_witness::compile(manifest, raw)
        else {
            panic!("complete filtered-damage source must lower");
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
    c
}
fn prepared(sources: &[(&str, u8, u8, Zone)]) -> Prepared {
    let c = catalog();
    let mut board = Board::new(c, "step-program-domain-witness").step(TurnStep::PrecombatMain);
    for &(name, owner, controller, zone) in sources {
        board = board.card(name, owner, controller, zone);
    }
    board = board.libraries("Swamp", 8);
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

fn game() -> Game {
    Game::new(prepared(&[
        ("Research Thief", 2, 0, Zone::Battlefield),
        ("Alpha Myr", 3, 0, Zone::Battlefield),
    ]))
}
fn attack(g: &mut Game, turn: u64, attacks: &[(u32, u8)]) {
    to_step(g, turn, TurnStep::DeclareAttackers);
    let attacks = attacks
        .iter()
        .map(|(id, p)| (Driver::object(g, *id), PlayerId(*p)))
        .collect();
    Driver::answer(g, u64::MAX, Response::Attackers { attacks });
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
    let mode = TraceMode::Detailed;
    let mut g = Game::with_trace(
        prepared(&[
            ("Research Thief", 2, 0, Zone::Battlefield),
            ("Alpha Myr", 3, 0, Zone::Battlefield),
        ]),
        mode,
    );
    attack(&mut g, 1, &[(1, 1)]);
    pending(&mut g);
    assert_eq!(g.state.rules.pending_triggers.len(), 1);
    let trigger = &g.state.rules.pending_triggers[0];
    assert_eq!(trigger.source, Driver::object(&g, 0));
    assert_eq!(trigger.controller, PlayerId(0));
    assert_eq!(trigger.source_facts.owner, PlayerId(2));
    assert!(
        matches!(witness(trigger),TriggerEventWitness::DamageDealt {damage,..} if damage.source.object==Driver::object(&g,1) && damage.source.owner==PlayerId(3) && damage.source.controller==PlayerId(0))
    );

    placed(&mut g);

    let count = g.state.rules.libraries[&PlayerId(0)].len();
    resolve(&mut g);
    assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), count - 1);
}
#[test]
fn filtered_damage_creature_type_conjunction_control_and_actual_occurrence_negatives() {
    // Standalone typed predicate probes retain exact source facts. In
    // particular the invocation controller cannot replace the damager's.
    let mut g = Game::new(prepared(&[
        ("Research Thief", 2, 0, Zone::Battlefield),
        ("Alpha Myr", 3, 0, Zone::Battlefield),
        ("Sol Ring", 0, 0, Zone::Battlefield),
        ("Alpha Myr", 0, 1, Zone::Battlefield),
        ("Grizzly Bears", 0, 0, Zone::Battlefield),
    ]));
    let original = g.definition(CardId(0)).unwrap().triggered[0]
        .condition
        .clone();
    for (id, expected) in [(1, true), (2, false), (3, false), (4, false)] {
        let mut actual = ActualDamage {
            id: DamageId(0),
            source: g.object_before(Driver::object(&g, id)).unwrap(),
            controller: PlayerId(1),
            recipient: DamageRecipient::Player {
                player: PlayerId(2),
            },
            amount: 1.into(),
        };
        assert_eq!(
            triggers::damage_matches(
                &original,
                Driver::object(&g, 0),
                PlayerId(0),
                DamageKind::Combat,
                &actual,
                Some(&actual.source)
            ),
            expected
        );
        assert!(!triggers::damage_matches(
            &original,
            Driver::object(&g, 0),
            PlayerId(0),
            DamageKind::Noncombat,
            &actual,
            Some(&actual.source)
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
                    Driver::object(&g, 0),
                    PlayerId(0),
                    DamageKind::Noncombat,
                    &actual,
                    Some(&actual.source)
                ),
                id == 1
            );
        }
        // A permanent pattern never matches an appearance in another zone;
        // the actual damage member retains its independent source facts.
        let battlefield_fact = actual.source.clone();
        actual.source.zone = Zone::Hand {
            owner: actual.source.owner,
        };
        assert!(!triggers::damage_matches(
            &original,
            Driver::object(&g, 0),
            PlayerId(0),
            DamageKind::Combat,
            &actual,
            Some(&actual.source)
        ));
        actual.source = battlefield_fact;
        actual.amount = 0.into();
        assert!(!triggers::damage_matches(
            &original,
            Driver::object(&g, 0),
            PlayerId(0),
            DamageKind::Combat,
            &actual,
            Some(&actual.source)
        ));
    }
    let ring = Driver::object(&g, 2);
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
            ("Research Thief", 2, 0, Zone::Battlefield),
            (
                if enemy { "Alpha Myr" } else { "Grizzly Bears" },
                0,
                u8::from(enemy),
                Zone::Battlefield,
            ),
        ]));
        let turn = if enemy { 2 } else { 1 };
        attack(&mut g, turn, &[(1, if enemy { 0 } else { 1 })]);
        to_step(&mut g, turn, TurnStep::PostcombatMain);
        assert_eq!(g.state.rules.next_trigger_id, 0);
    }
}
#[test]
fn filtered_damage_multiple_observers_and_members_retain_occurrence_multiplicity() {
    let mut g = Game::new(prepared(&[
        ("Research Thief", 2, 0, Zone::Battlefield),
        ("Research Thief", 1, 0, Zone::Battlefield),
        ("Alpha Myr", 3, 0, Zone::Battlefield),
        ("Alpha Myr", 3, 0, Zone::Battlefield),
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

    placed(&mut g);
    let count = g.state.rules.libraries[&PlayerId(0)].len();
    while !g.state.rules.stack.is_empty() {
        resolve(&mut g);
    }
    assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), count - 4);
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

        assert!(
            matches!(&g.top().unwrap().kind,StackKind::Triggered {witness,..} if witness==&frozen)
        );
        let count = g.state.rules.libraries[&PlayerId(0)].len();
        resolve(&mut g);
        assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), count - 1);
    }
}
#[test]
fn filtered_damage_coherent_damager_and_recipient_lies_cannot_authorize_recovery() {
    let mut g = Game::new(prepared(&[
        ("Research Thief", 2, 0, Zone::Battlefield),
        ("Alpha Myr", 3, 0, Zone::Battlefield),
        ("Alpha Myr", 3, 0, Zone::Battlefield),
    ]));
    attack(&mut g, 1, &[(1, 1)]);
    pending(&mut g);

    for lie in ["damager", "recipient"] {
        let mut forged: serde_json::Value = serde_json::from_str(&g.snapshot().unwrap()).unwrap();
        let witness = &mut forged["state"]["pending_triggers"][0]["witness"];
        if lie == "damager" {
            let appearance =
                serde_json::to_value(g.object_before(Driver::object(&g, 2)).unwrap()).unwrap();
            witness["damage"]["source"] = appearance.clone();
            witness["match_source"] = appearance;
        } else {
            witness["damage"]["recipient"]["player"] = 3.into();
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
