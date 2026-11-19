//! Frozen source through a paid sacrifice and an actual simultaneous batch.
//! CR 205.2b, 601.2h, 603.3a, 603.10a, 700.4, 701.8, 701.19c and 702.12b.
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

fn answer(game: &mut Game, response: Response) {
    let request = request(game);
    game.respond(request.player(), request.decision_id(), response)
        .unwrap();
}

#[test]
fn source_dies_nonland_destroy_paid_sacrifice() {
    const RAMOS_MANIFEST: &str =
        include_str!("../../../../tests/fixtures/ramos/archive-manifest-current.json");
    const CHILD: &str =
        include_str!("../../../../tests/fixtures/ramos/70dbe8a9-505d-41c2-9b5b-a991d13ab459.json");
    const MANIFEST: &str = include_str!("../../../../tests/fixtures/archive-manifest-current.json");
    assert_eq!(
        sha256(CHILD.as_bytes()),
        "8c464a347a26ce2d48642b9650a516aa88608709e70baddd2536cdeadd470362"
    );
    let child = compile(RAMOS_MANIFEST, CHILD);
    assert!(matches!(
        child.triggered.as_slice(),
        [TriggeredAbility { condition: TriggerCondition::SelfDies, program, .. }]
            if matches!(program.instructions.as_slice(), [Instruction::DestroySet {
                set: ObjectSet::NonlandPermanents,
                regeneration_forbidden: true,
                source_block: 1,
            }])
    ));
    let mut catalog = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap();
    catalog.definitions.push(child);
    for source in [
        include_str!("../../../../tests/fixtures/f90fe86c-5cca-483b-93c2-6e856fa01c88.json"),
        include_str!("../../../../tests/fixtures/39451b4d-cd7a-40da-b457-cb51b609173f.json"),
    ] {
        catalog.definitions.push(compile(MANIFEST, source));
    }
    catalog.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();

    let caster = PlayerId(0);
    let owner = PlayerId(1);
    let opponent = PlayerId(2);
    let mut config = ScenarioConfig::turns_permanents();
    config.profile = "source-dies-nonland-destroy-private".into();
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
    let rites = add("Village Rites", caster, caster, Zone::Hand { owner: caster });
    let swamp = add("Swamp", caster, caster, Zone::Battlefield);
    let child = add("Child of Alara", owner, caster, Zone::Battlefield);
    let own_creature = add("Grizzly Bears", caster, caster, Zone::Battlefield);
    let opposing_artifact = add("Commander's Sphere", opponent, opponent, Zone::Battlefield);
    let immune = add("Darksteel Myr", opponent, opponent, Zone::Battlefield);
    let artifact_land = add("Seat of the Synod", owner, owner, Zone::Battlefield);
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
    let [swamp, child, own_creature, opposing_artifact, immune, artifact_land] =
        [swamp, child, own_creature, opposing_artifact, immune, artifact_land]
            .map(|id| game.object_id(id).unwrap());

    assert!(matches!(request(&mut game), Request::Priority { player, .. } if player == caster));
    answer(&mut game, Response::Cast { card: rites });
    answer(&mut game, Response::ActivateMana { object: swamp });
    answer(&mut game, Response::FinishMana { undo_mana: false });
    answer(
        &mut game,
        Response::SpendMana { spending: BTreeMap::from([(Color::B, 1.into())]) },
    );
    assert!(matches!(request(&mut game), Request::SacrificeCost { candidates, .. }
        if candidates.contains(&child)));
    answer(&mut game, Response::SacrificeCost { object: child });
    assert!(matches!(request(&mut game), Request::Priority { .. }));
    let trigger = game.top().unwrap();
    let trigger_id = trigger.id;
    assert_eq!(trigger.source, child);
    assert_eq!(trigger.controller, caster);
    let facts = trigger.source_facts.as_ref().unwrap();
    assert_eq!(facts.owner, owner);
    assert_eq!(facts.controller, caster);
    assert_eq!(facts.zone, Zone::Battlefield);
    assert!(facts.keywords.contains(&KeywordAbility::Trample));
    let successor = game.object_id(child.card).unwrap();
    assert_eq!(game.zone(successor), Some(Zone::Graveyard { owner }));
    let StackKind::Triggered {
        witness: TriggerEventWitness::Moved { movement },
        ..
    } = &trigger.kind
    else {
        panic!("expected the actual death witness on the stack");
    };
    assert_eq!(
        *movement,
        MoveOutcome {
            previous: child,
            successor,
            from: Zone::Battlefield,
            destination: Zone::Graveyard { owner },
        }
    );

    for _ in 0..4 {
        assert!(matches!(request(&mut game), Request::Priority { .. }));
        answer(&mut game, Response::Pass);
    }
    assert!(matches!(request(&mut game), Request::Priority { .. }));
    let batch = game.trace().records.iter().find_map(|entry| match &entry.record {
        EvidenceRecord::Batch { batch } if matches!(batch.cause,
            BatchCause::Instruction { stack_id, source, player, source_block: 1, .. }
                if stack_id == trigger_id && source == child && player == caster) =>
        {
            Some(batch)
        }
        _ => None,
    }).expect("source death trigger's destruction batch");
    assert_eq!(
        batch.group,
        EventGroup::Simultaneous {
            occurrences: [own_creature, opposing_artifact, immune]
                .map(|object| ProposedEvent::DestroyPermanent { object })
                .to_vec(),
        }
    );
    assert!(matches!(batch.result(&[0]), Some(EventOutcome::Moved { movement })
        if movement.previous == own_creature && movement.destination == Zone::Graveyard { owner: caster }));
    assert!(matches!(batch.result(&[1]), Some(EventOutcome::Moved { movement })
        if movement.previous == opposing_artifact && movement.destination == Zone::Graveyard { owner: opponent }));
    assert_eq!(
        batch.result(&[2]),
        Some(&EventOutcome::DestructionPrevented { object: immune })
    );
    for object in [swamp, artifact_land, immune] {
        assert_eq!(game.zone(object), Some(Zone::Battlefield));
    }
}
