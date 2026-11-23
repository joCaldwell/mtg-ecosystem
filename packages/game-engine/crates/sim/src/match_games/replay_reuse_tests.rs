//! Independent replay verification rejects forged terminal facts.
use super::*;

fn retained(game: &Game, config: MatchConfig) -> Evidence {
    let mut remaining = 4usize;
    let mut survivor_counts = vec![4];
    for event in game.events() {
        if let mtg_core::Event::PlayersLost { losses } = event {
            remaining -= losses.len();
            survivor_counts.push(remaining);
        }
    }
    Evidence {
        config,
        policy: Policy::Develop,
        mulligans: [0; 4],
        journal: game.replay_journal(),
        trace_mode: TraceMode::Detailed,
        losses: losses(game),
        checkpoints: vec![],
        outcome: "finished".into(),
        winner: Some(PlayerId(0)),
        decisions: game.journal().len(),
        advances: 0,
        rule_steps: game.rule_boundary(),
        query_calls: 0,
        work_limit: WORK_LIMIT,
        quantum: 1024,
        turns: game.turn().number,
        final_state_sha256: game.state_digest().unwrap(),
        surviving_seats: game.live_players(),
        survivor_counts,
        elapsed_millis: 0,
    }
}
fn setup() -> (Catalog, MatchConfig, Prepared) {
    let catalog = mtg_compiler::catalog::embedded_catalog("match").unwrap();
    let config =
        MatchConfig::for_profile("commander-foundations-v1", [31; 32], PlayerId(0)).unwrap();
    let prepared = Prepared::admit_pod(catalog.clone(), config.clone()).unwrap();
    (catalog, config, prepared)
}

#[test]
fn verified_replay_rejects_forged_terminal_facts() {
    let (catalog, config, prepared) = setup();
    let mut game = Game::with_trace(prepared, TraceMode::Detailed);
    for player in [PlayerId(1), PlayerId(2), PlayerId(3)] {
        game.concede(player).unwrap();
    }
    assert!(matches!(
        game.advance(Budget::unlimited()),
        Advance::Finished {
            winner: PlayerId(0)
        }
    ));
    let mut e = retained(&game, config);
    assert_eq!(verify_replay(&catalog, &e).unwrap(), e.losses);
    e.winner = Some(PlayerId(1));
    assert_eq!(
        verify_replay(&catalog, &e).err().unwrap(),
        "match terminal result or reported winner mismatch"
    );
    e.winner = Some(PlayerId(0));
    e.final_state_sha256 = "forged".into();
    assert_eq!(
        verify_replay(&catalog, &e).err().unwrap(),
        "match replay digest mismatch"
    );
}
