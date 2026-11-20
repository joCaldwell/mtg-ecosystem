use super::*;

fn fixture() -> Prepared {
    let mut p = prepared(&[
        ("Sol Ring", 0, false),
        ("Sol Ring", 0, false),
        ("Yargle and Multani", 0, true),
        ("Sol Ring", 0, true),
    ]);
    let raw = include_str!("../../../../tests/fixtures/1b3d7fce-e9fe-4176-9d9a-472415826cdd.json");
    let manifest = include_str!("../../../../tests/fixtures/archive-manifest-current.json");
    let result = mtg_compiler::compile(manifest, raw);
    let mtg_compiler::CompileResult::Lowered { definition } = result else {
        panic!("{result:?}")
    };
    for card in &mut p.config.cards[..2] {
        card.oracle_id.clone_from(&definition.oracle_id);
    }
    let catalog = Arc::make_mut(&mut p.catalog);
    catalog.definitions.push(*definition);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    p.catalog_digest = digest(catalog).unwrap();
    p.profile_digest = digest(&p.config).unwrap();
    p
}

#[test]
fn mana_life_gain_reverses_only_actual_gains_and_recovers_nested_receipts() {
    let mut game = Game::with_trace(fixture(), TraceMode::Detailed);
    reach(&mut game, TurnStep::PrecombatMain);
    let life = game.state.rules.players[0].life.clone();
    answer(&mut game, Response::Cast { card: CardId(2) });
    mana(&mut game, 0);
    mana(&mut game, 1);
    let _ = request(&mut game);
    assert_eq!(game.mana(PlayerId(0), Color::C), Some(2.into()));
    let mut gained = life.clone();
    gained.add(&2.into());
    assert_eq!(game.state.rules.players[0].life, gained);
    check(&game);

    let mut keep = game.fork();
    finish(&mut keep, false);
    assert!(matches!(request(&mut keep), Request::Priority { .. }));
    assert_eq!(keep.state.rules.players[0].life, gained);
    assert_eq!(keep.mana(PlayerId(0), Color::C), Some(2.into()));

    finish(&mut game, true);
    assert!(matches!(request(&mut game), Request::Priority { .. }));
    assert_eq!(game.state.rules.players[0].life, life);
    assert!(game.state.rules.players[0].mana.is_empty());
    assert!(!game.current(object(&game, 0)).unwrap().tapped);
    assert!(!game.current(object(&game, 1)).unwrap().tapped);

    answer(&mut game, Response::Cast { card: CardId(3) });
    mana(&mut game, 0);
    finish(&mut game, false);
    spend(&mut game, &[(Color::C, 1)]);
    assert!(matches!(request(&mut game), Request::Priority { .. }));
    let mut paid = life.clone();
    paid.add(&1.into());
    assert_eq!(game.state.rules.players[0].life, paid);
    check(&game);
    check(&keep);
}

#[test]
fn mana_life_gain_prohibition_leaves_mana_and_creates_no_gain_to_reverse() {
    let mut game = Game::new(fixture());
    reach(&mut game, TurnStep::PrecombatMain);
    game.state.rules.players[0].life_frozen_until_next_turn = true;
    let life = game.state.rules.players[0].life.clone();
    answer(&mut game, Response::Cast { card: CardId(2) });
    mana(&mut game, 0);
    let _ = request(&mut game);
    assert_eq!(game.state.rules.players[0].life, life);
    assert_eq!(game.mana(PlayerId(0), Color::C), Some(1.into()));
    finish(&mut game, true);
    assert!(matches!(request(&mut game), Request::Priority { .. }));
    assert_eq!(game.state.rules.players[0].life, life);
    assert!(game.state.rules.players[0].mana.is_empty());
}

#[test]
fn mana_life_gain_can_fund_phyrexian_life_after_announcement() {
    let mut p = fixture();
    p.config.initial.life[0] = 1.into();
    let manifest = mtg_compiler::fixtures::MANIFEST;
    let raw = include_str!("../../../../tests/fixtures/phyrexian-keywords-vault-skirge.json");
    let result = mtg_compiler::compile(manifest, raw);
    let mtg_compiler::CompileResult::Lowered { definition } = result else {
        panic!("{result:?}")
    };
    p.config.cards[3]
        .oracle_id
        .clone_from(&definition.oracle_id);
    let catalog = Arc::make_mut(&mut p.catalog);
    catalog.definitions.push(*definition);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    p.catalog_digest = digest(catalog).unwrap();
    p.profile_digest = digest(&p.config).unwrap();

    let mut game = Game::with_trace(p, TraceMode::Detailed);
    reach(&mut game, TurnStep::PrecombatMain);
    answer(&mut game, Response::Cast { card: CardId(3) });
    answer(
        &mut game,
        Response::HybridCost {
            colors: vec![HybridPayment::PayTwoLife],
        },
    );
    mana(&mut game, 0);
    mana(&mut game, 1);
    finish(&mut game, false);
    spend(&mut game, &[(Color::C, 1)]);
    assert!(matches!(request(&mut game), Request::Priority { .. }));
    assert_eq!(game.state.rules.players[0].life, 1.into());
    assert_eq!(game.mana(PlayerId(0), Color::C), Some(1.into()));
    check(&game);
}
