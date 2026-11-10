//! A deliberately fixed, tiny developer probe. This is trusted diagnostic data,
//! not a player API, full game, universal replay debugger, or acceptance gate.
use mtg_compiler::{CompileResult, compile, fixtures};
use mtg_contracts::{Catalog, Color, Quantity, canonical, digest, sha256, strict_read};
use mtg_core::{
    Advance, BUILD_DEBUG_ASSERTIONS, BUILD_ID, BUILD_OPT_LEVEL, BUILD_PROFILE, BUILD_RUSTC,
    BUILD_TARGET, Budget, Game, Observation, PlayerId, PlayerRequest, PlayerResponse, Prepared,
    QueryRejection, QueryResult, QueryValue, ReplayJournal, ScenarioConfig, TraceMode,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs::OpenOptions,
    io::{Read, Write},
    path::Path,
    time::Instant,
};

const MAX_BYTES: usize = 65_536;
const WORK: u64 = 1_024;
const SOURCE_NAMES: [&str; 5] = ["forest", "grizzly-bears", "island", "unsummon", "isamaru"];
const SCENARIO: &str = "land-return-mana-priority-smoke-v1";
const CLASSIFICATION: &str = "trusted_host_diagnostic_not_player_payload";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Checkpoint {
    after_inputs: usize,
    priority: Option<PlayerId>,
    island_tapped: bool,
    blue_mana: Quantity,
    state_sha256: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Artifact {
    format_version: u32,
    classification: String,
    scenario: String,
    seed: Option<String>,
    identity: Value,
    journal: ReplayJournal,
    checkpoints: Vec<Checkpoint>,
    failure: Option<String>,
}
fn short(text: &str) -> String {
    let mut out: String = text.chars().take(240).collect();
    if text.chars().count() > 240 {
        out.push('…');
    }
    out
}
fn identity() -> Value {
    json!({
        "core_build_id": BUILD_ID, "rustc": BUILD_RUSTC, "profile": BUILD_PROFILE,
        "opt_level": BUILD_OPT_LEVEL, "debug_assertions": BUILD_DEBUG_ASSERTIONS,
        "target": BUILD_TARGET,
        "smoke_source_sha256": sha256(include_str!("smoke.rs").as_bytes()),
        "cli_source_sha256": sha256(include_str!("main.rs").as_bytes()),
        "manifest_sha256": sha256(fixtures::MANIFEST.as_bytes()),
        "records": fixtures::RECORDS.iter().filter(|(name, _)| SOURCE_NAMES.contains(name)).map(|(name, raw)|
            json!({"fixture":name,"sha256":sha256(raw.as_bytes())})).collect::<Vec<_>>()
    })
}
fn prepare() -> Result<Prepared, String> {
    // Admission still requires the five-definition historical base pool, even
    // though this scenario has only three physical cards. Compile all five from
    // their exact archive-bound sources; never substitute the expected IR.
    let mut catalog = Catalog {
        schema_version: 3,
        rules_edition: mtg_contracts::RULES_EDITION.into(),
        rules_sha256: mtg_contracts::RULES_SHA256.into(),
        definitions: vec![],
    };
    for (name, raw) in fixtures::RECORDS
        .iter()
        .filter(|(name, _)| SOURCE_NAMES.contains(name))
    {
        match compile(fixtures::MANIFEST, raw) {
            CompileResult::Lowered { definition } => catalog.definitions.push(*definition),
            other => {
                return Err(format!(
                    "source compilation {name}: {}",
                    short(&format!("{other:?}"))
                ));
            }
        }
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    Prepared::admit(catalog, ScenarioConfig::land_return())
}
fn observe(game: &Game) -> Result<Observation, String> {
    match game.observe(PlayerId(0), Budget::new(WORK)) {
        QueryResult::Complete(QueryValue::Observation(value)) => Ok(value),
        _ => Err("public observation did not complete within smoke budget".into()),
    }
}
fn advance(game: &mut Game) -> Result<(), String> {
    match game.advance(Budget::new(WORK)) {
        Advance::AwaitingDecision(_) => Ok(()),
        Advance::Fault(reason) => Err(format!("advance fault: {}", short(&reason))),
        _ => Err("advance did not reach the expected decision within smoke budget".into()),
    }
}
fn submit(game: &mut Game, response: PlayerResponse) -> Result<(), String> {
    let observation = observe(game)?;
    let Some(PlayerRequest::Priority { ref token, .. }) = observation.decision else {
        return Err("expected player 0 public priority request".into());
    };
    match game.check_player_response(PlayerId(0), token.clone(), response, Budget::new(WORK)) {
        QueryResult::Complete(QueryValue::Validated(receipt)) => game.submit(receipt),
        _ => Err("public response validation did not complete successfully".into()),
    }
}
fn checkpoint(game: &Game) -> Result<Checkpoint, String> {
    let observation = observe(game)?;
    let island_id = &game.config().cards[0].oracle_id;
    let island = observation
        .view
        .objects
        .iter()
        .find(|o| &o.oracle_id == island_id)
        .ok_or("missing visible Island")?;
    let player = observation
        .view
        .players
        .iter()
        .find(|p| p.player == PlayerId(0))
        .ok_or("missing visible player 0")?;
    Ok(Checkpoint {
        after_inputs: game.replay_journal().decisions.len(),
        priority: observation.view.priority,
        island_tapped: island.tapped,
        blue_mana: player.mana.get(&Color::U).cloned().unwrap_or_default(),
        state_sha256: game.state_digest()?,
    })
}
fn assert_outcome(point: &Checkpoint, index: usize) -> Result<(), String> {
    let expected = json!({"priority":if index == 2 {1} else {0},
        "island_tapped":index > 0,"blue_mana":if index > 0 {"1"} else {"0"}});
    let actual = json!({"priority":point.priority,"island_tapped":point.island_tapped,
        "blue_mana":point.blue_mana});
    if let Some(difference) = first_difference(&expected, &actual, "") {
        return Err(format!(
            "checkpoint {index} outcome: {}",
            canonical(&difference)?
        ));
    }
    Ok(())
}
fn execute(artifact: &mut Artifact) -> Result<(), String> {
    let prepared = prepare()?;
    artifact.identity["catalog_sha256"] = prepared.catalog_digest().into();
    artifact.identity["config_sha256"] = digest(prepared.config())?.into();
    let mut game = Game::with_trace(prepared.clone(), TraceMode::Off);
    advance(&mut game)?;
    let initial = checkpoint(&game)?;
    artifact.checkpoints.push(initial.clone());
    assert_outcome(&initial, 0)?;
    let observation = observe(&game)?;
    let token = observation
        .decision
        .as_ref()
        .ok_or("missing decision")?
        .token()
        .clone();
    let land = observation
        .view
        .objects
        .iter()
        .find(|o| o.oracle_id == game.config().cards[0].oracle_id)
        .ok_or("missing Island handle")?
        .handle
        .clone();
    for (index, response) in [
        PlayerResponse::ActivateMana { object: land },
        PlayerResponse::Pass,
    ]
    .into_iter()
    .enumerate()
    {
        submit(&mut game, response)?;
        artifact.journal = game.replay_journal();
        advance(&mut game)?;
        let point = checkpoint(&game)?;
        artifact.checkpoints.push(point.clone());
        assert_outcome(&point, index + 1)?;
        // A rejected stale public token must neither mutate state nor enter the journal.
        let before = game.state_digest()?;
        if !matches!(
            game.check_player_response(
                PlayerId(0),
                token.clone(),
                PlayerResponse::Pass,
                Budget::new(WORK)
            ),
            QueryResult::Rejected(QueryRejection::InvalidResponse)
        ) || game.state_digest()? != before
        {
            return Err("stale public decision token was accepted or mutated state".into());
        }
        // Only two fixed, locally generated inputs enter the existing replay API.
        let replay = Game::replay_with_trace(prepared.clone(), &artifact.journal, TraceMode::Off)?;
        if replay.state_digest()? != point.state_sha256 {
            return Err(format!(
                "replay after input {index} differs at state_sha256; internal field unknown"
            ));
        }
    }
    Ok(())
}
fn run() -> Artifact {
    let mut artifact = Artifact {
        format_version: 1,
        classification: CLASSIFICATION.into(),
        scenario: SCENARIO.into(),
        seed: None,
        identity: identity(),
        journal: ReplayJournal {
            decisions: vec![],
            concessions: vec![],
        },
        checkpoints: vec![],
        failure: None,
    };
    if let Err(error) = execute(&mut artifact) {
        artifact.failure = Some(short(&error));
    }
    artifact
}
// Stable traversal returns one JSON pointer, never dumps entire states or traces.
fn first_difference(expected: &Value, actual: &Value, path: &str) -> Option<Value> {
    if expected == actual {
        return None;
    }
    match (expected, actual) {
        (Value::Object(a), Value::Object(b)) => {
            let keys: std::collections::BTreeSet<_> = a.keys().chain(b.keys()).collect();
            for key in keys {
                let pointer = format!("{path}/{}", key.replace('~', "~0").replace('/', "~1"));
                if let (Some(x), Some(y)) = (a.get(key), b.get(key)) {
                    if let Some(d) = first_difference(x, y, &pointer) {
                        return Some(d);
                    }
                } else {
                    return Some(
                        json!({"field":short(&pointer),"field_truncated":pointer.chars().count() > 240,"expected_present":a.contains_key(key),"actual_present":b.contains_key(key)}),
                    );
                }
            }
        }
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => {
            for (index, (x, y)) in a.iter().zip(b).enumerate() {
                if let Some(d) = first_difference(x, y, &format!("{path}/{index}")) {
                    return Some(d);
                }
            }
        }
        _ => {}
    }
    Some(
        json!({"field":short(path),"field_truncated":path.chars().count() > 240,"expected":short(&expected.to_string()),"actual":short(&actual.to_string())}),
    )
}
fn compare(saved: &Artifact, current: &Artifact) -> Result<(), Value> {
    // Identities first: comparing different builds is not a replay correctness claim.
    for (field, expected, actual) in [
        ("format_version", json!(1), json!(saved.format_version)),
        (
            "classification",
            json!(CLASSIFICATION),
            json!(saved.classification),
        ),
        ("scenario", json!(SCENARIO), json!(saved.scenario)),
        ("seed", Value::Null, json!(saved.seed)),
        ("identity", current.identity.clone(), saved.identity.clone()),
        ("journal", json!(current.journal), json!(saved.journal)),
        (
            "checkpoints",
            json!(current.checkpoints),
            json!(saved.checkpoints),
        ),
        ("failure", json!(current.failure), json!(saved.failure)),
    ] {
        if let Some(difference) = first_difference(&expected, &actual, &format!("/{field}")) {
            return Err(difference);
        }
    }
    Ok(())
}
fn read(path: &Path) -> Result<Artifact, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| short(&e.to_string()))?
        .take((MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| short(&e.to_string()))?;
    if bytes.len() > MAX_BYTES {
        return Err("artifact exceeds 65536-byte limit".into());
    }
    strict_read(std::str::from_utf8(&bytes).map_err(|e| short(&e.to_string()))?)
        .map_err(|e| short(&e))
}
fn write_new(path: &Path, artifact: &Artifact) -> Result<(), String> {
    let bytes = canonical(artifact)?;
    if bytes.len() > MAX_BYTES {
        return Err("artifact exceeds 65536-byte limit".into());
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| short(&e.to_string()))?;
    file.write_all(bytes.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|e| short(&e.to_string()))
}
fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
fn reproduce(args: &[String]) -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let mut command = format!(
        "cd {} && ",
        quote(cwd.to_str().ok_or("non-UTF8 working directory")?)
    );
    if let Ok(libraries) = std::env::var("LD_LIBRARY_PATH") {
        command += &format!("env LD_LIBRARY_PATH={} ", quote(&libraries));
    }
    command += &quote(exe.to_str().ok_or("non-UTF8 executable path")?);
    for arg in args {
        command += &format!(" {}", quote(arg));
    }
    if command.len() > 8192 {
        return Err("reproduction command exceeds 8192-byte limit".into());
    }
    Ok(command)
}
pub(super) fn cli(args: &[String]) -> Result<(), String> {
    let start = Instant::now();
    let mut report = json!({"scenario":SCENARIO,"seed":null,"classification":CLASSIFICATION,
        "locator_scope":"first recorded input/checkpoint field; digest-only differences do not locate internal state",
        "core_build_id":BUILD_ID});
    let result = (|| -> Result<(), String> {
        report["reproduce"] = reproduce(args)?.into();
        let (replay, path) = match args {
            [command] if command == "smoke" => (false, None),
            [command, flag, path] if command == "smoke" && flag == "--artifact" => {
                (false, Some(Path::new(path)))
            }
            [command, path] if command == "smoke-replay" => (true, Some(Path::new(path))),
            _ => {
                return Err(
                    "usage: mtg-sim smoke [--artifact NEW.json] | smoke-replay FILE".into(),
                );
            }
        };
        let saved = if replay {
            Some(read(path.unwrap())?)
        } else {
            None
        };
        let current = run();
        report["probe_failure"] = json!(current.failure);
        report["inputs"] = current.journal.decisions.len().into();
        report["checkpoints"] = current.checkpoints.len().into();
        report["final_state_sha256"] = current
            .checkpoints
            .last()
            .map(|p| p.state_sha256.clone())
            .into();
        if let Some(saved) = saved {
            if let Err(difference) = compare(&saved, &current) {
                report["first_difference"] = difference;
                return Err("saved smoke evidence differs from current fixed scenario".into());
            }
        } else if let Some(path) = path {
            write_new(path, &current)?;
        }
        if let Some(failure) = current.failure {
            return Err(failure);
        }
        Ok(())
    })();
    report["runtime_us"] = json!(start.elapsed().as_micros());
    report["status"] = if result.is_ok() { "passed" } else { "failed" }.into();
    if let Err(ref error) = result {
        report["error"] = short(error).into();
    }
    println!("{}", canonical(&report)?);
    result.map_err(|_| "smoke failed; see bounded JSON diagnostic".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn smoke_source_public_decisions_outcomes_and_replay() {
        let artifact = run();
        assert_eq!(artifact.failure, None);
        assert_eq!(artifact.journal.decisions.len(), 2);
        assert_eq!(artifact.checkpoints.len(), 3);
        assert!(canonical(&artifact).unwrap().len() < MAX_BYTES);
        assert!(compare(&artifact, &run()).is_ok());
    }
    #[test]
    fn smoke_reports_first_input_or_checkpoint_difference_and_build_mismatch() {
        let current = run();
        assert_eq!(current.failure, None);
        let mut saved = current.clone();
        saved.journal.decisions[1].decision_id += 1;
        assert_eq!(
            compare(&saved, &current).unwrap_err()["field"],
            "/journal/decisions/1/decision_id"
        );
        let mut saved = current.clone();
        saved.checkpoints[1].island_tapped = false;
        assert!(assert_outcome(&saved.checkpoints[1], 1).is_err());
        assert_eq!(
            compare(&saved, &current).unwrap_err()["field"],
            "/checkpoints/1/island_tapped"
        );
        let mut saved = current.clone();
        saved.identity["core_build_id"] = "another-build".into();
        assert_eq!(
            compare(&saved, &current).unwrap_err()["field"],
            "/identity/core_build_id"
        );
        let mut saved = current.clone();
        saved.failure = Some("retained failure".into());
        assert!(compare(&saved, &current).is_err());
    }
    #[test]
    fn smoke_artifacts_are_bounded_strict_and_never_overwrite() {
        let dir = std::env::temp_dir().join(format!(
            "mtg-smoke-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("evidence.json");
        let artifact = run();
        write_new(&path, &artifact).unwrap();
        let original = std::fs::read(&path).unwrap();
        assert!(write_new(&path, &artifact).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert!(compare(&read(&path).unwrap(), &artifact).is_ok());
        let oversized = dir.join("oversized.json");
        std::fs::write(&oversized, vec![b' '; MAX_BYTES + 1]).unwrap();
        assert!(read(&oversized).unwrap_err().contains("65536"));
        let malformed = dir.join("malformed.json");
        std::fs::write(&malformed, b"{\"format_version\":1").unwrap();
        assert!(read(&malformed).is_err());
        let mut unknown = serde_json::to_value(&artifact).unwrap();
        unknown["unexpected"] = true.into();
        std::fs::write(&malformed, canonical(&unknown).unwrap()).unwrap();
        assert!(read(&malformed).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
