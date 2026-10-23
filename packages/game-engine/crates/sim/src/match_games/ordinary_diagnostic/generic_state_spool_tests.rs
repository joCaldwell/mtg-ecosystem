use super::super::{canonical, equal_state_proofs, sha256, write_generic_state_value};
use super::*;
use std::io::Cursor;
use std::os::unix::fs::PermissionsExt;

fn input(raw: &[u8]) -> Result<SpoolingSnapshot, String> {
    decode(
        Cursor::new(raw),
        RawProof {
            bytes: raw.len() as u64,
            sha256: sha256(raw),
        },
    )
}
fn fixture(history: usize, viewers: usize) -> Vec<u8> {
    let views = (0..history).map(|i| serde_json::json!({
        "viewer":i%4,"objects":[{"handle":"雪\u{0000}\n\"","map":{"2":null,"10":"-7"}}],
        "optional":null,"duplicate":i/2,"turn":{"number":i},"every_unknown_future_field":[true,false,{"z":0,"a":-3}]
    })).collect::<Vec<_>>();
    let knowledge = (0..viewers).map(|i| serde_json::json!({"z":null,"history":views,"viewer":i,"extra":{"history":"not a structural history","2":false,"10":true}})).collect::<Vec<_>>();
    serde_json::to_vec(&serde_json::json!({"trace":{"records":[{"details":[1,2,3]},null]},"state":{"z":null,"visibility":knowledge,"a":{"2":"-3","10":"雪"},"after":"history"},"metadata":{"unknown":true}})).unwrap()
}
fn compare_old(raw: &[u8]) {
    // This is the old bounded String -> whole generic Value -> canonical oracle,
    // intentionally independent of seeds, spooled ranges and production memos.
    let old: Value = serde_json::from_str(std::str::from_utf8(raw).unwrap()).unwrap();
    let expected = canonical(&old["state"]).unwrap().into_bytes();
    let parsed = input(raw).unwrap();
    let directory = parsed.owner.directory.clone();
    let proof = parsed.finish().unwrap();
    assert_eq!(fs::read(&proof.path).unwrap(), expected);
    assert_eq!(proof.sha256, sha256(&expected));
    assert_eq!(proof.bytes, expected.len() as u64);
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
    assert_eq!(
        fs::metadata(&directory).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(&proof.path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    proof.remove().unwrap();
    assert!(!directory.exists());
}

#[test]
fn ordinary_spooled_generic_matches_old_all_fields_arrays_and_chunk_edges() {
    for histories in [0, 1, 31, 32, 33, 64, 97] {
        for viewers in [0, 1, 4, 5] {
            compare_old(&fixture(histories, viewers));
        }
    }
    // Nonlexical text, escaped keys, arbitrary new metadata/State/Knowledge fields
    // and a complete repeated/reordered history remain byte-exact, without a
    // fixed four-viewer or fixed field whitelist.
    compare_old(br#" {"trace":{"huge_ignored_shape":[null,{"new":false}]},"state":{"z":null,"visibility":[{"z":3,"history":[{"z":1,"a":2},{"x":null},{"z":1,"a":2}],"a":"h"}],"2":{"z":true,"a":false}}} "#);
}

#[test]
fn ordinary_spooled_generic_matches_actual_roots_trace_modes_and_old_oracle() {
    use mtg_core::{Prepared, TraceMode};
    let catalog = mtg_contracts::Catalog::read(super::super::CATALOG_JSON).unwrap();
    for index in [0, 4, 8] {
        let fixture = crate::search_roots::prepare(&catalog, index).unwrap();
        for trace in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
            let prepared =
                Prepared::admit_pod(catalog.clone(), fixture.case.config.clone()).unwrap();
            let game =
                Game::replay_with_trace(prepared, &fixture.game.replay_journal(), trace).unwrap();
            let old_snapshot = game.snapshot().unwrap();
            let envelope: Value = serde_json::from_str(&old_snapshot).unwrap();
            let mut old = GenericStateProof::create().unwrap();
            let mut file = private_file(&old.path).unwrap();
            (old.sha256, old.bytes) =
                write_generic_state_value(&envelope["state"], &mut file).unwrap();
            drop(file);
            let before = game.state_digest().unwrap();
            let journal = game.replay_journal();
            let new = prove(&game).unwrap();
            equal_state_proofs(&old, &new).unwrap();
            assert_eq!(new.sha256, before);
            assert_eq!(game.state_digest().unwrap(), before);
            assert_eq!(game.replay_journal(), journal);
            old.remove().unwrap();
            new.remove().unwrap();
        }
    }
}

#[test]
fn ordinary_spooled_generic_rejects_malformed_duplicates_ignored_trace_and_suffix() {
    for raw in [
        "",
        "[]",
        "{}",
        "{\"state\":null}",
        "{\"state\":[]}",
        "{\"state\":{}}",
        "{\"state\":{\"visibility\":null}}",
        "{\"state\":{\"visibility\":[null]}}",
        "{\"state\":{\"visibility\":[{}]}}",
        "{\"state\":{\"visibility\":[{\"history\":{}}]}}",
        "{\"state\":{\"visibility\":[],\"visibility\":[]}}",
        "{\"state\":{\"visibility\":[]},\"state\":{\"visibility\":[]}}",
        "{\"state\":{\"visibility\":[{\"history\":[],\"hist\\u006fry\":[]}]}}",
        "{\"state\":{\"visibility\":[{\"history\":[{\"x\":1,\"x\":2}]}]}}",
        "{\"state\":{\"visibility\":[]},\"trace\":{\"x\":1,\"\\u0078\":2}}",
        "{\"state\":{\"visibility\":[]}}{}",
        "{\"state\":{\"visibility\":[]}}x",
        "{\"state\":{\"visibility\":[]}",
    ] {
        assert!(input(raw.as_bytes()).is_err(), "accepted {raw}");
    }
    let raw = b" {\"state\":{\"visibility\":[]}} \n\t";
    compare_old(raw);
    assert!(
        decode(
            Cursor::new(raw),
            RawProof {
                bytes: raw.len() as u64 - 1,
                sha256: sha256(raw)
            }
        )
        .is_err()
    );
    assert!(
        decode(
            Cursor::new(raw),
            RawProof {
                bytes: raw.len() as u64,
                sha256: "0".repeat(64)
            }
        )
        .is_err()
    );
}

#[test]
fn ordinary_spooled_generic_binds_both_reads_positions_and_complete_eof() {
    struct Changing {
        one: Cursor<Vec<u8>>,
        second: Vec<u8>,
        changed: bool,
    }
    impl Read for Changing {
        fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
            self.one.read(b)
        }
    }
    impl Seek for Changing {
        fn seek(&mut self, p: SeekFrom) -> io::Result<u64> {
            if p == SeekFrom::Start(0) && !self.changed {
                self.one = Cursor::new(self.second.clone());
                self.changed = true;
            }
            self.one.seek(p)
        }
    }
    let raw = br#"{"state":{"visibility":[]},"trace":{"x":1}}"#.to_vec();
    let mut other = raw.clone();
    let end = other.iter().rposition(|b| *b == b'1').unwrap();
    other[end] = b'2';
    assert!(
        decode(
            Changing {
                one: Cursor::new(raw.clone()),
                second: other,
                changed: false
            },
            RawProof {
                bytes: raw.len() as u64,
                sha256: sha256(&raw)
            }
        )
        .is_err()
    );
    let mut source = BoundReader::new(
        Cursor::new(&raw),
        RawProof {
            bytes: raw.len() as u64,
            sha256: sha256(&raw),
        },
    )
    .unwrap();
    assert!(source.seek(SeekFrom::Start(0)).is_err());
    assert!(source.finish().is_err());
    let mut source = BoundReader::new(
        Cursor::new(&raw),
        RawProof {
            bytes: raw.len() as u64,
            sha256: sha256(&raw),
        },
    )
    .unwrap();
    let mut scratch = Vec::new();
    source.read_to_end(&mut scratch).unwrap();
    assert!(source.finish().is_err());
    struct Invalid;
    impl Read for Invalid {
        fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
            Ok(b.len() + 1)
        }
    }
    impl Seek for Invalid {
        fn seek(&mut self, _: SeekFrom) -> io::Result<u64> {
            Ok(0)
        }
    }
    let mut source = BoundReader::new(
        Invalid,
        RawProof {
            bytes: 0,
            sha256: sha256(b""),
        },
    )
    .unwrap();
    assert!(source.read(&mut [0; 3]).is_err());
    assert!(source.finish().is_err());
}

#[test]
fn ordinary_spooled_generic_write_counts_failures_and_flush_never_mint_proofs() {
    struct Sink {
        bytes: Vec<u8>,
        mode: u8,
        interrupted: bool,
    }
    impl Write for Sink {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            if !self.interrupted {
                self.interrupted = true;
                return Err(io::ErrorKind::Interrupted.into());
            }
            if self.mode == 1 {
                return Ok(0);
            }
            if self.mode == 2 {
                return Ok(b.len() + 1);
            }
            if self.mode == 3 {
                return Err(bad("write fail"));
            }
            let n = b.len().min(3);
            self.bytes.extend_from_slice(&b[..n]);
            Ok(n)
        }
        fn flush(&mut self) -> io::Result<()> {
            if self.mode == 4 {
                Err(bad("flush fail"))
            } else {
                Ok(())
            }
        }
    }
    // Interrupted is retriable, not a poison: a partial-writing real sink must
    // preserve exactly all accepted bytes and their successful final receipt.
    let data = "雪\0\n complete generic bytes".as_bytes();
    let mut out = AcceptedWriter::new(Sink {
        bytes: vec![],
        mode: 0,
        interrupted: false,
    });
    out.write_all(data).unwrap();
    let (sink, proof) = out.finish().unwrap();
    assert_eq!(sink.bytes, data);
    assert_eq!(proof.bytes, data.len() as u64);
    assert_eq!(proof.sha256, sha256(data));
    for mode in 1..=4 {
        let mut out = AcceptedWriter::new(Sink {
            bytes: vec![],
            mode,
            interrupted: true,
        });
        let result = out.write_all(data);
        if mode != 4 {
            assert!(result.is_err());
        } else {
            result.unwrap();
        }
        assert!(out.finish().is_err());
    }
    let mut out = AcceptedWriter::new(Vec::new());
    out.bytes = u64::MAX;
    assert!(out.write_all(b"x").is_err());
    assert!(out.finish().is_err());
}

#[test]
fn ordinary_spooled_generic_range_corruption_and_success_cleanup_are_checked() {
    for kind in 0..5 {
        let parsed = input(&fixture(33, 4)).unwrap();
        let dir = parsed.owner.directory.clone();
        match kind {
            0 => {
                let path = dir.join("histories.json");
                let f = fs::OpenOptions::new().write(true).open(path).unwrap();
                f.set_len(1).unwrap();
            }
            1 => {
                let mut f = fs::OpenOptions::new()
                    .append(true)
                    .open(dir.join("histories.json"))
                    .unwrap();
                f.write_all(b"x").unwrap();
            }
            2 => {
                let mut parsed = parsed;
                parsed.state.visibility[1].history.start += 1;
                assert!(parsed.finish().is_err());
                assert!(!dir.exists());
                continue;
            }
            3 => {
                let mut f = fs::OpenOptions::new()
                    .write(true)
                    .open(dir.join("histories.json"))
                    .unwrap();
                f.write_all(b"x").unwrap();
            }
            _ => {
                fs::rename(dir.join("histories.json"), dir.join("moved.json")).unwrap();
                fs::create_dir(dir.join("histories.json")).unwrap();
            }
        }
        assert!(parsed.finish().is_err());
        assert!(!dir.exists());
    }
    let parsed = input(&fixture(1, 1)).unwrap();
    let dir = parsed.owner.directory.clone();
    drop(parsed);
    assert!(!dir.exists());
}

#[test]
fn ordinary_spooled_generic_copy_checks_short_interrupted_error_and_both_eofs() {
    struct Fragmented<'a> {
        bytes: &'a [u8],
        interrupted: bool,
    }
    impl Read for Fragmented<'_> {
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            if !self.interrupted {
                self.interrupted = true;
                return Err(io::ErrorKind::Interrupted.into());
            }
            let n = out.len().min(self.bytes.len()).min(7);
            out[..n].copy_from_slice(&self.bytes[..n]);
            self.bytes = &self.bytes[n..];
            Ok(n)
        }
    }
    let parsed = input(&fixture(33, 4)).unwrap();
    let bytes = fs::read(parsed.owner.directory.join("histories.json")).unwrap();
    let expected = RawProof {
        bytes: bytes.len() as u64,
        sha256: sha256(&bytes),
    };
    let mut direct = Vec::new();
    write_state(&parsed.state, &mut bytes.as_slice(), &mut direct, &expected).unwrap();
    let mut fragmented = Vec::new();
    write_state(
        &parsed.state,
        &mut Fragmented {
            bytes: &bytes,
            interrupted: false,
        },
        &mut fragmented,
        &expected,
    )
    .unwrap();
    assert_eq!(direct, fragmented);
    let mut appended = bytes.clone();
    appended.push(0);
    assert!(
        write_state(
            &parsed.state,
            &mut appended.as_slice(),
            &mut Vec::new(),
            &expected
        )
        .is_err()
    );
    assert!(
        write_state(
            &parsed.state,
            &mut bytes[..bytes.len() - 1].as_ref(),
            &mut Vec::new(),
            &expected
        )
        .is_err()
    );
    struct Broken;
    impl Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(bad("spool read failed"))
        }
    }
    assert!(write_state(&parsed.state, &mut Broken, &mut Vec::new(), &expected).is_err());
    struct Invalid;
    impl Read for Invalid {
        fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
            Ok(b.len() + 1)
        }
    }
    assert!(write_state(&parsed.state, &mut Invalid, &mut Vec::new(), &expected).is_err());
    struct NoWrite;
    impl Write for NoWrite {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(bad("State output failed"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    assert!(
        write_state(
            &parsed.state,
            &mut bytes.as_slice(),
            &mut NoWrite,
            &expected
        )
        .is_err()
    );
}

#[test]
fn ordinary_spooled_generic_reader_rejects_io_errors_bad_seek_and_overflow() {
    struct WrongSeek {
        source: Cursor<Vec<u8>>,
    }
    impl Read for WrongSeek {
        fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
            self.source.read(b)
        }
    }
    impl Seek for WrongSeek {
        fn seek(&mut self, p: SeekFrom) -> io::Result<u64> {
            if p == SeekFrom::Start(0) {
                Ok(1)
            } else {
                self.source.seek(p)
            }
        }
    }
    let raw = fixture(1, 1);
    assert!(
        decode(
            WrongSeek {
                source: Cursor::new(raw.clone())
            },
            RawProof {
                bytes: raw.len() as u64,
                sha256: sha256(&raw)
            }
        )
        .is_err()
    );
    struct Broken;
    impl Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(bad("snapshot read failed"))
        }
    }
    impl Seek for Broken {
        fn seek(&mut self, _: SeekFrom) -> io::Result<u64> {
            Ok(0)
        }
    }
    assert!(
        decode(
            Broken,
            RawProof {
                bytes: raw.len() as u64,
                sha256: sha256(&raw)
            }
        )
        .is_err()
    );
    let mut overflow = BoundReader::new(
        Cursor::new(b"x"),
        RawProof {
            bytes: 1,
            sha256: sha256(b"x"),
        },
    )
    .unwrap();
    overflow.bytes = u64::MAX;
    assert!(overflow.read(&mut [0; 1]).is_err());
    assert!(overflow.finish().is_err());
    let mut wrong_position = Cursor::new(raw);
    wrong_position.set_position(1);
    assert!(
        BoundReader::new(
            wrong_position,
            RawProof {
                bytes: 0,
                sha256: sha256(b"")
            }
        )
        .is_err()
    );
}

fn events(data: &std::rc::Rc<std::cell::RefCell<Vec<u8>>>) -> Vec<Value> {
    data.borrow()
        .split(|b| *b == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice(line).unwrap())
        .collect()
}
fn ended(data: &std::rc::Rc<std::cell::RefCell<Vec<u8>>>, phase: &str) -> bool {
    events(data)
        .iter()
        .any(|e| e["kind"] == "end" && e["phase"] == phase && e["outcome"] == "ok")
}
fn receipt(raw: &[u8]) -> RawProof {
    RawProof {
        bytes: raw.len() as u64,
        sha256: sha256(raw),
    }
}

#[test]
fn generic_state_spool_profile_paired_bytes_schema_privacy_and_disabled_zero_activity() {
    let raw = fixture(3, 2);
    let (mut profile, data) = GenericProofProfile::memory(None);
    let clock = profile.clock_reads.clone();
    let before = data.borrow().clone();
    let scope = ProofScope::root(4).unwrap().operand(Operand::A);
    let plain = decode_with_profile(Cursor::new(&raw), receipt(&raw), scope, None)
        .unwrap()
        .finish_with_profile(scope, None)
        .unwrap();
    assert_eq!(clock.get(), 0);
    assert_eq!(*data.borrow(), before);
    let profiled = decode_with_profile(Cursor::new(&raw), receipt(&raw), scope, Some(&mut profile))
        .unwrap()
        .finish_with_profile(scope, Some(&mut profile))
        .unwrap();
    equal_state_proofs(&plain, &profiled).unwrap();
    assert_eq!(
        fs::read(&plain.path).unwrap(),
        fs::read(&profiled.path).unwrap()
    );
    profile.finish(true).unwrap();
    assert!(clock.get() > 0);
    let rows = events(&data);
    assert_eq!(rows.len(), 12); // header, five coarse pairs, terminal
    for (seq, event) in rows.iter().enumerate() {
        assert_eq!(event["seq"], seq);
        assert_eq!(event["schema"], PROFILE_SCHEMA);
        let expected: &[&str] = match event["kind"].as_str().unwrap() {
            "header" => &[
                "schema",
                "kind",
                "seq",
                "build_id",
                "collection_scope",
                "instrumentation",
            ],
            "terminal" => &["schema", "kind", "seq", "at_ns", "collection_outcome"],
            "begin" | "end" => &[
                "schema",
                "kind",
                "seq",
                "scope",
                "phase",
                "at_ns",
                "duration_ns",
                "outcome",
                "bytes",
            ],
            _ => panic!("unknown event"),
        };
        assert_eq!(
            event
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<std::collections::BTreeSet<_>>(),
            expected.iter().copied().collect()
        );
        if let Some(scope) = event.get("scope") {
            assert_eq!(
                scope,
                &serde_json::json!({"living":4,"equality_ordinal":1,"operand":"a"})
            );
        }
    }
    let text = String::from_utf8(data.borrow().clone()).unwrap();
    for private in [
        "雪",
        "unknown_future_field",
        "history\":",
        "state\":",
        "sha256",
        "state.json",
        "journal",
    ] {
        assert!(!text.contains(private), "private payload {private}");
    }
    assert!(
        data.borrow()
            .split_inclusive(|b| *b == b'\n')
            .all(|l| l.len() <= MAX_LINE)
    );
    let total = profile.totals[ProofPhase::TypedPass2DecodeSpool as usize];
    assert_eq!((total.calls, total.bytes), (1, raw.len() as u64));
    plain.remove().unwrap();
    profiled.remove().unwrap();
}

#[test]
fn generic_state_spool_profile_strict_end_not_raw_eof_and_observational_seek() {
    let scope = ProofScope::root(3).unwrap();
    // strict pass1 can reach raw EOF and only then reject an unfinished document.
    for raw in [
        b"{\"state\":".as_slice(),
        b"{\"state\":{\"visibility\":[]}}x",
        b"{\"state\":{\"visibility\":[]},\"trace\":{\"x\":1,\"x\":2}}",
    ] {
        let (mut p, data) = GenericProofProfile::memory(None);
        assert!(decode_with_profile(Cursor::new(raw), receipt(raw), scope, Some(&mut p)).is_err());
        assert!(!ended(&data, "strict_pass1"));
        assert!(
            !events(&data)
                .iter()
                .any(|e| e["phase"] == "typed_pass2_decode_spool")
        );
        assert!(p.finish(true).is_err());
        assert_eq!(events(&data).last().unwrap()["collection_outcome"], "error");
    }
    let raw = fixture(0, 0);
    let (mut p, data) = GenericProofProfile::memory(None);
    let mut bound = BoundReader::new(Cursor::new(&raw), receipt(&raw)).unwrap();
    bound.scope = scope;
    bound.pass_start = Some(p.begin(scope, ProofPhase::StrictPass1).unwrap());
    bound.profile = Some(&mut p);
    bound.read_to_end(&mut Vec::new()).unwrap();
    assert_eq!(bound.read(&mut [0; 1]).unwrap(), 0);
    assert_eq!(bound.stream_position().unwrap(), raw.len() as u64);
    assert!(!ended(&data, "strict_pass1"));
    bound.seek(SeekFrom::Start(0)).unwrap();
    assert!(ended(&data, "strict_pass1"));
    assert!(!ended(&data, "typed_pass2_decode_spool"));
    assert!(bound.seek(SeekFrom::Start(0)).is_err());
    assert!(bound.finish().is_err());
}

#[test]
fn generic_state_spool_profile_failed_rewind_and_changed_truncated_typed_pass() {
    struct Changing {
        current: Cursor<Vec<u8>>,
        next: Option<Vec<u8>>,
        wrong_seek: bool,
    }
    impl Read for Changing {
        fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
            self.current.read(b)
        }
    }
    impl Seek for Changing {
        fn seek(&mut self, p: SeekFrom) -> io::Result<u64> {
            if p == SeekFrom::Start(0) {
                if self.wrong_seek {
                    return Err(bad("injected rewind failure"));
                }
                if let Some(next) = self.next.take() {
                    self.current = Cursor::new(next);
                }
            }
            self.current.seek(p)
        }
    }
    let raw = fixture(1, 1);
    for mode in 0..3 {
        let mut next = raw.clone();
        if mode == 1 {
            let i = next.iter().position(|b| *b == b'3').unwrap();
            next[i] = b'4';
        }
        if mode == 2 {
            next.pop();
        }
        let (mut p, data) = GenericProofProfile::memory(None);
        assert!(
            decode_with_profile(
                Changing {
                    current: Cursor::new(raw.clone()),
                    next: Some(next),
                    wrong_seek: mode == 0
                },
                receipt(&raw),
                ProofScope::root(2).unwrap(),
                Some(&mut p)
            )
            .is_err()
        );
        assert!(ended(&data, "strict_pass1"));
        assert!(!ended(&data, "typed_pass2_decode_spool"));
        assert_eq!(
            events(&data)
                .iter()
                .any(|e| e["kind"] == "begin" && e["phase"] == "typed_pass2_decode_spool"),
            mode != 0
        );
        assert!(p.finish(true).is_err());
    }
}

#[test]
fn generic_state_spool_profile_corrupt_spool_and_final_flush_cannot_certify_assembly() {
    let scope = ProofScope::root(4).unwrap();
    for mode in 0..6 {
        let parsed = input(&fixture(2, 2)).unwrap();
        let dir = parsed.owner.directory.clone();
        let mut parsed = parsed;
        match mode {
            0 => {
                parsed.spool.get_mut().failed = true;
            }
            1 => {
                fs::OpenOptions::new()
                    .write(true)
                    .open(dir.join("histories.json"))
                    .unwrap()
                    .set_len(1)
                    .unwrap();
            }
            2 => {
                fs::OpenOptions::new()
                    .append(true)
                    .open(dir.join("histories.json"))
                    .unwrap()
                    .write_all(b"x")
                    .unwrap();
            }
            3 => {
                parsed.state.visibility[1].history.start += 1;
            }
            4 => {
                fs::OpenOptions::new()
                    .write(true)
                    .open(dir.join("histories.json"))
                    .unwrap()
                    .write_all(b"x")
                    .unwrap();
            }
            _ => {
                fs::rename(dir.join("histories.json"), dir.join("moved")).unwrap();
                fs::create_dir(dir.join("histories.json")).unwrap();
            }
        }
        let (mut p, data) = GenericProofProfile::memory(None);
        assert!(parsed.finish_with_profile(scope, Some(&mut p)).is_err());
        assert!(!dir.exists());
        assert_eq!(ended(&data, "spool_finalize"), !matches!(mode, 0..=2));
        assert!(!ended(&data, "generic_state_assembly") || mode == 5);
        assert!(!ended(&data, "history_cleanup"));
        assert!(p.finish(true).is_err());
    }
    // A real final buffered flush error: replace the accepted sink handle with
    // a read-only handle and leave one actual buffered byte to be flushed.
    let mut parsed = input(&fixture(1, 1)).unwrap();
    let dir = parsed.owner.directory.clone();
    parsed.spool.get_mut().sink = fs::File::open(dir.join("histories.json")).unwrap();
    parsed.spool.write_all(b" ").unwrap();
    let (mut p, data) = GenericProofProfile::memory(None);
    assert!(parsed.finish_with_profile(scope, Some(&mut p)).is_err());
    assert!(!ended(&data, "spool_finalize"));
    assert!(!dir.exists());
}

#[test]
fn generic_state_spool_profile_each_finish_boundary_cancellation_drops_owned_files() {
    let scope = ProofScope::root(4).unwrap();
    // Header then finalize/assembly/cleanup begins and ends. Fail each complete
    // publication, including an end after a successful operation.
    for record in 1..=6 {
        let parsed = input(&fixture(1, 1)).unwrap();
        let dir = parsed.owner.directory.clone();
        let (mut p, data) = GenericProofProfile::memory(Some(record));
        assert!(parsed.finish_with_profile(scope, Some(&mut p)).is_err());
        assert!(!dir.exists());
        p.assert_scratch_released();
        assert!(p.finish(true).is_err());
        assert!(
            !events(&data)
                .iter()
                .any(|e| e["collection_outcome"] == "root_collection_complete")
        );
    }
}

#[test]
fn generic_state_spool_profile_sink_fragment_interrupt_failure_bounds_and_overflow() {
    let sink = TestSink {
        fragment: Some(3),
        interrupt: true,
        ..Default::default()
    };
    let data = sink.data.clone();
    let mut p = GenericProofProfile::new(EventSink::Memory(sink)).unwrap();
    p.finish(true).unwrap();
    assert_eq!(events(&data).len(), 2);
    for mode in 0..4 {
        let sink = TestSink {
            fail_after: (mode == 0).then_some(5),
            fail_flush: mode == 1,
            fail_sync: mode == 2,
            fragment: (mode == 3).then_some(0),
            ..Default::default()
        };
        assert!(GenericProofProfile::new(EventSink::Memory(sink)).is_err());
    }
    for sync in [false, true] {
        let (mut p, data) = GenericProofProfile::memory(None);
        measured(
            &mut Some(&mut p),
            ProofScope::root(4).unwrap(),
            ProofPhase::CanonicalExport,
            |_| Ok(()),
            |_| None,
        )
        .unwrap();
        assert!(ended(&data, "canonical_export"));
        if let EventSink::Memory(sink) = &mut p.sink {
            sink.fail_flush = !sync;
            sink.fail_sync = sync;
        }
        assert!(
            measured(
                &mut Some(&mut p),
                ProofScope::root(4).unwrap(),
                ProofPhase::RewindAndBind,
                |_| Ok(()),
                |_| None
            )
            .is_err()
        );
        let bytes = data.borrow().len();
        assert!(p.finish(true).is_err());
        assert_eq!(data.borrow().len(), bytes);
        assert!(
            !events(&data)
                .iter()
                .any(|e| e["collection_outcome"] == "root_collection_complete")
        );
    }
    for mode in 0..7 {
        let (mut p, data) = GenericProofProfile::memory(None);
        match mode {
            0 => p.seq = MAX_RECORDS,
            1 => p.bytes = MAX_BYTES,
            2 => p.seq = u64::MAX,
            3 => p.bytes = u64::MAX,
            4 => p.totals[0].calls = u64::MAX,
            5 => p.totals[0].ns = u64::MAX,
            _ => p.totals[0].bytes = u64::MAX,
        }
        let mut ran = false;
        let result = measured(
            &mut Some(&mut p),
            ProofScope::root(4).unwrap(),
            ProofPhase::CanonicalExport,
            |_| {
                ran = true;
                Ok(1)
            },
            |_| Some(1),
        );
        assert!(result.is_err());
        assert_eq!(ran, mode >= 4);
        assert!(p.finish(true).is_err());
        assert!(
            !events(&data)
                .iter()
                .any(|e| e["collection_outcome"] == "root_collection_complete")
        );
    }
    let (mut p, _) = GenericProofProfile::memory(None);
    assert!(
        p.emit(serde_json::json!({"oversized": "x".repeat(MAX_LINE)}))
            .is_err()
    );
    assert!(p.finish(true).is_err());
    let (mut p, data) = GenericProofProfile::memory(None);
    let before = data.borrow().clone();
    if let EventSink::Memory(sink) = &mut p.sink {
        sink.fail_after = Some(before.len() + 7);
    }
    assert!(p.finish(true).is_err());
    let bytes = data.borrow();
    assert!(!bytes.ends_with(b"\n")); // torn tail is no complete terminal receipt
    assert_eq!(
        bytes
            .split(|b| *b == b'\n')
            .filter(|l| serde_json::from_slice::<Value>(l).is_ok())
            .count(),
        1
    );
}

#[test]
fn generic_state_spool_profile_private_paths_modes_collisions_and_retention() {
    use std::os::unix::fs::symlink;
    let owner = GenericStateProof::create().unwrap();
    let base = &owner.directory;
    for path in [
        std::path::PathBuf::from("relative"),
        base.join("."),
        base.join("child/../"),
        base.join("./child"),
        base.join("missing"),
    ] {
        assert!(GenericProofProfile::create(&path).is_err(), "{path:?}");
    }
    let plain = base.join("plain");
    fs::DirBuilder::new().mode(0o700).create(&plain).unwrap();
    let linked = base.join("linked");
    symlink(&plain, &linked).unwrap();
    assert!(GenericProofProfile::create(&linked).is_err());
    let nested = plain.join("nested");
    fs::DirBuilder::new().mode(0o700).create(&nested).unwrap();
    assert!(GenericProofProfile::create(&linked.join("nested")).is_err());
    let file = base.join("file");
    fs::write(&file, b"private sentinel").unwrap();
    assert!(GenericProofProfile::create(&file).is_err());
    for mode in [0o755, 0o770, 0o1700] {
        fs::set_permissions(&plain, fs::Permissions::from_mode(mode)).unwrap();
        assert!(GenericProofProfile::create(&plain).is_err());
    }
    fs::set_permissions(&plain, fs::Permissions::from_mode(0o700)).unwrap();
    // Preexisting sidecar through a preexisting directory is rejected in place.
    let child = nested.join("generic-proof-phase-v1");
    fs::create_dir(&child).unwrap();
    fs::write(child.join("events.ndjson"), b"sentinel").unwrap();
    assert!(GenericProofProfile::create(&nested).is_err());
    assert_eq!(fs::read(child.join("events.ndjson")).unwrap(), b"sentinel");
    let linked_child = base.join("generic-proof-phase-v1");
    symlink(&plain, &linked_child).unwrap();
    assert!(GenericProofProfile::create(base).is_err());
    fs::remove_file(linked_child).unwrap();
    let mut p = GenericProofProfile::create(base).unwrap();
    let events_path = base.join("generic-proof-phase-v1/events.ndjson");
    assert_eq!(
        fs::metadata(events_path.parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o7777,
        0o700
    );
    assert_eq!(
        fs::metadata(&events_path).unwrap().permissions().mode() & 0o7777,
        0o600
    );
    assert!(GenericProofProfile::create(base).is_err());
    p.finish(true).unwrap();
    drop(p);
    assert!(events_path.is_file());
    let scratch = input(&fixture(1, 1)).unwrap();
    drop(scratch);
    assert!(events_path.is_file());
    let failed = std::panic::catch_unwind(|| {
        let p = GenericProofProfile::create(&plain).unwrap();
        drop(p);
        panic!("test evidence survives unwinding");
    });
    assert!(failed.is_err());
    assert!(plain.join("generic-proof-phase-v1/events.ndjson").is_file());
    assert_eq!(fs::read(file).unwrap(), b"private sentinel");
}

#[test]
fn generic_state_spool_profile_terminal_write_failure_retains_only_partial_evidence() {
    for accepted in [0, 7] {
        let (mut p, data) = GenericProofProfile::memory(None);
        let before = data.borrow().clone();
        if let EventSink::Memory(sink) = &mut p.sink {
            sink.fail_after = Some(before.len() + accepted);
        }
        let error = p.finish(true).unwrap_err();
        assert_eq!(error, "injected profile write failure");
        assert!(p.failed && p.sink_failed && !p.finished);
        assert_eq!(p.seq, 1);
        assert_eq!(p.bytes, before.len() as u64);
        assert_eq!(&data.borrow()[..before.len()], before);
        assert_eq!(data.borrow().len(), before.len() + accepted);
        let retained = data.borrow().clone();
        assert!(p.finish(true).is_err());
        assert_eq!(*data.borrow(), retained);
    }
    // Even a syntactically complete JSON tail is partial evidence without the
    // final newline. This deliberately does not use events(), which accepts a
    // non-newline-terminated final JSON value and is only a test convenience.
    let prefix = b"{\"kind\":\"header\"}\n";
    let terminal = b"{\"kind\":\"terminal\",\"collection_outcome\":\"root_collection_complete\"}\n";
    let sink = TestSink {
        fail_after: Some(prefix.len() + terminal.len() - 1),
        ..Default::default()
    };
    sink.data.borrow_mut().extend_from_slice(prefix);
    let data = sink.data.clone();
    assert!(
        EventSink::Memory(sink)
            .publish_terminal(terminal, prefix.len() as u64)
            .is_err()
    );
    let retained = data.borrow();
    assert_eq!(&retained[..prefix.len()], prefix);
    assert!(serde_json::from_slice::<Value>(&retained[prefix.len()..]).is_ok());
    let complete: Vec<_> = retained
        .split_inclusive(|b| *b == b'\n')
        .filter(|line| line.ends_with(b"\n"))
        .collect();
    assert_eq!(complete, [prefix.as_slice()]);
}

#[test]
fn generic_state_spool_profile_terminal_full_line_flush_sync_failure_rolls_back() {
    for complete in [false, true] {
        for sync in [false, true] {
            let (mut p, data) = GenericProofProfile::memory(None);
            measured(
                &mut Some(&mut p),
                ProofScope::root(4).unwrap(),
                ProofPhase::CanonicalExport,
                |_| Ok(()),
                |_| None,
            )
            .unwrap();
            let before = data.borrow().clone();
            let seq = p.seq;
            if let EventSink::Memory(sink) = &mut p.sink {
                sink.fail_flush = !sync;
                sink.fail_sync = sync;
            }
            let error = p.finish(complete).unwrap_err();
            assert_eq!(
                error,
                if sync {
                    "injected profile sync failure"
                } else {
                    "injected profile flush failure"
                }
            );
            let EventSink::Memory(sink) = &mut p.sink else {
                unreachable!()
            };
            assert_eq!(&sink.failed_commit[..before.len()], before);
            let attempted = &sink.failed_commit[before.len()..];
            assert!(attempted.ends_with(b"\n"));
            let terminal: Value = serde_json::from_slice(attempted).unwrap();
            assert_eq!(terminal["kind"], "terminal");
            assert_eq!(terminal["seq"], seq);
            assert_eq!(
                terminal["collection_outcome"],
                if complete {
                    "root_collection_complete"
                } else {
                    "error"
                }
            );
            sink.fail_flush = false;
            sink.fail_sync = false;
            assert_eq!(*data.borrow(), before);
            assert!(ended(&data, "canonical_export"));
            assert_eq!(events(&data).len(), 3);
            assert!(p.failed && p.sink_failed && !p.finished);
            assert_eq!(p.seq, seq);
            assert_eq!(p.bytes, before.len() as u64);
            assert!(p.finish(true).unwrap_err().contains("poisoned"));
            assert_eq!(*data.borrow(), before);
        }
    }
}

#[test]
fn generic_state_spool_profile_terminal_rollback_failure_preserves_primary_and_poison() {
    for sync in [false, true] {
        for truncate in [false, true] {
            let (mut p, data) = GenericProofProfile::memory(None);
            let before = data.borrow().clone();
            if let EventSink::Memory(sink) = &mut p.sink {
                sink.fail_flush = !sync;
                sink.fail_sync = sync;
                sink.fail_rollback_truncate = truncate;
                sink.fail_rollback_sync = !truncate;
            }
            let error = p.finish(true).unwrap_err();
            let primary = if sync {
                "injected profile sync failure"
            } else {
                "injected profile flush failure"
            };
            let rollback = if truncate {
                "injected terminal truncate failure"
            } else {
                "injected terminal rollback sync failure"
            };
            assert_eq!(
                error,
                format!("{primary}; terminal rollback failed: {rollback}")
            );
            assert_eq!(&data.borrow()[..before.len()], before);
            if truncate {
                // Recovery failed: a terminal can remain, but native failure
                // forbids accepting it alone as a successful collection.
                assert_eq!(events(&data).last().unwrap()["kind"], "terminal");
            } else {
                assert_eq!(*data.borrow(), before);
            }
            assert!(p.failed && p.sink_failed && !p.finished);
            assert_eq!(p.seq, 1);
            assert_eq!(p.bytes, before.len() as u64);
            let retained = data.borrow().clone();
            assert!(p.finish(true).is_err());
            assert_eq!(*data.borrow(), retained);
        }
    }
}

#[test]
fn generic_state_spool_profile_terminal_owned_file_rollback_preserves_committed_records() {
    let owner = GenericStateProof::create().unwrap();
    let mut p = GenericProofProfile::create(&owner.directory).unwrap();
    measured(
        &mut Some(&mut p),
        ProofScope::root(4).unwrap(),
        ProofPhase::CanonicalExport,
        |_| Ok(()),
        |_| None,
    )
    .unwrap();
    let path = owner.directory.join("generic-proof-phase-v1/events.ndjson");
    let before = fs::read(&path).unwrap();
    assert_eq!(before.len() as u64, p.bytes);
    // Exercise the actual owned-file recovery on a fully accepted terminal,
    // separately from the deterministic flush/sync fault injection above.
    p.sink
        .write_line(
            b"{\"kind\":\"terminal\",\"collection_outcome\":\"root_collection_complete\"}\n",
        )
        .unwrap();
    assert!(fs::metadata(&path).unwrap().len() > p.bytes);
    p.sink.rollback_terminal(p.bytes).unwrap();
    drop(p);
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o7777,
        0o600
    );
    let mut read_only = EventSink::File(fs::File::open(&path).unwrap());
    assert!(read_only.rollback_terminal(before.len() as u64).is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
}
