// Engine build identity, computed at the executable edge. Included with
// `#[path]` by the mtg-sim and mtg-server build scripts, never compiled into
// mtg-core: core receives the value through `mtg_core::install_build_identity`
// so compiler, simulator, host and test-only edits never rebuild it.
//
// The hash covers the native source closure: path NUL contents NUL for every
// `engine_build_input` file in path-byte order,
// then rustc `--version` output and profile metadata separated by NUL.
#[path = "contracts/src/build_input.rs"]
#[allow(dead_code)]
mod build_input;

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
        } else if p.extension().is_some_and(|x| x == "rs")
            || p.file_name().is_some_and(|x| x == "Cargo.toml")
        {
            out.push(p);
        }
    }
}

/// Emits `MTG_BUILD_ID`, `MTG_RUSTC` and `MTG_{PROFILE,TARGET,...}` for the
/// including crate. `root` is the repository root.
pub fn emit(root: &Path) {
    let mut files = vec![
        root.join("Cargo.toml"),
        root.join("Cargo.lock"),
        root.join("rust-toolchain.toml"),
    ];
    collect(&root.join("packages/game-engine/crates"), &mut files);
    let relative = |p: &Path| p.strip_prefix(root).unwrap().to_string_lossy().into_owned();
    files.retain(|p| build_input::engine_build_input(&relative(p)));
    // The manifest is ordered by serialized path bytes, not PathBuf's
    // component ordering (which differs once sources have nested modules).
    files.sort_by_key(|p| relative(p));
    let mut hash = Sha256::new();
    for p in &files {
        // Watch identity files individually, never a directory: Cargo scans a
        // watched directory recursively, so test edits would rerun this script
        // and rebuild the executable. A new production module needs a `mod`
        // edit in a watched file; a new crate edits the watched workspace.
        // https://doc.rust-lang.org/cargo/reference/build-scripts.html#rerun-if-changed
        println!("cargo:rerun-if-changed={}", p.display());
        hash.update(relative(p).as_bytes());
        hash.update([0]);
        hash.update(fs::read(p).expect("build input"));
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
