//! One bounded IPC writer and one isolated worker. No Game lives in this process.
use crate::{
    ipc::{self, ReplyFrame, RequestFrame, WorkerCommand, WorkerReady, WorkerReply},
    protocol::ExecutionStatus,
};
use std::{
    path::PathBuf,
    process::Stdio,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::{Child, ChildStdin, ChildStdout, Command},
    time::{Instant as TokioInstant, timeout, timeout_at},
};

const MAX_TIMEOUT: Duration = Duration::from_secs(120);
const CONTROL_TIMEOUT: Duration = Duration::from_secs(3);
#[derive(Clone, Debug)]
pub struct SpawnConfig {
    pub expected_identity: ipc::WorkerIdentity,
    pub binary: PathBuf,
    pub directory: PathBuf,
    pub catalog: PathBuf,
    pub config: Option<PathBuf>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BridgeError {
    InvalidTimeout,
    InvalidRequest,
    Unavailable,
    Isolation,
    Protocol,
    TransportUncertain,
    Cleanup,
}
impl std::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "worker bridge {self:?}")
    }
}
impl std::error::Error for BridgeError {}

pub struct WorkerProcess {
    child: Option<Child>,
    input: Option<ChildStdin>,
    output: Option<ChildStdout>,
    unit: String,
    ready: WorkerReady,
    status: ExecutionStatus,
    next_id: u64,
    usable: bool,
    cleaned: bool,
}
fn duration(value: Duration) -> Result<(), BridgeError> {
    if value.is_zero() || value > MAX_TIMEOUT {
        Err(BridgeError::InvalidTimeout)
    } else {
        Ok(())
    }
}
fn unit_name() -> Result<String, BridgeError> {
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes).map_err(|_| BridgeError::Unavailable)?;
    let suffix: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    Ok(format!("mtg-p8-{}-{suffix}.service", std::process::id()))
}
/// Registry and actual recovered executor must name the same closed match.
/// This is host-private; never publish seed-bound profile hashes over HTTP.
pub fn verify_identity(
    expected: &ipc::WorkerIdentity,
    actual: &ipc::WorkerIdentity,
) -> Result<(), BridgeError> {
    let hash = |text: &str| {
        text.len() == 64
            && text
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    };
    if !crate::worker::valid_match_id(&expected.match_id)
        || !hash(&expected.engine_build_id)
        || !hash(&expected.server_build_id)
        || !hash(&expected.catalog_sha256)
        || !hash(&expected.profile_sha256)
        || actual != expected
    {
        return Err(BridgeError::Isolation);
    }
    Ok(())
}
async fn read_reply(output: &mut ChildStdout) -> Result<ReplyFrame, BridgeError> {
    let mut length = [0; 4];
    output
        .read_exact(&mut length)
        .await
        .map_err(|_| BridgeError::TransportUncertain)?;
    let length = u32::from_be_bytes(length) as usize;
    if length == 0 || length > ipc::MAX_FRAME_BYTES {
        return Err(BridgeError::Protocol);
    }
    let mut body = vec![0; length];
    output
        .read_exact(&mut body)
        .await
        .map_err(|_| BridgeError::TransportUncertain)?;
    ipc::decode(&body).map_err(|_| BridgeError::Protocol)
}
async fn show(unit: &str) -> Result<String, BridgeError> {
    // The selected properties are bounded scalar values, not logs/environment.
    let mut command = Command::new("systemctl");
    command
        .args([
            "--user",
            "show",
            "--no-pager",
            "--property=MemoryMax,MemorySwapMax,TasksMax,ControlGroup,MainPID",
            unit,
        ])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let output = timeout(CONTROL_TIMEOUT, command.output())
        .await
        .map_err(|_| BridgeError::Isolation)?
        .map_err(|_| BridgeError::Isolation)?;
    if !output.status.success() || output.stdout.len() > 16_384 {
        return Err(BridgeError::Isolation);
    }
    String::from_utf8(output.stdout).map_err(|_| BridgeError::Isolation)
}
fn verify_properties(text: &str, ready: &WorkerReady) -> Result<(), BridgeError> {
    let mut fields = std::collections::BTreeMap::new();
    for line in text.lines() {
        let (key, value) = line.split_once('=').ok_or(BridgeError::Isolation)?;
        if fields.insert(key, value).is_some() {
            return Err(BridgeError::Isolation);
        }
    }
    let memory = ipc::MEMORY_MAX.to_string();
    let tasks = ipc::TASKS_MAX.to_string();
    let pid = ready.pid.to_string();
    let expected = [
        ("MemoryMax", memory.as_str()),
        ("MemorySwapMax", "0"),
        ("TasksMax", tasks.as_str()),
        ("MainPID", pid.as_str()),
        ("ControlGroup", ready.limits.cgroup.as_str()),
    ];
    if fields.len() != expected.len()
        || expected
            .iter()
            .any(|(key, value)| fields.get(key) != Some(value))
    {
        return Err(BridgeError::Isolation);
    }
    if ipc::limits_at(&ready.limits.cgroup).map_err(|_| BridgeError::Isolation)? != ready.limits {
        return Err(BridgeError::Isolation);
    }
    Ok(())
}
async fn control(args: &[&str]) -> bool {
    let mut command = Command::new("systemctl");
    command
        .arg("--user")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    matches!(timeout(CONTROL_TIMEOUT,command.status()).await,Ok(Ok(status)) if status.success())
}
fn empty_group(ready: &WorkerReady) -> bool {
    let Ok(dir) = ipc::cgroup_directory(&ready.limits.cgroup) else {
        return false;
    };
    match std::fs::read_to_string(dir.join("cgroup.events")) {
        Ok(events) => events.lines().any(|line| line == "populated 0"),
        Err(e) => e.kind() == std::io::ErrorKind::NotFound,
    }
}
fn kill_group(ready: &WorkerReady) {
    // Kernel fallback, restricted to the exact verified worker-owned group.
    if let Ok(dir) = ipc::cgroup_directory(&ready.limits.cgroup) {
        let _ = std::fs::write(dir.join("cgroup.kill"), b"1");
    }
}
fn control_sync(args: &[&str]) -> bool {
    let Ok(mut child) = std::process::Command::new("systemctl")
        .arg("--user")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    let until = Instant::now() + CONTROL_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Err(_) => break,
            Ok(None) => {}
        }
        if Instant::now() >= until {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let _ = child.kill();
    // A stuck control helper must not make emergency Drop wait without bound.
    // Its detached reaper owns the handle; the isolated game unit is separately
    // killed through its verified cgroup and its own run child is reaped below.
    let _ = std::thread::Builder::new()
        .name("mtg-control-reaper".into())
        .spawn(move || {
            let _ = child.wait();
        });
    false
}

fn reap_sync(child: &mut Child) {
    let _ = child.start_kill();
    let until = Instant::now() + CONTROL_TIMEOUT;
    loop {
        if matches!(child.try_wait(), Ok(Some(_))) {
            break;
        }
        if Instant::now() >= until {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
struct StartupGuard {
    unit: String,
    child: Option<Child>,
    armed: bool,
}
impl Drop for StartupGuard {
    fn drop(&mut self) {
        if self.armed {
            control_sync(&["kill", "--kill-whom=all", "--signal=KILL", &self.unit]);
            control_sync(&["stop", &self.unit]);
            if let Some(child) = &mut self.child {
                reap_sync(child);
            }
        }
    }
}
struct CallLease<'a> {
    process: &'a mut WorkerProcess,
    finished: bool,
}
impl Drop for CallLease<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.process.emergency_cleanup();
        }
    }
}
fn reply_matches(command: &WorkerCommand, reply: &WorkerReply) -> bool {
    matches!(
        (command, reply),
        (_, WorkerReply::Error { .. })
            | (
                WorkerCommand::Submit { .. } | WorkerCommand::CancelQueued { .. },
                WorkerReply::Submission { .. }
            )
            | (WorkerCommand::Advance, WorkerReply::Advanced { .. })
            | (WorkerCommand::Page { .. }, WorkerReply::Page { .. })
            | (
                WorkerCommand::Execution { .. },
                WorkerReply::Execution { .. }
            )
            | (WorkerCommand::Clock { .. }, WorkerReply::Clocked { .. })
            | (WorkerCommand::Shutdown, WorkerReply::Stopped)
    )
}
impl WorkerProcess {
    pub async fn spawn(
        config: SpawnConfig,
        startup_timeout: Duration,
    ) -> Result<Self, BridgeError> {
        duration(startup_timeout)?;
        verify_identity(&config.expected_identity, &config.expected_identity)?;
        if !config.binary.is_absolute()
            || !config.directory.is_absolute()
            || !config.catalog.is_absolute()
            || config.config.as_ref().is_some_and(|p| !p.is_absolute())
        {
            return Err(BridgeError::InvalidRequest);
        }
        let unit = unit_name()?;
        let mut command = Command::new("systemd-run");
        // Official systemd-run/resource-control/kill man sources (checked2026-10-15):
        // https://github.com/systemd/systemd/blob/main/man/systemd-run.xml
        // https://github.com/systemd/systemd/blob/main/man/systemd.resource-control.xml
        // --pipe waits; Type=exec catches failed exec; never shell-expand paths.
        command
            .args([
                "--user",
                "--pipe",
                "--collect",
                "--quiet",
                "--no-ask-password",
                "--expand-environment=no",
            ])
            .arg(format!("--unit={unit}"))
            .args([
                "--property=Type=exec",
                "--property=MemoryMax=4294967296",
                "--property=MemorySwapMax=0",
                "--property=TasksMax=32",
                "--property=KillMode=control-group",
                "--property=SendSIGKILL=yes",
                "--property=TimeoutStopSec=2s",
            ])
            .arg("--")
            .arg(&config.binary)
            .arg("--worker")
            .arg(&config.directory)
            .arg(&config.catalog)
            .arg(&config.expected_identity.match_id);
        if let Some(config) = config.config {
            command.arg(config);
        }
        // No private child error detail reaches HTTP/logs; replies carry codes.
        // Tokio recommends explicit kill+wait; kill_on_drop is the last fallback:
        // https://docs.rs/tokio/1.53.3/tokio/process/struct.Child.html
        let child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| BridgeError::Unavailable)?;
        let mut guard = StartupGuard {
            unit: unit.clone(),
            child: Some(child),
            armed: true,
        };
        let child = guard.child.as_mut().ok_or(BridgeError::Unavailable)?;
        let input = child.stdin.take().ok_or(BridgeError::Unavailable)?;
        let mut output = child.stdout.take().ok_or(BridgeError::Unavailable)?;
        let deadline = TokioInstant::now() + startup_timeout;
        let startup = timeout_at(deadline, async {
            let frame = read_reply(&mut output).await?;
            let WorkerReply::Ready { ready } = frame.reply else {
                return Err(BridgeError::Protocol);
            };
            if frame.ipc_version != ipc::IPC_VERSION || frame.request_id != 0 || ready.pid == 0 {
                return Err(BridgeError::Protocol);
            }
            verify_identity(&config.expected_identity, &ready.identity)?;
            verify_properties(&show(&unit).await?, &ready)?;
            Ok(ready)
        })
        .await;
        let ready = match startup {
            Ok(Ok(ready)) => ready,
            _ => return Err(BridgeError::Isolation),
        };
        let child = guard.child.take().ok_or(BridgeError::Unavailable)?;
        guard.armed = false;

        Ok(Self {
            child: Some(child),
            input: Some(input),
            output: Some(output),
            status: ready.status.clone(),
            ready,
            unit,
            next_id: 1,
            usable: true,
            cleaned: false,
        })
    }
    pub fn ready(&self) -> &WorkerReady {
        &self.ready
    }
    pub fn status(&self) -> &ExecutionStatus {
        &self.status
    }
    pub fn unit(&self) -> &str {
        &self.unit
    }
    pub async fn call(
        &mut self,
        command: WorkerCommand,
        wait: Duration,
    ) -> Result<WorkerReply, BridgeError> {
        duration(wait)?;
        if !self.usable {
            return Err(BridgeError::Unavailable);
        }
        command
            .validate()
            .map_err(|_| BridgeError::InvalidRequest)?;
        let id = self.next_id;
        let next = id.checked_add(1).ok_or(BridgeError::Unavailable)?;
        let frame = ipc::encode(&RequestFrame {
            ipc_version: ipc::IPC_VERSION,
            request_id: id,
            command: command.clone(),
        })
        .map_err(|_| BridgeError::InvalidRequest)?;
        self.next_id = next;
        self.usable = false;
        // Cancellation by an outer HTTP/broker task must also stop the unit.
        let mut lease = CallLease {
            process: self,
            finished: false,
        };
        let result = timeout(wait, async {
            let input = lease
                .process
                .input
                .as_mut()
                .ok_or(BridgeError::Unavailable)?;
            input
                .write_all(&frame)
                .await
                .map_err(|_| BridgeError::TransportUncertain)?;
            input
                .flush()
                .await
                .map_err(|_| BridgeError::TransportUncertain)?;
            let frame = read_reply(
                lease
                    .process
                    .output
                    .as_mut()
                    .ok_or(BridgeError::Unavailable)?,
            )
            .await?;
            if frame.ipc_version != ipc::IPC_VERSION
                || frame.request_id != id
                || !reply_matches(&command, &frame.reply)
            {
                return Err(BridgeError::Protocol);
            }
            Ok(frame.reply)
        })
        .await;
        let answer = match result {
            Ok(Ok(reply)) => {
                if let Some(status) = reply.status() {
                    lease.process.status = status.clone();
                }
                lease.process.usable = !matches!(
                    reply,
                    WorkerReply::Stopped
                        | WorkerReply::Error {
                            code: ipc::WorkerErrorCode::Unavailable
                        }
                );
                Ok(reply)
            }
            _ => {
                // A partial write or lost reply may have committed. NEVER retry.
                let _ = lease.process.cleanup().await;
                Err(BridgeError::TransportUncertain)
            }
        };
        lease.finished = true;
        answer
    }
    async fn cleanup(&mut self) -> Result<(), BridgeError> {
        self.usable = false;
        self.input.take();
        self.output.take();
        kill_group(&self.ready);
        control(&["kill", "--kill-whom=all", "--signal=KILL", &self.unit]).await;
        control(&["stop", &self.unit]).await;
        if let Some(child) = &mut self.child
            && !matches!(timeout(CONTROL_TIMEOUT, child.wait()).await, Ok(Ok(_)))
        {
            let _ = child.start_kill();
            if !matches!(timeout(CONTROL_TIMEOUT, child.wait()).await, Ok(Ok(_))) {
                return Err(BridgeError::Cleanup);
            }
        }
        if !empty_group(&self.ready) {
            return Err(BridgeError::Cleanup);
        }
        self.child.take();
        self.cleaned = true;
        Ok(())
    }
    pub async fn shutdown(mut self, wait: Duration) -> Result<(), BridgeError> {
        let result = if self.usable {
            self.call(WorkerCommand::Shutdown, wait).await.map(|_| ())
        } else {
            Ok(())
        };
        let cleanup = self.cleanup().await;
        result.and(cleanup)
    }
}
impl WorkerProcess {
    fn emergency_cleanup(&mut self) {
        if self.cleaned {
            return;
        }
        self.usable = false;
        self.input.take();
        self.output.take();
        kill_group(&self.ready);
        control_sync(&["kill", "--kill-whom=all", "--signal=KILL", &self.unit]);
        control_sync(&["stop", &self.unit]);
        if let Some(child) = &mut self.child {
            reap_sync(child);
        }
        if empty_group(&self.ready) {
            self.cleaned = true;
        } else {
            eprintln!("worker emergency cleanup could not verify empty isolation group");
        }
    }
}
impl Drop for WorkerProcess {
    fn drop(&mut self) {
        self.emergency_cleanup();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timeouts_and_property_mismatches_fail_closed() {
        assert_eq!(duration(Duration::ZERO), Err(BridgeError::InvalidTimeout));
        assert_eq!(
            duration(Duration::from_secs(121)),
            Err(BridgeError::InvalidTimeout)
        );
        assert!(duration(MAX_TIMEOUT).is_ok());
        let ready = WorkerReady {
            identity: ipc::WorkerIdentity {
                match_id: "a1".into(),
                engine_build_id: "a".repeat(64),
                server_build_id: "b".repeat(64),
                catalog_sha256: "b".repeat(64),
                profile_sha256: "c".repeat(64),
            },
            pid: 123,
            limits: ipc::KernelLimits {
                cgroup: "/not-a-real-worker".into(),
                memory_max: ipc::MEMORY_MAX,
                memory_swap_max: 0,
                tasks_max: 32,
            },
            status: ExecutionStatus::Running,
        };
        for properties in [
            "MemoryMax=4294967296\n",
            "MemoryMax=1\nMemorySwapMax=0\nTasksMax=32\nMainPID=123\nControlGroup=/not-a-real-worker\n",
            "MemoryMax=1\nMemoryMax=1\n",
        ] {
            assert_eq!(
                verify_properties(properties, &ready),
                Err(BridgeError::Isolation)
            );
        }
    }
    #[test]
    fn replies_cannot_cross_operation_kinds() {
        assert!(reply_matches(
            &WorkerCommand::Shutdown,
            &WorkerReply::Stopped
        ));
        assert!(!reply_matches(
            &WorkerCommand::Advance,
            &WorkerReply::Stopped
        ));
        assert!(!reply_matches(
            &WorkerCommand::Shutdown,
            &WorkerReply::Advanced {
                status: ExecutionStatus::Running
            }
        ));
        assert!(reply_matches(
            &WorkerCommand::Advance,
            &WorkerReply::Error {
                code: ipc::WorkerErrorCode::Unavailable
            }
        ));
    }
    #[test]
    fn units_are_fixed_safe_unique_names() {
        let a = unit_name().unwrap();
        let b = unit_name().unwrap();
        assert_ne!(a, b);
        assert!(a.starts_with("mtg-p8-"));
        assert!(a.ends_with(".service"));
        assert!(
            a.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-.".contains(&b))
        );
    }
}
