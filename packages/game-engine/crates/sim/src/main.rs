use mtg_contracts::{Catalog, canonical, sha256, strict_read};
use mtg_core::{
    Advance, Budget, Game, Observation, PlayerId, Prepared, QueryResult, QueryValue, ReplayJournal,
    SnapshotVerifier, TraceMode, TurnStep, Zone,
};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path, time::Instant};

/// This executable's engine build identity: build.rs hashes the production
/// engine source closure (`crates/build_identity.rs`). `main` installs it into
/// mtg-core before any engine use. Unit tests leave core at
/// `BuildIdentity::TEST`, and nothing here compares that test identity with
/// this compiled one.
pub(crate) const BUILD: mtg_core::BuildIdentity = mtg_core::compiled_build_identity!();

mod bulk_input;
mod coverage_cli;
mod deck_analytics;
mod match_games;
#[cfg(test)]
mod smoke;

const USAGE: &str = "usage: mtg-sim COMMAND
  catalog coverage ARCHIVE [--out DIR] [--only-oracle-ids FILE | --only-deck-snapshot FILE]
  catalog build-ir ARCHIVE --out NEW_DIR
  catalog build MANIFEST [--write]
  catalog extract ARCHIVE PROVENANCE EXACT_NAME_OR_ORACLE_ID...
  catalog verify ARCHIVE PROVENANCE
  schema
  deck-inspect --artifact DIR --manifest-sha SHA --decks JSON
  match-game PROFILE INDEX --journal-dir ABSOLUTE_DIR
  match-replay PROFILE EVIDENCE
  match-replay-live PROFILE JOURNAL
  bench-pods (PROFILE | --bulk-artifact DIR --manifest-sha SHA --pod-decks JSON) [OPTIONS]";

fn embedded_profile_catalog(name: &str) -> Result<mtg_contracts::Catalog, String> {
    let profile = mtg_core::match_profile(name).ok_or("unknown match profile")?;
    let manifest_name = Path::new(&profile.catalog_manifest)
        .file_stem()
        .and_then(|n| n.to_str())
        .ok_or("catalog manifest name")?;
    mtg_compiler::catalog::embedded_catalog(manifest_name).map_err(|e| e.to_string())
}

fn catalog(args: &[String]) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    Ok(match args.first().map(String::as_str) {
        Some("coverage") => {
            let cli = coverage_cli::parse(&args[1..])?;
            let only = cli
                .selection
                .as_ref()
                .map(coverage_cli::Selection::load)
                .transpose()?;
            serde_json::to_value(mtg_compiler::coverage::coverage_selected(
                &cli.archive,
                &cli.out,
                only.as_ref(),
            )?)?
        }
        Some("build-ir") if args.len() == 4 && args[2] == "--out" => serde_json::to_value(
            mtg_compiler::bulk::build_ir(Path::new(&args[1]), Path::new(&args[3]), BUILD.id)?,
        )?,
        Some("build") if matches!(args.len(), 2 | 3) => {
            let write = match args.get(2).map(String::as_str) {
                None => false,
                Some("--write") => true,
                Some(flag) => return Err(format!("invalid catalog build flag {flag:?}").into()),
            };
            match mtg_compiler::generate_catalog(Path::new(&args[1]), write) {
                Ok(report) => report,
                // The typed compiler diagnostic is the only error format.
                Err(error) => {
                    eprintln!("{}", canonical(&error)?);
                    std::process::exit(1);
                }
            }
        }
        Some("verify") if args.len() == 3 => {
            mtg_compiler::verify_archive(Path::new(&args[1]), Path::new(&args[2]))?
        }
        Some("extract") if args.len() >= 4 => {
            mtg_compiler::extract_records(Path::new(&args[1]), Path::new(&args[2]), &args[3..])?
        }
        _ => return Err(USAGE.into()),
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    mtg_core::install_build_identity(BUILD)?;
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("catalog") => println!("{}", canonical(&catalog(&args[1..])?)?),
        Some("schema") if args.len() == 1 => println!(
            "{}",
            serde_json::to_string_pretty(&mtg_contracts::catalog_schema())?
        ),
        Some("deck-inspect") => bulk_input::cli(&args)?,
        Some("bench-pods") => match_games::bench_pods::cli(&args[1..])?,
        Some("match-game") if args.len() == 5 && args[3] == "--journal-dir" => {
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
                Path::new(&args[4]),
            )?;
            println!("{}", canonical(&evidence)?);
        }
        Some("match-replay") if args.len() == 3 => {
            let catalog = embedded_profile_catalog(&args[1])?;
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
        _ => return Err(USAGE.into()),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recorded_debug_assertions_match_the_target_configuration() {
        assert_eq!(BUILD.debug_assertions, cfg!(debug_assertions).to_string());
    }
}
