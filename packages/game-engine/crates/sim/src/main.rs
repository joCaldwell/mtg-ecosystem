use mtg_compiler::{CompileResult, compile, fixture_catalog, fixtures};
use mtg_contracts::{Catalog, canonical, sha256, strict_read};
use mtg_core::{
    Advance, BUILD_ID, BUILD_PROFILE, BUILD_RUSTC, BUILD_RUSTFLAGS, BUILD_TARGET, Budget, CardId,
    Game, ObjectId, Observation, PlayerId, Prepared, QueryResult, QueryValue, ReplayJournal,
    Response, ScenarioConfig, SearchContext, TraceMode, Zone,
};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path, time::Instant};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceFile {
    path: String,
    sha256: String,
    contents: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Bundle {
    bundle_version: u32,
    trace_runs: Vec<TraceRun>,
    departure: ScenarioEvidence,
    response: ScenarioEvidence,
    detached_response: ScenarioEvidence,
    build_id: String,
    catalog: Catalog,
    config: ScenarioConfig,
    profile_digest: String,
    archive_manifest: String,
    records: Vec<String>,
    initial_snapshot: String,
    announcement_snapshot: String,
    journal: ReplayJournal,
    final_snapshot: String,
    source_files: Vec<SourceFile>,
    rustc: String,
    build_profile: String,
    target: String,
    rustflags: String,
    query_sidecar: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScenarioEvidence {
    config: ScenarioConfig,
    journal: ReplayJournal,
    checkpoints: Vec<String>,
    final_snapshot: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TraceRun {
    config: ScenarioConfig,
    journal: ReplayJournal,
    mode: TraceMode,
    final_snapshot: String,
}
fn trace_runs(
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
fn checkpoint(game: &Game, saved: &mut Vec<String>) -> Result<(), String> {
    let snapshot = game.snapshot()?;
    if Game::restore(
        Prepared::admit(fixture_catalog()?, game.config().clone())?,
        &snapshot,
    )?
    .state_digest()?
        != game.state_digest()?
    {
        return Err("departure checkpoint recovery differs".into());
    }
    saved.push(snapshot);
    Ok(())
}
fn departure_demo(catalog: Catalog) -> Result<(ScenarioEvidence, serde_json::Value), String> {
    let prepared = Prepared::admit(catalog, ScenarioConfig::commander_foundation())?;
    let mut game = Game::new(prepared.clone());
    let mut checkpoints = vec![];
    checkpoint(&game, &mut checkpoints)?;
    let land = game.object_id(CardId(0)).ok_or("missing land")?;
    respond(&mut game, Response::ActivateMana { object: land })?;
    respond(&mut game, Response::Cast { card: CardId(1) })?;
    let _ = game.advance(Budget::unlimited());
    checkpoint(&game, &mut checkpoints)?;
    let target = game.object_id(CardId(2)).ok_or("missing target")?;
    respond(
        &mut game,
        Response::Targets {
            objects: vec![target],
        },
    )?;
    let _ = game.advance(Budget::new(1));
    checkpoint(&game, &mut checkpoints)?;
    let _ = game.advance(Budget::new(1));
    checkpoint(&game, &mut checkpoints)?;
    respond(&mut game, Response::Pass)?;
    let _ = game.advance(Budget::new(1));
    checkpoint(&game, &mut checkpoints)?;
    // Departing default controller exiles the targeted Bears (800.4a).
    game.concede(PlayerId(2))?;
    checkpoint(&game, &mut checkpoints)?;
    let _ = game.advance(Budget::new(1));
    checkpoint(&game, &mut checkpoints)?;
    respond(&mut game, Response::Pass)?;
    let _ = game.advance(Budget::new(1));
    checkpoint(&game, &mut checkpoints)?;
    game.concede(PlayerId(3))?;
    checkpoint(&game, &mut checkpoints)?;
    let _ = game.advance(Budget::new(1));
    checkpoint(&game, &mut checkpoints)?;
    let _ = game.advance(Budget::new(1));
    checkpoint(&game, &mut checkpoints)?;
    game.concede(PlayerId(0))?;
    checkpoint(&game, &mut checkpoints)?;
    if game.advance(Budget::new(1))
        != (Advance::Finished {
            winner: PlayerId(1),
        })
    {
        return Err("departure winner mismatch".into());
    }
    checkpoint(&game, &mut checkpoints)?;
    let replay = Game::replay(prepared.clone(), &game.replay_journal())?;
    if replay.state_digest()? != game.state_digest()? {
        return Err("departure replay differs".into());
    }
    let report = serde_json::json!({"status":"departure_verified","state_sha256":game.state_digest()?,"events":game.events(),
        "journal":game.replay_journal(),"checkpoints":checkpoints.len(),"winner":1,"observation":observation(&game,PlayerId(1))?,
        "commander_ledger":game.commander_ledger(),"profile_digest":prepared.profile_digest()});
    Ok((
        ScenarioEvidence {
            config: prepared.config().clone(),
            journal: game.replay_journal(),
            checkpoints,
            final_snapshot: game.snapshot()?,
        },
        report,
    ))
}
fn checked_advance(game: &mut Game, saved: &mut Vec<String>) -> Result<Advance, String> {
    let result = game.advance(Budget::new(1));
    if let Advance::Fault(reason) = &result {
        return Err(reason.clone());
    }
    checkpoint(game, saved)?;
    Ok(result)
}
fn checked_input(
    game: &mut Game,
    response: Response,
    saved: &mut Vec<String>,
) -> Result<(), String> {
    loop {
        match checked_advance(game, saved)? {
            Advance::AwaitingDecision(request) => {
                game.respond(request.player(), request.decision_id(), response)?;
                checkpoint(game, saved)?;
                return Ok(());
            }
            Advance::Suspended { .. } => {}
            _ => return Err("response run finished prematurely".into()),
        }
    }
}
fn response_demo(
    catalog: Catalog,
    detach: bool,
) -> Result<(ScenarioEvidence, serde_json::Value), String> {
    let prepared = Prepared::admit(catalog, ScenarioConfig::stack_response())?;
    let mut game = Game::new(prepared.clone());
    let mut checkpoints = vec![];
    checkpoint(&game, &mut checkpoints)?;
    checked_input(
        &mut game,
        Response::ActivateMana {
            object: ObjectId {
                card: CardId(0),
                incarnation: 0,
            },
        },
        &mut checkpoints,
    )?;
    checked_input(
        &mut game,
        Response::Cast { card: CardId(1) },
        &mut checkpoints,
    )?;
    checked_input(
        &mut game,
        Response::Targets {
            objects: vec![ObjectId {
                card: CardId(2),
                incarnation: 0,
            }],
        },
        &mut checkpoints,
    )?;
    let responder = if detach { 3_u8 } else { 2_u8 };
    for _ in 0..responder {
        checked_input(&mut game, Response::Pass, &mut checkpoints)?;
    }
    checked_input(
        &mut game,
        Response::ActivateMana {
            object: ObjectId {
                card: CardId(2 * u32::from(responder) + 1),
                incarnation: 0,
            },
        },
        &mut checkpoints,
    )?;
    checked_input(
        &mut game,
        Response::Cast {
            card: CardId(2 * u32::from(responder) + 2),
        },
        &mut checkpoints,
    )?;
    checked_input(
        &mut game,
        Response::Targets {
            objects: vec![ObjectId {
                card: CardId(2),
                incarnation: 0,
            }],
        },
        &mut checkpoints,
    )?;
    let stacked = loop {
        if matches!(
            checked_advance(&mut game, &mut checkpoints)?,
            Advance::AwaitingDecision(_)
        ) {
            break observation(&game, PlayerId(0))?;
        }
    };
    for _ in 0..4 {
        checked_input(&mut game, Response::Pass, &mut checkpoints)?;
    }
    if detach {
        // The last pass starts resolution. Concede before its first instruction.
        let _ = checked_advance(&mut game, &mut checkpoints)?;
        game.concede(PlayerId(responder))?;
        checkpoint(&game, &mut checkpoints)?;
    }
    loop {
        if matches!(
            checked_advance(&mut game, &mut checkpoints)?,
            Advance::AwaitingDecision(_)
        ) {
            break;
        }
    }
    let after_top = observation(&game, PlayerId(0))?;
    for _ in 0..if detach { 3 } else { 4 } {
        checked_input(&mut game, Response::Pass, &mut checkpoints)?;
    }
    loop {
        if matches!(
            checked_advance(&mut game, &mut checkpoints)?,
            Advance::AwaitingDecision(_)
        ) {
            break;
        }
    }
    let report = serde_json::json!({"status":"response_verified","detached":detach,"state_sha256":game.state_digest()?,"events":game.events(),"journal":game.replay_journal(),
        "checkpoints":checkpoints.len(),"stacked_observation":stacked,"after_top_observation":after_top,"final_observations":(0..4).map(|v|observation(&game,PlayerId(v))).collect::<Result<Vec<_>,_>>()?,"profile_digest":prepared.profile_digest()});
    Ok((
        ScenarioEvidence {
            config: prepared.config().clone(),
            journal: game.replay_journal(),
            checkpoints,
            final_snapshot: game.snapshot()?,
        },
        report,
    ))
}
fn observation(game: &Game, viewer: PlayerId) -> Result<Observation, String> {
    match game.observe(viewer, Budget::unlimited()) {
        QueryResult::Complete(QueryValue::Observation(o)) => Ok(o),
        _ => Err("observation did not complete".into()),
    }
}
fn search_context(game: &Game, viewer: PlayerId, seed: String) -> Result<SearchContext, String> {
    match game.search_context(viewer, seed, Budget::unlimited()) {
        QueryResult::Complete(QueryValue::SearchContext(c)) => Ok(c),
        _ => Err("context did not complete".into()),
    }
}
fn resident_bytes() -> Option<u64> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    let kb = status.lines().find_map(|line| {
        line.strip_prefix("VmRSS:")
            .and_then(|v| v.split_whitespace().next())
            .and_then(|v| v.parse::<u64>().ok())
    })?;
    kb.checked_mul(1024)
}
fn respond(game: &mut Game, response: Response) -> Result<(), String> {
    let Advance::AwaitingDecision(r) = game.advance(Budget::unlimited()) else {
        return Err("scenario fault before input".into());
    };
    game.respond(r.player(), r.decision_id(), response)
}
fn finish(game: &mut Game) -> Result<(), String> {
    respond(
        game,
        Response::Targets {
            objects: vec![ObjectId {
                card: CardId(2),
                incarnation: 0,
            }],
        },
    )?;
    for _ in 0..4 {
        respond(game, Response::Pass)?;
    }
    match game.advance(Budget::unlimited()) {
        Advance::AwaitingDecision(_) => Ok(()),
        Advance::Finished { .. } => Err("unexpected scenario finish".into()),
        Advance::Fault(e) => Err(e),
        Advance::Suspended { .. } => Err("unexpected suspension with an unlimited budget".into()),
    }
}
fn source_files() -> Result<Vec<SourceFile>, String> {
    fn walk(root: &Path, path: &Path, files: &mut Vec<SourceFile>) -> Result<(), String> {
        for entry in fs::read_dir(path).map_err(|e| e.to_string())? {
            let p = entry.map_err(|e| e.to_string())?.path();
            if p.is_dir() {
                walk(root, &p, files)?;
            } else if p.extension().is_some_and(|x| x == "rs" || x == "json")
                || p.file_name().is_some_and(|x| x == "Cargo.toml")
            {
                let contents = fs::read_to_string(&p).map_err(|e| e.to_string())?;
                files.push(SourceFile {
                    path: p
                        .strip_prefix(root)
                        .map_err(|e| e.to_string())?
                        .to_string_lossy()
                        .into_owned(),
                    sha256: sha256(contents.as_bytes()),
                    contents,
                });
            }
        }
        Ok(())
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../..")
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let mut files = vec![];
    walk(&root, &root.join("packages/game-engine/crates"), &mut files)?;
    walk(
        &root,
        &root.join("packages/game-engine/tests/fixtures"),
        &mut files,
    )?;
    for file in ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"] {
        let contents = fs::read_to_string(root.join(file)).map_err(|e| e.to_string())?;
        files.push(SourceFile {
            path: file.into(),
            sha256: sha256(contents.as_bytes()),
            contents,
        });
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}
fn demo() -> Result<(Bundle, serde_json::Value), String> {
    let catalog = fixture_catalog()?;
    let prepared = Prepared::admit(catalog.clone(), ScenarioConfig::land_return())?;
    let mut game = Game::new(prepared.clone());
    let initial_snapshot = game.snapshot()?;
    let QueryResult::Incomplete { work, .. } = game.observe(PlayerId(0), Budget::new(2)) else {
        return Err("missing resumable observation".into());
    };
    let query_sidecar = game.save_work(&work)?;
    let start = Instant::now();
    respond(
        &mut game,
        Response::ActivateMana {
            object: ObjectId {
                card: CardId(0),
                incarnation: 0,
            },
        },
    )?;
    respond(&mut game, Response::Cast { card: CardId(1) })?;
    let _ = game.advance(Budget::unlimited());
    let announcement_snapshot = game.snapshot()?;
    let mut restored = Game::restore(prepared.clone(), &announcement_snapshot)?;
    let mut fork = game.fork();
    finish(&mut game)?;
    finish(&mut restored)?;
    finish(&mut fork)?;
    let replay = Game::replay(prepared.clone(), &game.replay_journal())?;
    if game.state_digest()? != restored.state_digest()?
        || game.state_digest()? != fork.state_digest()?
        || game.state_digest()? != replay.state_digest()?
    {
        return Err("restored/forked/replayed semantic state differs".into());
    }
    if game.zone(game.object_id(CardId(2)).ok_or("missing target")?)
        != Some(Zone::Hand { owner: PlayerId(1) })
        || game.zone(game.object_id(CardId(1)).ok_or("missing spell")?)
            != Some(Zone::Graveyard { owner: PlayerId(0) })
    {
        return Err("unexpected final owner/destination".into());
    }
    let elapsed = start.elapsed().as_nanos().to_string();
    let query_start = Instant::now();
    for _ in 0..1000 {
        std::hint::black_box(game.decision_for(PlayerId(0))?);
    }
    let query_ns = query_start.elapsed().as_nanos() / 1000;
    let fork_start = Instant::now();
    for _ in 0..1000 {
        std::hint::black_box(game.fork());
    }
    let fork_ns = fork_start.elapsed().as_nanos() / 1000;
    let rss_before = resident_bytes();
    let retained = (0..256).map(|_| game.fork()).collect::<Vec<_>>();
    let rss_after = resident_bytes();
    let retained_branch_observations = serde_json::json!({"branches":retained.len(),"rss_before_bytes":rss_before,"rss_after_bytes":rss_after,
        "rss_increment_bytes":rss_before.zip(rss_after).and_then(|(before,after)|after.checked_sub(before)),
        "snapshot_bytes":game.snapshot()?.len(),"representative_commander_benchmark":false});
    std::hint::black_box(&retained);
    drop(retained);
    let rustc = format!("{BUILD_RUSTC}\n");
    let (departure, departure_report) = departure_demo(catalog.clone())?;
    let departure_game = Game::restore(
        Prepared::admit(catalog.clone(), departure.config.clone())?,
        &departure.final_snapshot,
    )?;
    let (response, response_report) = response_demo(catalog.clone(), false)?;
    let (detached_response, detached_report) = response_demo(catalog.clone(), true)?;
    let response_game = Game::restore(
        Prepared::admit(catalog.clone(), response.config.clone())?,
        &response.final_snapshot,
    )?;
    let detached_game = Game::restore(
        Prepared::admit(catalog.clone(), detached_response.config.clone())?,
        &detached_response.final_snapshot,
    )?;
    let (trace_runs, trace_report) = trace_runs(
        &catalog,
        &[&game, &departure_game, &response_game, &detached_game],
    )?;
    let bundle = Bundle {
        trace_runs,
        departure,
        response,
        detached_response,
        bundle_version: 5,
        build_id: BUILD_ID.into(),
        catalog,
        config: prepared.config().clone(),
        profile_digest: prepared.profile_digest().into(),
        archive_manifest: fixtures::MANIFEST.into(),
        records: fixtures::RECORDS.iter().map(|(_, r)| (*r).into()).collect(),
        initial_snapshot,
        announcement_snapshot,
        journal: game.replay_journal(),
        final_snapshot: game.snapshot()?,
        source_files: source_files()?,
        rustc,
        build_profile: BUILD_PROFILE.into(),
        target: BUILD_TARGET.into(),
        rustflags: BUILD_RUSTFLAGS.into(),
        query_sidecar,
    };
    let report = serde_json::json!({"status":"scenario_verified","departure":departure_report,"response":response_report,"detached_response":detached_report,"trace_equivalence":trace_report,"build_id":BUILD_ID,"catalog_digest":prepared.catalog_digest(),"profile_digest":prepared.profile_digest(),
        "state_sha256":game.state_digest()?,"accepted_inputs":game.journal().len(),"events":game.events(),"restore_fork_replay_equal":true,
        "player_observations": (0..4).map(|v|observation(&game,PlayerId(v))).collect::<Result<Vec<_>,_>>()?,
        "search_context":search_context(&game,PlayerId(0),"separate-sampling-seed".into())?,
        "retained_branch_observations":retained_branch_observations,
        "timing_observations":{"four_equivalent_runs_ns":elapsed,"priority_query_mean_ns":query_ns.to_string(),"clone_and_dispose_mean_ns":fork_ns.to_string(),"representative_commander_benchmark":false}});
    Ok((bundle, report))
}
fn verify_bundle(json: &str) -> Result<String, String> {
    let b: Bundle = strict_read(json)?;
    if b.bundle_version != 5 || b.build_id != BUILD_ID {
        return Err("unsupported bundle version/build".into());
    }
    if !b.source_files.windows(2).all(|w| w[0].path < w[1].path) {
        return Err("retained source paths must be unique and ordered".into());
    }
    let mut source_bytes = Vec::new();
    for file in &b.source_files {
        if sha256(file.contents.as_bytes()) != file.sha256 {
            return Err("retained source hash mismatch".into());
        }
        source_bytes.extend_from_slice(file.path.as_bytes());
        source_bytes.push(0);
        source_bytes.extend_from_slice(file.contents.as_bytes());
        source_bytes.push(0);
    }
    source_bytes.extend_from_slice(b.rustc.as_bytes());
    for value in [&b.build_profile, &b.target, &b.rustflags] {
        source_bytes.push(0);
        source_bytes.extend_from_slice(value.as_bytes());
    }
    if sha256(&source_bytes) != b.build_id {
        return Err(
            "retained source/toolchain closure does not reproduce the build identity".into(),
        );
    }
    let mut definitions = vec![];
    let mut unsupported = 0;
    for record in &b.records {
        match compile(&b.archive_manifest, record) {
            CompileResult::Lowered { definition } => definitions.push(*definition),
            CompileResult::Unsupported { .. } => unsupported += 1,
            CompileResult::Invalid { .. } => return Err("retained record is invalid".into()),
        }
    }
    if unsupported != 1 {
        return Err("missing paired unsupported source fixture".into());
    }
    definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    if definitions != b.catalog.definitions {
        return Err("retained source-to-IR mismatch".into());
    }
    if b.trace_runs.len() != 12 {
        return Err("missing trace-mode evidence".into());
    }
    for (index, run) in b.trace_runs.iter().enumerate() {
        let required_mode = [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed][index % 3];
        let (required_config, required_journal, reference_snapshot) = match index / 3 {
            0 => (&b.config, &b.journal, &b.final_snapshot),
            1 => (
                &b.departure.config,
                &b.departure.journal,
                &b.departure.final_snapshot,
            ),
            2 => (
                &b.response.config,
                &b.response.journal,
                &b.response.final_snapshot,
            ),
            3 => (
                &b.detached_response.config,
                &b.detached_response.journal,
                &b.detached_response.final_snapshot,
            ),
            _ => return Err("unexpected trace-mode group".into()),
        };
        if run.mode != required_mode
            || &run.config != required_config
            || &run.journal != required_journal
        {
            return Err("trace-mode coverage/configuration mismatch".into());
        }
        let p = Prepared::admit(b.catalog.clone(), run.config.clone())?;
        let expected = Game::restore(p.clone(), &run.final_snapshot)?;
        let reference = Game::restore(p.clone(), reference_snapshot)?;
        let replay = Game::replay_with_trace(p, &run.journal, run.mode)?;
        if reference.trace().mode != TraceMode::Detailed
            || expected.trace().mode != run.mode
            || expected.state_digest()? != reference.state_digest()?
            || expected.state_digest()? != replay.state_digest()?
            || expected.trace() != replay.trace()
            || (run.mode != TraceMode::Off
                && expected.trace().verification != reference.trace().verification)
        {
            return Err("retained trace-mode equivalence failed".into());
        }
    }
    for scenario in [&b.response, &b.detached_response] {
        let p = Prepared::admit(b.catalog.clone(), scenario.config.clone())?;
        for snapshot in &scenario.checkpoints {
            Game::restore(p.clone(), snapshot)?;
        }
        let restored = Game::restore(p.clone(), &scenario.final_snapshot)?;
        let replay = Game::replay(p, &scenario.journal)?;
        if restored.state_digest()? != replay.state_digest()? || restored.trace() != replay.trace()
        {
            return Err("response checkpoint/replay mismatch".into());
        }
    }
    let departure_prepared = Prepared::admit(b.catalog.clone(), b.departure.config)?;
    for snapshot in &b.departure.checkpoints {
        Game::restore(departure_prepared.clone(), snapshot)?;
    }
    let departure_replay = Game::replay(departure_prepared.clone(), &b.departure.journal)?;
    let departure_expected = Game::restore(departure_prepared, &b.departure.final_snapshot)?;
    if departure_replay.state_digest()? != departure_expected.state_digest()? {
        return Err("retained departure replay differs".into());
    }
    let prepared = Prepared::admit(b.catalog, b.config)?;
    if prepared.profile_digest() != b.profile_digest {
        return Err("retained profile mismatch".into());
    }
    let initial = Game::restore(prepared.clone(), &b.initial_snapshot)?;
    if initial.state_digest()? != Game::new(prepared.clone()).state_digest()? {
        return Err("initial scenario mismatch".into());
    }
    let work = initial.restore_work(&b.query_sidecar)?;
    match initial.resume_work(work, Budget::unlimited()) {
        QueryResult::Complete(QueryValue::Observation(recovered))
            if recovered == observation(&initial, PlayerId(0))? => {}
        _ => return Err("retained query sidecar did not reproduce its observation".into()),
    }
    let restored = Game::restore(prepared.clone(), &b.announcement_snapshot)?;
    let announcement = Game::replay(
        prepared.clone(),
        &ReplayJournal {
            decisions: b.journal.decisions[..2.min(b.journal.decisions.len())].to_vec(),
            concessions: vec![],
        },
    )?;
    if restored.state_digest()? != announcement.state_digest()? {
        return Err("announcement restore mismatch".into());
    }
    let replay = Game::replay(prepared.clone(), &b.journal)?;
    let expected = Game::restore(prepared, &b.final_snapshot)?;
    if replay.state_digest()? != expected.state_digest()? {
        return Err("retained replay differs from expected final state".into());
    }
    replay.state_digest()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("schema") if args.len() == 1 => println!(
            "{}",
            serde_json::to_string_pretty(&mtg_contracts::catalog_schema())?
        ),
        Some("compile") if args.len() == 1 => println!("{}", canonical(&fixture_catalog()?)?),
        Some("observe") if args.len() == 2 => {
            let viewer = PlayerId(args[1].parse()?);
            let game = Game::new(Prepared::admit(
                fixture_catalog()?,
                ScenarioConfig::land_return(),
            )?);
            println!("{}", canonical(&observation(&game, viewer)?)?);
        }
        Some("context") if args.len() == 3 => {
            let viewer = PlayerId(args[1].parse()?);
            let game = Game::new(Prepared::admit(
                fixture_catalog()?,
                ScenarioConfig::land_return(),
            )?);
            println!(
                "{}",
                canonical(&search_context(&game, viewer, args[2].clone())?)?
            );
        }
        Some("report") if args.len() == 1 => {
            for (name, r) in fixtures::RECORDS {
                println!(
                    "{}",
                    serde_json::json!({"fixture":name,"result":compile(fixtures::MANIFEST,r)})
                );
            }
        }
        Some("demo") if args.len() == 1 || (args.len() == 3 && args[1] == "--bundle") => {
            let (bundle, report) = demo()?;
            verify_bundle(&canonical(&bundle)?)?;
            if args.len() == 3 {
                fs::create_dir_all(&args[2])?;
                let bytes = canonical(&bundle)?;
                let path = Path::new(&args[2]).join(format!("{}.json", sha256(bytes.as_bytes())));
                fs::write(&path, bytes)?;
                eprintln!("replay bundle: {}", path.display());
            }
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Some("verify-bundle") if args.len() == 2 => {
            let state = verify_bundle(&fs::read_to_string(&args[1])?)?;
            println!(
                "{}",
                serde_json::json!({"status":"replay_verified","state_sha256":state})
            );
        }
        _ => {
            return Err(
                "usage: mtg-sim schema|compile|report|observe PLAYER|context PLAYER SEED|demo [--bundle DIR]|verify-bundle FILE"
                    .into(),
            );
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundle_contains_replayable_source_to_runtime_evidence() {
        let (bundle, report) = demo().unwrap();
        let expected: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/expected-scenario.json"
        ))
        .unwrap();
        assert_eq!(report["events"], expected["events"]);
        let p2: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/expected-stack-scenario.json"
        ))
        .unwrap();
        assert_eq!(report["state_sha256"], p2["final_state_sha256"]);
        assert_eq!(report["departure"]["events"], p2["departure_events"]);
        assert_eq!(report["departure"]["journal"], p2["departure_journal"]);
        assert_eq!(
            report["departure"]["state_sha256"],
            p2["departure_state_sha256"]
        );
        let final_snapshot: serde_json::Value =
            serde_json::from_str(&bundle.final_snapshot).unwrap();
        assert_eq!(final_snapshot["snapshot_version"], p2["snapshot_version"]);
        assert_eq!(final_snapshot["state"]["rule_steps"], p2["rule_steps"]);
        assert_eq!(
            serde_json::to_value(&bundle.journal.decisions).unwrap(),
            expected["accepted_inputs"]
        );
        let prior: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/expected-departure-scenario.json"
        ))
        .unwrap();
        assert_eq!(report["departure"]["events"], prior["departure_events"]);
        assert_eq!(report["departure"]["journal"], prior["departure_journal"]);
        assert_eq!(bundle.trace_runs.len(), 12);
        let normalized:Vec<_>=report["trace_equivalence"].as_array().unwrap().iter().map(|x|serde_json::json!({"profile":x["profile"],"mode":x["mode"],"state_sha256":x["state_sha256"],"verification":x["verification"]})).collect();
        assert_eq!(
            serde_json::to_value(normalized).unwrap(),
            p2["trace_verification"]
        );
        for name in ["response", "detached_response"] {
            assert_eq!(report[name]["journal"], p2[name]["journal"]);
            assert_eq!(report[name]["events"], p2[name]["events"]);
            assert_eq!(report[name]["state_sha256"], p2[name]["state_sha256"]);
        }
        let json = canonical(&bundle).unwrap();
        assert_eq!(
            verify_bundle(&json).unwrap(),
            report["state_sha256"].as_str().unwrap()
        );
        assert!(
            verify_bundle(&json.replace("\"bundle_version\":5", "\"bundle_version\":6")).is_err()
        );
        let mut corrupt: serde_json::Value = serde_json::from_str(&json).unwrap();
        corrupt["source_files"] = serde_json::json!([]);
        assert!(verify_bundle(&canonical(&corrupt).unwrap()).is_err());
    }
}
