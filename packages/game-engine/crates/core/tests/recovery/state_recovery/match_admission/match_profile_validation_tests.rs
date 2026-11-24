//! Typed validation of every embedded match profile. Formerly ran in build.rs,
//! which made mtg-contracts a build dependency compiled a second time. The
//! runtime loader is the same `MatchProfile::load` these rows exercise.
use super::*;
use std::{fs, path::Path};

fn fixtures() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures"))
}

#[test]
fn every_profile_file_is_embedded_and_strictly_valid() {
    let mut files = fs::read_dir(fixtures().join("profiles"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect::<Vec<_>>();
    files.sort();
    let mut names = vec![];
    for path in &files {
        let raw = fs::read_to_string(path).unwrap();
        let profile: MatchProfile = mtg_contracts::strict_read(&raw)
            .unwrap_or_else(|e| panic!("invalid profile {}: {e}", path.display()));
        names.push(profile.name);
    }
    names.sort();
    // Forces every embedded `MatchProfile::load` (panics on any invalid row).
    let embedded = MATCH_PROFILES
        .iter()
        .map(|p| p.name.clone())
        .collect::<Vec<_>>();
    assert_eq!(embedded, names);
}

#[test]
fn embedded_profiles_select_only_legal_unbanned_archive_records() {
    for profile in MATCH_PROFILES.iter() {
        let manifest_path = fixtures().join("profiles").join(&profile.catalog_manifest);
        let manifest: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
        let banned: serde_json::Value = serde_json::from_str(&profile.banned_list_raw).unwrap();
        // Derive the old pool review from each authentic archive source record.
        for record in manifest["records"].as_array().unwrap() {
            let card: serde_json::Value = serde_json::from_str(
                &fs::read_to_string(
                    manifest_path
                        .parent()
                        .unwrap()
                        .join(record.as_str().unwrap()),
                )
                .unwrap(),
            )
            .unwrap();
            let source = &card["source"];
            let id = card["oracleIdentity"].as_str().unwrap();
            assert_eq!(
                source["legalities"]["commander"], "legal",
                "{}: Commander legality: {id}",
                profile.name
            );
            assert!(
                !source["type_line"]
                    .as_str()
                    .unwrap()
                    .split_whitespace()
                    .any(|x| x == "Conspiracy"),
                "{id}"
            );
            assert!(
                !source["oracle_text"]
                    .as_str()
                    .unwrap_or("")
                    .split(|c: char| !c.is_alphabetic())
                    .any(|x| x.eq_ignore_ascii_case("ante")),
                "{id}"
            );
            assert!(
                !banned["named_bans"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .chain(banned["offensive_policy"]["records"].as_array().unwrap())
                    .any(|b| b["oracle_id"] == id),
                "{}: banned card: {id}",
                profile.name
            );
        }
    }
}

#[test]
fn embedded_profiles_fit_shared_stack_and_trigger_capacities() {
    use super::super::capacities::{STACK_LIMIT, TOKEN_CAPACITY};
    for profile in MATCH_PROFILES.iter() {
        let catalog = profile.catalog();
        assert_eq!(
            profile.catalog_sha256,
            mtg_contracts::digest(catalog).unwrap()
        );
        assert!(
            catalog
                .definitions
                .iter()
                .filter(|d| d.characteristics.basic)
                .all(|d| d.triggered.is_empty()),
            "{}: basic triggered sources invalidate singleton sizing",
            profile.name
        );
        let cast_wave = catalog
            .cast_trigger_fanout_bound(400)
            .expect("cast trigger sizing overflow");
        assert!(
            cast_wave < STACK_LIMIT / 2,
            "{}: cast trigger bound {cast_wave} exceeds shared stack policy",
            profile.name
        );
        let sources = 4 * catalog
            .definitions
            .iter()
            .map(|d| d.triggered.len())
            .sum::<usize>();
        let wave = (400 + TOKEN_CAPACITY as usize)
            .checked_mul(sources)
            .expect("trigger sizing overflow");
        assert!(
            wave < STACK_LIMIT / 2,
            "{}: simultaneous trigger bound {wave} exceeds shared stack policy; revisit capacities",
            profile.name
        );
    }
}
