use super::*;

const R5: &str = include_str!("../../../../tests/fixtures/compile-inputs-phase1-r5.json");
const R6: &str = include_str!("../../../../tests/fixtures/compile-inputs-phase1-r6.json");
const GRAMMAR_BATCH_TWO: &str =
    include_str!("../../../../tests/fixtures/compile-inputs-grammar-batch2.json");
const PRODUCER: &str = "0000000000000000000000000000000000000000000000000000000000000001";

fn r5() -> Value {
    strict_read(R5).unwrap()
}

fn read(manifest: &Value) -> Result<FrozenInputs, CompileError> {
    FrozenInputs::read(&manifest.to_string())
}

fn scratch(label: &str) -> PathBuf {
    for attempt in 0.. {
        let dir =
            std::env::temp_dir().join(format!("mtg-compile-{label}-{}-{attempt}", process::id()));
        match fs::create_dir(&dir) {
            Ok(()) => {
                fs::create_dir(dir.join("cache")).unwrap();
                return dir;
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => panic!("cannot create {}: {e}", dir.display()),
        }
    }
    unreachable!()
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
fn r6_requires_its_versioned_pins_and_the_promoted_registration() {
    let r6: Value = strict_read(R6).unwrap();
    let inputs = read(&r6).unwrap();
    let registration = check_registration(&inputs).unwrap();
    assert_eq!(registration.cut, ArchiveCut::R6);
    assert!(!registration.candidate);
    let logical = inputs.logical();
    assert_eq!(logical["r6"]["printed"], r6["r6"]["printed"]);
    assert_eq!(logical["r6"]["nameCatalog"], r6["r6"]["nameCatalog"]);
    for key in ["archiveManifest", "legacyArchive", "rulesPath"] {
        assert!(logical["r6"].get(key).is_none(), "{key}");
    }

    let mut downgraded = r6.clone();
    downgraded["schemaVersion"] = 1.into();
    assert!(matches!(read(&downgraded), Err(CompileError::Input(e)) if e.contains("/r6")));
    let mut missing = r6.clone();
    missing["r6"]["printed"]
        .as_object_mut()
        .unwrap()
        .remove("recordsSha256");
    assert!(
        matches!(read(&missing), Err(CompileError::Input(e)) if e.contains("/r6/printed/recordsSha256"))
    );
    let mut extra = r6.clone();
    extra["r6"]["nameCatalog"]["path"] = "unpinned.json".into();
    assert!(
        matches!(read(&extra), Err(CompileError::Input(e)) if e.contains("/r6/nameCatalog/path"))
    );
    for pointer in [
        "/r6/printed/schemaSha256",
        "/r6/printed/vocabularySha256",
        "/r6/printed/generatorSha256",
        "/r6/printed/recordsSha256",
        "/r6/nameCatalog/sha256",
    ] {
        let mut changed = r6.clone();
        *changed.pointer_mut(pointer).unwrap() = PRODUCER.into();
        assert!(matches!(
            check_registration(&read(&changed).unwrap()),
            Err(CompileError::Registration(_))
        ));
    }
}

#[test]
fn changed_legacy_rows_are_required_only_for_the_grammar_cut() {
    let mut grammar: Value = strict_read(GRAMMAR_BATCH_TWO).unwrap();
    let inputs = read(&grammar).unwrap();
    let promoted = check_registration(&inputs).unwrap();
    let second_r6 = accepted_archives()
        .unwrap()
        .iter()
        .filter(|row| row.cut == ArchiveCut::R6)
        .nth(1)
        .unwrap();
    assert_eq!(promoted.manifest_sha256, second_r6.manifest_sha256);
    assert!(!promoted.pending && !promoted.candidate);
    assert_eq!(inputs.logical()["r6"]["legacyRows"], "changed");

    grammar["r6"].as_object_mut().unwrap().remove("legacyRows");
    assert!(matches!(
        check_registration(&read(&grammar).unwrap()),
        Err(CompileError::Registration(_))
    ));

    let mut base: Value = strict_read(R6).unwrap();
    base["r6"]["legacyRows"] = "changed".into();
    assert!(matches!(
        check_registration(&read(&base).unwrap()),
        Err(CompileError::Registration(_))
    ));
}

#[test]
fn r6_fresh_writer_fails_closed_on_a_different_reader() {
    let dir = scratch("fresh-reader");
    let mut inputs = read(&strict_read(R6).unwrap()).unwrap();
    inputs.rules.sha256 = sha256(b"rules");
    let r6 = inputs.r6.as_mut().unwrap();
    r6.rules_path = "rules".into();
    r6.legacy_archive = "legacy".into();
    fs::write(dir.join("rules"), b"rules").unwrap();
    fs::write(dir.join("legacy"), b"legacy").unwrap();
    fs::create_dir(dir.join("scripts")).unwrap();
    // The stand-in writer exits 3 unless it is handed the pinned reader.
    fs::write(
        dir.join("scripts/engine-env.sh"),
        r#"case "$*" in *"--expect-source 9fa9dd672858102bf79c3a8ddbd78d9fd672f80ac253d1d2fb60b0e6e7043f5a --expect-generator 436b1b172a982c79711de3e6479451431e5645a9ee897fbe45af0f216f2dfa9e"*) exit 3 ;; esac
exit 1
"#,
    )
    .unwrap();
    let Err(CompileError::Reader(reason)) = generate_archive(&inputs, &dir, &dir) else {
        panic!("a different reader must fail as a reader error");
    };
    assert!(reason.contains("readable in cached mode only"), "{reason}");
    assert!(reason.contains(&inputs.reader.source_sha256), "{reason}");
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn r6_fresh_writer_ignores_the_reserved_r5_cache_basename() {
    let dir = scratch("fresh-basename");
    let mut manifest: Value = strict_read(R6).unwrap();
    manifest["archive"]["file"] = "oracle-ast-phase1-r5.jsonl".into();
    let mut inputs = read(&manifest).unwrap();
    check_registration(&inputs).unwrap();
    // Substitute a small writer with the same reserved basename as the real one.
    inputs.rules.sha256 = sha256(b"rules");
    let r6 = inputs.r6.as_mut().unwrap();
    r6.rules_path = "rules".into();
    r6.legacy_archive = "legacy".into();
    fs::write(dir.join("rules"), b"rules").unwrap();
    fs::write(dir.join("legacy"), b"legacy").unwrap();
    fs::create_dir(dir.join("scripts")).unwrap();
    fs::write(
        dir.join("scripts/engine-env.sh"),
        r#"while [ "$1" != --output ]; do shift; done
case ${2##*/} in oracle-ast-phase1-r5.jsonl) exit 1 ;; esac
printf 'generated archive' > "$2"
"#,
    )
    .unwrap();
    let archive = generate_archive(&inputs, &dir, &dir).unwrap();
    assert_eq!(fs::read(archive.path()).unwrap(), b"generated archive");
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn oversized_manifest_rejects_before_decoding() {
    let dir = scratch("input-limit");
    let manifest = dir.join("inputs.json");
    let mut bytes = R5.as_bytes().to_vec();
    bytes.resize(INPUTS_MAX_BYTES as usize + 1, b' ');
    fs::write(&manifest, bytes).unwrap();
    assert_eq!(
        identity(&manifest, &dir, PRODUCER).unwrap_err(),
        input("input manifest exceeds 64 KiB"),
    );
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn staged_writer_io_errors_are_destination_failures() {
    let dir = scratch("staged-io");
    let path = dir.join("catalog.json");
    fs::write(&path, b"occupied").unwrap();
    let error = write_payload(&path, 64, |_| Ok(())).unwrap_err();
    assert!(matches!(
        CompileError::from(error),
        CompileError::Destination(_)
    ));
    fs::remove_file(&path).unwrap();
    let error = write_payload(&path, 1, |writer| Ok(writer.write_all(b"too long")?)).unwrap_err();
    assert!(matches!(
        CompileError::from(error),
        CompileError::Destination(_)
    ));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn every_r5_pin_except_build_id_is_mandatory() {
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
