use std::{
    collections::BTreeSet,
    env, fs,
    path::{Component, Path, PathBuf},
};

fn relative(base: &Path, name: &str) -> PathBuf {
    let mut out = PathBuf::new();
    for c in base.join(name).components() {
        match c {
            Component::ParentDir if out.file_name().is_some_and(|n| n != "..") => {
                out.pop();
            }
            Component::CurDir => {}
            _ => out.push(c.as_os_str()),
        }
    }
    out
}
fn json(root: &Path, name: &Path) -> Option<serde_json::Value> {
    serde_json::from_slice(&fs::read(root.join(name)).ok()?).ok()
}
fn main() {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../tests/fixtures");
    // Directory watches recurse: only the manifest directory is discoverable.
    // Source records/archives are watched individually below, including missing
    // paths so repairing an input retriggers the build.
    // https://doc.rust-lang.org/cargo/reference/build-scripts.html#rerun-if-changed
    println!("cargo:rerun-if-changed={}", root.join("catalogs").display());
    let mut files: BTreeSet<PathBuf> = [
        "accepted-archives.json",
        "provenance.json",
        "archive-manifest-phase1-r5.json",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect();
    let mut provenances = BTreeSet::from([PathBuf::from("provenance.json")]);
    if let Ok(entries) = fs::read_dir(root.join("catalogs")) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.extension().is_some_and(|e| e == "json") {
                continue;
            }
            let name = Path::new("catalogs").join(path.file_name().unwrap());
            if let Some(m) = json(&root, &name)
                && let Some(p) = m["provenance"].as_str()
            {
                provenances.insert(relative(Path::new("catalogs"), p));
            }
            // Include even malformed manifests, so the CLI can diagnose them.
            files.insert(name);
        }
    }
    for provenance in provenances {
        let base = provenance.parent().unwrap();
        files.insert(provenance.clone());
        files.insert(base.join("accepted-archives.json"));
        if let Some(p) = json(&root, &provenance)
            && let Some(rows) = p["records"].as_array()
        {
            for row in rows {
                for key in ["file", "archive"] {
                    if let Some(name) = row[key].as_str() {
                        files.insert(relative(base, name));
                    }
                }
            }
        }
    }
    let mut generated = String::new();
    generated += "pub const FILES: &[(&str, &str)] = &[\n";
    // Embed the exact inputs needed to lower each reviewed manifest.
    for name in &files {
        println!("cargo:rerun-if-changed={}", root.join(name).display());
        if let Ok(raw) = fs::read_to_string(root.join(name)) {
            generated += &format!("({:?}, {raw:?}),\n", name.to_str().unwrap());
        }
    }
    generated += "];\n";
    let header =
        fs::read_to_string(root.join("archive-manifest-phase1-r5.json")).unwrap_or_default();
    generated += &format!("pub const MANIFEST: &str = {header:?};\n");
    fs::write(
        PathBuf::from(env::var("OUT_DIR").unwrap()).join("fixtures.rs"),
        generated,
    )
    .unwrap();
}
