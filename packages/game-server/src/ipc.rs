//! Host-private, length-prefixed stdio protocol. Never route these DTOs to HTTP.
pub use crate::worker::WorkerIdentity;
use crate::{
    protocol::{
        CommandEnvelope, CommandExecution, ExecutionStatus, SeatResume, StreamPage,
        SubmissionResult,
    },
    worker::{ClockControl, Worker, WorkerPhase},
};
use mtg_contracts::{Catalog, strict_read};
use mtg_core::MatchConfig;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
    time::Instant,
};

pub const IPC_VERSION: u32 = 1;
pub const MAX_FRAME_BYTES: usize = 3 * 1024 * 1024;
pub const MEMORY_MAX: u64 = 4 * 1024 * 1024 * 1024;
pub const TASKS_MAX: u64 = 32;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkerCommand {
    Submit {
        seat: u8,
        request: CommandEnvelope,
    },
    CancelQueued {
        seat: u8,
        request: CommandEnvelope,
    },
    Resume {
        seat: u8,
    },
    QueryActions {
        seat: u8,
        request: crate::protocol::ActionQueryRequest,
    },
    Advance,
    Page {
        seat: u8,
        after: u64,
        limit: u32,
    },
    Execution {
        seat: u8,
        command_id: String,
    },
    Clock {
        control: ClockControl,
    },
    Shutdown,
}
impl WorkerCommand {
    pub fn validate(&self) -> Result<(), FrameError> {
        let seat = match self {
            Self::Submit { seat, .. }
            | Self::CancelQueued { seat, .. }
            | Self::QueryActions { seat, .. }
            | Self::Resume { seat }
            | Self::Page { seat, .. }
            | Self::Execution { seat, .. } => Some(*seat),
            _ => None,
        };
        if seat.is_some_and(|seat| seat >= 4)
            || matches!(self, Self::Page {limit,..} if !(1..=32).contains(limit))
            || matches!(self, Self::Execution {command_id,..}
                if command_id.len()!=32 || !command_id.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
        {
            return Err(FrameError::Invalid);
        }
        if let Self::QueryActions { request, .. } = self {
            let text = serde_json::to_string(request).map_err(|_| FrameError::Invalid)?;
            crate::protocol::decode_action_request(&text).map_err(|_| FrameError::Invalid)?;
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerErrorCode {
    Conflict,
    InvalidRequest,
    Unavailable,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KernelLimits {
    pub cgroup: String,
    pub memory_max: u64,
    pub memory_swap_max: u64,
    pub tasks_max: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerReady {
    pub identity: WorkerIdentity,
    pub pid: u32,
    pub limits: KernelLimits,
    pub status: ExecutionStatus,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reply", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkerReply {
    Ready {
        ready: WorkerReady,
    },
    Submission {
        result: SubmissionResult,
        status: ExecutionStatus,
    },
    Advanced {
        status: ExecutionStatus,
    },
    Clocked {
        status: ExecutionStatus,
    },
    Page {
        page: Box<StreamPage>,
    },
    Execution {
        execution: Option<CommandExecution>,
    },
    Resume {
        resume: Box<SeatResume>,
    },
    Actions {
        response: Box<crate::protocol::ActionQueryResponse>,
    },
    Stopped,
    Error {
        code: WorkerErrorCode,
    },
}
impl WorkerReply {
    pub fn status(&self) -> Option<&ExecutionStatus> {
        match self {
            Self::Ready { ready } => Some(&ready.status),
            Self::Submission { status, .. }
            | Self::Advanced { status }
            | Self::Clocked { status } => Some(status),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestFrame {
    pub ipc_version: u32,
    pub request_id: u64,
    pub command: WorkerCommand,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplyFrame {
    pub ipc_version: u32,
    pub request_id: u64,
    pub reply: WorkerReply,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameError {
    Io,
    Truncated,
    Oversized,
    Invalid,
}
impl std::fmt::Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "private IPC {self:?}")
    }
}
impl std::error::Error for FrameError {}

pub fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, FrameError> {
    struct Buffer {
        bytes: Vec<u8>,
        overflow: bool,
    }
    impl Write for Buffer {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self
                .bytes
                .len()
                .checked_add(bytes.len())
                .is_none_or(|n| n > MAX_FRAME_BYTES)
            {
                self.overflow = true;
                return Err(io::Error::other("IPC size bound"));
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut buffer = Buffer {
        bytes: Vec::new(),
        overflow: false,
    };
    if serde_json::to_writer(&mut buffer, value).is_err() {
        return Err(if buffer.overflow {
            FrameError::Oversized
        } else {
            FrameError::Invalid
        });
    }
    let body = buffer.bytes;
    if body.is_empty() || body.len() > MAX_FRAME_BYTES {
        return Err(FrameError::Oversized);
    }
    let mut bytes = Vec::with_capacity(body.len() + 4);
    bytes.extend_from_slice(
        &u32::try_from(body.len())
            .map_err(|_| FrameError::Oversized)?
            .to_be_bytes(),
    );
    bytes.extend(body);
    Ok(bytes)
}
pub fn decode<T: DeserializeOwned + Serialize>(body: &[u8]) -> Result<T, FrameError> {
    if body.is_empty() || body.len() > MAX_FRAME_BYTES {
        return Err(FrameError::Oversized);
    }
    let text = std::str::from_utf8(body).map_err(|_| FrameError::Invalid)?;
    crate::codec::exact(text).map_err(|_| FrameError::Invalid)
}
pub fn read_frame<R: Read, T: DeserializeOwned + Serialize>(
    reader: &mut R,
) -> Result<Option<T>, FrameError> {
    let mut length = [0; 4];
    loop {
        match reader.read(&mut length[..1]) {
            Ok(0) => return Ok(None),
            Ok(_) => break,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(FrameError::Io),
        }
    }
    reader
        .read_exact(&mut length[1..])
        .map_err(|_| FrameError::Truncated)?;
    let length = u32::from_be_bytes(length) as usize;
    if length == 0 || length > MAX_FRAME_BYTES {
        return Err(FrameError::Oversized);
    }
    let mut body = vec![0; length];
    reader
        .read_exact(&mut body)
        .map_err(|_| FrameError::Truncated)?;
    decode(&body).map(Some)
}
pub fn write_frame<W: Write, T: Serialize>(writer: &mut W, value: &T) -> Result<(), FrameError> {
    writer
        .write_all(&encode(value)?)
        .map_err(|_| FrameError::Io)?;
    writer.flush().map_err(|_| FrameError::Io)
}
fn bounded_file(path: &Path, limit: usize) -> Result<String, String> {
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|_| "private worker file unavailable")?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "private worker file unavailable")?;
    if bytes.len() > limit {
        return Err("private worker file exceeds bound".into());
    }
    String::from_utf8(bytes).map_err(|_| "private worker file invalid".into())
}
pub fn cgroup_directory(group: &str) -> Result<PathBuf, String> {
    let path = Path::new(group);
    if !path.is_absolute()
        || path
            .components()
            .any(|part| !matches!(part, Component::RootDir | Component::Normal(_)))
        || group == "/"
    {
        return Err("invalid worker cgroup".into());
    }
    Ok(Path::new("/sys/fs/cgroup").join(
        path.strip_prefix("/")
            .map_err(|_| "invalid worker cgroup")?,
    ))
}
pub fn limits_at(group: &str) -> Result<KernelLimits, String> {
    let directory = cgroup_directory(group)?;
    let number = |name: &str| -> Result<u64, String> {
        bounded_file(&directory.join(name), 128)?
            .trim()
            .parse()
            .map_err(|_| "worker resource limit unavailable".into())
    };
    let limits = KernelLimits {
        cgroup: group.into(),
        memory_max: number("memory.max")?,
        memory_swap_max: number("memory.swap.max")?,
        tasks_max: number("pids.max")?,
    };
    if limits.memory_max != MEMORY_MAX
        || limits.memory_swap_max != 0
        || limits.tasks_max != TASKS_MAX
    {
        return Err("worker kernel isolation differs from policy".into());
    }
    Ok(limits)
}
fn own_limits() -> Result<KernelLimits, String> {
    let cgroup = bounded_file(Path::new("/proc/self/cgroup"), 8192)?;
    let mut groups = cgroup.lines().filter_map(|line| line.strip_prefix("0::"));
    let group = groups.next().ok_or("unified worker cgroup unavailable")?;
    if groups.next().is_some() {
        return Err("ambiguous worker cgroup".into());
    }
    limits_at(group)
}
fn status(worker: &Worker) -> ExecutionStatus {
    match worker.host_state().phase {
        WorkerPhase::Running => ExecutionStatus::Running,
        WorkerPhase::Waiting => ExecutionStatus::Waiting,
        WorkerPhase::Paused { .. } => ExecutionStatus::ResourcePaused,
        WorkerPhase::Faulted => ExecutionStatus::Faulted,
        WorkerPhase::Finished { .. } => ExecutionStatus::Finished,
    }
}
fn failure(reason: &str) -> WorkerReply {
    WorkerReply::Error {
        code: if reason.contains("idempotency key reused with different command") {
            WorkerErrorCode::Conflict
        } else {
            WorkerErrorCode::Unavailable
        },
    }
}
fn handle(worker: &mut Worker, command: WorkerCommand) -> WorkerReply {
    match command {
        WorkerCommand::QueryActions { seat, request } => WorkerReply::Actions {
            response: Box::new(worker.query_actions(seat, request, Instant::now())),
        },
        WorkerCommand::Submit { seat, request } => {
            match worker.submit(seat, request, Instant::now()) {
                Ok(result) => WorkerReply::Submission {
                    result,
                    status: status(worker),
                },
                Err(reason) => failure(&reason),
            }
        }
        WorkerCommand::CancelQueued { seat, request } => {
            match worker.cancel_queued(seat, request, Instant::now()) {
                Ok(result) => WorkerReply::Submission {
                    result,
                    status: status(worker),
                },
                Err(reason) => failure(&reason),
            }
        }
        WorkerCommand::Advance => match worker.advance(Instant::now()) {
            Ok(status) => WorkerReply::Advanced { status },
            Err(reason) => failure(&reason),
        },
        WorkerCommand::Page { seat, after, limit } => {
            match worker.stream_page(seat, after, limit) {
                Ok(page) => WorkerReply::Page {
                    page: Box::new(page),
                },
                Err(reason)
                    if reason == "invalid private page bound"
                        || reason == "cursor exceeds viewer history" =>
                {
                    WorkerReply::Error {
                        code: WorkerErrorCode::InvalidRequest,
                    }
                }
                Err(_) => WorkerReply::Error {
                    code: WorkerErrorCode::Unavailable,
                },
            }
        }
        WorkerCommand::Resume { seat } => match worker.resume(seat) {
            Ok(resume) => WorkerReply::Resume {
                resume: Box::new(resume),
            },
            Err(_) => WorkerReply::Error {
                code: WorkerErrorCode::Unavailable,
            },
        },
        WorkerCommand::Execution { seat, command_id } => {
            match worker.command_execution(seat, &command_id) {
                Ok(execution) => WorkerReply::Execution { execution },
                Err(_) => WorkerReply::Error {
                    code: WorkerErrorCode::Unavailable,
                },
            }
        }
        WorkerCommand::Clock { control } => match worker.clock(control, Instant::now()) {
            Ok(status) => WorkerReply::Clocked { status },
            Err(reason)
                if [
                    "worker is not paused",
                    "exhausted clock cannot be refilled by resume",
                    "terminal/faulted worker cannot change clock state",
                    "invalid resource pause cause",
                ]
                .contains(&reason.as_str()) =>
            {
                WorkerReply::Error {
                    code: WorkerErrorCode::InvalidRequest,
                }
            }
            Err(reason) => failure(&reason),
        },
        WorkerCommand::Shutdown => WorkerReply::Stopped,
    }
}
/// stdout is EXCLUSIVELY framed IPC. Files and cgroup facts are host-private.
pub fn run_child(
    directory: &Path,
    catalog_path: &Path,
    match_id: &str,
    config_path: Option<&Path>,
) -> Result<(), String> {
    // Check actual kernel files before allocating a catalog, checkpoint or Game.
    let limits = own_limits()?;
    let catalog = Catalog::read(&bounded_file(catalog_path, MAX_FRAME_BYTES)?)
        .map_err(|_| "worker catalog invalid")?;
    let mut worker = if let Some(config) = config_path {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if fs::metadata(config)
                .map_err(|_| "private config unavailable")?
                .permissions()
                .mode()
                & 0o077
                != 0
            {
                return Err("worker config must be private".into());
            }
        }
        let config: MatchConfig =
            strict_read(&bounded_file(config, 65_536)?).map_err(|_| "worker config invalid")?;
        Worker::create(directory, catalog, config, match_id, Instant::now())?
    } else {
        Worker::recover(directory, catalog, match_id, Instant::now())?
    };
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    write_frame(
        &mut output,
        &ReplyFrame {
            ipc_version: IPC_VERSION,
            request_id: 0,
            reply: WorkerReply::Ready {
                ready: WorkerReady {
                    identity: worker.identity().clone(),
                    pid: std::process::id(),
                    limits,
                    status: status(&worker),
                },
            },
        },
    )
    .map_err(|e| e.to_string())?;
    let mut next = 1u64;
    while let Some(frame) = read_frame::<_, RequestFrame>(&mut input).map_err(|e| e.to_string())? {
        if frame.ipc_version != IPC_VERSION || frame.request_id != next {
            return Err("invalid worker IPC correlation".into());
        }
        next = next.checked_add(1).ok_or("worker IPC sequence exhausted")?;
        let reply = if frame.command.validate().is_err() {
            WorkerReply::Error {
                code: WorkerErrorCode::InvalidRequest,
            }
        } else {
            handle(&mut worker, frame.command)
        };
        let stop = matches!(
            reply,
            WorkerReply::Stopped
                | WorkerReply::Error {
                    code: WorkerErrorCode::Unavailable
                }
        );
        write_frame(
            &mut output,
            &ReplyFrame {
                ipc_version: IPC_VERSION,
                request_id: frame.request_id,
                reply,
            },
        )
        .map_err(|e| e.to_string())?;
        if stop {
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn framing_roundtrip_fragmentation_and_clean_eof() {
        let value = RequestFrame {
            ipc_version: 1,
            request_id: 1,
            command: WorkerCommand::Advance,
        };
        struct Fragments<'a>(&'a [u8]);
        impl Read for Fragments<'_> {
            fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
                let n = b.len().min(self.0.len()).min(1);
                b[..n].copy_from_slice(&self.0[..n]);
                self.0 = &self.0[n..];
                Ok(n)
            }
        }
        let bytes = encode(&value).unwrap();
        let mut input = Fragments(&bytes);
        assert_eq!(
            read_frame::<_, RequestFrame>(&mut input).unwrap(),
            Some(value)
        );
        assert_eq!(read_frame::<_, RequestFrame>(&mut input).unwrap(), None);
    }
    #[test]
    fn frame_limits_torn_data_duplicate_and_unknown_fields_reject() {
        for bytes in [
            vec![0],
            vec![0, 0, 0],
            vec![0, 0, 0, 2, b'{'],
            (MAX_FRAME_BYTES as u32 + 1).to_be_bytes().to_vec(),
            vec![0, 0, 0, 0],
        ] {
            assert!(read_frame::<_, RequestFrame>(&mut &bytes[..]).is_err());
        }
        for body in [
            br#"{"ipc_version":1,"request_id":1,"command":{"command":"advance"},"extra":true}"#
                .as_slice(),
            br#"{"ipc_version":1,"ipc_version":1,"request_id":1,"command":{"command":"advance"}}"#,
            br#"{"ipc_version":1,"request_id":1,"command":{"command":"advance","extra":true}}"#,
        ] {
            assert!(decode::<RequestFrame>(body).is_err());
        }
        assert!(encode(&"x".repeat(MAX_FRAME_BYTES)).is_err());
    }
    #[test]
    fn invalid_seats_pages_ids_and_cgroup_paths_reject() {
        assert!(
            WorkerCommand::Page {
                seat: 4,
                after: 0,
                limit: 1
            }
            .validate()
            .is_err()
        );
        assert!(
            WorkerCommand::Page {
                seat: 0,
                after: 0,
                limit: 33
            }
            .validate()
            .is_err()
        );
        assert!(
            WorkerCommand::Execution {
                seat: 0,
                command_id: "x".repeat(32)
            }
            .validate()
            .is_err()
        );
        for path in ["/", "relative", "/../../other", "/x/../y"] {
            assert!(cgroup_directory(path).is_err());
        }
    }
}
