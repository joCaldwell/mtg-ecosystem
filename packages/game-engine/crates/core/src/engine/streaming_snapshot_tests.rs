//! Host API/IO and unchanged proof-cursor regression sources, not run evidence.
use super::*;
use std::io::{self, Cursor, Read, Seek, SeekFrom};

fn prepared() -> Prepared {
    Prepared::admit(
        Catalog::read(include_str!("../../../../tests/fixtures/expected-ir.json")).unwrap(),
        ScenarioConfig::land_return(),
    )
    .unwrap()
}
fn old_json(game: &Game) -> String {
    let state_sha256 = sha256(canonical(&game.state).unwrap().as_bytes());
    // Full original generic SnapshotRef; independent of codec/write_state.
    canonical(&SnapshotRef {
        snapshot_version: 18,
        build_id: BUILD_ID,
        catalog_digest: &game.prepared.catalog_digest,
        profile_digest: &game.prepared.profile_digest,
        state_sha256: &state_sha256,
        state: &game.state,
        trace: &game.trace,
    })
    .unwrap()
}
struct Sink {
    bytes: Vec<u8>,
    left: usize,
    zero: bool,
    interrupted: bool,
    flush_fails: bool,
    flushes: usize,
}
impl Sink {
    fn good() -> Self {
        Self {
            bytes: vec![],
            left: usize::MAX,
            zero: false,
            interrupted: false,
            flush_fails: false,
            flushes: 0,
        }
    }
}
impl Write for Sink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.interrupted {
            self.interrupted = false;
            return Err(io::ErrorKind::Interrupted.into());
        }
        if self.left == 0 {
            return if self.zero {
                Ok(0)
            } else {
                Err(io::Error::other("injected sink failure"))
            };
        }
        let count = bytes.len().min(3).min(self.left);
        self.bytes.extend_from_slice(&bytes[..count]);
        self.left -= count;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.flushes += 1;
        if self.flush_fails {
            Err(io::Error::other("injected flush failure"))
        } else {
            Ok(())
        }
    }
}
struct Source {
    inner: Cursor<Vec<u8>>,
    replace_on_rewind: Option<Vec<u8>>,
    interrupted: bool,
    fail_after: Option<u64>,
    fail_rewind: bool,
}
impl Source {
    fn new(bytes: &[u8]) -> Self {
        Self {
            inner: Cursor::new(bytes.to_vec()),
            replace_on_rewind: None,
            interrupted: false,
            fail_after: None,
            fail_rewind: false,
        }
    }
}
impl Read for Source {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.interrupted {
            self.interrupted = false;
            return Err(io::ErrorKind::Interrupted.into());
        }
        if self.fail_after.is_some_and(|n| self.inner.position() >= n) {
            return Err(io::Error::other("injected reader failure"));
        }
        let count = buffer.len().min(7);
        self.inner.read(&mut buffer[..count])
    }
}
impl Seek for Source {
    fn seek(&mut self, offset: SeekFrom) -> io::Result<u64> {
        if matches!(offset, SeekFrom::Start(_)) {
            if self.fail_rewind {
                return Err(io::Error::other("injected rewind failure"));
            }
            if let Some(bytes) = self.replace_on_rewind.take() {
                self.inner = Cursor::new(bytes);
            }
        }
        self.inner.seek(offset)
    }
}

#[test]
fn streamed_snapshot_writer_raw_receipt_counts_only_complete_successfully_flushed_bytes() {
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        let game = Game::with_trace(prepared(), mode);
        let expected = old_json(&game);
        let mut sink = Sink::good();
        sink.interrupted = true;
        let info = game.write_snapshot(&mut sink).unwrap();
        assert_eq!(sink.bytes, expected.as_bytes());
        assert_eq!(info.bytes, expected.len() as u64);
        assert_eq!(info.snapshot_sha256, sha256(expected.as_bytes()));
        assert_eq!(info.state_sha256, game.state_digest().unwrap());
        assert_eq!(sink.flushes, 1);
        for length in [0, 1, 63, expected.len() / 2, expected.len() - 1] {
            for zero in [false, true] {
                let mut bad = Sink::good();
                bad.left = length;
                bad.zero = zero;
                assert!(game.write_snapshot(&mut bad).is_err());
                assert!(expected.as_bytes().starts_with(&bad.bytes));
            }
        }
        let mut late = Sink::good();
        late.flush_fails = true;
        assert!(game.write_snapshot(&mut late).is_err());
        assert_eq!(late.bytes, expected.as_bytes());
        assert_eq!(late.flushes, 1);
    }
    struct InvalidWrite;
    impl Write for InvalidWrite {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            Ok(bytes.len() + 1)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    assert!(
        Game::new(prepared())
            .write_snapshot(&mut InvalidWrite)
            .is_err()
    );
    let mut invalid = CheckedWrite {
        sink: InvalidWrite,
        bytes: 0,
        sha256: Sha256Sink::new(),
    };
    assert!(
        invalid
            .write(b"x")
            .unwrap_err()
            .to_string()
            .contains("invalid byte count")
    );
    assert_eq!(invalid.bytes, 0);
    assert_eq!(invalid.sha256.finalize_hex(), sha256(b""));

    let mut checked = CheckedWrite {
        sink: Vec::new(),
        bytes: u64::MAX,
        sha256: Sha256Sink::new(),
    };
    assert!(
        checked
            .write(b"x")
            .unwrap_err()
            .to_string()
            .contains("overflow")
    );
}

#[test]
fn streamed_snapshot_restore_uses_same_identity_state_and_full_replay_checks() {
    let game = Game::new(prepared());
    let expected = old_json(&game);
    let mut source = Source::new(format!("prefix{expected} \n\t").as_bytes());
    source.inner.set_position(6);
    source.interrupted = true;
    let restored = Game::restore_from_reader(prepared(), &mut source).unwrap();
    assert_eq!(restored.state, game.state);
    assert_eq!(restored.trace, game.trace);
    assert_eq!(source.inner.position(), source.inner.get_ref().len() as u64);
    let original: Snapshot = strict_read(&expected).unwrap();
    // Validly rehashed but unreachable history must still fail full replay,
    // rather than pass because a streamed decoder verified its raw bytes.
    let mut unreachable = original.clone();
    let last = unreachable.state.visibility[0]
        .history
        .last()
        .unwrap()
        .clone();
    unreachable.state.visibility[0].history.push(last);
    unreachable.state_sha256 = digest(&unreachable.state).unwrap();
    let raw = canonical(&unreachable).unwrap();
    assert!(Game::restore(prepared(), &raw).is_err());
    assert!(Game::restore_from_reader(prepared(), &mut Cursor::new(raw.as_bytes())).is_err());
    let match_prepared = Prepared::admit_pod(
        Catalog::read(include_str!(
            "../../../../tests/fixtures/expected-match-ir.json"
        ))
        .unwrap(),
        MatchConfig::for_profile("commander-foundations-v1", [21; 32], PlayerId(0)).unwrap(),
    )
    .unwrap();
    let mut rng: Snapshot = strict_read(&old_json(&Game::new(match_prepared.clone()))).unwrap();
    rng.state.match_state.as_mut().unwrap().random.seed[0] ^= 1;
    rng.state_sha256 = digest(&rng.state).unwrap();
    let raw = canonical(&rng).unwrap();
    assert!(Game::restore(match_prepared.clone(), &raw).is_err());
    assert!(Game::restore_from_reader(match_prepared, &mut Cursor::new(raw.as_bytes())).is_err());
}

#[test]
fn streamed_snapshot_failed_io_changed_input_and_late_forgery_cannot_poison_verifier() {
    let mut game = Game::new(prepared());
    let initial = old_json(&game);
    let mut verifier = SnapshotVerifier::new(prepared(), TraceMode::Detailed);
    verifier
        .verify_reader(&mut Cursor::new(initial.as_bytes()))
        .unwrap();
    let proved = verifier.verified_digest().unwrap().to_string();
    let mut changed = Source::new(initial.as_bytes());
    changed.replace_on_rewind = Some(
        initial
            .replace("\"snapshot_version\":18", "\"snapshot_version\":17")
            .into_bytes(),
    );
    let error = match verifier.verify_reader(&mut changed) {
        Ok(_) => panic!("changed source accepted"),
        Err(e) => e,
    };
    assert!(error.contains("changed between"));
    assert_eq!(verifier.verified_digest(), Some(proved.as_str()));
    for fail_at in [0, 13, initial.len() as u64] {
        let mut bad = Source::new(initial.as_bytes());
        bad.fail_after = Some(fail_at);
        assert!(verifier.verify_reader(&mut bad).is_err());
        assert_eq!(verifier.verified_digest(), Some(proved.as_str()));
    }
    let mut bad = Source::new(initial.as_bytes());
    bad.fail_rewind = true;
    assert!(verifier.verify_reader(&mut bad).is_err());
    assert_eq!(verifier.verified_digest(), Some(proved.as_str()));
    game.concede(PlayerId(1)).unwrap();
    let good = old_json(&game);
    let mut bad: Snapshot = strict_read(&good).unwrap();
    bad.state.journal.clear();
    bad.state.external_inputs.clear();
    bad.state_sha256 = digest(&bad.state).unwrap();
    let raw = canonical(&bad).unwrap();
    assert!(
        verifier
            .verify_reader(&mut Cursor::new(raw.as_bytes()))
            .is_err()
    );
    assert_eq!(verifier.verified_digest(), Some(proved.as_str()));
    let resumed = verifier
        .verify_reader(&mut Cursor::new(good.as_bytes()))
        .unwrap();
    assert_eq!(resumed.state, game.state);
    assert_eq!(resumed.trace, game.trace);
}
