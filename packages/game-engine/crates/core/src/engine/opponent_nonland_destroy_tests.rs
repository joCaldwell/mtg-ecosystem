//! Complete frozen sources in one private, paid scenario.
//! CR 109.5, 205.2b, 603.10a, 608.2h, 701.8a and 702.12b:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

fn compile(manifest: &str, source: &str) -> Definition {
    match mtg_compiler::compile(manifest, source) {
        mtg_compiler::CompileResult::Lowered { definition } => *definition,
        other => panic!("expected complete frozen source: {other:?}"),
    }
}

fn request(game: &mut Game) -> Request {
    match game.advance(Budget::unlimited()) {
        Advance::AwaitingDecision(request) => request,
        other => panic!("expected a scenario decision: {other:?}"),
    }
}

fn answer(game: &mut Game, request: &Request, response: Response) {
    game.respond(request.player(), request.decision_id(), response)
        .unwrap();
}

#[test]
fn opponent_nonland_destroy_paid_three_object_batch() {
    const RAMOS_MANIFEST: &str =
        include_str!("../../../../tests/fixtures/ramos/archive-manifest-current.json");
    const ULTIMATUM: &str =
        include_str!("../../../../tests/fixtures/ramos/a6f38908-aa4f-4f99-a28e-85d11dab52e4.json");
    const MANIFEST: &str = include_str!("../../../../tests/fixtures/archive-manifest-current.json");
    assert_eq!(
        sha256(ULTIMATUM.as_bytes()),
        "8c96b59081de8c1d2e3e15c608d6ffdcf2a93dc581b7dde1f1d3bb79f5cb7863"
    );
    let mut catalog = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap();
    catalog.definitions.push(compile(RAMOS_MANIFEST, ULTIMATUM));
    for source in [
        include_str!("../../../../tests/fixtures/f90fe86c-5cca-483b-93c2-6e856fa01c88.json"),
        include_str!("../../../../tests/fixtures/39451b4d-cd7a-40da-b457-cb51b609173f.json"),
    ] {
        catalog.definitions.push(compile(MANIFEST, source));
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();

    let caster = PlayerId(0);
    let mut config = ScenarioConfig::turns_permanents();
    config.profile = "opponent-nonland-destroy-private".into();
    config.initial.turn_step = TurnStep::PrecombatMain;
    config.cards.clear();
    let mut add = |name, owner, controller, zone| {
        let id = CardId(config.cards.len() as u32);
        let definition = catalog.definitions.iter().find(|d| d.name == name).unwrap();
        config.cards.push(CardSetup {
            id,
            oracle_id: definition.oracle_id.clone(),
            owner,
            controller,
            zone,
        });
        id
    };
    let spell = add(
        "Ruinous Ultimatum",
        caster,
        caster,
        Zone::Hand { owner: caster },
    );
    let mana = [
        "Mountain", "Mountain", "Plains", "Plains", "Plains", "Swamp", "Swamp",
    ]
    .map(|name| add(name, caster, caster, Zone::Battlefield));
    let dies = add(
        "Doomed Dissenter",
        PlayerId(1),
        PlayerId(1),
        Zone::Battlefield,
    );
    let borrowed = add("Commander's Sphere", caster, PlayerId(2), Zone::Battlefield);
    let immune = add("Darksteel Myr", PlayerId(3), PlayerId(3), Zone::Battlefield);
    let artifact_land = add(
        "Seat of the Synod",
        PlayerId(2),
        PlayerId(2),
        Zone::Battlefield,
    );
    let opposing_land = add("Island", PlayerId(1), PlayerId(1), Zone::Battlefield);
    let controlled = add("Grizzly Bears", PlayerId(2), caster, Zone::Battlefield);

    let mut game = Game::with_trace(
        Prepared {
            catalog_digest: digest(&catalog).unwrap(),
            profile_digest: digest(&config).unwrap(),
            catalog: Arc::new(catalog),
            config,
            match_config: None,
        },
        TraceMode::Detailed,
    );
    let [dies, borrowed, immune] = [dies, borrowed, immune].map(|id| game.object_id(id).unwrap());
    let priority = request(&mut game);
    assert!(matches!(priority, Request::Priority { player, .. } if player == caster));
    answer(&mut game, &priority, Response::Cast { card: spell });
    for id in mana {
        let payment = request(&mut game);
        assert!(matches!(payment, Request::Mana { player, .. } if player == caster));
        let object = game.object_id(id).unwrap();
        answer(&mut game, &payment, Response::ActivateMana { object });
    }
    let payment = request(&mut game);
    assert!(matches!(payment, Request::Mana { .. }));
    answer(
        &mut game,
        &payment,
        Response::FinishMana { undo_mana: false },
    );
    let spend = request(&mut game);
    let Request::SpendMana {
        cost, available, ..
    } = &spend
    else {
        panic!("expected printed cost");
    };
    let spending = BTreeMap::from([
        (Color::R, 2.into()),
        (Color::W, 3.into()),
        (Color::B, 2.into()),
    ]);
    assert_eq!(cost.generic, Quantity::zero());
    assert_eq!(
        cost.colored,
        [
            Color::R,
            Color::R,
            Color::W,
            Color::W,
            Color::W,
            Color::B,
            Color::B
        ]
    );
    assert_eq!(available, &spending);
    answer(&mut game, &spend, Response::SpendMana { spending });

    let mut priority = request(&mut game);
    let stack_id = game.top().unwrap().id;
    assert_eq!(game.top().unwrap().paid.as_ref(), Some(cost));
    for _ in 0..4 {
        assert!(matches!(priority, Request::Priority { .. }));
        answer(&mut game, &priority, Response::Pass);
        priority = request(&mut game);
    }
    let batch = game
        .trace()
        .records
        .iter()
        .find_map(|entry| match &entry.record {
            EvidenceRecord::Batch { batch }
                if matches!(batch.cause, BatchCause::Instruction { stack_id: id, .. } if id == stack_id) =>
            {
                Some(batch)
            }
            _ => None,
        })
        .expect("paid spell destruction batch");
    assert_eq!(
        batch.group,
        EventGroup::Simultaneous {
            occurrences: [dies, borrowed, immune]
                .map(|object| ProposedEvent::DestroyPermanent { object })
                .to_vec(),
        }
    );
    let [dies_result, borrow_result, immune_result] = batch.occurrences.as_slice() else {
        panic!("expected three common-prior destruction attempts");
    };
    assert!(
        matches!(&dies_result.actual, EventOutcome::Moved { movement }
        if movement.previous == dies && movement.destination == Zone::Graveyard { owner: PlayerId(1) })
    );
    assert_eq!(borrow_result.before.objects[0].controller, PlayerId(2));
    assert_eq!(borrow_result.before.objects[0].owner, caster);
    assert!(
        matches!(&borrow_result.actual, EventOutcome::Moved { movement }
        if movement.previous == borrowed && movement.destination == Zone::Graveyard { owner: caster })
    );
    assert_eq!(
        immune_result.actual,
        EventOutcome::DestructionPrevented { object: immune }
    );
    assert!(
        immune_result.before.objects[0]
            .keywords
            .contains(&KeywordAbility::Indestructible)
    );
    for id in mana
        .into_iter()
        .chain([artifact_land, opposing_land, controlled, immune.card])
    {
        let id = game.object_id(id).unwrap();
        assert_eq!(id.incarnation, 0);
        assert_eq!(game.zone(id), Some(Zone::Battlefield));
    }
    assert!(game.events().iter().any(|event| {
        matches!(event, Event::TriggerCollected {
            source, player: PlayerId(1), occurrence: TriggerOccurrenceId::Batch(id), ..
        } if *source == dies && id.batch == batch.batch_id && id.path == [0])
    }));
}
