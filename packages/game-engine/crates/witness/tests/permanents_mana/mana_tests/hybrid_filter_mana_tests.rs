use super::*;

fn filter_fixture(input: Color) -> Prepared {
    let land = if input == Color::U {
        "Island"
    } else {
        "Mountain"
    };
    let mut prepared = prepared(&[
        ("Sol Ring", 0, false),
        (land, 0, false),
        ("Yargle and Multani", 0, true),
    ]);
    let raw =
        include_str!("../../../../../tests/fixtures/f1603384-4361-49c9-98aa-7785fc3504c4.json");
    let manifest = include_str!("../../../../../tests/fixtures/archive-manifest-current.json");
    let result = mtg_compiler::compile(manifest, raw);
    let mtg_compiler::CompileResult::Lowered { definition } = result else {
        panic!("{result:?}")
    };
    prepared.config.cards[0].oracle_id = definition.oracle_id.clone();
    let catalog = Arc::make_mut(&mut prepared.catalog);
    catalog.definitions.push(*definition);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    prepared.catalog_digest = digest(catalog).unwrap();
    prepared.profile_digest = digest(&prepared.config).unwrap();
    prepared
}

fn ability(game: &mut Game, index: u32) {
    assert!(
        matches!(request(game), Request::ChooseManaAbility { ref choices, .. }
        if choices == &[ManaAbilityIdentity::Printed { index: 0 }, ManaAbilityIdentity::Printed { index: 1 }])
    );
    answer(
        game,
        Response::ChooseManaAbility {
            ability: ManaAbilityIdentity::Printed { index },
        },
    );
}

#[test]
fn hybrid_filter_mana_replays_whole_bundles_and_reverses_nested_payment() {
    for (input, blue) in [(Color::U, 0), (Color::U, 1), (Color::R, 2)] {
        let mut game = Game::with_trace(filter_fixture(input), TraceMode::Detailed);
        reach(&mut game, TurnStep::PrecombatMain);
        answer(&mut game, Response::Cast { card: CardId(2) });
        mana(&mut game, 0);
        ability(&mut game, 1);
        let decision = request(&mut game);
        assert!(
            matches!(decision, Request::ManaColor { ref colors, .. } if colors == &[Color::U, Color::R])
        );
        assert!(
            game.respond(
                decision.player(),
                decision.decision_id(),
                Response::ManaColor { color: Color::G }
            )
            .is_err()
        );
        check(&game);
        choose(&mut game, input);
        assert!(
            matches!(request(&mut game), Request::Mana { ref cost, .. } if cost.colored == [input])
        );
        let mut unfunded = game.fork();
        finish(&mut unfunded, true);
        assert!(matches!(request(&mut unfunded), Request::Mana { .. }));
        assert!(!unfunded.current(object(&unfunded, 0)).unwrap().tapped);
        mana(&mut game, 1);
        assert!(!game.current(object(&game, 0)).unwrap().tapped);
        finish(&mut game, false);
        check(&game);
        spend(&mut game, &[(input, 1)]);
        let decision = request(&mut game);
        assert!(
            matches!(decision, Request::ManaCombination { ref colors, ref total, .. }
            if colors == &[Color::U, Color::R] && *total == Quantity::from(2))
        );
        assert!(
            game.respond(
                decision.player(),
                decision.decision_id(),
                Response::ManaCombination {
                    amounts: [2_i64, 0, 0, 0, 0].map(Quantity::from)
                }
            )
            .is_err()
        );
        check(&game);
        answer(
            &mut game,
            Response::ManaCombination {
                amounts: [0_i64, blue, 0, 2 - blue, 0].map(Quantity::from),
            },
        );
        assert!(matches!(request(&mut game), Request::Mana { .. }));
        assert_eq!(
            game.mana(PlayerId(0), Color::U).unwrap_or_default(),
            blue.into()
        );
        assert_eq!(
            game.mana(PlayerId(0), Color::R).unwrap_or_default(),
            (2 - blue).into()
        );
        assert!(game.current(object(&game, 0)).unwrap().tapped);
        finish(&mut game, true);
        assert!(matches!(request(&mut game), Request::Priority { .. }));
        assert!(game.state.rules.players[0].mana.is_empty());
        for id in 0..=1 {
            assert!(!game.current(object(&game, id)).unwrap().tapped);
        }
        check(&game);
    }

    let mut game = Game::with_trace(filter_fixture(Color::U), TraceMode::Detailed);
    reach(&mut game, TurnStep::PrecombatMain);
    mana(&mut game, 0);
    ability(&mut game, 0);
    assert!(matches!(request(&mut game), Request::Priority { .. }));
    assert_eq!(game.mana(PlayerId(0), Color::C), Some(1.into()));
    check(&game);
}
