use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::{Component, Path, PathBuf},
};

// Legacy evidence/mutation-test adapters only. Their contents and lengths come
// from record-groups.json; they never determine executable catalog membership.
const GROUPS: &[&str] = &[
    "WIPES_RECORDS",
    "SACRIFICE_RECORDS",
    "REMOVAL_RECORDS",
    "ATTACHMENT_RECORDS",
    "FOUR_COLORS_RECORDS",
    "MANA_RECORDS",
    "STAPLES_RECORDS",
    "RECORDS",
    "COMMANDER_RECORDS",
    "LIBRARY_RECORDS",
    "DAMAGE_RECORDS",
    "ACTION_RECORDS",
    "STACK_RECORDS",
    "DAMAGE_LIFE_RECORDS",
    "TRIGGER_RECORDS",
    "EFFECT_RECORDS",
    "CHOICE_RECORDS",
    "MATCH_RECORDS",
    "TOKEN_RECORDS",
    "COMBAT_KEYWORD_RECORDS",
    "KEYWORD_RECORDS",
];
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
    println!("cargo:rerun-if-changed={}", root.display());
    let mut files: BTreeSet<PathBuf> = [
        "accepted-archives.json",
        "record-groups.json",
        "provenance.json",
        "archive-manifest.json",
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
    let groups: Result<BTreeMap<String, Vec<String>>, String> =
        fs::read(root.join("record-groups.json"))
            .map_err(|e| e.to_string())
            .and_then(|raw| serde_json::from_slice(&raw).map_err(|e| e.to_string()));
    let mut errors = vec![];
    let groups = groups.unwrap_or_else(|e| {
        errors.push(e);
        BTreeMap::new()
    });
    for name in groups.keys() {
        if !GROUPS.contains(&name.as_str()) {
            errors.push(format!("unknown evidence record group {name}"));
        }
    }
    let mut generated = String::new();
    for name in GROUPS {
        let records = groups.get(*name).cloned().unwrap_or_else(|| {
            errors.push(format!("missing evidence record group {name}"));
            vec![]
        });
        generated += &format!("pub const {name}: [(&str, &str); {}] = [\n", records.len());
        for record in records {
            files.insert(PathBuf::from(&record));
            let raw = fs::read_to_string(root.join(&record)).unwrap_or_else(|e| {
                errors.push(format!("{record}: {e}"));
                String::new()
            });
            generated += &format!("({:?}, {raw:?}),\n", record.trim_end_matches(".json"));
        }
        generated += "];\n";
    }
    generated += &format!(
        "pub const RECORD_GROUP_ERROR: Option<&str> = {:?};\n",
        if errors.is_empty() {
            None
        } else {
            Some(errors.join("; "))
        }
    );
    generated += "pub const FILES: &[(&str, &str)] = &[\n";
    // Embed source inputs only. Expected IR, policy copies and other evidence
    // remain on disk; they are not needed for lowering or runtime diagnostics.
    for name in files {
        if let Ok(raw) = fs::read_to_string(root.join(&name)) {
            generated += &format!("({:?}, {raw:?}),\n", name.to_str().unwrap());
        }
    }
    generated += "];\n";
    let header = fs::read_to_string(root.join("archive-manifest.json")).unwrap_or_default();
    generated += &format!("pub const MANIFEST: &str = {header:?};\n");
    fs::write(
        PathBuf::from(env::var("OUT_DIR").unwrap()).join("fixtures.rs"),
        generated,
    )
    .unwrap();
}
