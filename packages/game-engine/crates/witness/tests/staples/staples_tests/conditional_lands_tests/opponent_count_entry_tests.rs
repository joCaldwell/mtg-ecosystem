use super::*;

const SOURCE: &str = include_str!("../../../../../../tests/fixtures/phase1-r4/bond-land.json");
const R4: &str = include_str!("../../../../../../tests/fixtures/archive-manifest-phase1-r4.json");

#[test]
fn opponent_count_entry_uses_remaining_players_and_replays() {
    let mut catalog = land_catalog();
    let mtg_compiler::CompileResult::Lowered { definition } = mtg_witness::compile(R4, SOURCE)
    else {
        panic!("the exact R4 Training Center row lowers");
    };
    catalog.definitions.push(*definition);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    let prepared = midgame_with_catalog(&[("Training Center", 0, 0, true)], catalog);
    for departures in [0, 1, 2] {
        let mut game = Game::with_trace(prepared.clone(), TraceMode::Detailed);
        for player in [PlayerId(3), PlayerId(2)].into_iter().take(departures) {
            game.concede(player).unwrap();
            let _ = request(&mut game);
        }
        let _ = request(&mut game);
        let before = named(&game, "Training Center");
        let object = game.handle_for(PlayerId(0), before).unwrap();
        public(&mut game, PlayerResponse::PlayLand { object });
        let _ = request(&mut game);
        let entered = named(&game, "Training Center");
        assert_ne!(entered, before);
        assert_eq!(game.current(entered).unwrap().tapped, departures == 2);
        assert_eq!(game.turn().lands_played, 1);
        check(&game);
    }
}
