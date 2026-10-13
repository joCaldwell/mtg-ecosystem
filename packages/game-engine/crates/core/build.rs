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
            collect(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs" || x == "json")
            || p.file_name().is_some_and(|x| x == "Cargo.toml")
        {
            out.push(p);
        }
    }
}
fn main() {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../../..");
    let mut files = vec![
        root.join("Cargo.toml"),
        root.join("Cargo.lock"),
        root.join("rust-toolchain.toml"),
    ];
    collect(&root.join("packages/game-engine/crates"), &mut files);
    collect(
        &root.join("packages/game-engine/tests/fixtures"),
        &mut files,
    );
    println!(
        "cargo:rerun-if-changed={}",
        root.join("packages/game-engine/crates").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        root.join("packages/game-engine/tests/fixtures").display()
    );
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
    for key in ["PROFILE", "TARGET", "CARGO_ENCODED_RUSTFLAGS"] {
        let value = env::var(key).unwrap_or_default();
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
