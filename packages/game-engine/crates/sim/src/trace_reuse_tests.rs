use super::*;

// Frozen pre-optimization algorithm, kept only as a focused equality/performance
// oracle. All original replay, semantic, verification and restore checks remain.
fn original_trace_runs(
    catalog: &Catalog,
    runs: &[&Game],
) -> Result<(Vec<TraceRun>, serde_json::Value), String> {
    let mut evidence = vec![];
    let mut reports = vec![];
    for reference in runs {
        for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
            let prepared = Prepared::admit(catalog.clone(), reference.config().clone())?;
            let game =
                Game::replay_with_trace(prepared.clone(), &reference.replay_journal(), mode)?;
            if game.state_digest()? != reference.state_digest()?
                || (mode != TraceMode::Off
                    && game.trace().verification != reference.trace().verification)
            {
                return Err("trace modes changed semantic state or verification".into());
            }
            let restored = Game::restore(prepared, &game.snapshot()?)?;
            if restored.trace() != game.trace() {
                return Err("trace-mode restore differs".into());
            }
            reports.push(serde_json::json!({"profile":reference.config().profile,"mode":mode,"state_sha256":game.state_digest()?,"verification":game.trace().verification,
                "snapshot_bytes":game.snapshot()?.len(),"retained_records":game.trace().records.len()}));
            evidence.push(TraceRun {
                config: reference.config().clone(),
                journal: reference.replay_journal(),
                mode,
                final_snapshot: game.snapshot()?,
            });
        }
    }
    Ok((evidence, serde_json::Value::Array(reports)))
}

fn short_games() -> (Catalog, Game, Game) {
    let catalog = mtg_compiler::catalog::embedded_catalog("base").unwrap();
    let prepared = Prepared::admit(catalog.clone(), ScenarioConfig::land_return()).unwrap();
    let initial = Game::new(prepared);
    let mut resolved = initial.fork();
    let land = resolved.object_id(CardId(0)).unwrap();
    respond(&mut resolved, Response::ActivateMana { object: land }).unwrap();
    respond(&mut resolved, Response::Cast { card: CardId(1) }).unwrap();
    finish(&mut resolved).unwrap();
    assert!(!resolved.trace().records.is_empty());
    assert_ne!(
        initial.state_digest().unwrap(),
        resolved.state_digest().unwrap()
    );
    (catalog, initial, resolved)
}

fn compare_pair(catalog: &Catalog, runs: &[&Game], original_first: bool) -> serde_json::Value {
    let timed = |original| {
        let start = Instant::now();
        let result = if original {
            original_trace_runs(catalog, runs)
        } else {
            trace_runs(catalog, runs)
        }
        .unwrap();
        (result, start.elapsed().as_nanos())
    };
    let (first, first_ns) = timed(original_first);
    let (second, second_ns) = timed(!original_first);
    assert_eq!(canonical(&first.0).unwrap(), canonical(&second.0).unwrap());
    assert_eq!(canonical(&first.1).unwrap(), canonical(&second.1).unwrap());
    assert_eq!(first.0.len(), runs.len() * 3);
    let (original_ns, reused_ns) = if original_first {
        (first_ns, second_ns)
    } else {
        (second_ns, first_ns)
    };
    serde_json::json!({
        "scope":"trace_runs_component_wall_only",
        "reference_games":runs.len(),
        "trace_executions":first.0.len(),
        "original_first":original_first,
        "original_wall_ns":original_ns,
        "reused_wall_ns":reused_ns,
        "exact_evidence_and_report_equality":true,
    })
}

#[test]
fn immutable_snapshot_reuse_preserves_trace_evidence_and_reports() {
    let (catalog, initial, resolved) = short_games();
    eprintln!("{}", compare_pair(&catalog, &[&initial, &resolved], true));

    // The same mismatched reference-verification diagnostic must still reject.
    let off = Game::with_trace(
        Prepared::admit(catalog.clone(), initial.config().clone()).unwrap(),
        TraceMode::Off,
    );
    let original_error = original_trace_runs(&catalog, &[&off]).err().unwrap();
    let reused_error = trace_runs(&catalog, &[&off]).err().unwrap();
    assert_eq!(
        original_error,
        "trace modes changed semantic state or verification"
    );
    assert_eq!(reused_error, original_error);
}

#[test]
#[ignore = "component timing probe; explicitly run after source review, not a full acceptance gate"]
fn immutable_snapshot_reuse_component_timing() {
    let (catalog, initial, resolved) = short_games();
    for index in 0..6 {
        eprintln!(
            "{}",
            compare_pair(&catalog, &[&initial, &resolved], index % 2 == 0)
        );
    }
}
