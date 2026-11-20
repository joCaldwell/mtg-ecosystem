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

#[test]
fn restricted_mana_subtypes_pay_the_matching_spell_and_replay() {
    let mut p = prepared(&[
        ("Elvish Mystic", 0, false),
        ("Grizzly Bears", 0, true),
        ("Sol Ring", 0, false),
        ("Sol Ring", 0, true),
        ("Golgari Signet", 0, false),
    ]);
    let result = mtg_compiler::compile(
        include_str!("../../../../tests/fixtures/archive-manifest-current.json"),
        include_str!("../../../../tests/fixtures/392f6cd3-3895-42b3-8bb9-6b0c6a689a29.json"),
    );
    let mtg_compiler::CompileResult::Lowered { definition } = result else {
        panic!("{result:?}");
    };
    let ManaProduction::RestrictedFixed {
        spend: restriction, ..
    } = definition.mana_abilities[0].production
    else {
        panic!("expected restricted mana");
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

    let mut game = Game::with_trace(p, TraceMode::Detailed);
    reach(&mut game, TurnStep::PrecombatMain);
    mana(&mut game, 0);
    answer(&mut game, Response::Cast { card: CardId(3) });
    mana(&mut game, 2);
    finish(&mut game, false);
    assert!(matches!(
        request(&mut game),
        Request::SpendMana { restricted_available, .. } if restricted_available.is_empty()
    ));
    spend(&mut game, &[(Color::C, 1)]);
    pass(&mut game, 4);
    answer(&mut game, Response::Cast { card: CardId(1) });
    mana(&mut game, 3);
    finish(&mut game, false);
    assert!(matches!(
        request(&mut game),
        Request::SpendMana { ref restricted_available, .. }
            if *restricted_available == [RestrictedMana { amount: 2.into(), ..unit(restriction, Color::R) }]
    ));
    check(&game);
    answer(
        &mut game,
        Response::SpendManaAllocated {
            spending: [(Color::C, 2.into())].into(),
            restricted: vec![unit(restriction, Color::R)],
        },
    );
    pass(&mut game, 4);
    let keep = object(&game, 1);
    answer(&mut game, Response::LegendRule { keep });
    let _ = request(&mut game);
    assert_eq!(
        game.state.rules.players[0].restricted_mana,
        [unit(restriction, Color::R)]
    );
    assert!(game.characteristics(object(&game, 1)).unwrap().legendary);
    mana(&mut game, 1);
    let _ = request(&mut game);
    assert_eq!(
        game.state.rules.players[0].restricted_mana,
        [RestrictedMana {
            amount: 3.into(),
            ..unit(restriction, Color::R)
        }]
    );
    check(&game);
}

#[test]
fn paid_restricted_land_selects_a_branch_and_unwinds_nested_payment() {
    let mut p = prepared(&[
        ("Forest", 0, true),
        ("Forest", 0, false),
        ("Forest", 0, false),
        ("Forest", 0, false),
        ("Forest", 0, false),
        ("Grizzly Bears", 0, true),
    ]);
    let result = mtg_compiler::compile(
        include_str!("../../../../tests/fixtures/archive-manifest-current.json"),
        include_str!("../../../../tests/fixtures/de75e5dd-8a52-406c-b55c-96d686885500.json"),
    );
    let mtg_compiler::CompileResult::Lowered { definition } = result else {
        panic!("{result:?}");
    };
    p.config.cards[0]
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
    answer(&mut game, Response::PlayLand { card: CardId(0) });
    let _ = request(&mut game);
    let source = object(&game, 0);
    assert!(!game.current(source).unwrap().tapped);

    // First reverse the entire planned activation while the outer spell
    // remains unpayable. Then choose the same paid branch and cast normally.
    for reverse in [true, false] {
        answer(&mut game, Response::Cast { card: CardId(5) });
        mana(&mut game, 0);
        let decision = request(&mut game);
        assert!(
            matches!(&decision, Request::ChooseManaAbility { choices, .. }
            if *choices == [ManaAbilityIdentity::Printed { index: 0 }, ManaAbilityIdentity::Printed { index: 1 }])
        );
        assert!(
            game.respond(
                decision.player(),
                decision.decision_id(),
                Response::ManaColor { color: Color::G }
            )
            .is_err()
        );
        if reverse {
            answer(
                &mut game,
                Response::ChooseManaAbility {
                    ability: ManaAbilityIdentity::Printed { index: 1 },
                },
            );
            mana(&mut game, 1);
            check(&game);
            finish(&mut game, true);
            let _ = request(&mut game);
            assert!(!game.current(object(&game, 1)).unwrap().tapped);
            assert!(!game.current(source).unwrap().tapped);
            finish(&mut game, true);
            let _ = request(&mut game);
            assert!(game.state.rules.players[0].restricted_mana.is_empty());
            check(&game);
            continue;
        }
        answer(
            &mut game,
            Response::ChooseManaAbility {
                ability: ManaAbilityIdentity::Printed { index: 1 },
            },
        );
        for id in 1..5 {
            mana(&mut game, id);
        }
        finish(&mut game, false);
        check(&game);
        spend(&mut game, &[(Color::G, 4)]);
        finish(&mut game, false);
        let spend = ManaSpendRestriction::CreatureSpellOrBattlefieldCreatureAbility;
        assert!(
            matches!(request(&mut game), Request::SpendMana { ref restricted_available, .. }
            if *restricted_available == [RestrictedMana { amount: 6.into(), ..unit(spend, Color::G) }])
        );
        answer(
            &mut game,
            Response::SpendManaAllocated {
                spending: BTreeMap::new(),
                restricted: vec![RestrictedMana {
                    amount: 2.into(),
                    ..unit(spend, Color::G)
                }],
            },
        );
        pass(&mut game, 4);
        assert_eq!(
            game.state.rules.players[0].restricted_mana,
            [RestrictedMana {
                amount: 4.into(),
                ..unit(spend, Color::G)
            }]
        );
        assert_eq!(game.zone(object(&game, 5)), Some(Zone::Battlefield));
        check(&game);
    }
}
