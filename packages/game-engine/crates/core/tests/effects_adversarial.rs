//! Independent adversarial proofs for the frozen finite effects/damage family.
//! Rules: pinned 2026-09-25 CR514.2, 611.2a/c, 614.5, 615.7, 616.1f.
//! Tests consume player projections; snapshots/events are host verification only.
use mtg_contracts::{Catalog, Color, Quantity};
use mtg_core::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;

const ISAMARU: &str = "cfb756dd-6fe1-430a-8f04-8b10cac62a86";
const OX: &str = "19b4cf0b-e1e4-4e50-80bc-673bc6ef8bb8";
const HEALER: &str = "95a0ca48-d924-47f4-86ed-42c673ee778c";
const SORCERER: &str = "5e961d15-5972-4e4b-9385-1cd7cd7c6bbe";
const ARTIST: &str = "310f141c-7f37-4729-aed6-dd9c09db448d";
const GROWTH: &str = "5748ebf1-24e3-499d-ab7c-c2cebd462a24";
const DISFIGURE: &str = "77eafe49-b9c5-461d-89c5-cec217dd2974";
const BOLT: &str = "4457ed35-7c10-48c8-9776-456485fdf070";
const FOREST: &str = "b34bb2dc-c1af-4d77-b0b3-a0fb342a5fc6";
const MOUNTAIN: &str = "a3fb7228-e76b-4e96-a40e-20b5fed75685";
const SWAMP: &str = "56719f6a-1a6c-4c0a-8d21-18f7d7350b68";

fn prepared(config: ScenarioConfig) -> Prepared {
    Prepared::admit(
        Catalog::read(include_str!(
            "../../../tests/fixtures/expected-effects-ir.json"
        ))
        .unwrap(),
        config,
    )
    .unwrap()
}
fn complete(g: &Game, mut r: QueryResult) -> QueryValue {
    for _ in 0..20_000 {
        match r {
            QueryResult::Complete(v) => return v,
            QueryResult::Incomplete { work, .. } => r = g.resume_work(work, Budget::new(1)),
            QueryResult::Rejected(e) => panic!("query rejected {e:?}"),
        }
    }
    panic!("query budget bound")
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
                _ => panic!("decision type"),
            },
        )
        .expect("missing decision")
}
fn verify(g: &Game, p: &Prepared) {
    let restored = Game::restore(p.clone(), &g.snapshot().unwrap()).unwrap();
    assert_eq!(g.state_digest().unwrap(), restored.state_digest().unwrap());
    assert_eq!(g.trace(), restored.trace());
}
fn replay_equal(g: &Game, p: &Prepared) {
    let replay = Game::replay(p.clone(), &g.replay_journal()).unwrap();
    assert_eq!(g.state_digest().unwrap(), replay.state_digest().unwrap());
}
fn settle(g: &mut Game) -> Advance {
    for _ in 0..2000 {
        let a = g.advance(Budget::new(1));
        if !matches!(a, Advance::Suspended { .. }) {
            return a;
        }
    }
    panic!("automatic budget bound")
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
fn handle(
    g: &Game,
    viewer: PlayerId,
    oracle: &str,
    controller: PlayerId,
    zone: Zone,
) -> ObjectHandle {
    observe(g, viewer)
        .view
        .objects
        .into_iter()
        .find(|o| o.oracle_id == oracle && o.controller == controller && o.zone == zone)
        .expect("visible card")
        .handle
}
fn battlefield(g: &Game, oracle: &str, controller: PlayerId) -> ObjectHandle {
    handle(g, request(g).0, oracle, controller, Zone::Battlefield)
}
fn life(g: &Game, player: PlayerId) -> Quantity {
    observe(g, g.live_players()[0])
        .view
        .players
        .into_iter()
        .find(|p| p.player == player)
        .unwrap()
        .life
}
fn begin_spell(g: &mut Game, p: &Prepared, oracle: &str) {
    let actor = request(g).0;
    let card = handle(g, actor, oracle, actor, Zone::Hand { owner: actor });
    input(g, p, PlayerResponse::Cast { card });
}
fn pay(g: &mut Game, p: &Prepared, color: Color, land: &str) {
    let actor = request(g).0;
    let object = handle(g, actor, land, actor, Zone::Battlefield);
    input(g, p, PlayerResponse::ActivateMana { object });
    input(g, p, PlayerResponse::FinishMana { undo_mana: true });
    input(
        g,
        p,
        PlayerResponse::SpendMana {
            spending: BTreeMap::from([(color, Quantity::from(1))]),
        },
    );
}
fn pump(g: &mut Game, p: &Prepared, target: &str) {
    begin_spell(g, p, GROWTH);
    let object = battlefield(g, target, PlayerId(0));
    input(
        g,
        p,
        PlayerResponse::Targets {
            objects: vec![object],
        },
    );
    pay(g, p, Color::G, FOREST);
    round(g, p);
}
fn activate(g: &mut Game, p: &Prepared, source: &str, target: PlayerTargetEntity) {
    let object = battlefield(g, source, PlayerId(0));
    input(g, p, PlayerResponse::ActivateAbility { object, ability: 0 });
    input(
        g,
        p,
        PlayerResponse::EntityTargets {
            entities: vec![target],
        },
    );
}
fn shield(g: &mut Game, p: &Prepared, target: PlayerId) {
    activate(g, p, HEALER, PlayerTargetEntity::Player { player: target });
    round(g, p);
}
fn bolt(g: &mut Game, p: &Prepared, target: PlayerTargetEntity) {
    begin_spell(g, p, BOLT);
    input(
        g,
        p,
        PlayerResponse::EntityTargets {
            entities: vec![target],
        },
    );
    pay(g, p, Color::R, MOUNTAIN);
    round(g, p);
}
fn passive_until(g: &mut Game, wanted: impl Fn(&Game, &PlayerRequest) -> bool) {
    for _ in 0..400 {
        let req = request(g).1;
        if wanted(g, &req) {
            return;
        }
        let response = match req {
            PlayerRequest::Priority { .. } => PlayerResponse::Pass,
            PlayerRequest::Attackers { .. } => PlayerResponse::Attackers { attacks: vec![] },
            PlayerRequest::Blockers { .. } => PlayerResponse::Blockers { blocks: vec![] },
            _ => panic!("unexpected passive request {req:?}"),
        };
        submit(g, response);
        assert!(matches!(settle(g), Advance::AwaitingDecision(_)));
    }
    panic!("passive bound")
}
fn applications(g: &Game) -> Vec<Value> {
    serde_json::to_value(g.events())
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["actual"]["outcome"] == "damage_effect_applied")
        .cloned()
        .collect()
}
fn committed(g: &Game) -> Vec<Value> {
    serde_json::to_value(g.events())
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["actual"]["outcome"] == "damage_committed")
        .cloned()
        .collect()
}
fn choose(g: &Game, kind: DamageEffectKind) -> PlayerResponse {
    let (_, PlayerRequest::DamageEffect { candidates, .. }) = request(g) else {
        panic!("damage effect request")
    };
    let c = candidates
        .into_iter()
        .find(|c| c.kind == kind)
        .expect("offered effect");
    PlayerResponse::DamageEffect {
        effect: c.effect,
        damage: c.damage,
    }
}
fn advance_one_application(g: &mut Game, p: &Prepared) {
    let before = applications(g).len();
    for _ in 0..50 {
        let a = g.advance(Budget::new(1));
        if applications(g).len() > before {
            assert_eq!(applications(g).len(), before + 1);
            verify(g, p);
            return;
        }
        assert!(matches!(a, Advance::Suspended { .. }));
    }
    panic!("application not reached")
}
fn bolt_into_shield_and_furnace() -> (Prepared, Game) {
    let p = prepared(ScenarioConfig::effects_furnace());
    let mut g = Game::new(p.clone());
    shield(&mut g, &p, PlayerId(1));
    bolt(
        &mut g,
        &p,
        PlayerTargetEntity::Player {
            player: PlayerId(1),
        },
    );
    assert_eq!(request(&g).0, PlayerId(1));
    assert_eq!(life(&g, PlayerId(1)), Quantity::from(40));
    (p, g)
}

#[test]
fn accepted_provider_choice_revalidates_but_applied_double_survives_provider_departure() {
    let (p, g) = bolt_into_shield_and_furnace();
    for applied in [false, true] {
        let mut branch = g.fork();
        let response = choose(&branch, DamageEffectKind::DoubleDamage);
        submit(&mut branch, response);
        if applied {
            advance_one_application(&mut branch, &p);
            assert_eq!(life(&branch, PlayerId(1)), Quantity::from(40));
        }
        branch.concede(PlayerId(2)).unwrap();
        assert!(matches!(settle(&mut branch), Advance::AwaitingDecision(_)));
        assert_eq!(
            life(&branch, PlayerId(1)),
            Quantity::from(if applied { 35 } else { 38 })
        );
        assert_eq!(applications(&branch).len(), if applied { 2 } else { 1 });
        assert_eq!(committed(&branch).len(), 1);
        verify(&branch, &p);
        replay_equal(&branch, &p);
    }
}

#[test]
fn applied_shield_consumption_is_not_refunded_when_recipient_departs_before_damage() {
    let (p, mut g) = bolt_into_shield_and_furnace();
    let response = choose(&g, DamageEffectKind::PreventNextDamage);
    submit(&mut g, response);
    advance_one_application(&mut g, &p);
    assert_eq!(life(&g, PlayerId(1)), Quantity::from(40));
    let prefix = applications(&g);
    assert_eq!(prefix.len(), 1);
    g.concede(PlayerId(1)).unwrap();
    assert!(matches!(settle(&mut g), Advance::AwaitingDecision(_)));
    assert_eq!(applications(&g), prefix);
    assert_eq!(life(&g, PlayerId(0)), Quantity::from(40));
    assert_eq!(life(&g, PlayerId(3)), Quantity::from(40));
    let damage = committed(&g);
    assert_eq!(damage.len(), 1);
    assert_eq!(damage[0]["actual"]["actual"], json!([]));
    verify(&g, &p);
    replay_equal(&g, &p);
}

#[test]
fn fixed_one_damage_retains_pumped_source_lki_after_borrowed_owner_departure() {
    let p = prepared(ScenarioConfig::effects_borrowed());
    let mut g = Game::new(p.clone());
    pump(&mut g, &p, SORCERER);
    let source = g.object_id(CardId(3)).unwrap();
    let row = observe(&g, PlayerId(0))
        .view
        .objects
        .into_iter()
        .find(|o| o.oracle_id == SORCERER && o.controller == PlayerId(0))
        .unwrap();
    let facts = serde_json::to_value(row).unwrap();
    assert_eq!(facts["characteristics"]["power"], json!("4"));
    assert_eq!(facts["characteristics"]["toughness"], json!("4"));
    activate(
        &mut g,
        &p,
        SORCERER,
        PlayerTargetEntity::Player {
            player: PlayerId(1),
        },
    );
    g.concede(PlayerId(2)).unwrap();
    assert!(matches!(settle(&mut g), Advance::AwaitingDecision(_)));
    assert!(g.object_id(CardId(3)).is_none());
    assert_eq!(
        observe(&g, PlayerId(0))
            .view
            .stack
            .last()
            .unwrap()
            .ability
            .as_ref()
            .unwrap()
            .source
            .current,
        None
    );
    round(&mut g, &p);
    assert_eq!(life(&g, PlayerId(1)), Quantity::from(39));
    let commits = committed(&g);
    assert_eq!(commits.len(), 1);
    let actual = &commits[0]["actual"]["actual"][0];
    assert_eq!(
        actual["source"]["object"],
        serde_json::to_value(source).unwrap()
    );
    assert_eq!(actual["source"]["characteristics"]["power"], json!("4"));
    assert_eq!(actual["amount"], json!("1"));
    verify(&g, &p);
    replay_equal(&g, &p);
}

#[test]
fn cleanup_expires_pump_and_clears_damage_together_without_false_artist_death() {
    let p = prepared(ScenarioConfig::effects());
    let mut g = Game::new(p.clone());
    pump(&mut g, &p, ARTIST);
    let target = battlefield(&g, ARTIST, PlayerId(0));
    bolt(&mut g, &p, PlayerTargetEntity::Object { object: target });
    let source = g.object_id(CardId(4)).unwrap();
    assert_eq!(g.zone(source), Some(Zone::Battlefield));
    assert_eq!(
        observe(&g, PlayerId(0))
            .view
            .objects
            .into_iter()
            .find(|o| o.oracle_id == ARTIST && o.controller == PlayerId(0))
            .unwrap()
            .marked_damage,
        Quantity::from(3)
    );
    passive_until(&mut g, |g, r| {
        matches!(r, PlayerRequest::Priority { .. })
            && g.turn().number == 2
            && g.turn().step == TurnStep::Upkeep
    });
    assert_eq!(g.object_id(CardId(4)), Some(source));
    let row = observe(&g, PlayerId(0))
        .view
        .objects
        .into_iter()
        .find(|o| o.oracle_id == ARTIST && o.controller == PlayerId(0))
        .unwrap();
    assert_eq!(row.marked_damage, Quantity::zero());
    assert_eq!(
        serde_json::to_value(row).unwrap()["characteristics"]["toughness"],
        json!("1")
    );
    assert!(
        !g.events()
            .iter()
            .any(|e| matches!(e, Event::TriggerCollected { .. }))
    );
    verify(&g, &p);
    replay_equal(&g, &p);
}

#[test]
fn surviving_negative_power_ox_assigns_no_damage_even_with_static_furnace() {
    let p = prepared(ScenarioConfig::effects_furnace());
    let mut g = Game::new(p.clone());
    begin_spell(&mut g, &p, DISFIGURE);
    let object = battlefield(&g, OX, PlayerId(0));
    input(
        &mut g,
        &p,
        PlayerResponse::Targets {
            objects: vec![object],
        },
    );
    pay(&mut g, &p, Color::B, SWAMP);
    round(&mut g, &p);
    let row = observe(&g, PlayerId(0))
        .view
        .objects
        .into_iter()
        .find(|o| o.oracle_id == OX)
        .unwrap();
    assert_eq!(
        serde_json::to_value(&row).unwrap()["characteristics"]["power"],
        json!("-2")
    );
    passive_until(&mut g, |_, r| matches!(r, PlayerRequest::Attackers { .. }));
    let attacker = battlefield(&g, OX, PlayerId(0));
    input(
        &mut g,
        &p,
        PlayerResponse::Attackers {
            attacks: vec![(attacker, PlayerId(3))],
        },
    );
    passive_until(&mut g, |g, r| {
        matches!(r, PlayerRequest::Priority { .. }) && g.turn().step == TurnStep::PostcombatMain
    });
    assert_eq!(life(&g, PlayerId(3)), Quantity::from(40));
    assert_eq!(
        g.zone(g.object_id(CardId(1)).unwrap()),
        Some(Zone::Battlefield)
    );
    assert!(applications(&g).is_empty());
    verify(&g, &p);
    replay_equal(&g, &p);
}

#[test]
fn one_shared_point_is_allocated_to_selected_simultaneous_source_and_commander_history() {
    let p = prepared(ScenarioConfig::effects());
    let mut g = Game::new(p.clone());
    shield(&mut g, &p, PlayerId(3));
    pump(&mut g, &p, OX);
    passive_until(&mut g, |_, r| matches!(r, PlayerRequest::Attackers { .. }));
    let commander = battlefield(&g, ISAMARU, PlayerId(0));
    let ox = battlefield(&g, OX, PlayerId(0));
    input(
        &mut g,
        &p,
        PlayerResponse::Attackers {
            attacks: vec![(commander, PlayerId(3)), (ox, PlayerId(3))],
        },
    );
    passive_until(&mut g, |_, r| {
        matches!(r, PlayerRequest::DamageEffect { .. })
    });
    let (chooser, PlayerRequest::DamageEffect { candidates, .. }) = request(&g) else {
        panic!("allocation choice")
    };
    assert_eq!(chooser, PlayerId(3));
    assert_eq!(candidates.len(), 2);
    assert!(
        candidates
            .iter()
            .all(|c| c.kind == DamageEffectKind::PreventNextDamage)
    );
    assert_eq!(candidates[0].effect, candidates[1].effect);
    assert_ne!(candidates[0].damage, candidates[1].damage);
    for oracle in [ISAMARU, OX] {
        let mut branch = g.fork();
        let c = candidates
            .iter()
            .find(|c| c.damage_oracle_id == oracle)
            .unwrap();
        input(
            &mut branch,
            &p,
            PlayerResponse::DamageEffect {
                effect: c.effect.clone(),
                damage: c.damage.clone(),
            },
        );
        assert_eq!(life(&branch, PlayerId(3)), Quantity::from(36));
        assert_eq!(
            branch
                .commander_ledger()
                .history(CardId(0))
                .unwrap()
                .combat_damage
                .get(&PlayerId(3))
                .cloned()
                .unwrap_or_default(),
            Quantity::from(if oracle == ISAMARU { 1 } else { 2 })
        );
        assert_eq!(applications(&branch).len(), 1);
        verify(&branch, &p);
        replay_equal(&branch, &p);
    }
}

fn shared_combat() -> (Prepared, Game) {
    let p = prepared(ScenarioConfig::effects());
    let mut g = Game::new(p.clone());
    shield(&mut g, &p, PlayerId(3));
    pump(&mut g, &p, OX);
    passive_until(&mut g, |_, r| matches!(r, PlayerRequest::Attackers { .. }));
    let commander = battlefield(&g, ISAMARU, PlayerId(0));
    let ox = battlefield(&g, OX, PlayerId(0));
    input(
        &mut g,
        &p,
        PlayerResponse::Attackers {
            attacks: vec![(commander, PlayerId(3)), (ox, PlayerId(3))],
        },
    );
    passive_until(&mut g, |_, r| {
        matches!(r, PlayerRequest::DamageEffect { .. })
    });
    (p, g)
}
#[test]
fn assigned_source_departure_preserves_occurrences_and_legal_accepted_allocation() {
    let (p, g) = shared_combat();
    for accepted in [false, true] {
        let mut branch = g.fork();
        let (_, PlayerRequest::DamageEffect { candidates, .. }) = request(&branch) else {
            panic!("allocation")
        };
        let c = candidates
            .into_iter()
            .find(|c| c.damage_oracle_id == ISAMARU)
            .unwrap();
        let response = PlayerResponse::DamageEffect {
            effect: c.effect,
            damage: c.damage,
        };
        if accepted {
            submit(&mut branch, response.clone());
        }
        branch.concede(PlayerId(0)).unwrap();
        assert!(matches!(settle(&mut branch), Advance::AwaitingDecision(_)));
        assert!(branch.object_id(CardId(0)).is_none());
        assert!(branch.object_id(CardId(1)).is_none());
        if !accepted {
            assert_eq!(request(&branch).0, PlayerId(3));
            let (_, PlayerRequest::DamageEffect { candidates, .. }) = request(&branch) else {
                panic!("still legal allocation")
            };
            assert_eq!(candidates.len(), 2);
            assert!(candidates.iter().all(|c| c.damage_source.current.is_none()));
            input(&mut branch, &p, response);
        }
        assert_eq!(life(&branch, PlayerId(3)), Quantity::from(36));
        assert_eq!(
            branch
                .commander_ledger()
                .history(CardId(0))
                .unwrap()
                .combat_damage[&PlayerId(3)],
            Quantity::from(1)
        );
        assert_eq!(applications(&branch).len(), 1);
        assert_eq!(committed(&branch).len(), 1);
        verify(&branch, &p);
        replay_equal(&branch, &p);
    }
}

fn reject(g: &Game, actor: PlayerId, token: DecisionToken, response: PlayerResponse) {
    let before = g.snapshot().unwrap();
    let mut result = g.check_player_response(actor, token, response, Budget::new(0));
    for _ in 0..2000 {
        match result {
            QueryResult::Rejected(_) => {
                assert_eq!(g.snapshot().unwrap(), before);
                return;
            }
            QueryResult::Incomplete { work, .. } => result = g.resume_work(work, Budget::new(1)),
            QueryResult::Complete(_) => panic!("invalid response accepted"),
        }
    }
    panic!("rejection bound")
}
#[test]
fn damage_choice_handles_queries_sidecars_and_receipts_do_not_grant_foreign_authority() {
    let (p, mut g) = bolt_into_shield_and_furnace();
    let (actor, req) = request(&g);
    let response = choose(&g, DamageEffectKind::DoubleDamage);
    let before = g.snapshot().unwrap();
    let view = observe(&g, actor);
    for _ in 0..3 {
        assert_eq!(view, observe(&g, actor));
    }
    assert_eq!(before, g.snapshot().unwrap());
    reject(&g, PlayerId(0), req.token().clone(), response.clone());
    let foreign_effect = observe(&g, PlayerId(0))
        .view
        .effects
        .into_iter()
        .find_map(|e| match e {
            VisibleEffect::DoubleDamageToPermanentOrPlayer { handle, .. } => Some(handle),
            _ => None,
        })
        .unwrap();
    let PlayerResponse::DamageEffect { damage, .. } = &response else {
        unreachable!()
    };
    reject(
        &g,
        actor,
        req.token().clone(),
        PlayerResponse::DamageEffect {
            effect: foreign_effect,
            damage: damage.clone(),
        },
    );
    reject(&g, actor, req.token().clone(), PlayerResponse::Pass);
    let QueryResult::Incomplete { work, .. } =
        g.check_player_response(actor, req.token().clone(), response.clone(), Budget::new(0))
    else {
        panic!("budgeted work")
    };
    let saved = g.save_work(&work).unwrap();
    let mut forgery: Value = serde_json::from_str(&saved).unwrap();
    assert_eq!(forgery["work_version"], 15);
    forgery["data"]["units_done"] = json!(999);
    assert!(
        g.restore_work(&serde_json::to_string(&forgery).unwrap())
            .is_err()
    );
    let mut restored = Game::restore(p.clone(), &before).unwrap();
    let work = restored.restore_work(&saved).unwrap();
    let QueryValue::Validated(receipt) =
        complete(&restored, restored.resume_work(work, Budget::new(1)))
    else {
        panic!("recovered receipt")
    };
    assert!(g.submit(receipt).is_err());
    assert_eq!(g.snapshot().unwrap(), before);
    let work = restored.restore_work(&saved).unwrap();
    let QueryValue::Validated(receipt) =
        complete(&restored, restored.resume_work(work, Budget::new(1)))
    else {
        panic!("local receipt")
    };
    restored.submit(receipt).unwrap();
    assert!(matches!(
        settle(&mut restored),
        Advance::AwaitingDecision(_)
    ));
    assert_eq!(life(&restored, PlayerId(1)), Quantity::from(35));
    reject(&restored, actor, req.token().clone(), response);
    verify(&restored, &p);
    replay_equal(&restored, &p);
}

fn mutate_first(value: &mut Value, key: &str, new: &Value) -> bool {
    match value {
        Value::Object(map) => {
            if let Some(old) = map.get_mut(key) {
                *old = new.clone();
                return true;
            }
            map.values_mut().any(|v| mutate_first(v, key, new))
        }
        Value::Array(values) => values.iter_mut().any(|v| mutate_first(v, key, new)),
        _ => false,
    }
}
#[test]
fn rehashed_effect_and_damage_prefix_forgeries_are_rejected_by_reachable_history() {
    let (p, mut g) = bolt_into_shield_and_furnace();
    let response = choose(&g, DamageEffectKind::DoubleDamage);
    submit(&mut g, response);
    advance_one_application(&mut g, &p);
    let before: Value = serde_json::from_str(&g.snapshot().unwrap()).unwrap();
    // Mutate real typed fields rather than relying on unknown-field rejection.
    for (key, new) in [
        ("original_amount", json!("99")),
        ("amount", json!("99")),
        ("applied", json!([])),
        ("controller", json!(3)),
        ("budget_before", json!("99")),
    ] {
        let mut forged = before.clone();
        assert!(
            mutate_first(&mut forged["state"]["frame"], key, &new),
            "missing transaction field {key}"
        );
        forged["state_sha256"] = json!(mtg_contracts::digest(&forged["state"]).unwrap());
        assert!(
            Game::restore(p.clone(), &serde_json::to_string(&forged).unwrap()).is_err(),
            "forged {key} restored"
        );
    }
    assert!(matches!(settle(&mut g), Advance::AwaitingDecision(_)));
    assert_eq!(life(&g, PlayerId(1)), Quantity::from(35));
    verify(&g, &p);
    replay_equal(&g, &p);
}

#[test]
fn started_spell_damage_and_resolved_shield_survive_their_controller_departure() {
    let (p, g) = bolt_into_shield_and_furnace();
    for accepted in [false, true] {
        let mut branch = g.fork();
        let response = choose(&branch, DamageEffectKind::DoubleDamage);
        if accepted {
            submit(&mut branch, response.clone());
        }
        branch.concede(PlayerId(0)).unwrap();
        assert!(matches!(settle(&mut branch), Advance::AwaitingDecision(_)));
        if !accepted {
            let (chooser, PlayerRequest::DamageEffect { candidates, .. }) = request(&branch) else {
                panic!("surviving recipient still owns the choice")
            };
            assert_eq!(chooser, PlayerId(1));
            assert_eq!(candidates.len(), 2);
            assert!(candidates.iter().all(|c| c.damage_source.current.is_none()));
            input(&mut branch, &p, response);
        }
        assert_eq!(life(&branch, PlayerId(1)), Quantity::from(35));
        assert_eq!(applications(&branch).len(), 2);
        assert_eq!(committed(&branch).len(), 1);
        assert!(
            branch
                .events()
                .iter()
                .any(|e| matches!(e, Event::SpellCompleted { resolved: true, .. }))
        );
        verify(&branch, &p);
        replay_equal(&branch, &p);
    }
}

#[test]
fn borrowed_healer_can_create_shield_after_source_owner_leaves_using_original_history() {
    let p = prepared(ScenarioConfig::effects_borrowed());
    let mut g = Game::new(p.clone());
    activate(
        &mut g,
        &p,
        HEALER,
        PlayerTargetEntity::Player {
            player: PlayerId(1),
        },
    );
    let historical = observe(&g, PlayerId(0)).view.stack[0]
        .ability
        .as_ref()
        .unwrap()
        .source
        .announced
        .clone();
    g.concede(PlayerId(2)).unwrap();
    assert!(matches!(settle(&mut g), Advance::AwaitingDecision(_)));
    assert!(g.object_id(CardId(2)).is_none());
    round(&mut g, &p);
    let view = observe(&g, PlayerId(0)).view;
    let effect = view
        .effects
        .iter()
        .find(|e| matches!(e, VisibleEffect::PreventNextDamage { .. }))
        .expect("independent ability created its shield");
    let VisibleEffect::PreventNextDamage {
        origin, remaining, ..
    } = effect
    else {
        unreachable!()
    };
    assert_eq!(origin.source.announced, historical);
    assert_eq!(origin.source.current, None);
    assert_eq!(*remaining, Quantity::from(1));
    bolt(
        &mut g,
        &p,
        PlayerTargetEntity::Player {
            player: PlayerId(1),
        },
    );
    assert_eq!(life(&g, PlayerId(1)), Quantity::from(38));
    assert_eq!(applications(&g).len(), 1);
    verify(&g, &p);
    replay_equal(&g, &p);
}

#[test]
fn effect_creation_consumed_targets_and_resolved_modifier_provenance_are_complete() {
    let p = prepared(ScenarioConfig::effects());
    let mut g = Game::new(p.clone());
    let ox = g.object_id(CardId(1)).unwrap();
    let target = battlefield(&g, OX, PlayerId(0));
    activate(
        &mut g,
        &p,
        HEALER,
        PlayerTargetEntity::Object { object: target },
    );
    round(&mut g, &p);
    pump(&mut g, &p, SORCERER);
    let sorcerer = g.object_id(CardId(3)).unwrap();
    input(&mut g, &p, PlayerResponse::Pass);
    let object = battlefield(&g, HEALER, PlayerId(1));
    input(
        &mut g,
        &p,
        PlayerResponse::ActivateAbility { object, ability: 0 },
    );
    input(
        &mut g,
        &p,
        PlayerResponse::EntityTargets {
            entities: vec![PlayerTargetEntity::Player {
                player: PlayerId(3),
            }],
        },
    );
    round(&mut g, &p);
    let mut object_targets = vec![];
    let mut player_targets = vec![];
    for record in g.trace().records.iter() {
        let EvidenceRecord::Batch { batch } = &record.record else {
            continue;
        };
        for occurrence in &batch.occurrences {
            let ProposedEvent::CreateEffect { effect } = &occurrence.proposed else {
                continue;
            };
            match effect.kind {
                ResolvedEffectKind::ModifyPowerToughness { target, .. }
                | ResolvedEffectKind::PreventNextDamage {
                    target: TargetEntity::Object { object: target },
                    ..
                } => {
                    assert!(
                        occurrence
                            .before
                            .objects
                            .iter()
                            .any(|facts| facts.object == target)
                    );
                    object_targets.push(target);
                }
                ResolvedEffectKind::PreventNextDamage {
                    target: TargetEntity::Player { player },
                    ..
                } => {
                    assert!(
                        occurrence
                            .before
                            .players
                            .iter()
                            .any(|facts| facts.player == player)
                    );
                    player_targets.push(player);
                }
                _ => panic!("unexpected effect"),
            }
        }
    }
    assert_eq!(object_targets, vec![ox, sorcerer]);
    assert_eq!(player_targets, vec![PlayerId(3)]);
    let before: Value = serde_json::from_str(&g.snapshot().unwrap()).unwrap();
    for mutation in 0..4 {
        let mut forged = before.clone();
        let effect = forged["state"]["effects"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|e| e["kind"]["effect"] == "modify_power_toughness")
            .unwrap();
        match mutation {
            0 => effect["kind"]["power"] = json!("99"),
            1 => effect["expires_in_cleanup_of_turn"] = json!(99),
            2 => effect["origin"]["source_handles"][0] = json!("v0-o999"),
            _ => effect["origin"]["controller"] = json!(3),
        }
        forged["state_sha256"] = json!(mtg_contracts::digest(&forged["state"]).unwrap());
        assert!(Game::restore(p.clone(), &serde_json::to_string(&forged).unwrap()).is_err());
    }
    verify(&g, &p);
    replay_equal(&g, &p);
}
