//! Explicit local M1 provisioning. All seats remain human-owned; no bot starts.
//! Generates private files only, never prints capabilities or match state.
use std::{path::Path, time::Instant};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: provision_client NEW_REGISTRY NEW_PRIVATE_EXPORT_DIR ORIGIN".into());
    }
    let root = Path::new(&args[0]);
    let output = Path::new(&args[1]);
    if root.exists() || output.exists() {
        return Err("provision paths must be new".into());
    }
    // Parent is explicitly prepared mode 0700 by the operator; registry enforces
    // ownership/mode/ancestor checks, as do export-seat and issued-file writers.
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new().mode(0o700).create(output)?;
    let catalog = mtg_contracts::Catalog::read(include_str!(
        "../../game-engine/tests/fixtures/expected-match-ir.json"
    ))?;
    let mut seed = [0; 32];
    getrandom::fill(&mut seed)?;
    let config = mtg_core::MatchConfig::for_profile(
        "commander-foundations-v1",
        seed,
        mtg_core::PlayerId(0),
    )?;
    let mut registry = mtg_server::registry::MatchRegistry::open(root)?;
    let issued = output.join("issued.json");
    registry.create(
        "a1",
        &catalog,
        &config,
        &issued,
        |directory, catalog, config| {
            let worker = mtg_server::worker::Worker::create(
                directory,
                catalog.clone(),
                config.clone(),
                "a1",
                Instant::now(),
            )?;
            drop(worker);
            Ok(())
        },
    )?;
    for seat in 0..4 {
        registry.export_seat(
            "a1",
            seat,
            &args[2],
            &issued,
            &output.join(format!("seat-{seat}.json")),
        )?;
    }
    Ok(())
}
