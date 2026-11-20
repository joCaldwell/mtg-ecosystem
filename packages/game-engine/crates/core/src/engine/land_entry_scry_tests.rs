use super::*;

#[test]
fn land_entry_scry_keeps_replacement_and_private_trigger_resolution_separate() {
    let mut catalog = land_catalog();
    let definition = lower(include_str!(
        "../../../../tests/fixtures/79f94050-d850-41ca-b1db-5ae0cf743f0a.json"
    ));
    let name = definition.name.clone();
    catalog.definitions.push(definition);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    let mut game = Game::with_trace(
        midgame_with_catalog(&[(name.as_str(), 0, 0, true)], catalog),
        TraceMode::Detailed,
    );
    let _ = request(&mut game);
    let id = named(&game, &name);
    let object = game.handle_for(PlayerId(0), id).unwrap();
    public(&mut game, PlayerResponse::PlayLand { object });
    let mut decision = request(&mut game);
    assert!(game.current(named(&game, &name)).unwrap().tapped);
    assert_eq!(game.state.rules.stack.len(), 1);
    for _ in 0..4 {
        assert!(matches!(decision, Request::Priority { .. }));
        game.respond(decision.player(), decision.decision_id(), Response::Pass)
            .unwrap();
        decision = request(&mut game);
    }
    let Request::Scry { ref candidates, .. } = decision else {
        panic!("{decision:?}")
    };
    assert_eq!(candidates.len(), 1);
    let top = candidates[0].card;
    assert!(observation(&game, 1).decision.is_none());
    let Some(PlayerRequest::Scry { candidates, .. }) = observation(&game, 0).decision else {
        panic!()
    };
    assert_eq!(candidates.len(), 1);
    check(&game);
    public(
        &mut game,
        PlayerResponse::Scry {
            top: vec![],
            bottom: candidates,
        },
    );
    let _ = request(&mut game);
    assert_eq!(game.state.rules.libraries[&PlayerId(0)].last(), Some(&top));
    check(&game);
}
