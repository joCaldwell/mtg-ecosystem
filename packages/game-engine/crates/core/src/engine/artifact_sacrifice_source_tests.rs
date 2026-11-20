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

fn fixed_artifact_setup(with_mana: bool) -> Prepared {
    let mut setup = prepared(&[
        ("Grizzly Bears", 0, 0, false),
        ("Sol Ring", 1, 0, false),
        ("Alpha Myr", 2, 0, false),
        ("Commander's Sphere", 2, 0, false),
        ("Mountain", 0, 0, false),
    ]);
    let catalog = Arc::make_mut(&mut setup.catalog);
    let mut source = catalog
        .definition_by_oracle_id(&setup.config.cards[0].oracle_id)
        .unwrap()
        .clone();
    source.name = "Foundry Tallykeeper".into();
    source.oracle_id = "synthetic-foundry-tallykeeper".into();
    source.source_labels = None;
    source.activated = vec![ActivatedAbility {
        functioning_zone: FunctioningZone::Battlefield,
        cost: ActivationCost::SacrificePermanent {
            count: 2,
            filter: SacrificeFilter {
                types: vec![CardKind::Artifact],
                subtypes: vec![],
            },
            mana: with_mana.then(|| ManaCost {
                generic: 0.into(),
                colored: vec![Color::R],
            }),
        },
        program: SpellProgram {
            functioning_zone: FunctioningZone::Stack,
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            additional_sacrifice: None,
            optional_additional_costs: vec![],
            targets: vec![],
            modes: vec![],
            instructions: vec![Instruction::DrawCards {
                count: 1.into(),
                recipient: None,
                source_block: 0,
            }],
        },
    }];
    source.capabilities = source.requirements();
    assert!(
        source
            .capabilities
            .contains(&Capability::FixedArtifactSacrificeCostV1)
    );
    // A count can exceed the temporary battlefield and even the original
    // byte width. Definition validity is independent of current payability.
    let mut large = source.activated[0].clone();
    if let ActivationCost::SacrificePermanent { count, .. } = &mut large.cost {
        *count = u32::MAX;
    }
    source.activated.push(large);
    let mut modal = source.activated[0].clone();
    let draw = modal.program.clone();
    let mut damage = draw.clone();
    damage.targets.push(TargetSpec {
        slot: 0,
        selection_id: "synthetic-target".into(),
        chooser: Chooser::Controller,
        stage: EvaluationStage::Announcement,
        domain: TargetDomain::AnyDamageTarget,
        filter: None,
    });
    damage.instructions = vec![Instruction::DealDamage {
        target_slot: 0,
        amount: 3.into(),
        source_block: 0,
    }];
    modal.program.instructions.clear();
    modal.program.modes = vec![SpellMode { program: draw }, SpellMode { program: damage }];
    source.activated.push(modal);
    source.capabilities = source.requirements();
    setup.config.cards[0].oracle_id = source.oracle_id.clone();
    catalog.definitions.push(source);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    setup.catalog_digest = digest(catalog).unwrap();
    setup.profile_digest = digest(&setup.config).unwrap();
    setup
}

// Original private IR witness. Counted source records stay outside compiler
// admission until a complete ability (cost, effect and references) can lower.
#[test]
fn fixed_artifact_sacrifice_private_payment_and_departure() {
    let payer = PlayerId(0);
    for with_mana in [false, true] {
        let mut g = Game::with_trace(fixed_artifact_setup(with_mana), TraceMode::Detailed);
        let [origin, first, second, third, land] = [0, 1, 2, 3, 4].map(|id| object(&g, id));
        let priority = request(&mut g, 1);
        reject(
            &mut g,
            payer,
            priority.decision_id(),
            Response::ActivateAbility {
                object: origin,
                ability: 1,
            },
        );

        answer(
            &mut g,
            1,
            Response::ActivateAbility {
                object: origin,
                ability: 0,
            },
        );
        if with_mana {
            answer(&mut g, 1, Response::ActivateMana { object: land });
            answer(&mut g, 1, Response::FinishMana { undo_mana: false });
            answer(
                &mut g,
                1,
                Response::SpendMana {
                    spending: BTreeMap::from([(Color::R, 1.into())]),
                },
            );
        }
        let r = request(&mut g, 1);
        assert!(
            matches!(&r, Request::SacrificeCost { player,candidates,.. } if *player == payer && candidates == &[first,second])
        );
        reject(
            &mut g,
            payer,
            r.decision_id(),
            Response::SacrificeCost { object: third },
        );
        answer(&mut g, 1, Response::SacrificeCost { object: first });
        let r = request(&mut g, 1);
        assert!(
            matches!(&r, Request::SacrificeCost {candidates,..} if candidates == &[second,third])
        );
        reject(
            &mut g,
            payer,
            r.decision_id(),
            Response::SacrificeCost { object: first },
        );
        for card in [first, second, third] {
            assert_eq!(g.zone(card), Some(Zone::Battlefield));
        }
        if with_mana {
            pool(&g, 0, 1);
        }
        let QueryResult::Complete(QueryValue::Observation(other)) =
            g.observe(PlayerId(3), Budget::unlimited())
        else {
            panic!("opponent observation")
        };
        assert!(other.decision.is_none());
        recover(&g);

        // Losing a selected borrowed artifact leaves a new, complete pair.
        let mut restart = g.fork();
        restart.concede(PlayerId(1)).unwrap();
        assert!(
            matches!(request(&mut restart,1),Request::SacrificeCost { candidates,.. } if candidates == [second])
        );
        recover(&restart);
        // Losing both unselected artifacts makes the activation unpayable.
        let mut short = g.fork();
        short.concede(PlayerId(2)).unwrap();
        assert!(matches!(request(&mut short, 1), Request::Priority { .. }));
        assert!(short.state.rules.stack.is_empty());
        assert_eq!(short.zone(first), Some(Zone::Battlefield));
        if with_mana {
            pool(&short, 0, 1);
        }
        recover(&short);

        answer(&mut g, 1, Response::SacrificeCost { object: third });
        assert!(matches!(request(&mut g, 1), Request::Priority { .. }));
        let stack = g.top().unwrap();
        assert_eq!(stack.sacrificed_set, [first, third]);
        let id = stack.id;
        let occurrences = [first, third]
            .map(|object| ProposedEvent::Sacrifice {
                object,
                player: payer,
            })
            .to_vec();
        assert_eq!(
            g.trace()
                .records
                .iter()
                .filter(|r| matches!(&r.record, EvidenceRecord::Batch{batch}
            if matches!(batch.cause,BatchCause::Payment{stack_id,..} if stack_id==id)
                && batch.group == EventGroup::Simultaneous{occurrences:occurrences.clone()}))
                .count(),
            1
        );
        assert_eq!(
            g.zone(object(&g, 1)),
            Some(Zone::Graveyard { owner: PlayerId(1) })
        );
        assert_eq!(
            g.zone(object(&g, 3)),
            Some(Zone::Graveyard { owner: PlayerId(2) })
        );
        assert_eq!(g.zone(second), Some(Zone::Battlefield));
        pool(&g, 0, 0);
        recover(&g);
        let previous = g.state.rules.libraries[&payer].len();
        settle(&mut g, 1);
        assert_eq!(g.state.rules.libraries[&payer].len() + 1, previous);
    }
}

#[test]
fn fixed_artifact_modal_activation_announces_child_before_root_payment() {
    let payer = PlayerId(0);
    let target = PlayerId(3);
    for (mode, mana) in [(0, false), (1, false), (0, true), (1, true)] {
        let mut g = Game::with_trace(fixed_artifact_setup(mana), TraceMode::Detailed);
        let [source, first, third, land] = [0, 1, 3, 4].map(|id| object(&g, id));
        let library = g.state.rules.libraries[&payer].len();
        let life = g.state.rules.players[3].life.clone();
        answer(
            &mut g,
            1,
            Response::ActivateAbility {
                object: source,
                ability: 2,
            },
        );
        assert!(matches!(request(&mut g,1),Request::Modes {candidates,..} if candidates==[0,1]));
        recover(&g);
        answer(&mut g, 1, Response::Modes { mode });
        if mode == 1 {
            assert!(matches!(request(&mut g, 1), Request::EntityTargets { .. }));
            recover(&g);
            answer(
                &mut g,
                1,
                Response::EntityTargets {
                    entities: vec![TargetEntity::Player { player: target }],
                },
            );
        }
        if mana {
            assert!(matches!(request(&mut g, 1), Request::Mana { .. }));
            answer(&mut g, 1, Response::ActivateMana { object: land });
            answer(&mut g, 1, Response::FinishMana { undo_mana: false });
            answer(
                &mut g,
                1,
                Response::SpendMana {
                    spending: BTreeMap::from([(Color::R, 1.into())]),
                },
            );
        }
        assert!(matches!(request(&mut g, 1), Request::SacrificeCost { .. }));
        assert_eq!(g.top().unwrap().mode, Some(mode));
        answer(&mut g, 1, Response::SacrificeCost { object: first });
        request(&mut g, 1);
        recover(&g);
        answer(&mut g, 1, Response::SacrificeCost { object: third });
        assert!(matches!(request(&mut g, 1), Request::Priority { .. }));
        assert_eq!(g.top().unwrap().sacrificed_set, [first, third]);
        settle(&mut g, 1);
        let mut expected_life = life;
        if mode == 1 {
            expected_life.subtract(&3.into());
        }
        assert_eq!(g.state.rules.players[3].life, expected_life);
        assert_eq!(
            g.state.rules.libraries[&payer].len(),
            library - usize::from(mode == 0)
        );
        recover(&g);
    }
}

#[test]
fn counted_artifact_compiled_modes_freeze_target_then_pay_root_set() {
    let header = include_str!("../../../../tests/fixtures/archive-manifest-current.json");
    let raw = include_str!("../../../../tests/fixtures/synthetic-counted-artifact-modal.json");
    let mtg_compiler::CompileResult::Lowered { definition } = mtg_compiler::compile(header, raw)
    else {
        panic!("synthetic artifact")
    };
    let mut p = prepared(&[
        ("Sol Ring", 0, 0, false),
        ("Sol Ring", 1, 0, false),
        ("Alpha Myr", 2, 0, false),
        ("Grizzly Bears", 3, 3, false),
        ("Mountain", 0, 0, false),
        ("Mountain", 0, 0, false),
        ("Mountain", 0, 0, false),
        ("Mountain", 0, 0, false),
    ]);
    let catalog = Arc::make_mut(&mut p.catalog);
    p.config.cards[0].oracle_id = definition.oracle_id.clone();
    catalog.definitions.push(*definition);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    p.catalog_digest = digest(catalog).unwrap();
    p.profile_digest = digest(&p.config).unwrap();
    for mode in [0, 1, 4] {
        let mut g = Game::with_trace(p.clone(), TraceMode::Detailed);
        let [dial, ring, myr, bear] = [0, 1, 2, 3].map(|x| object(&g, x));
        answer(
            &mut g,
            1,
            Response::ActivateAbility {
                object: dial,
                ability: 0,
            },
        );
        answer(&mut g, 1, Response::Modes { mode });
        if mode == 1 {
            recover(&g);
            answer(
                &mut g,
                1,
                Response::Targets {
                    objects: vec![bear],
                },
            );
        }
        if mode == 4 {
            let r = request(&mut g, 1);
            assert!(
                matches!(&r,Request::PlayerTargets{occurrences,candidates,..} if occurrences[0].domain==TargetDomain::PlayerOrPlaneswalker && candidates==&g.live_players())
            );
            reject(
                &mut g,
                r.player(),
                r.decision_id(),
                Response::Targets {
                    objects: vec![bear],
                },
            );
            reject(
                &mut g,
                r.player(),
                r.decision_id(),
                Response::EntityTargets {
                    entities: vec![TargetEntity::Object { object: bear }],
                },
            );
            recover(&g);
            answer(
                &mut g,
                1,
                Response::PlayerTargets {
                    players: vec![PlayerId(0)],
                },
            );
        }
        for land in 4..8 {
            let land = object(&g, land);
            answer(&mut g, 1, Response::ActivateMana { object: land });
        }
        answer(&mut g, 1, Response::FinishMana { undo_mana: false });
        answer(
            &mut g,
            1,
            Response::SpendMana {
                spending: BTreeMap::from([(Color::R, 4.into())]),
            },
        );
        for candidate in [dial, ring, myr] {
            answer(&mut g, 1, Response::SacrificeCost { object: candidate });
        }
        assert!(matches!(request(&mut g, 1), Request::Priority { .. }));
        recover(&g);
        settle(&mut g, 1);
        if mode == 0 {
            assert_eq!(g.state.rules.players[0].life, 48.into());
        } else if mode == 4 {
            assert_eq!(g.state.rules.players[0].life, 34.into());
        } else {
            assert_eq!(g.characteristics(bear).unwrap().power, Some(4.into()));
        }
        recover(&g);
    }
}

#[test]
fn fixed_set_exile_moves_one_zone_batch_and_preserves_commander_choice() {
    let manifest = include_str!("../../../../tests/fixtures/archive-manifest-current.json");
    for (n, raw) in include_str!("../../../../tests/fixtures/synthetic-set-exile.jsonl")
        .lines()
        .enumerate()
    {
        let parsed = mtg_compiler::compile(manifest, raw);
        let mtg_compiler::CompileResult::Lowered { definition } = parsed else {
            panic!("{parsed:?}")
        };
        let mut p = prepared(&[
            ("Grizzly Bears", 0, 0, true),
            ("Sol Ring", 0, 0, false),
            ("Alpha Myr", 1, 1, false),
            ("Grizzly Bears", 2, 2, false),
            ("Isamaru, Hound of Konda", 1, 1, false),
            ("Sol Ring", 2, 2, false),
            ("Alpha Myr", 2, 2, true),
        ]);
        p.config.cards[0].oracle_id = definition.oracle_id.clone();
        for id in [4usize, 5] {
            p.config.cards[id].zone = Zone::Graveyard {
                owner: p.config.cards[id].owner,
            };
        }
        p.config.commanders = vec![CardId(4)];
        let catalog = Arc::make_mut(&mut p.catalog);
        catalog.definitions.push(*definition);
        catalog
            .definitions
            .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
        catalog.validate().unwrap();
        p.catalog_digest = digest(catalog).unwrap();
        p.profile_digest = digest(&p.config).unwrap();
        let mut g = Game::with_trace(p, TraceMode::Detailed);
        answer(&mut g, 1, Response::Cast { card: CardId(0) });
        answer(&mut g, 1, Response::FinishMana { undo_mana: false });
        g.complete_test_mana();
        let targets = if n == 0 {
            vec![CardId(1), CardId(2)]
        } else {
            vec![CardId(4), CardId(5)]
        };
        let mut chose = false;
        loop {
            let r = request(&mut g, 1);
            match r {
                Request::CommanderReturn { .. } => {
                    assert!(n != 0 && !chose);
                    recover(&g);
                    assert_eq!(g.state.rules.objects[&CardId(4)].zone, Zone::Exile);
                    answer(&mut g, 1, Response::CommanderReturn { to_command: true });
                    chose = true;
                }
                Request::Priority { .. } if g.state.rules.stack.is_empty() => break,
                Request::Priority { .. } => answer(&mut g, 1, Response::Pass),
                other => panic!("{other:?}"),
            }
        }
        let moved: Vec<_> = g
            .events()
            .iter()
            .filter_map(|e| match e {
                Event::Committed {
                    actual: EventOutcome::Moved { movement },
                    ..
                } if movement.destination == Zone::Exile => Some(movement.previous.card),
                _ => None,
            })
            .collect();
        assert_eq!(moved, targets);
        assert_eq!(chose, n != 0);
        assert_eq!(g.state.rules.objects[&CardId(3)].zone, Zone::Battlefield);
        assert_eq!(
            g.state.rules.objects[&CardId(6)].zone,
            Zone::Hand { owner: PlayerId(2) }
        );
        if chose {
            assert_eq!(
                g.state.rules.objects[&CardId(4)].zone,
                Zone::Command { owner: PlayerId(1) }
            );
        }
        recover(&g);
    }
}

#[test]
fn paid_set_exile_subset_resolves_printed_order_and_recovers() {
    let raw = include_str!("../../../../tests/fixtures/ramos/4eb813fd-2d5a-4b02-8193-662681ef4e7d.json");
    let parsed = mtg_compiler::compile(
        include_str!("../../../../tests/fixtures/ramos/archive-manifest-current.json"),
        raw,
    );
    let mtg_compiler::CompileResult::Lowered { definition } = parsed else {
        panic!("{parsed:?}")
    };
    let mut p = prepared(&[
        ("Grizzly Bears", 0, 0, true),
        ("Grizzly Bears", 1, 1, false),
        ("Sol Ring", 2, 2, false),
        ("Plains", 0, 0, false),
        ("Plains", 0, 0, false),
        ("Sol Ring", 0, 0, false),
        ("Sol Ring", 0, 0, false),
    ]);
    p.config.cards[0].oracle_id = definition.oracle_id.clone();
    let catalog = Arc::make_mut(&mut p.catalog);
    catalog.definitions.push(*definition);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    p.catalog_digest = digest(catalog).unwrap();
    p.profile_digest = digest(&p.config).unwrap();
    let mut g = Game::with_trace(p, TraceMode::Detailed);
    answer(&mut g, 1, Response::Cast { card: CardId(0) });
    let r = request(&mut g, 1);
    assert!(matches!(&r,Request::Modes{one_or_more:true,candidates,..} if candidates==&[0,1,2,3]));
    for modes in [vec![], vec![1, 1], vec![4]] {
        assert!(
            g.respond(r.player(), r.decision_id(), Response::ModeSubset { modes })
                .is_err()
        );
    }
    recover(&g);
    answer(&mut g, 1, Response::ModeSubset { modes: vec![1, 0] });
    assert!(matches!(request(&mut g, 1), Request::Mana { .. }));
    let plan = g.top().unwrap().mode_subset.as_ref().unwrap();
    assert_eq!(plan.indices, [0, 1]);
    recover(&g);
    let mut bad = g.fork();
    bad.state
        .rules
        .stack
        .last_mut()
        .unwrap()
        .mode_subset
        .as_mut()
        .unwrap()
        .indices
        .reverse();
    assert!(Game::restore(bad.prepared.as_ref().clone(), &bad.snapshot().unwrap()).is_err());
    for id in 3..=6 {
        let object = object(&g, id);
        answer(&mut g, 1, Response::ActivateMana { object });
    }
    answer(&mut g, 1, Response::FinishMana { undo_mana: false });
    answer(&mut g, 1, Response::SpendMana {
        spending: BTreeMap::from([(Color::W, 2.into()), (Color::C, 4.into())]),
    });
    loop {
        let request = request(&mut g, 1);
        if g.state.rules.stack.is_empty() {
            break;
        }
        assert!(matches!(request, Request::Priority { .. }));
        answer(&mut g, 1, Response::Pass);
    }
    let moved = g
        .trace()
        .records
        .iter()
        .filter_map(|row| match &row.record {
            EvidenceRecord::Batch { batch }
                if matches!(batch.cause, BatchCause::Instruction { .. }) =>
            {
                Some(batch)
            }
            _ => None,
        })
        .flat_map(|batch| batch.occurrences.iter())
        .filter_map(|occurrence| match &occurrence.actual {
            EventOutcome::Moved { movement } if movement.destination == Zone::Exile => {
                Some(movement.previous.card)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(moved, [CardId(2), CardId(1)]);
    recover(&g);
}
