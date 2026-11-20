//! Paid exact-source witness for layer-six capture and simultaneous destruction.
use super::*;

#[test]
fn keyword_removal_wipe_paid_capture_and_recovery() {
    let manifest = include_str!("../../../../tests/fixtures/ramos/archive-manifest-current.json");
    let raw =
        include_str!("../../../../tests/fixtures/ramos/8d90ffb5-63da-48d5-a5a2-7b61062556f6.json");
    let compiled = mtg_compiler::compile(manifest, raw);
    let mtg_compiler::CompileResult::Lowered { definition } = compiled else {
        panic!("{compiled:?}");
    };
    let name = definition.name.clone();
    let cost = definition.characteristics.mana_cost.clone();
    let mut catalog = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap();
    catalog.definitions.push(*definition);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();

    let caster = PlayerId(0);
    let rival = PlayerId(1);
    let mut config = ScenarioConfig::turns_permanents();
    config.profile = "keyword-removal-wipe-private".into();
    config.initial.turn_step = TurnStep::PrecombatMain;
    config.cards.clear();
    let mut add = |name: &str, owner, zone| {
        let id = CardId(config.cards.len() as u32);
        let definition = catalog.definitions.iter().find(|d| d.name == name).unwrap();
        config.cards.push(CardSetup {
            id,
            oracle_id: definition.oracle_id.clone(),
            owner,
            controller: owner,
            zone,
        });
        id
    };
    let spell = add(&name, caster, Zone::Hand { owner: caster });
    let mut sources = vec![];
    let mut spending = BTreeMap::<Color, Quantity>::new();
    for color in &cost.colored {
        let land = match color {
            Color::W => "Plains",
            Color::U => "Island",
            Color::B => "Swamp",
            Color::R => "Mountain",
            Color::G => "Forest",
            Color::C => panic!("unexpected colorless requirement"),
        };
        sources.push(add(land, caster, Zone::Battlefield));
        spending.entry(*color).or_default().add(&1.into());
    }
    let generic: usize = cost.generic.to_string().parse().unwrap();
    for _ in 0..generic {
        sources.push(add("Island", caster, Zone::Battlefield));
        spending.entry(Color::U).or_default().add(&1.into());
    }
    let opposing = add("Darksteel Myr", rival, Zone::Battlefield);
    let own = add("Darksteel Myr", caster, Zone::Battlefield);
    let ordinary = add("Grizzly Bears", rival, Zone::Battlefield);
    let land = add("Island", rival, Zone::Battlefield);
    let prepared = Prepared {
        catalog_digest: digest(&catalog).unwrap(),
        profile_digest: digest(&config).unwrap(),
        catalog: Arc::new(catalog),
        config,
        match_config: None,
    };
    let mut game = Game::with_trace(prepared.clone(), TraceMode::Detailed);
    let answer = |game: &mut Game, response| {
        let Advance::AwaitingDecision(request) = game.advance(Budget::unlimited()) else {
            panic!("expected a decision");
        };
        game.respond(request.player(), request.decision_id(), response)
            .unwrap();
    };
    answer(&mut game, Response::Cast { card: spell });
    for id in sources {
        let object = game.object_id(id).unwrap();
        answer(&mut game, Response::ActivateMana { object });
    }
    answer(&mut game, Response::FinishMana { undo_mana: false });
    answer(&mut game, Response::SpendMana { spending });
    for _ in 0..4 {
        answer(&mut game, Response::Pass);
    }
    let Advance::AwaitingDecision(Request::Priority { .. }) = game.advance(Budget::unlimited())
    else {
        panic!("expected priority after destruction");
    };
    assert!(game.state.rules.stack.is_empty());
    for id in [opposing, ordinary] {
        assert_eq!(
            game.current(game.object_id(id).unwrap()).unwrap().zone,
            Zone::Graveyard { owner: rival }
        );
    }
    for id in [own, land] {
        assert_eq!(
            game.current(game.object_id(id).unwrap()).unwrap().zone,
            Zone::Battlefield
        );
    }
    let destroy = game
        .trace()
        .records
        .iter()
        .find_map(|record| match &record.record {
            EvidenceRecord::Batch { batch }
                if matches!(batch.group, EventGroup::Simultaneous { .. })
                    && matches!(
                        batch.cause,
                        BatchCause::Instruction {
                            source_block: 1,
                            ..
                        }
                    ) =>
            {
                Some(batch)
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(destroy.occurrences.len(), 3);
    assert!(matches!(
        destroy.occurrences[1].actual,
        EventOutcome::DestructionPrevented { .. }
    ));
    let restored = Game::restore(prepared.clone(), &game.snapshot().unwrap()).unwrap();
    assert_eq!(restored.state, game.state);
    let replay = Game::replay_to(
        prepared,
        &game.replay_journal(),
        Some(game.rule_boundary()),
        TraceMode::Detailed,
    )
    .unwrap();
    assert_eq!(replay.state, game.state);
}
