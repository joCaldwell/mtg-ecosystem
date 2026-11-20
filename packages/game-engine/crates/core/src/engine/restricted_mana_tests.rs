use super::*;

const ACTIVATION: ManaSpendRestriction = ManaSpendRestriction::ActivatedAbility;
const COLORLESS: ManaSpendRestriction = ManaSpendRestriction::ColorlessSpellOrActivatedAbility;

fn unit(restriction: ManaSpendRestriction, color: Color) -> RestrictedMana {
    RestrictedMana {
        restriction,
        color,
        amount: 1.into(),
    }
}

fn fixture() -> Prepared {
    let mut p = prepared(&[
        ("Elvish Mystic", 0, false),
        ("Grizzly Bears", 0, false),
        ("Sol Ring", 0, true),
        ("Golgari Signet", 0, false),
        ("Yargle and Multani", 0, true),
    ]);
    let manifest = include_str!("../../../../tests/fixtures/archive-manifest-current.json");
    let catalog = Arc::make_mut(&mut p.catalog);
    for (index, raw) in [
        include_str!("../../../../tests/fixtures/215fd0b4-5817-4335-a0ed-1694e44bde41.json"),
        include_str!("../../../../tests/fixtures/359a570e-92a7-4954-9a05-89cc75df57e7.json"),
    ]
    .into_iter()
    .enumerate()
    {
        let result = mtg_compiler::compile(manifest, raw);
        let mtg_compiler::CompileResult::Lowered { definition } = result else {
            panic!("{result:?}")
        };
        p.config.cards[index]
            .oracle_id
            .clone_from(&definition.oracle_id);
        catalog.definitions.push(*definition);
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    p.catalog_digest = digest(catalog).unwrap();
    p.profile_digest = digest(&p.config).unwrap();
    p
}

fn allocation(unit: RestrictedMana) -> Response {
    Response::SpendManaAllocated {
        spending: BTreeMap::new(),
        restricted: vec![unit],
    }
}

#[test]
fn restricted_mana_selects_same_color_eligible_bucket_and_keeps_the_other() {
    let mut game = Game::with_trace(fixture(), TraceMode::Detailed);
    reach(&mut game, TurnStep::PrecombatMain);
    mana(&mut game, 0);
    mana(&mut game, 1);
    let _ = request(&mut game);
    assert!(game.state.rules.players[0].mana.is_empty());
    answer(&mut game, Response::Cast { card: CardId(2) });
    finish(&mut game, false);
    let decision = request(&mut game);
    assert!(
        matches!(&decision, Request::SpendMana { available, restricted_available, .. }
        if available.is_empty() && *restricted_available == [unit(COLORLESS, Color::C)])
    );
    let before = game.state_digest().unwrap();
    assert!(
        game.respond(
            decision.player(),
            decision.decision_id(),
            allocation(unit(ACTIVATION, Color::C))
        )
        .is_err()
    );
    assert_eq!(game.state_digest().unwrap(), before);
    answer(&mut game, allocation(unit(COLORLESS, Color::C)));
    let _ = request(&mut game);
    assert_eq!(
        game.state.rules.players[0].restricted_mana,
        [unit(ACTIVATION, Color::U), unit(ACTIVATION, Color::C)]
    );
    check(&game);
}

#[test]
fn restricted_mana_nested_spending_reverses_the_same_bucket() {
    let mut game = Game::with_trace(fixture(), TraceMode::Detailed);
    reach(&mut game, TurnStep::PrecombatMain);
    answer(&mut game, Response::Cast { card: CardId(4) });
    mana(&mut game, 3);
    mana(&mut game, 0);
    mana(&mut game, 1);
    finish(&mut game, false);
    answer(&mut game, allocation(unit(ACTIVATION, Color::C)));
    let _ = request(&mut game);
    assert_eq!(
        game.state.rules.players[0].restricted_mana,
        [unit(ACTIVATION, Color::U), unit(COLORLESS, Color::C)]
    );
    check(&game);
    finish(&mut game, true);
    assert!(matches!(request(&mut game), Request::Priority { .. }));
    assert!(game.state.rules.players[0].mana.is_empty());
    assert!(game.state.rules.players[0].restricted_mana.is_empty());
    for id in [0, 1, 3] {
        assert!(!game.current(object(&game, id)).unwrap().tapped);
    }
    check(&game);
}
