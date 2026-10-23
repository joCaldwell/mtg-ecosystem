//! Small whole-Value/old-Vec oracles only; no timing or resource-fit claim.
use super::*;
use std::{io::Cursor, os::unix::fs::PermissionsExt};

#[test]
fn search_spool_compare_bytes_accepts_fragmented_empty_block_edges_and_multiblock_streams() {
    let directory = Directory::new().unwrap();
    let path = directory.0.clone();
    for length in [0, 1, BLOCK - 1, BLOCK, BLOCK + 1, 2 * BLOCK + 37] {
        let expected = (0..length)
            .map(|i| ((i % 251) * 37 % 251) as u8)
            .collect::<Vec<_>>();
        for fragment in [1, 13, BLOCK + 7] {
            compare_bytes(
                &directory,
                |out| out.write_all(&expected).map_err(text),
                |out| {
                    for part in expected.chunks(fragment) {
                        out.write_all(part).map_err(text)?;
                    }
                    Ok(())
                },
            )
            .unwrap_or_else(|e| panic!("length {length}, fragment {fragment}: {e}"));
        }
    }
    directory.cleanup().unwrap();
    assert!(!path.exists());
}

#[test]
fn search_spool_compare_bytes_checks_partial_final_buffer_before_complete_eof() {
    let directory = Directory::new().unwrap();
    let expected = vec![b'a'; 2 * BLOCK + 37];
    for (variant, error) in [
        (0, "complete query bytes differ"),
        (1, "actual ended before complete reference EOF"),
        (2, "actual contains bytes beyond reference EOF"),
    ] {
        let mut actual = expected.clone();
        match variant {
            0 => *actual.last_mut().unwrap() = b'b',
            1 => {
                actual.pop();
            }
            2 => actual.push(b'a'),
            _ => unreachable!(),
        }
        let mut serialized = false;
        let result = compare_bytes(
            &directory,
            |out| out.write_all(&expected).map_err(text),
            |out| {
                for part in actual.chunks(BLOCK) {
                    out.write_all(part).map_err(text)?;
                }
                // These writes succeed: only the checked final flush/finish
                // can expose the late mismatch or stream length difference.
                serialized = true;
                Ok(())
            },
        );
        assert!(serialized);
        assert_eq!(result.unwrap_err(), error);
    }
    for (reference, actual, error) in [
        (
            b"".as_slice(),
            b"x".as_slice(),
            "actual contains bytes beyond reference EOF",
        ),
        (
            b"x".as_slice(),
            b"".as_slice(),
            "actual ended before complete reference EOF",
        ),
    ] {
        assert_eq!(
            compare_bytes(
                &directory,
                |out| out.write_all(reference).map_err(text),
                |out| out.write_all(actual).map_err(text),
            )
            .unwrap_err(),
            error
        );
    }
}

#[test]
fn search_spool_compare_bytes_ignored_write_or_flush_errors_remain_poisoned() {
    let directory = Directory::new().unwrap();
    let expected = vec![b'a'; BLOCK + 37];
    let wrong_block = vec![b'b'; BLOCK];
    for explicit_flush in [false, true] {
        for write_after_error in [false, true] {
            let result = compare_bytes(
                &directory,
                |out| out.write_all(&expected).map_err(text),
                |out| {
                    let error = if explicit_flush {
                        out.write_all(b"b").unwrap();
                        out.flush().unwrap_err()
                    } else {
                        out.write_all(&wrong_block).unwrap_err()
                    };
                    assert_eq!(text(error), "complete query bytes differ");
                    if write_after_error {
                        // Buffer acceptance cannot clear the inner poison.
                        out.write_all(b"a").unwrap();
                    }
                    Ok(()) // Deliberately ignore the failed write/flush.
                },
            );
            assert_eq!(result.unwrap_err(), "poisoned byte comparison");
        }
    }
}

#[test]
fn search_spool_compare_bytes_serializer_errors_take_precedence_over_deferred_flush_errors() {
    let directory = Directory::new().unwrap();
    for wrong_tail in [false, true] {
        for explicit_flush in [false, true] {
            let result = compare_bytes(
                &directory,
                |out| out.write_all(b"abc").map_err(text),
                |out| {
                    out.write_all(if wrong_tail { b"abx" } else { b"abc" })
                        .unwrap();
                    if explicit_flush {
                        assert_eq!(out.flush().is_err(), wrong_tail);
                    }
                    Err("actual serializer failure".into())
                },
            );
            assert_eq!(result.unwrap_err(), "actual serializer failure");
        }
    }
    let mut actual_called = false;
    assert_eq!(
        compare_bytes(
            &directory,
            |out| {
                out.write_all(b"abc").unwrap();
                Err("reference serializer failure".into())
            },
            |_| {
                actual_called = true;
                Ok(())
            },
        )
        .unwrap_err(),
        "reference serializer failure"
    );
    assert!(!actual_called);
}

#[test]
fn search_spool_compare_bytes_requires_original_complete_count_sha_and_reference_eof() {
    let directory = Directory::new().unwrap();
    for (changed_reference, actual, error) in [
        (
            b"xyz".as_slice(),
            b"xyz".as_slice(),
            "complete comparison count/SHA mismatch",
        ),
        (
            b"ab".as_slice(),
            b"ab".as_slice(),
            "complete comparison count/SHA mismatch",
        ),
        (
            b"abcx".as_slice(),
            b"abc".as_slice(),
            "actual ended before complete reference EOF",
        ),
    ] {
        assert_eq!(
            compare_bytes(
                &directory,
                |out| out.write_all(b"abc").map_err(text),
                |out| {
                    // Change the already-open file after its initial length
                    // check. Equal bytes alone must not certify the receipt.
                    fs::write(directory.0.join("reference.json"), changed_reference)
                        .map_err(text)?;
                    out.write_all(actual).map_err(text)
                },
            )
            .unwrap_err(),
            error
        );
    }
}

#[test]
fn search_spool_compare_bytes_buffered_failures_and_unwind_cleanup_private_files() {
    for unwind in [false, true] {
        let mut path = None;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let directory = Directory::new().unwrap();
            path = Some(directory.0.clone());
            let result = compare_bytes(
                &directory,
                |out| out.write_all(b"abc").map_err(text),
                |out| {
                    out.write_all(b"abx").unwrap();
                    if unwind {
                        panic!("buffered serializer unwind");
                    }
                    Ok(())
                },
            );
            assert_eq!(result.unwrap_err(), "complete query bytes differ");
        }));
        assert_eq!(result.is_err(), unwind);
        assert!(!path.unwrap().exists());
    }
}

#[test]
fn search_spool_reused_scratch_keeps_fragment_boundaries_complete_bytes_and_sha() {
    for length in [0, 1, BLOCK - 1, BLOCK, BLOCK + 1, 2 * BLOCK + 37] {
        let expected = (0..length)
            .map(|i| ((i % 251) * 37 % 251) as u8)
            .collect::<Vec<_>>();
        for fragment in [1, 13, BLOCK + 7] {
            let mut equal = Equal::new(Cursor::new(expected.as_slice()));
            for part in expected.chunks(fragment) {
                equal.write_all(part).unwrap();
            }
            equal.flush().unwrap();
            assert_eq!(
                equal.finish().unwrap(),
                (u64::try_from(length).unwrap(), sha256(&expected)),
                "length {length}, fragment {fragment}"
            );
        }
    }
}

#[test]
fn search_spool_reused_scratch_short_reads_never_compare_stale_tails_or_recover_errors() {
    struct Choppy {
        data: Cursor<Vec<u8>>,
        calls: usize,
    }
    impl Read for Choppy {
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            self.calls += 1;
            if self.calls % 11 == 1 {
                return Err(io::ErrorKind::Interrupted.into());
            }
            let n = out.len().min(self.calls % 17 + 1);
            // Only the returned prefix is overwritten; all other scratch bytes
            // deliberately retain contents from earlier fragments and reads.
            self.data.read(&mut out[..n])
        }
    }
    let expected = (0..2 * BLOCK + 37)
        .map(|i| ((i % 251) * 37 % 251) as u8)
        .collect::<Vec<_>>();
    for variant in 0..6 {
        let mut actual = expected.clone();
        match variant {
            0 => {}
            1 => actual[BLOCK - 1] ^= 1,
            2 => actual[BLOCK] ^= 1,
            3 => *actual.last_mut().unwrap() ^= 1,
            4 => {
                actual.pop();
            }
            5 => actual.push(99),
            _ => unreachable!(),
        }
        let mut equal = Equal::new(Choppy {
            data: Cursor::new(expected.clone()),
            calls: 0,
        });
        let failed_write = actual
            .chunks(193)
            .any(|part| equal.write_all(part).is_err());
        if failed_write {
            assert_ne!(variant, 0);
            assert!(equal.write(b"").is_err());
            assert!(equal.flush().is_err());
            assert!(equal.finish().is_err());
        } else if variant == 0 {
            assert_eq!(
                equal.finish().unwrap(),
                (u64::try_from(expected.len()).unwrap(), sha256(&expected))
            );
        } else {
            assert!(equal.finish().is_err(), "variant {variant}");
        }
    }
    let mut equal = Equal::new(Short {
        data: Cursor::new(b"abcdef".to_vec()),
        interrupt: false,
        fail: false,
    });
    equal.write_all(b"ab").unwrap();
    equal.reader.fail = true;
    assert!(equal.write_all(b"cd").is_err());
    assert!(equal.write(b"").is_err());
    assert!(equal.flush().is_err());
    assert!(equal.finish().is_err());
}

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

// Local injected sink/clock only: no process-wide environment or observer state.
#[derive(Default)]
struct ProfileIo {
    bytes: Vec<u8>,
    writes: u64,
    flushes: u64,
    syncs: u64,
    clocks: u64,
    short: bool,
    interrupt: bool,
    zero: bool,
    fail_write: Option<u64>,
    fail_flush: bool,
    fail_sync: bool,
}
#[derive(Clone, Default)]
struct ProfileSink(std::rc::Rc<std::cell::RefCell<ProfileIo>>);
impl Write for ProfileSink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let mut io = self.0.borrow_mut();
        io.writes += 1;
        if io.interrupt {
            io.interrupt = false;
            return Err(io::ErrorKind::Interrupted.into());
        }
        if io.fail_write == Some(io.writes) {
            return Err(io::Error::other("private injected write error"));
        }
        if io.zero {
            return Ok(0);
        }
        let n = if io.short {
            bytes.len().min(7)
        } else {
            bytes.len()
        };
        io.bytes.extend_from_slice(&bytes[..n]);
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        let mut io = self.0.borrow_mut();
        io.flushes += 1;
        if io.fail_flush {
            Err(io::Error::other("private injected flush error"))
        } else {
            Ok(())
        }
    }
}
impl ProofSink for ProfileSink {
    fn sync(&mut self) -> io::Result<()> {
        let mut io = self.0.borrow_mut();
        io.syncs += 1;
        if io.fail_sync {
            Err(io::Error::other("private injected sync error"))
        } else {
            Ok(())
        }
    }
    fn rollback_terminal(&mut self, committed_bytes: u64) -> io::Result<()> {
        self.0
            .borrow_mut()
            .bytes
            .truncate(usize::try_from(committed_bytes).map_err(io::Error::other)?);
        Ok(())
    }
}
fn memory_profile() -> (LegalProofProfile, ProfileSink) {
    let sink = ProfileSink::default();
    let clock = sink.clone();
    let p = LegalProofProfile::new(
        Box::new(sink.clone()),
        Box::new(move || {
            let mut io = clock.0.borrow_mut();
            io.clocks += 1;
            Ok(io.clocks)
        }),
        "ordinary_4",
    )
    .unwrap();
    (p, sink)
}
fn profile_records(sink: &ProfileSink) -> Vec<serde_json::Value> {
    let io = sink.0.borrow();
    assert!(io.bytes.ends_with(b"\n"));
    io.bytes
        .split_inclusive(|b| *b == b'\n')
        .enumerate()
        .map(|(seq, line)| {
            assert!(line.len() <= PROFILE_LINE);
            let record: serde_json::Value = serde_json::from_slice(line).unwrap();
            assert_eq!(record["seq"], seq);
            assert_eq!(record["schema"], PROFILE_SCHEMA);
            record
        })
        .collect()
}
fn phase_ok(records: &[serde_json::Value], phase: &str) -> bool {
    records
        .iter()
        .any(|r| r["kind"] == "end" && r["phase"] == phase && r["outcome"] == "ok")
}

#[test]
fn search_spool_profile_none_has_zero_clock_sink_activity_and_preserves_query_restore_bytes() {
    let game = match_root();
    let before = (
        game.snapshot().unwrap(),
        game.state_digest().unwrap(),
        canonical(&game.trace).unwrap(),
        canonical(&game.replay_journal()).unwrap(),
    );
    let (mut profile, sink) = memory_profile();
    let writes = sink.0.borrow().writes;
    super::super::compare_queries_with_profile(&game, None);
    let plain = restore_snapshot_from_private_file_with_profile(&game, None).unwrap();
    assert_eq!(sink.0.borrow().clocks, 0);
    assert_eq!(sink.0.borrow().writes, writes);
    assert_eq!(sink.0.borrow().syncs, 0);
    super::super::compare_queries_with_profile(&game, Some(&mut profile));
    profile.root(ProofKind::Root);
    let observed =
        restore_snapshot_from_private_file_with_profile(&game, Some(&mut profile)).unwrap();
    profile.finish(true).unwrap();
    assert_eq!(plain.state, observed.state);
    assert_eq!(plain.snapshot().unwrap(), observed.snapshot().unwrap());
    assert_eq!(plain.trace, observed.trace);
    assert_eq!(plain.replay_journal(), observed.replay_journal());
    assert_eq!(
        before,
        (
            game.snapshot().unwrap(),
            game.state_digest().unwrap(),
            canonical(&game.trace).unwrap(),
            canonical(&game.replay_journal()).unwrap()
        )
    );
    let records = profile_records(&sink);
    assert_eq!(
        records.last().unwrap()["collection_outcome"],
        "root_collection_complete"
    );
    assert!(phase_ok(
        &records,
        "read_receipt_and_state_digest_validation"
    ));
    // Exact schema key sets keep semantic payloads and arbitrary errors out.
    for record in &records {
        let mut keys = record
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>();
        keys.sort_unstable();
        let expected: &[&str] = match record["kind"].as_str().unwrap() {
            "header" => &[
                "build_id",
                "collection_scope",
                "instrumentation",
                "kind",
                "schema",
                "seq",
            ],
            "begin" | "end" => &[
                "at_ns",
                "bytes",
                "duration_ns",
                "kind",
                "outcome",
                "phase",
                "schema",
                "scope",
                "seq",
            ],
            "checkpoint" => &[
                "at_ns",
                "kind",
                "last_completed_prefix",
                "schema",
                "scope",
                "seq",
                "totals",
            ],
            "terminal" => &["at_ns", "collection_outcome", "kind", "schema", "seq"],
            _ => panic!("unknown kind"),
        };
        assert_eq!(keys, expected);
        if let Some(scope) = record.get("scope") {
            assert_eq!(
                scope
                    .as_object()
                    .unwrap()
                    .keys()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
                [
                    "case",
                    "history_entries",
                    "prefix_ordinal",
                    "proof_kind",
                    "quantum",
                    "query_kind",
                    "units_done",
                    "viewer"
                ]
            );
        }
        if record["kind"] == "begin" {
            assert!(
                record["bytes"].is_null()
                    && record["duration_ns"].is_null()
                    && record["outcome"].is_null()
            );
        }
    }
}

#[test]
fn search_spool_profile_all_prefix_totals_exact_cold_checkpoints_and_duplicate_sampling() {
    let game = match_root();
    let query = Query::Search {
        viewer: PlayerId(0),
        sampling_seed: "private:雪\n\0".into(),
    };
    let (mut profile, sink) = memory_profile();
    compare_with_profile(&game, query, Some(&mut profile));
    profile.finish(true).unwrap();
    let records = profile_records(&sink);
    // Classify REAL emitted records, rather than just assuming the reservation
    // formula: regular contexts stay within8 pairs; nested cold work stays in
    // its separate20-checkpoint allowance across the four-viewer collection.
    let mut regular = std::collections::BTreeMap::new();
    let mut in_cold = false;
    let mut nested_records = 0_u64;
    for r in &records {
        if r["kind"] == "begin" && r["phase"] == "sparse_cold_proof" {
            in_cold = true;
        }
        if in_cold {
            nested_records += 1;
        } else if r["kind"] == "begin" {
            let scope = &r["scope"];
            let key = (
                scope["quantum"].as_u64().unwrap(),
                scope["prefix_ordinal"].as_u64().unwrap(),
            );
            *regular.entry(key).or_insert(0_u64) += 1;
        }
        if r["kind"] == "end" && r["phase"] == "sparse_cold_proof" {
            in_cold = false;
        }
    }
    assert!(!in_cold);
    assert!(regular.values().all(|calls| *calls <= 8));
    assert!(nested_records * 4 <= 1024);
    let completed = records
        .iter()
        .filter(|r| r["kind"] == "end" && r["phase"] == "completed_result")
        .map(|r| {
            (
                r["scope"]["quantum"].as_u64().unwrap(),
                r["scope"]["units_done"].as_u64().unwrap(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(completed.len(), 3);
    assert!(completed.iter().all(|(_, u)| *u == completed[0].1));
    let cold = records
        .iter()
        .filter(|r| r["kind"] == "end" && r["phase"] == "sparse_cold_proof")
        .map(|r| {
            (
                r["scope"]["quantum"].as_u64().unwrap(),
                r["scope"]["units_done"].as_u64().unwrap(),
            )
        })
        .collect::<Vec<_>>();
    let units = completed[0].1;
    let mut expected = [1, 5, 31, 129]
        .into_iter()
        .filter(|u| *u < units)
        .map(|u| (1, u))
        .collect::<Vec<_>>();
    if units > 64 {
        expected.push((64, 64));
    }
    assert_eq!(cold, expected);
    let mut seen = std::collections::BTreeSet::new();
    for r in records.iter().filter(|r| r["kind"] == "checkpoint") {
        let q = r["scope"]["quantum"].as_u64().unwrap();
        let ordinal = r["last_completed_prefix"].as_u64().unwrap();
        assert!(seen.insert((q, ordinal)), "duplicate sampled prefix");
        let is_cold = cold.contains(&(q, r["scope"]["units_done"].as_u64().unwrap()));
        let is_final = completed.contains(&(q, r["scope"]["units_done"].as_u64().unwrap()));
        assert!(ordinal == 0 || ordinal.is_power_of_two() || is_cold || is_final);
        if is_final {
            let totals = r["totals"].as_array().unwrap();
            let calls = |id| {
                totals.iter().find(|t| t[0] == id).unwrap()[1]
                    .as_u64()
                    .unwrap()
            };
            assert_eq!(calls(0), ordinal);
            assert_eq!(calls(1), ordinal);
            let cold_calls = cold.iter().filter(|(quantum, _)| *quantum == q).count() as u64;
            // Every lazy/incomplete/final prefix plus both existing cold comparisons.
            assert_eq!(calls(3), ordinal + 1 + cold_calls * 2);
            assert_eq!(calls(4), calls(3));
            assert_eq!(calls(5), calls(3));
            assert_eq!(calls(2), ordinal + 1 + cold_calls);
        }
    }
    assert!(
        !String::from_utf8(sink.0.borrow().bytes.clone())
            .unwrap()
            .contains("private:")
    );
    let (mut profile, sink) = memory_profile();
    profile.quantum(1, 1);
    profile.checkpoint().unwrap();
    let writes = sink.0.borrow().writes;
    profile.checkpoint().unwrap();
    assert_eq!(sink.0.borrow().writes, writes);
}

#[test]
fn search_spool_profile_comparator_parity_late_tail_eof_digest_poison_and_error_precedence() {
    let directory = Directory::new().unwrap();
    for length in [0, 1, BLOCK - 1, BLOCK, BLOCK + 1, 2 * BLOCK + 37] {
        let reference = vec![b'a'; length];
        for variant in 0..9 {
            let mut actual = reference.clone();
            if variant == 1 {
                actual.push(b'x');
            }
            if variant == 2 && !actual.is_empty() {
                actual.pop();
            }
            if variant == 3 && !actual.is_empty() {
                *actual.last_mut().unwrap() = b'x';
            }
            if variant == 7 {
                actual.fill(b'x');
            }
            if variant == 8 && !actual.is_empty() {
                actual.pop();
            }
            let run = |profile| {
                compare_bytes_with_profile(
                    &directory,
                    |out| {
                        out.write_all(&reference).map_err(text)?;
                        if variant == 4 {
                            Err("reference private error".into())
                        } else {
                            Ok(())
                        }
                    },
                    |out| {
                        if matches!(variant, 7 | 8) {
                            // Equal raw bytes and EOF on BOTH operands are still
                            // insufficient: keep the earlier count/SHA receipt.
                            fs::write(directory.0.join("reference.json"), &actual).map_err(text)?;
                        }
                        for part in actual.chunks(13) {
                            out.write_all(part).map_err(text)?;
                        }
                        if variant == 5 {
                            return Err("actual private error".into());
                        }
                        if variant == 6 {
                            // Same-length alteration after reopen: the original SHA receipt must fail.
                            fs::write(directory.0.join("reference.json"), vec![b'x'; length])
                                .map_err(text)?;
                        }
                        Ok(())
                    },
                    profile,
                )
            };
            let plain = run(None);
            let (mut profile, sink) = memory_profile();
            let observed = run(Some(&mut profile));
            assert_eq!(observed, plain, "length {length}, variant {variant}");
            let records = profile_records(&sink);
            if matches!(variant, 7 | 8) && length > 0 {
                assert_eq!(
                    observed.as_ref().unwrap_err(),
                    "complete comparison count/SHA mismatch"
                );
                assert!(phase_ok(&records, "actual_serialize_compare_hash"));
            }
            assert_eq!(
                phase_ok(&records, "finish_eof_count_digest"),
                observed.is_ok()
            );
            if observed.is_err() {
                assert!(profile.finish(true).is_err());
                assert!(!records.iter().any(|r| r["kind"] == "terminal"));
                assert!(
                    records
                        .iter()
                        .filter(|r| r["outcome"] == "error")
                        .all(|r| r["bytes"].is_null())
                );
            }
            let log = String::from_utf8(sink.0.borrow().bytes.clone()).unwrap();
            assert!(!log.contains("private error"));
        }
    }
    for flush in [false, true] {
        let (mut profile, sink) = memory_profile();
        let result = compare_bytes_with_profile(
            &directory,
            |out| out.write_all(&vec![b'a'; BLOCK + 37]).map_err(text),
            |out| {
                if flush {
                    out.write_all(b"b").unwrap();
                    assert!(out.flush().is_err());
                } else {
                    assert!(out.write_all(&vec![b'b'; BLOCK]).is_err());
                }
                Ok(())
            },
            Some(&mut profile),
        );
        assert_eq!(result.unwrap_err(), "poisoned byte comparison");
        assert!(!phase_ok(
            &profile_records(&sink),
            "finish_eof_count_digest"
        ));
    }
}

#[test]
fn search_spool_profile_sink_short_interrupted_zero_write_flush_sync_and_counter_overflow_fail_closed()
 {
    let (mut p, sink) = memory_profile();
    sink.0.borrow_mut().short = true;
    sink.0.borrow_mut().interrupt = true;
    proof_phase(Some(&mut p), ProofPhase::QueryProof, |_| Ok(())).unwrap();
    p.finish(true).unwrap();
    assert_eq!(profile_records(&sink).len(), 4);
    for variant in 0..9 {
        let (mut p, sink) = memory_profile();
        match variant {
            0 => sink.0.borrow_mut().zero = true,
            1 => sink.0.borrow_mut().fail_write = Some(2),
            2 => sink.0.borrow_mut().fail_flush = true,
            3 => sink.0.borrow_mut().fail_sync = true,
            4 => p.seq = u64::MAX,
            5 => p.bytes = u64::MAX,
            6 => p.totals[0][0] = u64::MAX,
            7 => p.totals[0][1] = u64::MAX,
            8 => p.totals[0][2] = u64::MAX,
            _ => unreachable!(),
        }
        let result = if matches!(variant, 2 | 3) {
            p.finish(true)
        } else {
            proof_phase_bytes(Some(&mut p), ProofPhase::ReferenceResume, |_| {
                Ok(((), Some(1)))
            })
        };
        assert!(result.is_err(), "variant {variant}");
        assert!(p.failed);
        assert!(p.finish(true).is_err());
    }
    let (mut p, _) = memory_profile();
    p.clock = Box::new(|| Err("clock overflow".into()));
    assert!(proof_phase(Some(&mut p), ProofPhase::QueryProof, |_| Ok(())).is_err());
    let (mut p, _) = memory_profile();
    p.last_at = u64::MAX;
    assert!(proof_phase(Some(&mut p), ProofPhase::QueryProof, |_| Ok(())).is_err());
}

#[test]
fn search_spool_profile_raw_path_privacy_modes_exclusive_creation_and_retention() {
    use std::ffi::OsStr;
    assert!(!LegalProofProfile::requested(None, None).unwrap());
    assert!(!LegalProofProfile::requested(None, Some(OsStr::new("wrong"))).unwrap());
    for value in ["", "0", "true", "01", "1 "] {
        assert!(
            LegalProofProfile::requested(Some(OsStr::new(value)), Some(OsStr::new("120s-2gib-v1")))
                .is_err()
        );
    }
    assert!(LegalProofProfile::requested(Some(OsStr::new("1")), None).is_err());
    assert!(
        LegalProofProfile::requested(Some(OsStr::new("1")), Some(OsStr::new("120s-2gib-v1")))
            .unwrap()
    );
    let base = Directory::new().unwrap();
    for bad in [
        PathBuf::from("relative"),
        base.0.join("."),
        base.0.join(".."),
        PathBuf::from(format!("{}/./child", base.0.display())),
    ] {
        assert!(LegalProofProfile::open_under(&bad, "ordinary_4").is_err());
    }
    let link = base.0.join("link");
    std::os::unix::fs::symlink(&base.0, &link).unwrap();
    assert!(LegalProofProfile::open_under(&link, "ordinary_4").is_err());
    assert!(LegalProofProfile::open_under(&link.join("descendant"), "ordinary_4").is_err());
    fs::write(base.0.join("file"), b"untouched").unwrap();
    assert!(LegalProofProfile::open_under(&base.0.join("file"), "ordinary_4").is_err());
    fs::set_permissions(&base.0, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(LegalProofProfile::open_under(&base.0, "ordinary_4").is_err());
    fs::set_permissions(&base.0, fs::Permissions::from_mode(0o700)).unwrap();
    let child = base.0.join("legal-proof-phase-v1");
    std::os::unix::fs::symlink(&base.0, &child).unwrap();
    assert!(LegalProofProfile::open_under(&base.0, "ordinary_4").is_err());
    fs::remove_file(&child).unwrap();
    fs::create_dir(&child).unwrap();
    fs::write(child.join("events.ndjson"), b"original").unwrap();
    assert!(LegalProofProfile::open_under(&base.0, "ordinary_4").is_err());
    assert_eq!(fs::read(child.join("events.ndjson")).unwrap(), b"original");
    fs::remove_dir_all(&child).unwrap();
    let mut p = LegalProofProfile::open_under(&base.0, "ordinary_4").unwrap();
    assert_eq!(
        fs::metadata(&child).unwrap().permissions().mode() & 0o7777,
        0o700
    );
    assert_eq!(
        fs::metadata(child.join("events.ndjson"))
            .unwrap()
            .permissions()
            .mode()
            & 0o7777,
        0o600
    );
    proof_phase(Some(&mut p), ProofPhase::QueryProof, |_| {
        Err::<(), _>("secret error payload".into())
    })
    .unwrap_err();
    assert!(p.finish(true).is_err());
    p.finish(false).unwrap();
    drop(p);
    let retained = fs::read_to_string(child.join("events.ndjson")).unwrap();
    assert!(!retained.contains("secret error payload"));
    assert!(retained.contains("\"collection_outcome\":\"error\""));
    assert!(LegalProofProfile::open_under(&base.0, "ordinary_4").is_err());
    // This is our test-owned base, explicitly removed after retained evidence assertions.
    base.cleanup().unwrap();
}

#[test]
fn search_spool_profile_sampling_record_line_size_and_category_bounds() {
    // Maximum distinct contexts: zero + all64 powers + four off-power cold + final.
    let contexts = 1_u64 + 64 + 4 + 1;
    assert_eq!(contexts, 70);
    let regular_pairs = 4 * 3 * contexts * 8 * 2;
    let checkpoints = 4 * 3 * contexts;
    let nested_cold = 1024;
    let root_query = 128;
    assert_eq!(
        regular_pairs + checkpoints + nested_cold + root_query,
        15432
    );
    assert!(regular_pairs + checkpoints + nested_cold + root_query < PROFILE_RECORDS);
    let (mut p, sink) = memory_profile();
    p.quantum(u64::MAX, u64::MAX);
    p.query(PlayerId(3), QueryKind::Search);
    p.scope.quantum = Some(u64::MAX);
    p.scope.history_entries = Some(u64::MAX);
    p.scope.units_done = Some(u64::MAX);
    p.scope.prefix_ordinal = Some(u64::MAX);
    p.totals = [[u64::MAX; 3]; 8];
    p.checkpoint().unwrap(); // worst-case integer widths still fit1024B including newline
    assert_eq!(profile_records(&sink).len(), 2);
    for variant in 0..3 {
        let (mut p, _) = memory_profile();
        match variant {
            0 => p.seq = PROFILE_RECORDS,
            1 => p.bytes = PROFILE_BYTES,
            2 => {}
            _ => unreachable!(),
        }
        let value = serde_json::json!({"test":if variant == 2 { "x".repeat(PROFILE_LINE) } else { String::new() }});
        assert!(p.emit(value).is_err());
        assert!(p.failed);
    }
}

#[test]
fn search_spool_profile_sink_cancellation_each_comparator_boundary_cleans_up_and_stops() {
    let (mut p, sink) = memory_profile();
    let directory = Directory::new().unwrap();
    compare_bytes_with_profile(
        &directory,
        |o| o.write_all(b"complete").map_err(text),
        |o| o.write_all(b"complete").map_err(text),
        Some(&mut p),
    )
    .unwrap();
    let records = profile_records(&sink);
    for fail in 2..=records.len() as u64 {
        let (mut p, sink) = memory_profile();
        sink.0.borrow_mut().fail_write = Some(fail);
        let path;
        {
            let directory = Directory::new().unwrap();
            path = directory.0.clone();
            assert!(
                compare_bytes_with_profile(
                    &directory,
                    |o| o.write_all(b"complete").map_err(text),
                    |o| o.write_all(b"complete").map_err(text),
                    Some(&mut p)
                )
                .is_err()
            );
        }
        assert!(!path.exists());
        assert_eq!(sink.0.borrow().writes, fail);
        assert!(
            !profile_records(&sink)
                .iter()
                .any(|r| r["kind"] == "terminal")
        );
        assert!(p.finish(true).is_err());
    }
}

#[test]
fn search_spool_profile_snapshot_each_coarse_boundary_cancellation_closes_owned_files() {
    let game = match_root();
    let original = game.snapshot().unwrap();
    let (mut p, sink) = memory_profile();
    restore_snapshot_from_private_file_with_profile(&game, Some(&mut p)).unwrap();
    let records = profile_records(&sink);
    for fail in 2..=records.len() as u64 {
        let directory = Directory::new().unwrap();
        let path = directory.0.clone();
        let (mut p, sink) = memory_profile();
        sink.0.borrow_mut().fail_write = Some(fail);
        assert!(restore_snapshot_in_directory(&game, directory, Some(&mut p)).is_err());
        assert!(!path.exists());
        assert_eq!(sink.0.borrow().writes, fail);
        assert!(p.finish(true).is_err());
        assert_eq!(game.snapshot().unwrap(), original);
    }
}

#[test]
fn search_spool_profile_checked_cleanup_failure_and_unwind_never_certify_collection() {
    let directory = Directory::new().unwrap();
    let path = directory.0.clone();
    let (mut p, sink) = memory_profile();
    let result = proof_phase(Some(&mut p), ProofPhase::CheckedDirectoryCleanup, |_| {
        fs::remove_dir_all(&directory.0).map_err(text)?;
        directory.cleanup().map_err(text)
    });
    assert!(result.is_err());
    assert!(!path.exists());
    assert!(!phase_ok(
        &profile_records(&sink),
        "checked_directory_cleanup"
    ));
    assert!(p.finish(true).is_err());
    let base = Directory::new().unwrap();
    let sidecar = base.0.join("legal-proof-phase-v1/events.ndjson");
    let mut p = LegalProofProfile::open_under(&base.0, "dense_4").unwrap();
    let directory = Directory::new().unwrap();
    let path = directory.0.clone();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        proof_phase(
            Some(&mut p),
            ProofPhase::QueryProof,
            |_| -> Result<(), String> {
                let _owned = directory;
                panic!("injected observer unwind")
            },
        )
        .unwrap();
    }));
    assert!(result.is_err());
    assert!(p.finish(true).is_err());
    drop(p);
    assert!(!path.exists());
    let raw = fs::read_to_string(&sidecar).unwrap();
    assert!(raw.contains("\"kind\":\"begin\""));
    assert!(!raw.contains("\"kind\":\"end\""));
    assert!(!raw.contains("\"kind\":\"terminal\""));
    // A killed/failed writer can leave only a prefix of the final NDJSON line.
    // Complete receipts before it remain parseable; no terminal is synthesized.
    let partial = format!("{raw}{{\"schema\":");
    let complete = partial
        .split_inclusive('\n')
        .filter(|l| l.ends_with('\n'))
        .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(complete.len(), 2);
    assert!(serde_json::from_str::<serde_json::Value>(partial.lines().last().unwrap()).is_err());
}

#[test]
fn search_spool_profile_checked_aggregate_bytes_and_unsampled_prefixes_do_not_publish() {
    let directory = Directory::new().unwrap();
    let (mut p, sink) = memory_profile();
    p.quantum(1, 2);
    p.prefix(3, 3, false, false); // neither power-of-two, zero, cold nor final
    let writes = sink.0.borrow().writes;
    for raw in [b"ab".as_slice(), b"cde".as_slice()] {
        compare_bytes_with_profile(
            &directory,
            |out| out.write_all(raw).map_err(text),
            |out| out.write_all(raw).map_err(text),
            Some(&mut p),
        )
        .unwrap();
    }
    p.checkpoint().unwrap();
    assert_eq!(sink.0.borrow().writes, writes);
    for id in [3, 4, 5] {
        assert_eq!(p.totals[id][0], 2);
        assert_eq!(p.totals[id][2], 5);
        assert!(p.totals[id][1] > 0);
    }
    assert_eq!(sink.0.borrow().syncs, 0);
    p.prefix(5, 5, true, false);
    p.checkpoint().unwrap();
    assert_eq!(sink.0.borrow().syncs, 1);
    assert_eq!(profile_records(&sink).len(), 2);
    let calls = p.totals[0][0];
    let elapsed = p.totals[0][1];
    assert!(
        proof_phase(Some(&mut p), ProofPhase::ReferenceResume, |_| Err::<(), _>(
            "private".into()
        ))
        .is_err()
    );
    assert_eq!(p.totals[0][0], calls);
    assert_eq!(p.totals[0][1], elapsed);
    assert!(p.finish(true).is_err());
}

#[test]
fn search_spool_profile_terminal_full_line_then_flush_sync_failure_rolls_back_only_terminal() {
    struct TerminalFile {
        file: File,
        path: PathBuf,
        sync_fault: bool,
        write_fault: bool,
        tail_pending: bool,
        rollback_fault: bool,
        terminal: bool,
    }
    impl Write for TerminalFile {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.tail_pending {
                return Err(io::Error::other("primary terminal write failure"));
            }
            let terminal = bytes
                .windows(b"root_collection_complete".len())
                .any(|w| w == b"root_collection_complete");
            if terminal && self.write_fault {
                let n = bytes.len() - 1;
                self.file.write_all(&bytes[..n])?;
                self.tail_pending = true;
                return Ok(n);
            }
            self.file.write_all(bytes)?;
            if bytes
                .windows(b"root_collection_complete".len())
                .any(|w| w == b"root_collection_complete")
            {
                // A separate read observes a complete success line BEFORE the
                // injected failure. Merely testing a rejected write misses this.
                self.terminal = true;
                let raw = fs::read(&self.path)?;
                assert!(raw.ends_with(b"\n"));
                let last = raw.split_inclusive(|b| *b == b'\n').next_back().unwrap();
                let record: serde_json::Value = serde_json::from_slice(last).unwrap();
                assert_eq!(record["collection_outcome"], "root_collection_complete");
            }
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            if self.terminal && !self.sync_fault {
                Err(io::Error::other("primary terminal flush failure"))
            } else {
                self.file.flush()
            }
        }
    }
    impl ProofSink for TerminalFile {
        fn sync(&mut self) -> io::Result<()> {
            if self.terminal && self.sync_fault {
                Err(io::Error::other("primary terminal sync failure"))
            } else {
                self.file.sync_all()
            }
        }
        fn rollback_terminal(&mut self, bytes: u64) -> io::Result<()> {
            if self.rollback_fault {
                return Err(io::Error::other("secondary rollback failure"));
            }
            self.file.rollback_terminal(bytes)
        }
    }
    for sync_fault in [false, true] {
        for rollback_fault in [false, true] {
            for write_fault in [false, true] {
                let directory = Directory::new().unwrap();
                let path = directory.0.join("terminal.ndjson");
                let file = TerminalFile {
                    file: directory.create("terminal.ndjson").unwrap(),
                    path: path.clone(),
                    sync_fault,
                    write_fault,
                    tail_pending: false,
                    rollback_fault,
                    terminal: false,
                };
                let mut tick = 0_u64;
                let mut profile = LegalProofProfile::new(
                    Box::new(file),
                    Box::new(move || {
                        tick += 1;
                        Ok(tick)
                    }),
                    "ordinary_4",
                )
                .unwrap();
                proof_phase(Some(&mut profile), ProofPhase::QueryProof, |_| Ok(())).unwrap();
                let committed = fs::read(&path).unwrap();
                let error = profile.finish(true).unwrap_err();
                assert!(profile.failed);
                assert!(!profile.finished);
                let primary = if write_fault {
                    "primary terminal write failure"
                } else if sync_fault {
                    "primary terminal sync failure"
                } else {
                    "primary terminal flush failure"
                };
                if rollback_fault {
                    assert_eq!(
                        error,
                        format!("{primary}; terminal rollback failed: secondary rollback failure")
                    );
                    // Failed rollback cannot promise invalidation. It is a native
                    // failure; a sidecar alone never overrides the outer success gate.
                    assert!(fs::read(&path).unwrap().starts_with(&committed));
                } else {
                    assert_eq!(error, primary);
                    assert_eq!(fs::read(&path).unwrap(), committed);
                    assert_eq!(profile.bytes, u64::try_from(committed.len()).unwrap());
                    assert!(
                        !String::from_utf8(committed)
                            .unwrap()
                            .contains("root_collection_complete")
                    );
                }
                assert!(profile.finish(true).is_err());
                drop(profile);
                assert!(
                    path.exists(),
                    "retain all preceding proof receipts on error"
                );
            }
        }
    }
}
