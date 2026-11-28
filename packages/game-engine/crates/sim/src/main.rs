use mtg_contracts::{canonical, strict_read};
use mtg_core::PlayerId;
use mtg_sim::{play, policy::Policy};
use std::{fs, path::Path, time::Instant};
/// This executable's engine build identity: build.rs hashes the production
/// engine source closure (`crates/build_identity.rs`). `main` installs it into
/// mtg-core before any engine use. Unit tests leave core at
/// `BuildIdentity::TEST`, and nothing here compares that test identity with
/// this compiled one.
pub(crate) const BUILD: mtg_core::BuildIdentity = mtg_core::compiled_build_identity!();

mod coverage_cli;
mod pod_cli;

const USAGE: &str = "usage: mtg-sim COMMAND
  catalog coverage ARCHIVE [--out DIR] [--only-oracle-ids FILE | --only-deck-snapshot FILE]
  catalog build-ir ARCHIVE --out NEW_DIR
  catalog reference-trace ARCHIVE --out FILE
  catalog identity --inputs FROZEN_MANIFEST --cache DIR
  catalog compile --inputs FROZEN_MANIFEST --cache DIR --out NEW_DIR [--fresh-reader ROOT | --reader-if-missing ROOT]
  catalog stages [PARENT]
  catalog stages --remove STAGE
  catalog build MANIFEST [--write]
  catalog extract [--promotion] ARCHIVE PROVENANCE EXACT_NAME_OR_ORACLE_ID...
  catalog verify [--promotion] ARCHIVE PROVENANCE
  schema
  deck-inspect --artifact DIR --manifest-sha SHA --decks JSON
  match-game PROFILE INDEX --journal-dir ABSOLUTE_DIR
  match-replay PROFILE EVIDENCE
  match-replay-live PROFILE JOURNAL
  bench-pair --help
  bench-pods (PROFILE | --bulk-artifact DIR --manifest-sha SHA --pod-decks JSON) [OPTIONS]";

fn embedded_profile_catalog(name: &str) -> Result<mtg_contracts::Catalog, String> {
    let profile = mtg_core::match_profile(name).ok_or("unknown match profile")?;
    let manifest_name = Path::new(&profile.catalog_manifest)
        .file_stem()
        .and_then(|n| n.to_str())
        .ok_or("catalog manifest name")?;
    mtg_compiler::catalog::embedded_catalog(manifest_name).map_err(|e| e.to_string())
}

/// `catalog identity|compile`: exact named options, each once. Usage errors
/// exit 2; located compile failures print `KIND: REASON` and exit 1.
fn frozen_compile(args: &[String]) -> ! {
    let compile = args[0] == "compile";
    let names: &[&str] = if compile {
        &["--inputs", "--cache", "--out"]
    } else {
        &["--inputs", "--cache"]
    };
    let mut options = std::collections::BTreeMap::new();
    let parsed = args[1..].chunks(2).all(|pair| {
        matches!(pair, [name, value] if (names.contains(&name.as_str())
            || compile && matches!(name.as_str(), "--fresh-reader" | "--reader-if-missing"))
            && options.insert(name.as_str(), Path::new(value)).is_none())
    }) && names.iter().all(|name| options.contains_key(name));
    use mtg_compiler::compile::ReaderMode;
    let reader = match (
        options.get("--fresh-reader"),
        options.get("--reader-if-missing"),
    ) {
        (Some(root), None) => Some(ReaderMode::Fresh(root)),
        (None, Some(root)) => Some(ReaderMode::Auto(root)),
        (None, None) => Some(ReaderMode::Cached),
        (Some(_), Some(_)) => None,
    };
    let Some(reader) = reader.filter(|_| parsed) else {
        eprintln!("{USAGE}");
        std::process::exit(2);
    };
    let (inputs, cache) = (options["--inputs"], options["--cache"]);
    let result = if compile {
        mtg_compiler::compile::compile_with_reader(
            inputs,
            cache,
            options["--out"],
            BUILD.id,
            reader,
        )
        .map(|report| canonical(&report))
    } else {
        mtg_compiler::compile::identity(inputs, cache, BUILD.id).map(|report| canonical(&report))
    };
    match result {
        Ok(Ok(report)) => {
            println!("{report}");
            std::process::exit(0)
        }
        Ok(Err(error)) => eprintln!("compiler: {error}"),
        Err(error) => eprintln!("{error}"),
    }
    std::process::exit(1)
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
        Some("reference-trace") if args.len() == 4 && args[2] == "--out" => {
            mtg_compiler::coverage::reference_trace(Path::new(&args[1]), Path::new(&args[3]))?
        }
        Some("build-ir") if args.len() == 4 && args[2] == "--out" => serde_json::to_value(
            mtg_compiler::bulk::build_ir(Path::new(&args[1]), Path::new(&args[3]), BUILD.id)?,
        )?,
        Some("stages") => match &args[1..] {
            [] => serde_json::to_value(mtg_compiler::bulk::list_stages(Path::new("."))?)?,
            [parent] if !parent.starts_with("--") => {
                serde_json::to_value(mtg_compiler::bulk::list_stages(Path::new(parent))?)?
            }
            [flag, path] if flag == "--remove" => {
                mtg_compiler::bulk::remove_stage(Path::new(path))
                    .map_err(|e| format!("cannot remove stage {path}: {e}"))?;
                serde_json::json!({"removed": path})
            }
            _ => return Err(USAGE.into()),
        },
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
        Some("verify" | "extract") => {
            let (promotion, operands) = match &args[1..] {
                [flag, rest @ ..] if flag == "--promotion" => (true, rest),
                rest => (false, rest),
            };
            match (args[0].as_str(), operands) {
                ("verify", [archive, provenance]) => mtg_compiler::catalog::verify_fixture_archive(
                    Path::new(archive),
                    Path::new(provenance),
                    promotion,
                )?,
                ("extract", [archive, provenance, selectors @ ..]) if !selectors.is_empty() => {
                    mtg_compiler::catalog::extract_fixture_records(
                        Path::new(archive),
                        Path::new(provenance),
                        selectors,
                        promotion,
                    )?
                }
                _ => return Err(USAGE.into()),
            }
        }
        _ => return Err(USAGE.into()),
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    mtg_core::install_build_identity(BUILD)?;
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("catalog")
            if matches!(
                args.get(1).map(String::as_str),
                Some("identity" | "compile")
            ) =>
        {
            frozen_compile(&args[1..])
        }
        Some("catalog") => println!("{}", canonical(&catalog(&args[1..])?)?),
        Some("schema") if args.len() == 1 => println!(
            "{}",
            serde_json::to_string_pretty(&mtg_contracts::catalog_schema())?
        ),
        Some("deck-inspect") => pod_cli::inspect(BUILD, &args)?,
        Some("bench-pair") => pod_cli::pair(BUILD, &args[1..])?,
        Some("bench-pods") => pod_cli::bench(BUILD, &args[1..])?,
        Some("match-game") if args.len() == 5 && args[3] == "--journal-dir" => {
            let index: u32 = args[2].parse()?;
            let config = mtg_core::MatchConfig::for_profile(
                &args[1],
                play::seed_for(&args[1], index),
                PlayerId((index % 4) as u8),
            )?;
            let start = Instant::now();
            let mut journal: Option<LiveJournal> = None;
            let mut evidence = play::run(
                BUILD,
                mtg_core::match_profile(&args[1])
                    .ok_or("unknown match profile")?
                    .catalog(),
                config,
                Policy::Develop,
                std::array::from_fn(|p| ((index as usize + p) % 3) as u8),
                1024,
                None,
                |record| match &mut journal {
                    Some(journal) => journal.append(record),
                    None => {
                        journal = Some(LiveJournal::new(Path::new(&args[4]), record)?);
                        Ok(())
                    }
                },
            )?;
            journal.ok_or("missing live journal")?.file.sync_data()?;
            evidence.elapsed_millis = start.elapsed().as_millis();
            println!("{}", canonical(&evidence)?);
        }
        Some("match-replay") if args.len() == 3 => {
            let catalog = embedded_profile_catalog(&args[1])?;
            let e: play::Evidence = strict_read(&fs::read_to_string(&args[2])?)?;
            if e.config.profile != args[1] {
                return Err("replay profile mismatch".into());
            }
            play::verify(&catalog, &e)?;
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
                canonical(&play::replay_live(
                    BUILD,
                    &args[1],
                    mtg_core::match_profile(&args[1])
                        .ok_or("unknown match profile")?
                        .catalog(),
                    std::io::BufReader::new(fs::File::open(&args[2])?)
                )?)?
            );
        }
        _ => return Err(USAGE.into()),
    }
    Ok(())
}

struct LiveJournal {
    file: fs::File,
}
impl LiveJournal {
    /// Live journals are the only record of an interrupted match. The caller
    /// names their directory explicitly; nothing defaults into a checkout.
    fn new(parent: &Path, header: &serde_json::Value) -> Result<Self, String> {
        if !parent.is_absolute() {
            return Err("live journal directory must be an absolute path".into());
        }
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        for index in 0..1000 {
            let path = parent.join(format!("run-{}-{index}.jsonl", std::process::id()));
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(file) => {
                    let mut out = Self { file };
                    out.append(header)?;
                    eprintln!("streaming accepted inputs to {}", path.display());
                    return Ok(out);
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.to_string()),
            }
        }
        Err("no fresh live journal path".into())
    }
    fn append(&mut self, value: &serde_json::Value) -> Result<(), String> {
        use std::io::Write;
        let mut line = serde_json::to_vec(value).map_err(|e| e.to_string())?;
        line.push(b'\n');
        self.file.write_all(&line).map_err(|e| e.to_string())
    }
}
