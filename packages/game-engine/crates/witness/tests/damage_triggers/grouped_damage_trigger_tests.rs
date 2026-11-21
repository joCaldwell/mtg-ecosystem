//! Complete archived Kutzil source and ordinary paid spells/combat. Helpers are
//! shared only with the adjacent cast-prohibition source witnesses.
use super::*;

pub(super) const KUTZIL: &str = "3233af43-7826-4895-b67e-03c6102c2cd5";
pub(super) const RESEARCH: &str = "ba5055ce-6809-47a4-bee6-fb97a5b60f8a";
pub(super) const CROW: &str = "000d5588-5a4c-434e-988d-396632ade42c";
pub(super) const ANTHEM: &str = "e3886fe8-9b76-4613-8891-4ec74657c087";
pub(super) const GROWTH: &str = "5748ebf1-24e3-499d-ab7c-c2cebd462a24";
const DREADMAW: &str = "08c7db90-c0cf-4482-b7ee-bb033e5996d2";
const ACE: &str = "2f810936-2ba6-4c2b-84a0-ff4c1deb026b";
const FOG: &str = "27e9db49-7af7-4bef-ad4c-bf5dfb92030d";
const HEALER: &str = "95a0ca48-d924-47f4-86ed-42c673ee778c";
const FURNACE: &str = "68715465-6cf9-4006-87e9-31f227fe9ed3";
const ALARM: &str = "5b2364d7-a811-4595-a1b4-224c70555ffa";
const HAMMER: &str = "dba35ac5-7ad3-488a-a006-6b9a1d54eea5";
const BEARS: &str = "14c8f55d-d177-4c25-a931-ebeb9e6062a0";
pub(super) const FOREST: &str = "b34bb2dc-c1af-4d77-b0b3-a0fb342a5fc6";
const PLAINS: &str = "bc71ebf6-2056-41f7-be35-b2e5c34afa99";
const ISLAND: &str = "b2c6aa39-2d2a-459c-a555-fb48ba993373";

pub(super) fn prepared(rows: &[(&str, u8, u8, Zone)]) -> Prepared {
    let mut c = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap();
    let header =
        include_str!("../../../../tests/fixtures/cards-data-d2ce/archive-manifest-current.json");
    for raw in [
        include_str!(
            "../../../../tests/fixtures/cards-data-d2ce/3233af43-7826-4895-b67e-03c6102c2cd5.json"
        ),
        include_str!(
            "../../../../tests/fixtures/cards-data-d2ce/ba5055ce-6809-47a4-bee6-fb97a5b60f8a.json"
        ),
    ] {
        let mtg_compiler::CompileResult::Lowered { definition } =
            mtg_compiler::compile(header, raw)
        else {
            panic!("complete archived source must lower");
        };
        c.definitions.push(*definition);
    }
    c.schema_version = mtg_contracts::CURRENT_IR_VERSION;
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    let kutzil = c.definition_by_oracle_id(KUTZIL).unwrap();
    assert!(
        kutzil
            .capabilities
            .contains(&Capability::OpponentCastProhibitionV1)
    );
    assert!(
        kutzil
            .capabilities
            .contains(&Capability::GroupedCreatureDamageTriggerV1)
    );
    let mut config = ScenarioConfig::turns_permanents();
    config.profile = "grouped-damage-domain-witness".into();
    config.initial.turn_step = TurnStep::PrecombatMain;
    config.cards.clear();
    for (id, (oracle, owner, controller, zone)) in rows.iter().enumerate() {
        let id = CardId(id as u32);
        config.cards.push(CardSetup {
            id,
            oracle_id: (*oracle).into(),
            owner: PlayerId(*owner),
            controller: PlayerId(*controller),
            zone: *zone,
        });
        if matches!(zone, Zone::Command { .. }) {
            config.commanders.push(id);
        }
    }
    for seat in 0..4 {
        for _ in 0..6 {
            config.cards.push(CardSetup {
                id: CardId(config.cards.len() as u32),
                oracle_id: FOREST.into(),
                owner: PlayerId(seat),
                controller: PlayerId(seat),
                zone: Zone::Library {
                    owner: PlayerId(seat),
                },
            });
        }
        for oracle in [
            FOREST, FOREST, FOREST, PLAINS, PLAINS, PLAINS, ISLAND, ISLAND, ISLAND, ISLAND,
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
pub(super) fn object(g: &Game, id: u32) -> ObjectId {
    g.object_id(CardId(id)).unwrap()
}
pub(super) fn request(g: &mut Game) -> Request {
    for _ in 0..1000 {
        match g.advance(Budget::new(1)) {
            Advance::AwaitingDecision(r) => return r,
            Advance::Suspended { .. } => (),
            x => panic!("group witness {:?}: {x:?}", g.state.rules.frame),
        }
    }
    panic!("group request budget");
}
pub(super) fn answer(g: &mut Game, response: Response) {
    let r = request(g);
    g.respond(r.player(), r.decision_id(), response).unwrap();
}
pub(super) fn passive(g: &mut Game, r: Request) {
    let response = match &r {
        Request::Priority { .. } => Response::Pass,
        Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
        Request::Blockers { .. } => Response::Blockers { blocks: vec![] },
        Request::OrderTriggers { candidates, .. } => Response::OrderTriggers {
            order: candidates.clone(),
        },
        Request::Discard {
            count, candidates, ..
        } => Response::Discard {
            objects: candidates.iter().take(*count).copied().collect(),
        },
        Request::DamageEffect { candidates, .. } => Response::DamageEffect {
            effect: candidates[0].effect.clone(),
            damage: candidates[0].damage,
        },
        x => panic!("unexpected group passive {x:?}"),
    };
    g.respond(r.player(), r.decision_id(), response).unwrap();
}
pub(super) fn priority(g: &mut Game, player: u8) {
    for _ in 0..5 {
        let r = request(g);
        assert!(matches!(r, Request::Priority { .. }));
        if r.player() == PlayerId(player) {
            return;
        }
        passive(g, r);
    }
    panic!("priority");
}
pub(super) fn fund(g: &mut Game) {
    for _ in 0..40 {
        let r = request(g);
        let response = match &r {
            Request::Mana { cost, .. } => {
                if g.payment_pool(r.player(), cost).is_ok() {
                    Response::FinishMana { undo_mana: false }
                } else {
                    let land = g
                        .state
                        .rules
                        .objects
                        .values()
                        .find(|o| {
                            o.controller == r.player()
                                && o.zone == Zone::Battlefield
                                && !o.tapped
                                && g.definition(o.id.card).unwrap().characteristics.kind
                                    == CardKind::Land
                        })
                        .unwrap()
                        .id;
                    Response::ActivateMana { object: land }
                }
            }
            Request::SpendMana {
                cost, available, ..
            } => {
                let remaining = g.payment_pool(r.player(), cost).unwrap();
                let spending = available
                    .iter()
                    .filter_map(|(color, amount)| {
                        let mut n = amount.clone();
                        n.subtract(&remaining.get(color).cloned().unwrap_or_default());
                        (n > Quantity::zero()).then_some((*color, n))
                    })
                    .collect();
                Response::SpendMana { spending }
            }
            Request::Priority { .. } => return,
            x => panic!("funding {x:?}"),
        };
        g.respond(r.player(), r.decision_id(), response).unwrap();
    }
    panic!("funding bound");
}
pub(super) fn announce(g: &mut Game, card: u32, targets: &[ObjectId]) {
    answer(g, Response::Cast { card: CardId(card) });
    if !targets.is_empty() {
        answer(
            g,
            Response::Targets {
                objects: targets.to_vec(),
            },
        );
    }
    fund(g);
}
pub(super) fn resolve(g: &mut Game) {
    for _ in 0..g.live_players().len() {
        answer(g, Response::Pass);
    }
    let _ = request(g);
}
pub(super) fn cast(g: &mut Game, card: u32, targets: &[ObjectId]) {
    announce(g, card, targets);
    resolve(g);
}
fn attackers(g: &mut Game, turn: u64, attacks: &[(u32, u8)]) {
    for _ in 0..2000 {
        let r = request(g);
        if g.turn().number == turn && matches!(r, Request::Attackers { .. }) {
            let attacks = attacks
                .iter()
                .map(|(card, p)| (object(g, *card), PlayerId(*p)))
                .collect();
            g.respond(r.player(), r.decision_id(), Response::Attackers { attacks })
                .unwrap();
            return;
        }
        passive(g, r);
    }
    panic!("attackers");
}
fn end_combat(g: &mut Game) {
    for _ in 0..500 {
        let r = request(g);
        if g.turn().step == TurnStep::EndCombat {
            return;
        }
        passive(g, r);
    }
    panic!("endcombat");
}
fn placed(g: &mut Game) {
    for _ in 0..300 {
        let r = request(g);
        if !g.state.rules.stack.is_empty() {
            assert!(matches!(r, Request::Priority { .. }));
            return;
        }
        passive(g, r);
    }
    panic!("group placement");
}
pub(super) fn hand(g: &Game, player: u8) -> usize {
    g.state
        .rules
        .objects
        .values()
        .filter(|o| {
            o.zone
                == (Zone::Hand {
                    owner: PlayerId(player),
                })
        })
        .count()
}
pub(super) fn recover(g: &Game) {
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
pub(super) fn view(g: &Game, player: u8) -> Observation {
    let QueryResult::Complete(QueryValue::Observation(o)) =
        g.observe(PlayerId(player), Budget::unlimited())
    else {
        panic!("view");
    };
    o
}
fn members_mut(s: &mut Snapshot) -> &mut Vec<GroupedDamageMember> {
    let StackKind::Triggered { witness, .. } = &mut s.state.rules.stack[0].kind else {
        panic!("trigger");
    };
    let TriggerEventWitness::GroupedDamageDealt { members, .. } = witness else {
        panic!("group");
    };
    members
}

#[test]
fn grouped_damage_uses_each_creatures_own_base_power_not_observer_power() {
    for (creature, anthem, draws) in [(DREADMAW, false, 0), (CROW, true, 1)] {
        let mut rows = vec![
            (KUTZIL, 2, 0, Zone::Battlefield),
            (creature, 0, 0, Zone::Battlefield),
        ];
        if anthem {
            rows.push((ANTHEM, 0, 0, Zone::Battlefield));
        }
        let mut g = Game::new(prepared(&rows));
        attackers(&mut g, 1, &[(1, 1)]);
        end_combat(&mut g);
        assert_eq!(g.state.rules.next_trigger_id, draws);
        assert_eq!(hand(&g, 0), draws as usize);
        assert_eq!(
            g.state.rules.players[1].life,
            if anthem { 38.into() } else { 34.into() }
        );
        recover(&g);
    }
}

#[test]
fn grouped_damage_same_player_split_players_and_mixed_qualification() {
    for split in [false, true] {
        let mut g = Game::new(prepared(&[
            (KUTZIL, 2, 0, Zone::Battlefield),
            (ANTHEM, 0, 0, Zone::Battlefield),
            (CROW, 0, 0, Zone::Battlefield),
            (CROW, 0, 0, Zone::Battlefield),
        ]));
        attackers(&mut g, 1, &[(2, 1), (3, if split { 2 } else { 1 })]);
        placed(&mut g);
        assert_eq!(g.state.rules.stack.len(), if split { 2 } else { 1 });
        recover(&g);
        end_combat(&mut g);
        assert_eq!(hand(&g, 0), if split { 2 } else { 1 });
        recover(&g);
    }
    let mut mixed = Game::new(prepared(&[
        (KUTZIL, 2, 0, Zone::Battlefield),
        (CROW, 0, 0, Zone::Battlefield),
        (DREADMAW, 0, 0, Zone::Battlefield),
        (GROWTH, 0, 0, Zone::Hand { owner: PlayerId(0) }),
    ]));
    let crow = object(&mixed, 1);
    cast(&mut mixed, 3, &[crow]);
    attackers(&mut mixed, 1, &[(1, 1), (2, 1)]);
    placed(&mut mixed);
    let StackKind::Triggered {
        witness: TriggerEventWitness::GroupedDamageDealt { members, .. },
        ..
    } = &mixed.state.rules.stack[0].kind
    else {
        panic!("group");
    };
    assert_eq!(members.len(), 1);
    assert_eq!(members[0].damage.source.object.card, CardId(1));
    end_combat(&mut mixed);
    assert_eq!(hand(&mixed, 0), 1);
    assert_eq!(mixed.state.rules.players[1].life, 30.into());
    recover(&mixed);
}

#[test]
fn grouped_damage_first_and_double_strike_are_separate_events_and_prevention_is_not_damage() {
    for fog in [false, true] {
        let mut rows = vec![
            (KUTZIL, 2, 0, Zone::Battlefield),
            (ANTHEM, 0, 0, Zone::Battlefield),
            (ACE, 0, 0, Zone::Battlefield),
        ];
        if fog {
            rows.push((FOG, 0, 0, Zone::Hand { owner: PlayerId(0) }));
        }
        let mut g = Game::new(prepared(&rows));
        if fog {
            cast(&mut g, 3, &[]);
        }
        attackers(&mut g, 1, &[(2, 1)]);
        end_combat(&mut g);
        assert_eq!(hand(&g, 0), if fog { 0 } else { 2 });
        assert_eq!(g.state.rules.next_trigger_id, if fog { 0 } else { 2 });
        assert_eq!(
            g.state.rules.players[1].life,
            if fog { 40.into() } else { 36.into() }
        );
        recover(&g);
    }
}

#[test]
fn grouped_damage_current_power_after_anthem_departure_and_frozen_collection_departures() {
    let mut g = Game::with_trace(
        prepared(&[
            (KUTZIL, 2, 0, Zone::Battlefield),
            (CROW, 0, 0, Zone::Battlefield),
            (ANTHEM, 3, 0, Zone::Battlefield),
            (FURNACE, 0, 0, Zone::Battlefield),
            (HEALER, 0, 0, Zone::Battlefield),
        ]),
        TraceMode::Detailed,
    );
    let healer = object(&g, 4);
    answer(
        &mut g,
        Response::ActivateAbility {
            object: healer,
            ability: 0,
        },
    );
    answer(
        &mut g,
        Response::EntityTargets {
            entities: vec![TargetEntity::Player {
                player: PlayerId(1),
            }],
        },
    );
    resolve(&mut g);
    attackers(&mut g, 1, &[(1, 1)]);
    for _ in 0..100 {
        let r = request(&mut g);
        if matches!(r, Request::DamageEffect { .. }) {
            break;
        }
        passive(&mut g, r);
    }
    assert_eq!(
        g.damage_transaction().unwrap().entries[0].original_amount,
        2.into()
    );
    g.concede(PlayerId(3)).unwrap();
    end_combat(&mut g);
    assert_eq!(
        g.characteristics(object(&g, 1)).unwrap().power,
        Some(1.into())
    );
    assert!(g.state.rules.players[1].life < 40.into());
    assert_eq!(g.state.rules.next_trigger_id, 0);
    assert_eq!(hand(&g, 0), 0);
    recover(&g);
    for departing in [2, 3] {
        let mut g = Game::new(prepared(&[
            (KUTZIL, 2, 0, Zone::Battlefield),
            (ANTHEM, 0, 0, Zone::Battlefield),
            (CROW, 3, 0, Zone::Battlefield),
        ]));
        attackers(&mut g, 1, &[(2, 1)]);
        placed(&mut g);
        g.concede(PlayerId(departing)).unwrap();
        let _ = request(&mut g);
        assert_eq!(g.state.rules.stack.len(), 1);
        recover(&g);
        resolve(&mut g);
        assert_eq!(hand(&g, 0), 1);
        recover(&g);
    }
}

#[test]
fn grouped_damage_restore_rejects_invalid_members_and_coherent_event_lies() {
    let mut g = Game::with_trace(
        prepared(&[
            (KUTZIL, 2, 0, Zone::Battlefield),
            (ANTHEM, 0, 0, Zone::Battlefield),
            (CROW, 0, 0, Zone::Battlefield),
            (CROW, 0, 0, Zone::Battlefield),
        ]),
        TraceMode::Detailed,
    );
    attackers(&mut g, 1, &[(2, 1), (3, 1)]);
    placed(&mut g);
    recover(&g);
    let base: Snapshot = strict_read(&g.snapshot().unwrap()).unwrap();
    let mut inspect = base.clone();
    assert_eq!(members_mut(&mut inspect).len(), 2);
    for lie in ["duplicate", "unqualified", "wrong_recipient"] {
        let mut bad = base.clone();
        let members = members_mut(&mut bad);
        match lie {
            "duplicate" => members.push(members[0].clone()),
            "unqualified" => members[0].match_source.characteristics.power = Some(1.into()),
            "wrong_recipient" => {
                members[0].damage.recipient = DamageRecipient::Player {
                    player: PlayerId(3),
                }
            }
            _ => unreachable!(),
        }
        let mut structural = g.fork();
        structural.state = bad.state.clone();
        assert!(structural.validate_trigger_state().is_err(), "{lie}");
        bad.state_sha256 = digest(&bad.state).unwrap();
        assert!(
            Game::restore(
                g.prepared.as_ref().clone(),
                &serde_json::to_string(&bad).unwrap()
            )
            .is_err(),
            "{lie}"
        );
    }
    for lie in ["omitted_member", "derived_power", "coherent_recipient"] {
        let mut bad = base.clone();
        match lie {
            "omitted_member" => {
                members_mut(&mut bad).pop();
            }
            "derived_power" => {
                members_mut(&mut bad)[0].match_source.characteristics.power = Some(9.into())
            }
            "coherent_recipient" => {
                let StackKind::Triggered {
                    witness:
                        TriggerEventWitness::GroupedDamageDealt {
                            recipient, members, ..
                        },
                    ..
                } = &mut bad.state.rules.stack[0].kind
                else {
                    panic!("group");
                };
                *recipient = PlayerId(3);
                for m in members {
                    m.damage.recipient = DamageRecipient::Player {
                        player: PlayerId(3),
                    };
                }
            }
            _ => unreachable!(),
        }
        let mut structural = g.fork();
        structural.state = bad.state.clone();
        structural.validate_trigger_state().unwrap();
        bad.state_sha256 = digest(&bad.state).unwrap();
        assert!(
            Game::restore(
                g.prepared.as_ref().clone(),
                &serde_json::to_string(&bad).unwrap()
            )
            .is_err(),
            "{lie}"
        );
    }
}

#[test]
fn grouped_damage_real_tokens_keep_member_base_facts_after_lethal_sba() {
    let mut g = Game::with_trace(
        prepared(&[
            (KUTZIL, 0, 0, Zone::Battlefield),
            (ANTHEM, 0, 0, Zone::Battlefield),
            (HAMMER, 0, 0, Zone::Battlefield),
            (BEARS, 1, 1, Zone::Battlefield),
            (ALARM, 0, 0, Zone::Hand { owner: PlayerId(0) }),
        ]),
        TraceMode::Detailed,
    );
    cast(&mut g, 4, &[]);
    let tokens = g.state.rules.tokens.keys().copied().collect::<Vec<_>>();
    assert_eq!(tokens.len(), 2);
    let equipped = g.object_id(tokens[0]).unwrap();
    let other = g.object_id(tokens[1]).unwrap();
    let hammer = object(&g, 2);
    answer(
        &mut g,
        Response::ActivateAbility {
            object: hammer,
            ability: 0,
        },
    );
    answer(
        &mut g,
        Response::Targets {
            objects: vec![equipped],
        },
    );
    fund(&mut g);
    resolve(&mut g);
    attackers(&mut g, 5, &[(tokens[0].0, 1), (tokens[1].0, 1)]);
    for _ in 0..100 {
        let r = request(&mut g);
        if matches!(
            r,
            Request::Blockers {
                player: PlayerId(1),
                ..
            }
        ) {
            let bears = object(&g, 3);
            g.respond(
                r.player(),
                r.decision_id(),
                Response::Blockers {
                    blocks: vec![(bears, equipped)],
                },
            )
            .unwrap();
            break;
        }
        passive(&mut g, r);
    }
    for _ in 0..100 {
        let r = request(&mut g);
        if let Request::Damage {
            lethal_requirements,
            ..
        } = &r
        {
            assert_eq!(lethal_requirements, &vec![(object(&g, 3), 2.into())]);
            break;
        }
        passive(&mut g, r);
    }
    let bears = object(&g, 3);
    answer(
        &mut g,
        Response::TrampleDamage {
            assignments: vec![(bears, 2.into())],
            player: PlayerId(1),
            amount: 3.into(),
        },
    );
    placed(&mut g);
    assert!(g.object_id(tokens[0]).is_none());
    assert!(g.object_id(tokens[1]).is_some());
    assert_eq!(
        g.definition(tokens[0]).unwrap().characteristics.power,
        Some(1.into())
    );
    let StackKind::Triggered {
        witness: TriggerEventWitness::GroupedDamageDealt { members, .. },
        ..
    } = &g.state.rules.stack[0].kind
    else {
        panic!("token group");
    };
    assert_eq!(members.len(), 2);
    assert!(members.iter().any(|m| m.match_source.object == equipped));
    assert!(members.iter().any(|m| m.match_source.object == other));
    recover(&g);
    resolve(&mut g);
    assert_eq!(
        hand(&g, 0),
        2,
        "turn-five draw plus exactly one grouped draw"
    );
    recover(&g);
}
