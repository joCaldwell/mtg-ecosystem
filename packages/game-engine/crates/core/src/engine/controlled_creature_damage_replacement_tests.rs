//! Complete original Gratuitous Violence source, ordinary paid abilities and
//! combat. The control-grant setup is explicitly a private kernel probe.
use super::*;

const GV: &str = "7c340a39-4ee0-4ba1-bb66-6674f8020fda";
const PRODIGAL: &str = "5e961d15-5972-4e4b-9385-1cd7cd7c6bbe";
const MOGG: &str = "7efd5d62-6da2-428a-98e3-5b842f668656";
const BOMBARDMENT: &str = "edad60c6-80de-4033-af1b-a703ac332983";
const BOLT: &str = "4457ed35-7c10-48c8-9776-456485fdf070";
const BEARS: &str = "14c8f55d-d177-4c25-a931-ebeb9e6062a0";
const HEALER: &str = "95a0ca48-d924-47f4-86ed-42c673ee778c";
const FURNACE: &str = "68715465-6cf9-4006-87e9-31f227fe9ed3";
const HAMMER: &str = "dba35ac5-7ad3-488a-a006-6b9a1d54eea5";
const FOREST: &str = "b34bb2dc-c1af-4d77-b0b3-a0fb342a5fc6";
const MOUNTAIN: &str = "a3fb7228-e76b-4e96-a40e-20b5fed75685";

fn prepared(rows: &[(&str, u8, u8, Zone)]) -> Prepared {
    let mut c = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap();
    let mtg_compiler::CompileResult::Lowered { definition } = mtg_compiler::compile(
        include_str!("../../../../tests/fixtures/archive-manifest.json"),
        include_str!("../../../../tests/fixtures/gratuitous-violence.json"),
    ) else {
        panic!("complete original Gratuitous Violence source must lower");
    };
    assert!(
        definition
            .capabilities
            .contains(&Capability::ControlledCreatureDamageReplacementV1)
    );
    c.definitions.push(*definition);
    c.schema_version = mtg_contracts::CURRENT_IR_VERSION;
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    let mut config = ScenarioConfig::turns_permanents();
    config.profile = "controlled-creature-damage-replacement-domain-witness".into();
    config.initial.turn_step = TurnStep::PrecombatMain;
    config.cards = rows
        .iter()
        .enumerate()
        .map(|(id, (oracle, owner, controller, zone))| CardSetup {
            id: CardId(id as u32),
            oracle_id: (*oracle).into(),
            owner: PlayerId(*owner),
            controller: PlayerId(*controller),
            zone: *zone,
        })
        .collect();
    for seat in 0..4 {
        for _ in 0..4 {
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
        for _ in 0..2 {
            config.cards.push(CardSetup {
                id: CardId(config.cards.len() as u32),
                oracle_id: MOUNTAIN.into(),
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
fn game(rows: &[(&str, u8, u8, Zone)]) -> Game {
    Game::with_trace(prepared(rows), TraceMode::Detailed)
}
fn object(g: &Game, card: u32) -> ObjectId {
    g.object_id(CardId(card)).unwrap()
}
fn request(g: &mut Game) -> Request {
    for _ in 0..600 {
        match g.advance(Budget::new(1)) {
            Advance::AwaitingDecision(r) => return r,
            Advance::Suspended { .. } => (),
            x => panic!("controlled replacement {:?}: {x:?}", g.state.rules.frame),
        }
    }
    panic!("replacement request budget");
}
fn answer(g: &mut Game, response: Response) {
    let r = request(g);
    g.respond(r.player(), r.decision_id(), response).unwrap();
}
fn passive(g: &mut Game, r: Request) {
    let response = match &r {
        Request::Priority { .. } => Response::Pass,
        Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
        Request::Blockers { .. } => Response::Blockers { blocks: vec![] },
        Request::OrderTriggers { candidates, .. } => Response::OrderTriggers {
            order: candidates.clone(),
        },
        Request::DamageEffect { candidates, .. } => Response::DamageEffect {
            effect: candidates[0].effect.clone(),
            damage: candidates[0].damage,
        },
        x => panic!("replacement passive {x:?}"),
    };
    g.respond(r.player(), r.decision_id(), response).unwrap();
}
fn priority(g: &mut Game, player: u8) {
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
fn announce_tail(g: &mut Game, target: TargetEntity, sacrifice: Option<ObjectId>) {
    for _ in 0..20 {
        let r = request(g);
        let response = match &r {
            Request::EntityTargets { .. } => Response::EntityTargets {
                entities: vec![target],
            },
            Request::SacrificeCost { .. } => Response::SacrificeCost {
                object: sacrifice.expect("actual creature sacrifice cost"),
            },
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
                    .filter_map(|(color, n)| {
                        let mut used = n.clone();
                        used.subtract(&remaining.get(color).cloned().unwrap_or_default());
                        (used > Quantity::zero()).then_some((*color, used))
                    })
                    .collect();
                Response::SpendMana { spending }
            }
            Request::Priority { .. } => return,
            x => panic!("replacement announcement {x:?}"),
        };
        g.respond(r.player(), r.decision_id(), response).unwrap();
    }
    panic!("announcement budget");
}
fn activate(g: &mut Game, card: u32, actor: u8, target: TargetEntity, sacrifice: Option<ObjectId>) {
    priority(g, actor);
    let source = object(g, card);
    answer(
        g,
        Response::ActivateAbility {
            object: source,
            ability: 0,
        },
    );
    announce_tail(g, target, sacrifice);
}
fn begin_resolution(g: &mut Game) {
    for _ in 0..g.live_players().len() {
        answer(g, Response::Pass);
    }
}
fn finish_ability(g: &mut Game) {
    for _ in 0..150 {
        let r = request(g);
        if g.state.rules.stack.is_empty() {
            assert!(matches!(r, Request::Priority { .. }));
            return;
        }
        passive(g, r);
    }
    panic!("ability completion");
}
fn resolve(g: &mut Game) {
    begin_resolution(g);
    finish_ability(g);
}
fn shield(g: &mut Game, healer: u32) {
    activate(
        g,
        healer,
        0,
        TargetEntity::Player {
            player: PlayerId(1),
        },
        None,
    );
    resolve(g);
}
fn ordering(g: &mut Game, pinger: u32) {
    activate(
        g,
        pinger,
        0,
        TargetEntity::Player {
            player: PlayerId(1),
        },
        None,
    );
    begin_resolution(g);
    let r = request(g);
    assert!(matches!(
        r,
        Request::DamageEffect {
            player: PlayerId(1),
            ..
        }
    ));
}
fn choose(g: &mut Game, key: EffectKey) {
    let r = request(g);
    let Request::DamageEffect { ref candidates, .. } = r else {
        panic!("effect choice");
    };
    let c = candidates
        .iter()
        .find(|c| c.effect == key)
        .expect("current applicable effect");
    let before = g.damage_transaction().unwrap().applications.len();
    g.respond(
        r.player(),
        r.decision_id(),
        Response::DamageEffect {
            effect: c.effect.clone(),
            damage: c.damage,
        },
    )
    .unwrap();
    for _ in 0..20 {
        let outcome = g.advance(Budget::new(1));
        let applied = g
            .damage_transaction()
            .is_ok_and(|tx| tx.applications.len() > before);
        match outcome {
            Advance::Suspended { .. } => {
                if applied {
                    return;
                }
            }
            // advance checks for the next decision before charging another
            // step. Accept that boundary only after the chosen application
            // has actually extended the retained transaction prefix.
            Advance::AwaitingDecision(_) if applied => return,
            other => panic!(
                "effect application did not reach its recorded prefix: applied={applied}, outcome={other:?}"
            ),
        }
    }
    panic!("effect application boundary");
}
fn actual(g: &Game) -> Vec<ActualDamage> {
    g.trace
        .records
        .iter()
        .filter_map(|r| {
            if let EvidenceRecord::Batch { batch } = &r.record {
                Some(&batch.occurrences)
            } else {
                None
            }
        })
        .flatten()
        .filter_map(|o| {
            if let EventOutcome::DamageCommitted { actual, .. } = &o.actual {
                Some(actual.iter().cloned())
            } else {
                None
            }
        })
        .flatten()
        .collect()
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
fn view(g: &Game, player: u8) -> Observation {
    let QueryResult::Complete(QueryValue::Observation(o)) =
        g.observe(PlayerId(player), Budget::unlimited())
    else {
        panic!("view");
    };
    o
}

#[test]
fn controlled_creature_damage_replacement_real_combat_and_owned_borrowed_opponent_abilities() {
    for (owner, controller, combat, expected) in [
        (0, 0, false, 2),
        (3, 0, false, 2),
        (1, 1, false, 1),
        (3, 0, true, 2),
    ] {
        let mut g = game(&[
            (GV, 2, 0, Zone::Battlefield),
            (PRODIGAL, owner, controller, Zone::Battlefield),
        ]);
        if combat {
            for _ in 0..100 {
                let r = request(&mut g);
                if matches!(r, Request::Attackers { .. }) {
                    let source = object(&g, 1);
                    g.respond(
                        r.player(),
                        r.decision_id(),
                        Response::Attackers {
                            attacks: vec![(source, PlayerId(1))],
                        },
                    )
                    .unwrap();
                    break;
                }
                passive(&mut g, r);
            }
            for _ in 0..150 {
                let r = request(&mut g);
                if g.turn().step == TurnStep::EndCombat {
                    break;
                }
                passive(&mut g, r);
            }
        } else {
            activate(
                &mut g,
                1,
                controller,
                TargetEntity::Player {
                    player: PlayerId(1),
                },
                None,
            );
            resolve(&mut g);
        }
        assert_eq!(g.state.rules.players[1].life, Quantity::from(40 - expected));
        let damage = actual(&g);
        assert_eq!(damage.len(), 1);
        assert_eq!(damage[0].amount, expected.into());
        assert_eq!(damage[0].source.owner, PlayerId(owner));
        assert_eq!(damage[0].controller, PlayerId(controller));
        recover(&g);
    }
    let mut g = game(&[
        (GV, 0, 0, Zone::Battlefield),
        (PRODIGAL, 0, 0, Zone::Battlefield),
        (BEARS, 1, 1, Zone::Battlefield),
    ]);
    let victim = object(&g, 2);
    activate(&mut g, 1, 0, TargetEntity::Object { object: victim }, None);
    resolve(&mut g);
    assert_eq!(
        g.state.rules.objects[&CardId(2)].zone,
        Zone::Graveyard { owner: PlayerId(1) }
    );
    assert_eq!(actual(&g)[0].amount, 2.into());
    recover(&g);
}

#[test]
fn controlled_creature_damage_replacement_excludes_bolt_and_bombardment_damage_source() {
    let mut g = game(&[
        (GV, 0, 0, Zone::Battlefield),
        (BOLT, 0, 0, Zone::Hand { owner: PlayerId(0) }),
        (BEARS, 0, 0, Zone::Battlefield),
        (BOMBARDMENT, 0, 0, Zone::Battlefield),
    ]);
    answer(&mut g, Response::Cast { card: CardId(1) });
    announce_tail(
        &mut g,
        TargetEntity::Player {
            player: PlayerId(1),
        },
        None,
    );
    resolve(&mut g);
    let bears = object(&g, 2);
    activate(
        &mut g,
        3,
        0,
        TargetEntity::Player {
            player: PlayerId(1),
        },
        Some(bears),
    );
    resolve(&mut g);
    let damage = actual(&g);
    assert_eq!(
        damage.iter().map(|d| d.amount.clone()).collect::<Vec<_>>(),
        vec![3.into(), 1.into()]
    );
    assert_eq!(damage[1].source.object.card, CardId(3));
    assert_eq!(damage[1].source.characteristics.kind, CardKind::Enchantment);
    assert_eq!(g.state.rules.players[1].life, 36.into());
    assert_eq!(
        g.state.rules.objects[&CardId(2)].zone,
        Zone::Graveyard { owner: PlayerId(0) }
    );
    recover(&g);
}

#[test]
fn controlled_creature_damage_replacement_sacrificed_mogg_lki_survives_owner_departure() {
    for owner_leaves in [false, true] {
        let mut g = game(&[
            (GV, 2, 0, Zone::Battlefield),
            (MOGG, 3, 0, Zone::Battlefield),
        ]);
        let former = object(&g, 1);
        activate(
            &mut g,
            1,
            0,
            TargetEntity::Player {
                player: PlayerId(1),
            },
            None,
        );
        assert!(g.current(former).is_err());
        assert_eq!(g.state.rules.stack.len(), 1);
        if owner_leaves {
            g.concede(PlayerId(3)).unwrap();
            let _ = request(&mut g);
            assert!(g.object_id(CardId(1)).is_none());
        }
        resolve(&mut g);
        assert_eq!(g.state.rules.players[1].life, 38.into());
        let damage = actual(&g);
        assert_eq!(damage[0].source.object, former);
        assert_eq!(damage[0].source.owner, PlayerId(3));
        assert_eq!(damage[0].controller, PlayerId(0));
        assert_eq!(damage[0].amount, 2.into());
        recover(&g);
    }
}

#[test]
fn controlled_creature_damage_replacement_prevention_order_and_each_distinct_doubler_once() {
    for double_first in [false, true] {
        let mut g = game(&[
            (GV, 0, 0, Zone::Battlefield),
            (PRODIGAL, 0, 0, Zone::Battlefield),
            (HEALER, 0, 0, Zone::Battlefield),
        ]);
        shield(&mut g, 2);
        ordering(&mut g, 1);
        let key = if double_first {
            EffectKey::Static {
                source: object(&g, 0),
                ability: 0,
            }
        } else {
            let Request::DamageEffect { candidates, .. } = request(&mut g) else {
                panic!("choice");
            };
            candidates
                .iter()
                .find(|c| matches!(c.effect, EffectKey::Resolved { .. }))
                .unwrap()
                .effect
                .clone()
        };
        choose(&mut g, key);
        let prefix = &g.damage_transaction().unwrap().applications;
        assert_eq!(prefix.len(), 1);
        assert_eq!(prefix[0].qualification.is_some(), double_first);
        recover(&g);
        finish_ability(&mut g);
        assert_eq!(
            g.state.rules.players[1].life,
            if double_first { 39.into() } else { 40.into() }
        );
        assert_eq!(actual(&g).len(), usize::from(double_first));
        recover(&g);
    }
    for furnace in [false, true] {
        let mut rows = vec![
            (GV, 0, 0, Zone::Battlefield),
            (PRODIGAL, 0, 0, Zone::Battlefield),
            (GV, 0, 0, Zone::Battlefield),
        ];
        if furnace {
            rows.push((FURNACE, 0, 0, Zone::Battlefield));
        }
        let mut g = game(&rows);
        activate(
            &mut g,
            1,
            0,
            TargetEntity::Player {
                player: PlayerId(1),
            },
            None,
        );
        begin_resolution(&mut g);
        let mut seen = BTreeSet::new();
        for _ in 0..20 {
            let r = request(&mut g);
            if g.state.rules.stack.is_empty() {
                break;
            }
            if let Request::DamageEffect { candidates, .. } = &r {
                let c = &candidates[0];
                assert!(seen.insert(c.effect.clone()));
            }
            passive(&mut g, r);
        }
        let expected = if furnace { 8 } else { 4 };
        assert_eq!(
            seen.len(),
            if furnace { 2 } else { 1 },
            "final sole candidate applies automatically"
        );
        assert_eq!(actual(&g)[0].amount, expected.into());
        assert_eq!(g.state.rules.players[1].life, Quantity::from(40 - expected));
        recover(&g);
    }
}

#[test]
fn controlled_creature_damage_replacement_provider_departure_preserves_only_applied_prefix() {
    for applied in [false, true] {
        let mut g = game(&[
            (GV, 2, 0, Zone::Battlefield),
            (PRODIGAL, 0, 0, Zone::Battlefield),
            (HEALER, 0, 0, Zone::Battlefield),
        ]);
        shield(&mut g, 2);
        ordering(&mut g, 1);
        if applied {
            let key = EffectKey::Static {
                source: object(&g, 0),
                ability: 0,
            };
            choose(&mut g, key);
            assert!(
                g.damage_transaction().unwrap().applications[0]
                    .qualification
                    .is_some()
            );
            recover(&g);
        }
        g.concede(PlayerId(2)).unwrap();
        let _ = g.advance(Budget::new(1));
        assert!(g.object_id(CardId(0)).is_none());
        recover(&g);
        finish_ability(&mut g);
        assert_eq!(
            g.state.rules.players[1].life,
            if applied { 39.into() } else { 40.into() }
        );
        assert_eq!(actual(&g).len(), usize::from(applied));
        recover(&g);
    }
}

#[test]
fn controlled_creature_damage_replacement_private_kernel_control_return_is_common_rules() {
    // No admitted control spell creates this initial grant. The source cards
    // and subsequent activation/concession are real, but replay is deliberately
    // not asserted for the synthetic control-grant setup or its continuation.
    for (gv, already_applied, owner_leaves) in [
        (false, false, false),
        (true, false, false),
        (true, true, false),
        (true, false, true),
    ] {
        let mut g = game(&[
            (if gv { GV } else { FOREST }, 0, 0, Zone::Battlefield),
            (
                PRODIGAL,
                if owner_leaves { 2 } else { 0 },
                1,
                Zone::Battlefield,
            ),
            (HEALER, 0, 0, Zone::Battlefield),
            (FURNACE, 0, 0, Zone::Battlefield),
            (BEARS, 3, 3, Zone::Battlefield),
        ]);
        let damager = object(&g, 1);
        let grant_source = object(&g, 4);
        g.state
            .rules
            .objects
            .get_mut(&CardId(1))
            .unwrap()
            .controller = PlayerId(0);
        g.state.rules.control_effects = vec![ControlEffect {
            id: 1,
            object: damager,
            controller: PlayerId(0),
            source: ControlSource::Static {
                object: grant_source,
            },
        }];
        shield(&mut g, 2);
        ordering(&mut g, 1);
        assert_eq!(
            g.damage_transaction().unwrap().entries[0].source.controller,
            PlayerId(0)
        );
        assert_eq!(
            g.damage_transaction().unwrap().entries[0]
                .replacement_source
                .is_some(),
            gv
        );
        if already_applied {
            let key = EffectKey::Static {
                source: object(&g, 0),
                ability: 0,
            };
            choose(&mut g, key);
        }
        g.concede(PlayerId(3)).unwrap();
        let _ = g.advance(Budget::new(1));
        assert_eq!(g.current(damager).unwrap().controller, PlayerId(1));
        let tx = g.damage_transaction().unwrap();
        assert_eq!(tx.entries[0].source.controller, PlayerId(0));
        let fresh = tx.entries[0].replacement_source.as_ref().unwrap();
        assert_eq!(fresh.controller, PlayerId(1));
        assert!(!fresh.attack_ready);
        if owner_leaves {
            // CR 609.7c: once the creature leaves, the latest appearance is
            // controller one's, not its original assignment-time controller.
            g.concede(PlayerId(2)).unwrap();
            let _ = g.advance(Budget::new(1));
            assert!(g.current(damager).is_err());
            let latest = g.damage_transaction().unwrap().entries[0]
                .replacement_source
                .as_ref()
                .unwrap();
            assert_eq!(latest.owner, PlayerId(2));
            assert_eq!(latest.controller, PlayerId(1));
        }
        let furnace = EffectKey::Static {
            source: object(&g, 3),
            ability: 0,
        };
        choose(&mut g, furnace);
        finish_ability(&mut g);
        assert_eq!(actual(&g)[0].controller, PlayerId(0));
        assert_eq!(actual(&g)[0].source.controller, PlayerId(0));
        assert_eq!(
            actual(&g)[0].amount,
            if already_applied { 3.into() } else { 1.into() }
        );
    }
}

#[test]
fn controlled_creature_damage_replacement_private_kernel_lifelink_uses_current_controller() {
    // The same unadmitted control-grant setup as above supplies a borrowed
    // creature with default controller one. Equip, costs, targets, concession
    // and damage are ordinary actions; no replay claim covers the kernel setup.
    let mut g = game(&[
        (GV, 0, 0, Zone::Battlefield),
        (PRODIGAL, 0, 1, Zone::Battlefield),
        (HEALER, 0, 0, Zone::Battlefield),
        (FURNACE, 0, 0, Zone::Battlefield),
        (BEARS, 3, 3, Zone::Battlefield),
        (HAMMER, 0, 0, Zone::Battlefield),
        // The shared setup supplies two more Mountains: equip costs three.
        (MOUNTAIN, 0, 0, Zone::Battlefield),
    ]);
    let damager = object(&g, 1);
    let grant_source = object(&g, 4);
    g.state
        .rules
        .objects
        .get_mut(&CardId(1))
        .unwrap()
        .controller = PlayerId(0);
    g.state.rules.control_effects = vec![ControlEffect {
        id: 1,
        object: damager,
        controller: PlayerId(0),
        source: ControlSource::Static {
            object: grant_source,
        },
    }];
    let hammer = object(&g, 5);
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
            objects: vec![damager],
        },
    );
    announce_tail(
        &mut g,
        TargetEntity::Player {
            player: PlayerId(2),
        },
        None,
    );
    resolve(&mut g);
    assert!(
        g.current_keywords(damager)
            .contains(&KeywordAbility::Lifelink)
    );
    activate(
        &mut g,
        2,
        0,
        TargetEntity::Player {
            player: PlayerId(2),
        },
        None,
    );
    resolve(&mut g);
    activate(
        &mut g,
        1,
        0,
        TargetEntity::Player {
            player: PlayerId(2),
        },
        None,
    );
    begin_resolution(&mut g);
    assert!(matches!(
        request(&mut g),
        Request::DamageEffect {
            player: PlayerId(2),
            ..
        }
    ));
    g.concede(PlayerId(3)).unwrap();
    let _ = g.advance(Budget::new(1));
    assert_eq!(g.current(damager).unwrap().controller, PlayerId(1));
    let furnace = EffectKey::Static {
        source: object(&g, 3),
        ability: 0,
    };
    choose(&mut g, furnace);
    finish_ability(&mut g);
    let damage = actual(&g);
    assert_eq!(damage[0].controller, PlayerId(0));
    assert_eq!(damage[0].source.controller, PlayerId(0));
    assert!(
        damage[0]
            .source
            .keywords
            .contains(&KeywordAbility::Lifelink)
    );
    assert_eq!(damage[0].amount, 1.into());
    assert_eq!(g.state.rules.players[0].life, 40.into());
    assert_eq!(g.state.rules.players[1].life, 41.into());
    assert_eq!(g.state.rules.players[2].life, 39.into());
    let occurrence = g.trace.records.iter().find_map(|record| {
        let EvidenceRecord::Batch { batch } = &record.record else { return None; };
        batch.occurrences.iter().find(|o| matches!(&o.actual, EventOutcome::DamageCommitted { actual, .. } if !actual.is_empty()))
    }).expect("actual ping commit");
    let EventOutcome::DamageCommitted { life_gains, .. } = &occurrence.actual else {
        unreachable!();
    };
    assert_eq!(life_gains.len(), 1);
    assert_eq!(life_gains[0].source, damager);
    assert_eq!(life_gains[0].player, PlayerId(1));
    assert_eq!(life_gains[0].amount, 1.into());
    assert_eq!(life_gains[0].previous, 40.into());
    assert_eq!(life_gains[0].current, 41.into());
    assert_eq!(
        occurrence
            .before
            .players
            .iter()
            .map(|p| p.player)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([PlayerId(1), PlayerId(2)])
    );
    assert!(
        occurrence
            .before
            .players
            .iter()
            .all(|p| p.life == Quantity::from(40))
    );
}

fn transaction_mut(snapshot: &mut Snapshot) -> &mut DamageTransaction {
    let Frame::DamageEffects { transaction } = &mut snapshot.state.rules.frame else {
        panic!("damage effect frame");
    };
    transaction
}

#[test]
fn controlled_creature_damage_replacement_prefix_recovery_features_and_public_privacy() {
    let mut g = game(&[
        (GV, 0, 0, Zone::Battlefield),
        (PRODIGAL, 0, 0, Zone::Battlefield),
        (HEALER, 0, 0, Zone::Battlefield),
    ]);
    for (search, hosted, feature) in [
        (true, false, Feature::Search),
        (false, true, Feature::Hosting),
    ] {
        let mut profile = match_profile("commander-foundations-v1").unwrap().clone();
        profile.search = search;
        profile.hosted = hosted;
        assert!(
            matches!(profile.validate_requested_features(&g.prepared.catalog),Err(ProfileError::UnreviewedCapabilities {feature:f,capabilities}) if f==feature&&capabilities.contains(&Capability::ControlledCreatureDamageReplacementV1))
        );
    }
    shield(&mut g, 2);
    ordering(&mut g, 1);
    let Some(PlayerRequest::DamageEffect { candidates, .. }) = view(&g, 1).decision else {
        panic!("affected-player public ordering");
    };
    assert_eq!(candidates.len(), 2);
    assert!(
        candidates
            .iter()
            .any(|c| c.effect_oracle_id == GV && c.damage_oracle_id == PRODIGAL)
    );
    assert!(view(&g, 0).decision.is_none());
    for seat in 0..4 {
        let o = view(&g, seat);
        assert!(
            !o.view
                .objects
                .iter()
                .any(|o| matches!(o.zone, Zone::Library { .. }))
        );
        let wire = serde_json::to_string(&o).unwrap();
        assert!(!wire.contains("replacement_source"));
        assert!(!wire.contains("qualification"));
    }
    recover(&g);
    let pending: Snapshot = strict_read(&g.snapshot().unwrap()).unwrap();
    for wrong_zone in [false, true] {
        let mut bad = pending.clone();
        let source = transaction_mut(&mut bad).entries[0]
            .replacement_source
            .as_mut()
            .unwrap();
        if wrong_zone {
            source.zone = Zone::Stack;
        } else {
            source.object.card = CardId(2);
        }
        let mut structural = g.fork();
        structural.state = bad.state.clone();
        assert!(structural.validate_effect_state().is_err());
        bad.state_sha256 = digest(&bad.state).unwrap();
        assert!(
            Game::restore(
                g.prepared.as_ref().clone(),
                &serde_json::to_string(&bad).unwrap()
            )
            .is_err()
        );
    }
    let key = EffectKey::Static {
        source: object(&g, 0),
        ability: 0,
    };
    choose(&mut g, key);
    let base: Snapshot = strict_read(&g.snapshot().unwrap()).unwrap();
    assert!(
        g.damage_transaction().unwrap().applications[0]
            .qualification
            .is_some()
    );
    recover(&g);
    for lie in [
        "missing",
        "effect_incarnation",
        "damager_identity",
        "wrong_control",
        "immutable_snow",
    ] {
        let mut bad = base.clone();
        let q = &mut transaction_mut(&mut bad).applications[0].qualification;
        if lie == "missing" {
            *q = None;
        } else {
            let q = q.as_mut().unwrap();
            match lie {
                "effect_incarnation" => q.effect_source.object.incarnation += 1,
                "damager_identity" => q.damage_source.object.card = CardId(2),
                "wrong_control" => q.damage_source.controller = PlayerId(1),
                "immutable_snow" => q.damage_source.characteristics.snow = true,
                _ => unreachable!(),
            }
        }
        let mut structural = g.fork();
        structural.state = bad.state.clone();
        assert!(structural.validate_effect_state().is_err(), "{lie}");
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
    let mut coherent = base;
    transaction_mut(&mut coherent).applications[0]
        .qualification
        .as_mut()
        .unwrap()
        .damage_source
        .characteristics
        .power = Some(9.into());
    let mut structural = g.fork();
    structural.state = coherent.state.clone();
    structural.validate_effect_state().unwrap();
    coherent.state_sha256 = digest(&coherent.state).unwrap();
    assert!(
        Game::restore(
            g.prepared.as_ref().clone(),
            &serde_json::to_string(&coherent).unwrap()
        )
        .is_err()
    );
}
