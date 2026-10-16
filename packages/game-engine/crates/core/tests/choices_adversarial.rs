//! Independent occurrence, modal, intervening-if and finite-allocation proofs.
//! Pinned CR115.3,601.2b/c,603.4/603.10a,608.2a/b,615.7/616.1.
use mtg_contracts::{Catalog, Color, Quantity};
use mtg_core::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;
const SEEDS: &str = "dd3d4d96-1e88-49e1-8276-7f62c22ad91a";
const SALVE: &str = "8da8644c-75a1-4fe9-8e94-900d948d631c";
const BOUNCER: &str = "82076865-7917-4d3b-8fe8-c21bfcd64d29";
const OX: &str = "19b4cf0b-e1e4-4e50-80bc-673bc6ef8bb8";
const ISAMARU: &str = "cfb756dd-6fe1-430a-8f04-8b10cac62a86";
const AZURE: &str = "bbf9fd47-10e1-4749-b06f-4fe72200afc7";
const BOLT: &str = "4457ed35-7c10-48c8-9776-456485fdf070";
const BEARS: &str = "14c8f55d-d177-4c25-a931-ebeb9e6062a0";
const UNSUMMON: &str = "837182db-1bf3-4a2c-bd01-1af9d9873561";
const LANDS: [&str; 5] = [
    "b34bb2dc-c1af-4d77-b0b3-a0fb342a5fc6",
    "bc71ebf6-2056-41f7-be35-b2e5c34afa99",
    "b2c6aa39-2d2a-459c-a555-fb48ba993373",
    "56719f6a-1a6c-4c0a-8d21-18f7d7350b68",
    "a3fb7228-e76b-4e96-a40e-20b5fed75685",
];
fn prepared(c: ScenarioConfig) -> Prepared {
    Prepared::admit(
        Catalog::read(include_str!(
            "../../../tests/fixtures/expected-choices-ir.json"
        ))
        .unwrap(),
        c,
    )
    .unwrap()
}
fn complete(g: &Game, mut r: QueryResult) -> QueryValue {
    for _ in 0..20000 {
        match r {
            QueryResult::Complete(v) => return v,
            QueryResult::Incomplete { work, .. } => r = g.resume_work(work, Budget::new(1)),
            QueryResult::Rejected(e) => panic!("query {e:?}"),
        }
    }
    panic!("query bound")
}
fn observe(g: &Game, p: PlayerId) -> Observation {
    let QueryValue::Observation(v) = complete(g, g.observe(p, Budget::new(0))) else {
        panic!("observation")
    };
    v
}
fn request(g: &Game) -> (PlayerId, PlayerRequest) {
    g.live_players()
        .into_iter()
        .find_map(
            |p| match complete(g, g.player_decision_for(p, Budget::new(0))) {
                QueryValue::Decision(Some(r)) => Some((p, r)),
                QueryValue::Decision(None) => None,
                _ => panic!("decision"),
            },
        )
        .expect("request")
}
fn verify(g: &Game, p: &Prepared) {
    let r = Game::restore(p.clone(), &g.snapshot().unwrap()).unwrap();
    assert_eq!(r.state_digest().unwrap(), g.state_digest().unwrap());
    assert_eq!(r.trace(), g.trace());
}
fn replay(g: &Game, p: &Prepared) {
    let r = Game::replay(p.clone(), &g.replay_journal()).unwrap();
    assert_eq!(r.state_digest().unwrap(), g.state_digest().unwrap());
}
fn settle(g: &mut Game) -> Advance {
    for _ in 0..3000 {
        let a = g.advance(Budget::new(1));
        if !matches!(a, Advance::Suspended { .. }) {
            return a;
        }
    }
    panic!("advance bound")
}
fn submit(g: &mut Game, response: PlayerResponse) {
    let (actor, r) = request(g);
    let QueryValue::Validated(receipt) = complete(
        g,
        g.check_player_response(actor, r.token().clone(), response, Budget::new(0)),
    ) else {
        panic!("receipt")
    };
    g.submit(receipt).unwrap();
}
fn input(g: &mut Game, p: &Prepared, response: PlayerResponse) {
    submit(g, response);
    assert!(matches!(settle(g), Advance::AwaitingDecision(_)));
    verify(g, p);
}
fn round(g: &mut Game, p: &Prepared) {
    for _ in 0..g.live_players().len() {
        input(g, p, PlayerResponse::Pass);
    }
}
fn reject(g: &Game, actor: PlayerId, token: DecisionToken, response: PlayerResponse) {
    let before = g.snapshot().unwrap();
    let mut q = g.check_player_response(actor, token, response, Budget::new(0));
    for _ in 0..20000 {
        match q {
            QueryResult::Rejected(_) => {
                assert_eq!(before, g.snapshot().unwrap());
                return;
            }
            QueryResult::Incomplete { work, .. } => q = g.resume_work(work, Budget::new(1)),
            QueryResult::Complete(_) => panic!("illegal accepted"),
        }
    }
    panic!("reject bound")
}
fn card(g: &Game, oracle: &str, controller: PlayerId, zone: Zone) -> ObjectHandle {
    let viewer = request(g).0;
    observe(g, viewer)
        .view
        .objects
        .into_iter()
        .find(|o| o.oracle_id == oracle && o.controller == controller && o.zone == zone)
        .expect("card")
        .handle
}
fn body(g: &Game, oracle: &str, controller: PlayerId) -> ObjectHandle {
    card(g, oracle, controller, Zone::Battlefield)
}
fn life(g: &Game, seat: PlayerId) -> Quantity {
    observe(g, g.live_players()[0])
        .view
        .players
        .into_iter()
        .find(|p| p.player == seat)
        .unwrap()
        .life
}
fn begin(g: &mut Game, p: &Prepared, oracle: &str) {
    let actor = request(g).0;
    let card = card(g, oracle, actor, Zone::Hand { owner: actor });
    input(g, p, PlayerResponse::Cast { card });
}
fn pay(g: &mut Game, p: &Prepared, spending: &[(Color, i64)]) {
    let actor = request(g).0;
    loop {
        let source = observe(g, actor).view.objects.into_iter().find(|o| {
            o.zone == Zone::Battlefield
                && o.controller == actor
                && !o.tapped
                && LANDS.contains(&o.oracle_id.as_str())
        });
        let Some(source) = source else { break };
        input(
            g,
            p,
            PlayerResponse::ActivateMana {
                object: source.handle,
            },
        );
    }
    input(g, p, PlayerResponse::FinishMana { undo_mana: true });
    input(
        g,
        p,
        PlayerResponse::SpendMana {
            spending: spending
                .iter()
                .map(|(c, n)| (*c, Quantity::from(*n)))
                .collect::<BTreeMap<_, _>>(),
        },
    );
}
fn salve(g: &mut Game, p: &Prepared, mode: u32, player: PlayerId) {
    begin(g, p, SALVE);
    assert!(matches!(request(g).1, PlayerRequest::Modes { .. }));
    input(g, p, PlayerResponse::Modes { mode });
    input(
        g,
        p,
        if mode == 0 {
            PlayerResponse::PlayerTargets {
                players: vec![player],
            }
        } else {
            PlayerResponse::EntityTargets {
                entities: vec![PlayerTargetEntity::Player { player }],
            }
        },
    );
    pay(g, p, &[(Color::W, 1)]);
    round(g, p);
}
fn seeds(g: &mut Game, p: &Prepared, targets: Vec<ObjectHandle>) {
    begin(g, p, SEEDS);
    input(g, p, PlayerResponse::Targets { objects: targets });
    pay(g, p, &[(Color::G, 1), (Color::W, 1)]);
    round(g, p);
}
fn passive(g: &mut Game, wanted: impl Fn(&Game, &PlayerRequest) -> bool) {
    for _ in 0..400 {
        let r = request(g).1;
        if wanted(g, &r) {
            return;
        }
        let response = match r {
            PlayerRequest::Priority { .. } => PlayerResponse::Pass,
            PlayerRequest::Attackers { .. } => PlayerResponse::Attackers { attacks: vec![] },
            PlayerRequest::Blockers { .. } => PlayerResponse::Blockers { blocks: vec![] },
            _ => panic!("passive {r:?}"),
        };
        submit(g, response);
        assert!(matches!(settle(g), Advance::AwaitingDecision(_)));
    }
    panic!("passive bound")
}
fn place(g: &mut Game, p: &Prepared) {
    for _ in 0..50 {
        let (_, r) = request(g);
        let response = match r {
            PlayerRequest::Priority { .. } => return,
            PlayerRequest::OrderTriggers { candidates, .. } => PlayerResponse::OrderTriggers {
                order: candidates.into_iter().map(|c| c.handle).collect(),
            },
            PlayerRequest::Targets { .. } => PlayerResponse::Targets {
                objects: vec![body(g, BEARS, PlayerId(1))],
            },
            PlayerRequest::PlayerTargets { .. } => PlayerResponse::PlayerTargets {
                players: vec![PlayerId(1)],
            },
            PlayerRequest::CommanderReturn { .. } => {
                PlayerResponse::CommanderReturn { to_command: true }
            }
            _ => panic!("placement {r:?}"),
        };
        input(g, p, response);
    }
    panic!("placement bound")
}
fn resolve(g: &mut Game, p: &Prepared) {
    for _ in 0..300 {
        place(g, p);
        if observe(g, g.live_players()[0]).view.stack.is_empty() {
            return;
        }
        input(g, p, PlayerResponse::Pass);
    }
    panic!("resolve bound")
}
fn checks(g: &Game) -> Vec<(InterveningStage, InterveningCheck)> {
    g.events()
        .iter()
        .filter_map(|e| match e {
            Event::TriggerConditionChecked { stage, check, .. } => {
                Some((stage.clone(), check.clone()))
            }
            _ => None,
        })
        .collect()
}
fn effects(g: &Game) -> Vec<Value> {
    serde_json::to_value(g.events())
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["actual"]["outcome"] == "effect_created")
        .map(|e| e["actual"]["effect"].clone())
        .collect()
}
fn mixed_combat(config: ScenarioConfig) -> (Prepared, Game) {
    let p = prepared(config);
    let mut g = Game::new(p.clone());
    let commander = body(&g, ISAMARU, PlayerId(0));
    let ox = body(&g, OX, PlayerId(0));
    seeds(&mut g, &p, vec![commander, ox.clone(), ox]);
    salve(&mut g, &p, 1, PlayerId(3));
    passive(&mut g, |_, r| matches!(r, PlayerRequest::Attackers { .. }));
    let commander = body(&g, ISAMARU, PlayerId(0));
    let ox = body(&g, OX, PlayerId(0));
    input(
        &mut g,
        &p,
        PlayerResponse::Attackers {
            attacks: vec![(commander, PlayerId(3)), (ox, PlayerId(3))],
        },
    );
    passive(&mut g, |_, r| {
        matches!(
            r,
            PlayerRequest::DamageAllocation { .. } | PlayerRequest::DamageEffect { .. }
        )
    });
    (p, g)
}
fn allocation(g: &Game, commander: i64, ox: i64) -> PlayerResponse {
    let (_, PlayerRequest::DamageAllocation { candidates, .. }) = request(g) else {
        panic!("allocation")
    };
    PlayerResponse::DamageAllocation {
        allocations: candidates
            .into_iter()
            .map(|c| {
                let n = if c.oracle_id == ISAMARU {
                    commander
                } else {
                    assert_eq!(c.oracle_id, OX);
                    ox
                };
                (c.damage, Quantity::from(n))
            })
            .collect(),
    }
}

#[test]
fn seeds_repeated_target_occurrences_create_three_distinct_ordered_effects() {
    let p = prepared(ScenarioConfig::choices());
    let mut g = Game::new(p.clone());
    let ox = body(&g, OX, PlayerId(0));
    seeds(&mut g, &p, vec![ox.clone(), ox.clone(), ox]);
    let e = effects(&g);
    assert_eq!(e.len(), 3);
    for (i, e) in e.iter().enumerate() {
        assert_eq!(e["origin"]["cursor"], json!(i));
        assert_eq!(e["origin"]["source_block"], json!(i));
        assert_eq!(e["kind"]["power"], json!("1"));
    }
    let row = observe(&g, PlayerId(0))
        .view
        .objects
        .into_iter()
        .find(|o| o.oracle_id == OX)
        .unwrap();
    assert_eq!(row.characteristics.unwrap().power, Some(Quantity::from(3)));
    replay(&g, &p);
}

#[test]
fn shared_three_point_vector_expresses_middle_split_and_exact_commander_attribution() {
    let (p, g) = mixed_combat(ScenarioConfig::choices());
    let (
        actor,
        PlayerRequest::DamageAllocation {
            required,
            remaining,
            candidates,
            ..
        },
    ) = request(&g)
    else {
        panic!("allocation")
    };
    assert_eq!(actor, PlayerId(3));
    assert_eq!(required, Quantity::from(3));
    assert_eq!(remaining, Quantity::from(3));
    assert_eq!(candidates.len(), 2);
    for (cmd, ox) in [(3, 0), (2, 1), (1, 2)] {
        let mut b = g.fork();
        let response = allocation(&b, cmd, ox);
        input(&mut b, &p, response);
        assert_eq!(life(&b, PlayerId(3)), Quantity::from(38));
        assert_eq!(
            b.commander_ledger()
                .history(CardId(0))
                .unwrap()
                .combat_damage
                .get(&PlayerId(3))
                .cloned()
                .unwrap_or_default(),
            Quantity::from(3 - cmd)
        );
        replay(&b, &p);
    }
}

#[test]
fn allocation_rejects_non_total_duplicate_missing_over_source_and_foreign_inputs() {
    let (p, mut g) = mixed_combat(ScenarioConfig::choices());
    let (actor, r) = request(&g);
    let PlayerRequest::DamageAllocation { candidates, .. } = &r else {
        panic!("allocation")
    };
    let valid = allocation(&g, 2, 1);
    for (cmd, ox) in [(1, 1), (3, 1), (-1, 4), (0, 3)] {
        reject(&g, actor, r.token().clone(), allocation(&g, cmd, ox));
    }
    reject(
        &g,
        actor,
        r.token().clone(),
        PlayerResponse::DamageAllocation {
            allocations: vec![],
        },
    );
    reject(
        &g,
        actor,
        r.token().clone(),
        PlayerResponse::DamageAllocation {
            allocations: vec![
                (candidates[0].damage.clone(), Quantity::from(1)),
                (candidates[0].damage.clone(), Quantity::from(2)),
            ],
        },
    );
    reject(&g, PlayerId(0), r.token().clone(), valid.clone());
    let before = g.snapshot().unwrap();
    let view = observe(&g, actor);
    assert_eq!(view, observe(&g, actor));
    assert_eq!(before, g.snapshot().unwrap());
    let QueryResult::Incomplete { work, .. } =
        g.check_player_response(actor, r.token().clone(), valid, Budget::new(0))
    else {
        panic!("work")
    };
    let saved = g.save_work(&work).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&saved).unwrap()["work_version"],
        14
    );
    let mut b = Game::restore(p.clone(), &before).unwrap();
    let work = b.restore_work(&saved).unwrap();
    let QueryValue::Validated(receipt) = complete(&b, b.resume_work(work, Budget::new(1))) else {
        panic!("receipt")
    };
    assert!(
        g.submit(receipt).is_err(),
        "receipt belongs to restored executor"
    );
    assert_eq!(before, g.snapshot().unwrap());
    let work = b.restore_work(&saved).unwrap();
    let QueryValue::Validated(receipt) = complete(&b, b.resume_work(work, Budget::new(1))) else {
        panic!("local receipt")
    };
    b.submit(receipt).unwrap();
    assert!(matches!(settle(&mut b), Advance::AwaitingDecision(_)));
    reject(&b, actor, r.token().clone(), allocation(&g, 2, 1));
    replay(&b, &p);
}

#[test]
fn accepted_allocation_recipient_departure_cancels_without_consuming_unapplied_shield() {
    let (p, g) = mixed_combat(ScenarioConfig::choices());
    for accepted in [false, true] {
        let mut b = g.fork();
        if accepted {
            let response = allocation(&b, 2, 1);
            submit(&mut b, response);
        }
        b.concede(PlayerId(3)).unwrap();
        assert!(matches!(settle(&mut b), Advance::AwaitingDecision(_)));
        let shield = observe(&b, PlayerId(0))
            .view
            .effects
            .into_iter()
            .find_map(|e| match e {
                VisibleEffect::PreventNextDamage { remaining, .. } => Some(remaining),
                _ => None,
            })
            .unwrap();
        assert_eq!(shield, Quantity::from(3));
        assert!(
            b.commander_ledger()
                .history(CardId(0))
                .unwrap()
                .combat_damage
                .is_empty()
        );
        verify(&b, &p);
        replay(&b, &p);
    }
}

#[test]
fn salve_mode_is_announced_before_target_and_bound_effect_survives_source_departure() {
    let p = prepared(ScenarioConfig::choices());
    let mut g = Game::new(p.clone());
    begin(&mut g, &p, SALVE);
    let (actor, r) = request(&g);
    assert!(matches!(&r,PlayerRequest::Modes{candidates,..} if candidates==&vec![0,1]));
    reject(
        &g,
        actor,
        r.token().clone(),
        PlayerResponse::Modes { mode: 2 },
    );
    reject(
        &g,
        actor,
        r.token().clone(),
        PlayerResponse::Targets { objects: vec![] },
    );
    for mode in [0, 1] {
        let mut b = g.fork();
        input(&mut b, &p, PlayerResponse::Modes { mode });
        if mode == 0 {
            assert!(matches!(request(&b).1, PlayerRequest::PlayerTargets { .. }));
            input(
                &mut b,
                &p,
                PlayerResponse::PlayerTargets {
                    players: vec![PlayerId(0)],
                },
            );
        } else {
            assert!(matches!(request(&b).1, PlayerRequest::EntityTargets { .. }));
            input(
                &mut b,
                &p,
                PlayerResponse::EntityTargets {
                    entities: vec![PlayerTargetEntity::Player {
                        player: PlayerId(1),
                    }],
                },
            );
        }
        pay(&mut b, &p, &[(Color::W, 1)]);
        round(&mut b, &p);
        if mode == 0 {
            assert_eq!(life(&b, PlayerId(0)), Quantity::from(43));
            assert!(effects(&b).is_empty());
        } else {
            let created = effects(&b);
            assert_eq!(created.len(), 1);
            assert_eq!(created[0]["origin"]["mode"], json!(1));
            let mut forged: Value = serde_json::from_str(&b.snapshot().unwrap()).unwrap();
            forged["state"]["effects"][0]["origin"]["mode"] = json!(0);
            forged["state_sha256"] = json!(mtg_contracts::digest(&forged["state"]).unwrap());
            assert!(Game::restore(p.clone(), &serde_json::to_string(&forged).unwrap()).is_err());
            b.concede(PlayerId(0)).unwrap();
            assert!(matches!(settle(&mut b), Advance::AwaitingDecision(_)));
            assert_eq!(request(&b).0, PlayerId(1));
            begin(&mut b, &p, BOLT);
            input(
                &mut b,
                &p,
                PlayerResponse::EntityTargets {
                    entities: vec![PlayerTargetEntity::Player {
                        player: PlayerId(1),
                    }],
                },
            );
            pay(&mut b, &p, &[(Color::R, 1)]);
            round(&mut b, &p);
            assert_eq!(life(&b, PlayerId(1)), Quantity::from(40));
        }
        verify(&b, &p);
        replay(&b, &p);
    }
}

#[test]
fn accepted_mode_survives_unrelated_departure_but_caster_departure_removes_proposal() {
    let p = prepared(ScenarioConfig::choices());
    let mut g = Game::new(p.clone());
    begin(&mut g, &p, SALVE);
    for seat in [PlayerId(2), PlayerId(0)] {
        let mut b = g.fork();
        submit(&mut b, PlayerResponse::Modes { mode: 1 });
        b.concede(seat).unwrap();
        assert!(matches!(settle(&mut b), Advance::AwaitingDecision(_)));
        if seat == PlayerId(2) {
            assert_eq!(request(&b).0, PlayerId(0));
            assert!(matches!(request(&b).1, PlayerRequest::EntityTargets { .. }));
        } else {
            assert!(observe(&b, PlayerId(1)).view.stack.is_empty());
            assert!(effects(&b).is_empty());
        }
        verify(&b, &p);
        replay(&b, &p);
    }
}

fn kill_bouncer(config: ScenarioConfig) -> (Prepared, Game) {
    let p = prepared(config);
    let mut g = Game::new(p.clone());
    input(&mut g, &p, PlayerResponse::Pass);
    assert_eq!(request(&g).0, PlayerId(1));
    begin(&mut g, &p, BOLT);
    let object = body(&g, BOUNCER, PlayerId(0));
    input(
        &mut g,
        &p,
        PlayerResponse::EntityTargets {
            entities: vec![PlayerTargetEntity::Object { object }],
        },
    );
    pay(&mut g, &p, &[(Color::R, 1)]);
    round(&mut g, &p);
    (p, g)
}
fn mage_draw(g: &mut Game, p: &Prepared) {
    assert_eq!(request(g).0, PlayerId(0));
    let object = body(g, AZURE, PlayerId(0));
    input(g, p, PlayerResponse::ActivateAbility { object, ability: 0 });
    pay(g, p, &[(Color::U, 1), (Color::G, 2), (Color::B, 1)]);
    round(g, p);
}
#[test]
fn intervening_condition_rechecks_once_at_start_and_allows_true_false_true() {
    let (p, mut g) = kill_bouncer(ScenarioConfig::conditions());
    let first = checks(&g);
    assert_eq!(first.len(), 1);
    assert!(matches!(first[0].0, InterveningStage::Match { .. }));
    assert_eq!(first[0].1.controller, PlayerId(0));
    assert_eq!(first[0].1.hand_count, 0);
    assert!(first[0].1.passed);
    place(&mut g, &p);
    mage_draw(&mut g, &p);
    assert_eq!(
        observe(&g, PlayerId(0))
            .view
            .players
            .iter()
            .find(|p| p.player == PlayerId(0))
            .unwrap()
            .hand_count,
        1
    );
    assert_eq!(
        checks(&g).len(),
        1,
        "hand change is not a continuous condition watcher"
    );
    for cast in [false, true] {
        let mut b = g.fork();
        if cast {
            salve(&mut b, &p, 0, PlayerId(0));
        }
        resolve(&mut b, &p);
        let all = checks(&b);
        assert_eq!(all.len(), 2);
        assert!(matches!(all[1].0, InterveningStage::ResolutionStart { .. }));
        assert_eq!(all[1].1.passed, cast);
        assert_eq!(all[1].1.hand_count, if cast { 0 } else { 1 });
        assert_eq!(effects(&b).len(), usize::from(cast));
        assert_eq!(
            life(&b, PlayerId(1)),
            Quantity::from(if cast { 36 } else { 38 })
        );
        verify(&b, &p);
        replay(&b, &p);
    }
}

#[test]
fn false_match_never_creates_a_trigger_even_if_hand_empties_later() {
    let (p, mut g) = kill_bouncer(ScenarioConfig::conditions_nonempty());
    let first = checks(&g);
    assert_eq!(first.len(), 1);
    assert!(!first[0].1.passed);
    assert_eq!(first[0].1.hand_count, 1);
    resolve(&mut g, &p);
    salve(&mut g, &p, 0, PlayerId(0));
    assert_eq!(checks(&g).len(), 1);
    assert!(effects(&g).is_empty());
    assert!(
        !g.events()
            .iter()
            .any(|e| matches!(e,Event::TriggerCollected{source,..} if source.card==CardId(2)))
    );
    verify(&g, &p);
    replay(&g, &p);
}

#[test]
fn borrowed_bouncer_uses_controller_hand_and_survives_owner_departure() {
    let (p, mut g) = kill_bouncer(ScenarioConfig::conditions_borrowed());
    assert!(
        observe(&g, PlayerId(0))
            .view
            .players
            .iter()
            .find(|p| p.player == PlayerId(2))
            .unwrap()
            .hand_count
            > 0
    );
    assert_eq!(checks(&g)[0].1.controller, PlayerId(0));
    assert!(checks(&g)[0].1.passed);
    g.concede(PlayerId(2)).unwrap();
    assert!(matches!(settle(&mut g), Advance::AwaitingDecision(_)));
    assert!(g.object_id(CardId(2)).is_none());
    place(&mut g, &p);
    resolve(&mut g, &p);
    let all = checks(&g);
    assert_eq!(all.len(), 2);
    assert!(all[1].1.passed);
    assert_eq!(all[1].1.controller, PlayerId(0));
    let e = effects(&g);
    assert_eq!(e.len(), 1);
    assert_eq!(e[0]["origin"]["source"]["card"], json!(2));
    assert_eq!(life(&g, PlayerId(1)), Quantity::from(38));
    verify(&g, &p);
    replay(&g, &p);
}

#[test]
fn mandatory_bouncer_without_any_legal_creature_is_removed_without_hanging() {
    let (p, g) = kill_bouncer(ScenarioConfig::conditions_no_targets());
    assert!(matches!(request(&g).1, PlayerRequest::Priority { .. }));
    assert!(observe(&g, PlayerId(0)).view.stack.is_empty());
    let c = checks(&g);
    assert_eq!(c.len(), 1);
    assert!(c[0].1.passed);
    assert_eq!(
        g.events()
            .iter()
            .filter(|e| matches!(
                e,
                Event::TriggeredAbilityNotPlaced {
                    reason: TriggerPlacementFailure::NoLegalTargets,
                    ..
                }
            ))
            .count(),
        1
    );
    assert!(effects(&g).is_empty());
    verify(&g, &p);
    replay(&g, &p);
}

#[test]
fn seeds_partial_or_all_illegal_occurrences_never_rebind_bounced_successor() {
    for partial in [false, true] {
        let p = prepared(ScenarioConfig::choices());
        let mut g = Game::new(p.clone());
        let a = body(&g, BEARS, PlayerId(1));
        let b = body(&g, OX, PlayerId(0));
        begin(&mut g, &p, SEEDS);
        input(
            &mut g,
            &p,
            PlayerResponse::Targets {
                objects: vec![a.clone(), a.clone(), if partial { b } else { a.clone() }],
            },
        );
        pay(&mut g, &p, &[(Color::G, 1), (Color::W, 1)]);
        input(&mut g, &p, PlayerResponse::Pass);
        assert_eq!(request(&g).0, PlayerId(1));
        begin(&mut g, &p, UNSUMMON);
        let own_a = body(&g, BEARS, PlayerId(1));
        input(
            &mut g,
            &p,
            PlayerResponse::Targets {
                objects: vec![own_a],
            },
        );
        pay(&mut g, &p, &[(Color::U, 1)]);
        round(&mut g, &p);
        round(&mut g, &p);
        let e = effects(&g);
        assert_eq!(e.len(), usize::from(partial));
        if partial {
            assert_eq!(e[0]["origin"]["cursor"], json!(2));
            assert_eq!(e[0]["origin"]["source_block"], json!(2));
        }
        let mut completions = g.events().iter().filter_map(|e| match e {
            Event::SpellCompleted { resolved, .. } => Some(*resolved),
            _ => None,
        });
        assert_eq!(completions.next(), Some(true));
        assert_eq!(completions.next(), Some(partial));
        verify(&g, &p);
        replay(&g, &p);
    }
}

#[test]
fn allocation_preserves_frozen_assigned_sources_when_their_controller_departs() {
    let (p, g) = mixed_combat(ScenarioConfig::choices());
    for accepted in [false, true] {
        let mut b = g.fork();
        let response = allocation(&b, 2, 1);
        if accepted {
            submit(&mut b, response.clone());
        }
        b.concede(PlayerId(0)).unwrap();
        assert!(matches!(settle(&mut b), Advance::AwaitingDecision(_)));
        if !accepted {
            let (_, PlayerRequest::DamageAllocation { candidates, .. }) = request(&b) else {
                panic!("allocation survives source departure")
            };
            assert!(candidates.iter().all(|c| c.source.current.is_none()));
            input(&mut b, &p, response);
        }
        assert_eq!(life(&b, PlayerId(3)), Quantity::from(38));
        assert_eq!(
            b.commander_ledger()
                .history(CardId(0))
                .unwrap()
                .combat_damage[&PlayerId(3)],
            Quantity::from(1)
        );
        verify(&b, &p);
        replay(&b, &p);
    }
}

#[test]
fn allocation_after_provider_departure_keeps_earlier_double_and_uses_transformed_amounts() {
    let (p, mut g) = mixed_combat(ScenarioConfig::choices_furnace());
    let (_, PlayerRequest::DamageEffect { candidates, .. }) = request(&g) else {
        panic!("effect order")
    };
    let c = candidates
        .into_iter()
        .find(|c| c.kind == DamageEffectKind::DoubleDamage && c.damage_oracle_id == ISAMARU)
        .unwrap();
    input(
        &mut g,
        &p,
        PlayerResponse::DamageEffect {
            effect: c.effect,
            damage: c.damage,
        },
    );
    let (_, PlayerRequest::DamageEffect { candidates, .. }) = request(&g) else {
        panic!("shield order")
    };
    let c = candidates
        .into_iter()
        .find(|c| c.kind == DamageEffectKind::PreventNextDamage && c.allocation_required)
        .unwrap();
    assert_eq!(
        c.amount_before, c.amount_after,
        "anchor chooses a shield group, not source consumption"
    );
    input(
        &mut g,
        &p,
        PlayerResponse::DamageEffect {
            effect: c.effect,
            damage: c.damage,
        },
    );
    let (_, PlayerRequest::DamageAllocation { candidates, .. }) = request(&g) else {
        panic!("allocation")
    };
    assert_eq!(
        candidates
            .iter()
            .find(|c| c.oracle_id == ISAMARU)
            .unwrap()
            .amount,
        Quantity::from(6)
    );
    for accepted in [false, true] {
        let mut b = g.fork();
        let response = allocation(&b, 2, 1);
        if accepted {
            submit(&mut b, response.clone());
        }
        b.concede(PlayerId(2)).unwrap();
        assert!(matches!(settle(&mut b), Advance::AwaitingDecision(_)));
        if !accepted {
            input(&mut b, &p, response);
        }
        assert_eq!(life(&b, PlayerId(3)), Quantity::from(35));
        assert_eq!(
            b.commander_ledger()
                .history(CardId(0))
                .unwrap()
                .combat_damage[&PlayerId(3)],
            Quantity::from(4)
        );
        verify(&b, &p);
        replay(&b, &p);
    }
}

#[test]
fn rehashed_allocation_vectors_prefix_ranges_and_budgets_cannot_forge_restore() {
    let (p, mut g) = mixed_combat(ScenarioConfig::choices());
    let response = allocation(&g, 2, 1);
    submit(&mut g, response);
    let mut snap = None;
    for _ in 0..50 {
        let _ = g.advance(Budget::new(1));
        let value: Value = serde_json::from_str(&g.snapshot().unwrap()).unwrap();
        if value["state"]["frame"]["transaction"]["allocation_groups"]
            .as_array()
            .is_some_and(|v| !v.is_empty())
        {
            snap = Some(value);
            break;
        }
    }
    let original = snap.expect("durable applied vector before simultaneous damage commit");
    verify(&g, &p);
    for change in 0..5 {
        let mut forged = original.clone();
        let tx = &mut forged["state"]["frame"]["transaction"];
        match change {
            0 => tx["allocation_groups"][0]["allocations"][0][1] = json!("1"),
            1 => tx["allocation_groups"][0]["application_start"] = json!(1),
            2 => tx["allocation_groups"][0]["budget_before"] = json!("4"),
            3 => tx["applications"][0]["before"] = json!("99"),
            _ => tx["entries"][0]["source_handles"][0] = json!("v0-o99999"),
        }
        forged["state_sha256"] = json!(mtg_contracts::digest(&forged["state"]).unwrap());
        assert!(
            Game::restore(p.clone(), &serde_json::to_string(&forged).unwrap()).is_err(),
            "forgery {change}"
        );
    }
    assert!(matches!(settle(&mut g), Advance::AwaitingDecision(_)));
    verify(&g, &p);
    replay(&g, &p);
}

#[test]
fn started_conditional_invocation_retains_second_check_after_controller_departure() {
    let (p, mut g) = kill_bouncer(ScenarioConfig::conditions());
    place(&mut g, &p);
    while observe(&g, PlayerId(0)).view.stack.len() > 1 {
        round(&mut g, &p);
    }
    for _ in 0..3 {
        input(&mut g, &p, PlayerResponse::Pass);
    }
    submit(&mut g, PlayerResponse::Pass);
    let mut original = None;
    for _ in 0..20 {
        let _ = g.advance(Budget::new(1));
        let v: Value = serde_json::from_str(&g.snapshot().unwrap()).unwrap();
        if v["state"]["frame"]["frame"]["condition_check"].is_object() {
            original = Some(v);
            break;
        }
    }
    let original = original.expect("resolution starts with cached second check");
    assert_eq!(
        original["state"]["frame"]["frame"]["condition_check"]["hand_count"],
        json!(0)
    );
    let mut forged = original.clone();
    forged["state"]["frame"]["frame"]["condition_check"]["hand_count"] = json!(1);
    forged["state"]["frame"]["frame"]["condition_check"]["passed"] = json!(false);
    forged["state_sha256"] = json!(mtg_contracts::digest(&forged["state"]).unwrap());
    assert!(Game::restore(p.clone(), &serde_json::to_string(&forged).unwrap()).is_err());
    verify(&g, &p);
    g.concede(PlayerId(0)).unwrap();
    assert!(matches!(settle(&mut g), Advance::AwaitingDecision(_)));
    place(&mut g, &p);
    resolve(&mut g, &p);
    assert_eq!(
        checks(&g).len(),
        2,
        "second check is not repeated after departure"
    );
    assert_eq!(
        effects(&g).len(),
        1,
        "started program applies its already checked condition"
    );
    assert_eq!(life(&g, PlayerId(1)), Quantity::from(36));
    verify(&g, &p);
    replay(&g, &p);
}
