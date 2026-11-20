//! Exact source witnesses for fixed and counted output at normal action boundaries.
use super::*;

const LOTUS: &str =
    include_str!("../../../../tests/fixtures/9a02a9a7-39d9-4763-85d3-747a0540b60b.json");
const WEAVER: &str =
    include_str!("../../../../tests/fixtures/acfe7ec0-0606-4d5e-b1fa-25f0c7aeec47.json");

fn mana_catalog() -> Catalog {
    let mut catalog = land_catalog();
    catalog.definitions.extend([lower(LOTUS), lower(WEAVER)]);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    catalog
}

#[test]
fn same_color_mana_paid_and_reversed_without_splitting() {
    for (name, quantity) in [("Gilded Lotus", 3), ("Sanctum Weaver", 2)] {
        let mut rows = vec![
            (name, 0, 0, false),
            ("Colossal Dreadmaw", 0, 0, true),
            ("Rampant Growth", 0, 0, true),
        ];
        if name == "Sanctum Weaver" {
            // One borrowed enchantment counts, another seat's does not.
            rows.extend([
                ("Sanctum Weaver", 1, 0, false),
                ("Sanctum Weaver", 2, 2, false),
            ]);
        }
        let mut game = Game::with_trace(
            midgame_with_catalog(&rows, mana_catalog()),
            TraceMode::Detailed,
        );
        let source = named(&game, name);
        let dreadmaw = named(&game, "Colossal Dreadmaw").card;
        answer(&mut game, Response::Cast { card: dreadmaw });
        let _ = request(&mut game);
        activate_public(&mut game, source);
        choose(&mut game, Color::G);
        assert_eq!(game.mana(PlayerId(0), Color::G), Some(quantity.into()));
        answer(&mut game, Response::FinishMana { undo_mana: true });
        let _ = request(&mut game);
        assert_eq!(game.mana(PlayerId(0), Color::G), Some(0.into()));
        assert!(!game.current(source).unwrap().tapped);
        check(&game);

        let ramp = named(&game, "Rampant Growth").card;
        answer(&mut game, Response::Cast { card: ramp });
        let _ = request(&mut game);
        activate_public(&mut game, source);
        choose(&mut game, Color::G);
        answer(&mut game, Response::FinishMana { undo_mana: false });
        answer(
            &mut game,
            Response::SpendMana {
                spending: BTreeMap::from([(Color::G, 2.into())]),
            },
        );
        let _ = request(&mut game);
        let paid = game.state.rules.stack.last().unwrap();
        assert_eq!(paid.paid.as_ref(), Some(&paid.total_cost));
        assert_eq!(
            game.mana(PlayerId(0), Color::G),
            Some((quantity - 2).into())
        );
        assert!(game.current(source).unwrap().tapped);
        check(&game);
    }
}

#[test]
fn same_color_mana_dork_cannot_tap_after_paid_entry() {
    let mut game = Game::with_trace(
        midgame_with_catalog(
            &[
                ("Sanctum Weaver", 0, 0, true),
                ("Forest", 0, 0, false),
                ("Sol Ring", 0, 0, false),
            ],
            mana_catalog(),
        ),
        TraceMode::Detailed,
    );
    let card = named(&game, "Sanctum Weaver").card;
    answer(&mut game, Response::Cast { card });
    let _ = request(&mut game);
    let source = named(&game, "Forest");
    activate_public(&mut game, source);
    let ring = named(&game, "Sol Ring");
    activate_public(&mut game, ring);
    answer(&mut game, Response::FinishMana { undo_mana: false });
    answer(
        &mut game,
        Response::SpendMana {
            spending: BTreeMap::from([(Color::G, 1.into()), (Color::C, 1.into())]),
        },
    );
    resolve(&mut game, vec![]);
    let entered = named(&game, "Sanctum Weaver");
    assert_eq!(game.zone(entered), Some(Zone::Battlefield));
    assert!(!game.current(entered).unwrap().tapped);
    assert!(game.validate_mana_activation(PlayerId(0), entered).is_err());
    check(&game);
}
