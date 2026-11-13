//! Archived programs run unchanged in private scenarios. Synthetic edits are
//! explicitly labeled; no extra legal hosting/search profile is admitted.
use super::*;
const ARENA: &str = "ee579a32-a048-4335-b966-231ba731cdea";
const RAM: &str = "5a20113c-7ea8-4edf-af9f-148ccd326a88";
const SWAMP: &str = "56719f6a-1a6c-4c0a-8d21-18f7d7350b68";
fn catalog() -> Catalog {
    let mut c = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap();
    let add = Catalog::read(include_str!(
        "../../../../tests/fixtures/cards-data-d2c/expected-ir.json"
    ))
    .unwrap();
    let next = Catalog::read(include_str!(
        "../../../../tests/fixtures/cards-data-d2c/expected-d2c-b-ir.json"
    ))
    .unwrap();
    c.definitions.extend(next.definitions);
    c.definitions.extend(
        Catalog::read(include_str!(
            "../../../../tests/fixtures/cards-data-d2cc/expected-ir.json"
        ))
        .unwrap()
        .definitions,
    );
    c.schema_version = mtg_contracts::CURRENT_IR_VERSION;
    c.definitions.extend(add.definitions);
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    c
}
fn prepared(sources: &[(&str, u8, u8, Zone)], step: TurnStep) -> Prepared {
    let c = catalog();
    let mut config = ScenarioConfig::turns_permanents();
    config.profile = "step-program-domain-witness".into();
    config.initial.turn_step = step;
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
    for _ in 0..4 {
        answer(g, Response::Pass);
    }
    let _ = settle(g);
}

const STUDENT: &str = "0adcf8d2-6eb9-4679-8ad5-f28e2f4aba39";
const SYTHIS: &str = "0fc64fd6-f057-4056-9dca-47accb7ff036";
const ARCHER: &str = "2e9289d6-dbc6-456d-88cf-d1f534e731d6";
const SIGIL: &str = "3b9b5b22-5a7d-4e37-a870-ca0f0efa4f36";
const MINE: &str = "d26b27db-a567-4631-b4b6-7294222fbdd1";
const PLAINS: &str = "bc71ebf6-2056-41f7-be35-b2e5c34afa99";
const FOREST: &str = "b34bb2dc-c1af-4d77-b0b3-a0fb342a5fc6";
const MOUNTAIN: &str = "a3fb7228-e76b-4e96-a40e-20b5fed75685";
const ISLAND: &str = "b2c6aa39-2d2a-459c-a555-fb48ba993373";
fn edit(p: &mut Prepared, id: &str, f: impl FnOnce(&mut Definition)) {
    let c = Arc::make_mut(&mut p.catalog);
    let d = c
        .definitions
        .iter_mut()
        .find(|d| d.oracle_id == id)
        .unwrap();
    f(d);
    d.capabilities = d.requirements();
    c.validate().unwrap();
    p.catalog_digest = digest(c).unwrap();
}
fn priority(g: &mut Game, player: u8) {
    for _ in 0..4 {
        if settle(g).player() == PlayerId(player) {
            return;
        }
        answer(g, Response::Pass);
    }
    panic!("missing priority");
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
fn pass_until(g: &mut Game, pred: impl Fn(&Game) -> bool) {
    for _ in 0..300 {
        if pred(g) {
            return;
        }
        match g.advance(Budget::new(1)) {
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(_) => passive(g),
            x => panic!("{x:?}"),
        }
    }
    panic!("missing boundary");
}
fn modes() -> [TraceMode; 3] {
    [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed]
}
fn synthetic_relation(p: &mut Prepared, relation: CasterRelation, predicate: TypePredicate) {
    edit(p, STUDENT, |d| {
        d.triggered[0].condition = TriggerCondition::SpellCast {
            caster: relation,
            spell: predicate,
        }
    });
}
#[test]
fn d2cc_archived_student_actual_paid_cast_no_self_or_entry_trigger_all_traces() {
    for mode in modes() {
        let mut g = Game::with_trace(
            prepared(
                &[
                    (STUDENT, 0, 0, Zone::Hand { owner: PlayerId(0) }),
                    (MINE, 0, 0, Zone::Hand { owner: PlayerId(0) }),
                ],
                TurnStep::PrecombatMain,
            ),
            mode,
        );
        cast(&mut g, 0);
        assert_eq!(g.state.rules.next_trigger_id, 0);
        resolve(&mut g);
        assert!(g.state.rules.stack.is_empty());
        cast(&mut g, 1);
        assert_eq!(g.state.rules.stack.len(), 2);
        assert!(g.state.rules.stack[0].kind.is_spell());
        assert_eq!(g.state.rules.next_trigger_id, 1);
        recover(&g);
        let StackKind::Triggered {
            event: TriggerOccurrenceId::Cast(id),
            witness: TriggerEventWitness::SpellCast { cast },
            ..
        } = &g.state.rules.stack[1].kind
        else {
            panic!("cast evidence")
        };
        assert_eq!(id.cast, g.state.rules.stack[0].id);
        assert_eq!(cast.caster, PlayerId(0));
        assert_eq!(cast.spell.object, g.state.rules.stack[0].source);
        assert_eq!(cast.cast_from, Zone::Hand { owner: PlayerId(0) });
        resolve(&mut g);
        assert_eq!(g.state.rules.players[0].life, 42.into());
        resolve(&mut g);
        assert_eq!(g.state.rules.next_trigger_id, 1);
        recover(&g);
    }
}
#[test]
fn d2cc_archived_enchantment_combined_types_wrong_type_and_caster_all_traces() {
    for mode in modes() {
        for (spell, who, expected) in [
            (ARENA, 0, true),
            (RAM, 0, true),
            (MINE, 0, false),
            (ARENA, 1, false),
        ] {
            let mut p = prepared(
                &[
                    (SYTHIS, 0, 0, Zone::Battlefield),
                    (SIGIL, 0, 0, Zone::Battlefield),
                    (
                        spell,
                        who,
                        who,
                        Zone::Hand {
                            owner: PlayerId(who),
                        },
                    ),
                ],
                TurnStep::PrecombatMain,
            );
            // Nonactive spells are synthetic flash composition, with unchanged trigger programs.
            if who != 0 {
                edit(&mut p, spell, |d| {
                    d.keywords.push(KeywordAbility::Flash);
                    d.spell.as_mut().unwrap().timing = SpellTiming::Instant;
                });
            }
            let mut g = Game::with_trace(p, mode);
            if who != 0 {
                priority(&mut g, who);
            }
            cast(&mut g, 2);
            assert_eq!(g.state.rules.next_trigger_id, if expected { 2 } else { 0 });
            recover(&g);
            if expected {
                let n = g.state.rules.libraries[&PlayerId(0)].len();
                resolve(&mut g);
                resolve(&mut g);
                assert_eq!(g.state.rules.players[0].life, 41.into());
                assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), n - 1);
                assert_eq!(g.state.rules.tokens.len(), 1);
                recover(&g);
            }
        }
    }
}
#[test]
fn d2cc_archived_archer_damage_each_opponent_and_source_owner_departure_all_traces() {
    for mode in modes() {
        let mut g = Game::with_trace(
            prepared(
                &[
                    (ARCHER, 2, 0, Zone::Battlefield),
                    (MINE, 0, 0, Zone::Hand { owner: PlayerId(0) }),
                ],
                TurnStep::PrecombatMain,
            ),
            mode,
        );
        cast(&mut g, 1);
        g.concede(PlayerId(2)).unwrap();
        placed(&mut g);
        recover(&g);
        resolve(&mut g);
        assert_eq!(g.state.rules.players[0].life, 40.into());
        assert_eq!(g.state.rules.players[1].life, 39.into());
        assert_eq!(g.state.rules.players[3].life, 39.into());
        recover(&g);
    }
}
#[test]
fn d2cc_synthetic_nonactive_flash_caster_apnap_order_and_actor_binding_all_traces() {
    for mode in modes() {
        let mut p = prepared(
            &[
                (STUDENT, 2, 0, Zone::Battlefield),
                (STUDENT, 1, 1, Zone::Battlefield),
                (STUDENT, 2, 2, Zone::Battlefield),
                (MINE, 3, 3, Zone::Hand { owner: PlayerId(3) }),
            ],
            TurnStep::PrecombatMain,
        );
        synthetic_relation(&mut p, CasterRelation::AnyPlayer, TypePredicate::Any {});
        edit(&mut p, STUDENT, |d| {
            d.triggered[0].program.instructions = vec![Instruction::GainLife {
                recipient: LifeRecipient::EventPlayer,
                amount: 2.into(),
                source_block: 0,
            }]
        });
        edit(&mut p, MINE, |d| {
            d.keywords.push(KeywordAbility::Flash);
            d.spell.as_mut().unwrap().timing = SpellTiming::Instant;
        });
        let mut g = Game::with_trace(p, mode);
        priority(&mut g, 3);
        cast(&mut g, 3);
        assert_eq!(g.state.rules.active, Some(PlayerId(0)));
        assert_eq!(
            g.state
                .rules
                .stack
                .iter()
                .map(|s| s.controller)
                .collect::<Vec<_>>(),
            vec![PlayerId(3), PlayerId(0), PlayerId(1), PlayerId(2)]
        );
        for s in &g.state.rules.stack[1..] {
            assert_eq!(s.kind.event_player().unwrap(), Some(PlayerId(3)));
        }
        recover(&g);
        resolve(&mut g);
        assert_eq!(g.state.rules.players[3].life, 42.into());
        assert_eq!(g.state.rules.players[0].life, 40.into());
        recover(&g);
    }
}
#[test]
fn d2cc_synthetic_same_controller_order_and_independent_opponent_predicate() {
    let mut p = prepared(
        &[
            (STUDENT, 0, 0, Zone::Battlefield),
            (STUDENT, 0, 0, Zone::Battlefield),
            (STUDENT, 1, 1, Zone::Battlefield),
            (MINE, 0, 0, Zone::Hand { owner: PlayerId(0) }),
        ],
        TurnStep::PrecombatMain,
    );
    synthetic_relation(&mut p, CasterRelation::Opponent, TypePredicate::Any {});
    let mut g = Game::new(p);
    cast(&mut g, 3);
    assert_eq!(g.state.rules.stack.len(), 2);
    assert_eq!(g.state.rules.stack[1].controller, PlayerId(1));
    recover(&g);
    let mut p = g.prepared.as_ref().clone();
    synthetic_relation(&mut p, CasterRelation::Controller, TypePredicate::Any {});
    let mut g = Game::new(p);
    answer(&mut g, Response::Cast { card: CardId(3) });
    fund(&mut g);
    let Request::OrderTriggers { mut candidates, .. } = settle(&mut g) else {
        panic!("ordering")
    };
    candidates.reverse();
    answer(
        &mut g,
        Response::OrderTriggers {
            order: candidates.clone(),
        },
    );
    placed(&mut g);
    assert_eq!(
        g.state.rules.stack[1..]
            .iter()
            .map(|s| match s.kind {
                StackKind::Triggered { trigger_id, .. } => trigger_id,
                _ => panic!("trigger"),
            })
            .collect::<Vec<_>>(),
        candidates
    );
    recover(&g);
}
#[test]
fn d2cc_synthetic_runtime_boolean_combined_type_positive_and_negative() {
    let filters = [
        (
            TypePredicate::NotType {
                kind: CardKind::Creature,
            },
            false,
        ),
        (
            TypePredicate::Type {
                kind: CardKind::Enchantment,
            },
            true,
        ),
        (
            TypePredicate::All {
                terms: vec![
                    TypePredicate::Type {
                        kind: CardKind::Enchantment,
                    },
                    TypePredicate::Type {
                        kind: CardKind::Creature,
                    },
                ],
            },
            true,
        ),
        (
            TypePredicate::AnyOf {
                terms: vec![
                    TypePredicate::Type {
                        kind: CardKind::Artifact,
                    },
                    TypePredicate::Type {
                        kind: CardKind::Instant,
                    },
                ],
            },
            false,
        ),
    ];
    for (predicate, expected) in filters {
        let mut p = prepared(
            &[
                (STUDENT, 0, 0, Zone::Battlefield),
                (RAM, 0, 0, Zone::Hand { owner: PlayerId(0) }),
            ],
            TurnStep::PrecombatMain,
        );
        synthetic_relation(&mut p, CasterRelation::Controller, predicate);
        let mut g = Game::new(p);
        cast(&mut g, 1);
        assert_eq!(g.state.rules.next_trigger_id, u64::from(expected));
        recover(&g);
    }
}
#[test]
fn d2cc_no_cast_on_land_mana_activation_unpaid_reversal_or_departing_caster() {
    for mode in modes() {
        for undo in [false, true] {
            let mut p = prepared(
                &[
                    (STUDENT, 0, 0, Zone::Battlefield),
                    (MINE, 0, 0, Zone::Hand { owner: PlayerId(0) }),
                    (SWAMP, 0, 0, Zone::Hand { owner: PlayerId(0) }),
                ],
                TurnStep::PrecombatMain,
            );
            // Leave exactly one initial land: the spell cannot be paid in this case.
            let mut seen = false;
            p.config.cards.retain(|c| {
                if c.zone == Zone::Battlefield && c.oracle_id != STUDENT {
                    let keep = !seen;
                    seen = true;
                    keep
                } else {
                    true
                }
            });
            p.profile_digest = digest(&p.config).unwrap();
            let mut g = Game::with_trace(p, mode);
            answer(&mut g, Response::PlayLand { card: CardId(2) });
            settle(&mut g);
            assert_eq!(g.state.rules.next_trigger_id, 0);
            let land = g.object_id(CardId(2)).unwrap();
            answer(&mut g, Response::ActivateMana { object: land });
            settle(&mut g);
            assert_eq!(g.state.rules.next_trigger_id, 0);
            let mut branch = Game::with_trace(g.prepared.as_ref().clone(), mode);
            let original = branch.object_id(CardId(1)).unwrap();
            answer(&mut branch, Response::Cast { card: CardId(1) });
            assert!(matches!(settle(&mut branch), Request::Mana { .. }));
            let other = branch
                .state
                .rules
                .objects
                .values()
                .find(|o| {
                    o.zone == Zone::Battlefield
                        && o.controller == PlayerId(0)
                        && !o.tapped
                        && o.id.card != CardId(0)
                })
                .unwrap()
                .id;
            answer(&mut branch, Response::ActivateMana { object: other });
            answer(&mut branch, Response::FinishMana { undo_mana: undo });
            settle(&mut branch);
            assert_eq!(branch.state.rules.next_trigger_id, 0);
            assert_eq!(branch.object_id(CardId(1)).unwrap(), original);
            assert_eq!(branch.current(other).unwrap().tapped, !undo);
            assert!(
                !branch
                    .events()
                    .iter()
                    .any(|e| matches!(e, Event::SpellCast { .. }))
            );
            recover(&branch);
            // Reachable cancellation: the caster departs in its mana window.
            answer(&mut g, Response::Cast { card: CardId(1) });
            settle(&mut g);
            g.concede(PlayerId(0)).unwrap();
            placed(&mut g);
            assert_eq!(g.state.rules.next_trigger_id, 0);
            recover(&g);
        }
    }
}
#[test]
fn d2cc_no_cast_on_nonmana_activated_invocation_or_its_resolution() {
    // Select the existing complete archive-backed mana-cost activation by data.
    let c = catalog();
    let mage = c
        .definitions
        .iter()
        .find(|d| d.name == "Azure Mage")
        .unwrap()
        .oracle_id
        .clone();
    let mut p = prepared(
        &[
            (STUDENT, 0, 0, Zone::Battlefield),
            (&mage, 0, 0, Zone::Battlefield),
        ],
        TurnStep::PrecombatMain,
    );
    synthetic_relation(&mut p, CasterRelation::AnyPlayer, TypePredicate::Any {});
    let mut g = Game::new(p);
    let object = g.object_id(CardId(1)).unwrap();
    answer(&mut g, Response::ActivateAbility { object, ability: 0 });
    fund(&mut g);
    placed(&mut g);
    assert_eq!(g.state.rules.next_trigger_id, 0);
    resolve(&mut g);
    assert_eq!(g.state.rules.next_trigger_id, 0);
    recover(&g);
}
#[test]
fn d2cc_command_origin_success_tax_and_unpaid_command_reversal() {
    let c = catalog();
    let bounce = c
        .definitions
        .iter()
        .find(|d| d.name == "Unsummon")
        .unwrap()
        .oracle_id
        .clone();
    let mut p = prepared(
        &[
            (STUDENT, 0, 0, Zone::Battlefield),
            (YARGLE, 0, 0, Zone::Command { owner: PlayerId(0) }),
            (&bounce, 1, 1, Zone::Hand { owner: PlayerId(1) }),
        ],
        TurnStep::PrecombatMain,
    );
    p.config.commanders.push(CardId(1));
    p.profile_digest = digest(&p.config).unwrap();
    synthetic_relation(&mut p, CasterRelation::Controller, TypePredicate::Any {});
    let mut g = Game::new(p);
    cast(&mut g, 1);
    let first = g.state.rules.stack[0].source;
    let StackKind::Triggered {
        witness: TriggerEventWitness::SpellCast { cast },
        ..
    } = &g.state.rules.stack[1].kind
    else {
        panic!("command cast")
    };
    assert_eq!(cast.cast_from, Zone::Command { owner: PlayerId(0) });
    assert_eq!(cast.spell.object, first);
    recover(&g);
    resolve(&mut g);
    resolve(&mut g);
    priority(&mut g, 1);
    answer(&mut g, Response::Cast { card: CardId(2) });
    let object = g.object_id(CardId(1)).unwrap();
    answer(
        &mut g,
        Response::Targets {
            objects: vec![object],
        },
    );
    fund(&mut g);
    placed(&mut g);
    for _ in 0..4 {
        answer(&mut g, Response::Pass);
    }
    assert!(matches!(settle(&mut g), Request::CommanderReturn { .. }));
    answer(&mut g, Response::CommanderReturn { to_command: true });
    placed(&mut g);
    priority(&mut g, 0);
    recover(&g);
    answer(&mut g, Response::Cast { card: CardId(1) });
    settle(&mut g);
    let cost = g.top().unwrap().total_cost.clone();
    assert_eq!(cost.generic, 5.into());
    answer(&mut g, Response::FinishMana { undo_mana: true });
    settle(&mut g);
    assert_eq!(g.state.rules.next_trigger_id, 1);
    assert_eq!(
        g.state.rules.objects[&CardId(1)].zone,
        Zone::Command { owner: PlayerId(0) }
    );
    recover(&g);
}
#[test]
fn d2cc_source_controller_owner_actor_departures_pending_and_placed_all_traces() {
    for mode in modes() {
        for pending in [false, true] {
            for departing in [0, 1, 2, 3] {
                let mut p = prepared(
                    &[
                        (STUDENT, 2, 1, Zone::Battlefield),
                        (MINE, 3, 3, Zone::Hand { owner: PlayerId(3) }),
                    ],
                    TurnStep::PrecombatMain,
                );
                synthetic_relation(&mut p, CasterRelation::AnyPlayer, TypePredicate::Any {});
                edit(&mut p, MINE, |d| {
                    d.keywords.push(KeywordAbility::Flash);
                    d.spell.as_mut().unwrap().timing = SpellTiming::Instant;
                });
                let mut g = Game::with_trace(p, mode);
                priority(&mut g, 3);
                answer(&mut g, Response::Cast { card: CardId(1) });
                fund(&mut g);
                if pending {
                    pass_until(&mut g, |g| !g.state.rules.pending_triggers.is_empty());
                } else {
                    placed(&mut g);
                }
                recover(&g);
                g.concede(PlayerId(departing)).unwrap();
                placed(&mut g);
                recover(&g);
                let trigger_count = g
                    .state
                    .rules
                    .stack
                    .iter()
                    .filter(|s| !s.kind.is_spell())
                    .count();
                assert_eq!(trigger_count, usize::from(departing != 1));
                if departing != 1 {
                    resolve(&mut g);
                    assert_eq!(g.state.rules.players[1].life, 42.into());
                    recover(&g);
                }
            }
        }
    }
}
#[test]
fn d2cc_cast_witness_pending_placed_resolving_fragmented_restore_and_coherent_forgery() {
    for mode in modes() {
        let mut p = prepared(
            &[
                (STUDENT, 0, 0, Zone::Battlefield),
                (MINE, 0, 0, Zone::Hand { owner: PlayerId(0) }),
            ],
            TurnStep::PrecombatMain,
        );
        synthetic_relation(&mut p, CasterRelation::AnyPlayer, TypePredicate::Any {});
        let mut g = Game::with_trace(p, mode);
        answer(&mut g, Response::Cast { card: CardId(1) });
        fund(&mut g);
        let (mut pending, mut placed, mut resolving) = (false, false, false);
        for _ in 0..100 {
            recover(&g);
            pending |= !g.state.rules.pending_triggers.is_empty();
            placed |= g.state.rules.stack.len() == 2;
            resolving |= matches!(g.state.rules.frame, Frame::Resolve { .. });
            if !g.state.rules.pending_triggers.is_empty() || g.state.rules.stack.len() == 2 {
                let mut s: serde_json::Value =
                    serde_json::from_str(&g.snapshot().unwrap()).unwrap();
                let w = if !g.state.rules.pending_triggers.is_empty() {
                    &mut s["state"]["pending_triggers"][0]["witness"]
                } else {
                    &mut s["state"]["stack"][1]["kind"]["witness"]
                };
                // Any-player + any-spell still match. Both actor and frozen
                // spell controller agree, so only independent replay proves this lie.
                w["cast"]["caster"] = 1.into();
                w["cast"]["spell"]["controller"] = 1.into();
                s["state_sha256"] = digest(&s["state"]).unwrap().into();
                let forged: Snapshot = serde_json::from_value(s.clone()).unwrap();
                let mut structural = g.fork();
                structural.state = forged.state;
                structural.validate_trigger_state().unwrap();
                assert!(Game::restore(g.prepared.as_ref().clone(), &s.to_string()).is_err());
            }
            if g.state.rules.players[0].life == 42.into() && g.state.rules.stack.len() == 1 {
                break;
            }
            match g.advance(Budget::new(1)) {
                Advance::Suspended { .. } => (),
                Advance::AwaitingDecision(_) => passive(&mut g),
                x => panic!("{x:?}"),
            }
        }
        assert!(pending && placed && resolving);
        assert_eq!(g.state.rules.next_trigger_id, 1);
        recover(&g);
    }
}
#[test]
fn d2cc_counter_original_preserves_cast_trigger_counter_trigger_preserves_spell() {
    for mode in modes() {
        for trigger in [false, true] {
            let c = catalog();
            let counter = c
                .definitions
                .iter()
                .find(|d| d.name == if trigger { "Stifle" } else { "Counterspell" })
                .unwrap()
                .oracle_id
                .clone();
            let mut p = prepared(
                &[
                    (STUDENT, 1, 1, Zone::Battlefield),
                    (MINE, 0, 0, Zone::Hand { owner: PlayerId(0) }),
                    (&counter, 2, 2, Zone::Hand { owner: PlayerId(2) }),
                ],
                TurnStep::PrecombatMain,
            );
            synthetic_relation(
                &mut p,
                CasterRelation::AnyPlayer,
                TypePredicate::Type {
                    kind: CardKind::Artifact,
                },
            );
            let mut g = Game::with_trace(p, mode);
            cast(&mut g, 1);
            let target = g.state.rules.stack[usize::from(trigger)].id;
            priority(&mut g, 2);
            answer(&mut g, Response::Cast { card: CardId(2) });
            answer(
                &mut g,
                Response::StackTargets {
                    entries: vec![target],
                },
            );
            fund(&mut g);
            placed(&mut g);
            recover(&g);
            resolve(&mut g);
            recover(&g);
            assert_eq!(g.state.rules.stack.len(), 1);
            assert_eq!(g.state.rules.stack[0].kind.is_spell(), trigger);
            resolve(&mut g);
            assert_eq!(
                g.state.rules.players[1].life,
                if trigger { 40.into() } else { 42.into() }
            );
            assert_eq!(g.state.rules.next_trigger_id, 1);
            recover(&g);
        }
    }
}
#[test]
fn d2cc_cast_collection_id_and_retained_history_faults_rollback_payment() {
    for mode in modes() {
        for history in [false, true] {
            let mut g = Game::with_trace(
                prepared(
                    &[
                        (STUDENT, 0, 0, Zone::Battlefield),
                        (MINE, 0, 0, Zone::Hand { owner: PlayerId(0) }),
                    ],
                    TurnStep::PrecombatMain,
                ),
                mode,
            );
            answer(&mut g, Response::Cast { card: CardId(1) });
            fund(&mut g);
            // fund leaves the accepted spending input unexecuted. This synthetic
            // boundary isolates failure after payment staging, without a huge game.
            if history {
                let charge = g.history_charge();
                g.state.rules.retained_auxiliary_charge += capacities::HISTORY_LIMIT - charge;
            } else {
                g.state.rules.next_trigger_id = u64::MAX;
            }
            let before = g.state.clone();
            let trace = g.trace.clone();
            assert!(matches!(g.advance(Budget::new(1)), Advance::Fault(_)));
            if history {
                assert!(matches!(
                    g.typed_fault(),
                    Some(RuleFault::HistoryCapacityExceeded { .. })
                ));
            }
            let mut actual = g.state.clone();
            actual.rules.frame = before.rules.frame.clone();
            actual.rules.typed_fault = before.rules.typed_fault.clone();
            assert_eq!(actual, before);
            assert_eq!(g.trace, trace);
        }
    }
}
#[test]
fn d2cc_stack_capacity_fault_at_cast_collection_preserves_payment_and_tax() {
    let mut p = prepared(
        &[
            (STUDENT, 0, 0, Zone::Battlefield),
            (YARGLE, 0, 0, Zone::Command { owner: PlayerId(0) }),
        ],
        TurnStep::PrecombatMain,
    );
    p.config.commanders.push(CardId(1));
    p.profile_digest = digest(&p.config).unwrap();
    synthetic_relation(&mut p, CasterRelation::Controller, TypePredicate::Any {});
    let mut g = Game::with_trace(p, TraceMode::Off);
    answer(&mut g, Response::Cast { card: CardId(1) });
    fund(&mut g);
    pass_until(&mut g, |g| !g.state.rules.pending_triggers.is_empty());
    let t = g.state.rules.pending_triggers[0].clone();
    // Private synthetic saturation of the common mechanical bound; the source
    // program and successful payment path are the same as reachable casts.
    let mut fresh = Game::with_trace(g.prepared.as_ref().clone(), TraceMode::Off);
    // Synthetic prior completed command cast makes the unchanged tax nonzero.
    fresh
        .state
        .rules
        .commanders
        .record_cast(CardId(1), PlayerId(0), Zone::Command { owner: PlayerId(0) })
        .unwrap();
    assert_eq!(
        fresh
            .commander_ledger()
            .tax(CardId(1), PlayerId(0))
            .unwrap(),
        Quantity::from(2)
    );
    answer(&mut fresh, Response::Cast { card: CardId(1) });
    fund(&mut fresh);
    assert_eq!(fresh.top().unwrap().total_cost.generic, Quantity::from(5));
    assert_eq!(
        fresh.top().unwrap().cast_from,
        Zone::Command { owner: PlayerId(0) }
    );
    fresh
        .state
        .rules
        .pending_triggers
        .resize(capacities::STACK_LIMIT - 1, t);
    let mana = fresh.state.rules.players[0].mana.clone();
    let batches = fresh.state.rules.next_batch_id;
    let next_id = fresh.state.rules.next_trigger_id;
    let trace = fresh.trace.clone();
    let journal = fresh.replay_journal();
    let charges = fresh.state.rules.retained_auxiliary_charge;
    let commanders = fresh.state.rules.commanders.clone();
    assert!(matches!(fresh.advance(Budget::new(1)), Advance::Fault(_)));
    assert!(
        matches!(fresh.typed_fault(),Some(RuleFault::StackCapacityExceeded {requested,..}) if *requested==capacities::STACK_LIMIT+1)
    );
    assert_eq!(fresh.state.rules.players[0].mana, mana);
    assert!(fresh.state.rules.stack[0].paid.is_none());
    assert_eq!(fresh.state.rules.next_batch_id, batches);
    assert_eq!(fresh.state.rules.next_trigger_id, next_id);
    assert_eq!(fresh.trace, trace);
    assert_eq!(fresh.replay_journal(), journal);
    assert_eq!(fresh.state.rules.retained_auxiliary_charge, charges);
    assert_eq!(fresh.state.rules.commanders, commanders);
    assert_eq!(
        fresh
            .commander_ledger()
            .tax(CardId(1), PlayerId(0))
            .unwrap(),
        Quantity::from(2)
    );
}
#[test]
fn d2cc_capability_excluded_from_hosting_and_search() {
    let c = catalog();
    for (search, hosted, feature) in [
        (true, false, Feature::Search),
        (false, true, Feature::Hosting),
    ] {
        let mut p = match_profile("commander-foundations-v1").unwrap().clone();
        p.search = search;
        p.hosted = hosted;
        assert!(
            matches!(p.validate_requested_features(&c),Err(ProfileError::UnreviewedCapabilities {feature:f,capabilities}) if f==feature && capabilities.contains(&Capability::CastTriggerV1))
        );
    }
}
#[test]
fn d2cc_actor_commander_draw_suspension_departure_and_decline_all_traces() {
    for mode in modes() {
        for departure in [false, true] {
            let mut p = prepared(
                &[
                    (STUDENT, 2, 1, Zone::Battlefield),
                    (MINE, 0, 0, Zone::Hand { owner: PlayerId(0) }),
                ],
                TurnStep::PrecombatMain,
            );
            synthetic_relation(&mut p, CasterRelation::AnyPlayer, TypePredicate::Any {});
            edit(&mut p, STUDENT, |d| {
                d.triggered[0].program.instructions = vec![
                    Instruction::DrawCards {
                        recipient: Some(LifeRecipient::EventPlayer),
                        count: 1.into(),
                        source_block: 0,
                    },
                    Instruction::DrawCards {
                        recipient: Some(LifeRecipient::EventPlayer),
                        count: 1.into(),
                        source_block: 0,
                    },
                    Instruction::GainLife {
                        recipient: LifeRecipient::Controller,
                        amount: 3.into(),
                        source_block: 0,
                    },
                ]
            });
            let first = p
                .config
                .cards
                .iter_mut()
                .find(|c| c.zone == Zone::Library { owner: PlayerId(0) })
                .unwrap();
            let commander = first.id;
            first.oracle_id = YARGLE.into();
            p.config.commanders.push(commander);
            p.profile_digest = digest(&p.config).unwrap();
            let mut g = Game::with_trace(p, mode);
            cast(&mut g, 1);
            for _ in 0..4 {
                answer(&mut g, Response::Pass);
            }
            assert!(matches!(
                settle(&mut g),
                Request::CommanderReturn {
                    player: PlayerId(0),
                    ..
                }
            ));
            recover(&g);
            if departure {
                g.concede(PlayerId(0)).unwrap();
            } else {
                answer(&mut g, Response::CommanderReturn { to_command: false });
            }
            placed(&mut g);
            assert_eq!(g.state.rules.players[1].life, 43.into());
            if !departure {
                assert_eq!(
                    g.state.rules.objects[&commander].zone,
                    Zone::Hand { owner: PlayerId(0) }
                );
                assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), 6);
            }
            recover(&g);
        }
    }
}
#[test]
fn d2cc_exact_source_incarnation_after_bounce_flash_recast() {
    let c = catalog();
    let bounce = c
        .definitions
        .iter()
        .find(|d| d.name == "Unsummon")
        .unwrap()
        .oracle_id
        .clone();
    let mut p = prepared(
        &[
            (STUDENT, 0, 0, Zone::Battlefield),
            (MINE, 0, 0, Zone::Hand { owner: PlayerId(0) }),
            (&bounce, 1, 1, Zone::Hand { owner: PlayerId(1) }),
        ],
        TurnStep::PrecombatMain,
    );
    // Preserve white mana for the later recast after the artifact's generic
    // payment. The fixture uses actual source-backed Plains mana abilities.
    for _ in 0..2 {
        p.config.cards.push(CardSetup {
            id: CardId(p.config.cards.len() as u32),
            oracle_id: PLAINS.into(),
            owner: PlayerId(0),
            controller: PlayerId(0),
            zone: Zone::Battlefield,
        });
    }
    p.profile_digest = digest(&p.config).unwrap();
    // Synthetic source-counter program plus flash; ordinary bounce, reentry,
    // payment and exact-incarnation lookup remain real accepted-input paths.
    edit(&mut p, STUDENT, |d| {
        d.keywords.push(KeywordAbility::Flash);
        d.spell.as_mut().unwrap().timing = SpellTiming::Instant;
        d.triggered[0].program.instructions.insert(
            0,
            Instruction::PutPlusOneCounters {
                on: CounterRecipient::Source,
                amount: 1.into(),
                source_block: 0,
            },
        );
    });
    let mut g = Game::new(p);
    let original = g.object_id(CardId(0)).unwrap();
    cast(&mut g, 1);
    priority(&mut g, 1);
    answer(&mut g, Response::Cast { card: CardId(2) });
    answer(
        &mut g,
        Response::Targets {
            objects: vec![original],
        },
    );
    fund(&mut g);
    placed(&mut g);
    resolve(&mut g);
    priority(&mut g, 0);
    cast(&mut g, 0);
    resolve(&mut g);
    let successor = g.object_id(CardId(0)).unwrap();
    assert_ne!(original, successor);
    assert_eq!(g.state.rules.next_trigger_id, 1);
    recover(&g);
    resolve(&mut g);
    assert_eq!(g.state.rules.players[0].life, 42.into());
    assert_eq!(
        g.state.rules.objects[&successor.card].plus_one_counters,
        Quantity::zero()
    );
    recover(&g);
}
#[test]
fn d2cc_d2cb_status_tap_then_untap_current_and_cached_resolution_forgery() {
    for mode in modes() {
        let mut g = Game::with_trace(
            prepared(&[(MINE, 2, 1, Zone::Battlefield)], TurnStep::Upkeep),
            mode,
        );
        to_step(&mut g, 1, TurnStep::Draw);
        recover(&g);
        let source = g.object_id(CardId(0)).unwrap();
        // Explicit synthetic event-domain fixture: untap spells are not added.
        g.commit(ProposedEvent::Tap { object: source }, BatchCause::Fixture)
            .unwrap();
        let tapped_stack = g.top().unwrap().clone();
        assert!(matches!(
            g.check_resolution_condition(&tapped_stack).unwrap(),
            Some(InterveningCheck::SourceStatus {
                tapped: true,
                passed: false,
                information: SourceInformation::Current,
                ..
            })
        ));
        g.commit(ProposedEvent::Untap { object: source }, BatchCause::Fixture)
            .unwrap();
        let stack = g.top().unwrap().clone();
        assert!(matches!(
            g.check_resolution_condition(&stack).unwrap(),
            Some(InterveningCheck::SourceStatus {
                tapped: false,
                passed: true,
                information: SourceInformation::Current,
                ..
            })
        ));
        // A reachable cached resolution is separately checked by full replay.
        let mut g = Game::with_trace(g.prepared.as_ref().clone(), mode);
        to_step(&mut g, 1, TurnStep::Draw);
        for _ in 0..4 {
            answer(&mut g, Response::Pass);
        }
        pass_until(
            &mut g,
            |g| matches!(&g.state.rules.frame,Frame::Resolve {frame} if frame.condition_check.is_some()),
        );
        recover(&g);
        let mut s: serde_json::Value = serde_json::from_str(&g.snapshot().unwrap()).unwrap();
        let check = &mut s["state"]["frame"]["frame"]["condition_check"];
        check["tapped"] = true.into();
        check["passed"] = false.into();
        s["state_sha256"] = digest(&s["state"]).unwrap().into();
        let forged: Snapshot = serde_json::from_value(s.clone()).unwrap();
        let Frame::Resolve { frame } = &forged.state.rules.frame else {
            panic!("cached frame")
        };
        g.validate_condition_cache(frame).unwrap();
        assert!(Game::restore(g.prepared.as_ref().clone(), &s.to_string()).is_err());
    }
}
#[test]
fn d2cc_additive_old_batch_witness_bytes_and_strict_identity_domains() {
    let id = OccurrenceId {
        batch: 17,
        path: vec![0, 1],
    };
    let wrapped = TriggerOccurrenceId::Batch(id.clone());
    assert_eq!(
        serde_json::to_string(&id).unwrap(),
        serde_json::to_string(&wrapped).unwrap()
    );
    assert_eq!(
        serde_json::from_str::<TriggerOccurrenceId>(&serde_json::to_string(&id).unwrap()).unwrap(),
        wrapped
    );
    for raw in [
        r#"{"cast":1,"batch":1,"path":[]}"#,
        r#"{"cast":1,"unknown":true}"#,
        r#"{"batch":1,"path":[],"unknown":true}"#,
    ] {
        assert!(serde_json::from_str::<TriggerOccurrenceId>(raw).is_err());
    }
}
#[test]
fn d2cc_archived_kambal_frozen_opponent_actor_life_and_departure() {
    let c = catalog();
    let kambal = c
        .definitions
        .iter()
        .find(|d| d.name == "Kambal, Consul of Allocation")
        .unwrap()
        .oracle_id
        .clone();
    for mode in modes() {
        for leave in [false, true] {
            let mut g = Game::with_trace(
                prepared(
                    &[
                        (&kambal, 2, 1, Zone::Battlefield),
                        (MINE, 0, 0, Zone::Hand { owner: PlayerId(0) }),
                    ],
                    TurnStep::PrecombatMain,
                ),
                mode,
            );
            cast(&mut g, 1);
            if leave {
                g.concede(PlayerId(0)).unwrap();
                placed(&mut g);
            }
            recover(&g);
            resolve(&mut g);
            assert_eq!(g.state.rules.players[1].life, 42.into());
            if !leave {
                assert_eq!(g.state.rules.players[0].life, 38.into());
            }
            recover(&g);
        }
    }
}
#[test]
fn d2cc_archived_targeted_counter_program_and_instant_or_sorcery_source_counter() {
    let c = catalog();
    let visitor = c
        .definitions
        .iter()
        .find(|d| d.name == "Generous Visitor")
        .unwrap()
        .oracle_id
        .clone();
    let pyre = c
        .definitions
        .iter()
        .find(|d| d.name == "Pyre Hound")
        .unwrap()
        .oracle_id
        .clone();
    let bounce = c
        .definitions
        .iter()
        .find(|d| d.name == "Unsummon")
        .unwrap()
        .oracle_id
        .clone();
    let mut g = Game::new(prepared(
        &[
            (&visitor, 0, 0, Zone::Battlefield),
            (STUDENT, 1, 1, Zone::Battlefield),
            (ARENA, 0, 0, Zone::Hand { owner: PlayerId(0) }),
        ],
        TurnStep::PrecombatMain,
    ));
    answer(&mut g, Response::Cast { card: CardId(2) });
    fund(&mut g);
    let target = g.object_id(CardId(1)).unwrap();
    assert!(matches!(settle(&mut g), Request::Targets { .. }));
    answer(
        &mut g,
        Response::Targets {
            objects: vec![target],
        },
    );
    placed(&mut g);
    recover(&g);
    resolve(&mut g);
    assert_eq!(
        g.state.rules.objects[&CardId(1)].plus_one_counters,
        1.into()
    );
    recover(&g);
    let mut g = Game::new(prepared(
        &[
            (&pyre, 0, 0, Zone::Battlefield),
            (STUDENT, 1, 1, Zone::Battlefield),
            (&bounce, 0, 0, Zone::Hand { owner: PlayerId(0) }),
        ],
        TurnStep::PrecombatMain,
    ));
    answer(&mut g, Response::Cast { card: CardId(2) });
    let target = g.object_id(CardId(1)).unwrap();
    answer(
        &mut g,
        Response::Targets {
            objects: vec![target],
        },
    );
    fund(&mut g);
    placed(&mut g);
    resolve(&mut g);
    assert_eq!(
        g.state.rules.objects[&CardId(0)].plus_one_counters,
        1.into()
    );
    recover(&g);
}
#[test]
fn d2cc_archived_sythis_cast_matching_own_types_functions_only_after_entry() {
    for mode in modes() {
        let mut g = Game::with_trace(
            prepared(
                &[
                    (SYTHIS, 0, 0, Zone::Hand { owner: PlayerId(0) }),
                    (ARENA, 0, 0, Zone::Hand { owner: PlayerId(0) }),
                ],
                TurnStep::PrecombatMain,
            ),
            mode,
        );
        cast(&mut g, 0);
        // Sythis itself is an enchantment spell. Its ordinary ability functions
        // only on the battlefield, so this matching self cast does not trigger.
        assert_eq!(g.state.rules.next_trigger_id, 0);
        recover(&g);
        resolve(&mut g);
        assert_eq!(g.state.rules.next_trigger_id, 0);
        cast(&mut g, 1);
        assert_eq!(g.state.rules.next_trigger_id, 1);
        recover(&g);
    }
}
