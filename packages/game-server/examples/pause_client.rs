//! Offline test/operator control through the existing durable worker clock API.
//! The registry lock excludes a running supervisor; never modifies SQLite itself.
use std::{path::Path, time::Instant};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: pause_client REGISTRY MATCH_ID (host must be stopped)".into());
    }
    let registry = mtg_server::registry::MatchRegistry::open(Path::new(&args[0]))?;
    let item = registry
        .load_all_matches()?
        .into_iter()
        .find(|m| m.match_id == args[1])
        .ok_or("unknown match")?;
    let mut worker = mtg_server::worker::Worker::recover(
        &item.worker_directory,
        item.catalog,
        &item.match_id,
        Instant::now(),
    )?;
    worker.clock(mtg_server::worker::ClockControl::Pause, Instant::now())?;
    Ok(())
}
