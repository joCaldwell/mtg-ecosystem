use super::*;

const SOURCE: &str =
    include_str!("../../../../tests/fixtures/7137aae6-260d-41de-8b4e-42a8cf752697.json");

#[test]
fn reveal_land_entry_choices_share_only_the_selected_identity_and_replay() {
    let mut catalog = land_catalog();
    catalog.definitions.push(lower(SOURCE));
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    let prepared = midgame_with_catalog(
        &[
            ("Frostboil Snarl", 0, 0, true),
            ("Island", 0, 0, true),
            ("Cinder Glade", 0, 0, true),
            ("Plains", 0, 0, true),
            ("Mountain", 1, 1, true),
        ],
        catalog,
    );
    let mut game = Game::with_trace(prepared, TraceMode::Detailed);
    let _ = request(&mut game);
    let source = named(&game, "Frostboil Snarl");
    let object = game.handle_for(PlayerId(0), source).unwrap();
    public(&mut game, PlayerResponse::PlayLand { object });
    let decision = request(&mut game);
    let Request::EntryReveal { ref candidates, .. } = decision else {
        panic!("{decision:?}")
    };
    let island = named(&game, "Island");
    let other_match = named(&game, "Cinder Glade");
    assert_eq!(candidates, &[island, other_match]);
    let PlayerRequest::EntryReveal { candidates, .. } = observation(&game, 0).decision.unwrap()
    else {
        panic!("missing private reveal candidates")
    };
    assert_eq!(candidates[0], game.handle_for(PlayerId(0), island).unwrap());
    observation_work(&game, PlayerId(0));
    assert!(observation(&game, 1).decision.is_none());
    assert!(game.publicly_searched_hand(PlayerId(1)).unwrap().is_empty());
    let before = game.state.clone();
    assert!(
        game.respond(
            PlayerId(0),
            decision.decision_id(),
            Response::OptionalEffect { perform: true },
        )
        .is_err()
    );
    assert_eq!(game.state, before);
    for card in [named(&game, "Plains"), named(&game, "Mountain")] {
        assert!(
            game.respond(
                PlayerId(0),
                decision.decision_id(),
                Response::EntryReveal { card: Some(card) }
            )
            .is_err()
        );
        assert_eq!(game.state, before);
    }
    check(&game);

    let mut decline = game.fork();
    public(&mut decline, PlayerResponse::EntryReveal { card: None });
    let _ = request(&mut decline);
    assert!(
        decline
            .current(named(&decline, "Frostboil Snarl"))
            .unwrap()
            .tapped
    );
    assert!(
        decline
            .publicly_searched_hand(PlayerId(1))
            .unwrap()
            .is_empty()
    );
    check(&decline);

    public(
        &mut game,
        PlayerResponse::EntryReveal {
            card: Some(candidates[0].clone()),
        },
    );
    let _ = request(&mut game);
    assert!(
        !game
            .current(named(&game, "Frostboil Snarl"))
            .unwrap()
            .tapped
    );
    assert_eq!(game.zone(island), Some(Zone::Hand { owner: PlayerId(0) }));
    assert_eq!(
        game.publicly_searched_hand(PlayerId(1)).unwrap(),
        [(
            PlayerId(0),
            game.definition(island.card).unwrap().oracle_id.clone()
        )]
    );
    assert!(game.handle_for(PlayerId(1), island).is_err());
    assert_eq!(game.turn().lands_played, 1);
    let batch = game
        .trace
        .records
        .iter()
        .find_map(|record| {
            let EvidenceRecord::Batch { batch } = &record.record else {
                return None;
            };
            matches!(batch.result(&[0]),
                Some(EventOutcome::HandCardRevealedForEntry { player: PlayerId(0), card })
                if *card == island
            )
            .then_some(batch)
        })
        .expect("reveal occurrence");
    assert!(matches!(batch.result(&[1, 0]),
        Some(EventOutcome::Moved { movement }) if movement.previous == source
    ));
    check(&game);
}
