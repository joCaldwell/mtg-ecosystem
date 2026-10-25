use sha2::{Digest, Sha256};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};
fn collect(path: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(path).expect("engine source directory") {
        let p = entry.expect("source entry").path();
        if p.is_dir() {
            if p.file_name().is_some_and(|name| {
                name == "generated" || name == "target" || name == "node_modules" || name == ".git"
            }) {
                continue;
            }
            collect(&p, out);
        } else if p
            .extension()
            .is_some_and(|x| x == "rs" || x == "json" || x == "sha256")
            || p.file_name().is_some_and(|x| x == "Cargo.toml")
        {
            out.push(p);
        }
    }
}
fn main() {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../../..");
    // Catalog digests/counts and scenario definition pins derive from reviewed
    // expected IR. The compiler freshness test checks it against source records.
    let data = root.join("packages/game-engine/tests/fixtures/catalogs");
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
        let Some(manifest) = fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        else {
            continue;
        };
        let Some(expected) = manifest["expected_ir"].as_str() else {
            continue;
        };
        let Some(catalog) = fs::read(data.join(expected))
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
    let mut files = vec![
        root.join("Cargo.toml"),
        root.join("Cargo.lock"),
        root.join("rust-toolchain.toml"),
    ];
    collect(&root.join("packages/game-engine/crates"), &mut files);
    println!(
        "cargo:rerun-if-changed={}",
        root.join("packages/game-engine/crates").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        root.join("packages/game-engine/tests/fixtures").display()
    );
    files.retain(|p| {
        mtg_contracts::engine_build_input(&p.strip_prefix(&root).unwrap().to_string_lossy())
    });
    // The manifest is ordered by serialized path bytes, not PathBuf's
    // component ordering (which differs once sources have nested modules).
    files.sort_by_key(|p| {
        p.strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .into_owned()
    });
    let mut hash = Sha256::new();
    for p in files {
        println!("cargo:rerun-if-changed={}", p.display());
        hash.update(p.strip_prefix(&root).unwrap().to_string_lossy().as_bytes());
        hash.update([0]);
        hash.update(fs::read(&p).expect("build input"));
        hash.update([0]);
    }
    let rustc = Command::new(env::var("RUSTC").unwrap())
        .arg("--version")
        .output()
        .expect("rustc identity");
    hash.update(&rustc.stdout);
    // Cargo PROFILE alone conflates dev and optimized test builds. Capture
    // effective optimization and checks as part of the reproducible identity.
    // https://doc.rust-lang.org/cargo/reference/environment-variables.html
    for key in [
        "PROFILE",
        "TARGET",
        "CARGO_ENCODED_RUSTFLAGS",
        "OPT_LEVEL",
        "DEBUG",
        "DEBUG_ASSERTIONS",
    ] {
        let value = if key == "DEBUG_ASSERTIONS" {
            env::var_os("CARGO_CFG_DEBUG_ASSERTIONS")
                .is_some()
                .to_string()
        } else {
            env::var(key).unwrap_or_default()
        };
        hash.update([0]);
        hash.update(value.as_bytes());
        println!("cargo:rustc-env=MTG_{key}={value}");
    }
    println!("cargo:rustc-env=MTG_BUILD_ID={:x}", hash.finalize());
    println!(
        "cargo:rustc-env=MTG_RUSTC={}",
        String::from_utf8(rustc.stdout).unwrap().trim_end()
    );
}
