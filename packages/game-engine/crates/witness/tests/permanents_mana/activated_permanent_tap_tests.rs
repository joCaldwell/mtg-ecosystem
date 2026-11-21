//! Complete Icy Manipulator/Regress sources in private configurations, not pods.
//! All tap states, costs and zone changes below use accepted runtime inputs.
//! Pinned CR113.7a,602.1a/2b,608.2b,609.3,701.26a:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

fn prepared(rows: &[(&str, u8, bool)]) -> Prepared {
    prepared_with_catalog(rows, tap_catalog())
}

fn tap_catalog() -> Catalog {
    let mut catalog = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap();
    for raw in [
        include_str!("../../../../tests/fixtures/3608f1f7-8dc5-4dd1-ae91-c830e1de9529.json"),
        include_str!("../../../../tests/fixtures/2fa763ad-4d94-4c3a-a099-13ac22c09ce4.json"),
    ] {
        let mtg_compiler::CompileResult::Lowered { definition } = mtg_witness::compile(
            include_str!("../../../../tests/fixtures/archive-manifest-current.json"),
            raw,
        ) else {
            panic!("complete activated tap witness source must lower");
        };
        if definition.name == "Icy Manipulator" {
            assert!(
                definition
                    .capabilities
                    .contains(&Capability::ActivatedTapPermanentV1)
            );
            assert!(
                definition
                    .capabilities
                    .contains(&Capability::TapPermanentV1)
            );
            assert_eq!(definition.activated.len(), 1);
        }
        catalog.definitions.push(*definition);
    }
    catalog
}

fn prepared_with_catalog(rows: &[(&str, u8, bool)], mut catalog: Catalog) -> Prepared {
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    let mut config = ScenarioConfig::turns_permanents();
    config.profile = "activated-permanent-tap-source-witness".into();
    config.initial.turn_step = TurnStep::PrecombatMain;
    config.cards = rows
        .iter()
        .enumerate()
        .map(|(id, (name, seat, hand))| CardSetup {
            id: CardId(id as u32),
            oracle_id: catalog
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
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(r) => return r,
            other => panic!("activated tap {:?}: {other:?}", g.state.rules.frame),
        }
    }
    panic!("activated tap work bound");
}
fn answer(g: &mut Game, quantum: u64, response: Response) {
    let r = request(g, quantum);
    g.respond(r.player(), r.decision_id(), response).unwrap();
}
fn priority(g: &mut Game, quantum: u64, seat: u8) {
    for _ in 0..8 {
        let r = request(g, quantum);
        assert!(matches!(r, Request::Priority { .. }));
        if r.player() == PlayerId(seat) {
            return;
        }
        answer(g, quantum, Response::Pass);
    }
    panic!("priority bound");
}
fn pay(g: &mut Game, quantum: u64, target: ObjectId, lands: &[u32]) -> StackId {
    answer(
        g,
        quantum,
        Response::Targets {
            objects: vec![target],
        },
    );
    for id in lands {
        let land = object(g, *id);
        answer(g, quantum, Response::ActivateMana { object: land });
    }
    answer(g, quantum, Response::FinishMana { undo_mana: false });
    assert!(matches!(request(g, quantum), Request::SpendMana { .. }));
    answer(
        g,
        quantum,
        Response::SpendMana {
            spending: BTreeMap::from([(Color::U, Quantity::from(lands.len() as i64))]),
        },
    );
    assert!(matches!(request(g, quantum), Request::Priority { .. }));
    g.top().unwrap().id
}
fn activate(g: &mut Game, quantum: u64, target: ObjectId) -> StackId {
    let source = object(g, 0);
    priority(g, quantum, 0);
    answer(
        g,
        quantum,
        Response::ActivateAbility {
            object: source,
            ability: 0,
        },
    );
    assert!(matches!(request(g, quantum), Request::Targets { .. }));
    let stack = pay(g, quantum, target, &[1]);
    assert_eq!(
        g.top().unwrap().paid,
        Some(ManaCost {
            generic: 1.into(),
            colored: vec![]
        })
    );
    assert!(g.current(source).unwrap().tapped);
    assert!(g.current(object(g, 1)).unwrap().tapped);
    assert_eq!(g.mana(PlayerId(0), Color::U), Some(Quantity::zero()));
    stack
}
fn finish_to(g: &mut Game, quantum: u64, remaining: usize) {
    for _ in 0..30 {
        assert!(matches!(request(g, quantum), Request::Priority { .. }));
        if g.state.rules.stack.len() == remaining {
            return;
        }
        answer(g, quantum, Response::Pass);
    }
    panic!("activated tap completion bound");
}
fn resolve_start(g: &mut Game) {
    for _ in 0..30 {
        if matches!(&g.state.rules.frame,Frame::Resolve { frame } if frame.cursor == 0) {
            return;
        }
        match g.advance(Budget::new(1)) {
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(r) if matches!(r, Request::Priority { .. }) => {
                g.respond(r.player(), r.decision_id(), Response::Pass)
                    .unwrap();
            }
            other => panic!("activation resolution boundary {other:?}"),
        }
    }
    panic!("activation resolution boundary work bound");
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
}
fn reject(g: &mut Game, r: &Request, player: PlayerId, response: Response) {
    let before = g.snapshot().unwrap();
    assert!(g.respond(player, r.decision_id(), response).is_err());
    assert_eq!(g.snapshot().unwrap(), before);
}
fn taps(g: &Game, target: ObjectId) -> usize {
    g.events().iter().filter(|e|matches!(e,Event::Committed { actual:EventOutcome::Tapped { object },.. } if *object == target)).count()
}
fn completed(g: &Game, stack: StackId) {
    if g.trace.mode == TraceMode::Detailed {
        assert!(
            g.events()
                .iter()
                .any(|e| matches!(e,Event::AbilityCompleted { stack_id } if *stack_id == stack))
        );
    }
}
#[test]
fn activated_permanent_tap_remains_outside_reviewed_hosting_and_search() {
    use mtg_core::testkit::{Feature, ProfileError};
    let p = prepared(&[("Icy Manipulator", 0, false), ("Island", 0, false)]);
    let profile = mtg_core::testkit::match_profile("commander-foundations-v1").unwrap();
    for feature in [Feature::Hosting, Feature::Search] {
        assert!(matches!(
            profile.require_feature(&p.catalog, feature),
            Err(ProfileError::UnreviewedCapabilities { feature: actual, capabilities })
                if actual == feature && capabilities.contains(&Capability::ActivatedTapPermanentV1)
        ));
    }
}
fn foreign_handle_rejected(g: &Game) {
    let QueryResult::Complete(QueryValue::Decision(Some(PlayerRequest::Targets { token, .. }))) =
        g.player_decision_for(PlayerId(0), Budget::unlimited())
    else {
        panic!("own target decision");
    };
    let QueryResult::Complete(QueryValue::Observation(foreign)) =
        g.observe(PlayerId(1), Budget::unlimited())
    else {
        panic!("foreign observation");
    };
    let icy = g.definition(CardId(0)).unwrap().oracle_id.clone();
    let handle = foreign
        .view
        .objects
        .iter()
        .find(|o| o.oracle_id == icy && o.zone == Zone::Battlefield)
        .unwrap()
        .handle
        .clone();
    let before = g.snapshot().unwrap();
    assert!(matches!(
        g.check_player_response(
            PlayerId(0),
            token,
            PlayerResponse::Targets {
                objects: vec![handle]
            },
            Budget::unlimited()
        ),
        QueryResult::Rejected(QueryRejection::InvalidResponse)
    ));
    assert_eq!(g.snapshot().unwrap(), before);
}

#[test]
fn activated_permanent_tap_union_targets_pay_cost_and_reject_ineligible_objects_atomically() {
    for name in ["Sol Ring", "Grizzly Bears", "Island"] {
        let p = prepared(&[
            ("Icy Manipulator", 0, false),
            ("Island", 0, false),
            (name, 2, false),
            ("Glorious Anthem", 2, false),
            ("Sol Ring", 1, true),
        ]);
        let mut baseline = None;
        for (quantum, mode) in [(1, TraceMode::Detailed), (4096, TraceMode::Off)] {
            let mut g = Game::with_trace(p.clone(), mode);
            let source = object(&g, 0);
            let target = object(&g, 2);
            answer(
                &mut g,
                quantum,
                Response::ActivateAbility {
                    object: source,
                    ability: 0,
                },
            );
            let r = request(&mut g, quantum);
            assert!(
                matches!(&r,Request::Targets { candidates,.. } if candidates.contains(&source) && candidates.contains(&object(&g,1)) && candidates.contains(&target) && !candidates.contains(&object(&g,3)) && !candidates.contains(&object(&g,4)))
            );
            for id in [3, 4] {
                let invalid = object(&g, id);
                reject(
                    &mut g,
                    &r,
                    r.player(),
                    Response::Targets {
                        objects: vec![invalid],
                    },
                );
            }
            reject(
                &mut g,
                &r,
                PlayerId(1),
                Response::Targets {
                    objects: vec![target],
                },
            );
            foreign_handle_rejected(&g);
            recover(&g);
            let stack = pay(&mut g, quantum, target, &[1]);
            assert!(g.current(source).unwrap().tapped);
            assert_eq!(
                g.top().unwrap().paid,
                Some(ManaCost {
                    generic: 1.into(),
                    colored: vec![]
                })
            );
            recover(&g);
            let r = request(&mut g, quantum);
            reject(
                &mut g,
                &r,
                PlayerId(0),
                Response::ActivateAbility {
                    object: source,
                    ability: 0,
                },
            );
            finish_to(&mut g, quantum, 0);
            completed(&g, stack);
            assert!(g.current(target).unwrap().tapped);
            if mode == TraceMode::Detailed {
                assert_eq!(taps(&g, source), 1);
                assert_eq!(taps(&g, target), 1);
            }
            recover(&g);
            if let Some(old) = &baseline {
                assert_eq!(old, &g.state);
            } else {
                baseline = Some(g.state.clone());
            }
        }
    }
}
#[test]
fn paid_creature_source_tap_checks_target_and_pays_before_resolution() {
    let record =
        include_str!("../../../../tests/fixtures/00d11662-9d10-4915-b80c-c1ad54b4e71b.json");
    let manifest = include_str!("../../../../tests/fixtures/archive-manifest-current.json");
    assert_eq!(
        sha256(record.trim_end().as_bytes()),
        "4f16034cdfdd8b8cf0bec6de688533a427eb89b882205497497a08aaf84b56ea"
    );
    let mtg_compiler::CompileResult::Lowered { definition } =
        mtg_witness::compile(manifest, record)
    else {
        panic!("complete Half-Elf Monk source must lower");
    };
    assert!(
        definition
            .capabilities
            .contains(&Capability::ActivatedCreatureTapV1)
    );
    assert!(
        !definition
            .capabilities
            .contains(&Capability::ActivatedTapPermanentV1)
    );
    let mut catalog = tap_catalog();
    catalog.definitions.push(*definition);
    let p = prepared_with_catalog(
        &[
            ("Half-Elf Monk", 0, false),
            ("Plains", 0, false),
            ("Plains", 0, false),
            ("Grizzly Bears", 2, false),
            ("Island", 2, false),
        ],
        catalog,
    );
    let mut game = Game::with_trace(p, TraceMode::Detailed);
    let source = object(&game, 0);
    let target = object(&game, 3);
    let noncreature = object(&game, 4);
    const WORK: u64 = 4096;
    answer(
        &mut game,
        WORK,
        Response::ActivateAbility {
            object: source,
            ability: 0,
        },
    );
    let targets = request(&mut game, WORK);
    assert!(matches!(&targets,Request::Targets {candidates,..}
        if candidates.contains(&target) && !candidates.contains(&noncreature)));
    reject(
        &mut game,
        &targets,
        PlayerId(0),
        Response::Targets {
            objects: vec![noncreature],
        },
    );
    answer(
        &mut game,
        WORK,
        Response::Targets {
            objects: vec![target],
        },
    );
    for card in [1, 2] {
        let land = object(&game, card);
        answer(&mut game, WORK, Response::ActivateMana { object: land });
    }
    answer(&mut game, WORK, Response::FinishMana { undo_mana: false });
    answer(
        &mut game,
        WORK,
        Response::SpendMana {
            spending: BTreeMap::from([(Color::W, 2.into())]),
        },
    );
    assert!(matches!(request(&mut game, WORK), Request::Priority { .. }));
    assert_eq!(
        game.top().unwrap().paid,
        Some(ManaCost {
            generic: 1.into(),
            colored: vec![Color::W]
        })
    );
    assert!(game.current(source).unwrap().tapped);
    assert!(!game.current(target).unwrap().tapped);
    let stack = game.top().unwrap().id;
    finish_to(&mut game, WORK, 0);
    completed(&game, stack);
    assert!(game.current(target).unwrap().tapped);
    assert_eq!(taps(&game, source), 1);
    assert_eq!(taps(&game, target), 1);
}

#[test]
fn activated_permanent_tap_self_target_cost_taps_once_and_resolution_is_noop() {
    let p = prepared(&[("Icy Manipulator", 0, false), ("Island", 0, false)]);
    let mut baseline = None;
    for (quantum, mode) in [(1, TraceMode::Detailed), (4096, TraceMode::Off)] {
        let mut g = Game::with_trace(p.clone(), mode);
        let source = object(&g, 0);
        let stack = activate(&mut g, quantum, source);
        recover(&g);
        finish_to(&mut g, quantum, 0);
        completed(&g, stack);
        assert!(g.current(source).unwrap().tapped);
        if mode == TraceMode::Detailed {
            assert_eq!(taps(&g, source), 1);
        }
        recover(&g);
        if let Some(old) = &baseline {
            assert_eq!(old, &g.state);
        } else {
            baseline = Some(g.state.clone());
        }
    }
}
fn bounce_case(source_bounce: bool, quantum: u64, mode: TraceMode) -> Game {
    let mut g = Game::with_trace(
        prepared(&[
            ("Icy Manipulator", 0, false),
            ("Island", 0, false),
            ("Grizzly Bears", 2, false),
            ("Regress", 1, true),
            ("Island", 1, false),
            ("Island", 1, false),
            ("Island", 1, false),
        ]),
        mode,
    );
    let source = object(&g, 0);
    let target = object(&g, 2);
    let ability = activate(&mut g, quantum, target);
    recover(&g);
    priority(&mut g, quantum, 1);
    answer(&mut g, quantum, Response::Cast { card: CardId(3) });
    let bounced = if source_bounce { source } else { target };
    pay(&mut g, quantum, bounced, &[4, 5, 6]);
    finish_to(&mut g, quantum, 1);
    assert!(g.current(bounced).is_err());
    let successor = object(&g, bounced.card.0);
    assert_eq!(
        g.current(successor).unwrap().zone,
        Zone::Hand {
            owner: if source_bounce {
                PlayerId(0)
            } else {
                PlayerId(2)
            }
        }
    );
    recover(&g);
    resolve_start(&mut g);
    assert!(
        matches!(&g.state.rules.frame,Frame::Resolve { frame } if frame.stack_id == ability && frame.legal_targets == [source_bounce])
    );
    recover(&g);
    finish_to(&mut g, quantum, 0);
    completed(&g, ability);
    if source_bounce {
        assert!(g.current(target).unwrap().tapped);
    } else {
        assert!(g.current(source).unwrap().tapped);
    }
    if mode == TraceMode::Detailed {
        assert_eq!(taps(&g, source), 1);
        assert_eq!(taps(&g, target), usize::from(source_bounce));
    }
    recover(&g);
    g
}
#[test]
fn activated_permanent_tap_paid_ability_survives_real_regress_source_bounce() {
    let a = bounce_case(true, 1, TraceMode::Detailed);
    let b = bounce_case(true, 4096, TraceMode::Off);
    assert_eq!(a.state, b.state);
    assert_eq!(a.replay_journal(), b.replay_journal());
}
#[test]
fn activated_permanent_tap_real_target_bounce_fizzles_without_refunding_source_cost() {
    let a = bounce_case(false, 1, TraceMode::Detailed);
    let b = bounce_case(false, 4096, TraceMode::Off);
    assert_eq!(a.state, b.state);
    assert_eq!(a.replay_journal(), b.replay_journal());
}

fn assert_city_damage(g: &Game, city: ObjectId, controller: PlayerId) {
    assert_eq!(taps(g, city), 1);
    let mut collected = g.events().iter().filter_map(|event| match event {
        Event::TriggerCollected {
            source,
            player,
            witness: TriggerEventWitness::Tapped { object, controller },
            source_facts,
            ..
        } if *source == city => Some((*player, *object, *controller, source_facts.as_ref())),
        _ => None,
    });
    let (player, object, at_tap, facts) = collected.next().unwrap();
    assert_eq!((player, object, at_tap), (controller, city, controller));
    assert_eq!(facts.zone, Zone::Battlefield);
    assert!(facts.tapped);
    assert!(collected.next().is_none());

    let mut damage = g.events().iter().filter_map(|event| match event {
        Event::Committed {
            actual: EventOutcome::DamageCommitted { actual, .. },
            ..
        } => Some(actual),
        _ => None,
    });
    let [hit] = damage.next().unwrap().as_slice() else {
        panic!("expected one damage transaction member");
    };
    assert_eq!(hit.source.object, city);
    assert_eq!(hit.controller, controller);
    assert_eq!(
        hit.recipient,
        DamageRecipient::Player { player: controller }
    );
    assert_eq!(hit.amount, 1.into());
    assert!(damage.next().is_none());
    assert_eq!(g.state.rules.players[controller.0 as usize].life, 39.into());
    assert_eq!(g.state.rules.players[0].life, 40.into());
}

#[test]
fn city_of_brass_paid_taps_queue_damage_for_trigger_controller() {
    let result = mtg_witness::compile(
        include_str!("../../../../tests/fixtures/ramos/archive-manifest-current.json"),
        include_str!("../../../../tests/fixtures/ramos/f25351e3-539b-4bbc-b92d-6480acf4d722.json"),
    );
    let mtg_compiler::CompileResult::Lowered { definition } = result else {
        panic!("complete tap damage source must lower: {result:?}");
    };
    assert_eq!(
        definition.capabilities,
        BTreeSet::from([
            Capability::LandPlayV1,
            Capability::NormalLayoutV1,
            Capability::NoncombatDamageV1,
            Capability::PermanentManaV1,
            Capability::SourceTapTriggerV1,
            Capability::TriggeredAbilityV1,
        ])
    );
    let [mana] = definition.mana_abilities.as_slice() else {
        panic!("expected the separate mana ability");
    };
    assert_eq!(mana.production, ManaProduction::AnyColor);
    assert_eq!(mana.self_damage, None);
    let labels = definition.source_labels.as_ref().unwrap();
    assert_eq!(labels.triggered, [0]);
    assert_eq!(labels.mana, [1]);
    let mut incomplete = definition.as_ref().clone();
    incomplete.triggered.clear();
    incomplete.capabilities = incomplete.requirements();
    assert!(incomplete.validate().is_err());

    let mut catalog = tap_catalog();
    catalog.definitions.push(*definition);
    let mut g = Game::with_trace(
        prepared_with_catalog(
            &[
                ("Icy Manipulator", 0, false),
                ("Island", 0, false),
                ("City of Brass", 2, false),
                ("Regress", 1, true),
                ("Island", 1, false),
                ("Island", 1, false),
                ("Island", 1, false),
            ],
            catalog,
        ),
        TraceMode::Detailed,
    );
    let city = object(&g, 2);
    let controller = PlayerId(2);
    let icy = activate(&mut g, 1, city);
    assert!(!g.current(city).unwrap().tapped);

    // The City can pay its own mana cost in response. It produces mana before
    // the tap trigger goes on the stack. The pending Icy tap then does nothing.
    let mut mana_branch = g.fork();
    priority(&mut mana_branch, 1, controller.0);
    answer(&mut mana_branch, 1, Response::ActivateMana { object: city });
    assert!(matches!(
        request(&mut mana_branch, 1),
        Request::ManaColor { .. }
    ));
    assert!(mana_branch.current(city).unwrap().tapped);
    assert_eq!(mana_branch.state.rules.stack.len(), 1);
    assert_eq!(mana_branch.state.rules.players[2].life, 40.into());
    answer(&mut mana_branch, 1, Response::ManaColor { color: Color::U });
    assert!(matches!(
        request(&mut mana_branch, 1),
        Request::Priority { .. }
    ));
    assert_eq!(mana_branch.mana(controller, Color::U), Some(1.into()));
    assert_eq!(mana_branch.state.rules.players[2].life, 40.into());
    assert_eq!(mana_branch.state.rules.stack.len(), 2);
    assert_eq!(mana_branch.state.rules.stack[0].id, icy);
    assert!(matches!(
        &mana_branch.top().unwrap().kind,
        StackKind::Triggered { .. }
    ));
    finish_to(&mut mana_branch, 1, 0);
    assert_city_damage(&mana_branch, city, controller);

    // In the other branch the paid opponent's ability actually taps the land.
    // A paid response removes the source while its separate trigger waits.
    resolve_start(&mut g);
    finish_to(&mut g, 1, 1);
    let trigger = g.top().unwrap();
    assert!(matches!(&trigger.kind, StackKind::Triggered { .. }));
    assert_eq!(trigger.controller, controller);
    assert_eq!(trigger.source, city);
    assert_ne!(trigger.id, icy);
    assert_eq!(g.state.rules.players[2].life, 40.into());

    priority(&mut g, 1, 1);
    answer(&mut g, 1, Response::Cast { card: CardId(3) });
    pay(&mut g, 1, city, &[4, 5, 6]);
    finish_to(&mut g, 1, 1);
    assert!(g.current(city).is_err());
    let successor = object(&g, city.card.0);
    assert_eq!(g.zone(successor), Some(Zone::Hand { owner: controller }));
    recover(&g);
    finish_to(&mut g, 1, 0);
    assert_city_damage(&g, city, controller);
    recover(&g);
}
