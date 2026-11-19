use super::*;

const COAST: &str =
    include_str!("../../../../tests/fixtures/ramos/40b36bc6-c185-4bda-99e7-0118953c2c97.json");

fn shield(game: &Game) -> Quantity {
    let [effect] = game.state.rules.effects.as_slice() else {
        panic!("expected the paid shield");
    };
    let ResolvedEffectKind::PreventNextDamage { remaining, .. } = &effect.kind else {
        panic!("expected the paid next-damage shield");
    };
    remaining.clone()
}

#[test]
fn pain_mana_paid_choice_nested_undo_and_recovery() {
    let mut catalog = land_catalog();
    catalog.definitions.push(lower(COAST));
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    let prepared = midgame_with_catalog(
        &[
            ("Samite Healer", 0, 0, false),
            ("Golgari Signet", 0, 0, false),
            ("Yavimaya Coast", 0, 0, false),
            ("Colossal Dreadmaw", 0, 0, true),
            ("Furnace of Rath", 3, 3, false),
        ],
        catalog,
    );
    let mut game = Game::with_trace(prepared, TraceMode::Detailed);
    let player = PlayerId(0);
    let _ = request(&mut game);
    let healer = game
        .handle_for(player, named(&game, "Samite Healer"))
        .unwrap();
    public(
        &mut game,
        PlayerResponse::ActivateAbility {
            object: healer,
            ability: 0,
        },
    );
    let _ = request(&mut game);
    public(
        &mut game,
        PlayerResponse::EntityTargets {
            entities: vec![PlayerTargetEntity::Player { player }],
        },
    );
    for _ in 0..4 {
        answer(&mut game, Response::Pass);
    }
    assert!(matches!(request(&mut game), Request::Priority { .. }));
    assert_eq!(shield(&game), 1.into());
    let baseline_pool = game.state.rules.players[0].mana.clone();

    let spell = named(&game, "Colossal Dreadmaw");
    let card = game.handle_for(player, spell).unwrap();
    public(&mut game, PlayerResponse::Cast { card });
    assert!(matches!(request(&mut game), Request::Mana { .. }));
    let signet = named(&game, "Golgari Signet");
    let source = named(&game, "Yavimaya Coast");
    for object in [signet, source] {
        let object = game.handle_for(player, object).unwrap();
        public(&mut game, PlayerResponse::ActivateMana { object });
        let _ = request(&mut game);
    }
    assert!(!game.current(source).unwrap().tapped);
    assert_eq!(game.state.rules.players[0].mana, baseline_pool);
    let PlayerRequest::ManaColor {
        colors,
        mana_ability_source,
        ..
    } = observation(&game, 0).decision.unwrap()
    else {
        panic!("expected an unpaid source choice");
    };
    assert_eq!(
        colors.into_iter().collect::<BTreeSet<_>>(),
        BTreeSet::from([Color::C, Color::G, Color::U])
    );
    assert_eq!(
        mana_ability_source,
        Some(game.handle_for(player, source).unwrap())
    );
    assert!(observation(&game, 1).decision.is_none());
    let mut safe = game.fork();
    public(&mut safe, PlayerResponse::ManaColor { color: Color::C });
    assert!(matches!(request(&mut safe), Request::Mana { .. }));
    assert!(safe.current(source).unwrap().tapped);
    assert_eq!(safe.mana(player, Color::C), Some(1.into()));
    assert_eq!(safe.state.rules.players[0].life, 40.into());
    assert_eq!(shield(&safe), 1.into());

    public(&mut game, PlayerResponse::ManaColor { color: Color::U });
    let r = request(&mut game);
    assert!(matches!(r, Request::DamageEffect { .. }));
    assert!(game.current(source).unwrap().tapped);
    assert_eq!(game.mana(player, Color::U), Some(1.into()));
    assert!(!game.current(signet).unwrap().tapped);
    assert_eq!(game.state.rules.stack.len(), 1);
    assert!(game.top().unwrap().paid.is_none());
    check(&game);
    let PlayerRequest::DamageEffect { candidates, .. } =
        observation(&game, 0).decision.unwrap()
    else {
        panic!("expected a real damage-effect order choice");
    };
    let doubling = candidates
        .into_iter()
        .find(|candidate| candidate.kind == DamageEffectKind::DoubleDamage)
        .unwrap();
    public(
        &mut game,
        PlayerResponse::DamageEffect {
            effect: doubling.effect,
            damage: doubling.damage,
        },
    );
    assert!(matches!(request(&mut game), Request::Mana { .. }));
    assert_eq!(game.state.rules.players[0].life, 39.into());
    assert_eq!(shield(&game), 0.into());
    public(&mut game, PlayerResponse::FinishMana { undo_mana: false });
    assert!(matches!(request(&mut game), Request::SpendMana { .. }));
    public(
        &mut game,
        PlayerResponse::SpendMana {
            spending: BTreeMap::from([(Color::U, 1.into())]),
        },
    );
    assert!(matches!(request(&mut game), Request::Mana { .. }));
    assert!(game.current(signet).unwrap().tapped);
    assert_eq!(game.mana(player, Color::G), Some(1.into()));
    assert_eq!(game.mana(player, Color::B), Some(1.into()));

    let mut keep = game.fork();
    public(&mut keep, PlayerResponse::FinishMana { undo_mana: false });
    assert!(matches!(request(&mut keep), Request::Priority { .. }));
    assert_eq!(keep.state.rules.players[0].life, 39.into());
    assert_eq!(shield(&keep), 0.into());
    assert!(keep.current(source).unwrap().tapped);

    let restored = Game::restore(
        game.prepared.as_ref().clone(),
        &game.snapshot().unwrap(),
    )
    .unwrap();
    assert_eq!(restored.state, game.state);
    game = restored;
    public(&mut game, PlayerResponse::FinishMana { undo_mana: true });
    assert!(matches!(request(&mut game), Request::Priority { .. }));
    assert_eq!(game.state.rules.players[0].life, 40.into());
    assert_eq!(game.state.rules.players[0].mana, baseline_pool);
    assert_eq!(shield(&game), 1.into());
    assert!(!game.current(source).unwrap().tapped);
    assert!(!game.current(signet).unwrap().tapped);
    assert_eq!(
        game.zone(named(&game, "Colossal Dreadmaw")),
        Some(Zone::Hand { owner: player })
    );
}
