//! Exact basic fetch sources, paid activations and trusted replay.
use super::super::typed_land_search_tests::typed_prepared;
use super::*;

const VISTA: &str =
    include_str!("../../../../tests/fixtures/032b8a0d-491a-4a12-ab9f-689010054d5b.json");
const WILDS: &str =
    include_str!("../../../../tests/fixtures/a75445d3-1303-4bb5-89ad-26ea93fecd48.json");
const BAUBLE: &str =
    include_str!("../../../../tests/fixtures/31f15274-301b-47c5-ba19-0ced04520878.json");

#[test]
fn basic_land_fetch_paid_sources_private_choice_and_recovery() {
    for (raw, tapped, life, mana) in [
        (VISTA, false, 1, false),
        (WILDS, true, 0, false),
        (BAUBLE, true, 0, true),
    ] {
        let name = lower(raw).name;
        let (prepared, typed_nonbasic) = seeded_fetch(raw);
        let mut game = Game::with_trace(prepared, TraceMode::Detailed);
        let card = develop_search(&mut game, &name, if mana { 3 } else { 0 });
        if mana {
            paid_cast(&mut game, &name);
            resolve(&mut game, vec![]);
        } else {
            let object = game
                .handle_for(PlayerId(0), game.object_id(card).unwrap())
                .unwrap();
            public(&mut game, PlayerResponse::PlayLand { object });
            let _ = request(&mut game);
        }
        let source = game.object_id(card).unwrap();
        assert_eq!(game.zone(source), Some(Zone::Battlefield));
        let mut expected_life = game.state.rules.players[0].life.clone();
        expected_life.subtract(&Quantity::from(life));
        let _ = request(&mut game);
        let object = game.handle_for(PlayerId(0), source).unwrap();
        let activate = PlayerResponse::ActivateAbility { object, ability: 0 };
        public(&mut game, activate.clone());
        if mana {
            answer(&mut game, Response::FinishMana { undo_mana: true });
            let _ = request(&mut game);
            assert_eq!(game.zone(source), Some(Zone::Battlefield));
            assert!(!game.current(source).unwrap().tapped);
            check(&game);
            public(&mut game, activate);
            let _ = request(&mut game);
            let sources = game
                .state
                .rules
                .objects
                .values()
                .filter(|object| {
                    game.validate_mana_activation(PlayerId(0), object.id)
                        .is_ok()
                })
                .map(|object| object.id)
                .collect::<Vec<_>>();
            assert_eq!(sources.len(), 2);
            for object in sources {
                activate_public(&mut game, object);
            }
            answer(&mut game, Response::FinishMana { undo_mana: false });
            answer(
                &mut game,
                Response::SpendMana {
                    spending: BTreeMap::from([(Color::G, 2.into())]),
                },
            );
        }
        let mut decision = request(&mut game);
        let invocation = game.state.rules.stack.last().unwrap();
        assert_eq!(invocation.paid.as_ref(), Some(&invocation.total_cost));
        assert!(invocation.source_facts.as_ref().unwrap().tapped);
        assert!(matches!(
            game.zone(game.object_id(source.card).unwrap()),
            Some(Zone::Graveyard { .. })
        ));
        assert_eq!(game.state.rules.players[0].life, expected_life);
        check(&game);

        // Four passes start resolving the paid activation.
        for _ in 0..4 {
            assert!(matches!(decision, Request::Priority { .. }));
            game.respond(decision.player(), decision.decision_id(), Response::Pass)
                .unwrap();
            decision = request(&mut game);
        }
        let Request::SearchLands {
            candidates,
            maximum: 1,
            ..
        } = decision
        else {
            panic!("{decision:?}")
        };
        assert!(
            !candidates
                .iter()
                .any(|object| object.card == typed_nonbasic)
        );
        let PlayerRequest::SearchLands {
            candidates: visible,
            library,
            ..
        } = observation(&game, 0).decision.unwrap()
        else {
            panic!()
        };
        assert!(library.contains(&lower(CINDER).oracle_id));
        assert!(library.windows(2).all(|ids| ids[0] <= ids[1]));
        assert_eq!(
            library.len(),
            game.state.rules.libraries[&PlayerId(0)].len()
        );
        let handle = visible[0].handle.clone();
        for seat in 1..4 {
            assert!(observation(&game, seat).decision.is_none());
            assert!(game.resolve_search_handle(PlayerId(seat), &handle).is_err());
        }
        let object = game.resolve_search_handle(PlayerId(0), &handle).unwrap();
        let prior_shuffles = observation(&game, 0).view.players[0].library_shuffles;
        check(&game);
        public(
            &mut game,
            PlayerResponse::SearchLands {
                objects: vec![handle],
            },
        );
        let _ = request(&mut game);
        assert_eq!(
            observation(&game, 0).view.players[0].library_shuffles,
            prior_shuffles + 1
        );
        let entered = game.object_id(object.card).unwrap();
        assert_ne!(object, entered);
        assert_eq!(game.zone(entered), Some(Zone::Battlefield));
        assert_eq!(game.current(entered).unwrap().tapped, tapped);
        check(&game);
    }
}

// Keep the genuine seeded setup. Replace one card away from the opening and
// the first few draws, before constructing the replay root.
fn seeded_fetch(raw: &str) -> (Prepared, CardId) {
    let mut prepared = typed_prepared(raw);
    let mut opening_probe = Game::new(prepared.clone());
    opening(&mut opening_probe);
    let nonbasic = opening_probe.state.rules.libraries[&PlayerId(0)][50];
    let definition = lower(CINDER);
    prepared
        .config
        .cards
        .iter_mut()
        .find(|row| row.id == nonbasic)
        .unwrap()
        .oracle_id = definition.oracle_id.clone();
    let catalog = Arc::make_mut(&mut prepared.catalog);
    catalog
        .definitions
        .retain(|old| old.oracle_id != definition.oracle_id);
    catalog.definitions.push(definition);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    prepared.catalog_digest = digest(catalog).unwrap();
    let mut counts = BTreeMap::new();
    for row in prepared
        .config
        .cards
        .iter()
        .filter(|row| row.owner == PlayerId(0))
    {
        *counts.entry(row.oracle_id.clone()).or_insert(0) += 1;
    }
    prepared.match_config.as_mut().unwrap().decks[0].cards = counts
        .into_iter()
        .map(|(oracle_id, count)| DeckEntry { oracle_id, count })
        .collect();
    prepared.profile_digest = digest(&(&prepared.config, &prepared.match_config)).unwrap();
    (prepared, nonbasic)
}
