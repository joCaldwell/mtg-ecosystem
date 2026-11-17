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
        if fs::symlink_metadata(self.directory.join("match.sqlite3-journal")).is_ok() {
            return Err("unexpected database journal".into());
        }
        Ok(())
    }
    pub(crate) fn read(&self, seat: u8) -> Result<store::CommittedPublications, String> {
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
    pub(crate) fn revoke(&self) {
        self.active.store(false, Ordering::SeqCst);
    }
    fn check(&self) -> Result<(), String> {
        if !self.active.load(Ordering::SeqCst) {
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
