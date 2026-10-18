//! Small whole-Value/old-Vec oracles only; no timing or resource-fit claim.
use super::*;
use std::{io::Cursor, os::unix::fs::PermissionsExt};
fn bytes(write: impl FnOnce(&mut dyn Write) -> Result<(), String>) -> Vec<u8> {
    let mut out = vec![];
    write(&mut out).unwrap();
    out
}
fn public() -> PermittedView {
    match_root().state.visibility[0]
        .history
        .last()
        .unwrap()
        .to_public()
}
#[test]
fn search_spool_every_prefix_and_result_matches_full_old_vec_and_generic_actual_wrappers() {
    let game = match_root();
    let directory = Directory::new().unwrap();
    let query = Query::Search {
        viewer: PlayerId(0),
        sampling_seed: "spool:雪\n\u{0}".into(),
    };
    let mut work = game.start_work(query.clone()).unwrap();
    let mut reference = Reference::new(&work, &directory).unwrap();
    let mut checkpoints = 0;
    loop {
        // Independent full original array retained ONLY in this bounded test.
        let old = game.state.visibility[0]
            .history
            .iter()
            .take(reference.history.len())
            .map(|v| v.to_public())
            .collect::<Vec<_>>();
        let full = OldData {
            query: &reference.query,
            task: OldTask::Search {
                builder: &reference.builder,
                observation: &reference.observation,
                history: &old,
                decks: &reference.decks,
            },
            units_done: reference.units_done,
        };
        let expected = canonical(&full).unwrap();
        let meta = reference.data().unwrap();
        assert_eq!(
            bytes(
                |out| replaced(&meta, &["task", "history"], out, &mut |out| reference
                    .history
                    .write_array(out))
            ),
            expected.as_bytes()
        );
        let actual_meta = actual_data(&work.data).unwrap();
        let Task::Search { history, .. } = &work.data.task else {
            unreachable!()
        };
        assert_eq!(
            bytes(
                |out| replaced(&actual_meta, &["task", "history"], out, &mut |out| {
                    write_actual_history(history, out)
                })
            ),
            canonical(&work.data).unwrap().as_bytes()
        );
        assert_eq!(canonical(&work.data).unwrap(), expected);
        scratch(&directory, &mut reference, &work.data).unwrap();
        if reference.history.len() == 1 || (reference.history.len() == 2 && checkpoints == 1) {
            let saved = game.save_work(&work).unwrap();
            let meta = serde_json::json!({"work_version":15,"build_id":BUILD_ID,"catalog_digest":game.prepared.catalog_digest,"state_sha256":game.state_digest().unwrap(),"data":reference.data().unwrap()});
            assert_eq!(
                bytes(
                    |out| replaced(&meta, &["data", "task", "history"], out, &mut |out| {
                        reference.history.write_array(out)
                    })
                ),
                saved.as_bytes()
            );
            work = game.restore_work(&saved).unwrap();
            checkpoints += 1;
        }
        let done = reference.resume(&game, Budget::new(1)).unwrap();
        match game.resume_work(work, Budget::new(1)) {
            QueryResult::Incomplete { work: next, .. } => {
                assert!(!done);
                work = next;
            }
            QueryResult::Complete(QueryValue::SearchContext(context)) => {
                assert!(done);
                assert!(checkpoints >= 2);
                let old = game.state.visibility[0]
                    .history
                    .iter()
                    .map(|v| v.to_public())
                    .collect::<Vec<_>>();
                let Query::Search { sampling_seed, .. } = &query else {
                    unreachable!()
                };
                let full = serde_json::json!({"context":OldContext {context_version:16,observation:reference.observation.as_deref().unwrap(),permitted_history:&old,open_decklists:&reference.decks,catalog_digest:&game.prepared.catalog_digest,profile_id:&game.prepared.config.profile,sampling_seed}});
                let old_meta = reference.result(&game).unwrap();
                let new_meta = actual_result(&context).unwrap();
                let expected = canonical(&full).unwrap();
                assert_eq!(
                    bytes(|out| replaced(
                        &old_meta,
                        &["context", "permitted_history"],
                        out,
                        &mut |out| reference.history.write_array(out)
                    )),
                    expected.as_bytes()
                );
                let actual = bytes(|out| {
                    replaced(
                        &new_meta,
                        &["context", "permitted_history"],
                        out,
                        &mut |out| write_actual_history(&context.permitted_history, out),
                    )
                });
                assert_eq!(actual, expected.as_bytes());
                assert_eq!(
                    actual,
                    canonical(&query_value(QueryValue::SearchContext(context)))
                        .unwrap()
                        .as_bytes()
                );
                break;
            }
            _ => panic!("original completion unit changed"),
        }
    }
}
#[test]
fn search_spool_complete_byte_comparison_rejects_edits_reorder_omission_truncation_append_and_io_faults()
 {
    let mut a = public();
    a.objects[0].oracle_id = "generic:雪\n\u{0}".into();
    let mut b = a.clone();
    b.turn.number += 7;
    let expected = canonical(&vec![a.clone(), b.clone()]).unwrap();
    for variant in 0..5 {
        let directory = Directory::new().unwrap();
        let mut spool = HistorySpool::new(&directory).unwrap();
        spool.append(&a).unwrap();
        spool.append(&b).unwrap();
        match variant {
            0 => {
                spool.file.seek(SeekFrom::Start(1)).unwrap();
                spool.file.write_all(b"X").unwrap();
            }
            1 => {
                spool
                    .file
                    .set_len(spool.file.metadata().unwrap().len() - 1)
                    .unwrap();
            }
            2 => {
                spool.file.seek(SeekFrom::End(0)).unwrap();
                spool.file.write_all(b"x").unwrap();
            }
            3 => {
                spool.ends.pop();
            }
            4 => {
                spool.file.set_len(0).unwrap();
                spool.ends.clear();
                spool.append(&b).unwrap();
                spool.append(&a).unwrap();
            }
            _ => unreachable!(),
        }
        assert!(
            compare_bytes(
                &directory,
                |out| spool.write_array(out),
                |out| out.write_all(expected.as_bytes()).map_err(text)
            )
            .is_err(),
            "corruption {variant}"
        );
    }
    let directory = Directory::new().unwrap();
    let mut spool = HistorySpool::new(&directory).unwrap();
    spool.file = File::open(directory.0.join("history.json-fragments")).unwrap();
    assert!(spool.append(&a).is_err());
    assert!(spool.poisoned);
    assert_eq!(spool.len(), 0);
    assert!(spool.write_array(&mut vec![]).is_err());
    for (reference, actual) in [
        (b"abc".as_slice(), b"ab".as_slice()),
        (b"ab", b"abc"),
        (b"abc", b"axc"),
    ] {
        assert!(
            compare_bytes(
                &directory,
                |out| out.write_all(reference).map_err(text),
                |out| out.write_all(actual).map_err(text)
            )
            .is_err()
        );
    }
    assert!(compare_bytes(&directory, |_| Err("reference I/O".into()), |_| Ok(())).is_err());
    assert!(
        compare_bytes(
            &directory,
            |out| out.write_all(b"abc").map_err(text),
            |out| {
                out.write_all(b"a").map_err(text)?;
                Err("actual writer failure".into())
            }
        )
        .is_err()
    );
}
#[test]
fn search_spool_replacement_requires_exact_once_empty_path_and_keeps_every_other_field() {
    let skeleton = serde_json::json!({"z":null,"task":{"history":[],"a":{"escape":"雪\n","number":-7}},"query":{"seed":"x"},"units_done":13});
    let actual = bytes(|out| {
        replaced(&skeleton, &["task", "history"], out, &mut |out| {
            out.write_all(b"[1,2,3]").map_err(text)
        })
    });
    let mut full = skeleton.clone();
    full["task"]["history"] = serde_json::json!([1, 2, 3]);
    assert_eq!(actual, canonical(&full).unwrap().as_bytes());
    for bad in [
        serde_json::json!({}),
        serde_json::json!({"task":{}}),
        serde_json::json!({"task":{"history":[1]}}),
        serde_json::json!({"task":{"history":null}}),
    ] {
        assert!(replaced(&bad, &["task", "history"], &mut vec![], &mut |_| Ok(())).is_err());
    }
    let mut guard = Replacement::default();
    assert!(guard.finish().is_err());
    guard.hit().unwrap();
    assert!(guard.hit().is_err());
    assert!(guard.finish().is_err());
}
struct Short {
    data: Cursor<Vec<u8>>,
    interrupt: bool,
    fail: bool,
}
impl Read for Short {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.interrupt {
            self.interrupt = false;
            return Err(io::Error::from(io::ErrorKind::Interrupted));
        }
        if self.fail {
            return Err(io::Error::other("injected read"));
        }
        let size = out.len().min(2);
        self.data.read(&mut out[..size])
    }
}
#[test]
fn search_spool_short_reads_interrupts_poison_counts_and_flush_are_fail_closed() {
    let mut equal = Equal::new(Short {
        data: Cursor::new(b"abcdef".to_vec()),
        interrupt: true,
        fail: false,
    });
    equal.write_all(b"abcdef").unwrap();
    equal.flush().unwrap();
    assert_eq!(equal.finish().unwrap(), (6, sha256(b"abcdef")));
    let mut equal = Equal::new(Short {
        data: Cursor::new(vec![]),
        interrupt: false,
        fail: true,
    });
    assert!(equal.write_all(b"x").is_err());
    assert!(equal.write(b"").is_err());
    assert!(equal.flush().is_err());
    assert!(equal.finish().is_err());
    let mut equal = Equal::new(Cursor::new(b"x"));
    equal.bytes = u64::MAX;
    assert!(equal.write_all(b"x").is_err());
    assert!(equal.finish().is_err());
    struct Over;
    impl Read for Over {
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            Ok(out.len() + 1)
        }
    }
    assert!(Equal::new(Over).write_all(b"x").is_err());
    struct FailFlush;
    impl Write for FailFlush {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            Ok(b.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::other("injected flush"))
        }
    }
    let mut writer = Counted {
        inner: FailFlush,
        hash: Sha256Sink::new(),
        bytes: 0,
    };
    writer.write_all(b"abc").unwrap();
    assert!(writer.flush().is_err());
    assert_eq!(writer.bytes, 3);
    let mut reader = Short {
        data: Cursor::new(b"ab".to_vec()),
        interrupt: true,
        fail: false,
    };
    assert!(copy_exact(&mut reader, &mut vec![], 3).is_err());
}
#[test]
fn search_spool_private_files_cleanup_on_success_error_and_unwind() {
    for panic in [false, true] {
        let mut path = None;
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let directory = Directory::new().unwrap();
            path = Some(directory.0.clone());
            assert_eq!(
                fs::metadata(&directory.0).unwrap().permissions().mode() & 0o777,
                0o700
            );
            let mut spool = HistorySpool::new(&directory).unwrap();
            assert_eq!(
                spool.file.metadata().unwrap().permissions().mode() & 0o777,
                0o600
            );
            spool.append(&public()).unwrap();
            assert!(
                compare_bytes(
                    &directory,
                    |out| spool.write_array(out),
                    |_| Err("error".into())
                )
                .is_err()
            );
            if panic {
                panic!("expected cleanup domain");
            }
        }));
        assert!(!path.unwrap().exists());
    }
}

#[test]
fn snapshot_file_receipt_rewinds_only_after_complete_eof_and_never_on_position_query() {
    let raw = b"{\"a\":1}";
    let mut reader = ReceiptRead::new(Cursor::new(raw)).unwrap();
    let mut bytes = vec![];
    reader.read_to_end(&mut bytes).unwrap();
    let count = reader.bytes;
    assert_eq!(reader.stream_position().unwrap(), count);
    assert_eq!(reader.bytes, count);
    assert!(reader.eof);
    reader.seek(SeekFrom::Start(0)).unwrap();
    assert_eq!(reader.bytes, 0);
    assert!(!reader.eof);
    assert!(reader.first.is_some());
    let value: serde_json::Value = serde_json::from_reader(&mut reader).unwrap();
    assert_eq!(value, serde_json::json!({"a":1}));
    assert_eq!(reader.finish().unwrap(), (raw.len() as u64, sha256(raw)));
    let mut reader = ReceiptRead::new(Cursor::new(raw)).unwrap();
    reader.read_exact(&mut [0; 1]).unwrap();
    assert!(reader.seek(SeekFrom::Start(0)).is_err());
    assert!(reader.finish().is_err());
    let mut reader = ReceiptRead::new(Cursor::new(raw)).unwrap();
    reader.read_to_end(&mut vec![]).unwrap();
    assert!(reader.finish().is_err());
    let mut reader = ReceiptRead::new(Cursor::new(raw)).unwrap();
    reader.read_to_end(&mut vec![]).unwrap();
    reader.seek(SeekFrom::Start(0)).unwrap();
    reader.read_to_end(&mut vec![]).unwrap();
    assert!(reader.seek(SeekFrom::Start(0)).is_err());
    assert!(reader.finish().is_err());
}
#[test]
fn snapshot_file_receipt_errors_changed_second_pass_and_incomplete_decode_cannot_succeed() {
    let mut reader = ReceiptRead::new(Cursor::new(b"{\"a\":1}".to_vec())).unwrap();
    reader.read_to_end(&mut vec![]).unwrap();
    reader.source.get_mut()[5] = b'2';
    reader.seek(SeekFrom::Start(0)).unwrap();
    reader.read_to_end(&mut vec![]).unwrap();
    assert!(reader.finish().is_err());
    struct Bad {
        read_error: bool,
    }
    impl Read for Bad {
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            if self.read_error {
                Err(io::Error::other("read"))
            } else {
                Ok(out.len() + 1)
            }
        }
    }
    impl Seek for Bad {
        fn seek(&mut self, p: SeekFrom) -> io::Result<u64> {
            if matches!(p, SeekFrom::Current(0)) {
                Ok(0)
            } else {
                Err(io::Error::other("seek"))
            }
        }
    }
    for read_error in [false, true] {
        let mut reader = ReceiptRead::new(Bad { read_error }).unwrap();
        assert!(reader.read(&mut [0; 1]).is_err());
        assert!(reader.finish().is_err());
    }
    struct SeekFault {
        inner: Cursor<Vec<u8>>,
        wrong_position: bool,
    }
    impl Read for SeekFault {
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            self.inner.read(out)
        }
    }
    impl Seek for SeekFault {
        fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
            if matches!(position, SeekFrom::Start(0)) {
                if self.wrong_position {
                    return Ok(7);
                }
                return Err(io::Error::other("injected rewind failure"));
            }
            self.inner.seek(position)
        }
    }
    for wrong_position in [false, true] {
        let mut reader = ReceiptRead::new(SeekFault {
            inner: Cursor::new(b"{}".to_vec()),
            wrong_position,
        })
        .unwrap();
        reader.read_to_end(&mut vec![]).unwrap();
        assert!(reader.seek(SeekFrom::Start(0)).is_err());
        assert!(reader.finish().is_err());
    }
    struct Interrupt {
        inner: Cursor<Vec<u8>>,
        pending: bool,
    }
    impl Read for Interrupt {
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            if self.pending {
                self.pending = false;
                return Err(io::Error::from(io::ErrorKind::Interrupted));
            }
            self.inner.read(out)
        }
    }
    impl Seek for Interrupt {
        fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
            self.inner.seek(position)
        }
    }
    let mut reader = ReceiptRead::new(Interrupt {
        inner: Cursor::new(b"{}".to_vec()),
        pending: true,
    })
    .unwrap();
    assert_eq!(
        strict_read_seekable::<serde_json::Value, _>(&mut reader).unwrap(),
        serde_json::json!({})
    );
    assert_eq!(reader.finish().unwrap(), (2, sha256(b"{}")));
    let mut reader = ReceiptRead::new(Cursor::new(b"x")).unwrap();
    reader.bytes = u64::MAX;
    assert!(reader.read(&mut [0; 1]).is_err());
    assert!(reader.finish().is_err());
    let mut reader = ReceiptRead::new(Cursor::new(b"{\"a\":".to_vec())).unwrap();
    assert!(strict_read_seekable::<serde_json::Value, _>(&mut reader).is_err());
    assert!(reader.finish().is_err());
}
#[test]
fn snapshot_file_restore_matches_bounded_original_string_reader_full_state_and_trace() {
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        let prepared = Prepared::admit(
            Catalog::read(include_str!("../../../../tests/fixtures/expected-ir.json")).unwrap(),
            ScenarioConfig::land_return(),
        )
        .unwrap();
        let mut game = Game::with_trace(prepared.clone(), mode);
        let Advance::AwaitingDecision(request) = game.advance(Budget::unlimited()) else {
            panic!("priority")
        };
        game.respond(request.player(), request.decision_id(), Response::Pass)
            .unwrap();
        // The old whole String oracle is deliberately retained ONLY here.
        let old = Game::restore(prepared, &game.snapshot().unwrap()).unwrap();
        let streamed = restore_snapshot_from_private_file(&game).unwrap();
        assert_eq!(streamed.state, old.state);
        assert_eq!(streamed.trace, old.trace);
        assert_eq!(streamed.state, game.state);
        assert_eq!(streamed.replay_journal(), old.replay_journal());
        assert_eq!(
            streamed.state_digest().unwrap(),
            game.state_digest().unwrap()
        );
    }
}

#[test]
fn search_spool_plain_ancestry_exclusive_names_and_explicit_success_cleanup() {
    let directory = Directory::new().unwrap();
    let path = directory.0.clone();
    assert!(Directory::under(Path::new("relative")).is_err());
    assert!(Directory::under(&path.join("..")).is_err());
    let link = directory.0.join("linked-parent");
    std::os::unix::fs::symlink(&directory.0, &link).unwrap();
    assert!(Directory::under(&link).is_err());
    assert!(directory.create("../escape").is_err());
    let mut outside = directory.create("outside").unwrap();
    outside.write_all(b"untouched marker").unwrap();
    outside.flush().unwrap();
    drop(outside);
    std::os::unix::fs::symlink(
        directory.0.join("outside"),
        directory.0.join("reference.json"),
    )
    .unwrap();
    assert!(compare_bytes(&directory, |_| Ok(()), |_| Ok(())).is_err());
    assert_eq!(
        fs::read(directory.0.join("outside")).unwrap(),
        b"untouched marker"
    );
    directory.cleanup().unwrap();
    assert!(!path.exists());
    let directory = Directory::new().unwrap();
    let path = directory.0.clone();
    let file = directory.create("proof").unwrap();
    drop(file);
    fs::remove_dir_all(&path).unwrap();
    assert!(directory.cleanup().is_err());
}
