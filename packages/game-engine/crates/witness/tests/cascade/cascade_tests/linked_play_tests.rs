//! Paid activation after a real ETB and a later untap. Two cards in the
//! opening library leave a singleton bottom and need no synthetic randomness.
use super::*;
use mtg_witness::temporary::pass;

const PLAYER: PlayerId = PlayerId(0);

fn scenario(hit: &str) -> Prepared {
    let mut catalog = Catalog::read(include_str!(
        "../../../../../tests/fixtures/expected-mana-ir.json"
    ))
    .unwrap();
    let archive = std::fs::read(
        std::env::var_os("MTG_LINKED_HIDEAWAY_ARCHIVE_PROBE")
            .expect("supply the exact linked hideaway archive"),
    )
    .unwrap();
    assert_eq!(
        mtg_contracts::sha256(&archive),
        "114f814949a6c0ddf680ac569113b15452e9a9efb025cbd79e04f58220d9fe4f"
    );
    let lines = std::str::from_utf8(&archive)
        .unwrap()
        .lines()
        .collect::<Vec<_>>();
    let [manifest, record] = lines.as_slice() else {
        panic!("expected one frozen source")
    };
    let mtg_compiler::CompileResult::Lowered { definition } =
        mtg_witness::compile(manifest, record)
    else {
        panic!("complete Mosswort Bridge source must lower");
    };
    catalog.definitions.push(*definition);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    let mut config = ScenarioConfig::turns_permanents();
    config.profile = "private-linked-land-play".into();
    config.initial.turn_step = TurnStep::PrecombatMain;
    for (index, name, zone) in [
        (1, "Mosswort Bridge", Zone::Hand { owner: PLAYER }),
        (2, hit, Zone::Library { owner: PLAYER }),
        (3, "Yargle and Multani", Zone::Battlefield),
    ] {
        let card = &mut config.cards[index];
        card.zone = zone;
        card.oracle_id = catalog
            .definitions
            .iter()
            .find(|d| d.name == name)
            .unwrap()
            .oracle_id
            .clone();
    }
    Prepared {
        catalog_digest: digest(&catalog).unwrap(),
        profile_digest: digest(&config).unwrap(),
        catalog: Arc::new(catalog),
        config,
        match_config: None,
    }
}

#[test]
#[ignore = "requires the pinned MTG_LINKED_HIDEAWAY_ARCHIVE_PROBE"]
fn mosswort_paid_hideaway_and_linked_play() {
    for hit in ["Grizzly Bears", "Forest"] {
        let mut g = Game::with_trace(scenario(hit), TraceMode::Off);
        answer(&mut g, Response::PlayLand { card: CardId(1) });
        for _ in 0..4 {
            answer(&mut g, Response::Pass);
        }
        let Request::Hideaway { candidates, .. } = request(&mut g) else {
            panic!("missing ETB look")
        };
        assert_eq!(candidates.len(), 2);
        let chosen = candidates[0];
        answer(&mut g, Response::Hideaway { object: chosen });

        let mut ready = false;
        for _ in 0..400 {
            let r = request(&mut g);
            if g.active_player() == Some(PLAYER)
                && g.turn().step == TurnStep::Upkeep
                && matches!(r, Request::Priority { player: PLAYER, .. })
            {
                ready = true;
                break;
            }
            g.respond(r.player(), r.decision_id(), pass(&r)).unwrap();
        }
        assert!(ready, "Bridge should untap before the next draw");
        let bridge = g.object_id(CardId(1)).unwrap();
        let hidden = g.object_id(chosen.card).unwrap();
        assert_eq!(g.linked_hideaway_cards(bridge, 0), [hidden]);
        assert_eq!(g.turn().lands_played, 0);
        answer(
            &mut g,
            Response::ActivateAbility {
                object: bridge,
                ability: 0,
            },
        );
        let r = request(&mut g);
        assert!(matches!(r, Request::Mana { .. }));
        assert!(
            g.check_response(
                PLAYER,
                r.decision_id(),
                Response::ActivateMana { object: bridge }
            )
            .is_err()
        );
        let forest = g.object_id(CardId(0)).unwrap();
        answer(&mut g, Response::ActivateMana { object: forest });
        answer(&mut g, Response::FinishMana { undo_mana: false });
        answer(
            &mut g,
            Response::SpendMana {
                spending: BTreeMap::from([(Color::G, 1.into())]),
            },
        );
        for _ in 0..4 {
            answer(&mut g, Response::Pass);
        }
        let Request::PlayHideawayLinked {
            candidates,
            decision_id,
            ..
        } = request(&mut g)
        else {
            panic!("missing paid linked play")
        };
        assert_eq!(candidates, [hidden]);
        assert!(g.current(bridge).unwrap().tapped);
        assert!(
            g.check_response(
                PLAYER,
                decision_id,
                Response::PlayHideawayLinked {
                    object: Some(chosen)
                }
            )
            .is_err()
        );
        prove(&g);
        answer(
            &mut g,
            Response::PlayHideawayLinked {
                object: Some(hidden),
            },
        );
        let r = request(&mut g);
        assert!(matches!(r, Request::Priority { .. }));
        assert!(g.linked_hideaway_cards(bridge, 0).is_empty());
        if hit == "Forest" {
            assert!(g.state.rules.stack.is_empty());
            assert_eq!(g.turn().lands_played, 1);
            assert_eq!(g.state.rules.objects[&chosen.card].zone, Zone::Battlefield);
        } else {
            let [child] = g.state.rules.stack.as_slice() else {
                panic!("expected only the paid child")
            };
            assert_eq!(child.source.card, chosen.card);
            assert_eq!(child.cast_from, Zone::Exile);
            assert_eq!(child.paid.as_ref(), Some(&zero_mana_cost()));
            assert_eq!(g.turn().lands_played, 0);
        }
        prove(&g);
    }
}
