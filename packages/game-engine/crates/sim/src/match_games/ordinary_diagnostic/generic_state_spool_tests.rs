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
