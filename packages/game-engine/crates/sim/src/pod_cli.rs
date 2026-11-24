use mtg_contracts::canonical;
use mtg_core::{BuildIdentity, DynamicPodTemplate};
use mtg_sim::pods::{Options, PodSource, USAGE, bulk_input, run};
use std::{path::Path, time::Instant};
pub fn bench(build: BuildIdentity, args: &[String]) -> Result<(), String> {
    if args == ["--help"] {
        println!("{USAGE}");
        return Ok(());
    }
    let (report, verified) = if args.first().is_some_and(|arg| arg == "--bulk-artifact") {
        let [
            _,
            directory,
            manifest_flag,
            pin,
            decks_flag,
            decks_path,
            options @ ..,
        ] = args
        else {
            return Err(USAGE.into());
        };
        if manifest_flag != "--manifest-sha" || decks_flag != "--pod-decks" {
            return Err(USAGE.into());
        }
        let options_args = options;
        let options = Options::parse(options)?;
        options.ensure_new_analytics_out()?;
        let decks = bulk_input::read_pod_decks(Path::new(decks_path))?;
        let start = Instant::now();
        let artifact = bulk_input::load(build, Path::new(directory), pin)?;
        let load_ns = start.elapsed().as_nanos();
        let start = Instant::now();
        let template = DynamicPodTemplate::admit(&artifact, &decks)
            .map_err(|error| serde_json::to_string(&error).unwrap_or_else(|_| error.to_string()))?;
        // Fail before any pod runs: analytics needs the headless-qualified tier.
        if options_requested(options_args, "--analytics-out") {
            mtg_sim::analytics::require_headless_qualified(
                template.catalog().catalog(),
                decks.iter().flatten().map(|row| row.oracle_id.as_str()),
            )?;
        }
        let projection_ns = start.elapsed().as_nanos();
        let source = PodSource::Dynamic {
            template: &template,
            decks: &decks,
        };
        let (mut report, verified) = run(build, source, &options)?;
        report["bulk"] = serde_json::json!({
            "manifest_sha256": artifact.manifest_sha256(),
            "catalog_sha256": artifact.catalog().digest(),
            "policy_sha256": template.policy_digest(),
            "load_ns": load_ns,
            "projection_ns": projection_ns,
        });
        (report, verified)
    } else {
        let name = args.first().ok_or(USAGE)?;
        let options = Options::parse(&args[1..])?;
        options.ensure_new_analytics_out()?;
        let profile =
            mtg_core::match_profile(name).ok_or_else(|| format!("unknown match profile {name}"))?;
        run(build, PodSource::Embedded(profile), &options)?
    };
    publish(report, verified)
}

fn publish(report: serde_json::Value, verified: bool) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(&report).map_err(|error| error.to_string())?
    );
    if !verified {
        return Err("bench-pods sampled replay verification failed".into());
    }
    Ok(())
}

pub fn inspect(build: BuildIdentity, args: &[String]) -> Result<(), String> {
    let [command, flag1, directory, flag2, pin, flag3, file] = args else {
        return Err("usage: deck-inspect --artifact DIR --manifest-sha SHA --decks JSON".into());
    };
    if command != "deck-inspect"
        || flag1 != "--artifact"
        || flag2 != "--manifest-sha"
        || flag3 != "--decks"
    {
        return Err("invalid bulk artifact command flags".into());
    }
    let run = || -> Result<(), String> {
        let start = Instant::now();
        let artifact = bulk_input::load(build, Path::new(directory), pin)?;
        let load_micros = start.elapsed().as_micros();
        let start = Instant::now();
        let report = bulk_input::inspect_snapshot(&artifact, Path::new(file))?;
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

/// True when a bench-pods flag was given, read with `Options::parse`'s
/// grammar: `--rotate-seats` stands alone; every other flag takes a value.
fn options_requested(options: &[String], wanted: &str) -> bool {
    let mut args = options.iter();
    while let Some(flag) = args.next() {
        if flag == wanted {
            return true;
        }
        if flag != "--rotate-seats" {
            args.next();
        }
    }
    false
}
