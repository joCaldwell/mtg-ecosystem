//! CR 202.3, 400.7, 712.6–8, 712.11–14, 903.4d.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use mtg_core::testkit::*;
use serde_json::{Value, json};

macro_rules! record {
    ($id:literal) => {
        include_str!(concat!(
            "../../../../tests/fixtures/phase1-r5/",
            $id,
            ".json"
        ))
    };
}

const PATHWAY: &str = record!("1c633e02-95ef-445e-b4e0-fbfbc5ed9cc9");
const WITCH: &str = record!("0355249a-8e4e-41db-9cea-1b901faffbe6");
const ONDU: &str = record!("15fc4e74-300e-4c2d-8ed7-004553b2f7c2");
const FLORAHEDRON: &str = record!("53542c79-a62a-4d6a-97db-5296e9c68302");
const BOOMERANG: &str = record!("dc4a4996-108a-4aac-850f-2d9f76403446");
const BEARS: &str = include_str!("../../../../tests/fixtures/phase1-r5/grizzly-bears.json");
const STONE_RAIN: &str = record!("6e880df6-fc00-43d2-a9c8-f575f40b78c6");

fn lower(raw: &str) -> Definition {
    match mtg_witness::compile(mtg_witness::R5_MANIFEST, raw) {
        mtg_compiler::CompileResult::Lowered { definition } => *definition,
        outcome => panic!("modal witness did not lower: {outcome:?}"),
    }
}

fn board(records: &[&str]) -> Board {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    for record in records {
        let definition = lower(record);
        if catalog
            .definition_by_oracle_id(&definition.oracle_id)
            .is_none()
        {
            catalog.definitions.push(definition);
        }
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    Board::new(catalog, "modal-double-faced")
        .step(TurnStep::PrecombatMain)
        .seats(2)
}

fn view(game: &Game, seat: u8) -> Observation {
    let QueryResult::Complete(QueryValue::Observation(view)) =
        game.observe(PlayerId(seat), Budget::unlimited())
    else {
        panic!("observation unavailable");
    };
    view
}

fn can(decision: &Decision<Rules>, action: Action<Rules>) -> bool {
    decision.index(&Candidate::Action { action }).is_some()
}

fn settle_stack(game: &mut Game) {
    while !game.state.rules.stack.is_empty() {
        Driver::pass(game);
        Driver::request(game, u64::MAX);
    }
}

#[test]
fn modal_double_faced_land_choice_uses_one_land_play_and_resets_on_bounce() {
    let printed = lower(PATHWAY);
    assert_eq!(printed.characteristics.color_identity, [Color::B, Color::W]);
    let back = printed.back_face.as_ref().unwrap();
    assert_eq!(back.characteristics.color_identity, [Color::B, Color::W]);
    assert_eq!(
        mtg_contracts::kir::definition_tiers(&printed).unwrap(),
        mtg_contracts::kir::Tiers::EXECUTES
    );
    let mut game = Game::new(
        board(&[PATHWAY, BOOMERANG])
            .permanent_or_hand("Brightclimb Pathway", 0, 0, true)
            .permanent_or_hand("Boomerang", 0, 0, true)
            .permanent_or_hand("Island", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, false)
            .prepare(),
    );
    let hand = Driver::object(&game, 0);
    let decision = Driver::request(&mut game, 1);
    assert!(can(&decision, Action::PlayLand { object: hand }));
    assert!(can(&decision, Action::PlayLandBackFace { object: hand }));
    assert!(!can(&decision, Action::CastBackFace { card: hand }));
    let mine = view(&game, 0);
    let hand_row = mine
        .view
        .objects
        .iter()
        .find(|o| o.oracle_id == printed.oracle_id)
        .unwrap();
    let faces = hand_row.faces.as_ref().unwrap();
    assert_eq!(faces.front_name, "Brightclimb Pathway");
    assert_eq!(faces.back_name, "Grimclimb Pathway");
    assert_eq!(faces.active_face, Some(CardFace::Front));
    let public = view(&game, 1);
    assert!(
        public
            .view
            .objects
            .iter()
            .all(|o| o.oracle_id != printed.oracle_id)
    );
    assert!(public.decision.is_none());

    // Choose through the public decision's local index.
    let allowed = mine.decision.as_ref().unwrap();
    let answer = allowed
        .pick_one(&Candidate::Action {
            action: Action::PlayLandBackFace {
                object: hand_row.handle.clone(),
            },
        })
        .unwrap();
    let response =
        game.check_player_response(PlayerId(0), allowed.id.clone(), answer, Budget::unlimited());
    let QueryResult::Complete(QueryValue::Validated(receipt)) = response else {
        panic!("back-face land choice did not validate");
    };
    game.submit(receipt).unwrap();
    Driver::request(&mut game, 1);
    let land = Driver::object(&game, 0);
    assert_ne!(land, hand);
    assert_eq!(land.face, CardFace::Back);
    assert_eq!(
        game.copiable_definition(land).unwrap().name,
        "Grimclimb Pathway"
    );
    assert_eq!(game.mana_abilities(land).unwrap(), [Color::B]);
    assert_eq!(game.turn().lands_played, 1);
    let visible = view(&game, 1);
    let row = visible
        .view
        .objects
        .iter()
        .find(|o| o.oracle_id == printed.oracle_id)
        .unwrap();
    assert_eq!(
        row.faces.as_ref().unwrap().active_face,
        Some(CardFace::Back)
    );
    assert_eq!(
        row.characteristics.as_ref().unwrap().printed_mana_value(),
        0.into()
    );
    assert_recoverable((&game).into());

    Driver::cast(&mut game, CardId(1));
    Driver::pick_object(&mut game, land);
    Driver::pay(&mut game, &[2, 3]);
    settle_stack(&mut game);
    let returned = Driver::object(&game, 0);
    assert_eq!(returned.face, CardFace::Front);
    assert_eq!(
        game.current(returned).unwrap().zone,
        Zone::Hand { owner: PlayerId(0) }
    );
    let decision = Driver::request(&mut game, 1);
    assert!(!can(&decision, Action::PlayLand { object: returned }));
    assert!(!can(
        &decision,
        Action::PlayLandBackFace { object: returned }
    ));
    assert!(
        view(&game, 1)
            .view
            .objects
            .iter()
            .all(|o| o.oracle_id != printed.oracle_id)
    );
}

#[test]
fn modal_double_faced_entry_payment_uses_the_chosen_land() {
    for pay in [true, false] {
        let mut game = Game::new(
            board(&[WITCH])
                .permanent_or_hand("Witch Enchanter", 0, 0, true)
                .prepare(),
        );
        let card = Driver::object(&game, 0);
        let decision = Driver::request(&mut game, 1);
        assert!(can(&decision, Action::Cast { card }));
        assert!(!can(&decision, Action::PlayLand { object: card }));
        Driver::action(&mut game, Action::PlayLandBackFace { object: card });
        let entry = Driver::request(&mut game, 1);
        assert_eq!(entry.reason, Reason::OptionalEffect);
        assert_eq!(Driver::object(&game, 0), card);
        assert_recoverable((&game).into());
        let before_restore = [view(&game, 0), view(&game, 1)];
        let snapshot = game.snapshot().unwrap();
        game = Game::restore(game.prepared.as_ref().clone(), &snapshot).unwrap();
        let owner = view(&game, 0);
        let opponent = view(&game, 1);
        assert_eq!(owner, before_restore[0]);
        assert_eq!(opponent, before_restore[1]);
        assert_eq!(
            owner.decision.as_ref().unwrap().reason,
            Reason::OptionalEffect
        );
        let hand = &owner.view.objects[0];
        assert_eq!(hand.zone, Zone::Hand { owner: PlayerId(0) });
        assert_eq!(
            hand.faces.as_ref().unwrap().active_face,
            Some(CardFace::Front)
        );
        assert_eq!(
            hand.characteristics.as_ref().unwrap().kind,
            CardKind::Creature
        );
        let source = owner.decision_context.unwrap().source.unwrap();
        assert_eq!(source.object.as_ref(), Some(&hand.handle));
        assert_eq!(source.oracle_id, hand.oracle_id);
        assert_eq!(
            source.text,
            "Pay 3 life as Witch-Blessed Meadow enters? It enters tapped if you decline."
        );
        assert!(opponent.view.objects.is_empty());
        assert!(opponent.decision.is_none());
        assert!(opponent.decision_context.is_none());

        let mut life = game.state.rules.players[0].life.clone();
        if pay {
            life.subtract(&3.into());
        }
        let flag = if pay { Flag::Accept } else { Flag::Decline };
        Driver::answer(
            &mut game,
            1,
            entry.pick_one(&Candidate::Flag { flag }).unwrap(),
        );
        Driver::request(&mut game, 1);
        let land = Driver::object(&game, 0);
        assert_eq!(land.face, CardFace::Back);
        assert_eq!(game.current(land).unwrap().tapped, !pay);
        assert_eq!(game.state.rules.players[0].life, life);
    }
}

#[test]
fn modal_double_faced_front_spell_and_back_land_have_distinct_zone_characteristics() {
    let mut game = Game::new(
        board(&[ONDU, FLORAHEDRON])
            .permanent_or_hand("Ondu Inversion", 0, 0, true)
            .permanent_or_hand("Tangled Florahedron", 0, 0, true)
            .prepare(),
    );
    let land = Driver::object(&game, 1);
    Driver::action(&mut game, Action::PlayLandBackFace { object: land });
    Driver::request(&mut game, 1);
    let land = Driver::object(&game, 1);
    assert_eq!(game.current(land).unwrap().zone, Zone::Battlefield);
    assert!(game.current(land).unwrap().tapped);
    assert_eq!(game.characteristics(land).unwrap().kind, CardKind::Land);
    let ondu = Driver::object(&game, 0);
    Driver::action(&mut game, Action::Cast { card: ondu });
    Driver::request(&mut game, 1);
    let source = Driver::object(&game, 0);
    assert_eq!(source.face, CardFace::Front);
    assert_eq!(
        game.characteristics(source).unwrap().kind,
        CardKind::Sorcery
    );
    assert_eq!(
        game.stack_mana_value(&game.state.rules.stack[0]).unwrap(),
        8.into()
    );
}

#[test]
fn modal_double_faced_rejects_unreviewed_layouts_by_name() {
    let cases = [
        ("split", record!("03d3fee8-f7de-4152-8c18-6d279cc6c125")),
        ("prototype", record!("02457c7f-eced-4bf3-9b61-309b4ceab2d5")),
        ("flip", record!("0664dcc8-fa68-4802-936f-ecd2979ebf3c")),
        ("meld", record!("022b42a6-ee0f-4080-b973-c1b68938c3be")),
    ];
    for (layout, raw) in cases {
        let mtg_compiler::CompileResult::Unsupported { path, reason } =
            mtg_witness::compile(mtg_witness::R5_MANIFEST, raw)
        else {
            panic!("{layout} did not reject");
        };
        assert_eq!(path, "source.layout");
        assert_eq!(reason, format!("layout:{layout}"));
    }
}

/// No frozen MDFC with two spell faces lowers at this checkpoint. This
/// synthetic layout exercises the generic choice using two exact R5 blocks.
fn dual_spell_record(back: &str) -> String {
    let mut front: Value = serde_json::from_str(STONE_RAIN).unwrap();
    let back: Value = serde_json::from_str(back).unwrap();
    front["source"]["layout"] = json!("modal_dfc");
    front["source"]["name"] = json!(format!(
        "Stone Rain // {}",
        back["source"]["name"].as_str().unwrap()
    ));
    let mut identity = back["source"]["color_identity"].as_array().unwrap().clone();
    identity.push(json!("R"));
    front["source"]["color_identity"] = json!(identity);
    front["source"]["card_faces"] = json!([
        serde_json::from_str::<Value>(STONE_RAIN).unwrap()["source"],
        back["source"],
    ]);
    let mut back_face = back["faces"][0].clone();
    back_face["sourceFaceIndex"] = json!(1);
    front["faces"].as_array_mut().unwrap().push(back_face);
    let mut back_resolution = back["resolution"]["faces"][0].clone();
    back_resolution["sourceFaceIndex"] = json!(1);
    front["resolution"]["faces"]
        .as_array_mut()
        .unwrap()
        .push(back_resolution);
    let abilities = json!([
        front["faces"][0]["result"]["abilities"],
        front["faces"][1]["result"]["abilities"],
    ]);
    front["astSha256"] = json!(sha256(abilities.to_string().as_bytes()));
    for (key, hash) in [
        ("source", "sourceSha256"),
        ("resolution", "resolutionSha256"),
        ("binding", "bindingSha256"),
    ] {
        front[hash] = json!(sha256(front[key].to_string().as_bytes()));
    }
    front.to_string()
}

#[test]
fn modal_double_faced_cast_choice_uses_the_selected_spell_and_cost() {
    let raw = dual_spell_record(BOOMERANG);
    let mut game = Game::new(
        board(&[&raw])
            .step(TurnStep::Upkeep)
            .permanent_or_hand("Stone Rain", 0, 0, true)
            .permanent_or_hand("Forest", 1, 1, false)
            .permanent_or_hand("Island", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, false)
            .prepare(),
    );
    let hand = Driver::object(&game, 0);
    let front = game.copiable_definition(hand).unwrap();
    assert_eq!(front.characteristics.printed_mana_value(), 3.into());
    let priority = Driver::request(&mut game, 1);
    assert!(!can(&priority, Action::Cast { card: hand }));
    assert!(can(&priority, Action::CastBackFace { card: hand }));

    Driver::action(&mut game, Action::CastBackFace { card: hand });
    let target = Driver::object(&game, 1);
    Driver::pick_object(&mut game, target);
    let payment = Driver::request(&mut game, 1);
    let Shape::Pay { cost, .. } = payment.shape else {
        panic!("expected back-face payment");
    };
    assert_eq!(cost.generic, 0.into());
    assert_eq!(cost.colored, [Color::U, Color::U]);
    assert_eq!(game.state.rules.stack[0].source.face, CardFace::Back);
    assert_eq!(view(&game, 1).view.stack[0].mana_value, Some(2.into()));
    assert_recoverable((&game).into());

    // Reversing this cast restores the same hidden front incarnation.
    let mut reversed = game.fork();
    Driver::answer(
        &mut reversed,
        1,
        Answer::Pay {
            pay: PayAnswer::Finish {
                undo_on_failure: true,
            },
        },
    );
    Driver::request(&mut reversed, 1);
    assert_eq!(Driver::object(&reversed, 0), hand);
    assert!(view(&reversed, 1).view.stack.is_empty());

    Driver::pay(&mut game, &[2, 3]);
    settle_stack(&mut game);
    let card = Driver::object(&game, 0);
    assert_eq!(card.face, CardFace::Front);
    assert_eq!(
        game.current(card).unwrap().zone,
        Zone::Graveyard { owner: PlayerId(0) }
    );
    assert_eq!(
        game.characteristics(card).unwrap().printed_mana_value(),
        3.into()
    );
    assert_eq!(
        game.current(Driver::object(&game, 1)).unwrap().zone,
        Zone::Hand { owner: PlayerId(1) }
    );
}

#[test]
fn modal_double_faced_permanent_spell_enters_with_the_cast_face() {
    let raw = dual_spell_record(BEARS);
    let mut game = Game::new(
        board(&[&raw])
            .permanent_or_hand("Stone Rain", 0, 0, true)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .prepare(),
    );
    let card = Driver::object(&game, 0);
    let priority = Driver::request(&mut game, 1);
    assert!(can(&priority, Action::Cast { card }));
    assert!(can(&priority, Action::CastBackFace { card }));
    Driver::action(&mut game, Action::CastBackFace { card });
    Driver::pay(&mut game, &[1, 2]);
    settle_stack(&mut game);
    let permanent = Driver::object(&game, 0);
    assert_eq!(permanent.face, CardFace::Back);
    assert_eq!(game.current(permanent).unwrap().zone, Zone::Battlefield);
    assert_eq!(
        game.copiable_definition(permanent).unwrap().name,
        "Grizzly Bears"
    );
    let characteristics = game.characteristics(permanent).unwrap();
    assert_eq!(characteristics.kind, CardKind::Creature);
    assert_eq!(characteristics.power, Some(2.into()));
    assert_eq!(characteristics.toughness, Some(2.into()));
    assert_recoverable((&game).into());
}

#[test]
fn modal_double_faced_snapshot_rejects_impossible_face_state() {
    let raw = dual_spell_record(BOOMERANG);
    let valid = Game::new(
        board(&[PATHWAY, &raw])
            .permanent_or_hand("Brightclimb Pathway", 0, 0, true)
            .permanent_or_hand("Forest", 0, 0, false)
            .card("Stone Rain", 0, 0, Zone::Graveyard { owner: PlayerId(0) })
            .prepare(),
    );
    for (id, zone, message) in [
        (0, Zone::Hand { owner: PlayerId(0) }, "impossible back face"),
        (1, Zone::Battlefield, "card has no selected face"),
        (2, Zone::Battlefield, "impossible back face"),
    ] {
        let mut forged = valid.fork();
        let object = forged.state.rules.objects.get_mut(&CardId(id)).unwrap();
        object.id.face = CardFace::Back;
        object.zone = zone;
        let snapshot = forged.snapshot().unwrap();
        let Err(error) = Game::restore(forged.prepared.as_ref().clone(), &snapshot) else {
            panic!("accepted impossible face state on object {id}");
        };
        assert!(error.contains(message), "{error}");
    }
    let malformed = valid.snapshot().unwrap().replacen(
        r#""incarnation":0"#,
        r#""incarnation":0,"face":"transformed""#,
        1,
    );
    assert_ne!(malformed, valid.snapshot().unwrap());
    let Err(error) = Game::restore(valid.prepared.as_ref().clone(), &malformed) else {
        panic!("accepted an unknown face state");
    };
    assert!(error.contains("transformed"), "{error}");
}
