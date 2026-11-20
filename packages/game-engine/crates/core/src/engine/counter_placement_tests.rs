//! M1b small private setup, NOT profile/exact-deck/host/search admission. Frozen
//! Ramos/Scales/Corpsejack are paid normally after construction. Separate tests
//! explicitly label synthetic quantities, control/zone probes and IR variation.
use super::*;
use mtg_contracts::{CounterPlacementModification, CounterRecipient};
fn prepared() -> Prepared {
    let mut c = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-staples-ir.json"
    ))
    .unwrap();
    let mana = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-mana-ir.json"
    ))
    .unwrap();
    c.definitions.push(
        mana.definitions
            .into_iter()
            .find(|d| d.name == "Sol Ring")
            .unwrap(),
    );
    for raw in [
        include_str!("../../../../tests/fixtures/ramos/3ed41d2d-211b-4013-8562-8c64d54cc43a.json"),
        include_str!("../../../../tests/fixtures/ramos/a1f3da21-af6d-450e-bf0b-985d158418e6.json"),
        include_str!("../../../../tests/fixtures/ramos/ca0cc02b-b106-4eca-9388-d4b48dd3be49.json"),
    ] {
        let result = mtg_compiler::compile(
            include_str!("../../../../tests/fixtures/ramos/archive-manifest-current.json"),
            raw,
        );
        let mtg_compiler::CompileResult::Lowered { definition } = result else {
            panic!("{result:?}")
        };
        c.definitions.push(*definition);
    }
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    let rows = [
        ("Ramos, Dragon Engine", 0, true),
        ("Hardened Scales", 0, true),
        ("Corpsejack Menace", 0, true),
        ("Seeds of Strength", 0, true),
        ("Battlegrowth", 0, true),
        ("Ironpaw Aspirant", 0, true),
        ("Satyr Grovedancer", 0, true),
        ("Sol Ring", 0, true),
        ("Grizzly Bears", 1, false),
        ("Hardened Scales", 1, false),
        ("Hardened Scales", 0, true),
        ("Corpsejack Menace", 0, true),
        ("Grizzly Bears", 0, false),
    ];
    let mut cards: Vec<_> = rows
        .iter()
        .enumerate()
        .map(|(n, (name, seat, hand))| CardSetup {
            id: CardId(n as u32),
            oracle_id: c
                .definitions
                .iter()
                .find(|d| d.name == *name)
                .unwrap()
                .oracle_id
                .clone(),
            owner: PlayerId(*seat),
            controller: PlayerId(*seat),
            zone: if *hand {
                Zone::Hand {
                    owner: PlayerId(*seat),
                }
            } else {
                Zone::Battlefield
            },
        })
        .collect();
    for (name, n) in [("Forest", 18), ("Swamp", 4), ("Plains", 4)] {
        let id = c
            .definitions
            .iter()
            .find(|d| d.name == name)
            .unwrap()
            .oracle_id
            .clone();
        for _ in 0..n {
            cards.push(CardSetup {
                id: CardId(cards.len() as u32),
                oracle_id: id.clone(),
                owner: PlayerId(0),
                controller: PlayerId(0),
                zone: Zone::Battlefield,
            });
        }
    }
    for seat in 0..4 {
        for _ in 0..4 {
            cards.push(CardSetup {
                id: CardId(cards.len() as u32),
                oracle_id: FOREST.into(),
                owner: PlayerId(seat),
                controller: PlayerId(seat),
                zone: Zone::Library {
                    owner: PlayerId(seat),
                },
            });
        }
    }
    let config = ScenarioConfig {
        initial: ScenarioInitial {
            turn_step: TurnStep::PrecombatMain,
            ..ScenarioInitial::default()
        },
        mode: Mode::CommanderMatch,
        players: 4,
        profile: "ramos-m1b-small-private-source-witness".into(),
        commanders: vec![],
        cards,
    };
    Prepared {
        catalog_digest: digest(&c).unwrap(),
        profile_digest: digest(&config).unwrap(),
        catalog: Arc::new(c),
        config,
        match_config: None,
    }
}
fn counter_request(g: &mut Game, q: u64) -> Request {
    for _ in 0..10000 {
        match g.advance(Budget::new(q)) {
            Advance::AwaitingDecision(r) => return r,
            Advance::Suspended { .. } => (),
            x => panic!("{x:?}: {:?}", g.state.rules.frame),
        }
    }
    panic!("counter request budget")
}
fn drive(g: &mut Game, add_first: bool, targets: &[TargetEntity], q: u64, inspect: bool) {
    for _ in 0..10000 {
        let a = g.advance(Budget::new(q));
        if inspect && matches!(g.state.rules.frame, Frame::CounterPlacement { .. }) {
            check(g);
        }
        match a {
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(r) => {
                if matches!(r, Request::Priority { .. })
                    && g.state.rules.stack.is_empty()
                    && g.state.rules.pending_triggers.is_empty()
                {
                    return;
                }
                let response = if let Request::CounterPlacementEffect { candidates, .. } = &r {
                    privacy(g);
                    let effect = candidates
                        .iter()
                        .find(|k| {
                            matches!(
                                g.definition(k.source.card).unwrap().static_abilities
                                    [k.ability as usize]
                                    .effect,
                                StaticEffect::ControllerCreaturePlusOneCounterReplacement {
                                    modification: CounterPlacementModification::Add { .. }
                                }
                            ) == add_first
                        })
                        .unwrap_or(&candidates[0])
                        .clone();
                    Response::CounterPlacementEffect { effect }
                } else {
                    automatic(&r, targets)
                };
                g.respond(r.player(), r.decision_id(), response).unwrap();
            }
            x => panic!("{x:?}: {:?}", g.state.rules.frame),
        }
    }
    panic!("counter drive budget")
}
fn paid(g: &mut Game) -> ObjectId {
    for card in [0, 1, 2] {
        announce(g, card, vec![]);
        assert!(
            g.state
                .rules
                .stack
                .iter()
                .find(|s| s.source.card == CardId(card) && s.kind.is_spell())
                .unwrap()
                .paid
                .is_some()
        );
        drive(g, true, &[], 1, true);
    }
    let r = object(g, 0);
    // Scales's paid green cast gives one before it enters; Corpsejack's
    // paid two-color cast is changed by Scales before Corpsejack enters.
    assert_eq!(g.current(r).unwrap().plus_one_counters, 4.into());
    check(g);
    r
}
fn privacy(g: &Game) {
    let state = g.state.clone();
    let evidence = g.trace.clone();
    let Some(Request::CounterPlacementEffect { player, .. }) =
        g.counter_placement_request().unwrap()
    else {
        panic!()
    };
    for seat in 0..4 {
        let viewer = PlayerId(seat);
        let QueryResult::Incomplete { work, .. } = g.observe(viewer, Budget::new(1)) else {
            panic!("observe must suspend")
        };
        let saved = g.save_work(&work).unwrap();
        let work = g.restore_work(&saved).unwrap();
        let QueryResult::Complete(QueryValue::Observation(obs)) =
            g.resume_work(work, Budget::unlimited())
        else {
            panic!("observe recover")
        };
        assert!(
            obs.view
                .objects
                .iter()
                .all(|o| !matches!(o.zone, Zone::Library { .. })
                    && !matches!(o.zone,Zone::Hand{owner} if owner!=viewer))
        );
        if viewer != player {
            assert!(obs.decision.is_none());
            continue;
        }
        let PlayerRequest::CounterPlacementEffect {
            token, candidates, ..
        } = obs.decision.unwrap()
        else {
            panic!("chooser request")
        };
        assert!(candidates.len() >= 2);
        let effect = candidates[0].effect.clone();
        let response = PlayerResponse::CounterPlacementEffect {
            effect: effect.clone(),
        };
        let QueryResult::Incomplete { work, .. } =
            g.check_player_response(viewer, token.clone(), response.clone(), Budget::new(1))
        else {
            panic!("validation suspends")
        };
        let saved = g.save_work(&work).unwrap();
        let work = g.restore_work(&saved).unwrap();
        assert!(matches!(
            g.resume_work(work, Budget::unlimited()),
            QueryResult::Complete(QueryValue::Validated(_))
        ));
        let foreign = CounterPlacementEffectHandle {
            source: ObjectHandle("forged-foreign".into()),
            ability: effect.ability,
        };
        assert!(matches!(
            g.check_player_response(
                viewer,
                token.clone(),
                PlayerResponse::CounterPlacementEffect { effect: foreign },
                Budget::unlimited()
            ),
            QueryResult::Rejected(_)
        ));
        assert!(matches!(
            g.check_player_response(
                PlayerId((seat + 1) % 4),
                token,
                response,
                Budget::unlimited()
            ),
            QueryResult::Rejected(_)
        ));
        let raw = canonical(&candidates).unwrap();
        assert!(
            !raw.contains("incarnation")
                && !raw.contains("stack_id")
                && !raw.contains("applications")
                && !raw.contains("provider")
        );
    }
    assert_eq!(g.state, state);
    assert_eq!(g.trace, evidence);
}
#[test]
fn counter_placement_paid_full_sources_both_orders_zero_repeated_all_traces_budgets() {
    for add_first in [true, false] {
        let mut expected = None;
        for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
            for q in [1, 31] {
                let mut g = Game::with_trace(prepared(), mode);
                let r = paid(&mut g);
                announce(&mut g, 3, vec![TargetEntity::Object { object: r }; 3]);
                drive(&mut g, add_first, &[], q, true);
                assert_eq!(
                    g.current(r).unwrap().plus_one_counters,
                    Quantity::from(if add_first { 10 } else { 9 })
                );
                let before = g.current(r).unwrap().plus_one_counters.clone();
                announce(&mut g, 7, vec![]);
                drive(&mut g, add_first, &[], q, true);
                assert_eq!(g.current(r).unwrap().plus_one_counters, before); // paid colorless cast remains zero
                announce(&mut g, 4, vec![TargetEntity::Object { object: r }]);
                drive(&mut g, add_first, &[], q, true);
                assert_eq!(
                    g.current(r).unwrap().plus_one_counters,
                    Quantity::from(if add_first { 18 } else { 15 })
                ); // two distinct placements
                check(&g);
                let digest = g.state_digest().unwrap();
                if let Some(old) = &expected {
                    assert_eq!(&digest, old)
                } else {
                    expected = Some(digest)
                }
            }
        }
    }
}
#[test]
fn counter_placement_fixed_target_self_enters_and_opponent_control() {
    let mut g = Game::new(prepared());
    let r = paid(&mut g);
    for id in [5, 6] {
        announce(&mut g, id, vec![]);
        drive(&mut g, true, &[TargetEntity::Object { object: r }], 1, true);
    }
    // A targeted opponent creature uses that creature's controller's Scales,
    // never our Corpsejack (even though we control the spell).
    let other = object(&g, 8);
    announce(&mut g, 4, vec![TargetEntity::Object { object: other }]);
    drive(&mut g, true, &[], 1, true);
    assert_eq!(g.current(other).unwrap().plus_one_counters, 2.into());
    check(&g);
    // Synthetic IR variation of admitted source-counter instruction shape.
    let mut p = prepared();
    let c = Arc::make_mut(&mut p.catalog);
    let d = c
        .definitions
        .iter_mut()
        .find(|d| d.name == "Satyr Grovedancer")
        .unwrap();
    d.triggered[0].program.targets.clear();
    let Instruction::PutPlusOneCounters { on, .. } = &mut d.triggered[0].program.instructions[0]
    else {
        panic!()
    };
    *on = CounterRecipient::Source;
    d.capabilities = d.requirements();
    c.validate().unwrap();
    p.catalog_digest = digest(c).unwrap();
    let mut g = Game::new(p);
    paid(&mut g);
    announce(&mut g, 6, vec![]);
    drive(&mut g, true, &[], 1, true);
    assert_eq!(
        g.current(object(&g, 6)).unwrap().plus_one_counters,
        4.into()
    );
    check(&g);
}

fn to_choice(g: &mut Game) -> Request {
    for _ in 0..1000 {
        let r = counter_request(g, 1);
        if matches!(r, Request::CounterPlacementEffect { .. }) {
            return r;
        }
        let response = automatic(&r, &[]);
        g.respond(r.player(), r.decision_id(), response).unwrap();
    }
    panic!("replacement choice unreachable")
}
fn choose(g: &mut Game, source_card: u32) {
    let Request::CounterPlacementEffect {
        player,
        decision_id,
        candidates,
        ..
    } = to_choice(g)
    else {
        panic!()
    };
    let effect = candidates
        .into_iter()
        .find(|k| k.source.card == CardId(source_card))
        .unwrap();
    g.respond(
        player,
        decision_id,
        Response::CounterPlacementEffect { effect },
    )
    .unwrap();
    assert!(matches!(
        g.advance(Budget::new(1)),
        Advance::Suspended { .. } | Advance::AwaitingDecision(_)
    ));
}
#[test]
fn counter_placement_synthetic_three_prefix_snapshots_stale_foreign_and_forged_recovery() {
    for add_first in [true, false] {
        let mut p = prepared();
        let c = Arc::make_mut(&mut p.catalog);
        c.definitions
            .iter_mut()
            .find(|d| d.name == "Seeds of Strength")
            .unwrap()
            .characteristics
            .colors = vec![Color::W, Color::U, Color::G]; // explicit synthetic color-count variation
        p.catalog_digest = digest(c).unwrap();
        let mut g = Game::new(p);
        let r = paid(&mut g);
        announce(&mut g, 3, vec![TargetEntity::Object { object: r }; 3]);
        to_choice(&mut g);
        check(&g);
        let QueryResult::Complete(QueryValue::Decision(Some(
            PlayerRequest::CounterPlacementEffect {
                token, candidates, ..
            },
        ))) = g.player_decision_for(PlayerId(0), Budget::unlimited())
        else {
            panic!()
        };
        let selected = candidates
            .iter()
            .find(|c| {
                matches!(c.modification, CounterPlacementModification::Add { .. }) == add_first
            })
            .unwrap()
            .effect
            .clone();
        let response = PlayerResponse::CounterPlacementEffect { effect: selected };
        let QueryResult::Incomplete { work, .. } = g.observe(PlayerId(0), Budget::new(1)) else {
            panic!()
        };
        let saved = g.save_work(&work).unwrap();
        let foreign = PlayerResponse::CounterPlacementEffect {
            effect: CounterPlacementEffectHandle {
                source: g.handle_for(PlayerId(1), object(&g, 1)).unwrap(),
                ability: 0,
            },
        };
        assert!(matches!(
            g.check_player_response(PlayerId(0), token.clone(), foreign, Budget::unlimited()),
            QueryResult::Rejected(_)
        ));
        let QueryResult::Complete(QueryValue::Validated(receipt)) = g.check_player_response(
            PlayerId(0),
            token.clone(),
            response.clone(),
            Budget::unlimited(),
        ) else {
            panic!()
        };
        g.submit(receipt).unwrap();
        check(&g); // pending input snapshot
        assert!(matches!(
            g.advance(Budget::new(1)),
            Advance::Suspended { .. }
        ));
        check(&g);
        assert_eq!(g.current(r).unwrap().plus_one_counters, 4.into());
        let Frame::CounterPlacement { transaction: tx } = &g.state.rules.frame else {
            panic!()
        };
        let member = &tx.members[0];
        assert_eq!(member.applications.len(), 1);
        assert_eq!(member.original_amount, 3.into());
        assert_eq!(member.amount, Quantity::from(if add_first { 4 } else { 6 }));
        assert!(g.restore_work(&saved).is_err());
        assert!(matches!(
            g.resume_work(work, Budget::unlimited()),
            QueryResult::Rejected(_)
        ));
        assert!(matches!(
            g.check_player_response(PlayerId(0), token, response, Budget::unlimited()),
            QueryResult::Rejected(_)
        ));
        let applied = member.applications[0].effect.clone();
        assert!(
            g.validate_counter_placement_response(PlayerId(0), &applied)
                .is_err()
        );
        // Rehashing cannot authenticate forged original quantity/prefix/cause.
        for n in 0..5 {
            let mut forged = g.fork();
            let Frame::CounterPlacement { transaction: tx } = &mut forged.state.rules.frame else {
                panic!()
            };
            let member = &mut tx.members[0];
            match n {
                0 => {
                    member.original_amount.add(&1.into());
                    member.applications[0].before = member.original_amount.clone();
                    member.applications[0].after = member.applications[0]
                        .modification
                        .apply(&member.original_amount);
                    member.amount = member.applications[0].after.clone();
                }
                1 => member.applications.push(member.applications[0].clone()),
                2 => member.applications[0].effect.source.incarnation += 1,
                3 => member.object = object(&g, 8),
                _ => tx.frame.cursor += 1,
            }
            assert!(
                Game::restore(
                    forged.prepared.as_ref().clone(),
                    &forged.snapshot().unwrap()
                )
                .is_err()
            );
        }
        assert!(matches!(
            g.advance(Budget::new(1)),
            Advance::Suspended { .. }
        ));
        check(&g);
        let Frame::CounterPlacement { transaction: tx } = &g.state.rules.frame else {
            panic!()
        };
        assert_eq!(
            tx.members[0].amount,
            Quantity::from(if add_first { 8 } else { 7 })
        );
        assert_eq!(g.current(r).unwrap().plus_one_counters, 4.into());
        drive(&mut g, add_first, &[], 1, true);
        assert_eq!(
            g.current(r).unwrap().plus_one_counters,
            Quantity::from(if add_first { 12 } else { 11 })
        );
        check(&g);
    }
}
#[test]
fn counter_placement_source_owner_concession_retains_prefix_and_revalidates_pending() {
    for applied in [false, true] {
        let mut p = prepared();
        for n in [0, 1, 2, 10] {
            p.config.cards[n].zone = Zone::Battlefield;
        }
        p.config.cards[10].owner = PlayerId(1); // public borrowed provider in private initial setup
        p.profile_digest = digest(&p.config).unwrap();
        let mut g = Game::new(p);
        let r = object(&g, 0);
        announce(&mut g, 3, vec![TargetEntity::Object { object: r }; 3]);
        let Request::CounterPlacementEffect {
            player,
            decision_id,
            candidates,
            ..
        } = to_choice(&mut g)
        else {
            panic!()
        };
        let effect = candidates
            .into_iter()
            .find(|k| k.source.card == CardId(10))
            .unwrap();
        g.respond(
            player,
            decision_id,
            Response::CounterPlacementEffect { effect },
        )
        .unwrap();
        check(&g);
        if applied {
            assert!(matches!(
                g.advance(Budget::new(1)),
                Advance::Suspended { .. } | Advance::AwaitingDecision(_)
            ));
            check(&g);
        }
        g.concede(PlayerId(1)).unwrap();
        assert!(matches!(
            g.advance(Budget::new(1)),
            Advance::Suspended { .. } | Advance::AwaitingDecision(_)
        ));
        check(&g);
        assert!(!g.state.rules.objects.contains_key(&CardId(10)));
        drive(&mut g, true, &[], 1, true);
        assert_eq!(
            g.current(r).unwrap().plus_one_counters,
            Quantity::from(if applied { 8 } else { 6 })
        );
        check(&g);
    }
    // Recipient/controller departure while awaiting a choice completes its owned
    // continuation without committing counters or resurrecting any departed card.
    let mut p = prepared();
    for n in [0, 1, 2] {
        p.config.cards[n].zone = Zone::Battlefield;
    }
    p.profile_digest = digest(&p.config).unwrap();
    let mut g = Game::new(p);
    let r = object(&g, 0);
    announce(&mut g, 3, vec![TargetEntity::Object { object: r }; 3]);
    to_choice(&mut g);
    g.concede(PlayerId(0)).unwrap();
    drive(&mut g, true, &[], 1, true);
    assert!(!g.state.rules.objects.contains_key(&r.card));
    check(&g);
}
#[test]
fn counter_placement_synthetic_duplicate_incarnation_and_recipient_departure() {
    let mut p = prepared();
    for n in [0, 1, 2, 10, 11] {
        p.config.cards[n].zone = Zone::Battlefield;
    }
    p.profile_digest = digest(&p.config).unwrap();
    let mut g = Game::new(p);
    let r = object(&g, 0);
    announce(&mut g, 3, vec![TargetEntity::Object { object: r }; 3]);
    to_choice(&mut g);
    choose(&mut g, 1);
    let old = object(&g, 1);
    // Explicit synthetic source departure/reentry at the transaction boundary:
    // no admitted source can enter mid-transaction; exact keys still distinguish it.
    g.move_object(old, Zone::Exile).unwrap();
    g.move_object(object(&g, 1), Zone::Battlefield).unwrap();
    let new = object(&g, 1);
    // Complete the explicit synthetic boundary before querying new handles.
    g.refresh_visibility().unwrap();
    assert_ne!(new, old);
    assert!(
        g.counter_placement_candidates()
            .iter()
            .any(|k| k.source == new)
    );
    let Frame::CounterPlacement { transaction: tx } = &g.state.rules.frame else {
        panic!()
    };
    assert_eq!(tx.members[0].applications[0].effect.source, old);
    drive(&mut g, true, &[], 1, false);
    assert_eq!(g.current(r).unwrap().plus_one_counters, 20.into()); // (2+1 old+1 reentered+1 duplicate)*2*2
    let mut g = Game::new(prepared());
    let r = paid(&mut g);
    announce(&mut g, 3, vec![TargetEntity::Object { object: r }; 3]);
    to_choice(&mut g);
    choose(&mut g, 1);
    g.move_object(r, Zone::Exile).unwrap();
    g.move_object(object(&g, 0), Zone::Battlefield).unwrap();
    let new = object(&g, 0);
    drive(&mut g, true, &[], 1, false);
    assert_ne!(r, new);
    assert_eq!(g.current(new).unwrap().plus_one_counters, 0.into());
}
#[test]
fn counter_placement_counter_cost_and_payment_undo_bypass_replacements() {
    for undo in [true, false] {
        let mut p = prepared();
        let c = Arc::make_mut(&mut p.catalog);
        c.definitions
            .iter_mut()
            .find(|d| d.name == "Battlegrowth")
            .unwrap()
            .characteristics
            .mana_cost
            .generic = 100.into(); // synthetic unpayable enclosing cost
        p.catalog_digest = digest(c).unwrap();
        for n in [0, 1, 2] {
            p.config.cards[n].zone = Zone::Battlefield;
        }
        p.profile_digest = digest(&p.config).unwrap();
        let mut g = Game::new(p);
        let r = object(&g, 0);
        announce(&mut g, 3, vec![TargetEntity::Object { object: r }; 3]);
        drive(&mut g, true, &[], 1, true);
        assert_eq!(g.current(r).unwrap().plus_one_counters, 6.into());
        answer(&mut g, Response::Cast { card: CardId(4) });
        answer(&mut g, Response::Targets { objects: vec![r] });
        let req = counter_request(&mut g, 1);
        assert!(matches!(req, Request::Mana { .. }));
        answer(&mut g, Response::ActivateMana { object: r });
        let _ = counter_request(&mut g, 1);
        assert_eq!(g.current(r).unwrap().plus_one_counters, 1.into());
        assert!(g.mana_ability_used(r));
        check(&g);
        answer(&mut g, Response::FinishMana { undo_mana: undo });
        let _ = counter_request(&mut g, 1);
        assert_eq!(
            g.current(r).unwrap().plus_one_counters,
            Quantity::from(if undo { 6 } else { 1 })
        );
        assert_eq!(g.mana_ability_used(r), !undo);
        check(&g);
    }
}

#[test]
fn counter_placement_real_source_activation_and_unavailable_producer_boundaries() {
    let mut p = prepared();
    let extra = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-sacrifice-ir.json"
    ))
    .unwrap();
    let source = extra
        .definitions
        .into_iter()
        .find(|d| d.name == "Carrion Feeder")
        .unwrap();
    let oracle = source.oracle_id.clone();
    let c = Arc::make_mut(&mut p.catalog);
    c.definitions.push(source);
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    p.catalog_digest = digest(c).unwrap();
    for n in [0, 1, 2] {
        p.config.cards[n].zone = Zone::Battlefield;
    }
    let card = CardId(p.config.cards.len() as u32);
    p.config.cards.push(CardSetup {
        id: card,
        oracle_id: oracle,
        owner: PlayerId(1),
        controller: PlayerId(0),
        zone: Zone::Battlefield,
    });
    p.profile_digest = digest(&p.config).unwrap();
    let mut g = Game::new(p);
    let feeder = g.object_id(card).unwrap();
    answer(
        &mut g,
        Response::ActivateAbility {
            object: feeder,
            ability: 0,
        },
    );
    let sacrifice = object(&g, 12);
    answer(&mut g, Response::SacrificeCost { object: sacrifice });
    drive(&mut g, true, &[], 1, true);
    assert_eq!(g.current(feeder).unwrap().plus_one_counters, 4.into());
    check(&g);
    // Without an active placement transaction both raw placements and whole
    // groups are rejected atomically.
    let before = g.state.clone();
    assert!(
        g.commit(
            ProposedEvent::PutPlusOneCounters {
                object: feeder,
                amount: 3.into()
            },
            BatchCause::Fixture
        )
        .is_err()
    );
    assert_eq!(g.state, before);
    assert!(
        g.commit_batch(
            BatchCause::Fixture,
            EventGroup::Simultaneous {
                occurrences: vec![
                    ProposedEvent::PutPlusOneCounters {
                        object: feeder,
                        amount: 1.into()
                    },
                    ProposedEvent::PutPlusOneCounters {
                        object: object(&g, 8),
                        amount: 1.into()
                    },
                ]
            }
        )
        .is_err()
    );
    assert_eq!(g.state, before);
}

#[test]
fn m4d_retained_cast_history_counter_choice_and_wrapper_accounting() {
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        let mut g = Game::with_trace(prepared(), mode);
        let ramos = paid(&mut g);
        announce(&mut g, 3, vec![TargetEntity::Object { object: ramos }; 3]);
        let mut resolving_charge = None;
        for _ in 0..500 {
            if let Frame::Resolve { frame } = &g.state.rules.frame {
                if matches!(
                    &frame.spell.kind,
                    StackKind::Triggered {
                        witness: TriggerEventWitness::SpellCast { .. },
                        ..
                    }
                ) {
                    resolving_charge = Some(g.grant_payload_charge());
                }
            }
            if matches!(g.state.rules.frame, Frame::CounterPlacement { .. }) {
                break;
            }
            match g.advance(Budget::new(1)) {
                Advance::Suspended { .. } => (),
                Advance::AwaitingDecision(request) => {
                    if matches!(request, Request::CounterPlacementEffect { .. }) {
                        break;
                    }
                    let response = automatic(&request, &[]);
                    g.respond(request.player(), request.decision_id(), response)
                        .unwrap();
                }
                other => panic!("counter continuation {other:?}"),
            }
        }
        let expected = resolving_charge.expect("real Ramos resolving observer");
        assert_eq!(
            g.grant_payload_charge(),
            expected,
            "retained copy survives counter choice"
        );
        let Frame::CounterPlacement { transaction } = &g.state.rules.frame else {
            panic!()
        };
        let resolving = transaction.frame.as_ref().clone();
        let StackKind::Triggered {
            witness: TriggerEventWitness::SpellCast { cast },
            ..
        } = &resolving.spell.kind
        else {
            panic!()
        };
        let fact_charge = stored_history::payload_charge(&cast.history);
        assert!(fact_charge > 0 && expected >= fact_charge * 2);
        check(&g);
        // Structural wrapper controls are not source-admission claims.
        for frame in [
            Frame::OptionalDamage {
                frame: resolving.clone(),
            },
            Frame::ForcedSacrifice {
                frame: resolving.clone(),
                remaining: vec![],
                choices: vec![],
                source_block: 0,
            },
            Frame::CommanderReturn {
                object: ramos,
                destination: Zone::Graveyard { owner: PlayerId(0) },
                continuation: Box::new(CommanderContinuation::ResolutionDraw {
                    frame: resolving.clone(),
                    player: PlayerId(0),
                    source_block: 0,
                }),
            },
            Frame::CommanderReturn {
                object: ramos,
                destination: Zone::Library { owner: PlayerId(0) },
                continuation: Box::new(CommanderContinuation::HandReturn {
                    frame: resolving.clone(),
                    objects: vec![ramos],
                    remaining: vec![ramos],
                    choices: vec![],
                    source_block: 0,
                }),
            },
            Frame::Stabilize {
                resume: Box::new(g.state.rules.frame.clone()),
                cleanup: false,
                changed: false,
            },
        ] {
            let mut structural = g.fork();
            structural.state.rules.frame = frame;
            assert_eq!(structural.grant_payload_charge(), expected);
        }
        // Synthetic one-over boundary, based on the independently retained Resolve charge.
        let other = g
            .history_charge()
            .checked_sub(g.grant_payload_charge())
            .unwrap();
        g.state.rules.retained_auxiliary_charge += capacities::HISTORY_LIMIT
            .checked_sub(other + expected)
            .unwrap()
            + 1;
        assert!(g.history_charge() > capacities::HISTORY_LIMIT);
        let request = g.counter_placement_request().unwrap().unwrap();
        let Request::CounterPlacementEffect { candidates, .. } = &request else {
            panic!()
        };
        g.respond(
            request.player(),
            request.decision_id(),
            Response::CounterPlacementEffect {
                effect: candidates[0].clone(),
            },
        )
        .unwrap();
        let before = g.state.clone();
        let trace = g.trace.clone();
        assert!(matches!(g.advance(Budget::new(1)), Advance::Fault(_)));
        assert!(matches!(
            g.typed_fault(),
            Some(RuleFault::HistoryCapacityExceeded { .. })
        ));
        let mut after = g.state.clone();
        after.rules.frame = before.rules.frame.clone();
        after.rules.typed_fault = before.rules.typed_fault.clone();
        assert_eq!(after, before);
        assert_eq!(g.trace, trace);
    }
}

#[test]
fn proliferate_landfall_chooses_nontargeted_incarnations_and_places_simultaneously() {
    let mut p = prepared();
    let result = mtg_compiler::compile(
        include_str!("../../../../tests/fixtures/archive-manifest-current.json"),
        include_str!("../../../../tests/fixtures/b45fdeab-00cc-4422-af9f-66f30a880a7c.json"),
    );
    let mtg_compiler::CompileResult::Lowered { definition } = result else {
        panic!("{result:?}");
    };
    let sage = definition.oracle_id.clone();
    let result = mtg_compiler::compile(
        include_str!("../../../../tests/fixtures/ramos/archive-manifest-current.json"),
        include_str!("../../../../tests/fixtures/synthetic-saga-life.json"),
    );
    let mtg_compiler::CompileResult::Lowered { definition: saga } = result else {
        panic!("{result:?}");
    };
    let saga_id = saga.oracle_id.clone();
    let c = Arc::make_mut(&mut p.catalog);
    c.definitions.extend([*definition, *saga]);
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    p.catalog_digest = digest(c).unwrap();
    let land = CardId(p.config.cards.len() as u32);
    let saga_card = CardId(land.0 + 2);
    p.config.cards.extend([
        CardSetup {
            id: land,
            oracle_id: FOREST.into(),
            owner: PlayerId(0),
            controller: PlayerId(0),
            zone: Zone::Hand { owner: PlayerId(0) },
        },
        CardSetup {
            id: CardId(land.0 + 1),
            oracle_id: sage,
            owner: PlayerId(0),
            controller: PlayerId(0),
            zone: Zone::Battlefield,
        },
        CardSetup {
            id: saga_card,
            oracle_id: saga_id,
            owner: PlayerId(0),
            controller: PlayerId(0),
            zone: Zone::Hand { owner: PlayerId(0) },
        },
    ]);
    p.profile_digest = digest(&p.config).unwrap();

    let mut g = Game::with_trace(p, TraceMode::Detailed);
    let own = paid(&mut g);
    let opponent = object(&g, 8);
    announce(&mut g, 4, vec![TargetEntity::Object { object: opponent }]);
    drive(&mut g, true, &[], 1, true);
    assert_eq!(g.current(opponent).unwrap().plus_one_counters, 2.into());
    announce(&mut g, saga_card.0, vec![]);
    drive(&mut g, true, &[], 1, true);
    let saga = object(&g, saga_card.0);
    assert_eq!(g.current(saga).unwrap().lore_counters, 1.into());
    let mut own_after = g.current(own).unwrap().plus_one_counters.clone();
    own_after.add(&4.into());
    answer(&mut g, Response::PlayLand { card: land });

    for expected in [own, opponent, saga] {
        loop {
            let r = counter_request(&mut g, 1);
            if let Request::Proliferate { candidate, .. } = r {
                assert_eq!(candidate, expected);
                let viewer = r.player();
                assert!(matches!(
                    g.player_decision_for(PlayerId(1), Budget::unlimited()),
                    QueryResult::Complete(QueryValue::Decision(None))
                ));
                let QueryResult::Complete(QueryValue::Decision(Some(PlayerRequest::Proliferate {
                    token,
                    candidate,
                    ..
                }))) = g.player_decision_for(viewer, Budget::unlimited())
                else {
                    panic!("missing scoped proliferate decision")
                };
                let response = PlayerResponse::Proliferate {
                    object: candidate,
                    include: true,
                };
                let QueryResult::Complete(QueryValue::Validated(receipt)) =
                    g.check_player_response(viewer, token, response, Budget::unlimited())
                else {
                    panic!("invalid scoped proliferate response");
                };
                g.submit(receipt).unwrap();
                check(&g);
                break;
            }
            answer(&mut g, Response::Pass);
        }
    }
    drive(&mut g, true, &[], 1, true);
    assert_eq!(g.current(own).unwrap().plus_one_counters, own_after);
    assert_eq!(g.current(saga).unwrap().lore_counters, 2.into());
    assert_eq!(g.current(opponent).unwrap().plus_one_counters, 4.into());
    assert!(g.trace().records.iter().any(|record| matches!(&record.record,
        EvidenceRecord::Batch {batch} if matches!(&batch.group,
            EventGroup::Simultaneous {occurrences} if occurrences.len() == 3
                && occurrences.iter().all(|e| matches!(e, ProposedEvent::ProliferateCounters {..}))
        ))));
    assert!(g.trace().records.iter().any(|record| matches!(&record.record,
        EvidenceRecord::Batch {batch} if batch.occurrences.iter().any(|row| matches!(&row.actual,
            EventOutcome::Proliferated {lore: Some(change), ..}
                if change.object == saga && change.before == 1.into() && change.after == 2.into()
        )))));
    check(&g);
}

#[test]
fn proliferate_follows_the_modifier_before_state_based_actions() {
    let mut p = prepared();
    let result = mtg_compiler::compile(
        include_str!("../../../../tests/fixtures/archive-manifest-current.json"),
        include_str!("../../../../tests/fixtures/e4912bc3-bee9-4a2f-a13e-3a99018f8a65.json"),
    );
    let mtg_compiler::CompileResult::Lowered { definition } = result else {
        panic!("{result:?}");
    };
    let id = p.config.cards.len() as u32;
    p.config.cards.push(CardSetup {
        id: CardId(id),
        oracle_id: definition.oracle_id.clone(),
        owner: PlayerId(0),
        controller: PlayerId(0),
        zone: Zone::Hand { owner: PlayerId(0) },
    });
    let c = Arc::make_mut(&mut p.catalog);
    c.definitions.push(*definition);
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    p.catalog_digest = digest(c).unwrap();
    p.profile_digest = digest(&p.config).unwrap();

    let mut g = Game::with_trace(p, TraceMode::Detailed);
    let opponent = object(&g, 8);
    let target = vec![TargetEntity::Object { object: opponent }];
    announce(&mut g, 4, target.clone());
    drive(&mut g, true, &[], 1, true);
    assert_eq!(g.current(opponent).unwrap().plus_one_counters, 2.into());

    announce(&mut g, id, target);
    let mut selected = false;
    for _ in 0..100 {
        let r = counter_request(&mut g, 1);
        if let Request::Proliferate { candidate, .. } = r {
            assert_eq!(candidate, opponent);
            assert_eq!(
                g.characteristics(opponent).unwrap().toughness,
                Some(0.into())
            );
            check(&g);
            answer(
                &mut g,
                Response::Proliferate {
                    object: opponent,
                    include: true,
                },
            );
            selected = true;
            break;
        }
        answer(&mut g, Response::Pass);
    }
    assert!(selected, "missing proliferate choice");
    drive(&mut g, true, &[], 1, true);
    assert_eq!(g.current(opponent).unwrap().plus_one_counters, 4.into());
    assert_eq!(
        g.characteristics(opponent).unwrap().toughness,
        Some(2.into())
    );
    check(&g);
}
