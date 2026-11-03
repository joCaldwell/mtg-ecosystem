//! Exact archive programs execute in private source-backed scenarios. Synthetic
//! IR additions (mana, flash/creature body, extra controller effects or alternate
//! predicates) are explicitly named; they establish composition, not card facts.
use super::*;
const MINE: &str = "d26b27db-a567-4631-b4b6-7294222fbdd1";
const SEIZAN: &str = "04d0d20f-720e-4cb6-a3ad-ea2b57bb7efa";
const FRETWORK: &str = "95f5ddb6-e3e5-4b35-88ac-eae192783eda";
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
fn priority(g: &mut Game, who: u8) {
    for _ in 0..4 {
        if settle(g).player() == PlayerId(who) {
            return;
        }
        answer(g, Response::Pass);
    }
    panic!("missing priority {who}")
}
fn add_mana(p: &mut Prepared, id: &str) {
    edit(p, id, |d| {
        d.mana_abilities = vec![ManaAbility {
            costs: PermanentActivationCosts {
                tap: true,
                mana: None,
                sacrifice: false,
            },
            production: ManaProduction::Fixed {
                symbols: vec![Color::C],
            },
            source_block: 0,
        }]
    });
}
fn tap(g: &mut Game, card: u32, who: u8) {
    priority(g, who);
    let object = g.object_id(CardId(card)).unwrap();
    answer(g, Response::ActivateMana { object });
    let _ = settle(g);
    assert!(g.current(object).unwrap().tapped);
}
fn pass_until(g: &mut Game, pred: impl Fn(&Game) -> bool) {
    for _ in 0..100 {
        if pred(g) {
            return;
        }
        match g.advance(Budget::new(1)) {
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(_) => passive(g),
            x => panic!("{x:?}"),
        }
    }
    panic!("missing rule boundary")
}
#[test]
fn d2cb_archived_mine_cast_normal_draw_then_actor_extra_draw_all_traces() {
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        let mut p = prepared(
            &[(MINE, 0, 0, Zone::Hand { owner: PlayerId(0) })],
            TurnStep::PrecombatMain,
        );
        for i in 1..=2 {
            p.config.cards[i].zone = Zone::Battlefield;
        }
        p.profile_digest = digest(&p.config).unwrap();
        let mut g = Game::with_trace(p, mode);
        answer(&mut g, Response::Cast { card: CardId(0) });
        for i in 1..=2 {
            let object = g.object_id(CardId(i)).unwrap();
            answer(&mut g, Response::ActivateMana { object });
        }
        answer(&mut g, Response::FinishMana { undo_mana: false });
        g.complete_test_mana();
        resolve(&mut g);
        to_step(&mut g, 2, TurnStep::Upkeep);
        assert_eq!(g.state.next_trigger_id, 0, "wrong step");
        let before = g.state.libraries[&PlayerId(1)].len();
        // Fragment across matching, normal entry and automatic placement.
        for _ in 0..4 {
            answer(&mut g, Response::Pass);
        }
        pass_until(&mut g, |g| matches!(g.state.frame, Frame::EnterStep));
        assert_eq!(g.state.pending_triggers.len(), 1);
        assert_eq!(g.state.libraries[&PlayerId(1)].len(), before);
        recover(&g);
        placed(&mut g);
        assert_eq!(g.state.libraries[&PlayerId(1)].len(), before - 1);
        let StackKind::Triggered {
            witness:
                TriggerEventWitness::StepBegan {
                    active: Some(actor),
                    ..
                },
            ..
        } = &g.state.stack[0].kind
        else {
            panic!("actor")
        };
        assert_eq!(*actor, PlayerId(1));
        assert_eq!(g.state.stack[0].controller, PlayerId(0));
        recover(&g);
        resolve(&mut g);
        assert_eq!(g.state.libraries[&PlayerId(1)].len(), before - 2);
        recover(&g);
        if mode == TraceMode::Detailed {
            let checks = g
                .events()
                .iter()
                .filter_map(|e| match e {
                    Event::TriggerConditionChecked { check, .. } => Some(check),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(checks.len(), 2);
            assert!(checks.iter().all(|c| matches!(
                c,
                InterveningCheck::SourceStatus {
                    information: SourceInformation::Current,
                    tapped: false,
                    passed: true,
                    ..
                }
            )));
        }
    }
}
#[test]
fn d2cb_synthetic_status_collection_and_resolution_use_departure_lki_all_traces() {
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        for (before_collection, after_collection, leave) in [
            (true, false, false),
            (false, true, false),
            (false, true, true),
            (false, false, true),
            (false, false, false),
        ] {
            let mut p = prepared(&[(MINE, 2, 1, Zone::Battlefield)], TurnStep::Upkeep);
            add_mana(&mut p, MINE); // Synthetic tap affordance; Mine has no mana ability.
            let mut g = Game::with_trace(p, mode);
            if before_collection {
                tap(&mut g, 0, 1);
            }
            to_step(&mut g, 1, TurnStep::Draw);
            assert_eq!(g.state.stack.len(), usize::from(!before_collection));
            if before_collection {
                recover(&g);
                continue;
            }
            let n = g.state.libraries[&PlayerId(0)].len();
            if after_collection {
                tap(&mut g, 0, 1);
            }
            if leave {
                g.concede(PlayerId(2)).unwrap();
                placed(&mut g);
                assert!(!g.state.objects.contains_key(&CardId(0)));
            }
            recover(&g);
            // Passing after a retained mana action includes the remaining living seats.
            while !g.state.stack.is_empty() {
                passive(&mut g);
            }
            let _ = settle(&mut g);
            assert_eq!(
                g.state.libraries[&PlayerId(0)].len(),
                n - usize::from(!after_collection)
            );
            recover(&g);
            if mode == TraceMode::Detailed {
                assert!(g.events().iter().any(|e| matches!(e,Event::TriggerConditionChecked {stage:InterveningStage::ResolutionStart {..},check:InterveningCheck::SourceStatus {tapped,information,passed,..},..} if *tapped==after_collection && *passed!=after_collection && *information==if leave {SourceInformation::LastKnown}else{SourceInformation::Current})));
            }
        }
    }
}
#[test]
fn d2cb_archived_seizan_ordered_life_and_two_draws_before_sbas() {
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        for (life, library) in [(40, 8), (1, 8), (40, 1), (40, 0)] {
            let mut p = prepared(&[(SEIZAN, 2, 1, Zone::Battlefield)], TurnStep::Untap);
            p.config.initial.life[0] = life.into();
            let mut left: usize = library;
            p.config.cards.retain(|c| {
                if c.zone == (Zone::Library { owner: PlayerId(0) }) {
                    let keep = left > 0;
                    left = left.saturating_sub(1);
                    keep
                } else {
                    true
                }
            });
            p.profile_digest = digest(&p.config).unwrap();
            let mut g = Game::with_trace(p, mode);
            to_step(&mut g, 1, TurnStep::Upkeep);
            recover(&g);
            resolve(&mut g);
            assert_eq!(g.is_live(PlayerId(0)), life > 2 && library >= 2);
            assert_eq!(g.state.players[1].life, 40.into());
            if g.is_live(PlayerId(0)) {
                assert_eq!(g.state.libraries[&PlayerId(0)].len(), library - 2);
            } else {
                assert_eq!(
                    g.state.players[0].last_known.as_ref().unwrap().life,
                    Quantity::from(life - 2)
                );
            }
            if mode == TraceMode::Detailed {
                let ops = g
                    .trace
                    .records
                    .iter()
                    .filter_map(|r| match &r.record {
                        EvidenceRecord::Batch { batch }
                            if matches!(batch.cause, BatchCause::Instruction { .. }) =>
                        {
                            Some(batch.occurrences.iter().map(|o| &o.actual))
                        }
                        _ => None,
                    })
                    .flatten()
                    .collect::<Vec<_>>();
                assert_eq!(ops.len(), 3);
                assert!(matches!(
                    ops[0],
                    EventOutcome::LifeLost {
                        player: PlayerId(0),
                        ..
                    }
                ));
                // Both attempts occur even after lethal life loss or the first failed draw.
            }
            recover(&g);
        }
    }
}
#[test]
fn d2cb_actor_controller_owner_departures_remain_distinct() {
    for departing in [0, 1, 2] {
        for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
            let mut p = prepared(&[(MINE, 2, 1, Zone::Battlefield)], TurnStep::Upkeep);
            edit(&mut p, MINE, |d| {
                d.triggered[0]
                    .program
                    .instructions
                    .push(Instruction::GainLife {
                        recipient: LifeRecipient::Controller,
                        amount: 3.into(),
                        source_block: 0,
                    })
            });
            let mut g = Game::with_trace(p, mode);
            to_step(&mut g, 1, TurnStep::Draw);
            let n = g.state.libraries[&PlayerId(1)].len();
            g.concede(PlayerId(departing)).unwrap();
            placed(&mut g);
            recover(&g);
            while !g.state.stack.is_empty() {
                passive(&mut g);
            }
            let _ = settle(&mut g);
            if departing != 1 {
                assert_eq!(g.state.players[1].life, 43.into());
                assert_eq!(
                    g.state.libraries[&PlayerId(1)].len(),
                    n,
                    "never redirect actor draw"
                );
            } else {
                assert!(g.state.stack.is_empty() && g.state.pending_triggers.is_empty());
            }
            recover(&g);
        }
    }
}
#[test]
fn d2cb_synthetic_absent_actor_upkeep_draw_end_and_multiple_controller_apnap() {
    for step in [
        BeginningStep::Upkeep,
        BeginningStep::Draw,
        BeginningStep::End,
    ] {
        for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
            let mut p = prepared(
                &[
                    (SEIZAN, 1, 1, Zone::Battlefield),
                    (SEIZAN, 2, 2, Zone::Battlefield),
                    (SEIZAN, 3, 3, Zone::Battlefield),
                ],
                TurnStep::Untap,
            );
            edit(&mut p, SEIZAN, |d| {
                d.triggered[0].condition = TriggerCondition::StepBegan {
                    step,
                    turn_relation: TurnRelation::AnyPlayer,
                };
                d.triggered[0]
                    .program
                    .instructions
                    .push(Instruction::GainLife {
                        recipient: LifeRecipient::Controller,
                        amount: 1.into(),
                        source_block: 0,
                    });
            });
            let mut g = Game::with_trace(p, mode);
            g.concede(PlayerId(0)).unwrap();
            let runtime = match step {
                BeginningStep::Upkeep => TurnStep::Upkeep,
                BeginningStep::Draw => TurnStep::Draw,
                BeginningStep::End => TurnStep::End,
            };
            to_step(&mut g, 1, runtime);
            assert_eq!(g.state.active, None);
            assert_eq!(g.state.turn_owner, PlayerId(0));
            assert_eq!(
                g.state
                    .stack
                    .iter()
                    .map(|s| s.controller)
                    .collect::<Vec<_>>(),
                vec![PlayerId(1), PlayerId(2), PlayerId(3)]
            );
            recover(&g);
            while !g.state.stack.is_empty() {
                passive(&mut g);
            }
            let _ = settle(&mut g);
            for i in 1..4 {
                assert_eq!(g.state.players[i].life, 41.into());
                assert_eq!(g.state.libraries[&PlayerId(i as u8)].len(), 8);
            }
            recover(&g);
        }
    }
}
fn synthetic_flash_reentry(id: &str, tapped_mana: bool) -> Prepared {
    let mut p = prepared(
        &[
            (id, 0, 0, Zone::Battlefield),
            (SPELL, 0, 0, Zone::Hand { owner: PlayerId(0) }),
            (ISLAND, 0, 0, Zone::Battlefield),
        ],
        TurnStep::Untap,
    );
    let body = p
        .catalog
        .definitions
        .iter()
        .find(|d| d.oracle_id == RAM)
        .unwrap()
        .characteristics
        .clone();
    edit(&mut p, id, |d| {
        d.characteristics.kind = CardKind::Creature;
        d.characteristics.power = body.power;
        d.characteristics.toughness = body.toughness;
        d.characteristics.mana_cost = ManaCost {
            generic: 0.into(),
            colored: vec![],
        };
        d.keywords = vec![KeywordAbility::Flash, KeywordAbility::Haste];
        d.spell.as_mut().unwrap().timing = SpellTiming::Instant;
    });
    if tapped_mana {
        add_mana(&mut p, id);
    }
    p
}
fn bounce_recast(g: &mut Game) {
    priority(g, 0);
    let old = g.object_id(CardId(0)).unwrap();
    answer(g, Response::Cast { card: CardId(1) });
    answer(g, Response::Targets { objects: vec![old] });
    let object = g.object_id(CardId(2)).unwrap();
    answer(g, Response::ActivateMana { object });
    answer(g, Response::FinishMana { undo_mana: false });
    g.complete_test_mana();
    resolve(g);
    assert_eq!(
        g.state.objects[&CardId(0)].zone,
        Zone::Hand { owner: PlayerId(0) }
    );
    recover(g);
    answer(g, Response::Cast { card: CardId(0) });
    answer(g, Response::FinishMana { undo_mana: false });
    g.complete_test_mana();
    resolve(g);
    assert_ne!(g.object_id(CardId(0)).unwrap(), old);
    recover(g);
}
#[test]
fn d2cb_synthetic_reentry_never_overwrites_old_incarnation_status() {
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        for old_tapped in [false, true] {
            let mut g = Game::with_trace(synthetic_flash_reentry(MINE, true), mode);
            to_step(&mut g, 1, TurnStep::Draw);
            let n = g.state.libraries[&PlayerId(0)].len();
            if old_tapped {
                tap(&mut g, 0, 0);
            }
            bounce_recast(&mut g);
            if !old_tapped {
                tap(&mut g, 0, 0);
            }
            assert_eq!(g.state.stack.len(), 1);
            recover(&g);
            resolve(&mut g);
            assert_eq!(
                g.state.libraries[&PlayerId(0)].len(),
                n - usize::from(!old_tapped)
            );
            recover(&g);
        }
    }
}
#[test]
fn d2cb_synthetic_fretwork_reentry_preserves_loss_without_successor_counter() {
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        let mut g = Game::with_trace(synthetic_flash_reentry(FRETWORK, false), mode);
        to_step(&mut g, 1, TurnStep::Upkeep);
        bounce_recast(&mut g);
        resolve(&mut g);
        assert_eq!(g.state.objects[&CardId(0)].plus_one_counters, 0.into());
        assert_eq!(g.state.players[0].life, 39.into());
        recover(&g);
    }
}
#[test]
fn d2cb_archived_mine_actor_commander_draw_continuation_and_empty_library() {
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        for id in [MINE, SEIZAN] {
            let start = if id == MINE {
                TurnStep::Upkeep
            } else {
                TurnStep::Untap
            };
            let step = if id == MINE {
                TurnStep::Draw
            } else {
                TurnStep::Upkeep
            };
            let mut p = prepared(&[(id, 2, 1, Zone::Battlefield)], start);
            // Mine's turn-based draw precedes the trigger; Seizan starts with
            // the commander attempt and must continue to its second draw.
            let index = if id == MINE { 2 } else { 1 };
            let commander = p.config.cards[index].id;
            p.config.cards[index].oracle_id = YARGLE.into();
            p.config.commanders.push(commander);
            p.profile_digest = digest(&p.config).unwrap();
            let mut g = Game::with_trace(p, mode);
            to_step(&mut g, 1, step);
            recover(&g);
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
            // Source owner leaves during the suspended draw. The invocation's
            // historical condition check stays frozen and its actor stays player 0.
            g.concede(PlayerId(2)).unwrap();
            assert!(matches!(
                settle(&mut g),
                Request::CommanderReturn {
                    player: PlayerId(0),
                    ..
                }
            ));
            recover(&g);
            answer(&mut g, Response::CommanderReturn { to_command: true });
            placed(&mut g);
            assert_eq!(
                g.state.objects[&commander].zone,
                Zone::Command { owner: PlayerId(0) }
            );
            assert_eq!(g.state.libraries[&PlayerId(0)].len(), 6);
            assert_eq!(
                g.state.players[0].life,
                if id == SEIZAN { 38.into() } else { 40.into() }
            );
            recover(&g);
        }
        let mut p = prepared(&[(MINE, 2, 1, Zone::Battlefield)], TurnStep::Upkeep);
        let mut first = true;
        p.config.cards.retain(|c| {
            if c.zone == (Zone::Library { owner: PlayerId(0) }) {
                let keep = first;
                first = false;
                keep
            } else {
                true
            }
        });
        p.profile_digest = digest(&p.config).unwrap();
        let mut g = Game::with_trace(p, mode);
        to_step(&mut g, 1, TurnStep::Draw);
        resolve(&mut g);
        assert!(!g.is_live(PlayerId(0)));
        recover(&g);
    }
}
#[test]
fn d2cb_synthetic_hand_empty_composes_with_step_trigger_and_keeps_old_shape() {
    for empty in [false, true] {
        let mut p = prepared(&[(MINE, 2, 1, Zone::Battlefield)], TurnStep::Upkeep);
        edit(&mut p, MINE, |d| {
            d.triggered[0].intervening_if = Some(InterveningCondition::ControllerHandEmpty)
        });
        if !empty {
            p.config.cards[9].zone = Zone::Hand { owner: PlayerId(1) };
        }
        p.profile_digest = digest(&p.config).unwrap();
        let mut g = Game::new(p);
        to_step(&mut g, 1, TurnStep::Draw);
        assert_eq!(g.state.stack.len(), usize::from(empty));
        recover(&g);
        if empty {
            resolve(&mut g);
            recover(&g);
        }
    }
    let c = InterveningCheck::ControllerHandEmpty {
        controller: PlayerId(1),
        hand_count: 0,
        passed: true,
    };
    assert_eq!(
        serde_json::to_string(&c).unwrap(),
        r#"{"condition":"controller_hand_empty","controller":1,"hand_count":0,"passed":true}"#
    );
}
#[test]
fn d2cb_forged_actor_status_and_lki_evidence_rejected_all_traces() {
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        let mut g = Game::with_trace(
            prepared(&[(MINE, 2, 1, Zone::Battlefield)], TurnStep::Upkeep),
            mode,
        );
        to_step(&mut g, 1, TurnStep::Draw);
        g.concede(PlayerId(2)).unwrap();
        placed(&mut g);
        recover(&g);
        for field in ["actor", "lki", "status", "information", "source", "passed"] {
            let mut s: serde_json::Value = serde_json::from_str(&g.snapshot().unwrap()).unwrap();
            let entry = &mut s["state"]["stack"][0];
            match field {
                "actor" => entry["kind"]["witness"]["active"] = 1.into(),
                "lki" => entry["source_facts"]["tapped"] = true.into(),
                "status" => entry["kind"]["match_condition"]["status"] = "tapped".into(),
                "information" => {
                    entry["kind"]["match_condition"]["information"] = "last_known".into()
                }
                "source" => entry["kind"]["match_condition"]["source"]["incarnation"] = 1.into(),
                _ => entry["kind"]["match_condition"]["passed"] = false.into(),
            }
            s["state_sha256"] = digest(&s["state"]).unwrap().into();
            assert!(
                Game::restore(g.prepared.as_ref().clone(), &s.to_string()).is_err(),
                "{mode:?}/{field}"
            );
        }
    }
}
#[test]
fn d2cb_new_capabilities_excluded_from_hosting_and_search() {
    let c = Catalog::read(include_str!(
        "../../../../tests/fixtures/cards-data-d2c/expected-d2c-b-ir.json"
    ))
    .unwrap();
    for (search, hosted, feature) in [
        (true, false, Feature::Search),
        (false, true, Feature::Hosting),
    ] {
        let mut p = match_profile("commander-foundations-v1").unwrap().clone();
        p.search = search;
        p.hosted = hosted;
        let Err(ProfileError::UnreviewedCapabilities {
            feature: f,
            capabilities,
        }) = p.validate_requested_features(&c)
        else {
            panic!("review admission")
        };
        assert_eq!(f, feature);
        for cap in [
            Capability::EventPlayerRecipientV1,
            Capability::SourceStatusConditionV1,
        ] {
            assert!(capabilities.contains(&cap));
        }
    }
}
#[test]
fn d2cb_archived_direct_untargeted_self_damage_regression() {
    // Exact archive line33421; this is a D2c-a admission, not a new D2c-b one.
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        let mut g = Game::with_trace(
            prepared(
                &[(
                    "dd520d83-297a-487f-b8f3-4997bbc056e0",
                    2,
                    0,
                    Zone::Battlefield,
                )],
                TurnStep::Untap,
            ),
            mode,
        );
        to_step(&mut g, 1, TurnStep::Upkeep);
        recover(&g);
        resolve(&mut g);
        assert_eq!(g.state.players[0].life, 39.into());
        recover(&g);
    }
}
#[test]
fn d2cb_fragmented_match_placement_and_resolution_cache_restore_all_traces() {
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        for id in [MINE, SEIZAN] {
            let p = prepared(
                &[(id, 2, 1, Zone::Battlefield)],
                if id == MINE {
                    TurnStep::Upkeep
                } else {
                    TurnStep::Untap
                },
            );
            let mut g = Game::with_trace(p, mode);
            for _ in 0..4 {
                answer(&mut g, Response::Pass);
            }
            let mut pending = false;
            let mut cached = false;
            for _ in 0..100 {
                recover(&g);
                pending |= !g.state.pending_triggers.is_empty();
                if matches!(&g.state.frame,Frame::Resolve {frame} if frame.condition_check.is_some())
                {
                    cached = true;
                }
                match g.advance(Budget::new(1)) {
                    Advance::Suspended { .. } => (),
                    Advance::AwaitingDecision(_)
                        if g.state.next_trigger_id > 0
                            && g.state.stack.is_empty()
                            && g.state.pending_triggers.is_empty() =>
                    {
                        break;
                    }
                    Advance::AwaitingDecision(_) => passive(&mut g),
                    x => panic!("fragmented {x:?}"),
                }
            }
            assert!(pending);
            assert_eq!(cached, id == MINE);
            recover(&g);
        }
    }
}
#[test]
fn d2cb_source_status_collection_failure_rolls_back_atomic_evidence() {
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        let mut g = Game::with_trace(
            prepared(&[(MINE, 2, 1, Zone::Battlefield)], TurnStep::Upkeep),
            mode,
        );
        g.state.frame = Frame::Transition;
        g.state.next_trigger_id = u64::MAX;
        let before = g.fork();
        let token = g.state_token.clone();
        assert!(g.transition_turn_step().is_err());
        assert_eq!(g.state, before.state);
        assert_eq!(g.trace, before.trace);
        assert!(Arc::ptr_eq(&token, &g.state_token));
    }
}
#[test]
fn d2cb_synthetic_tapped_predicate_and_hand_recheck_after_normal_draw() {
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        let mut p = prepared(&[(MINE, 2, 1, Zone::Battlefield)], TurnStep::Upkeep);
        add_mana(&mut p, MINE);
        edit(&mut p, MINE, |d| {
            d.triggered[0].intervening_if = Some(InterveningCondition::SourceStatus {
                status: TapStatus::Tapped,
            })
        });
        let mut g = Game::with_trace(p, mode);
        tap(&mut g, 0, 1);
        to_step(&mut g, 1, TurnStep::Draw);
        assert_eq!(g.state.stack.len(), 1);
        g.concede(PlayerId(2)).unwrap();
        placed(&mut g);
        recover(&g);
        let n = g.state.libraries[&PlayerId(0)].len();
        while !g.state.stack.is_empty() {
            passive(&mut g);
        }
        let _ = settle(&mut g);
        assert_eq!(g.state.libraries[&PlayerId(0)].len(), n - 1);
        recover(&g);
        let mut p = prepared(&[(MINE, 0, 0, Zone::Battlefield)], TurnStep::Upkeep);
        edit(&mut p, MINE, |d| {
            d.triggered[0].intervening_if = Some(InterveningCondition::ControllerHandEmpty)
        });
        let mut g = Game::with_trace(p, mode);
        to_step(&mut g, 1, TurnStep::Draw);
        assert_eq!(g.state.stack.len(), 1, "empty hand at actual collection");
        assert_eq!(g.state.libraries[&PlayerId(0)].len(), 7);
        resolve(&mut g);
        assert_eq!(
            g.state.libraries[&PlayerId(0)].len(),
            7,
            "normal draw makes resolution check false"
        );
        recover(&g);
    }
}
