//! Private committed reads, independent of the rules IPC scheduling phase.
use crate::{
    ipc::WorkerIdentity,
    protocol::SeatResume,
    store::{self, ReadDescriptor},
};
use rusqlite::{Connection, OpenFlags};
use std::{
    fs::{self, File},
    os::unix::fs::{FileExt, MetadataExt, PermissionsExt},
    path::{Component, Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

/// An open proc inode remains bound to the original task, even after PID reuse.
/// Reading at offset zero avoids a cached seq_file result. Zombies are dead;
/// stopped/traced tasks remain live, including an intentionally held IPC call.
/// https://docs.kernel.org/filesystems/proc.html#process-specific-subdirectories
pub(crate) struct Liveness {
    stat: File,
    pid: u32,
    start: String,
}
impl Liveness {
    pub(crate) fn open(pid: u32) -> Result<Self, String> {
        let stat = File::open(format!("/proc/{pid}/stat")).map_err(|_| "worker not live")?;
        let (actual, start) = Self::sample(&stat)?;
        if actual != pid {
            return Err("worker PID mismatch".into());
        }
        Ok(Self { stat, pid, start })
    }
    fn sample(stat: &File) -> Result<(u32, String), String> {
        let mut bytes = [0; 4096];
        let n = stat.read_at(&mut bytes, 0).map_err(|_| "worker not live")?;
        if n == 0 || n == bytes.len() {
            return Err("invalid worker stat".into());
        }
        let text = std::str::from_utf8(&bytes[..n]).map_err(|_| "invalid worker stat")?;
        let (pid, rest) = text.split_once(" (").ok_or("invalid worker stat")?;
        // comm can itself contain spaces and parentheses.
        let (_, fields) = rest.rsplit_once(") ").ok_or("invalid worker stat")?;
        let fields: Vec<_> = fields.split_ascii_whitespace().collect();
        if fields.len() < 20 || !matches!(fields[0], "R" | "S" | "D" | "T" | "t" | "I") {
            return Err("worker not live".into());
        }
        let start = fields[19]
            .parse::<u64>()
            .map_err(|_| "invalid worker start time")?;
        Ok((
            pid.parse().map_err(|_| "invalid worker PID")?,
            start.to_string(),
        ))
    }
    pub(crate) fn check(&self) -> Result<(), String> {
        if Self::sample(&self.stat)? != (self.pid, self.start.clone()) {
            return Err("worker incarnation changed".into());
        }
        Ok(())
    }
}

#[derive(Clone, PartialEq, Eq)]
struct FileIdentity {
    dev: u64,
    ino: u64,
}
fn identity(meta: &fs::Metadata) -> FileIdentity {
    FileIdentity {
        dev: meta.dev(),
        ino: meta.ino(),
    }
}
/// All path components reject symlinks; the trusted registry root and every
/// descendant directory must be same-owner 0700. Outer parents may be public,
/// as in the registry/lifecycle contract. Same-UID concurrent tampering is out
/// of that existing threat model; do not use this reader for untrusted paths.
fn directories(directory: &Path, private_root: &Path, owner: u32) -> Result<(), String> {
    if !directory.is_absolute() || !directory.starts_with(private_root) {
        return Err("invalid reader directory".into());
    }
    let mut prefix = PathBuf::new();
    for part in directory.components() {
        if !matches!(part, Component::RootDir | Component::Normal(_)) {
            return Err("invalid reader path component".into());
        }
        prefix.push(part);
        let meta = fs::symlink_metadata(&prefix).map_err(|_| "reader directory unavailable")?;
        if !meta.is_dir()
            || (prefix.starts_with(private_root)
                && (meta.uid() != owner || meta.permissions().mode() & 0o777 != 0o700))
        {
            return Err("reader directories must be private and host-owned".into());
        }
    }
    Ok(())
}
fn private_file(path: &Path, owner: u32) -> Result<FileIdentity, String> {
    let meta = fs::symlink_metadata(path).map_err(|_| "reader file unavailable")?;
    if !meta.is_file()
        || meta.uid() != owner
        || meta.nlink() != 1
        || meta.permissions().mode() & 0o777 != 0o600
    {
        return Err("reader file must be private, regular, single-link and host-owned".into());
    }
    Ok(identity(&meta))
}
fn uri(path: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;
    let mut encoded = String::from("file:");
    for b in path.as_os_str().as_bytes() {
        if b.is_ascii_alphanumeric() || b"/-._~".contains(b) {
            encoded.push(char::from(*b));
        } else {
            encoded.push_str(&format!("%{b:02X}"));
        }
    }
    encoded.push_str("?mode=ro&readonly_shm=1");
    encoded
}

pub(crate) struct CommittedReader {
    connection: Connection,
    directory: PathBuf,
    private_root: PathBuf,
    owner: u32,
    files: [FileIdentity; 3],
    descriptor: ReadDescriptor,
}
impl CommittedReader {
    pub(crate) fn open(
        directory: &Path,
        private_root: &Path,
        descriptor: ReadDescriptor,
    ) -> Result<Self, String> {
        let owner = fs::metadata("/proc/self")
            .map_err(|_| "owner unavailable")?
            .uid();
        directories(directory, private_root, owner)?;
        let names = ["match.sqlite3", "match.sqlite3-wal", "match.sqlite3-shm"];
        let files = names.map(|name| private_file(&directory.join(name), owner));
        let [db, wal, shm] = files;
        let files = [db?, wal?, shm?];
        // Verified against rusqlite 0.40.2 and bundled libsqlite3-sys 0.38.2:
        // sqlite3.c unixOpenSharedMemory skips O_RDWR|O_CREAT for readonly_shm.
        // unixLockSharedMemory fails rather than initializes an unowned SHM.
        // https://sqlite.org/src/doc/trunk/src/os_unix.c
        // Never default-open, initialize, checkpoint, use immutable/nolock or
        // enable shared cache/read_uncommitted. URI is internally constructed.
        let flags = OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_PRIVATE_CACHE
            | OpenFlags::SQLITE_OPEN_NOFOLLOW
            | OpenFlags::SQLITE_OPEN_URI;
        let connection = Connection::open_with_flags(uri(&directory.join(names[0])), flags)
            .map_err(|_| "read-only database unavailable")?;
        connection
            .busy_timeout(Duration::from_millis(100))
            .map_err(|_| "reader timeout unavailable")?;
        let reader = Self {
            connection,
            directory: directory.into(),
            private_root: private_root.into(),
            owner,
            files,
            descriptor,
        };
        // Verify with a fresh snapshot before admitting any reader to the broker.
        reader.read(0)?;
        Ok(reader)
    }
    fn validate_files(&self) -> Result<(), String> {
        directories(&self.directory, &self.private_root, self.owner)?;
        for (index, name) in ["match.sqlite3", "match.sqlite3-wal", "match.sqlite3-shm"]
            .iter()
            .enumerate()
        {
            if private_file(&self.directory.join(name), self.owner)? != self.files[index] {
                return Err("reader database identity changed".into());
            }
        }
        // An unexpected rollback journal is not part of this WAL reader.
        match fs::symlink_metadata(self.directory.join("match.sqlite3-journal")) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            _ => return Err("unexpected or uninspectable database journal".into()),
        }
        Ok(())
    }
    pub(crate) fn read(&self, seat: u8) -> Result<store::CommittedPublications, String> {
        self.read_snapshot(seat, || {})
    }
    fn read_snapshot(
        &self,
        seat: u8,
        after_pins: impl FnOnce(),
    ) -> Result<store::CommittedPublications, String> {
        self.validate_files()?;
        let tx = self
            .connection
            .unchecked_transaction()
            .map_err(|_| "reader transaction unavailable")?;
        let generation: String = tx.query_row("SELECT CASE WHEN typeof(value)='text' AND length(CAST(value AS BLOB))<=20 THEN value ELSE NULL END FROM metadata WHERE key='generation'", [], |r| r.get(0)).map_err(|_| "reader generation unavailable")?;
        let bootstrap: String = tx.query_row("SELECT CASE WHEN typeof(value)='text' AND length(CAST(value AS BLOB))<=?1 THEN value ELSE NULL END FROM metadata WHERE key='bootstrap'", [store::MAX_RECORD_BYTES as i64], |r| r.get(0)).map_err(|_| "reader bootstrap unavailable")?;
        if generation != self.descriptor.generation.to_string()
            || mtg_contracts::sha256(bootstrap.as_bytes()) != self.descriptor.bootstrap_sha256
        {
            return Err("reader Ready pins changed".into());
        }
        after_pins();
        let result = store::resume_publications(&tx, seat)?;
        tx.commit().map_err(|_| "reader transaction failed")?;
        self.validate_files()?;
        Ok(result)
    }
}

/// One incarnation; revocation is independent of the mutable IPC call lease.
pub(crate) struct ResumeReader {
    active: AtomicBool,
    live: Liveness,
    identity: WorkerIdentity,
    reader: Mutex<CommittedReader>,
}
impl ResumeReader {
    pub(crate) fn open(
        directory: &Path,
        private_root: &Path,
        identity: WorkerIdentity,
        descriptor: ReadDescriptor,
        live: Liveness,
    ) -> Result<Self, String> {
        live.check()?;
        let reader = CommittedReader::open(directory, private_root, descriptor)?;
        live.check()?;
        Ok(Self {
            active: AtomicBool::new(true),
            live,
            identity,
            reader: Mutex::new(reader),
        })
    }
    #[cfg(test)]
    pub(crate) fn hold_for_test(&self) -> std::sync::MutexGuard<'_, CommittedReader> {
        self.reader.lock().unwrap()
    }
    pub(crate) fn revoke(&self) {
        self.active.store(false, Ordering::SeqCst);
    }
    pub(crate) fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }
    fn check(&self) -> Result<(), String> {
        if !self.is_active() {
            return Err("reader revoked".into());
        }
        self.live.check()
    }
    pub(crate) fn read(&self, seat: u8) -> Result<SeatResume, String> {
        self.check()?;
        let publications = {
            let reader = self.reader.lock().map_err(|_| "reader poisoned")?;
            self.check()?;
            reader.read(seat)?
        };
        let resume = crate::worker::assemble_resume(&self.identity, seat, publications)?;
        self.check()?;
        Ok(resume)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{
        protocol::{ExecutionStatus, ResumeState},
        worker::{ClockControl, Worker, WorkerPhase},
    };
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);
    pub(crate) struct Fixture {
        pub root: PathBuf,
        pub directory: PathBuf,
        pub worker: Worker,
    }
    impl Fixture {
        pub(crate) fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "mtg-resume-reader-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let directory = root.join("matches/a1/worker");
            for path in [&root, &root.join("matches"), &root.join("matches/a1")] {
                fs::create_dir(path).unwrap();
                fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
            }
            let worker = Worker::create(
                &directory,
                catalog(),
                mtg_core::MatchConfig::for_profile(
                    "commander-foundations-v1",
                    [3; 32],
                    mtg_core::PlayerId(0),
                )
                .unwrap(),
                "a1",
                std::time::Instant::now(),
            )
            .unwrap();
            Self {
                root,
                directory,
                worker,
            }
        }
        pub(crate) fn settle(&mut self) {
            for _ in 0..1000 {
                if !matches!(self.worker.host_state().phase, WorkerPhase::Running) {
                    return;
                }
                self.worker.advance(std::time::Instant::now()).unwrap();
            }
            panic!("bounded worker settle");
        }
        pub(crate) fn reader(&self) -> Arc<ResumeReader> {
            Arc::new(
                ResumeReader::open(
                    &self.directory,
                    &self.root,
                    self.worker.identity().clone(),
                    self.worker.read_descriptor().unwrap(),
                    Liveness::open(std::process::id()).unwrap(),
                )
                .unwrap(),
            )
        }
        fn sql(&self) -> Connection {
            Connection::open(self.directory.join("match.sqlite3")).unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
    fn catalog() -> mtg_contracts::Catalog {
        mtg_contracts::Catalog::read(include_str!(
            "../../game-engine/tests/fixtures/expected-match-ir.json"
        ))
        .unwrap()
    }
    #[test]
    fn differential_pending_all_seats_and_retained_view_after_lifecycle() {
        let mut f = Fixture::new();
        let reader = f.reader();
        for seat in 0..4 {
            assert_eq!(reader.read(seat).unwrap(), f.worker.resume(seat).unwrap());
            assert!(matches!(
                reader.read(seat).unwrap().state,
                ResumeState::Pending
            ));
        }
        assert!(reader.read(4).is_err());
        f.settle();
        let before: Vec<_> = (0..4).map(|seat| f.worker.resume(seat).unwrap()).collect();
        f.worker
            .clock(ClockControl::Pause, std::time::Instant::now())
            .unwrap();
        for seat in 0..4 {
            let actual = reader.read(seat).unwrap();
            assert_eq!(actual, f.worker.resume(seat).unwrap());
            assert_eq!(actual.state, before[usize::from(seat)].state);
            assert_eq!(actual.next.get(), before[usize::from(seat)].next.get() + 1);
            assert_eq!(actual.lifecycle, Some(ExecutionStatus::ResourcePaused));
        }
        reader.revoke();
        assert!(reader.read(0).is_err());
    }
    fn append_lifecycle(sql: &Connection, status: ExecutionStatus) {
        for seat in 0..4 {
            let cursor: i64 = sql
                .query_row(
                    "SELECT cursor FROM viewer_heads WHERE seat=?1",
                    [seat],
                    |r| r.get(0),
                )
                .unwrap();
            let cursor = u64::try_from(cursor + 1).unwrap();
            let page = crate::protocol::StreamPage {
                protocol_version: 1,
                entries: vec![crate::protocol::StreamEntry {
                    cursor: crate::protocol::Counter::new(cursor),
                    publication: crate::protocol::Publication::Lifecycle {
                        status: status.clone(),
                    },
                }],
                next: crate::protocol::Counter::new(cursor),
                has_more: false,
            };
            sql.execute(
                "INSERT INTO outbox VALUES(?1,?2,?3)",
                rusqlite::params![
                    seat,
                    i64::try_from(cursor).unwrap(),
                    crate::protocol::checked_publication(&page).unwrap()
                ],
            )
            .unwrap();
            sql.execute(
                "UPDATE viewer_heads SET cursor=?2 WHERE seat=?1",
                rusqlite::params![seat, i64::try_from(cursor).unwrap()],
            )
            .unwrap();
        }
    }
    #[test]
    fn fresh_snapshot_never_publishes_uncommitted_transition_or_rollback() {
        let mut f = Fixture::new();
        f.settle();
        let reader = f.reader();
        let prior = reader.read(0).unwrap();
        let mut sql = f.sql();
        let tx = sql.transaction().unwrap();
        append_lifecycle(&tx, ExecutionStatus::Faulted);
        assert_eq!(reader.read(0).unwrap(), prior);
        tx.rollback().unwrap();
        assert_eq!(reader.read(0).unwrap(), prior);
        let tx = sql.transaction().unwrap();
        append_lifecycle(&tx, ExecutionStatus::Finished);
        // Add the actual typed outcome in the same transaction as the cursor
        // and lifecycle. Neither it nor a new token may escape before commit.
        for seat in 0..4 {
            let cursor: i64 = tx
                .query_row(
                    "SELECT cursor FROM viewer_heads WHERE seat=?1",
                    [seat],
                    |r| r.get(0),
                )
                .unwrap();
            let page = crate::protocol::StreamPage {
                protocol_version: 1,
                next: crate::protocol::Counter::new((cursor + 1) as u64),
                has_more: false,
                entries: vec![crate::protocol::StreamEntry {
                    cursor: crate::protocol::Counter::new((cursor + 1) as u64),
                    publication: crate::protocol::Publication::Outcome {
                        result: crate::protocol::RulesResult::Draw,
                    },
                }],
            };
            tx.execute(
                "INSERT INTO outbox VALUES(?1,?2,?3)",
                rusqlite::params![
                    seat,
                    cursor + 1,
                    crate::protocol::checked_publication(&page).unwrap()
                ],
            )
            .unwrap();
            tx.execute(
                "UPDATE viewer_heads SET cursor=?2 WHERE seat=?1",
                rusqlite::params![seat, cursor + 1],
            )
            .unwrap();
        }
        for seat in 0..4 {
            let mut next = f.worker.resume(seat).unwrap();
            let ResumeState::Available { observation } = &mut next.state else {
                panic!("committed view");
            };
            observation.view.turn.number += 1;
            let cursor: i64 = tx
                .query_row(
                    "SELECT cursor FROM viewer_heads WHERE seat=?1",
                    [seat],
                    |r| r.get(0),
                )
                .unwrap();
            let page = crate::protocol::StreamPage {
                protocol_version: 1,
                next: crate::protocol::Counter::new((cursor + 1) as u64),
                has_more: false,
                entries: vec![crate::protocol::StreamEntry {
                    cursor: crate::protocol::Counter::new((cursor + 1) as u64),
                    publication: crate::protocol::Publication::ViewReplaced {
                        observation: observation.clone(),
                    },
                }],
            };
            tx.execute(
                "INSERT INTO outbox VALUES(?1,?2,?3)",
                rusqlite::params![
                    seat,
                    cursor + 1,
                    crate::protocol::checked_publication(&page).unwrap()
                ],
            )
            .unwrap();
            tx.execute(
                "UPDATE viewer_heads SET cursor=?2 WHERE seat=?1",
                rusqlite::params![seat, cursor + 1],
            )
            .unwrap();
        }
        assert_eq!(reader.read(0).unwrap(), prior);
        // Commit exactly between the production snapshot's pin verification
        // and its publication queries. Removing that transaction exposes new
        // rows here and fails this prior-snapshot oracle.
        let pinned = reader
            .reader
            .lock()
            .unwrap()
            .read_snapshot(0, || tx.commit().unwrap())
            .unwrap();
        assert_eq!(
            crate::worker::assemble_resume(f.worker.identity(), 0, pinned).unwrap(),
            prior
        );
        let new = reader.read(0).unwrap();
        assert_eq!(new, f.worker.resume(0).unwrap());
        let ResumeState::Available {
            observation: new_view,
        } = &new.state
        else {
            panic!("new view");
        };
        let ResumeState::Available {
            observation: prior_view,
        } = &prior.state
        else {
            panic!("prior view");
        };
        assert_eq!(new_view.view.turn.number, prior_view.view.turn.number + 1);
        assert_eq!(new.next.get(), prior.next.get() + 3);
        assert_eq!(new.lifecycle, Some(ExecutionStatus::Finished));
        assert_eq!(new.outcome, Some(crate::protocol::RulesResult::Draw));
    }
    #[test]
    fn generation_and_bootstrap_replacement_reject_in_every_fresh_snapshot() {
        for key in ["generation", "bootstrap"] {
            let f = Fixture::new();
            let reader = f.reader();
            let sql = f.sql();
            sql.execute(
                "UPDATE metadata SET value='replacement' WHERE key=?1",
                [key],
            )
            .unwrap();
            assert!(reader.read(0).is_err());
        }
    }
    #[test]
    fn wrong_viewer_unknown_fields_and_oversized_private_publications_reject() {
        for mutation in 0..3 {
            let mut f = Fixture::new();
            f.settle();
            let reader = f.reader();
            let sql = f.sql();
            let (cursor, body): (i64, String) = sql.query_row("SELECT cursor,body FROM outbox WHERE seat=0 AND json_extract(body,'$.entries[0].publication.kind')='view_replaced' ORDER BY cursor DESC LIMIT 1", [], |r| Ok((r.get(0)?,r.get(1)?))).unwrap();
            let mut body: serde_json::Value = serde_json::from_str(&body).unwrap();
            match mutation {
                0 => body["entries"][0]["publication"]["observation"]["view"]["viewer"] = 1.into(),
                1 => body["entries"][0]["publication"]["private_override"] = true.into(),
                _ => body["oversized"] = "x".repeat(store::MAX_VIEW_BYTES).into(),
            }
            sql.execute(
                "UPDATE outbox SET body=?1 WHERE seat=0 AND cursor=?2",
                rusqlite::params![body.to_string(), cursor],
            )
            .unwrap();
            assert!(reader.read(0).is_err(), "mutation {mutation}");
            assert!(f.worker.resume(0).is_err());
            assert!(reader.read(1).is_ok());
        }
    }
    #[test]
    fn reader_never_creates_missing_files_or_accepts_replaced_or_nonprivate_files() {
        for mutation in 0..7 {
            let f = Fixture::new();
            let reader = f.reader();
            let db = f.directory.join("match.sqlite3");
            let target = f.directory.join(match mutation {
                0 => "match.sqlite3",
                1 => "match.sqlite3-wal",
                2 => "match.sqlite3-shm",
                _ => "match.sqlite3",
            });
            match mutation {
                0..=2 => fs::remove_file(&target).unwrap(),
                3 => fs::set_permissions(&target, fs::Permissions::from_mode(0o644)).unwrap(),
                4 => {
                    fs::rename(&db, f.directory.join("saved-db")).unwrap();
                    std::os::unix::fs::symlink("saved-db", &db).unwrap();
                }
                5 => fs::hard_link(&db, f.directory.join("hardlink-db")).unwrap(),
                _ => {
                    fs::rename(&db, f.directory.join("saved-db")).unwrap();
                    fs::copy(f.directory.join("saved-db"), &db).unwrap();
                }
            }
            assert!(reader.read(0).is_err(), "mutation {mutation}");
            if mutation <= 2 {
                assert!(
                    CommittedReader::open(
                        &f.directory,
                        &f.root,
                        f.worker.read_descriptor().unwrap()
                    )
                    .is_err()
                );
                assert!(!target.exists(), "read must never recreate missing file");
            }
        }
        let f = Fixture::new();
        fs::set_permissions(f.root.join("matches"), fs::Permissions::from_mode(0o755)).unwrap();
        assert!(
            CommittedReader::open(&f.directory, &f.root, f.worker.read_descriptor().unwrap())
                .is_err()
        );
    }
    #[test]
    fn read_only_connection_cannot_write_or_advance_generation() {
        let f = Fixture::new();
        let descriptor = f.worker.read_descriptor().unwrap();
        let reader = CommittedReader::open(&f.directory, &f.root, descriptor.clone()).unwrap();
        assert!(
            reader
                .connection
                .execute("UPDATE metadata SET value='999' WHERE key='generation'", [])
                .is_err()
        );
        assert_eq!(f.worker.read_descriptor().unwrap(), descriptor);
        assert!(uri(&PathBuf::from("/tmp/a?b#c%/match.sqlite3")).contains("a%3Fb%23c%25"));
    }
    #[test]
    fn proc_handle_rechecks_death_including_zombie_and_rejects_wrong_incarnation() {
        let mut child = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .unwrap();
        let live = Liveness::open(child.id()).unwrap();
        live.check().unwrap();
        let wrong = Liveness {
            stat: live.stat.try_clone().unwrap(),
            pid: live.pid,
            start: "wrong".into(),
        };
        assert!(wrong.check().is_err());
        let stopped = std::process::Command::new("kill")
            .args(["-STOP", &child.id().to_string()])
            .status()
            .unwrap()
            .success();
        let paused_live = live.check().is_ok();
        // Always restore independently before any assertions can panic.
        let restored = std::process::Command::new("kill")
            .args(["-CONT", &child.id().to_string()])
            .status()
            .unwrap()
            .success();
        child.kill().unwrap();
        let until = std::time::Instant::now() + Duration::from_secs(1);
        while live.check().is_ok() && std::time::Instant::now() < until {
            std::thread::yield_now();
        }
        let zombie_rejected = live.check().is_err();
        child.wait().unwrap();
        assert!(stopped && restored && paused_live && zombie_rejected);
        assert!(live.check().is_err());
        assert!(
            live.check().is_err(),
            "open descriptor must not cache a live stat"
        );
    }
}
