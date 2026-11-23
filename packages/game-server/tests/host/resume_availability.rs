//! One opt-in bounded actual API witness; never part of the routine test matrix.
use mtg_contracts::Catalog;
use mtg_core::{MatchConfig, PlayerId};
use mtg_server::{
    auth::CredentialHashes,
    broker::Broker,
    http::{Host, HostedMatch},
    ipc::{WorkerCommand, WorkerReply},
    process::{SpawnConfig, WorkerProcess},
    protocol::{ExecutionStatus, PublicManifest, SeatResume},
    worker::{ClockControl, Worker, WorkerPhase},
};
use std::{
    fs::{self, File, OpenOptions},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::oneshot,
};
fn private_write(path: &Path, bytes: &[u8]) {
    use std::io::Write;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
}
fn signal(unit: &str, signal: &str) -> bool {
    Command::new("timeout")
        .args(["3s", "systemctl"])
        .args([
            "--user",
            "kill",
            "--kill-whom=main",
            &format!("--signal={signal}"),
            unit,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}
struct ResumeOnDrop(String);
impl Drop for ResumeOnDrop {
    fn drop(&mut self) {
        signal(&self.0, "CONT");
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "explicit single actual API witness; requires systemd-user isolation"]
async fn paused_committed_http_resume_during_held_clock_ipc() {
    let evidence = std::path::PathBuf::from(
        std::env::var_os("MTG_RESUME_WITNESS_ROOT").expect("explicit new private witness root"),
    );
    assert!(evidence.is_absolute());
    fs::create_dir(&evidence).unwrap();
    fs::set_permissions(&evidence, fs::Permissions::from_mode(0o700)).unwrap();
    let registry = evidence.join("registry");
    let directory = registry.join("matches/a1/worker");
    for p in [
        &registry,
        &registry.join("matches"),
        &registry.join("matches/a1"),
    ] {
        fs::create_dir(p).unwrap();
        fs::set_permissions(p, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let catalog_text = include_str!("../../../game-engine/tests/fixtures/expected-match-ir.json");
    let catalog = Catalog::read(catalog_text).unwrap();
    let config =
        MatchConfig::for_profile("commander-foundations-v1", [3; 32], PlayerId(0)).unwrap();
    let mut worker = Worker::create(
        &directory,
        catalog.clone(),
        config.clone(),
        "a1",
        Instant::now(),
    )
    .unwrap();
    for _ in 0..1000 {
        if !matches!(worker.host_state().phase, WorkerPhase::Running) {
            break;
        }
        worker.advance(Instant::now()).unwrap();
    }
    assert!(matches!(worker.host_state().phase, WorkerPhase::Waiting));
    worker.clock(ClockControl::Pause, Instant::now()).unwrap();
    let oracle = worker.resume(0).unwrap();
    assert_eq!(oracle.lifecycle, Some(ExecutionStatus::ResourcePaused));
    private_write(
        &evidence.join("oracle.json"),
        serde_json::to_string(&oracle).unwrap().as_bytes(),
    );
    let expected_identity = worker.identity().clone();
    drop(worker);
    let catalog_path = registry.join("matches/a1/catalog.json");
    private_write(&catalog_path, catalog_text.as_bytes());
    let binary = std::path::PathBuf::from(
        std::env::var_os("MTG_RESUME_WITNESS_BINARY").expect("explicit copied worker binary"),
    );
    assert!(binary.is_absolute());
    let process = WorkerProcess::spawn(
        SpawnConfig {
            expected_identity,
            binary,
            directory: directory.clone(),
            catalog: catalog_path,
            config: None,
        },
        Duration::from_secs(120),
    )
    .await
    .unwrap();
    let pid = process.ready().pid;
    let unit = process.unit().to_owned();
    let cgroup = process.ready().limits.cgroup.clone();
    let (broker, actor) = Broker::start(process);
    let (credentials, capabilities) = CredentialHashes::issue("a1").unwrap();
    let manifest = PublicManifest::new("a1".into(), catalog, &config).unwrap();
    let host = Host::new(vec![
        HostedMatch::new(credentials, manifest, broker.clone()).unwrap(),
    ])
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (done_tx, done_rx) = oneshot::channel();
    let router = host.clone().router_at(address).unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async {
                let _ = done_rx.await;
            })
            .await
    });
    // Arm the independent restore path BEFORE suspending the exact verified unit.
    let restore = ResumeOnDrop(unit.clone());
    let witness: Result<(u128, String), String> = async {
        if !signal(&unit, "STOP") { return Err("failed to suspend verified worker".into()); }
        let until = Instant::now() + Duration::from_secs(1);
        loop {
            let stat = fs::read_to_string(format!("/proc/{pid}/stat")).map_err(|_| "worker stat unavailable")?;
            if stat.rsplit_once(") ").is_some_and(|(_, fields)| fields.starts_with("T ")) { break; }
            if Instant::now() >= until { return Err("worker not stopped".into()); }
            tokio::task::yield_now().await;
        }
        let pending_broker = broker.clone();
        let mut clock = tokio::spawn(async move { pending_broker.call(WorkerCommand::Clock { control: ClockControl::Tick }).await });
        if tokio::time::timeout(Duration::from_millis(100), &mut clock).await.is_ok() { return Err("Clock was not held".into()); }
        let started = Instant::now();
        // One authenticated actual HTTP Resume; unchanged production deadlines.
        let response = tokio::time::timeout(Duration::from_secs(5), async {
            let mut stream = tokio::net::TcpStream::connect(address).await.map_err(|_| "HTTP connect")?;
            let request = format!("GET /v1/matches/a1/resume HTTP/1.1\r\nHost: {address}\r\nAuthorization: Bearer {}\r\nConnection: close\r\n\r\n", capabilities.seats[0]);
            stream.write_all(request.as_bytes()).await.map_err(|_| "HTTP write")?;
            let mut bytes = Vec::new(); stream.take((mtg_server::protocol::MAX_RESUME_BYTES + 8192) as u64).read_to_end(&mut bytes).await.map_err(|_| "HTTP read")?;
            String::from_utf8(bytes).map_err(|_| "HTTP encoding")
        }).await.map_err(|_| "five-second Resume deadline")??;
        let elapsed = started.elapsed().as_millis();
        if !response.starts_with("HTTP/1.1 200 ") { return Err("HTTP Resume not successful".into()); }
        let (_, body) = response.split_once("\r\n\r\n").ok_or("HTTP body missing")?;
        let actual: SeatResume = serde_json::from_str(body).map_err(|_| "Resume decode")?;
        if actual != oracle { return Err("Resume differs from committed paused oracle".into()); }
        if clock.is_finished() { return Err("Clock unexpectedly resolved before restore".into()); }
        if !signal(&unit, "CONT") { return Err("worker restore failed".into()); }
        let result = tokio::time::timeout(Duration::from_secs(5), clock).await.map_err(|_| "Clock join deadline")?.map_err(|_| "Clock task")?.map_err(|_| "Clock unavailable")?;
        if !matches!(result, WorkerReply::Clocked { status: ExecutionStatus::ResourcePaused }) { return Err("unexpected Clock reply".into()); }
        Ok((elapsed, body.to_owned()))
    }.await;
    drop(restore); // independent finally, also on every witness error
    host.stop();
    let _ = done_tx.send(());
    let actor_joined = tokio::time::timeout(Duration::from_secs(15), actor)
        .await
        .is_ok_and(|r| r.is_ok());
    let host_joined = tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .is_ok_and(|r| matches!(r, Ok(Ok(()))));
    let group_empty = fs::read_to_string(
        Path::new("/sys/fs/cgroup")
            .join(cgroup.trim_start_matches('/'))
            .join("cgroup.events"),
    )
    .map(|s| s.lines().any(|l| l == "populated 0"))
    .unwrap_or_else(|e| e.kind() == std::io::ErrorKind::NotFound);
    let owner = File::open(directory.join("owner.lock")).unwrap();
    let lease_released = owner.try_lock().is_ok();
    let port_released = tokio::net::TcpListener::bind(address).await.is_ok();
    let unit_state = Command::new("timeout")
        .args(["3s", "systemctl"])
        .args(["--user", "show", "--property=ActiveState", "--value", &unit])
        .output()
        .unwrap();
    let unit_inactive = String::from_utf8_lossy(&unit_state.stdout).trim() == "inactive";
    private_write(&evidence.join("cleanup.json"), serde_json::to_string(&serde_json::json!({"actor_joined":actor_joined,"host_joined":host_joined,"group_empty":group_empty,"lease_released":lease_released,"port_released":port_released,"unit_inactive":unit_inactive})).unwrap().as_bytes());
    assert!(
        actor_joined
            && host_joined
            && group_empty
            && lease_released
            && port_released
            && unit_inactive,
        "cleanup proof incomplete"
    );
    let (elapsed, body) = witness.unwrap();
    private_write(&evidence.join("response.json"), body.as_bytes());
    private_write(&evidence.join("result.json"), serde_json::to_string(&serde_json::json!({"resume_ms":elapsed,"deadline_ms":5000,"exact_paused_oracle":true,"held_clock":true,"engine_build_id":oracle.engine_build_id,"server_build_id":oracle.server_build_id})).unwrap().as_bytes());
    println!(
        "bounded HTTP Resume exact paused oracle in {elapsed}ms; Clock held; cleanup verified"
    );
}
