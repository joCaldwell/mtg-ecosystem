use mtg_compiler::fixtures;
use mtg_contracts::{Catalog, accepted_manifest, canonical, sha256, strict_read};
use mtg_core::{
    Advance, Budget, CardId, Game, ObjectId, Observation, PlayerId, Prepared, QueryResult,
    QueryValue, ReplayJournal, Response, ScenarioConfig, SearchContext, SnapshotVerifier,
    TraceMode, TurnStep, Zone,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{BufReader, Read},
    path::Path,
    time::Instant,
};

/// This executable's engine build identity: build.rs hashes the production
/// engine source closure (`crates/build_identity.rs`). `main` installs it into
/// mtg-core before any engine use; retained source-closure evidence reproduces
/// it. Unit tests leave core at `BuildIdentity::TEST`, and nothing here compares
/// that test identity with this compiled one.
pub(crate) const BUILD: mtg_core::BuildIdentity = mtg_core::compiled_build_identity!();

mod bulk_input;
mod combat_games;
mod coverage_cli;
mod deck_analytics;
#[cfg(all(test, feature = "diagnostic-allocations"))]
mod diagnostic_allocations;
mod match_games;
mod search_candidates;
mod search_cli;
mod search_jobs;
mod search_policy;
mod search_roots;
mod search_sampler;
mod smoke;
#[cfg(test)]
mod trace_reuse_tests;

#[derive(Clone, Serialize, Deserialize)]
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
    if metadata.bundle_version != 21 {
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
        let reference_digest = reference.state_digest()?;
        for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
            let prepared = Prepared::admit(catalog.clone(), reference.config().clone())?;
            let game =
                Game::replay_with_trace(prepared.clone(), &reference.replay_journal(), mode)?;
            let (snapshot, digest) = game.snapshot_with_digest()?;
            if digest != reference_digest
                || (mode != TraceMode::Off
                    && game.trace().verification != reference.trace().verification)
            {
                return Err("trace modes changed semantic state or verification".into());
            }
            let restored = Game::restore(prepared, &snapshot)?;
            if restored.trace() != game.trace() {
                return Err("trace-mode restore differs".into());
            }
            reports.push(serde_json::json!({"profile":reference.config().profile,"mode":mode,"state_sha256":digest,"verification":game.trace().verification,
                "snapshot_bytes":snapshot.len(),"retained_records":game.trace().records.len()}));
            evidence.push(TraceRun {
                config: reference.config().clone(),
                journal: reference.replay_journal(),
                mode,
                final_snapshot: snapshot,
            });
        }
    }
    Ok((evidence, serde_json::Value::Array(reports)))
}
fn checkpoint(game: &Game, saved: &mut Vec<String>) -> Result<(), String> {
    let snapshot = game.snapshot()?;
    if Game::restore(
        Prepared::admit(
            mtg_compiler::catalog::embedded_catalog("base").map_err(|e| e.to_string())?,
            game.config().clone(),
        )?,
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
                let finish = matches!(response, Response::FinishMana { .. });
                game.respond(request.player(), request.decision_id(), response)?;
                checkpoint(game, saved)?;
                if finish {
                    complete_spending(game)?;
                    checkpoint(game, saved)?;
                }
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
                let response = match &request {
                    mtg_core::Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
                    mtg_core::Request::Blockers { .. } => Response::Blockers { blocks: vec![] },
                    _ => Response::Pass,
                };
                game.respond(request.player(), request.decision_id(), response)?;
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
fn complete_spending(game: &mut Game) -> Result<(), String> {
    if let Advance::AwaitingDecision(mtg_core::Request::SpendMana {
        player,
        decision_id,
        cost,
        available,
        ..
    }) = game.advance(Budget::unlimited())
    {
        let spending = match_games::spend(&cost, &available).ok_or("unpayable spending request")?;
        game.respond(player, decision_id, Response::SpendMana { spending })?;
    }
    Ok(())
}
fn respond(game: &mut Game, response: Response) -> Result<(), String> {
    let finish = matches!(response, Response::FinishMana { .. });
    let Advance::AwaitingDecision(r) = game.advance(Budget::unlimited()) else {
        return Err("scenario fault before input".into());
    };
    game.respond(r.player(), r.decision_id(), response)?;
    if finish {
        complete_spending(game)?;
    }
    Ok(())
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
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../..")
        .canonicalize()
        .map_err(|e| e.to_string())?;
    source_files_from(&root)
}
fn verified_current_source_files() -> Result<Vec<SourceFile>, String> {
    let files = source_files()?;
    let rustc = format!("{}\n", BUILD.rustc);
    verify_native_source_bytes(
        &files,
        BUILD.id,
        [
            rustc.as_str(),
            BUILD.profile,
            BUILD.target,
            BUILD.rustflags,
            BUILD.opt_level,
            BUILD.debug,
            BUILD.debug_assertions,
        ],
    )?;
    Ok(files)
}
fn current_source_manifest() -> Result<serde_json::Value, String> {
    let files = verified_current_source_files()?;
    Ok(serde_json::json!({
        "status":"compiled-source-closure-verified", "build_id":BUILD.id,
        "files":files.iter().map(|file|serde_json::json!({
            "path":file.path,"sha256":file.sha256,"bytes":file.contents.len()
        })).collect::<Vec<_>>()
    }))
}
fn source_files_from(root: &Path) -> Result<Vec<SourceFile>, String> {
    fn walk(root: &Path, path: &Path, files: &mut Vec<SourceFile>) -> Result<(), String> {
        for entry in fs::read_dir(path).map_err(|e| e.to_string())? {
            let p = entry.map_err(|e| e.to_string())?.path();
            if p.is_dir() {
                if p.file_name().is_some_and(|name| {
                    name == "generated"
                        || name == "target"
                        || name == "node_modules"
                        || name == ".git"
                }) {
                    continue;
                }
                walk(root, &p, files)?;
            } else if p
                .extension()
                .is_some_and(|x| x == "rs" || x == "json" || x == "sha256")
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
    let mut files = vec![];
    walk(root, &root.join("packages/game-engine/crates"), &mut files)?;
    walk(root, &root.join("packages/game-server/src"), &mut files)?;
    walk(root, &root.join("packages/game-server/tests"), &mut files)?;
    walk(
        root,
        &root.join("packages/game-server/examples"),
        &mut files,
    )?;
    walk(
        root,
        &root.join("packages/game-engine/tests/fixtures"),
        &mut files,
    )?;
    for file in [
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
        "packages/game-server/Cargo.toml",
        "packages/game-server/build.rs",
        "packages/game-server/tools/client-artifacts.ts",
        "packages/game-server/tools/presentation.schema.json",
    ] {
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
    let catalog = mtg_compiler::catalog::embedded_catalog("base").map_err(|e| e.to_string())?;
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
    let rustc = format!("{}\n", BUILD.rustc);
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
    let (trace_runs, trace_report) = trace_runs(&catalog, &trace_games)?;
    // Trace evidence and reports own their data; release completed family games.
    drop(trace_games);
    drop(combat_games);
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
        bundle_version: 21,
        build_id: BUILD.id.into(),
        catalog,
        config: prepared.config().clone(),
        profile_digest: prepared.profile_digest().into(),
        archive_manifest: fixtures::MANIFEST.into(),
        records: mtg_compiler::embedded_catalog_records("base").map_err(|e| e.to_string())?,
        initial_snapshot,
        announcement_snapshot,
        journal: game.replay_journal(),
        final_snapshot: game.snapshot()?,
        source_files: source_files()?,
        rustc,
        build_profile: BUILD.profile.into(),
        opt_level: BUILD.opt_level.into(),
        debug: BUILD.debug.into(),
        debug_assertions: BUILD.debug_assertions.into(),
        target: BUILD.target.into(),
        rustflags: BUILD.rustflags.into(),
        query_sidecar,
    };
    let report = serde_json::json!({"status":"scenario_verified","combat":combat_report,"turns":turns_report,"departure":departure_report,"response":response_report,"detached_response":detached_report,"mana_casting":mana_casting_report,"mana_undo":mana_undo_report,"mana_keep":mana_keep_report,"trace_equivalence":trace_report,"build_id":BUILD.id,"catalog_digest":prepared.catalog_digest(),"profile_digest":prepared.profile_digest(),
        "state_sha256":game.state_digest()?,"accepted_inputs":game.journal().len(),"events":game.events(),"restore_fork_replay_equal":true,
        "player_observations": (0..4).map(|v|observation(&game,PlayerId(v))).collect::<Result<Vec<_>,_>>()?,
        "search_context":search_context(&game,PlayerId(0),"separate-sampling-seed".into())?,
        "retained_branch_observations":retained_branch_observations,
        "timing_observations":{"four_equivalent_runs_ns":elapsed,"priority_query_mean_ns":query_ns.to_string(),"clone_and_dispose_mean_ns":fork_ns.to_string(),"representative_commander_benchmark":false}});
    Ok((bundle, report))
}
fn verify_source_catalog(catalog: &Catalog, records: &[String]) -> Result<(), String> {
    let expected = mtg_compiler::embedded_catalog_records("base").map_err(|e| e.to_string())?;
    if records != expected {
        return Err("retained exact source fixtures differ from this build".into());
    }
    let rebuilt = mtg_compiler::embedded_catalog("base").map_err(|e| e.to_string())?;
    if *catalog != rebuilt {
        return Err("retained source admission coverage mismatch".into());
    }
    Ok(())
}
fn verify_native_source_closure(b: &Bundle) -> Result<(), String> {
    verify_native_source_bytes(
        &b.source_files,
        &b.build_id,
        [
            &b.rustc,
            &b.build_profile,
            &b.target,
            &b.rustflags,
            &b.opt_level,
            &b.debug,
            &b.debug_assertions,
        ],
    )
}
fn verify_native_source_bytes(
    files: &[SourceFile],
    build_id: &str,
    metadata: [&str; 7],
) -> Result<(), String> {
    let rustc = format!("{}\n", BUILD.rustc);
    if metadata
        != [
            rustc.as_str(),
            BUILD.profile,
            BUILD.target,
            BUILD.rustflags,
            BUILD.opt_level,
            BUILD.debug,
            BUILD.debug_assertions,
        ]
    {
        return Err("retained toolchain metadata differs from this executable".into());
    }
    for file in files {
        // The build identity uses NUL separators. Disallow embedded separators
        // and noncanonical paths before interpreting this retained file table.
        if file.path.contains(['\0', '\\', ':'])
            || file.contents.contains('\0')
            || file
                .path
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
        {
            return Err("unsafe native source path or embedded identity separator".into());
        }
    }
    if !files.windows(2).all(|w| w[0].path < w[1].path) {
        return Err("retained source paths must be unique and ordered".into());
    }
    let mut source_bytes = Vec::new();
    for file in files {
        if sha256(file.contents.as_bytes()) != file.sha256 {
            return Err("retained source hash mismatch".into());
        }
        // Validate every retained file hash above, while code identity is
        // independent of card data and hosting sources. Their artifact/catalog
        // hashes remain part of the separately verified evidence.
        if mtg_contracts::engine_build_input(&file.path) {
            source_bytes.extend_from_slice(file.path.as_bytes());
            source_bytes.push(0);
            source_bytes.extend_from_slice(file.contents.as_bytes());
            source_bytes.push(0);
        }
    }
    for (index, value) in metadata.iter().enumerate() {
        if index > 0 {
            source_bytes.push(0);
        }
        source_bytes.extend_from_slice(value.as_bytes());
    }
    if sha256(&source_bytes) != build_id {
        return Err(
            "retained source/toolchain closure does not reproduce the build identity".into(),
        );
    }
    Ok(())
}

fn verify_bundle(json: &str) -> Result<String, String> {
    let b: Bundle = strict_read(json)?;
    if b.bundle_version != 21 || b.build_id != BUILD.id {
        return Err("unsupported bundle version/build".into());
    }
    if b.archive_manifest != fixtures::MANIFEST {
        return Err("archive manifest differs from this build".into());
    }
    verify_native_source_closure(&b)?;
    verify_source_catalog(&b.catalog, &b.records)?;
    if b.trace_runs.len() != 12 * 3 {
        return Err("missing trace-mode evidence".into());
    }
    eprintln!(
        "verifying {} retained trace-mode executions",
        b.trace_runs.len()
    );
    // Bind the digest to its exact group reference and initialize it only after
    // the mode checks and expected digest succeed below.
    let mut reference_cache: Option<(Game, Option<String>)> = None;
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
            _ => return Err("unexpected trace-mode group".into()),
        };
        if run.mode != required_mode
            || &run.config != required_config
            || &run.journal != required_journal
        {
            return Err("trace-mode coverage/configuration mismatch".into());
        }
        let catalog = &b.catalog;
        let p = Prepared::admit(catalog.clone(), run.config.clone())?;
        let expected = Game::restore(p.clone(), &run.final_snapshot)?;
        if index % 3 == 0 {
            reference_cache = Some((Game::restore(p.clone(), reference_snapshot)?, None));
        }
        let (reference, reference_digest) = reference_cache
            .as_mut()
            .ok_or("missing verified trace reference")?;
        let reference = &*reference;
        let replay = Game::replay_with_trace(p, &run.journal, run.mode)?;
        if reference.trace().mode != TraceMode::Detailed
            || expected.trace().mode != run.mode
            || {
                let expected_digest = expected.state_digest()?;
                let reference_digest = match reference_digest {
                    Some(digest) => digest,
                    slot @ None => slot.insert(reference.state_digest()?),
                };
                expected_digest != *reference_digest || expected_digest != replay.state_digest()?
            }
            || expected.trace() != replay.trace()
            || (run.mode != TraceMode::Off
                && expected.trace().verification != reference.trace().verification)
        {
            return Err("retained trace-mode equivalence failed".into());
        }
    }
    eprintln!("verifying retained scenario recovery checkpoints");
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
    mtg_core::install_build_identity(BUILD)?;
    let args: Vec<String> = std::env::args().skip(1).collect();
    if matches!(
        args.first().map(String::as_str),
        Some("smoke" | "smoke-replay")
    ) {
        return smoke::cli(&args).map_err(Into::into);
    }
    match args.first().map(String::as_str) {
        Some("deck-inspect" | "dynamic-witness" | "dynamic-replay") => {
            return bulk_input::cli(&args).map_err(Into::into);
        }
        Some("catalog") => {
            let report = match args.get(1).map(String::as_str) {
                Some("coverage") => {
                    let cli = coverage_cli::parse(&args[2..])?;
                    let only = cli.selection.as_ref().map(coverage_cli::Selection::load).transpose()?;
                    serde_json::to_value(mtg_compiler::coverage::coverage_selected(&cli.archive, &cli.out, only.as_ref())?)?
                }
                Some("build-ir") if args.len() == 5 && args[3] == "--out" => {
                    serde_json::to_value(mtg_compiler::bulk::build_ir(Path::new(&args[2]), Path::new(&args[4]), BUILD.id)?)?
                }
                Some("build") if args.len() >= 3 => {
                    let mut write = false;
                    let mut json_errors = false;
                    for flag in &args[3..] {
                        match flag.as_str() {
                            "--write" if !write => write = true,
                            "--json-errors" if !json_errors => json_errors = true,
                            _ => return Err(format!("invalid or duplicate catalog build flag {flag:?}").into()),
                        }
                    }
                    match mtg_compiler::generate_catalog(Path::new(&args[2]), write) {
                        Ok(report) => report,
                        Err(error) if json_errors => {
                            eprintln!("{}", canonical(&error)?);
                            std::process::exit(1);
                        }
                        Err(error) => return Err(error.into_legacy_build_error().into()),
                    }
                }
                Some("verify") if args.len() == 4 => {
                    mtg_compiler::verify_archive(Path::new(&args[2]), Path::new(&args[3]))?
                }
                Some("extract") if args.len() >= 5 => {
                    mtg_compiler::extract_records(Path::new(&args[2]), Path::new(&args[3]), &args[4..])?
                }
                _ => return Err("usage: catalog coverage ARCHIVE [--out DIR] [--only-oracle-ids FILE | --only-deck-snapshot FILE] | catalog build-ir ARCHIVE --out NEW_DIR | catalog build MANIFEST [--write] [--json-errors] | catalog extract ARCHIVE PROVENANCE EXACT_NAME_OR_ORACLE_ID... | catalog verify ARCHIVE PROVENANCE".into()),
            };
            println!("{}", canonical(&report)?);
        }
        Some("native-source-manifest") if args.len() == 1 => {
            println!("{}", canonical(&current_source_manifest()?)?);
        }
        Some("search-benchmark") if args.len() == 3 && args[2] == "--quiescent" => {
            let catalog =
                mtg_compiler::catalog::embedded_catalog("match").map_err(|e| e.to_string())?;
            println!(
                "{}",
                canonical(&search_cli::benchmark(&catalog, Path::new(&args[1]))?)?
            );
        }
        Some("search-diagnostic" | "search-cancel-diagnostic") if args.len() == 2 => {
            let catalog =
                mtg_compiler::catalog::embedded_catalog("match").map_err(|e| e.to_string())?;
            println!(
                "{}",
                canonical(&search_cli::diagnostic(
                    &catalog,
                    args[1].parse()?,
                    args[0] == "search-cancel-diagnostic"
                )?)?
            );
        }
        Some("search-replay") if args.len() == 2 => {
            let catalog =
                mtg_compiler::catalog::embedded_catalog("match").map_err(|e| e.to_string())?;
            println!(
                "{}",
                canonical(&search_cli::replay(
                    &catalog,
                    &fs::read_to_string(&args[1])?
                )?)?
            );
        }
        Some("search-roots") if args.len() == 1 => {
            let catalog =
                mtg_compiler::catalog::embedded_catalog("match").map_err(|e| e.to_string())?;
            let mut reports = vec![];
            for index in 0..search_roots::ROOT_COUNT {
                let fixture = search_roots::prepare(&catalog, index)?;
                let observation = observation(&fixture.game, fixture.case.viewer)?;
                let capture = search_roots::capture(&fixture.game, fixture.case.viewer)?;
                reports.push(serde_json::json!({
                    "case": fixture.case,
                    "land_played_at_capture": fixture.land_played_at_capture,
                    "observation": observation,
                    "capture_id": capture.context_id(),
                    "known_bottom": capture.data().known_bottom,
                    "known_hands": capture.data().known_hands,
                    "permitted_history_views": capture.data().context.permitted_history.len(),
                    "journal_sha256": mtg_contracts::digest(&fixture.game.replay_journal())?,
                    "state_sha256": fixture.game.state_digest()?,
                }));
            }
            println!(
                "{}",
                canonical(&serde_json::json!({
                    "status": "trusted_fixture_report",
                    "workload": search_roots::WORKLOAD,
                    "build_id": BUILD.id,
                    "roots": reports,
                }))?
            );
        }
        Some("match-draw-failure") if args.len() == 1 => {
            println!(
                "{}",
                canonical(&match_games::draw_failure(
                    &mtg_compiler::catalog::embedded_catalog("match").map_err(|e| e.to_string())?
                )?)?
            );
        }
        Some("bench-pods") => match_games::bench_pods::cli(&args[1..])?,
        Some("match-batch") if args.len() == 3 => {
            let profile = mtg_core::match_profile(&args[1]).ok_or("unknown match profile")?;
            let manifest_name = Path::new(&profile.catalog_manifest)
                .file_stem()
                .and_then(|n| n.to_str())
                .ok_or("catalog manifest name")?;
            let catalog = mtg_compiler::catalog::embedded_catalog(manifest_name)?;
            println!(
                "{}",
                canonical(&match_games::batch(
                    &catalog,
                    &args[1],
                    Path::new(&args[2])
                )?)?
            );
        }
        Some("match-replay") if args.len() == 3 => {
            let profile = mtg_core::match_profile(&args[1]).ok_or("unknown match profile")?;
            let manifest_name = Path::new(&profile.catalog_manifest)
                .file_stem()
                .and_then(|n| n.to_str())
                .ok_or("catalog manifest name")?;
            let catalog = mtg_compiler::catalog::embedded_catalog(manifest_name)?;
            let e: match_games::Evidence = strict_read(&fs::read_to_string(&args[2])?)?;
            if e.config.profile != args[1] {
                return Err("replay profile mismatch".into());
            }
            match_games::verify(&catalog, &e)?;
            println!(
                "{}",
                canonical(
                    &serde_json::json!({"status":"match_replay_verified","profile":args[1],"state_sha256":e.final_state_sha256})
                )?
            );
        }
        Some("match-replay-live") if args.len() == 3 => {
            println!(
                "{}",
                canonical(&match_games::replay_live(
                    mtg_core::match_profile(&args[1])
                        .ok_or("unknown match profile")?
                        .catalog(),
                    &args[1],
                    Path::new(&args[2])
                )?)?
            );
        }

        Some("match-game") if args.len() == 3 => {
            let index: u32 = args[2].parse()?;
            let config = mtg_core::MatchConfig::for_profile(
                &args[1],
                match_games::seed_for(&args[1], index),
                PlayerId((index % 4) as u8),
            )?;
            let evidence = match_games::run(
                mtg_core::match_profile(&args[1])
                    .ok_or("unknown match profile")?
                    .catalog(),
                config,
                match_games::Policy::Develop,
                std::array::from_fn(|p| ((index as usize + p) % 3) as u8),
                1024,
            )?;
            println!("{}", canonical(&evidence)?);
        }
        Some("bundle-metadata") if args.len() == 2 => {
            let metadata = read_bundle_metadata(BufReader::new(fs::File::open(&args[1])?))?;
            println!("{}", canonical(&metadata)?);
        }
        Some("combat-games") if args.len() == 1 || (args.len() == 2 && args[1] == "--recover") => {
            let (_, _, report) = combat_games::suite(
                &mtg_compiler::catalog::embedded_catalog("base").map_err(|e| e.to_string())?,
                args.len() == 2,
            )?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Some("schema") if args.len() == 1 => println!(
            "{}",
            serde_json::to_string_pretty(&mtg_contracts::catalog_schema())?
        ),
        Some("observe") if args.len() == 2 => {
            let viewer = PlayerId(args[1].parse()?);
            let game = Game::new(Prepared::admit(
                mtg_compiler::catalog::embedded_catalog("base").map_err(|e| e.to_string())?,
                ScenarioConfig::land_return(),
            )?);
            println!("{}", canonical(&observation(&game, viewer)?)?);
        }
        Some("context") if args.len() == 3 => {
            let viewer = PlayerId(args[1].parse()?);
            let game = Game::new(Prepared::admit(
                mtg_compiler::catalog::embedded_catalog("base").map_err(|e| e.to_string())?,
                ScenarioConfig::land_return(),
            )?);
            println!(
                "{}",
                canonical(&search_context(&game, viewer, args[2].clone())?)?
            );
        }
        Some("demo") if args.len() == 1 || (args.len() == 3 && args[1] == "--bundle") => {
            let (bundle, report) = demo()?;
            let bytes = canonical(&bundle)?;
            drop(bundle);
            verify_bundle(&bytes)?;
            if args.len() == 3 {
                fs::create_dir_all(&args[2])?;
                let path = Path::new(&args[2]).join(format!("{}.json", sha256(bytes.as_bytes())));
                fs::write(&path, &bytes)?;
                eprintln!("replay bundle: {}", path.display());
            }
            drop(bytes);
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
                "usage: mtg-sim bench-pods PROFILE [OPTIONS]|combat-games [--recover]|bundle-metadata FILE|schema|catalog build MANIFEST|report|observe PLAYER|context PLAYER SEED|demo [--bundle DIR]|verify-bundle FILE"
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
            "bundle_version":21,"build_id":"historical-build",
            "source_files":[{"path":"Cargo.toml","sha256":"source-digest","contents":"[workspace]\n"}],
            "rustflags":"","opt_level":"3","debug":"false","debug_assertions":"false",
            "target":"x86_64-unknown-linux-gnu","catalog":mtg_compiler::catalog::embedded_catalog("base").unwrap(),
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
        bad["bundle_version"] = 18.into();
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
    fn source_coverage_binds_reviewed_definitions_to_retained_records() {
        let catalog = mtg_compiler::embedded_catalog("base").unwrap();
        let records = mtg_compiler::embedded_catalog_records("base").unwrap();
        verify_source_catalog(&catalog, &records).unwrap();
        assert!(verify_source_catalog(&catalog, &records[1..]).is_err());
        let mut forged = catalog;
        forged.definitions.pop();
        assert!(verify_source_catalog(&forged, &records).is_err());
    }
    #[test]
    fn native_source_collection_excludes_nested_generated_and_build_artifacts() {
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("mtg-native-source-{}-{nonce}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let _cleanup = Cleanup(root.clone());
        let real = [
            "Cargo.toml",
            "Cargo.lock",
            "rust-toolchain.toml",
            "packages/game-engine/crates/example/Cargo.toml",
            "packages/game-engine/crates/example/src/logic.rs",
            "packages/game-engine/tests/fixtures/positive.json",
            "packages/game-engine/tests/fixtures/catalogs/positive.sha256",
            "packages/game-server/Cargo.toml",
            "packages/game-server/build.rs",
            "packages/game-server/tools/client-artifacts.ts",
            "packages/game-server/tools/presentation.schema.json",
            "packages/game-server/src/authority.rs",
            "packages/game-server/tests/protocol.rs",
            "packages/game-server/examples/action_fixture.rs",
        ];
        for path in real {
            let path = root.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "retained source").unwrap();
        }
        for path in [
            "packages/game-engine/crates/example/generated/capture/game.json",
            "packages/game-engine/crates/example/target/deps/test.rs",
            "packages/game-engine/crates/example/node_modules/pkg/Cargo.toml",
            "packages/game-engine/crates/example/.git/hidden.rs",
            "packages/game-engine/tests/fixtures/generated/ignored.json",
            "packages/game-server/sdk/should-not-be-native.json",
            "packages/game-server/tools/not-a-build-input.ts",
            "packages/game-server/src/generated/ignored.rs",
            "packages/game-server/tests/target/ignored.json",
        ] {
            let path = root.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, [0xff]).unwrap();
        }
        let files = source_files_from(&root).unwrap();
        let mut expected = real.to_vec();
        expected.sort_unstable();
        assert_eq!(
            files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(),
            expected
        );
        assert!(
            files
                .iter()
                .all(|f| f.sha256 == sha256(f.contents.as_bytes()))
        );
    }
    #[test]
    fn recorded_debug_assertions_match_the_target_configuration() {
        assert_eq!(BUILD.debug_assertions, cfg!(debug_assertions).to_string());
    }
    #[test]
    fn code_identity_excludes_catalog_data_but_keeps_source_and_hash_checks() {
        let metadata = [
            BUILD.rustc.to_owned() + "\n",
            BUILD.profile.into(),
            BUILD.target.into(),
            BUILD.rustflags.into(),
            BUILD.opt_level.into(),
            BUILD.debug.into(),
            BUILD.debug_assertions.into(),
        ];
        let metadata = metadata.each_ref().map(String::as_str);
        let original = source_files().unwrap();
        verify_native_source_bytes(&original, BUILD.id, metadata).unwrap();
        let mut files = original.clone();
        let data = files
            .iter_mut()
            .find(|f| f.path.ends_with("fixtures/provenance.json"))
            .unwrap();
        data.contents.push(' ');
        // Separate per-file integrity remains enforced for data too.
        assert!(
            verify_native_source_bytes(&files, BUILD.id, metadata)
                .unwrap_err()
                .contains("source hash mismatch")
        );
        let data = files
            .iter_mut()
            .find(|f| f.path.ends_with("fixtures/provenance.json"))
            .unwrap();
        data.sha256 = sha256(data.contents.as_bytes());
        verify_native_source_bytes(&files, BUILD.id, metadata).unwrap();
        let host = files
            .iter_mut()
            .find(|f| f.path.starts_with("packages/game-server/src/"))
            .unwrap();
        host.contents.push(' ');
        host.sha256 = sha256(host.contents.as_bytes());
        verify_native_source_bytes(&files, BUILD.id, metadata).unwrap();
        // Test-only engine sources never enter an executable or its identity.
        for suffix in [
            "_tests.rs",
            "/tests.rs",
            "crates/core/tests/combat_adversarial.rs",
            "crates/core/tests/search_work/work/optimization_tests/search_reference_spool.rs",
            "crates/core/src/testkit.rs",
            "crates/witness/Cargo.toml",
            "crates/witness/src/lib.rs",
            "crates/witness/tests/cascade/main.rs",
        ] {
            let test = files
                .iter_mut()
                .find(|f| {
                    f.path.starts_with("packages/game-engine/crates/") && f.path.ends_with(suffix)
                })
                .unwrap();
            test.contents.push(' ');
            test.sha256 = sha256(test.contents.as_bytes());
            verify_native_source_bytes(&files, BUILD.id, metadata).unwrap();
        }
        let code = files
            .iter_mut()
            .find(|f| f.path.ends_with("crates/core/src/engine.rs"))
            .unwrap();
        code.contents.push(' ');
        code.sha256 = sha256(code.contents.as_bytes());
        assert!(
            verify_native_source_bytes(&files, BUILD.id, metadata)
                .unwrap_err()
                .contains("does not reproduce the build identity")
        );
    }
    #[test]
    fn retained_native_identity_rejects_separator_absorption_and_unsafe_paths() {
        fn metadata(rustc: &str) -> [&str; 7] {
            [
                rustc,
                BUILD.profile,
                BUILD.target,
                BUILD.rustflags,
                BUILD.opt_level,
                BUILD.debug,
                BUILD.debug_assertions,
            ]
        }
        let mut files = source_files().unwrap();
        let mut rustc = format!("{}\n", BUILD.rustc);
        verify_native_source_bytes(&files, BUILD.id, metadata(&rustc)).unwrap();
        let original_path = files[0].path.clone();
        for unsafe_path in [
            "/escape",
            "a/../escape",
            "a//b",
            "a/./b",
            "C:/escape",
            "a\\b",
            "a\0b",
        ] {
            files[0].path = unsafe_path.into();
            assert!(
                verify_native_source_bytes(&files, BUILD.id, metadata(&rustc))
                    .unwrap_err()
                    .contains("unsafe native source path")
            );
        }
        files[0].path = original_path;
        verify_native_source_bytes(&files, BUILD.id, metadata(&rustc)).unwrap();
        // A final file can otherwise be absorbed into rustc while preserving
        // the same separator stream. Metadata must match compiled constants.
        let last = files.pop().unwrap();
        let original_rustc = std::mem::take(&mut rustc);
        rustc = format!("{}\0{}\0{}", last.path, last.contents, original_rustc);
        assert!(
            verify_native_source_bytes(&files, BUILD.id, metadata(&rustc))
                .unwrap_err()
                .contains("toolchain metadata")
        );
        rustc = original_rustc;
        files.push(last);
        verify_native_source_bytes(&files, BUILD.id, metadata(&rustc)).unwrap();
        // Merging adjacent records preserves the old delimiter byte stream.
        // Explicit separator guards must reject the now-ambiguous file table.
        let second = files.remove(1);
        let first = &mut files[0];
        first.contents.push('\0');
        first.contents.push_str(&second.path);
        first.contents.push('\0');
        first.contents.push_str(&second.contents);
        first.sha256 = sha256(first.contents.as_bytes());
        assert!(verify_native_source_bytes(&files, BUILD.id, metadata(&rustc)).is_err());
        files.clear();
        assert!(verify_native_source_bytes(&files, BUILD.id, metadata(&rustc)).is_err());
    }
    #[test]
    fn bundle_contains_replayable_source_to_runtime_evidence() {
        let (bundle, report) = demo().unwrap();
        let final_snapshot: serde_json::Value =
            serde_json::from_str(&bundle.final_snapshot).unwrap();
        assert_eq!(
            final_snapshot["snapshot_version"],
            mtg_core::formats::SNAPSHOT_VERSION
        );
        let json = canonical(&bundle).unwrap();
        assert_eq!(
            verify_bundle(&json).unwrap(),
            report["state_sha256"].as_str().unwrap()
        );
        assert!(
            verify_bundle(&json.replace("\"bundle_version\":21", "\"bundle_version\":22")).is_err()
        );
        let mut changed = bundle.records.clone();
        changed[0].push(' ');
        assert!(verify_source_catalog(&bundle.catalog, &changed).is_err());
    }
}
