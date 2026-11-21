//! Actual paid sacrifice in a private scenario with exact archived card sources.
use super::*;

const OWNER: PlayerId = PlayerId(0);
const ENIGMA: CardId = CardId(1);
const ORACLE: &str = "6fb84ecd-ab60-4c2c-987d-8a0749db5777";

fn prepared(library_len: u32) -> Prepared {
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
            include_str!("../../../../tests/fixtures/ramos/archive-manifest-current.json"),
            include_str!(
                "../../../../tests/fixtures/ramos/6fb84ecd-ab60-4c2c-987d-8a0749db5777.json"
            ),
        ),
    ] {
        let compiled = mtg_compiler::compile(header, raw);
        let mtg_compiler::CompileResult::Lowered { definition } = compiled else {
            panic!("{compiled:?}");
        };
        catalog.definitions.push(*definition);
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    let mut config = ScenarioConfig::turns_permanents();
    config.profile = "successor-tuck-private-witness".into();
    config.initial.turn_step = TurnStep::PrecombatMain;
    config.cards = ["Shrapnel Blast", "Enigma Sphinx", "Sol Ring", "Mountain"]
        .iter()
        .enumerate()
        .map(|(id, name)| CardSetup {
            id: CardId(id as u32),
            oracle_id: catalog
                .definitions
                .iter()
                .find(|d| d.name == *name)
                .unwrap()
                .oracle_id
                .clone(),
            owner: OWNER,
            controller: OWNER,
            zone: if id == 0 {
                Zone::Hand { owner: OWNER }
            } else {
                Zone::Battlefield
            },
        })
        .chain((4..4 + library_len).map(|id| CardSetup {
            id: CardId(id),
            oracle_id: FOREST.into(),
            owner: OWNER,
            controller: OWNER,
            zone: Zone::Library { owner: OWNER },
        }))
        .collect();
    Prepared {
        catalog_digest: digest(&catalog).unwrap(),
        catalog: Arc::new(catalog),
        profile_digest: digest(&config).unwrap(),
        config,
        match_config: None,
    }
}

fn request(game: &mut Game) -> Request {
    let Advance::AwaitingDecision(request) = game.advance(Budget::unlimited()) else {
        panic!("expected decision in {:?}", game.state.rules.frame);
    };
    request
}

fn answer(game: &mut Game, response: Response) {
    let r = request(game);
    game.respond(r.player(), r.decision_id(), response).unwrap();
}

fn paid_death(game: &mut Game) -> ObjectId {
    let source = game.object_id(ENIGMA).unwrap();
    answer(game, Response::Cast { card: CardId(0) });
    answer(
        game,
        Response::EntityTargets {
            entities: vec![TargetEntity::Player {
                player: PlayerId(1),
            }],
        },
    );
    for card in [CardId(2), CardId(3)] {
        let object = game.object_id(card).unwrap();
        answer(game, Response::ActivateMana { object });
    }
    answer(game, Response::FinishMana { undo_mana: false });
    answer(
        game,
        Response::SpendMana {
            spending: BTreeMap::from([(Color::C, 1.into()), (Color::R, 1.into())]),
        },
    );
    answer(game, Response::SacrificeCost { object: source });
    assert!(matches!(request(game), Request::Priority { .. }));
    let successor = game.object_id(ENIGMA).unwrap();
    assert_ne!(successor, source);
    assert_eq!(game.zone(successor), Some(Zone::Graveyard { owner: OWNER }));
    assert_eq!(game.state.rules.stack.len(), 2);
    assert!(matches!(
        &game.top().unwrap().kind,
        StackKind::Triggered {
            witness: TriggerEventWitness::Moved { movement },
            ..
        } if movement.previous == source && movement.successor == successor
    ));
    successor
}

fn settle(game: &mut Game) {
    for _ in 0..12 {
        let r = request(game);
        assert!(matches!(r, Request::Priority { .. }), "{r:?}");
        if game.state.rules.stack.is_empty() {
            return;
        }
        game.respond(r.player(), r.decision_id(), Response::Pass)
            .unwrap();
    }
    panic!("paid spell and trigger did not finish");
}

fn observe(game: &Game, viewer: PlayerId) -> PermittedView {
    let QueryResult::Complete(QueryValue::Observation(observation)) =
        game.observe(viewer, Budget::unlimited())
    else {
        panic!("expected permitted observation");
    };
    observation.view
}

fn position(position: u32) -> Vec<KnownLibraryPosition> {
    vec![KnownLibraryPosition {
        library_owner: OWNER,
        position,
        oracle_id: ORACLE.into(),
    }]
}

#[test]
fn successor_tuck_paid_death_recovery_and_public_position() {
    let p = prepared(4);
    let mut g = Game::with_trace(p.clone(), TraceMode::Detailed);
    let successor = paid_death(&mut g);
    let preresolution = observe(&g, PlayerId(1));
    let graveyard_handle = preresolution
        .objects
        .iter()
        .find(|o| o.oracle_id == ORACLE)
        .unwrap()
        .handle
        .clone();
    assert!(preresolution.known_library_positions.is_empty());
    let mut restored = Game::restore(p.clone(), &g.snapshot().unwrap()).unwrap();
    settle(&mut g);
    settle(&mut restored);
    assert_eq!(restored.state, g.state);
    assert_eq!(restored.trace(), g.trace());
    assert_eq!(
        g.state.rules.libraries[&OWNER],
        [CardId(4), CardId(5), ENIGMA, CardId(6), CardId(7)]
    );
    assert_eq!(g.state.rules.players[1].life, 35.into());
    assert!(g.events().iter().any(|event| matches!(
        event,
        Event::Committed {
            proposed: ProposedEvent::PutInLibraryAtPosition { object, player: OWNER, .. },
            actual: EventOutcome::Moved { movement },
        } if *object == successor && movement.previous == successor
    )));
    let tucked = g.object_id(ENIGMA).unwrap();
    for viewer in [OWNER, PlayerId(1)] {
        let view = observe(&g, viewer);
        assert_eq!(view.known_library_positions, position(3));
        assert!(!view.objects.iter().any(|o| o.oracle_id == ORACLE));
        assert!(g.handle_for(viewer, tucked).is_err());
    }
    assert!(g.handle_for(PlayerId(1), successor).is_err());
    assert!(
        g.state.knowledge.visibility[1]
            .history
            .iter()
            .any(|stored| {
                stored.to_public().objects.iter().any(|o| {
                    o.handle == graveyard_handle && o.zone == (Zone::Graveyard { owner: OWNER })
                })
            })
    );
    assert_eq!(
        Game::restore(p, &g.snapshot().unwrap()).unwrap().state,
        g.state
    );

    // Exercise ordinary library removal and insertion on the same memory.
    let old_top = g.object_id(CardId(4)).unwrap();
    g.move_object(old_top, Zone::Hand { owner: OWNER }).unwrap();
    assert_eq!(g.known_library_positions(PlayerId(1)).unwrap(), position(2));
    let mountain = g.object_id(CardId(3)).unwrap();
    g.commit(
        ProposedEvent::PutOnTopOwnerLibrary { object: mountain },
        BatchCause::Fixture,
    )
    .unwrap();
    assert_eq!(g.known_library_positions(PlayerId(1)).unwrap(), position(3));
}

#[test]
fn successor_tuck_short_library_and_moved_successor() {
    let mut short = Game::new(prepared(1));
    let successor = paid_death(&mut short);
    let mut moved = short.fork();
    let exiled = moved.move_object(successor, Zone::Exile).unwrap().successor;
    let returned = moved
        .move_object(exiled, Zone::Graveyard { owner: OWNER })
        .unwrap()
        .successor;
    settle(&mut moved);
    assert_eq!(moved.zone(returned), Some(Zone::Graveyard { owner: OWNER }));
    assert_eq!(moved.state.rules.libraries[&OWNER], [CardId(4)]);
    assert!(observe(&moved, OWNER).known_library_positions.is_empty());

    settle(&mut short);
    assert_eq!(short.state.rules.libraries[&OWNER], [CardId(4), ENIGMA]);
    assert_eq!(
        observe(&short, PlayerId(1)).known_library_positions,
        position(2)
    );
}
