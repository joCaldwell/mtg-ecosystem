//! Complete archived Shrapnel Blast in private source configurations, not
//! admitted pods or pools. Gameplay uses accepted inputs and concessions.
//! Pinned CR118.8,601.2h,608.2b,701.21a:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

fn prepared(rows: &[(&str, u8, u8, bool)]) -> Prepared {
    let mut catalog = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap();
    for (header, raw) in [
        (
            include_str!("../../../../tests/fixtures/archive-manifest-current.json"),
            include_str!("../../../../tests/fixtures/d3c623f3-c7c9-47c0-8096-6dee3267baff.json"),
        ),
        (
            include_str!(
                "../../../../tests/fixtures/cards-data-filtered-damage/archive-manifest-current.json"
            ),
            include_str!(
                "../../../../tests/fixtures/cards-data-filtered-damage/4caa8e68-e599-416f-b839-3d009569ff29.json"
            ),
        ),
    ] {
        let mtg_compiler::CompileResult::Lowered { definition } =
            mtg_compiler::compile(header, raw)
        else {
            panic!("complete artifact sacrifice source must lower");
        };
        catalog.definitions.push(*definition);
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    let mut config = ScenarioConfig::turns_permanents();
    config.profile = "artifact-sacrifice-source-witness".into();
    config.initial.turn_step = TurnStep::PrecombatMain;
    config.cards = rows
        .iter()
        .enumerate()
        .map(|(id, (name, owner, controller, hand))| CardSetup {
            id: CardId(id as u32),
            oracle_id: catalog
                .definitions
                .iter()
                .find(|d| d.name == *name)
                .unwrap()
                .oracle_id
                .clone(),
            owner: PlayerId(*owner),
            controller: PlayerId(*controller),
            zone: if *hand {
                Zone::Hand {
                    owner: PlayerId(*owner),
                }
            } else {
                Zone::Battlefield
            },
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
    }
    Prepared {
        catalog_digest: digest(&catalog).unwrap(),
        catalog: Arc::new(catalog),
        profile_digest: digest(&config).unwrap(),
        config,
        match_config: None,
    }
}
fn object(g: &Game, card: u32) -> ObjectId {
    g.object_id(CardId(card)).unwrap()
}
fn request(g: &mut Game, quantum: u64) -> Request {
    for _ in 0..500 {
        match g.advance(Budget::new(quantum)) {
            Advance::AwaitingDecision(r) => return r,
            Advance::Suspended { .. } => (),
            other => panic!("artifact sacrifice {:?}: {other:?}", g.state.rules.frame),
        }
    }
    panic!("artifact sacrifice work bound");
}
fn answer(g: &mut Game, quantum: u64, response: Response) {
    let r = request(g, quantum);
    g.respond(r.player(), r.decision_id(), response).unwrap();
}
fn begin(g: &mut Game, quantum: u64, target: TargetEntity) -> ObjectId {
    assert!(matches!(
        request(g, quantum),
        Request::Priority {
            player: PlayerId(0),
            ..
        }
    ));
    answer(g, quantum, Response::Cast { card: CardId(0) });
    assert!(matches!(request(g, quantum), Request::EntityTargets { .. }));
    answer(
        g,
        quantum,
        Response::EntityTargets {
            entities: vec![target],
        },
    );
    assert!(matches!(request(g, quantum), Request::Mana { .. }));
    g.top().unwrap().source
}
fn fund(g: &mut Game, quantum: u64, ring: u32, mountain: u32) {
    let ring = object(g, ring);
    let mountain = object(g, mountain);
    answer(g, quantum, Response::ActivateMana { object: ring });
    answer(g, quantum, Response::ActivateMana { object: mountain });
    answer(g, quantum, Response::FinishMana { undo_mana: false });
    assert!(matches!(request(g, quantum), Request::SpendMana { .. }));
    answer(
        g,
        quantum,
        Response::SpendMana {
            spending: BTreeMap::from([(Color::C, 1.into()), (Color::R, 1.into())]),
        },
    );
    assert!(matches!(request(g, quantum), Request::SacrificeCost { .. }));
}
fn pay(g: &mut Game, quantum: u64, sacrifice: ObjectId) -> StackId {
    answer(g, quantum, Response::SacrificeCost { object: sacrifice });
    assert!(matches!(request(g, quantum), Request::Priority { .. }));
    g.top().unwrap().id
}
fn settle(g: &mut Game, quantum: u64) {
    for _ in 0..40 {
        let r = request(g, quantum);
        assert!(matches!(r, Request::Priority { .. }), "{r:?}");
        if g.state.rules.stack.is_empty() {
            return;
        }
        g.respond(r.player(), r.decision_id(), Response::Pass)
            .unwrap();
    }
    panic!("artifact sacrifice resolution bound");
}
fn recover(g: &Game) {
    let p = g.prepared.as_ref().clone();
    let restored = Game::restore(p.clone(), &g.snapshot().unwrap()).unwrap();
    assert_eq!(restored.state, g.state);
    assert_eq!(restored.trace(), g.trace());
    let replay = Game::replay_to(
        p,
        &g.replay_journal(),
        Some(g.rule_boundary()),
        g.trace.mode,
    )
    .unwrap();
    assert_eq!(replay.state, g.state);
    assert_eq!(replay.trace(), g.trace());
    assert_eq!(g.fork().state, g.state);
}
fn completed(g: &Game, stack: StackId, resolved: bool) {
    assert!(g.events().iter().any(|e| matches!(e, Event::SpellCompleted { stack_id, resolved: r } if *stack_id == stack && *r == resolved)));
}
fn pool(g: &Game, colorless: i64, red: i64) {
    let positive = g.state.rules.players[0]
        .mana
        .iter()
        .filter(|(_, n)| **n > Quantity::zero())
        .map(|(c, n)| (*c, n.clone()))
        .collect::<BTreeMap<_, _>>();
    let expected = [(Color::C, colorless), (Color::R, red)]
        .into_iter()
        .filter(|(_, n)| *n > 0)
        .map(|(c, n)| (c, Quantity::from(n)))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(positive, expected);
}
fn reject(g: &mut Game, player: PlayerId, decision: u64, response: Response) {
    let before = g.snapshot().unwrap();
    assert!(g.respond(player, decision, response).is_err());
    assert_eq!(g.snapshot().unwrap(), before);
}

#[test]
fn artifact_sacrifice_ring_pays_mana_then_cost_and_shrapnel_deals_five_with_replay() {
    let p = prepared(&[
        ("Shrapnel Blast", 0, 0, true),
        ("Sol Ring", 0, 0, false),
        ("Mountain", 0, 0, false),
    ]);
    let mut baseline = None;
    for (quantum, mode) in [(1, TraceMode::Detailed), (4096, TraceMode::Off)] {
        let mut g = Game::with_trace(p.clone(), mode);
        let ring = object(&g, 1);
        let source = begin(
            &mut g,
            quantum,
            TargetEntity::Player {
                player: PlayerId(1),
            },
        );
        fund(&mut g, quantum, 1, 2);
        assert!(g.current(ring).unwrap().tapped);
        recover(&g);
        let stack = pay(&mut g, quantum, ring);
        assert_eq!(
            g.current(object(&g, 1)).unwrap().zone,
            Zone::Graveyard { owner: PlayerId(0) }
        );
        let cost = g.top().unwrap().sacrificed.as_ref().unwrap();
        assert_eq!(cost.object, ring);
        assert!(cost.tapped);
        pool(&g, 1, 0);
        recover(&g);
        settle(&mut g, quantum);
        assert_eq!(g.state.rules.players[1].life, 35.into());
        pool(&g, 1, 0);
        if mode == TraceMode::Detailed {
            completed(&g, stack, true);
            let damage = g
                .trace
                .records
                .iter()
                .filter_map(|r| match &r.record {
                    EvidenceRecord::Batch { batch } => Some(&batch.occurrences),
                    _ => None,
                })
                .flatten()
                .filter_map(|o| match &o.actual {
                    EventOutcome::DamageCommitted { actual, .. } => Some(actual),
                    _ => None,
                })
                .flatten()
                .collect::<Vec<_>>();
            assert_eq!(damage.len(), 1);
            assert_eq!(damage[0].source.object, source);
            assert_eq!(damage[0].amount, 5.into());
        }
        recover(&g);
        if let Some(old) = &baseline {
            assert_eq!(old, &g.state);
        } else {
            baseline = Some(g.state.clone());
        }
    }
}

#[test]
fn artifact_sacrifice_candidates_use_complete_types_control_and_exact_identity_atomically() {
    let mut without_cost = Game::new(prepared(&[
        ("Shrapnel Blast", 0, 0, true),
        ("Grizzly Bears", 0, 0, false),
        ("Alpha Myr", 1, 1, false),
        ("Alpha Myr", 0, 0, true),
        ("Mountain", 0, 0, false),
        ("Mountain", 0, 0, false),
    ]));
    let r = request(&mut without_cost, 1);
    for id in [4, 5] {
        let land = object(&without_cost, id);
        assert!(
            without_cost
                .validate_mana_activation(PlayerId(0), land)
                .is_ok()
        );
        assert!(
            without_cost
                .mana_abilities(land)
                .unwrap()
                .contains(&Color::R)
        );
    }
    let before = without_cost.snapshot().unwrap();
    assert_eq!(
        without_cost
            .respond(
                r.player(),
                r.decision_id(),
                Response::Cast { card: CardId(0) }
            )
            .unwrap_err(),
        "no payable sacrifice cost"
    );
    assert_eq!(without_cost.snapshot().unwrap(), before);
    let mut g = Game::new(prepared(&[
        ("Shrapnel Blast", 0, 0, true),
        ("Sol Ring", 0, 0, false),
        ("Mountain", 0, 0, false),
        ("Alpha Myr", 2, 0, false),
        ("Grizzly Bears", 0, 0, false),
        ("Alpha Myr", 1, 1, false),
        ("Alpha Myr", 0, 0, true),
    ]));
    begin(
        &mut g,
        1,
        TargetEntity::Player {
            player: PlayerId(1),
        },
    );
    fund(&mut g, 1, 1, 2);
    let r = request(&mut g, 1);
    let Request::SacrificeCost { candidates, .. } = &r else {
        panic!("cost request");
    };
    let myr = object(&g, 3);
    assert_eq!(candidates, &vec![object(&g, 1), myr]);
    for id in [4, 5, 6] {
        let wrong = object(&g, id);
        reject(
            &mut g,
            r.player(),
            r.decision_id(),
            Response::SacrificeCost { object: wrong },
        );
    }
    reject(
        &mut g,
        PlayerId(1),
        r.decision_id(),
        Response::SacrificeCost { object: myr },
    );
    reject(
        &mut g,
        r.player(),
        r.decision_id().wrapping_add(1),
        Response::SacrificeCost { object: myr },
    );
    reject(
        &mut g,
        r.player(),
        r.decision_id(),
        Response::SacrificeCost {
            object: ObjectId {
                incarnation: myr.incarnation + 1,
                ..myr
            },
        },
    );
    recover(&g);
    pay(&mut g, 1, myr);
    assert_eq!(
        g.current(object(&g, 3)).unwrap().zone,
        Zone::Graveyard { owner: PlayerId(2) }
    );
    settle(&mut g, 1);
    assert_eq!(g.state.rules.players[1].life, 35.into());
    recover(&g);
}

#[test]
fn artifact_sacrifice_damage_target_may_pay_its_own_cost_then_fizzles_without_refund() {
    let mut g = Game::with_trace(
        prepared(&[
            ("Shrapnel Blast", 0, 0, true),
            ("Sol Ring", 0, 0, false),
            ("Mountain", 0, 0, false),
            ("Alpha Myr", 0, 0, false),
        ]),
        TraceMode::Detailed,
    );
    let myr = object(&g, 3);
    begin(&mut g, 1, TargetEntity::Object { object: myr });
    fund(&mut g, 1, 1, 2);
    let stack = pay(&mut g, 1, myr);
    assert_eq!(
        g.top().unwrap().entity_targets,
        vec![TargetEntity::Object { object: myr }]
    );
    recover(&g);
    settle(&mut g, 1);
    completed(&g, stack, false);
    assert_eq!(
        g.current(object(&g, 3)).unwrap().zone,
        Zone::Graveyard { owner: PlayerId(0) }
    );
    assert_eq!(
        g.current(object(&g, 0)).unwrap().zone,
        Zone::Graveyard { owner: PlayerId(0) }
    );
    assert!(g.current(object(&g, 1)).unwrap().tapped);
    assert!(g.current(object(&g, 2)).unwrap().tapped);
    pool(&g, 1, 0);
    assert!(g.state.rules.players.iter().all(|p| p.life == 40.into()));
    recover(&g);
}

#[test]
fn artifact_sacrifice_departure_revalidates_pending_cost_choice_and_keeps_paid_mana_atomic() {
    let mut g = Game::new(prepared(&[
        ("Shrapnel Blast", 0, 0, true),
        ("Sol Ring", 0, 0, false),
        ("Mountain", 0, 0, false),
        ("Alpha Myr", 2, 0, false),
    ]));
    begin(
        &mut g,
        1,
        TargetEntity::Player {
            player: PlayerId(1),
        },
    );
    fund(&mut g, 1, 1, 2);
    let myr = object(&g, 3);
    answer(&mut g, 1, Response::SacrificeCost { object: myr });
    // Accepted but not consumed: the owner's departure removes the selected
    // permanent before payment. Revalidate rather than sacrifice a stale ID.
    g.concede(PlayerId(2)).unwrap();
    let r = request(&mut g, 1);
    let Request::SacrificeCost { candidates, .. } = &r else {
        panic!("refreshed cost {r:?}");
    };
    assert_eq!(candidates, &vec![object(&g, 1)]);
    assert!(g.state.rules.pending.is_none());
    pool(&g, 2, 1);
    reject(
        &mut g,
        r.player(),
        r.decision_id(),
        Response::SacrificeCost { object: myr },
    );
    recover(&g);
    let ring = object(&g, 1);
    pay(&mut g, 1, ring);
    settle(&mut g, 1);
    assert_eq!(g.state.rules.players[1].life, 35.into());
    pool(&g, 1, 0);
    recover(&g);
}
