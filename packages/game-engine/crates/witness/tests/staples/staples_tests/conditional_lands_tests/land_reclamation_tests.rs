use super::*;

fn fixture() -> Prepared {
    let mut catalog = land_catalog();
    let source =
        include_str!("../../../../../../tests/fixtures/109cdefd-e8cc-4ac7-b6ba-2cfdef8d780f.json");
    catalog.definitions.push(lower(source));
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    let mut prepared = midgame_with_catalog(
        &[
            ("Icetill Explorer", 0, 0, false),
            ("Island", 0, 0, true),
            ("Mountain", 0, 0, true),
            ("Plains", 0, 0, true),
            ("Swamp", 1, 1, true),
        ],
        catalog,
    );
    for name in ["Island", "Swamp"] {
        let id = prepared
            .catalog
            .definitions
            .iter()
            .find(|d| d.name == name)
            .unwrap()
            .oracle_id
            .as_str();
        let card = prepared
            .config
            .cards
            .iter_mut()
            .find(|c| c.oracle_id == id)
            .unwrap();
        card.zone = Zone::Graveyard { owner: card.owner };
    }
    prepared.profile_digest = digest(&prepared.config).unwrap();
    prepared
}

#[test]
fn land_reclamation_plays_own_graveyard_and_extra_land_and_mills_on_both() {
    let mut game = Game::with_trace(fixture(), TraceMode::Detailed);
    let player = PlayerId(0);
    let _ = request(&mut game);
    let wrong_owner = named(&game, "Swamp");
    assert!(game.validate_land_play(player, wrong_owner.card).is_err());
    let before = game.state.rules.libraries[&player].len();
    for (used, name) in ["Island", "Mountain"].into_iter().enumerate() {
        let land = named(&game, name);
        let object = game.handle_for(player, land).unwrap();
        public(&mut game, PlayerResponse::PlayLand { object });
        let _ = request(&mut game);
        assert_eq!(game.zone(named(&game, name)), Some(Zone::Battlefield));
        for _ in 0..4 {
            let decision = request(&mut game);
            game.respond(decision.player(), decision.decision_id(), Response::Pass)
                .unwrap();
        }
        let _ = request(&mut game);
        assert_eq!(game.state.rules.libraries[&player].len(), before - used - 1);
    }
    let extra = named(&game, "Plains");
    assert!(game.validate_land_play(player, extra.card).is_err());
    check(&game);
}
