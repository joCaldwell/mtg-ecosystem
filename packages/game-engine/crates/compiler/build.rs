use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
};
fn collect(root: &Path, dir: &Path, files: &mut BTreeMap<String, PathBuf>) {
    for entry in fs::read_dir(dir).expect("fixture directory") {
        let p = entry.expect("fixture entry").path();
        if p.is_dir() {
            collect(root, &p, files);
        } else if p.extension().is_some_and(|e| e == "json" || e == "sha256") {
            files.insert(
                p.strip_prefix(root).unwrap().to_str().unwrap().to_owned(),
                p,
            );
        }
    }
}
fn main() {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("../../tests/fixtures")
        .canonicalize()
        .unwrap();
    println!("cargo:rerun-if-changed={}", root.display());
    let mut files = BTreeMap::new();
    collect(&root, &root, &mut files);
    let mut generated = String::from("pub const FILES: &[(&str, &str)] = &[\n");
    for (name, path) in &files {
        generated += &format!("({name:?}, include_str!({:?})),\n", path.to_str().unwrap());
    }
    generated += "];\n";
    // Compatibility adapters for mutation tests and retained evidence, never
    // catalog membership. Both their order and their lengths come from data.
    let groups: BTreeMap<String, Vec<String>> =
        serde_json::from_str(&fs::read_to_string(root.join("record-groups.json")).unwrap())
            .unwrap();
    for (name, records) in groups {
        generated += &format!("pub const {name}: [(&str, &str); {}] = [\n", records.len());
        for record in records {
            generated += &format!(
                "({:?}, include_str!({:?})),\n",
                record.trim_end_matches(".json"),
                root.join(&record).to_str().unwrap()
            );
        }
        generated += "];\n";
    }
    generated += &format!(
        "pub const MANIFEST: &str = include_str!({:?});\n",
        root.join("archive-manifest.json").to_str().unwrap()
    );
    fs::write(
        PathBuf::from(env::var("OUT_DIR").unwrap()).join("fixtures.rs"),
        generated,
    )
    .unwrap();
}
