//! Paid and simultaneous entry witnesses from the same exact archived source.
use super::*;

fn checkland_catalog() -> Catalog {
    let mut catalog = land_catalog();
    catalog.definitions.push(lower(include_str!(
        "../../../../../../tests/fixtures/6a6c5e17-6465-4a1f-9d63-8a3ce2edc522.json"
    )));
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    catalog
}

#[test]
fn checkland_entry_uses_common_prior_types_and_current_control() {
    // Mountain enters in the same group and Prairie Stream is an opponent's
    // permanent. Neither qualifies at this boundary.
    let mut game = Game::with_trace(
        midgame_with_catalog(
            &[
                ("Sulfur Falls", 0, 0, true),
                ("Mountain", 0, 0, true),
                ("Prairie Stream", 0, 1, false),
            ],
            checkland_catalog(),
        ),
        TraceMode::Detailed,
    );
    let mountain = named(&game, "Mountain");
    let checkland = named(&game, "Sulfur Falls");
    let batch = game
        .commit_batch(
            BatchCause::Fixture,
            EventGroup::Simultaneous {
                occurrences: vec![
                    ProposedEvent::Move {
                        object: mountain,
                        destination: Zone::Battlefield,
                    },
                    ProposedEvent::Move {
                        object: checkland,
                        destination: Zone::Battlefield,
                    },
                ],
            },
        )
        .unwrap();
    assert!(batch.occurrences[0].before.entry_land_types.is_empty());
    let facts = &batch.occurrences[1].before.entry_land_types;
    assert!(facts.contains(&(PlayerId(1), BasicLandType::Island)));
    assert!(!facts.contains(&(PlayerId(0), BasicLandType::Mountain)));
    assert!(game.current(named(&game, "Sulfur Falls")).unwrap().tapped);
}

#[test]
fn checkland_entry_paid_and_reversed_mana_replays_through_public_choice() {
    traces(
        midgame_with_catalog(
            &[
                ("Sulfur Falls", 0, 0, true),
                // Control of a nonbasic Island from another seat qualifies.
                ("Prairie Stream", 1, 0, false),
                ("Lightning Bolt", 0, 0, true),
                ("Colossal Dreadmaw", 0, 0, true),
            ],
            checkland_catalog(),
        ),
        |g| {
            let hand = named(g, "Sulfur Falls");
            let _ = request(g);
            let object = g.handle_for(PlayerId(0), hand).unwrap();
            public(g, PlayerResponse::PlayLand { object });
            let _ = request(g);
            let land = named(g, "Sulfur Falls");
            assert_ne!(land, hand);
            assert!(!g.current(land).unwrap().tapped);

            let dreadmaw = named(g, "Colossal Dreadmaw").card;
            answer(g, Response::Cast { card: dreadmaw });
            let _ = request(g);
            activate_public(g, land);
            choose(g, Color::U);
            answer(g, Response::FinishMana { undo_mana: true });
            let _ = request(g);
            assert!(!g.current(land).unwrap().tapped);
            assert_eq!(g.mana(PlayerId(0), Color::U), Some(0.into()));
            assert!(matches!(
                g.zone(named(g, "Colossal Dreadmaw")),
                Some(Zone::Hand { .. })
            ));
            check(g);

            let bolt = named(g, "Lightning Bolt").card;
            answer(g, Response::Cast { card: bolt });
            answer(
                g,
                Response::EntityTargets {
                    entities: vec![TargetEntity::Player {
                        player: PlayerId(1),
                    }],
                },
            );
            let _ = request(g);
            activate_public(g, land);
            choose(g, Color::R);
            answer(g, Response::FinishMana { undo_mana: false });
            answer(
                g,
                Response::SpendMana {
                    spending: BTreeMap::from([(Color::R, 1.into())]),
                },
            );
            let _ = request(g);
            assert!(g.current(land).unwrap().tapped);
            assert_eq!(g.mana(PlayerId(0), Color::R), Some(0.into()));
            let paid = g.state.rules.stack.last().unwrap();
            assert_eq!(paid.paid.as_ref(), Some(&paid.total_cost));
            check(g);
        },
    );
}
