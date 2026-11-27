use super::*;

const R5: &str = include_str!("../../../../tests/fixtures/compile-inputs-phase1-r5.json");
const PRODUCER: &str = "0000000000000000000000000000000000000000000000000000000000000001";

fn r5() -> Value {
    strict_read(R5).unwrap()
}

fn read(manifest: &Value) -> Result<FrozenInputs, CompileError> {
    FrozenInputs::read(&manifest.to_string())
}

fn scratch(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mtg-compile-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("cache")).unwrap();
    dir
}

/// A manifest in `dir` whose Oracle and paper-evidence pins name small local
/// files with their real lengths and digests.
fn local_manifest(dir: &Path, edit: impl FnOnce(&mut Value)) -> PathBuf {
    let mut manifest = r5();
    for (key, body) in [("oracle", "oracle bytes"), ("paperEvidence", "paper bytes")] {
        fs::write(dir.join(key), body).unwrap();
        manifest[key]["path"] = key.into();
        manifest[key]["bytes"] = body.len().into();
        manifest[key]["sha256"] = sha256(body.as_bytes()).into();
    }
    edit(&mut manifest);
    let path = dir.join("inputs.json");
    fs::write(&path, manifest.to_string()).unwrap();
    path
}

#[test]
fn r5_manifest_pins_everything_and_logical_identity_drops_locations() {
    let inputs = read(&r5()).unwrap();
    assert_eq!(inputs.archive.sha256, ARCHIVE_R5_SHA256);
    assert_eq!(inputs.compiler.build_id, None);
    let logical = inputs.logical();
    for pointer in [
        "/oracle/path",
        "/paperEvidence/path",
        "/archive/file",
        "/compiler/buildId",
    ] {
        assert_eq!(logical.pointer(pointer), None, "{pointer}");
    }
    // Relocating every file leaves the logical identity unchanged.
    let mut moved = r5();
    moved["oracle"]["path"] = "elsewhere/oracle.json".into();
    moved["archive"]["file"] = "renamed.jsonl".into();
    moved["compiler"]["buildId"] = PRODUCER.into();
    assert_eq!(read(&moved).unwrap().logical(), logical);
    // The tracked manifest is registered under this compiler.
    check_compiler(&inputs, PRODUCER).unwrap();
    assert_eq!(
        check_registration(&inputs).unwrap().manifest_file,
        "archive-manifest-phase1-r5.json"
    );
}
const ARCHIVE_R5_SHA256: &str = "7400edf1925081db984d619c56ce17f3f004153f99a57160b5f2f08fd52c5303";

#[test]
fn every_pin_except_build_id_is_mandatory() {
    for pointer in [
        "/oracle/sha256",
        "/paperEvidence/path",
        "/scope",
        "/rules/sha256",
        "/reader/sourceSha256",
        "/archive/sha256",
        "/archive/headerSha256",
        "/compiler/irVersion",
    ] {
        let mut manifest = r5();
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        manifest
            .pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(key);
        assert_eq!(
            read(&manifest).unwrap_err(),
            CompileError::Input(format!("missing pin `{pointer}`"))
        );
    }
    let mut manifest = r5();
    manifest["archive"]["bytes"] = Value::Null;
    assert_eq!(
        read(&manifest).unwrap_err(),
        CompileError::Input("missing pin `/archive/bytes`".into())
    );
    // The optional producer pin may be absent but never null.
    manifest = r5();
    manifest["compiler"]["buildId"] = Value::Null;
    assert_eq!(
        read(&manifest).unwrap_err(),
        CompileError::Input("null pin `/compiler/buildId`".into())
    );
}

#[test]
fn extra_unpinned_inputs_reject_by_pointer() {
    let mut manifest = r5();
    manifest["decks"] = serde_json::json!({"path": "decks.json"});
    assert_eq!(
        read(&manifest).unwrap_err(),
        CompileError::Input("unpinned input `/decks`".into())
    );
    manifest = r5();
    manifest["archive"]["mirror"] = "other.jsonl".into();
    assert_eq!(
        read(&manifest).unwrap_err(),
        CompileError::Input("unpinned input `/archive/mirror`".into())
    );
}

#[test]
fn malformed_pins_reject_before_any_file_is_read() {
    let cases: [(&str, Value); 5] = [
        ("/archive/sha256", "ABC".into()),
        ("/oracle/path", "/abs/oracle.json".into()),
        ("/archive/file", "../escape.jsonl".into()),
        ("/schemaVersion", 2.into()),
        ("/archive/bytes", 0.into()),
    ];
    for (pointer, value) in cases {
        let mut manifest = r5();
        *manifest.pointer_mut(pointer).unwrap() = value;
        assert!(
            matches!(read(&manifest), Err(CompileError::Input(_))),
            "{pointer}"
        );
    }
    let duplicate = R5.replacen("\"scope\":", "\"scope\": \"x\", \"scope\":", 1);
    assert!(matches!(
        FrozenInputs::read(&duplicate),
        Err(CompileError::Input(e)) if e.contains("duplicate")
    ));
}

#[test]
fn source_digest_mismatch_and_symlinks_reject_as_inputs() {
    let dir = scratch("digest");
    let cache = dir.join("cache");
    let manifest = local_manifest(&dir, |m| {
        m["paperEvidence"]["sha256"] = sha256(b"other bytes").into();
    });
    let error = identity(&manifest, &cache, PRODUCER).unwrap_err();
    assert_eq!(
        error,
        CompileError::Input(format!(
            "paperEvidence sha256 mismatch: pinned {}, found {}",
            sha256(b"other bytes"),
            sha256(b"paper bytes")
        ))
    );
    let manifest = local_manifest(&dir, |m| m["oracle"]["bytes"] = 99.into());
    assert!(matches!(
        identity(&manifest, &cache, PRODUCER),
        Err(CompileError::Input(e)) if e.starts_with("oracle byte length mismatch")
    ));
    let manifest = local_manifest(&dir, |_| {});
    fs::remove_file(dir.join("oracle")).unwrap();
    std::os::unix::fs::symlink(dir.join("paperEvidence"), dir.join("oracle")).unwrap();
    assert!(matches!(
        identity(&manifest, &cache, PRODUCER),
        Err(CompileError::Input(e)) if e.contains("is not a regular file")
    ));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn verified_sources_still_need_the_registered_archive_and_this_compiler() {
    let dir = scratch("registration");
    let cache = dir.join("cache");
    // Small local sources verify, but the R5 registration binds the real corpus.
    let manifest = local_manifest(&dir, |_| {});
    assert_eq!(
        identity(&manifest, &cache, PRODUCER).unwrap_err(),
        CompileError::Registration(
            "pinned reader or Oracle identity differs from the registered archive".into()
        )
    );
    let manifest = local_manifest(&dir, |m| m["archive"]["headerSha256"] = PRODUCER.into());
    assert!(matches!(
        identity(&manifest, &cache, PRODUCER),
        Err(CompileError::Registration(e)) if e.contains("is not registered")
    ));
    let manifest = local_manifest(&dir, |m| m["archive"]["sha256"] = PRODUCER.into());
    assert!(matches!(
        identity(&manifest, &cache, PRODUCER),
        Err(CompileError::Registration(e)) if e.contains("registered full-file pin")
    ));
    let other = "0000000000000000000000000000000000000000000000000000000000000002";
    let manifest = local_manifest(&dir, |m| m["compiler"]["buildId"] = other.into());
    assert_eq!(
        identity(&manifest, &cache, PRODUCER).unwrap_err(),
        CompileError::Compiler(format!(
            "pinned build {other} differs from this binary's {PRODUCER}"
        ))
    );
    let manifest = local_manifest(&dir, |m| m["rules"]["edition"] = "2020-01-01".into());
    assert!(matches!(
        identity(&manifest, &cache, PRODUCER),
        Err(CompileError::Compiler(_))
    ));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn error_classes_print_their_kind() {
    assert_eq!(CompileError::Reader("x".into()).to_string(), "reader: x");
    assert_eq!(CompileError::Destination("y".into()).kind(), "destination");
}
