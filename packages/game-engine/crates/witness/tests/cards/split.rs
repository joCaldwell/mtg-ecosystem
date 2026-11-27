//! Split spells from the frozen R5 archive. CR 709.3–5, 702.102, 702.127.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use mtg_core::testkit::*;
use mtg_sim::policy::{Policy, Strategy};

macro_rules! record {
    ($id:literal) => {
        include_str!(concat!(
            "../../../../tests/fixtures/phase1-r5/",
            $id,
            ".json"
        ))
    };
}
const ASSAULT: &str = record!("fcdeb83f-531c-4087-9254-7f19eb065f00");
const WARRANT: &str = record!("19053c47-97ce-4a25-a92c-1f4061e85bf3");

fn board(record: &str) -> Board {
    let mtg_compiler::CompileResult::Lowered { definition } =
        mtg_witness::compile(mtg_witness::R5_MANIFEST, record)
    else {
        panic!("expected a split spell")
    };
    assert_eq!(definition.layout, FaceLayout::Split);
    assert_eq!(
        mtg_contracts::kir::definition_tiers(&definition).unwrap(),
        mtg_contracts::kir::Tiers::EXECUTES
    );
    let mut catalog = FixtureCatalog::Witness.get().clone();
    catalog.definitions.push(*definition);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    Board::new(catalog, "split")
        .seats(2)
        .step(TurnStep::PrecombatMain)
}

fn view(game: &Game, seat: u8) -> Observation {
    let QueryResult::Complete(QueryValue::Observation(view)) =
        game.observe(PlayerId(seat), Budget::unlimited())
    else {
        panic!("missing observation")
    };
    view
}

#[test]
fn split_halves_use_their_own_payment_targets_text_and_resolution() {
    let me = PlayerId(0);
    for (face, color, value, land, payment) in [
        (CardFace::Front, Color::R, 1, "Mountain", vec![1]),
        (CardFace::Back, Color::G, 4, "Forest", vec![1, 2, 3, 4]),
    ] {
        let mut setup = board(ASSAULT).permanent_or_hand("Assault", 0, 0, true);
        for _ in &payment {
            setup = setup.permanent_or_hand(land, 0, 0, false);
        }
        let mut game = Game::new(setup.prepare());
        let hand = Driver::object(&game, 0);
        let legal = Driver::request(&mut game, 1);
        assert!(
            legal
                .index(&Candidate::Action {
                    action: Action::Cast { card: hand }
                })
                .is_some()
        );
        assert!(
            legal
                .index(&Candidate::Action {
                    action: Action::CastBackFace { card: hand }
                })
                .is_some()
        );

        let own = view(&game, 0);
        let row = own.view.objects.iter().find(|o| o.faces.is_some()).unwrap();
        let faces = row.faces.as_ref().unwrap();
        assert_eq!(
            (&*faces.front_name, &*faces.back_name, faces.active_face),
            ("Assault", "Battery", None)
        );
        assert!(
            view(&game, 1)
                .view
                .objects
                .iter()
                .all(|o| o.faces.is_none())
        );
        let both = row.characteristics.as_ref().unwrap();
        assert_eq!(both.printed_mana_value(), 5.into());
        assert_eq!(both.colors, [Color::R, Color::G]);
        let action = if face == CardFace::Front {
            Action::Cast {
                card: row.handle.clone(),
            }
        } else {
            Action::CastBackFace {
                card: row.handle.clone(),
            }
        };
        let decision = own.decision.as_ref().unwrap();
        let answer = Strategy::new(&game.prepared.catalog, Policy::Develop, [0; 4])
            .choose_with_game(&own, &game)
            .unwrap();
        assert_eq!(
            answer,
            decision.pick_one(&Candidate::Action { action }).unwrap()
        );
        let QueryResult::Complete(QueryValue::Validated(receipt)) =
            game.check_player_response(me, decision.id.clone(), answer, Budget::unlimited())
        else {
            panic!("simulator chose an illegal half")
        };
        game.submit(receipt).unwrap();
        let next = Driver::request(&mut game, 1);
        let stack = view(&game, 1);
        let row = stack
            .view
            .objects
            .iter()
            .find(|o| o.zone == Zone::Stack)
            .unwrap();
        assert_eq!(row.faces.as_ref().unwrap().active_face, Some(face));
        let half = row.characteristics.as_ref().unwrap();
        assert_eq!(half.colors, [color]);
        assert_eq!(half.printed_mana_value(), value.into());
        assert_eq!(stack.view.stack[0].mana_value, Some(value.into()));
        let snapshot = game.snapshot().unwrap();
        game = Game::restore(game.prepared.as_ref().clone(), &snapshot).unwrap();
        assert_eq!(view(&game, 1), stack);
        if face == CardFace::Front {
            let answer = next
                .pick_one(&Candidate::Player {
                    player: PlayerId(1),
                })
                .unwrap();
            Driver::answer(&mut game, 1, answer);
        }
        Driver::pay(&mut game, &payment);
        while !game.state.rules.stack.is_empty() {
            Driver::pass(&mut game);
            Driver::request(&mut game, 1);
        }
        let view = view(&game, 1);
        let card = view
            .view
            .objects
            .iter()
            .find(|o| o.faces.is_some())
            .unwrap();
        assert_eq!(card.zone, Zone::Graveyard { owner: me });
        assert_eq!(card.faces.as_ref().unwrap().active_face, None);
        assert_eq!(card.characteristics.as_ref().unwrap(), both);
        assert_eq!(
            game.state.rules.tokens.len(),
            usize::from(face == CardFace::Back)
        );
        assert_eq!(
            game.state.rules.players[1].life,
            if face == CardFace::Front {
                38.into()
            } else {
                40.into()
            }
        );
        assert_recoverable((&game).into());
    }
}

#[test]
fn split_combines_types_in_each_nonstack_zone_and_checks_each_half_before_casting() {
    let me = PlayerId(0);
    let zones = [
        Zone::Hand { owner: me },
        Zone::Graveyard { owner: me },
        Zone::Library { owner: me },
        Zone::Exile,
        Zone::Command { owner: me },
    ];
    let mut setup = board(WARRANT);
    for zone in zones {
        setup = setup.card("Warrant", 0, 0, zone);
    }
    let mut game = Game::new(setup.prepare());
    let decision = Driver::request(&mut game, 1);
    for (i, zone) in zones.into_iter().enumerate() {
        let id = Driver::object(&game, i as u32);
        let object = game.current(id).unwrap();
        let predicate = kir::Pred(vec![
            kir::Atom::Type {
                kind: CardKind::Instant,
            },
            kir::Atom::Type {
                kind: CardKind::Sorcery,
            },
            kir::Atom::Color { color: Color::U },
            kir::Atom::ManaValue {
                cmp: kir::Cmp::Exactly,
                value: Amount::Fixed { value: 7.into() },
            },
        ]);
        assert!(query::matches_pred(&game.object_facts(object), me, &predicate).is_yes());
        let facts = game.characteristics(id).unwrap();
        assert!(facts.has_type(CardKind::Instant) && facts.has_type(CardKind::Sorcery));
        assert_eq!(facts.printed_mana_value(), 7.into());
        assert_eq!(facts.colors, [Color::W, Color::U]);
        // Warrant requires an attacking or blocking creature; Warden is a sorcery.
        assert!(
            decision
                .index(&Candidate::Action {
                    action: Action::Cast { card: id }
                })
                .is_none()
        );
        assert_eq!(
            decision
                .index(&Candidate::Action {
                    action: Action::CastBackFace { card: id }
                })
                .is_some(),
            matches!(zone, Zone::Hand { .. })
        );
    }
    let id = Driver::object(&game, 0);
    Driver::action(&mut game, Action::CastBackFace { card: id });
    Driver::request(&mut game, 1);
    let half = game.characteristics(Driver::object(&game, 0)).unwrap();
    assert!(half.has_type(CardKind::Sorcery) && !half.has_type(CardKind::Instant));
    assert_eq!(half.printed_mana_value(), 5.into());
}

#[test]
fn split_rejects_rooms_and_deferred_cast_keywords_by_name() {
    for (source, expected) in [
        (
            record!("4122720b-cad9-4ebb-b458-be71411c21e3"),
            "subtype:Room",
        ),
        (
            record!("9842734c-1eac-4509-a731-4c22017ae586"),
            "keyword:fuse",
        ),
        (
            record!("2ab857cc-64b9-4115-8fa5-6dfae9edbdb5"),
            "keyword:aftermath",
        ),
    ] {
        assert!(
            matches!(mtg_witness::compile(mtg_witness::R5_MANIFEST,source), mtg_compiler::CompileResult::Unsupported { reason,.. } if reason==expected)
        );
    }
}
