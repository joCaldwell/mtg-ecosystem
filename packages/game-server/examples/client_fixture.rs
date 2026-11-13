//! Offline, source-backed permitted opening fixture. No credentials/private state
//! enter the export. Output and scratch directory must both be new.
use mtg_server::{
    protocol::PublicManifest,
    worker::{Worker, WorkerPhase},
};
use std::{fs::OpenOptions, io::Write, path::Path, time::Instant};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: client_fixture NEW_SCRATCH NEW_OUTPUT".into());
    }
    let catalog = mtg_contracts::Catalog::read(include_str!(
        "../../game-engine/tests/fixtures/expected-match-ir.json"
    ))?;
    let config = mtg_core::MatchConfig::for_profile(
        "commander-foundations-v1",
        [3; 32],
        mtg_core::PlayerId(0),
    )?;
    let manifest = PublicManifest::new("a1".into(), catalog.clone(), &config)?;
    let now = Instant::now();
    let mut worker = Worker::create(Path::new(&args[0]), catalog, config, "a1", now)?;
    for _ in 0..1000 {
        if !matches!(worker.host_state().phase, WorkerPhase::Running) {
            break;
        }
        worker.advance(Instant::now())?;
    }
    if !matches!(worker.host_state().phase, WorkerPhase::Waiting) {
        return Err("fixture did not reach opening decision".into());
    }
    // Seat zero only. Catalog is NOT included: frontend gets presentation separately.
    let resume = worker.resume(0)?;
    let fixture = serde_json::json!({
        "fixture_version":1,"scenario":"foundations-opening-seat-0",
        "catalog_fixture":"packages/game-engine/tests/fixtures/expected-match-ir.json",
        "profile":manifest.profile,"resume":resume
    });
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[1])?;
    output.write_all(mtg_contracts::canonical(&fixture)?.as_bytes())?;
    output.sync_all()?;
    Ok(())
}
