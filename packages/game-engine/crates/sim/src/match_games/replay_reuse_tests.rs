//! Focused verification/report reuse checks; no additional production interface.
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
fn verified_replay_owns_detailed_game_without_accepting_forged_terminal_facts() {
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
    let verified = verify_replay_game(&catalog, &e).unwrap();
    assert_eq!(verified.game.trace().mode, TraceMode::Detailed);
    assert_eq!(verified.game.state_digest().unwrap(), e.final_state_sha256);
    assert_eq!(verified.loss_facts, e.losses);
    assert_eq!(
        interactions::verified_game(&catalog, &verified).unwrap(),
        interactions::game(&catalog, &e).unwrap()
    );
    e.winner = Some(PlayerId(1));
    assert_eq!(
        verify_replay_game(&catalog, &e).err().unwrap(),
        "match terminal result or reported winner mismatch"
    );
    e.winner = Some(PlayerId(0));
    e.final_state_sha256 = "forged".into();
    assert_eq!(
        verify_replay_game(&catalog, &e).err().unwrap(),
        "match replay digest mismatch"
    );
}

#[test]
fn rejected_nonterminal_evidence_keeps_legacy_interaction_diagnostics() {
    let (catalog, config, prepared) = setup();
    let mut game = Game::with_trace(prepared, TraceMode::Detailed);
    assert!(matches!(
        game.advance(Budget::unlimited()),
        Advance::AwaitingDecision(_)
    ));
    let e = retained(&game, config);
    assert_eq!(
        verify_replay_game(&catalog, &e).err().unwrap(),
        "match terminal result or reported winner mismatch"
    );
    // Failed verification never grants a VerifiedReplay, but the historical
    // reporting-only path still scans a reachable incomplete journal.
    let diagnostic = interactions::game(&catalog, &e).unwrap();
    assert_eq!(diagnostic["cards_used"], serde_json::json!({}));
    assert_eq!(diagnostic["instructions"], serde_json::json!({}));
}

#[test]
#[ignore = "paired component timing; requires sealed MTG_REPLAY_PROBE_ROOT evidence"]
fn paired_sealed_replay_reporting_probe() {
    let root = std::path::PathBuf::from(
        std::env::var_os("MTG_REPLAY_PROBE_ROOT").expect("MTG_REPLAY_PROBE_ROOT is required"),
    );
    for (profile, catalog_name) in [
        ("commander-attachments-v1", "attachments"),
        ("commander-removal-v1", "removal"),
        ("commander-wipes-v1", "wipes"),
    ] {
        let catalog = mtg_compiler::catalog::embedded_catalog(catalog_name).unwrap();
        let directory = root.join("batches").join(profile);
        let path = directory.join("game-01.json");
        let input = fs::read_to_string(&path).unwrap();
        let e: Evidence = strict_read(&input).unwrap();
        verify_suite_entry(1, &e).unwrap();
        let archived: serde_json::Value =
            strict_read(&fs::read_to_string(directory.join("report.json")).unwrap()).unwrap();
        let expected = &archived["games"][1];
        assert_eq!(expected["index"], 1);

        // Old reporting performs a fresh detailed replay after complete replay
        // verification. Measure both old and new paths in this same executable;
        // compilation, evidence decoding and archived-report reading are excluded.
        let clock = Instant::now();
        let old_losses = verify_replay(&catalog, &e).unwrap();
        let old_verify_ns = clock.elapsed().as_nanos();
        let report_clock = Instant::now();
        let old_interactions = interactions::game(&catalog, &e).unwrap();
        let old_report_ns = report_clock.elapsed().as_nanos();
        let old_total_ns = clock.elapsed().as_nanos();

        let clock = Instant::now();
        let (new_losses, new_interactions, new_verify_ns, new_report_ns) = {
            let verified = verify_replay_game(&catalog, &e).unwrap();
            let verify_ns = clock.elapsed().as_nanos();
            let report_clock = Instant::now();
            let interactions = interactions::verified_game(&catalog, &verified).unwrap();
            let report_ns = report_clock.elapsed().as_nanos();
            (verified.loss_facts, interactions, verify_ns, report_ns)
        };
        // Includes disposal of the owned verified Game, as in the old path.
        let new_total_ns = clock.elapsed().as_nanos();
        assert_eq!(old_losses, new_losses);
        assert_eq!(old_interactions, new_interactions);
        assert_eq!(
            serde_json::to_value(&new_losses).unwrap(),
            expected["verified_losses"]
        );
        assert_eq!(new_interactions, expected["interactions"]);
        println!(
            "MTG_REPLAY_REUSE_PROBE={}",
            canonical(&serde_json::json!({
                "profile": profile,
                "index": 1,
                "build_id": BUILD_ID,
                "input": path,
                "input_sha256": sha256(input.as_bytes()),
                "journal_sha256": mtg_contracts::digest(&e.journal).unwrap(),
                "final_state_sha256": e.final_state_sha256,
                "old_verify_ns": old_verify_ns,
                "old_report_ns": old_report_ns,
                "old_total_ns": old_total_ns,
                "new_verify_ns": new_verify_ns,
                "new_report_ns": new_report_ns,
                "new_total_ns": new_total_ns,
                "exact_loss_and_interaction_equality": true,
                "archived_loss_and_interaction_equality": true,
                "scope": "component only: independent verification plus interaction reporting; same binary, old then new; no construction or end-to-end timing claim"
            })).unwrap()
        );
    }
}
