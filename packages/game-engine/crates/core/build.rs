// Pure data embedding: reviewed catalog sizes/definition pins and match
// profiles. No engine types, no engine source identity: mtg-core never depends
// on mtg-contracts at build time, and the executable edge supplies the build
// identity (`crates/build_identity.rs`). Typed profile/catalog validation lives
// in `src/engine/match_profile_validation_tests.rs`.
use sha2::{Digest, Sha256};
use std::{
    env, fs,
    path::{Path, PathBuf},
};
// Record every data read, including missing inputs that diagnostics tolerate.
// Cargo recursively watches directories, so avoid watching the fixture root.
// https://doc.rust-lang.org/cargo/reference/build-scripts.html#rerun-if-changed
fn read(path: impl AsRef<Path>) -> std::io::Result<Vec<u8>> {
    println!("cargo:rerun-if-changed={}", path.as_ref().display());
    fs::read(path)
}
fn read_to_string(path: impl AsRef<Path>) -> std::io::Result<String> {
    println!("cargo:rerun-if-changed={}", path.as_ref().display());
    fs::read_to_string(path)
}
fn main() {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../../..");
    // Catalog digests/counts and scenario definition pins derive from reviewed
    // expected IR. The compiler freshness test checks it against source records.
    let data = root.join("packages/game-engine/tests/fixtures/catalogs");
    println!("cargo:rerun-if-changed={}", data.display());
    let mut sizes = String::from(
        "pub(super) fn reviewed_catalog_size(hash: &str) -> Option<usize> { const SIZES: &[(&str, usize)] = &[\n",
    );
    let mut manifests = fs::read_dir(&data)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect::<Vec<_>>();
    manifests.sort();
    for path in manifests {
        if !path.extension().is_some_and(|e| e == "json") {
            continue;
        }
        // Source diagnostics belong to the runtime catalog loader. A malformed
        // manifest/output must never prevent building the diagnostic CLI.
        let Some(manifest) = read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        else {
            continue;
        };
        let Some(expected) = manifest["expected_ir"].as_str() else {
            continue;
        };
        let Some(catalog) = read(data.join(expected))
            .ok()
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        else {
            continue;
        };
        let Some(rows) = catalog["definitions"].as_array() else {
            continue;
        };
        let canonical = serde_json::to_vec(&catalog).unwrap();
        let digest = format!("{:x}", Sha256::digest(canonical));
        sizes += &format!("({digest:?}, {}),\n", rows.len());
        for row in rows {
            let Some(id) = row["oracle_id"].as_str() else {
                continue;
            };
            let hash = format!("{:x}", Sha256::digest(serde_json::to_vec(row).unwrap()));
            println!(
                "cargo:rustc-env=MTG_DEFINITION_{}_{id}={hash}",
                path.file_stem().unwrap().to_str().unwrap()
            );
        }
    }
    sizes += "]; SIZES.iter().find(|(h, _)| *h == hash).map(|(_, n)| *n) }\n";
    fs::write(
        PathBuf::from(env::var("OUT_DIR").unwrap()).join("catalog_sizes.rs"),
        sizes,
    )
    .unwrap();
    // Profile files select catalogs, decks and admission policy. Generated code
    // contains data only and is excluded from the engine code identity.
    let fixtures = data.parent().unwrap();
    println!(
        "cargo:rerun-if-changed={}",
        fixtures.join("profiles").display()
    );
    let mut names = std::collections::BTreeSet::new();
    let mut paths = fs::read_dir(fixtures.join("profiles"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect::<Vec<_>>();
    paths.sort();
    // A new policy enters through a profile, whose directory is watched. Do
    // not discover unused policy files at the fixture root: that would require
    // recursively watching every unrelated fixture and make warm/cold output
    // disagree after adding an unused policy file.
    let mut banned_files = std::collections::BTreeMap::new();
    let mut profiles = vec![];
    for path in &paths {
        let raw = read_to_string(path).unwrap();
        // Untyped selection only: the typed strict loader (the same one runtime
        // uses) validates every embedded profile in match_profile_validation_tests.
        let profile: serde_json::Value = serde_json::from_str(&raw)
            .unwrap_or_else(|e| panic!("invalid profile JSON {}: {e}", path.display()));
        let name = profile["name"].as_str().expect("profile name").to_owned();
        assert!(
            names.insert(name.clone()),
            "duplicate profile name {name:?} in {}",
            path.display()
        );
        let selected = path.parent().unwrap().join(
            profile["commander_policy"]["banned_list"]
                .as_str()
                .expect("profile banned list"),
        );
        let canonical = fs::canonicalize(&selected).unwrap();
        assert!(
            canonical.parent() == Some(fs::canonicalize(fixtures).unwrap().as_path())
                && canonical
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("commander-banned-")
                && canonical.extension().is_some_and(|e| e == "json"),
            "profile {} selects a file outside the banned-list data: {}",
            path.display(),
            selected.display()
        );
        banned_files.insert(canonical.clone(), read_to_string(&selected).unwrap());
        let manifest_path = path.parent().unwrap().join(
            profile["catalog_manifest"]
                .as_str()
                .expect("profile catalog manifest"),
        );
        let manifest: serde_json::Value =
            serde_json::from_slice(&read(&manifest_path).unwrap()).unwrap();
        let base = manifest_path.parent().unwrap();
        let catalog_raw =
            read_to_string(base.join(manifest["expected_ir"].as_str().unwrap())).unwrap();
        let ir: serde_json::Value = serde_json::from_str(&catalog_raw).unwrap();
        assert_eq!(
            ir["schema_version"], manifest["schema_version"],
            "manifest IR version"
        );
        let catalog_hash = format!("{:x}", Sha256::digest(serde_json::to_vec(&ir).unwrap()));
        assert_eq!(
            catalog_hash,
            read_to_string(base.join(manifest["expected_digest"].as_str().unwrap()))
                .unwrap()
                .trim(),
            "manifest catalog digest"
        );
        profiles.push((raw, catalog_raw, canonical));
    }
    let mut generated = String::from(
        "pub static MATCH_PROFILES: std::sync::LazyLock<Vec<MatchProfile>> = std::sync::LazyLock::new(|| { let mut profiles = vec![\n",
    );
    for (raw, catalog_raw, banned_path) in profiles {
        let banned_index = banned_files.keys().position(|p| p == &banned_path).unwrap();
        generated += &format!(
            "MatchProfile::load({raw:?}, {catalog_raw:?}, BANNED_LISTS[{banned_index}]).expect(\"test-validated profile\"),\n"
        );
    }
    generated += "]; profiles.sort_by(|a, b| a.name.cmp(&b.name)); profiles });\n";
    generated += "pub(crate) const BANNED_LISTS: &[&str] = &[\n";
    for raw in banned_files.values() {
        generated += &format!("{raw:?},\n");
    }
    generated += "];\n";
    fs::write(
        PathBuf::from(env::var("OUT_DIR").unwrap()).join("match_profiles.rs"),
        generated,
    )
    .unwrap();
    // Cargo profile of this unit for diagnostic test receipts. Never source
    // dependent; https://doc.rust-lang.org/cargo/reference/environment-variables.html
    for (key, name) in [
        ("OPT_LEVEL", "MTG_CORE_OPT_LEVEL"),
        ("PROFILE", "MTG_CORE_PROFILE"),
    ] {
        println!(
            "cargo:rustc-env={name}={}",
            env::var(key).unwrap_or_default()
        );
    }
}
