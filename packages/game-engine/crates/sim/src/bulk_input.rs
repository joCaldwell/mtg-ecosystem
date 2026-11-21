//! Offline artifact loading, batch inspection and bounded replay evidence.
use crate::BUILD_ID;
use mtg_contracts::{
    JsonByteProof,
    bulk_catalog::{BulkArtifact, BulkManifest, MANIFEST_MAX_BYTES, is_sha256},
    canonical, digest, strict_read_seekable_verified,
};
use mtg_core::{
    Advance, Budget, CaptureOutcome, DeckInspection, DeckRole, DynamicAdmissionIdentity,
    DynamicDeckEntry, DynamicMatchConfig, Game, MatchAdmissionError, MatchConfig, PlayerId,
    Prepared, ReplayJournal, Request, Response, SearchRejection, TraceMode, TurnStep,
    admit_dynamic_pod, inspect_dynamic_deck, match_profile,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    os::unix::fs::OpenOptionsExt,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

const SNAPSHOT_MAX: u64 = 1024 * 1024;
const DECK_MAX: usize = 64 * 1024;
const REPLAY_MAX: u64 = 4 * 1024 * 1024;
const OPENING_MAX_DECISIONS: usize = 100;
const FOUNDATIONS: &str = "commander-foundations-v1";

fn read_file<T: DeserializeOwned>(path: &Path, limit: u64) -> Result<(T, JsonByteProof), String> {
    let metadata = fs::symlink_metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if !metadata.is_file() || metadata.len() > limit {
        return Err(format!(
            "invalid regular input file or byte limit: {}",
            path.display()
        ));
    }
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    strict_read_seekable_verified(&mut file, limit).map_err(|e| format!("{}: {e}", path.display()))
}

pub(super) fn load(directory: &Path, pin: &str) -> Result<BulkArtifact, String> {
    if !is_sha256(pin) || !fs::symlink_metadata(directory).is_ok_and(|m| m.is_dir()) {
        return Err(
            "expected a regular artifact directory and a lowercase manifest SHA-256".into(),
        );
    }
    let manifest: (BulkManifest, _) =
        read_file(&directory.join("manifest.json"), MANIFEST_MAX_BYTES)?;
    if manifest.1.sha256 != pin {
        return Err("bulk manifest pin mismatch".into());
    }
    manifest.0.validate(BUILD_ID)?;
    let catalog = read_file(&directory.join("catalog.json"), manifest.0.catalog.bytes)?;
    let index = read_file(&directory.join("index.json"), manifest.0.index.bytes)?;
    BulkArtifact::verify(manifest, pin, BUILD_ID, catalog, index)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PodDecks {
    decks: [Vec<DynamicDeckEntry>; 4],
}

pub(super) fn read_pod_decks(path: &Path) -> Result<[Vec<DynamicDeckEntry>; 4], String> {
    let (pod, _): (PodDecks, _) = read_file(path, 4 * DECK_MAX as u64)?;
    Ok(pod.decks)
}

// Accept the existing frozen production snapshot shape. Only exact identity,
// quantity and role participate in admission. Display metadata never supplies facts.
#[derive(Deserialize)]
struct DeckSnapshot {
    #[serde(rename = "deckCount")]
    deck_count: usize,
    decks: Vec<SnapshotDeck>,
}
#[derive(Deserialize)]
struct SnapshotDeck {
    id: u64,
    name: String,
    cards: Vec<SnapshotCard>,
}
#[derive(Deserialize)]
struct SnapshotCard {
    oracle_id: String,
    quantity: u32,
    is_commander: bool,
}
#[derive(Serialize)]
struct InspectionRow {
    id: u64,
    name: String,
    status: &'static str,
    #[serde(flatten)]
    inspection: DeckInspection,
}
#[derive(Serialize)]
struct InspectionBatch {
    format_version: u32,
    bulk_manifest_sha256: String,
    catalog_digest: String,
    decks: Vec<InspectionRow>,
}
fn inspect_snapshot(artifact: &BulkArtifact, path: &Path) -> Result<InspectionBatch, String> {
    let (mut snapshot, _): (DeckSnapshot, _) = read_file(path, SNAPSHOT_MAX)?;
    snapshot.decks.sort_unstable_by_key(|d| d.id);
    if snapshot.decks.len() != snapshot.deck_count
        || !(1..=64).contains(&snapshot.deck_count)
        || snapshot.decks.windows(2).any(|r| r[0].id == r[1].id)
    {
        return Err(
            "deck snapshot count is inconsistent, exceeds the bound, or repeats an ID".into(),
        );
    }
    let cached = artifact.catalog().shared_catalog();
    let mut decks = Vec::with_capacity(snapshot.decks.len());
    for deck in snapshot.decks {
        let rows = deck
            .cards
            .into_iter()
            .map(|card| DynamicDeckEntry {
                oracle_id: card.oracle_id,
                quantity: card.quantity,
                role: if card.is_commander {
                    DeckRole::Commander
                } else {
                    DeckRole::Main
                },
            })
            .collect::<Vec<_>>();
        if deck.name.len() > 256 || canonical(&rows)?.len() > DECK_MAX {
            return Err(format!(
                "deck inspection request exceeds its byte limit for {}",
                deck.id
            ));
        }
        let inspection = inspect_dynamic_deck(artifact, &rows)?;
        let entry = InspectionRow {
            id: deck.id,
            name: deck.name,
            status: if inspection.admissible() {
                "admitted"
            } else {
                "rejected"
            },
            inspection,
        };
        if canonical(&entry)?.len() > DECK_MAX {
            return Err(format!(
                "deck inspection response exceeds its byte limit for {}",
                deck.id
            ));
        }
        decks.push(entry);
    }
    if !Arc::ptr_eq(&cached, &artifact.catalog().shared_catalog()) {
        return Err("inspection did not retain the loaded catalog identity".into());
    }
    Ok(InspectionBatch {
        format_version: 2,
        bulk_manifest_sha256: artifact.manifest_sha256().into(),
        catalog_digest: artifact.catalog().digest().into(),
        decks,
    })
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReplayEvidence {
    format_version: u32,
    runtime_build_id: String,
    identity: DynamicAdmissionIdentity,
    config: DynamicMatchConfig,
    journal_sha256: String,
    journal: ReplayJournal,
    final_state_sha256: String,
}

fn replay(artifact: &BulkArtifact, evidence: &ReplayEvidence) -> Result<String, String> {
    if evidence.format_version != 1
        || evidence.runtime_build_id != BUILD_ID
        || evidence.identity.bulk_manifest_sha256 != artifact.manifest_sha256()
        || !(1..=OPENING_MAX_DECISIONS).contains(&evidence.journal.decisions.len())
        || !evidence.journal.concessions.is_empty()
        || evidence.journal_sha256 != digest(&evidence.journal)?
    {
        return Err("dynamic replay envelope or journal identity mismatch".into());
    }
    let pod = admit_dynamic_pod(artifact, &evidence.config).map_err(|e| e.to_string())?;
    if pod.identity != evidence.identity || pod.normalized_config() != &evidence.config {
        return Err("dynamic replay admission, policy or projection identity mismatch".into());
    }
    let state = Game::replay(pod.prepared, &evidence.journal)?.state_digest()?;
    if state != evidence.final_state_sha256 {
        return Err("dynamic replay state digest mismatch".into());
    }
    Ok(state)
}

fn opening(prepared: Prepared, starter: PlayerId) -> Result<Game, String> {
    let mut game = Game::with_trace(prepared, TraceMode::Off);
    for _ in 0..OPENING_MAX_DECISIONS {
        let Advance::AwaitingDecision(request) = game.advance(Budget::new(100_000)) else {
            return Err("Foundations opening did not reach an ordinary decision".into());
        };
        if matches!(request, Request::Priority { player, .. } if player == starter)
            && game.turn().step == TurnStep::PrecombatMain
        {
            return Ok(game);
        }
        let response = match &request {
            Request::Mulligan { .. } => Response::Mulligan { take: false },
            Request::Priority { .. } => Response::Pass,
            _ => return Err("unexpected decision in Foundations opening".into()),
        };
        game.respond(request.player(), request.decision_id(), response)?;
    }
    Err("Foundations opening exceeded its decision bound".into())
}

fn foundations_witness(artifact: &BulkArtifact) -> Result<ReplayEvidence, String> {
    let starter = PlayerId(0);
    let embedded =
        MatchConfig::for_profile(FOUNDATIONS, [0; 32], starter).map_err(|e| e.to_string())?;
    let profile = match_profile(FOUNDATIONS).ok_or("Foundations profile unavailable")?;
    let frozen = profile.catalog();
    let mut config = DynamicMatchConfig::from_match_config(&embedded);
    let first = admit_dynamic_pod(artifact, &config).map_err(|e| e.to_string())?;
    for row in config.decks.iter().flatten() {
        let prior = frozen
            .definition_by_oracle_id(&row.oracle_id)
            .ok_or("missing frozen definition")?;
        let actual = artifact
            .catalog()
            .catalog()
            .definition_by_oracle_id(&row.oracle_id)
            .ok_or("missing bulk Foundations definition")?;
        let mut actual = actual.clone();
        if actual.provenance.ast_sha256 != prior.provenance.ast_sha256 {
            return Err(format!("Foundations AST changed for {}", row.oracle_id));
        }
        actual.provenance = prior.provenance.clone();
        if actual != *prior {
            return Err(format!("Foundations IR changed for {}", row.oracle_id));
        }
    }

    for deck in &mut config.decks {
        deck.reverse();
    }
    let second = admit_dynamic_pod(artifact, &config).map_err(|e| e.to_string())?;
    let config = first.normalized_config().clone();
    if first.identity != second.identity || &config != second.normalized_config() {
        return Err("dynamic admission identity changed with deck row order".into());
    }
    if !matches!(
        Prepared::admit_pod(
            frozen.clone(),
            first
                .prepared
                .match_config()
                .ok_or("missing config")?
                .clone()
        ),
        Err(MatchAdmissionError::UnsupportedProfile)
    ) {
        return Err("dynamic identity bypassed reviewed profile lookup".into());
    }
    let baseline = opening(
        Prepared::admit_pod(frozen.clone(), embedded).map_err(|e| e.to_string())?,
        starter,
    )?;
    let game = opening(first.prepared, starter)?;
    let reordered = opening(second.prepared, starter)?;
    let journal = game.replay_journal();
    let state = game.state_digest()?;
    if journal.decisions.is_empty()
        || journal != reordered.replay_journal()
        || state != reordered.state_digest()?
    {
        return Err("dynamic opening changed with deck row order".into());
    }
    if !matches!(
        baseline.capture_search(starter, Budget::unlimited()),
        CaptureOutcome::Complete(_)
    ) || !matches!(
        game.capture_search(starter, Budget::unlimited()),
        CaptureOutcome::Rejected(SearchRejection::UnsupportedRoot(reason))
            if reason == "root needs zero marks and owner-default control"
    ) {
        return Err("Foundations search boundary witness failed".into());
    }
    let evidence = ReplayEvidence {
        format_version: 1,
        runtime_build_id: BUILD_ID.into(),
        identity: first.identity,
        config,
        journal_sha256: digest(&journal)?,
        journal,
        final_state_sha256: state,
    };
    replay(artifact, &evidence)?;
    Ok(evidence)
}

fn write_evidence(path: &Path, evidence: &ReplayEvidence) -> Result<(), String> {
    let bytes = canonical(evidence)?;
    if bytes.len() as u64 > REPLAY_MAX {
        return Err("dynamic replay output exceeds its byte limit".into());
    }
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let parent = path.parent().ok_or("replay output has no parent")?;
    let (stage, mut output) = loop {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let stage = parent.join(format!(".dynamic-replay-{}-{n}", std::process::id()));
        if stage == path {
            continue;
        }
        match OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&stage)
        {
            Ok(file) => break (stage, file),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    };
    let result = output
        .write_all(bytes.as_bytes())
        .and_then(|()| output.sync_all())
        .and_then(|()| fs::hard_link(&stage, path));
    fs::remove_file(stage).map_err(|e| e.to_string())?;
    result.map_err(|e| e.to_string())
}

pub(super) fn cli(args: &[String]) -> Result<(), String> {
    let [command, flag1, directory, flag2, pin, flag3, file] = args else {
        return Err("usage: deck-inspect --artifact DIR --manifest-sha SHA --decks JSON | dynamic-witness --artifact DIR --manifest-sha SHA --out NEW_JSON | dynamic-replay --artifact DIR --manifest-sha SHA --journal JSON".into());
    };
    if flag1 != "--artifact"
        || flag2 != "--manifest-sha"
        || !matches!(
            (command.as_str(), flag3.as_str()),
            ("deck-inspect", "--decks")
                | ("dynamic-witness", "--out")
                | ("dynamic-replay", "--journal")
        )
    {
        return Err("invalid bulk artifact command flags".into());
    }
    let run = || -> Result<(), String> {
        let start = Instant::now();
        let artifact = load(Path::new(directory), pin)?;
        let load_micros = start.elapsed().as_micros();
        let path = Path::new(file);
        match command.as_str() {
            "deck-inspect" => {
                let start = Instant::now();
                let report = inspect_snapshot(&artifact, path)?;
                let batch_inspection_micros = start.elapsed().as_micros();
                println!("{}", canonical(&report)?);
                eprintln!(
                    "{}",
                    serde_json::json!({
                        "catalog_loads": 1, "load_micros": load_micros,
                        "batch_inspection_micros": batch_inspection_micros,
                        "decks": report.decks.len(),
                    })
                );
            }
            "dynamic-witness" => {
                let evidence = foundations_witness(&artifact)?;
                write_evidence(path, &evidence)?;
                println!(
                    "{}",
                    serde_json::json!({
                        "status":"replay_verified", "identity":evidence.identity,
                        "decisions":evidence.journal.decisions.len(),
                        "state_sha256":evidence.final_state_sha256,
                        "reordered_decks_verified":true, "dynamic_reviewed_profile_rejected":true,
                        "dynamic_search_rejected":true,
                    })
                );
            }
            "dynamic-replay" => {
                let (evidence, _) = read_file(path, REPLAY_MAX)?;
                let state = replay(&artifact, &evidence)?;
                println!(
                    "{}",
                    serde_json::json!({"status":"replay_verified","state_sha256":state})
                );
            }
            _ => unreachable!("command already checked"),
        }
        Ok(())
    };
    if let Err(detail) = run() {
        println!(
            "{}",
            serde_json::json!({"format_version":2,"status":"unavailable","detail":detail})
        );
        return Err(detail);
    }
    Ok(())
}
