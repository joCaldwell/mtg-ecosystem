use super::*;

const CASTER: PlayerId = PlayerId(0);
const OWNER: PlayerId = PlayerId(1);
const VEGA: CardId = CardId(4);
const COMMANDER: CardId = CardId(3);
const DRAWN: CardId = CardId(6);
const OTHER_LIBRARY: CardId = CardId(7);

fn prepared() -> Prepared {
    let result = mtg_compiler::compile(
        include_str!("../../../../tests/fixtures/ramos/archive-manifest-current.json"),
        include_str!("../../../../tests/fixtures/ramos/95e66851-7aff-4559-91b4-6b8a95c6e1f8.json"),
    );
    let mtg_compiler::CompileResult::Lowered { definition } = result else {
        panic!("{result:?}");
    };
    let c = &definition.characteristics;
    assert_eq!(definition.keywords, [KeywordAbility::Flying]);
    assert_eq!(c.kind, CardKind::Creature);
    assert!(c.legendary);
    assert_eq!(c.subtypes, ["Bird", "Spirit"]);
    assert_eq!(c.mana_cost.generic, 1.into());
    assert_eq!(c.mana_cost.colored, [Color::W, Color::U]);
    assert_eq!(c.colors, [Color::U, Color::W]);
    assert_eq!(c.color_identity, [Color::U, Color::W]);
    assert_eq!(c.power, Some(2.into()));
    assert_eq!(c.toughness, Some(2.into()));
    assert!(
        definition.capabilities.contains(&Capability::CastOriginTriggerV1)
    );
    let mut catalog = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-removal-ir.json"
    ))
    .unwrap();
    let vega = definition.oracle_id.clone();
    catalog.definitions.push(*definition);
    catalog.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();

    let mut config = ScenarioConfig::commander_foundation();
    config.profile = "cast-origin-draw-private-witness".into();
    config.initial.turn_step = TurnStep::PrecombatMain;
    config.cards.extend([
        CardSetup {
            id: VEGA,
            oracle_id: vega,
            owner: OWNER,
            controller: CASTER,
            zone: Zone::Battlefield,
        },
        CardSetup {
            id: CardId(5),
            oracle_id: PLAINS.into(),
            owner: CASTER,
            controller: CASTER,
            zone: Zone::Battlefield,
        },
    ]);
    for (id, owner) in [(DRAWN, CASTER), (OTHER_LIBRARY, OWNER)] {
        config.cards.push(CardSetup {
            id,
            oracle_id: FOREST.into(),
            owner,
            controller: owner,
            zone: Zone::Library { owner },
        });
    }
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

fn pay(game: &mut Game, land: CardId, color: Color) {
    let object = game.object_id(land).unwrap();
    answer(game, Response::ActivateMana { object });
    answer(game, Response::FinishMana { undo_mana: false });
    answer(game, Response::SpendMana {
        spending: BTreeMap::from([(color, 1.into())]),
    });
    assert!(matches!(
        request(game),
        Request::Priority { player: CASTER, .. }
    ));
}

fn resolve_top(game: &mut Game) {
    for _ in 0..4 {
        assert!(matches!(request(game), Request::Priority { .. }));
        answer(game, Response::Pass);
    }
    assert!(matches!(request(game), Request::Priority { .. }));
}

#[test]
fn cast_origin_paid_commander_draw_and_hand_control_replay() {
    let prepared = prepared();
    let mut game = Game::with_trace(prepared.clone(), TraceMode::Detailed);
    let source = game.object_id(VEGA).unwrap();
    answer(&mut game, Response::Cast { card: COMMANDER });
    pay(&mut game, CardId(5), Color::W);
    let stack = &game.state.rules.stack;
    assert_eq!(stack.len(), 2);
    assert!(stack[0].paid.is_some());
    let trigger = stack[1].id;
    let facts = stack[1].source_facts.as_ref().unwrap();
    assert_eq!(
        (facts.object, facts.owner, facts.controller),
        (source, OWNER, CASTER)
    );
    assert_eq!(stack[1].controller, CASTER);
    assert!(matches!(
        &stack[1].kind,
        StackKind::Triggered { witness: TriggerEventWitness::SpellCast { cast }, .. }
            if cast.cast_from == (Zone::Command { owner: CASTER }) && cast.stack_id == stack[0].id
    ));

    // The same controller casts Unsummon from hand while Vega is still on the
    // battlefield. That cast adds only its own spell, then removes Vega.
    answer(&mut game, Response::Cast { card: CardId(1) });
    answer(&mut game, Response::Targets { objects: vec![source] });
    pay(&mut game, CardId(0), Color::U);
    assert_eq!(game.state.rules.stack.len(), 3);
    assert_eq!(game.top().unwrap().cast_from, Zone::Hand { owner: CASTER });
    assert!(game.top().unwrap().paid.is_some());
    resolve_top(&mut game);
    assert_eq!(game.zone(source), None);
    assert_eq!(
        game.zone(game.object_id(VEGA).unwrap()),
        Some(Zone::Hand { owner: OWNER })
    );
    assert_eq!(game.top().unwrap().id, trigger);
    assert_eq!(game.state.rules.libraries[&CASTER], [DRAWN]);

    let mut restored = Game::restore(prepared.clone(), &game.snapshot().unwrap()).unwrap();
    for game in [&mut game, &mut restored] {
        resolve_top(game);
        assert_eq!(game.state.rules.stack.len(), 1);
        assert_eq!(game.top().unwrap().source.card, COMMANDER);
        assert!(game.state.rules.libraries[&CASTER].is_empty());
        assert_eq!(game.state.rules.libraries[&OWNER], [OTHER_LIBRARY]);
        assert_eq!(
            game.zone(game.object_id(DRAWN).unwrap()),
            Some(Zone::Hand { owner: CASTER })
        );
    }
    assert_eq!(restored.state, game.state);
    let replayed = Game::replay(prepared, &game.replay_journal()).unwrap();
    assert_eq!(replayed.state, game.state);
    assert_eq!(replayed.trace(), game.trace());
}
