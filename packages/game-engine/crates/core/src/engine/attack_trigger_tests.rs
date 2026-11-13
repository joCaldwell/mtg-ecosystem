//! Archived programs run unchanged in private scenarios. Synthetic edits are
//! explicitly labeled; no extra legal hosting/search profile is admitted.
use super::*;
const SWAMP: &str = "56719f6a-1a6c-4c0a-8d21-18f7d7350b68";
fn catalog() -> Catalog {
    let mut c = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap();
    c.definitions.extend(
        Catalog::read(include_str!(
            "../../../../tests/fixtures/cards-data-d2cd/expected-ir.json"
        ))
        .unwrap()
        .definitions,
    );
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

const CAUSE: &str = "218f835b-4195-4ee8-a4ef-e5ba58fa5374";
const LARCENIST: &str = "eb0e6f63-2861-4392-ac0e-64380fbbd8ae";
const THIEF: &str = "5905c55a-f1d1-4021-a9af-f85b02ae21f3";
const CONQUISTADOR: &str = "26b077df-7a05-459a-b5ff-bf27767bb519";
const SYPHONER: &str = "b81a748b-f7a6-4ec4-8b18-40a1819745f6";
const HERALD: &str = "cb97da84-1d13-4795-b68a-2bf111a50067";
const HELLKITE: &str = "27596982-f4ab-488f-a974-5b0404438504";
const REGISAUR: &str = "3d563149-7d4e-43e5-b701-28e3212051ec";
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

fn object(g: &Game, card: u32) -> ObjectId {
    g.object_id(CardId(card)).unwrap()
}
fn attackers(g: &mut Game) {
    to_step(g, 1, TurnStep::DeclareAttackers);
    assert!(matches!(settle(g), Request::Attackers { .. }));
}
fn declare(g: &mut Game, attacks: &[(u32, u8)]) {
    attackers(g);
    let attacks = attacks
        .iter()
        .map(|(c, p)| (object(g, *c), PlayerId(*p)))
        .collect();
    answer(g, Response::Attackers { attacks });
}
fn library(g: &Game) -> usize {
    g.state.rules.libraries[&PlayerId(0)].len()
}
fn predicate(p: &mut Prepared, relation: AttackerRelation, types: TypePredicate) {
    edit(p, CAUSE, |d| {
        d.triggered[0].condition = TriggerCondition::CreatureAttacked {
            attacker: AttackSubject::Filter {
                controller: relation,
                types,
            },
        }
    });
}
#[test]
fn d2cd_archived_whole_draw_life_keyword_programs_all_traces() {
    for mode in modes() {
        for (card, draws, life, opponent) in [
            (LARCENIST, 1, 40, 40),
            (THIEF, 1, 39, 40),
            (CONQUISTADOR, 0, 40, 39),
            (SYPHONER, 0, 41, 39),
            (HERALD, 0, 42, 40),
            (HELLKITE, 2, 40, 40),
        ] {
            let mut g = Game::with_trace(prepared(&[(card, 0, 0, Zone::Battlefield)]), mode);
            let before = library(&g);
            declare(&mut g, &[(0, 1)]);
            placed(&mut g);
            assert_eq!(g.state.rules.stack.len(), 1);
            assert_eq!(g.state.rules.next_trigger_id, 1);
            assert_eq!(g.state.rules.turn.step, TurnStep::DeclareAttackers);
            assert!(g.current(object(&g, 0)).unwrap().tapped);
            recover(&g);
            resolve(&mut g);
            assert_eq!(library(&g), before - draws);
            assert_eq!(g.state.rules.players[0].life, life.into());
            for i in 1..4 {
                assert_eq!(g.state.rules.players[i].life, opponent.into());
            }
            assert_eq!(g.state.rules.next_trigger_id, 1);
            recover(&g);
        }
    }
}
#[test]
fn d2cd_n_attackers_split_defenders_multiple_observers_apnap_actual_batch_members() {
    for mode in modes() {
        let mut g = Game::with_trace(
            prepared(&[
                (CAUSE, 0, 0, Zone::Battlefield),
                (CAUSE, 2, 1, Zone::Battlefield),
                (CAUSE, 3, 3, Zone::Battlefield),
                (LARCENIST, 2, 0, Zone::Battlefield),
                (LARCENIST, 0, 0, Zone::Battlefield),
            ]),
            mode,
        );
        declare(&mut g, &[(3, 1), (4, 2)]);
        pass_until(&mut g, |g| g.state.rules.pending_triggers.len() == 8);
        let mut distinct = BTreeSet::new();
        let mut batch = None;
        for t in &g.state.rules.pending_triggers {
            let evidence = serde_json::to_value(t).unwrap();
            let event: TriggerOccurrenceId =
                serde_json::from_value(evidence["event"].clone()).unwrap();
            let witness: TriggerEventWitness =
                serde_json::from_value(evidence["witness"].clone()).unwrap();
            let TriggerOccurrenceId::Batch(id) = &event else {
                panic!("real declaration batch")
            };
            let TriggerEventWitness::CreatureAttacked {
                attacker,
                defender_player_id,
            } = &witness
            else {
                panic!("attack witness")
            };
            assert_eq!(attacker.controller, PlayerId(0));
            assert!(!attacker.tapped);
            assert_eq!(
                *defender_player_id,
                if attacker.object.card == CardId(3) {
                    PlayerId(1)
                } else {
                    PlayerId(2)
                }
            );
            assert!(distinct.insert((t.source, t.ability, attacker.object)));
            if let Some(prior) = &batch {
                assert_eq!(id, prior);
            } else {
                batch = Some(id.clone());
            }
        }
        assert_eq!(distinct.len(), 8);
        recover(&g);
        placed(&mut g);
        assert_eq!(
            g.state
                .rules
                .stack
                .iter()
                .map(|s| s.controller.0)
                .collect::<Vec<_>>(),
            vec![0, 0, 0, 0, 1, 1, 3, 3]
        );
        // Detailed trace independently ties all member witnesses to the actual
        // emitted declaration occurrence; no synthetic paths are minted.
        if mode == TraceMode::Detailed {
            let declaration = g
                .trace
                .records
                .iter()
                .find_map(|r| match &r.record {
                    EvidenceRecord::Batch { batch } => batch
                        .occurrences
                        .iter()
                        .find(|o| matches!(o.actual, EventOutcome::AttackersDeclared { .. })),
                    _ => None,
                })
                .unwrap();
            assert_eq!(Some(&declaration.id), batch.as_ref());
            assert!(
                matches!(&declaration.actual,EventOutcome::AttackersDeclared {attacks} if attacks.len()==2)
            );
        }
        while !g.state.rules.stack.is_empty() {
            resolve(&mut g);
        }
        assert_eq!(g.state.rules.players[0].life, 42.into());
        assert_eq!(g.state.rules.players[1].life, 42.into());
        assert_eq!(g.state.rules.players[3].life, 42.into());
        assert_eq!(g.state.rules.next_trigger_id, 8);
        recover(&g);
    }
}
#[test]
fn d2cd_empty_declaration_has_no_occurrences_and_vigilance_is_not_a_tap_trigger() {
    for mode in modes() {
        for empty in [false, true] {
            let mut p = prepared(&[
                (HERALD, 0, 0, Zone::Battlefield),
                (CAUSE, 1, 1, Zone::Battlefield),
            ]);
            // Explicit synthetic intrinsic vigilance on the exact archive program.
            edit(&mut p, HERALD, |d| {
                d.keywords.push(KeywordAbility::Vigilance)
            });
            let mut g = Game::with_trace(p, mode);
            declare(&mut g, if empty { &[] } else { &[(0, 1)] });
            placed(&mut g);
            assert!(!g.current(object(&g, 0)).unwrap().tapped);
            assert_eq!(g.state.rules.next_trigger_id, if empty { 0 } else { 2 });
            assert_eq!(g.state.rules.stack.len(), if empty { 0 } else { 2 });
            recover(&g);
        }
    }
}
#[test]
fn d2cd_self_exact_source_other_copy_and_not_owner_or_active_player_relation() {
    for mode in modes() {
        for relation in [
            AttackerRelation::Controller,
            AttackerRelation::Opponent,
            AttackerRelation::AnyPlayer,
        ] {
            let mut p = prepared(&[
                (CAUSE, 0, 1, Zone::Battlefield),
                (LARCENIST, 1, 0, Zone::Battlefield),
                (LARCENIST, 0, 0, Zone::Battlefield),
            ]);
            predicate(&mut p, relation, TypePredicate::Any {});
            let mut g = Game::with_trace(p, mode);
            declare(&mut g, &[(1, 2)]);
            placed(&mut g);
            assert_eq!(
                g.state
                    .rules
                    .stack
                    .iter()
                    .filter(|s| s.source.card == CardId(1))
                    .count(),
                1
            );
            assert_eq!(
                g.state
                    .rules
                    .stack
                    .iter()
                    .filter(|s| s.source.card == CardId(2))
                    .count(),
                0
            );
            assert_eq!(
                g.state
                    .rules
                    .stack
                    .iter()
                    .filter(|s| s.source.card == CardId(0))
                    .count(),
                usize::from(relation != AttackerRelation::Controller)
            );
            recover(&g);
        }
    }
}
#[test]
fn d2cd_full_type_intersection_negation_and_explicit_or_runtime() {
    for mode in modes() {
        for (types, count) in [
            (
                TypePredicate::All {
                    terms: vec![
                        TypePredicate::Type {
                            kind: CardKind::Artifact,
                        },
                        TypePredicate::Type {
                            kind: CardKind::Creature,
                        },
                    ],
                },
                1,
            ),
            (
                TypePredicate::All {
                    terms: vec![
                        TypePredicate::Type {
                            kind: CardKind::Artifact,
                        },
                        TypePredicate::Type {
                            kind: CardKind::Enchantment,
                        },
                    ],
                },
                0,
            ),
            (
                TypePredicate::AnyOf {
                    terms: vec![
                        TypePredicate::Type {
                            kind: CardKind::Artifact,
                        },
                        TypePredicate::Type {
                            kind: CardKind::Enchantment,
                        },
                    ],
                },
                2,
            ),
            (
                TypePredicate::NotType {
                    kind: CardKind::Artifact,
                },
                1,
            ),
        ] {
            let mut p = prepared(&[
                (CAUSE, 1, 1, Zone::Battlefield),
                (HERALD, 0, 0, Zone::Battlefield),
                (LARCENIST, 0, 0, Zone::Battlefield),
            ]);
            // Explicit synthetic type additions exercise has_type, not kind equality.
            edit(&mut p, HERALD, |d| {
                d.characteristics.additional_types.push(CardKind::Artifact)
            });
            edit(&mut p, LARCENIST, |d| {
                d.characteristics
                    .additional_types
                    .push(CardKind::Enchantment)
            });
            predicate(&mut p, AttackerRelation::AnyPlayer, types);
            let mut g = Game::with_trace(p, mode);
            declare(&mut g, &[(1, 2), (2, 3)]);
            placed(&mut g);
            assert_eq!(
                g.state
                    .rules
                    .stack
                    .iter()
                    .filter(|s| s.source.card == CardId(0))
                    .count(),
                count
            );
            recover(&g);
        }
    }
}
#[test]
fn d2cd_invalid_duplicate_ineligible_declarations_are_atomic() {
    for mode in modes() {
        let mut g = Game::with_trace(
            prepared(&[
                (LARCENIST, 0, 0, Zone::Battlefield),
                (HERALD, 1, 1, Zone::Battlefield),
                (CAUSE, 0, 0, Zone::Battlefield),
            ]),
            mode,
        );
        attackers(&mut g);
        let a = object(&g, 0);
        let b = object(&g, 1);
        let enchantment = object(&g, 2);
        for attacks in [
            vec![(a, PlayerId(1)), (a, PlayerId(2))],
            vec![(b, PlayerId(0))],
            vec![(enchantment, PlayerId(1))],
            vec![(a, PlayerId(0))],
            vec![(a, PlayerId(4))],
        ] {
            let r = settle(&mut g);
            let before = g.state.clone();
            let trace = g.trace.clone();
            let journal = g.replay_journal();
            assert!(
                g.respond(r.player(), r.decision_id(), Response::Attackers { attacks })
                    .is_err()
            );
            assert_eq!(g.state, before);
            assert_eq!(g.trace, trace);
            assert_eq!(g.replay_journal(), journal);
        }
        recover(&g);
        answer(
            &mut g,
            Response::Attackers {
                attacks: vec![(a, PlayerId(1))],
            },
        );
        placed(&mut g);
        assert_eq!(g.state.rules.next_trigger_id, 2);
        recover(&g);
    }
}
#[test]
fn d2cd_pending_placed_resolving_restore_replay_forgery_and_independent_clone() {
    for mode in modes() {
        let mut g = Game::with_trace(
            prepared(&[
                (CAUSE, 1, 1, Zone::Battlefield),
                (YARGLE, 0, 0, Zone::Battlefield),
            ]),
            mode,
        );
        declare(&mut g, &[(1, 2)]);
        let (mut pending, mut placed, mut resolving) = (false, false, false);
        for _ in 0..150 {
            recover(&g);
            pending |= !g.state.rules.pending_triggers.is_empty();
            placed |= !g.state.rules.stack.is_empty();
            resolving |= matches!(g.state.rules.frame, Frame::Resolve { .. });
            if !g.state.rules.pending_triggers.is_empty()
                || !g.state.rules.stack.is_empty()
                || matches!(g.state.rules.frame, Frame::Resolve { .. })
            {
                let mut s: serde_json::Value =
                    serde_json::from_str(&g.snapshot().unwrap()).unwrap();
                let w = if !g.state.rules.pending_triggers.is_empty() {
                    &mut s["state"]["pending_triggers"][0]["witness"]
                } else if !g.state.rules.stack.is_empty() {
                    &mut s["state"]["stack"][0]["kind"]["witness"]
                } else {
                    &mut s["state"]["frame"]["frame"]["spell"]["kind"]["witness"]
                };
                // Both defenders are legal and the observer matches either.
                // Structural checks deliberately cannot prove the occurrence.
                w["defender_player_id"] = 3.into();
                // A resolving entry may still be the stack top. Forge both
                // copies coherently so top-binding is not the rejection reason.
                if matches!(g.state.rules.frame, Frame::Resolve { .. }) {
                    s["state"]["frame"]["frame"]["spell"]["kind"]["witness"]["defender_player_id"] =
                        3.into();
                }
                s["state_sha256"] = digest(&s["state"]).unwrap().into();
                let forged: Snapshot = serde_json::from_value(s.clone()).unwrap();
                let mut structural = g.fork();
                structural.state = forged.state;
                structural.validate_trigger_state().unwrap();
                if let Frame::Resolve { frame } = &structural.state.rules.frame {
                    structural.validate_resolving(frame).unwrap();
                }
                assert!(Game::restore(g.prepared.as_ref().clone(), &s.to_string()).is_err());
            }
            if g.state.rules.players[1].life == 41.into() {
                break;
            }
            match g.advance(Budget::new(1)) {
                Advance::Suspended { .. } => (),
                Advance::AwaitingDecision(_) => passive(&mut g),
                x => panic!("{x:?}"),
            }
        }
        assert!(pending && placed && resolving);
        recover(&g);
        let mut independent = g.fork();
        let original = g.state.clone();
        independent.concede(PlayerId(3)).unwrap();
        let _ = settle(&mut independent);
        assert_eq!(g.state, original);
        recover(&independent);
    }
}
#[test]
fn d2cd_attacker_source_owner_controller_defender_departures_all_traces() {
    for mode in modes() {
        for pending in [false, true] {
            for departing in 0..4 {
                let mut g = Game::with_trace(
                    prepared(&[
                        (CAUSE, 2, 1, Zone::Battlefield),
                        (YARGLE, 3, 0, Zone::Battlefield),
                    ]),
                    mode,
                );
                declare(&mut g, &[(1, 2)]);
                if pending {
                    pass_until(&mut g, |g| !g.state.rules.pending_triggers.is_empty());
                } else {
                    placed(&mut g);
                }
                recover(&g);
                g.concede(PlayerId(departing)).unwrap();
                placed(&mut g);
                recover(&g);
                assert_eq!(g.state.rules.stack.len(), usize::from(departing != 1));
                if departing != 1 {
                    resolve(&mut g);
                    assert_eq!(g.state.rules.players[1].life, 41.into());
                    recover(&g);
                }
            }
        }
    }
}
#[test]
fn d2cd_attacker_source_bounce_flash_reentry_keeps_original_invocation() {
    for mode in modes() {
        let bounce = catalog()
            .definitions
            .iter()
            .find(|d| d.name == "Unsummon")
            .unwrap()
            .oracle_id
            .clone();
        let mut p = prepared(&[
            (LARCENIST, 0, 0, Zone::Battlefield),
            (&bounce, 1, 1, Zone::Hand { owner: PlayerId(1) }),
        ]);
        // Explicit synthetic flash allows a real reentry before the old attack
        // trigger resolves; neither entry nor successful casting is an attack.
        edit(&mut p, LARCENIST, |d| {
            d.keywords.push(KeywordAbility::Flash);
            d.spell.as_mut().unwrap().timing = SpellTiming::Instant;
        });
        let mut g = Game::with_trace(p, mode);
        let original = object(&g, 0);
        declare(&mut g, &[(0, 2)]);
        placed(&mut g);
        priority(&mut g, 1);
        answer(&mut g, Response::Cast { card: CardId(1) });
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
        let successor = object(&g, 0);
        assert_ne!(successor, original);
        assert_eq!(g.state.rules.next_trigger_id, 1);
        assert!(g.state.rules.combat.is_empty());
        let StackKind::Triggered {
            witness: TriggerEventWitness::CreatureAttacked { attacker, .. },
            ..
        } = &g.top().unwrap().kind
        else {
            panic!("attack witness")
        };
        assert_eq!(attacker.object, original);
        assert_eq!(g.top().unwrap().source, original);
        let before = library(&g);
        recover(&g);
        resolve(&mut g);
        assert_eq!(library(&g), before - 1);
        recover(&g);
    }
}
fn target_player(g: &mut Game, player: u8) {
    assert!(matches!(settle(g), Request::EntityTargets { .. }));
    answer(
        g,
        Response::EntityTargets {
            entities: vec![TargetEntity::Player {
                player: PlayerId(player),
            }],
        },
    );
    placed(g);
}
#[test]
fn d2cd_targeted_attack_damage_choice_illegal_response_and_departed_source_lki() {
    for mode in modes() {
        let mut p = prepared(&[(REGISAUR, 3, 0, Zone::Battlefield)]);
        // Explicit synthetic lifelink verifies the departure-refreshed source's
        // damage characteristics and keyword facts independently of event facts.
        edit(&mut p, REGISAUR, |d| {
            d.keywords.push(KeywordAbility::Lifelink)
        });
        let mut g = Game::with_trace(p, mode);
        declare(&mut g, &[(0, 1)]);
        let r = settle(&mut g);
        assert!(matches!(r, Request::EntityTargets { .. }));
        let before = g.state.clone();
        assert!(
            g.respond(
                r.player(),
                r.decision_id(),
                Response::EntityTargets {
                    entities: vec![TargetEntity::Player {
                        player: PlayerId(9)
                    }]
                }
            )
            .is_err()
        );
        assert_eq!(g.state, before);
        recover(&g);
        target_player(&mut g, 2);
        let original = g.top().unwrap().source;
        g.concede(PlayerId(3)).unwrap();
        placed(&mut g);
        assert!(!g.state.rules.objects.contains_key(&CardId(0)));
        let fact = g.top().unwrap().source_facts.clone().unwrap();
        assert_eq!(fact.object, original);
        assert!(fact.tapped);
        assert!(fact.keywords.contains(&KeywordAbility::Lifelink));
        let StackKind::Triggered {
            witness: TriggerEventWitness::CreatureAttacked { attacker, .. },
            ..
        } = &g.top().unwrap().kind
        else {
            panic!("event")
        };
        assert!(!attacker.tapped);
        recover(&g);
        resolve(&mut g);
        assert_eq!(g.state.rules.players[2].life, 39.into());
        assert_eq!(g.state.rules.players[0].life, 41.into());
        recover(&g);
    }
}
#[test]
fn d2cd_illegal_target_or_stifled_attack_trigger_does_not_resolve_damage() {
    for mode in modes() {
        for stifle in [false, true] {
            let counter = catalog()
                .definitions
                .iter()
                .find(|d| d.name == "Stifle")
                .unwrap()
                .oracle_id
                .clone();
            let mut g = Game::with_trace(
                prepared(&[
                    (REGISAUR, 0, 0, Zone::Battlefield),
                    (&counter, 1, 1, Zone::Hand { owner: PlayerId(1) }),
                ]),
                mode,
            );
            declare(&mut g, &[(0, 2)]);
            target_player(&mut g, 3);
            if stifle {
                let id = g.top().unwrap().id;
                priority(&mut g, 1);
                answer(&mut g, Response::Cast { card: CardId(1) });
                answer(&mut g, Response::StackTargets { entries: vec![id] });
                fund(&mut g);
                placed(&mut g);
                resolve(&mut g);
                assert!(g.state.rules.stack.is_empty());
            } else {
                g.concede(PlayerId(3)).unwrap();
                placed(&mut g);
                resolve(&mut g);
            }
            assert_eq!(g.state.rules.next_trigger_id, 1);
            assert_eq!(g.state.rules.players[2].life, 40.into());
            assert_eq!(g.state.rules.combat.len(), 1);
            recover(&g);
        }
    }
}
#[test]
fn d2cd_no_tap_entry_or_current_attacking_status_as_occurrence() {
    for mode in modes() {
        let mut g = Game::with_trace(
            prepared(&[
                (HERALD, 0, 0, Zone::Battlefield),
                (CAUSE, 1, 1, Zone::Battlefield),
            ]),
            mode,
        );
        let _ = settle(&mut g);
        let source = object(&g, 0);
        // Explicit event-domain fixture, not a claim of reachable put-attacking
        // semantics: changing combat status has no trigger collector/event.
        g.commit(ProposedEvent::Tap { object: source }, BatchCause::Fixture)
            .unwrap();
        g.state.rules.combat.push(combat::Attacker {
            attacker: source,
            defender: PlayerId(2),
            blocked: false,
            blockers: vec![],
        });
        assert_eq!(g.state.rules.next_trigger_id, 0);
        assert!(g.state.rules.pending_triggers.is_empty());
        g.state.rules.combat.clear();
        let EventOutcome::Moved { movement } = g
            .commit(
                ProposedEvent::Move {
                    object: source,
                    destination: Zone::Exile,
                },
                BatchCause::Fixture,
            )
            .unwrap()
        else {
            panic!("move")
        };
        g.commit(
            ProposedEvent::Move {
                object: movement.successor,
                destination: Zone::Battlefield,
            },
            BatchCause::Fixture,
        )
        .unwrap();
        assert_eq!(g.state.rules.next_trigger_id, 0);
        assert!(g.state.rules.pending_triggers.is_empty());
    }
}
#[test]
fn d2cd_declaration_trigger_id_and_history_fault_rollback_taps_combat_and_pending() {
    for mode in modes() {
        for history in [false, true] {
            let mut g = Game::with_trace(
                prepared(&[
                    (CAUSE, 1, 1, Zone::Battlefield),
                    (HERALD, 0, 0, Zone::Battlefield),
                ]),
                mode,
            );
            declare(&mut g, &[(1, 2)]);
            // Explicit synthetic resource fault at the accepted declaration boundary.
            if history {
                let charge = g.history_charge();
                g.state.rules.retained_auxiliary_charge += capacities::HISTORY_LIMIT - charge;
            } else {
                g.state.rules.next_trigger_id = u64::MAX;
            }
            let before = g.state.clone();
            let trace = g.trace.clone();
            let journal = g.replay_journal();
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
            assert_eq!(g.replay_journal(), journal);
        }
    }
}
#[test]
fn d2cd_stack_capacity_fault_rolls_back_entire_declaration_after_first_match() {
    let p = prepared(&[
        (CAUSE, 1, 1, Zone::Battlefield),
        (HERALD, 0, 0, Zone::Battlefield),
    ]);
    let mut sample = Game::with_trace(p.clone(), TraceMode::Off);
    declare(&mut sample, &[(1, 2)]);
    pass_until(&mut sample, |g| !g.state.rules.pending_triggers.is_empty());
    let trigger = sample.state.rules.pending_triggers[0].clone();
    for mode in modes() {
        let mut g = Game::with_trace(p.clone(), mode);
        declare(&mut g, &[(1, 2)]);
        // Leave exactly one slot: the first observer collects; the second
        // faults, proving rollback of partial IDs/pending as well as tapping.
        g.state
            .rules
            .pending_triggers
            .resize(capacities::STACK_LIMIT - 1, trigger.clone());
        let before = g.state.clone();
        let trace = g.trace.clone();
        let journal = g.replay_journal();
        assert!(matches!(g.advance(Budget::new(1)), Advance::Fault(_)));
        assert!(
            matches!(g.typed_fault(),Some(RuleFault::StackCapacityExceeded {requested,..}) if *requested==capacities::STACK_LIMIT+1)
        );
        let mut actual = g.state.clone();
        actual.rules.frame = before.rules.frame.clone();
        actual.rules.typed_fault = before.rules.typed_fault.clone();
        assert_eq!(actual, before);
        assert_eq!(g.trace, trace);
        assert_eq!(g.replay_journal(), journal);
    }
}
#[test]
fn d2cd_unreviewed_for_hosting_and_search_and_stable_reporting_key() {
    assert_eq!(
        TriggerCondition::CreatureAttacked {
            attacker: AttackSubject::Source {}
        }
        .reporting_key(),
        "creature_attacked"
    );
    let c = catalog();
    for (search, hosted, feature) in [
        (true, false, Feature::Search),
        (false, true, Feature::Hosting),
    ] {
        let mut p = match_profile("commander-foundations-v1").unwrap().clone();
        p.search = search;
        p.hosted = hosted;
        assert!(
            matches!(p.validate_requested_features(&c),Err(ProfileError::UnreviewedCapabilities {feature:f,capabilities}) if f==feature && capabilities.contains(&Capability::AttackTriggerV1))
        );
    }
}

#[test]
fn d2cd_other_admitted_whole_sources_reuse_draw_life_and_damage_programs() {
    for mode in modes() {
        for (name, draws, life, opponent) in [
            ("Infectious Horror", 0, 40, 38),
            ("Pulse Tracker", 0, 40, 39),
            ("Moonglove Extractor", 1, 39, 40),
            ("Shopkeeper's Bane", 0, 42, 40),
            ("Screeching Soulbreaker", 0, 41, 39),
            ("Boomerang, Blade Flinger", 0, 41, 39),
        ] {
            let id = catalog()
                .definitions
                .iter()
                .find(|d| d.name == name)
                .unwrap()
                .oracle_id
                .clone();
            let mut g = Game::with_trace(prepared(&[(&id, 0, 0, Zone::Battlefield)]), mode);
            let before = library(&g);
            declare(&mut g, &[(0, 1)]);
            placed(&mut g);
            assert_eq!(g.state.rules.next_trigger_id, 1);
            recover(&g);
            resolve(&mut g);
            assert_eq!(library(&g), before - draws, "{name}");
            assert_eq!(g.state.rules.players[0].life, life.into(), "{name}");
            for i in 1..4 {
                assert_eq!(g.state.rules.players[i].life, opponent.into(), "{name}");
            }
            recover(&g);
        }
    }
}
#[test]
fn d2cd_admitted_named_sources_use_existing_resolution_start_attacking_set() {
    for mode in modes() {
        for name in ["Pianna, Nomad Captain", "Iroh, Firebending Instructor"] {
            let id = catalog()
                .definitions
                .iter()
                .find(|d| d.name == name)
                .unwrap()
                .oracle_id
                .clone();
            let mut g = Game::with_trace(
                prepared(&[
                    (&id, 0, 0, Zone::Battlefield),
                    (
                        "000d5588-5a4c-434e-988d-396632ade42c",
                        0,
                        0,
                        Zone::Battlefield,
                    ),
                    (
                        "000d5588-5a4c-434e-988d-396632ade42c",
                        0,
                        0,
                        Zone::Battlefield,
                    ),
                ]),
                mode,
            );
            let prior = (0..3)
                .map(|n| g.characteristics(object(&g, n)).unwrap())
                .collect::<Vec<_>>();
            declare(&mut g, &[(0, 1), (1, 2)]);
            placed(&mut g);
            assert_eq!(g.state.rules.next_trigger_id, 1);
            recover(&g);
            resolve(&mut g);
            for n in 0..3 {
                let actual = g.characteristics(object(&g, n)).unwrap();
                let increment = Quantity::from(if n < 2 { 1 } else { 0 });
                let mut power = prior[n as usize].power.clone().unwrap();
                power.add(&increment);
                let mut toughness = prior[n as usize].toughness.clone().unwrap();
                toughness.add(&increment);
                assert_eq!(actual.power.unwrap(), power, "{name}/{n}");
                assert_eq!(actual.toughness.unwrap(), toughness, "{name}/{n}");
            }
            recover(&g);
        }
    }
}
#[test]
fn d2cd_admitted_targeted_pump_uses_existing_creature_target_choice() {
    for mode in modes() {
        let id = catalog()
            .definitions
            .iter()
            .find(|d| d.name == "Burrenton Shield-Bearers")
            .unwrap()
            .oracle_id
            .clone();
        let mut g = Game::with_trace(
            prepared(&[
                (&id, 0, 0, Zone::Battlefield),
                (YARGLE, 1, 1, Zone::Battlefield),
            ]),
            mode,
        );
        let target = object(&g, 1);
        let prior = g.characteristics(target).unwrap();
        declare(&mut g, &[(0, 2)]);
        assert!(matches!(settle(&mut g), Request::Targets { .. }));
        recover(&g);
        answer(
            &mut g,
            Response::Targets {
                objects: vec![target],
            },
        );
        placed(&mut g);
        recover(&g);
        resolve(&mut g);
        let actual = g.characteristics(target).unwrap();
        assert_eq!(actual.power, prior.power);
        let mut toughness = prior.toughness.unwrap();
        toughness.add(&Quantity::from(3));
        assert_eq!(actual.toughness.unwrap(), toughness);
        recover(&g);
    }
}
