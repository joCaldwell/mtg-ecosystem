use mtg_compiler::{
    CompileResult, action_fixture_catalog, commander_fixture_catalog, compile,
    damage_fixture_catalog, damage_life_fixture_catalog, fixture_catalog, fixtures,
    library_fixture_catalog, stack_fixture_catalog,
};
use mtg_contracts::{Catalog, canonical, sha256, strict_read};
use mtg_core::{
    Advance, BUILD_DEBUG, BUILD_DEBUG_ASSERTIONS, BUILD_ID, BUILD_OPT_LEVEL, BUILD_PROFILE,
    BUILD_RUSTC, BUILD_RUSTFLAGS, BUILD_TARGET, Budget, CardId, Game, ObjectId, Observation,
    PlayerId, Prepared, QueryResult, QueryValue, ReplayJournal, Response, ScenarioConfig,
    SearchContext, SnapshotVerifier, TraceMode, TurnStep, Zone,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{BufReader, Read},
    path::Path,
    time::Instant,
};

mod actions_payments_games;
mod combat_games;
mod commander_games;
mod damage_library_games;
mod damage_life_games;
mod stack_costs_games;

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
    mana_casting: ScenarioEvidence,
    mana_undo: ScenarioEvidence,
    mana_keep: ScenarioEvidence,
    turns: ScenarioEvidence,
    combat: Vec<combat_games::CombatEvidence>,
    commander_catalog: Catalog,
    commander_records: Vec<String>,
    commander: Vec<commander_games::CommanderEvidence>,
    damage_catalog: Catalog,
    damage_records: Vec<String>,
    library_catalog: Catalog,
    library_records: Vec<String>,
    damage_library: Vec<damage_library_games::Evidence>,
    action_catalog: Catalog,
    action_records: Vec<String>,
    actions_payments: Vec<actions_payments_games::Evidence>,
    stack_catalog: Catalog,
    stack_records: Vec<String>,
    stack_costs: Vec<stack_costs_games::Evidence>,
    damage_life_catalog: Catalog,
    damage_life_records: Vec<String>,
    damage_life: Vec<damage_life_games::Evidence>,
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
    opt_level: String,
    debug: String,
    debug_assertions: String,
    target: String,
    rustflags: String,
    query_sidecar: String,
}
/// Retention orchestration needs only the small source/configuration closure.
/// Unknown bundle fields intentionally deserialize as IgnoredAny, so serde_json
/// skips large snapshot strings instead of materializing the entire bundle.
/// Full verify-bundle remains the authority for identity and replay validation.
#[derive(Serialize, Deserialize)]
struct BundleMetadata {
    bundle_version: u32,
    build_id: String,
    source_files: Vec<SourceFile>,
    rustflags: String,
    opt_level: String,
    debug: String,
    debug_assertions: String,
    target: String,
    catalog: Catalog,
}
fn read_bundle_metadata(reader: impl Read) -> Result<BundleMetadata, String> {
    let metadata: BundleMetadata =
        serde_json::from_reader(reader).map_err(|error| error.to_string())?;
    if metadata.bundle_version != 14 {
        return Err("unsupported metadata bundle version".into());
    }
    Ok(metadata)
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
    respond(&mut game, Response::FinishMana { undo_mana: false })?;
    checkpoint(&game, &mut checkpoints)?;
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
    checked_input(
        &mut game,
        Response::FinishMana { undo_mana: false },
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
    checked_input(
        &mut game,
        Response::FinishMana { undo_mana: false },
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
// Production source-backed witnesses: cast before producing U, and unpayable
// U casts with pre-floated G plus optional keep/undo of casting-time G.
fn mana_demo(
    catalog: Catalog,
    reversal: Option<bool>,
) -> Result<(ScenarioEvidence, serde_json::Value), String> {
    let config = if reversal.is_some() {
        ScenarioConfig::casting_reversal()
    } else {
        ScenarioConfig::land_return()
    };
    let prepared = Prepared::admit(catalog, config)?;
    let mut game = Game::new(prepared.clone());
    let mut checkpoints = vec![];
    checkpoint(&game, &mut checkpoints)?;
    if reversal.is_some() {
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
    }
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
    let _ = checked_advance(&mut game, &mut checkpoints)?;
    let mana_observation = observation(&game, PlayerId(0))?;
    checked_input(
        &mut game,
        Response::ActivateMana {
            object: ObjectId {
                card: CardId(if reversal.is_some() { 3 } else { 0 }),
                incarnation: 0,
            },
        },
        &mut checkpoints,
    )?;
    checked_input(
        &mut game,
        Response::FinishMana {
            undo_mana: reversal.unwrap_or(false),
        },
        &mut checkpoints,
    )?;
    loop {
        if matches!(
            checked_advance(&mut game, &mut checkpoints)?,
            Advance::AwaitingDecision(_)
        ) {
            break;
        }
    }
    if reversal.is_none() {
        for _ in 0..4 {
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
    }
    let expected_zone = if reversal.is_some() {
        Zone::Hand { owner: PlayerId(0) }
    } else {
        Zone::Graveyard { owner: PlayerId(0) }
    };
    if game.zone(
        game.object_id(CardId(1))
            .ok_or("missing mana witness spell")?,
    ) != Some(expected_zone)
        || (reversal.is_some()
            && game.mana(PlayerId(0), mtg_contracts::Color::G)
                != Some(mtg_contracts::Quantity::from(if reversal == Some(true) {
                    1
                } else {
                    2
                })))
    {
        return Err("mana witness final state mismatch".into());
    }
    let report = serde_json::json!({"status":"casting_mana_verified","undo_mana":reversal,"state_sha256":game.state_digest()?,"events":game.events(),"journal":game.replay_journal(),"checkpoints":checkpoints.len(),"mana_observation":mana_observation,"final_observations":(0..4).map(|v|observation(&game,PlayerId(v))).collect::<Result<Vec<_>,_>>()?,"profile_digest":prepared.profile_digest()});
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
// CR 302.1–2, 305.1–3, 500–514, 608.3a in the pinned 2026-09-25 rules.
// This policy stops at turn 5 main phase; it does not claim a completed match.
fn turns_demo(catalog: Catalog) -> Result<(ScenarioEvidence, serde_json::Value), String> {
    let prepared = Prepared::admit(catalog, ScenarioConfig::turns_permanents())?;
    let mut game = Game::new(prepared.clone());
    let mut checkpoints = vec![];
    checkpoint(&game, &mut checkpoints)?;
    checked_input(
        &mut game,
        Response::PlayLand { card: CardId(1) },
        &mut checkpoints,
    )?;
    checked_input(
        &mut game,
        Response::Cast { card: CardId(2) },
        &mut checkpoints,
    )?;
    for card in [0, 1] {
        let object = ObjectId {
            card: CardId(card),
            incarnation: u32::from(card == 1),
        };
        checked_input(
            &mut game,
            Response::ActivateMana { object },
            &mut checkpoints,
        )?;
    }
    checked_input(
        &mut game,
        Response::FinishMana { undo_mana: false },
        &mut checkpoints,
    )?;
    // Bounded policy guard is a host resource limit, never a rules result.
    let mut reached = false;
    for _ in 0..512 {
        match checked_advance(&mut game, &mut checkpoints)? {
            Advance::AwaitingDecision(request) => {
                if game.turn().number == 5 && game.turn().step == TurnStep::PrecombatMain {
                    reached = true;
                    break;
                }
                game.respond(request.player(), request.decision_id(), Response::Pass)?;
                checkpoint(&game, &mut checkpoints)?;
            }
            Advance::Suspended { .. } => {}
            _ => return Err("turn witness ended before its horizon".into()),
        }
    }
    if !reached {
        return Err("turn witness exhausted its host policy budget".into());
    }
    checked_input(
        &mut game,
        Response::PlayLand { card: CardId(3) },
        &mut checkpoints,
    )?;
    loop {
        if matches!(
            checked_advance(&mut game, &mut checkpoints)?,
            Advance::AwaitingDecision(_)
        ) {
            break;
        }
    }
    // Assert the reviewed final state independently of recorded state hashes.
    for (card, incarnation) in [(0, 0), (1, 1), (2, 2), (3, 1)] {
        if game.object_id(CardId(card))
            != Some(ObjectId {
                card: CardId(card),
                incarnation,
            })
            || game.zone(game.object_id(CardId(card)).ok_or("missing permanent")?)
                != Some(Zone::Battlefield)
        {
            return Err("turn witness permanent identity/destination mismatch".into());
        }
    }
    for (card, owner) in [(4, 0), (5, 1), (6, 2), (7, 3)] {
        if game.object_id(CardId(card))
            != Some(ObjectId {
                card: CardId(card),
                incarnation: 1,
            })
            || game.zone(game.object_id(CardId(card)).ok_or("missing drawn card")?)
                != Some(Zone::Hand {
                    owner: PlayerId(owner),
                })
        {
            return Err("turn witness drawn-card identity/destination mismatch".into());
        }
    }
    if game.turn().lands_played != 1 || game.active_player() != Some(PlayerId(0)) {
        return Err("turn witness scheduler mismatch".into());
    }
    let request = game
        .decision_for(PlayerId(0))?
        .ok_or("missing final priority")?;
    if game
        .check_response(
            PlayerId(0),
            request.decision_id(),
            Response::PlayLand { card: CardId(4) },
        )
        .is_ok()
    {
        return Err("turn witness allowed a second land".into());
    }
    let views = (0..4)
        .map(|v| observation(&game, PlayerId(v)))
        .collect::<Result<Vec<_>, _>>()?;
    // Visibility-safe public facts establish untap and empty pools, independent
    // of trusted full-state serialization.
    if views[0].view.objects.iter().any(|o| o.tapped)
        || views[0].view.players.iter().any(|p| {
            !p.mana.is_empty()
                || p.library_count != 0
                || p.life != mtg_contracts::Quantity::from(40)
        })
        || !views[0].view.stack.is_empty()
        || views[0].view.priority != Some(PlayerId(0))
    {
        return Err("turn witness untap/pool/life/priority mismatch".into());
    }
    let report = serde_json::json!({"status":"turns_permanents_verified","state_sha256":game.state_digest()?,"events":game.events(),"journal":game.replay_journal(),"checkpoints":checkpoints.len(),"final_observations":views,"turn":game.turn(),"profile_digest":prepared.profile_digest()});
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
    respond(game, Response::FinishMana { undo_mana: false })?;
    for _ in 0..4 {
        respond(game, Response::Pass)?;
    }
    match game.advance(Budget::unlimited()) {
        Advance::AwaitingDecision(_) => Ok(()),
        Advance::Finished { .. } | Advance::Draw => Err("unexpected scenario finish".into()),
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
    let (mana_casting, mana_casting_report) = mana_demo(catalog.clone(), None)?;
    let (mana_undo, mana_undo_report) = mana_demo(catalog.clone(), Some(true))?;
    let (mana_keep, mana_keep_report) = mana_demo(catalog.clone(), Some(false))?;
    let mana_games = [&mana_casting, &mana_undo, &mana_keep]
        .iter()
        .map(|e| {
            Game::restore(
                Prepared::admit(catalog.clone(), e.config.clone())?,
                &e.final_snapshot,
            )
        })
        .collect::<Result<Vec<_>, String>>()?;
    let (turns, turns_report) = turns_demo(catalog.clone())?;
    let turns_game = Game::restore(
        Prepared::admit(catalog.clone(), turns.config.clone())?,
        &turns.final_snapshot,
    )?;
    let (combat, combat_games, combat_report) = combat_games::suite(&catalog, false)?;
    let mut trace_games = vec![
        &game,
        &departure_game,
        &response_game,
        &detached_game,
        &mana_games[0],
        &mana_games[1],
        &mana_games[2],
        &turns_game,
    ];
    trace_games.extend(combat_games.iter());
    let (mut trace_runs, trace_report) = trace_runs(&catalog, &trace_games)?;
    let commander_catalog = commander_fixture_catalog()?;
    let (commander, commander_games, commander_report) =
        commander_games::suite(&commander_catalog, false)?;
    let commander_refs = commander_games.iter().collect::<Vec<_>>();
    let (commander_traces, commander_trace_report) =
        crate::trace_runs(&commander_catalog, &commander_refs)?;
    trace_runs.extend(commander_traces);
    let damage_catalog = damage_fixture_catalog()?;
    let library_catalog = library_fixture_catalog()?;
    let (damage_library, lifecycle_games, lifecycle_report) =
        damage_library_games::suite(&damage_catalog, &library_catalog, false)?;
    let mut lifecycle_trace_report = vec![];
    for (index, reference) in lifecycle_games.iter().enumerate() {
        let catalog = if index < 3 {
            &damage_catalog
        } else {
            &library_catalog
        };
        let (runs, reports) = crate::trace_runs(catalog, &[reference])?;
        trace_runs.extend(runs);
        lifecycle_trace_report.extend(
            reports
                .as_array()
                .ok_or("invalid lifecycle trace report")?
                .iter()
                .cloned(),
        );
    }
    let action_catalog = action_fixture_catalog()?;
    let (actions_payments, action_games, action_report) =
        actions_payments_games::suite(&action_catalog, false)?;
    let action_refs = action_games.iter().collect::<Vec<_>>();
    let (action_traces, action_trace_report) = crate::trace_runs(&action_catalog, &action_refs)?;
    trace_runs.extend(action_traces);
    let stack_catalog = stack_fixture_catalog()?;
    let (stack_costs, stack_games, stack_report) = stack_costs_games::suite(&stack_catalog, false)?;
    let stack_refs = stack_games.iter().collect::<Vec<_>>();
    let (stack_traces, stack_trace_report) = crate::trace_runs(&stack_catalog, &stack_refs)?;
    trace_runs.extend(stack_traces);
    let damage_life_catalog = damage_life_fixture_catalog()?;
    let (damage_life, damage_life_games, damage_life_report) =
        damage_life_games::suite(&damage_life_catalog, false)?;
    let damage_life_refs = damage_life_games.iter().collect::<Vec<_>>();
    let (damage_life_traces, damage_life_trace_report) =
        crate::trace_runs(&damage_life_catalog, &damage_life_refs)?;
    trace_runs.extend(damage_life_traces);
    let bundle = Bundle {
        trace_runs,
        departure,
        response,
        detached_response,
        mana_casting,
        mana_undo,
        mana_keep,
        turns,
        combat,
        commander_catalog,
        commander_records: fixtures::COMMANDER_RECORDS
            .iter()
            .map(|(_, record)| (*record).into())
            .collect(),
        commander,
        damage_catalog,
        damage_records: fixtures::DAMAGE_RECORDS
            .iter()
            .map(|(_, record)| (*record).into())
            .collect(),
        library_catalog,
        library_records: fixtures::LIBRARY_RECORDS
            .iter()
            .map(|(_, record)| (*record).into())
            .collect(),
        damage_library,
        action_catalog,
        action_records: fixtures::ACTION_RECORDS
            .iter()
            .map(|(_, record)| (*record).into())
            .collect(),
        actions_payments,
        stack_catalog,
        stack_records: fixtures::STACK_RECORDS
            .iter()
            .map(|(_, record)| (*record).into())
            .collect(),
        stack_costs,
        damage_life_catalog,
        damage_life_records: fixtures::DAMAGE_LIFE_RECORDS
            .iter()
            .map(|(_, record)| (*record).into())
            .collect(),
        damage_life,
        bundle_version: 14,
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
        opt_level: BUILD_OPT_LEVEL.into(),
        debug: BUILD_DEBUG.into(),
        debug_assertions: BUILD_DEBUG_ASSERTIONS.into(),
        target: BUILD_TARGET.into(),
        rustflags: BUILD_RUSTFLAGS.into(),
        query_sidecar,
    };
    let report = serde_json::json!({"status":"scenario_verified","damage_life":damage_life_report,"damage_life_trace_equivalence":damage_life_trace_report,"stack_costs":stack_report,"stack_costs_trace_equivalence":stack_trace_report,"actions_payments":action_report,"actions_payments_trace_equivalence":action_trace_report,"damage_library":lifecycle_report,"damage_library_trace_equivalence":lifecycle_trace_report,"commander":commander_report,"commander_trace_equivalence":commander_trace_report,"combat":combat_report,"turns":turns_report,"departure":departure_report,"response":response_report,"detached_response":detached_report,"mana_casting":mana_casting_report,"mana_undo":mana_undo_report,"mana_keep":mana_keep_report,"trace_equivalence":trace_report,"build_id":BUILD_ID,"catalog_digest":prepared.catalog_digest(),"profile_digest":prepared.profile_digest(),
        "state_sha256":game.state_digest()?,"accepted_inputs":game.journal().len(),"events":game.events(),"restore_fork_replay_equal":true,
        "player_observations": (0..4).map(|v|observation(&game,PlayerId(v))).collect::<Result<Vec<_>,_>>()?,
        "search_context":search_context(&game,PlayerId(0),"separate-sampling-seed".into())?,
        "retained_branch_observations":retained_branch_observations,
        "timing_observations":{"four_equivalent_runs_ns":elapsed,"priority_query_mean_ns":query_ns.to_string(),"clone_and_dispose_mean_ns":fork_ns.to_string(),"representative_commander_benchmark":false}});
    Ok((bundle, report))
}
/// Complete-card lowering and exact runtime admission are separate proofs.
/// Cruel Edict is now fully represented, while historical catalogs intentionally
/// retain their earlier pools. Blood Artist supplies the action pool's complete
/// but unsupported triggered source; exact bytes are bound before this check.
fn verify_source_catalog(
    manifest: &str,
    catalog: &Catalog,
    records: &[String],
    historical: bool,
) -> Result<(), String> {
    const CRUEL_EDICT: &str = "10c585c4-bf5b-4d8f-94a9-e9a5036a688f";
    const BLOOD_ARTIST: &str = "310f141c-7f37-4729-aed6-dd9c09db448d";
    const FLAME_RIFT: &str = "28479aa8-d1cd-421a-8bbb-0594bb4dd410";
    let damage_life = catalog.schema_version == 7;
    catalog.validate()?;
    let mut definitions = vec![];
    let mut lowered_unadmitted = vec![];
    let mut unsupported = vec![];
    for record in records {
        match compile(manifest, record) {
            CompileResult::Lowered { definition } => {
                if catalog
                    .definitions
                    .iter()
                    .any(|d| d.oracle_id == definition.oracle_id)
                {
                    definitions.push(*definition);
                } else if historical && definition.oracle_id == CRUEL_EDICT {
                    lowered_unadmitted.push(definition.oracle_id);
                } else {
                    return Err("unexpected lowered-but-unadmitted retained source".into());
                }
            }
            CompileResult::Unsupported { .. } => {
                let record: serde_json::Value = strict_read(record)?;
                let identity = record["oracleIdentity"]
                    .as_str()
                    .ok_or("missing unsupported source identity")?;
                if historical
                    || !(identity == BLOOD_ARTIST || damage_life && identity == FLAME_RIFT)
                {
                    return Err("unexpected unsupported retained source".into());
                }
                unsupported.push(identity.to_string());
            }
            CompileResult::Invalid { .. } => return Err("invalid retained source".into()),
        }
    }
    definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    unsupported.sort();
    let mut expected_unsupported = vec![BLOOD_ARTIST.to_string()];
    if damage_life {
        expected_unsupported.push(FLAME_RIFT.to_string());
    }
    expected_unsupported.sort();
    if definitions != catalog.definitions
        || (historical && (lowered_unadmitted != [CRUEL_EDICT] || !unsupported.is_empty()))
        || (!historical && (!lowered_unadmitted.is_empty() || unsupported != expected_unsupported))
    {
        return Err("retained source compilation/admission coverage mismatch".into());
    }
    Ok(())
}

fn verify_bundle(json: &str) -> Result<String, String> {
    let b: Bundle = strict_read(json)?;
    if b.bundle_version != 14 || b.build_id != BUILD_ID {
        return Err("unsupported bundle version/build".into());
    }
    if b.archive_manifest != fixtures::MANIFEST
        || !b
            .records
            .iter()
            .map(String::as_str)
            .eq(fixtures::RECORDS.iter().map(|(_, record)| *record))
        || !b
            .commander_records
            .iter()
            .map(String::as_str)
            .eq(fixtures::COMMANDER_RECORDS
                .iter()
                .map(|(_, record)| *record))
        || !b
            .damage_records
            .iter()
            .map(String::as_str)
            .eq(fixtures::DAMAGE_RECORDS.iter().map(|(_, record)| *record))
        || !b
            .library_records
            .iter()
            .map(String::as_str)
            .eq(fixtures::LIBRARY_RECORDS.iter().map(|(_, record)| *record))
        || !b
            .action_records
            .iter()
            .map(String::as_str)
            .eq(fixtures::ACTION_RECORDS.iter().map(|(_, record)| *record))
        || !b
            .stack_records
            .iter()
            .map(String::as_str)
            .eq(fixtures::STACK_RECORDS.iter().map(|(_, record)| *record))
        || !b
            .damage_life_records
            .iter()
            .map(String::as_str)
            .eq(fixtures::DAMAGE_LIFE_RECORDS
                .iter()
                .map(|(_, record)| *record))
    {
        return Err("retained exact source fixtures differ from this build".into());
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
    for value in [
        &b.build_profile,
        &b.target,
        &b.rustflags,
        &b.opt_level,
        &b.debug,
        &b.debug_assertions,
    ] {
        source_bytes.push(0);
        source_bytes.extend_from_slice(value.as_bytes());
    }
    if sha256(&source_bytes) != b.build_id {
        return Err(
            "retained source/toolchain closure does not reproduce the build identity".into(),
        );
    }
    for (catalog, records) in [
        (&b.catalog, &b.records),
        (&b.commander_catalog, &b.commander_records),
        (&b.damage_catalog, &b.damage_records),
        (&b.library_catalog, &b.library_records),
    ] {
        verify_source_catalog(&b.archive_manifest, catalog, records, true)?;
    }
    verify_source_catalog(
        &b.archive_manifest,
        &b.action_catalog,
        &b.action_records,
        false,
    )?;
    verify_source_catalog(
        &b.archive_manifest,
        &b.stack_catalog,
        &b.stack_records,
        false,
    )?;
    verify_source_catalog(
        &b.archive_manifest,
        &b.damage_life_catalog,
        &b.damage_life_records,
        false,
    )?;
    if b.trace_runs.len() != 258 {
        return Err("missing trace-mode evidence".into());
    }
    eprintln!("verifying 258 retained trace-mode executions");
    let mut reference_cache: Option<Game> = None;
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
            4 => (
                &b.mana_casting.config,
                &b.mana_casting.journal,
                &b.mana_casting.final_snapshot,
            ),
            5 => (
                &b.mana_undo.config,
                &b.mana_undo.journal,
                &b.mana_undo.final_snapshot,
            ),
            6 => (
                &b.mana_keep.config,
                &b.mana_keep.journal,
                &b.mana_keep.final_snapshot,
            ),
            7 => (&b.turns.config, &b.turns.journal, &b.turns.final_snapshot),
            n @ 8..=11 => {
                let entry = b.combat.get(n - 8).ok_or("missing combat trace group")?;
                (&entry.config, &entry.journal, &entry.final_snapshot)
            }
            n @ 12..=18 => {
                let entry = b
                    .commander
                    .get(n - 12)
                    .ok_or("missing commander trace group")?;
                (
                    &entry.scenario.config,
                    &entry.scenario.journal,
                    &entry.scenario.final_snapshot,
                )
            }
            n @ 19..=25 => {
                let entry = b
                    .damage_library
                    .get(n - 19)
                    .ok_or("missing damage/library trace group")?;
                (&entry.config, &entry.journal, &entry.final_snapshot)
            }
            n @ 26..=39 => {
                let entry = b
                    .actions_payments
                    .get(n - 26)
                    .ok_or("missing actions/payment trace group")?;
                (&entry.config, &entry.journal, &entry.final_snapshot)
            }
            n @ 40..=57 => {
                let entry = b
                    .stack_costs
                    .get(n - 40)
                    .ok_or("missing stack/cost trace group")?;
                (&entry.config, &entry.journal, &entry.final_snapshot)
            }
            n @ 58..=85 => {
                let entry = b
                    .damage_life
                    .get(n - 58)
                    .ok_or("missing damage/life trace group")?;
                (&entry.config, &entry.journal, &entry.final_snapshot)
            }
            _ => return Err("unexpected trace-mode group".into()),
        };
        if run.mode != required_mode
            || &run.config != required_config
            || &run.journal != required_journal
        {
            return Err("trace-mode coverage/configuration mismatch".into());
        }
        let catalog = match index / 3 {
            0..=11 => &b.catalog,
            12..=18 => &b.commander_catalog,
            19..=21 => &b.damage_catalog,
            22..=25 => &b.library_catalog,
            26..=39 => &b.action_catalog,
            40..=57 => &b.stack_catalog,
            _ => &b.damage_life_catalog,
        };
        let p = Prepared::admit(catalog.clone(), run.config.clone())?;
        let expected = Game::restore(p.clone(), &run.final_snapshot)?;
        if index % 3 == 0 {
            reference_cache = Some(Game::restore(p.clone(), reference_snapshot)?);
        }
        let reference = reference_cache
            .as_ref()
            .ok_or("missing verified trace reference")?;
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
    eprintln!("verifying retained legacy recovery checkpoints");
    for scenario in [
        &b.response,
        &b.detached_response,
        &b.mana_casting,
        &b.mana_undo,
        &b.mana_keep,
        &b.turns,
    ] {
        let p = Prepared::admit(b.catalog.clone(), scenario.config.clone())?;
        let mut verifier = SnapshotVerifier::new(p.clone(), TraceMode::Detailed);
        for snapshot in &scenario.checkpoints {
            verifier.verify(snapshot)?;
        }
        let restored = Game::restore(p.clone(), &scenario.final_snapshot)?;
        let replay = Game::replay(p, &scenario.journal)?;
        if restored.state_digest()? != replay.state_digest()? || restored.trace() != replay.trace()
        {
            return Err("response checkpoint/replay mismatch".into());
        }
    }
    combat_games::verify(&b.catalog, &b.combat)?;
    commander_games::verify(&b.commander_catalog, &b.commander)?;
    damage_library_games::verify(&b.damage_catalog, &b.library_catalog, &b.damage_library)?;
    actions_payments_games::verify(&b.action_catalog, &b.actions_payments)?;
    stack_costs_games::verify(&b.stack_catalog, &b.stack_costs)?;
    damage_life_games::verify(&b.damage_life_catalog, &b.damage_life)?;
    let departure_prepared = Prepared::admit(b.catalog.clone(), b.departure.config)?;
    let mut departure_verifier =
        SnapshotVerifier::new(departure_prepared.clone(), TraceMode::Detailed);
    for snapshot in &b.departure.checkpoints {
        departure_verifier.verify(snapshot)?;
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
        Some("bundle-metadata") if args.len() == 2 => {
            let metadata = read_bundle_metadata(BufReader::new(fs::File::open(&args[1])?))?;
            println!("{}", canonical(&metadata)?);
        }
        Some("actions-payments-games")
            if args.len() == 1 || (args.len() == 2 && args[1] == "--recover") =>
        {
            let (_, _, report) =
                actions_payments_games::suite(&action_fixture_catalog()?, args.len() == 2)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Some("stack-costs-games")
            if args.len() == 1 || (args.len() == 2 && args[1] == "--recover") =>
        {
            let (_, _, report) =
                stack_costs_games::suite(&stack_fixture_catalog()?, args.len() == 2)?;
            println!("{}", canonical(&report)?)
        }
        Some("damage-life-games")
            if args.len() == 1 || (args.len() == 2 && args[1] == "--recover") =>
        {
            let (_, _, report) =
                damage_life_games::suite(&damage_life_fixture_catalog()?, args.len() == 2)?;
            println!("{}", canonical(&report)?)
        }
        Some("compile-damage-life") if args.len() == 1 => {
            println!("{}", canonical(&damage_life_fixture_catalog()?)?)
        }
        Some("compile-stack") if args.len() == 1 => {
            println!("{}", canonical(&stack_fixture_catalog()?)?)
        }
        Some("compile-action") if args.len() == 1 => {
            println!("{}", canonical(&action_fixture_catalog()?)?)
        }
        Some("damage-library-games")
            if args.len() == 1 || (args.len() == 2 && args[1] == "--recover") =>
        {
            let (_, _, report) = damage_library_games::suite(
                &damage_fixture_catalog()?,
                &library_fixture_catalog()?,
                args.len() == 2,
            )?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Some("compile-library") if args.len() == 1 => {
            println!("{}", canonical(&library_fixture_catalog()?)?)
        }
        Some("compile-damage") if args.len() == 1 => {
            println!("{}", canonical(&damage_fixture_catalog()?)?)
        }
        Some("commander-games")
            if args.len() == 1 || (args.len() == 2 && args[1] == "--recover") =>
        {
            let (_, _, report) =
                commander_games::suite(&commander_fixture_catalog()?, args.len() == 2)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Some("compile-commander") if args.len() == 1 => {
            println!("{}", canonical(&commander_fixture_catalog()?)?)
        }
        Some("combat-games") if args.len() == 1 || (args.len() == 2 && args[1] == "--recover") => {
            let (_, _, report) = combat_games::suite(&fixture_catalog()?, args.len() == 2)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
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
                "usage: mtg-sim stack-costs-games [--recover]|compile-stack|bundle-metadata FILE|actions-payments-games [--recover]|compile-action|combat-games [--recover]|commander-games [--recover]|damage-library-games [--recover]|schema|compile|compile-commander|compile-library|compile-damage|report|observe PLAYER|context PLAYER SEED|demo [--bundle DIR]|verify-bundle FILE"
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
    fn metadata_reader_skips_unknown_nested_strings_and_requires_typed_closure() {
        let metadata = serde_json::json!({
            "bundle_version":14,"build_id":"historical-build",
            "source_files":[{"path":"Cargo.toml","sha256":"source-digest","contents":"[workspace]\n"}],
            "rustflags":"","opt_level":"3","debug":"false","debug_assertions":"false",
            "target":"x86_64-unknown-linux-gnu","catalog":fixture_catalog().unwrap(),
        });
        let raw = canonical(&metadata).unwrap();
        let unknown = canonical(&serde_json::json!([{"nested":["escaped quote: \" and slash: \\","\nUnicode: λ",{"fake":"}],\"build_id\":\"forged\""}]}])).unwrap();
        let wrapped = format!(
            "{{\"before\":{unknown},{},\"after\":{unknown}}}",
            &raw[1..raw.len() - 1]
        );
        let read = read_bundle_metadata(std::io::Cursor::new(wrapped.as_bytes())).unwrap();
        assert_eq!(serde_json::to_value(read).unwrap(), metadata);
        for key in metadata.as_object().unwrap().keys() {
            let mut missing = metadata.clone();
            missing.as_object_mut().unwrap().remove(key);
            assert!(
                read_bundle_metadata(std::io::Cursor::new(canonical(&missing).unwrap())).is_err(),
                "missing {key}"
            );
        }
        let mut bad = metadata.clone();
        bad["bundle_version"] = 13.into();
        assert!(read_bundle_metadata(std::io::Cursor::new(canonical(&bad).unwrap())).is_err());
        let mut bad = metadata.clone();
        bad["source_files"][0]["contents"] = 17.into();
        assert!(read_bundle_metadata(std::io::Cursor::new(canonical(&bad).unwrap())).is_err());
        // Parsing must also consume and validate unknown fields after the known
        // metadata; a truncated later snapshot cannot be accepted as metadata.
        let truncated = format!("{},\"after\":[\"unterminated", &raw[..raw.len() - 1]);
        assert!(read_bundle_metadata(std::io::Cursor::new(truncated)).is_err());
    }
    #[test]
    fn source_coverage_distinguishes_complete_lowering_from_catalog_admission() {
        fn records(values: &[(&str, &str)]) -> Vec<String> {
            values.iter().map(|(_, value)| (*value).into()).collect()
        }
        assert!(matches!(
            compile(fixtures::MANIFEST, fixtures::RECORDS[5].1),
            CompileResult::Lowered { .. }
        ));
        for (catalog, source) in [
            (fixture_catalog().unwrap(), records(&fixtures::RECORDS)),
            (
                commander_fixture_catalog().unwrap(),
                records(&fixtures::COMMANDER_RECORDS),
            ),
            (
                damage_fixture_catalog().unwrap(),
                records(&fixtures::DAMAGE_RECORDS),
            ),
            (
                library_fixture_catalog().unwrap(),
                records(&fixtures::LIBRARY_RECORDS),
            ),
        ] {
            verify_source_catalog(fixtures::MANIFEST, &catalog, &source, true).unwrap();
            assert!(verify_source_catalog(fixtures::MANIFEST, &catalog, &source, false).is_err());
            let mut wrong_negative = source.clone();
            wrong_negative[5] = fixtures::ACTION_RECORDS[11].1.into();
            assert!(
                verify_source_catalog(fixtures::MANIFEST, &catalog, &wrong_negative, true).is_err()
            );
        }
        for (catalog, mut source) in [
            (
                action_fixture_catalog().unwrap(),
                records(&fixtures::ACTION_RECORDS),
            ),
            (
                stack_fixture_catalog().unwrap(),
                records(&fixtures::STACK_RECORDS),
            ),
            (
                damage_life_fixture_catalog().unwrap(),
                records(&fixtures::DAMAGE_LIFE_RECORDS),
            ),
        ] {
            verify_source_catalog(fixtures::MANIFEST, &catalog, &source, false).unwrap();
            assert!(verify_source_catalog(fixtures::MANIFEST, &catalog, &source, true).is_err());
            source.remove(11);
            assert!(verify_source_catalog(fixtures::MANIFEST, &catalog, &source, false).is_err());
        }
    }
    fn current_projection_versions(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Array(values) => {
                values.iter_mut().for_each(current_projection_versions)
            }
            serde_json::Value::Object(values) => {
                for (key, value) in values.iter_mut() {
                    if key == "observation_version" || key == "context_version" {
                        *value = serde_json::json!(11);
                    } else {
                        current_projection_versions(value);
                    }
                }
            }
            _ => {}
        }
    }
    #[test]
    fn bundle_contains_replayable_source_to_runtime_evidence() {
        let (bundle, report) = demo().unwrap();
        let expected: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/expected-scenario.json"
        ))
        .unwrap();
        assert_eq!(report["events"], expected["events"]);
        let p2: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/expected-combat-games.json"
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
        assert_eq!(final_snapshot["snapshot_version"], 13);
        assert_eq!(final_snapshot["state"]["rule_steps"], p2["rule_steps"]);
        assert_eq!(
            serde_json::to_value(&bundle.journal.decisions).unwrap(),
            p2["accepted_inputs"]
        );
        let prior: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/expected-departure-scenario.json"
        ))
        .unwrap();
        assert_eq!(report["departure"]["events"], prior["departure_events"]);
        // Input IDs and boundaries now include the explicit mana-window decline.

        assert_eq!(bundle.trace_runs.len(), 258);
        let normalized:Vec<_>=report["trace_equivalence"].as_array().unwrap().iter().map(|x|serde_json::json!({"profile":x["profile"],"mode":x["mode"],"state_sha256":x["state_sha256"],"verification":x["verification"]})).collect();
        assert_eq!(
            serde_json::to_value(normalized).unwrap(),
            p2["trace_verification"]
        );
        for name in [
            "response",
            "detached_response",
            "mana_casting",
            "mana_undo",
            "mana_keep",
            "turns",
        ] {
            assert_eq!(report[name]["journal"], p2[name]["journal"]);
            assert_eq!(report[name]["events"], p2[name]["events"]);
            assert_eq!(report[name]["state_sha256"], p2[name]["state_sha256"]);
        }
        let mut historical_combat = p2["combat"].clone();
        current_projection_versions(&mut historical_combat);
        // The projection format changed; all historical semantic hashes,
        // accepted decisions and gameplay events remain exact references.
        assert_eq!(report["combat"], historical_combat);
        let mut commander: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/expected-commander-scenario.json"
        ))
        .unwrap();
        current_projection_versions(&mut commander);
        assert_eq!(report["commander"], commander);
        let mut lifecycle: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/expected-damage-library-scenario.json"
        ))
        .unwrap();
        current_projection_versions(&mut lifecycle);
        assert_eq!(report["damage_library"], lifecycle);
        let mut actions: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/expected-actions-payments-scenario.json"
        ))
        .unwrap();
        current_projection_versions(&mut actions);
        assert_eq!(report["actions_payments"], actions);
        let mut stack: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/expected-stack-costs-scenario.json"
        ))
        .unwrap();
        current_projection_versions(&mut stack);
        assert_eq!(report["stack_costs"], stack);
        let damage_life: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/expected-damage-life-scenario.json"
        ))
        .unwrap();
        assert_eq!(report["damage_life"], damage_life);
        let historical: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/expected-turns-scenario.json"
        ))
        .unwrap();
        for name in [
            "response",
            "detached_response",
            "mana_casting",
            "mana_undo",
            "mana_keep",
            "turns",
        ] {
            assert_eq!(report[name]["journal"], historical[name]["journal"]);
            assert_eq!(report[name]["events"], historical[name]["events"]);
        }
        let json = canonical(&bundle).unwrap();
        assert_eq!(
            verify_bundle(&json).unwrap(),
            report["state_sha256"].as_str().unwrap()
        );
        assert!(
            verify_bundle(&json.replace("\"bundle_version\":14", "\"bundle_version\":15")).is_err()
        );
        // A complete but changed unsupported record still reports Unsupported;
        // counting lowered/unsupported outcomes cannot bind retained evidence.
        for key in [
            "commander_records",
            "records",
            "library_records",
            "damage_records",
            "action_records",
            "stack_records",
            "damage_life_records",
        ] {
            let mut corrupt: serde_json::Value = serde_json::from_str(&json).unwrap();
            let mut record: serde_json::Value = serde_json::from_str(
                corrupt[key][if matches!(
                    key,
                    "action_records" | "stack_records" | "damage_life_records"
                ) {
                    11
                } else {
                    5
                }]
                .as_str()
                .unwrap(),
            )
            .unwrap();
            record["status"] = serde_json::json!("unsupported");
            let changed_record = canonical(&record).unwrap();
            assert!(matches!(
                compile(fixtures::MANIFEST, &changed_record),
                CompileResult::Unsupported { .. }
            ));
            corrupt[key][if matches!(
                key,
                "action_records" | "stack_records" | "damage_life_records"
            ) {
                11
            } else {
                5
            }] = serde_json::json!(changed_record);
            assert!(verify_bundle(&canonical(&corrupt).unwrap()).is_err());
        }
        let mut corrupt: serde_json::Value = serde_json::from_str(&json).unwrap();
        corrupt["source_files"] = serde_json::json!([]);
        assert!(verify_bundle(&canonical(&corrupt).unwrap()).is_err());
    }
}
