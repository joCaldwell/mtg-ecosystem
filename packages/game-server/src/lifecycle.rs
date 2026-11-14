//! Product-owned one-match lifecycle. Registry mutation stays offline; there is
//! no shared hot registry. Stdin EOF is the supervisor lifetime lease.
use crate::{
    broker::Broker,
    ipc::{WorkerCommand, WorkerReply},
    protocol::{ResumeState, RulesResult},
    registry::MatchRegistry,
    worker::{ClockControl, Worker},
};
use mtg_core::{Decklist, MatchConfig, PlayerId, match_profile, validate_pod};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub version: u32,
    pub generation: u64,
    pub id: String,
    pub profile: String,
    pub engine_build_id: String,
    pub server_build_id: String,
    pub catalog_sha256: String,
    pub deck_sha256: String,
    pub deck: Decklist,
    pub human_origin: String,
}
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Wait {
    token: Option<String>,
    since: Option<u64>,
}
#[derive(Serialize)]
struct Status<'a> {
    version: u32,
    human_waiting_since: Option<u64>,
    human_decision_token: Option<&'a str>,
    outcome: &'a Option<RulesResult>,
}
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn private_dir(path: &Path) -> Result<(), String> {
    // Existing path ancestors may be public; the lifecycle root itself and all
    // descendants must be private. No symlinks in any existing component.
    let mut prefix = PathBuf::new();
    for part in path.components() {
        if matches!(
            part,
            std::path::Component::ParentDir | std::path::Component::CurDir
        ) {
            return Err("invalid lifecycle path".into());
        }
        prefix.push(part.as_os_str());
        if let Ok(meta) = fs::symlink_metadata(&prefix) {
            if meta.file_type().is_symlink() {
                return Err("symlink lifecycle path".into());
            }
        }
    }
    match fs::create_dir(path) {
        Ok(()) => fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(err)?,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(err(e)),
    }
    let meta = fs::symlink_metadata(path).map_err(err)?;
    if meta.uid() != fs::metadata("/proc/self").map_err(err)?.uid()
        || !meta.is_dir()
        || meta.permissions().mode() & 0o777 != 0o700
    {
        return Err("lifecycle directory must be 0700".into());
    }
    Ok(())
}
fn read<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let m = fs::symlink_metadata(path).map_err(err)?;
    if !m.is_file() || m.permissions().mode() & 0o777 != 0o600 || m.len() > 65_536 {
        return Err("invalid private lifecycle file".into());
    }
    let bytes = fs::read_to_string(path).map_err(err)?;
    mtg_contracts::strict_read(&bytes).map_err(|_| "invalid lifecycle data".into())
}
fn create(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(err)?;
    file.write_all(bytes).map_err(err)?;
    file.sync_all().map_err(err)?;
    File::open(path.parent().ok_or("missing parent")?)
        .and_then(|f| f.sync_all())
        .map_err(err)
}
fn save(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let temporary = path.with_extension("pending");
    // Only the lifecycle lock owner writes these replaceable projections.
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(err)?;
    file.write_all(mtg_contracts::canonical(value)?.as_bytes())
        .map_err(err)?;
    file.sync_all().map_err(err)?;
    fs::rename(&temporary, path).map_err(err)?;
    File::open(path.parent().ok_or("missing parent")?)
        .and_then(|f| f.sync_all())
        .map_err(err)
}
fn check(binding: &Binding) -> Result<MatchConfig, String> {
    if binding.version != 1
        || binding.id.len() != 32
        || !binding
            .id
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        || binding.engine_build_id != mtg_core::BUILD_ID
        || binding.server_build_id != *crate::SERVER_BUILD_ID
        || !crate::delivery::is_hash(&binding.deck_sha256)
    {
        return Err("lifecycle identity differs".into());
    }
    let address = binding
        .human_origin
        .strip_prefix("http://")
        .and_then(|v| v.parse::<std::net::SocketAddr>().ok())
        .ok_or("invalid human origin")?;
    if address.port() == 0
        || address.ip() != std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
            && address.ip() != std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST)
        || binding.human_origin != format!("http://{address}")
    {
        return Err("invalid human origin".into());
    }
    let unique = binding
        .deck
        .cards
        .iter()
        .map(|c| &c.oracle_id)
        .collect::<std::collections::BTreeSet<_>>();
    if binding.deck.cards.len() > 100 || unique.len() != binding.deck.cards.len() {
        return Err("invalid lifecycle deck rows".into());
    }
    let p = match_profile(&binding.profile).ok_or("unsupported lifecycle profile")?;
    if p.catalog_sha256 != binding.catalog_sha256 {
        return Err("lifecycle catalog differs".into());
    }
    p.require_feature(p.catalog(), mtg_core::Feature::Hosting)
        .map_err(err)?;
    let mut seed = [0; 32];
    getrandom::fill(&mut seed).map_err(err)?;
    let mut config = MatchConfig::for_profile(&binding.profile, seed, PlayerId(0)).map_err(err)?;
    config.decks[0] = binding.deck.clone();
    validate_pod(p.catalog(), &config).map_err(|_| "unsupported lifecycle deck")?;
    Ok(config)
}
/// Each stop advances the durable generation before waiting for the owner.
/// A delayed ensure carries its original generation and cannot cross this fence.
/// A later explicit recovery can own the next generation of the SAME match.
pub fn stop(root: &Path) -> Result<(), String> {
    private_dir(root)?;
    let fence_lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(root.join("fence.lock"))
        .map_err(err)?;
    fence_lock.lock().map_err(err)?;
    let next = generation(root)?
        .checked_add(1)
        .ok_or("lifecycle generation exhausted")?;
    save(&root.join("generation.json"), &next)?;
    drop(fence_lock);
    join(root)
}
pub fn join(root: &Path) -> Result<(), String> {
    private_dir(root)?;
    let lease = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(root.join("owner.lock"))
        .map_err(err)?;
    lease.lock().map_err(err)?;
    quiesce(root)
}
/// Query/stop only our persisted generated transient service; never a caller
/// supplied unit or an arbitrary cgroup. Inactive/not-found proves no late worker
/// can launch after the lifecycle owner has released its kernel lease.
fn systemctl(arguments: &[&str]) -> Result<String, String> {
    use std::process::{Command, Stdio};
    let mut child = Command::new("systemctl")
        .arg("--user")
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(err)?;
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        if child.try_wait().map_err(err)?.is_some() {
            break;
        }
        if Instant::now() >= until {
            let _ = child.kill();
            let _ = child.wait();
            return Err("worker unit control deadline".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let mut output = String::new();
    child
        .stdout
        .take()
        .ok_or("worker control output unavailable")?
        .take(4097)
        .read_to_string(&mut output)
        .map_err(err)?;
    if output.len() > 4096 {
        return Err("worker control output exceeds bound".into());
    }
    Ok(output)
}
fn unit_inactive(unit: &str) -> Result<bool, String> {
    let output = systemctl(&[
        "show",
        "--property=LoadState,ActiveState,ControlGroup",
        unit,
    ])?;
    if output.lines().any(|s| s == "LoadState=not-found") {
        return Ok(true);
    }
    if !output
        .lines()
        .any(|s| s == "ActiveState=inactive" || s == "ActiveState=failed")
    {
        return Ok(false);
    }
    let group = output
        .lines()
        .find_map(|s| s.strip_prefix("ControlGroup="))
        .ok_or("worker unit state unavailable")?;
    if group.is_empty() {
        return Ok(true);
    }
    let directory = crate::ipc::cgroup_directory(group)?;
    match fs::read_to_string(directory.join("cgroup.events")) {
        Ok(events) => Ok(events.lines().any(|s| s == "populated 0")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(true),
        Err(_) => Err("worker group unavailable".into()),
    }
}
fn quiesce(root: &Path) -> Result<(), String> {
    let path = root.join("worker-unit.json");
    if !path.exists() {
        return Ok(());
    }
    let unit: String = read(&path)?;
    let suffix = unit
        .strip_prefix("mtg-p8-")
        .and_then(|s| s.strip_suffix(".service"))
        .ok_or("invalid owned worker unit")?;
    let (pid, nonce) = suffix.split_once('-').ok_or("invalid owned worker unit")?;
    if pid.is_empty()
        || !pid.bytes().all(|b| b.is_ascii_digit())
        || nonce.len() != 32
        || !nonce
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err("invalid owned worker unit".into());
    }
    if !unit_inactive(&unit)? {
        let _ = systemctl(&["kill", "--kill-whom=all", "--signal=KILL", &unit]);
        let _ = systemctl(&["stop", &unit]);
    }
    if unit_inactive(&unit)? {
        Ok(())
    } else {
        Err("owned worker unit remains active".into())
    }
}
fn generation(root: &Path) -> Result<u64, String> {
    if root.join("generation.json").exists() {
        read(&root.join("generation.json"))
    } else {
        Ok(0)
    }
}
fn fenced(root: &Path, alive: &AtomicBool, epoch: u64) -> bool {
    generation(root).ok() != Some(epoch) || !alive.load(Ordering::SeqCst)
}
fn now() -> Result<u64, String> {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(err)?
            .as_millis(),
    )
    .map_err(err)
}

pub async fn run(root: &Path) -> Result<(), String> {
    private_dir(root)?;
    let lease = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(root.join("owner.lock"))
        .map_err(err)?;
    lease.try_lock().map_err(|_| "lifecycle already owned")?;
    let alive = Arc::new(AtomicBool::new(true));
    let (tx, rx) = tokio::sync::oneshot::channel();
    let life = alive.clone();
    // Dedicated blocking reader observes EOF even during native registration and
    // worker startup. The pipe is never inherited by a worker or bot.
    std::thread::spawn(move || {
        let mut input = std::io::BufReader::new(std::io::stdin());
        let mut line = String::new();
        let request = input.by_ref().take(65_537).read_line(&mut line);
        let value = request
            .ok()
            .filter(|n| *n <= 65_536)
            .and_then(|_| mtg_contracts::strict_read::<Binding>(&line).ok());
        let _ = tx.send(value);
        let mut byte = [0; 1];
        // No additional control data: any byte or EOF ends this lease.
        let _ = input.read(&mut byte);
        life.store(false, Ordering::SeqCst);
    });
    let binding = rx.await.map_err(err)?.ok_or("invalid lifecycle request")?;
    let proposed = check(&binding)?;
    if fenced(root, &alive, binding.generation) {
        return Err("lifecycle stopped".into());
    }
    let binding_path = root.join("binding.json");
    if binding_path.exists() {
        let mut original: Binding = read(&binding_path)?;
        original.generation = binding.generation;
        if original != binding {
            return Err("immutable lifecycle binding differs".into());
        }
    } else {
        create(
            &binding_path,
            mtg_contracts::canonical(&binding)?.as_bytes(),
        )?;
    }
    let config_path = root.join("config.json");
    let config: MatchConfig = if config_path.exists() {
        read(&config_path)?
    } else {
        create(
            &config_path,
            mtg_contracts::canonical(&proposed)?.as_bytes(),
        )?;
        proposed.clone()
    };
    let mut expected_config = proposed;
    expected_config.seed = config.seed;
    if config != expected_config {
        return Err("immutable lifecycle configuration differs".into());
    }
    let mut registry = MatchRegistry::open(&root.join("registry"))?;
    let matches = registry.load_all_matches()?;
    if matches.is_empty() {
        if fenced(root, &alive, binding.generation) {
            return Err("lifecycle stopped".into());
        }
        let profile = match_profile(&binding.profile).ok_or("unsupported profile")?;
        registry.create(
            &binding.id,
            profile.catalog(),
            &config,
            &root.join("operator.json"),
            |dir, catalog, config| {
                drop(Worker::create(
                    dir,
                    catalog.clone(),
                    config.clone(),
                    &binding.id,
                    Instant::now(),
                )?);
                Ok(())
            },
        )?;
    }
    let mut matches = registry.load_all_matches()?;
    if matches.len() != 1 {
        return Err("lifecycle requires exactly one match".into());
    }
    let item = matches.pop().ok_or("missing lifecycle match")?;
    if item.match_id != binding.id || item.config != config {
        return Err("lifecycle registry binding differs".into());
    }
    if fenced(root, &alive, binding.generation) {
        return Err("lifecycle stopped".into());
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(err)?;
    let address = listener.local_addr().map_err(err)?;
    let native_origin = format!("http://{address}");
    // Export once to an epoch-specific private directory; capabilities are stable
    // across recovery while ephemeral native destinations are freshly bound.
    let mut epoch_bytes = [0; 16];
    getrandom::fill(&mut epoch_bytes).map_err(err)?;
    let epoch = root.join(format!(
        "session-{}",
        epoch_bytes
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    ));
    private_dir(&epoch)?;
    for seat in 0..4 {
        registry.export_seat(
            &binding.id,
            seat,
            if seat == 0 {
                &binding.human_origin
            } else {
                &native_origin
            },
            &root.join("operator.json"),
            &epoch.join(format!("seat-{seat}.json")),
        )?;
    }
    let spawn = crate::process::SpawnConfig {
        expected_identity: crate::worker::WorkerIdentity {
            match_id: binding.id.clone(),
            engine_build_id: binding.engine_build_id.clone(),
            server_build_id: binding.server_build_id.clone(),
            catalog_sha256: binding.catalog_sha256.clone(),
            profile_sha256: item.metadata.profile_sha256.clone(),
        },
        binary: std::env::current_exe().map_err(err)?,
        directory: item.worker_directory.clone(),
        catalog: registry.match_path(&binding.id)?.join("catalog.json"),
        config: None,
    };
    let auth = registry
        .snapshot_auth()?
        .remove(&binding.id)
        .ok_or("missing native auth")?;
    let manifest =
        crate::protocol::PublicManifest::new(binding.id.clone(), item.catalog, &item.config)?;
    let mut waiting: Wait = if root.join("wait.json").exists() {
        read(&root.join("wait.json"))?
    } else {
        Wait::default()
    };
    let resume_lifetime: bool = if root.join("resume-lifetime.json").exists() {
        read(&root.join("resume-lifetime.json"))?
    } else {
        false
    };
    quiesce(root)?;
    let process = tokio::select! {
        result = crate::process::WorkerProcess::spawn_with_unit(spawn.clone(), Duration::from_secs(120), |unit| save(&root.join("worker-unit.json"), &unit).map_err(|_| crate::process::BridgeError::Unavailable)) => result.map_err(err)?,
        () = async { while !fenced(root, &alive, binding.generation) { tokio::time::sleep(Duration::from_millis(50)).await; } } => return Err("lifecycle stopped during startup".into()),
    };
    let resume_operator = resume_lifetime
        && matches!(
            process.status(),
            crate::protocol::ExecutionStatus::ResourcePaused
        );
    let (broker, mut task) = Broker::start(process);
    let host = crate::http::Host::new(vec![crate::http::HostedMatch::new(
        auth,
        manifest,
        broker.clone(),
    )?])?;
    let router = host.clone().router_at(address)?;
    let (halt, halted) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(async move {
        axum::serve(
            crate::network::LimitedListener::new(listener).map_err(err)?,
            router,
        )
        .with_graceful_shutdown(async {
            let _ = halted.await;
        })
        .await
        .map_err(err)
    });
    let mut allow_recovery_pause = false;
    let result = async {
        if fenced(root, &alive, binding.generation) { return Err("lifecycle stopped".into()); }
        if resume_operator {
            match broker.call(WorkerCommand::Clock { control: ClockControl::Resume }).await.map_err(|_| "native clock unavailable")? {
                WorkerReply::Clocked { .. } => save(&root.join("resume-lifetime.json"), &false)?,
                _ => return Err("native resume unavailable".into()),
            }
        }
        let mut ready = false;
        while !fenced(root, &alive, binding.generation) && !task.is_finished() && !server.is_finished() {
            let WorkerReply::Resume { resume } = broker.call(WorkerCommand::Resume { seat: 0 }).await.map_err(|_| "native resume unavailable")? else { return Err("native resume unavailable".into()); };
            allow_recovery_pause = matches!(resume.lifecycle, Some(crate::protocol::ExecutionStatus::Running | crate::protocol::ExecutionStatus::Waiting | crate::protocol::ExecutionStatus::Applied | crate::protocol::ExecutionStatus::Queued));
            let token = match &resume.state { ResumeState::Available { observation } => observation.decision.as_ref().map(|d| d.token().0.clone()), ResumeState::Pending => None };
            if token != waiting.token { waiting = Wait { since: if token.is_some() { Some(now()?) } else { None }, token }; save(&root.join("wait.json"), &waiting)?; }
            save(&root.join("status.json"), &Status { version: 1, human_waiting_since: waiting.since, human_decision_token: waiting.token.as_deref(), outcome: &resume.outcome })?;
            if !ready {
                println!("{}", serde_json::json!({"version":1,"origin":native_origin,"session":epoch.file_name().and_then(|n| n.to_str()).ok_or("invalid session")?}));
                std::io::stdout().flush().map_err(err)?;
                ready = true;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Ok(())
    }.await;
    // Pause before removing HTTP/consumer access; wait for the resource-group
    // owner to join. No stop acknowledgment is emitted before that join.
    if allow_recovery_pause {
        // Only our own operator pause can authorize automatic lifetime recovery.
        // Native clock/resource pauses keep their original cause and limits.
        if matches!(
            broker
                .call(WorkerCommand::Clock {
                    control: ClockControl::Pause
                })
                .await,
            Ok(WorkerReply::Clocked { .. })
        ) {
            if let Err(error) = save(&root.join("resume-lifetime.json"), &true) {
                broker.stop();
                let _ = task.await;
                server.abort();
                let _ = server.await;
                return Err(error);
            }
        }
    }
    host.stop();
    let _ = halt.send(());
    match tokio::time::timeout(Duration::from_secs(125), &mut task).await {
        Ok(Ok(())) => {}
        Ok(Err(_)) => return Err("native owner join failed".into()),
        Err(_) => {
            task.abort();
            let _ = task.await;
            return Err("native stop deadline".into());
        }
    }
    server.abort();
    let _ = server.await;
    quiesce(root)?;
    drop(registry);
    drop(lease);
    result
}

/// Read-only operator discovery of exact native pins and the reviewed public pod.
pub fn profile_info(name: &str) -> Result<(), String> {
    let profile = match_profile(name).ok_or("unknown profile")?;
    profile
        .require_feature(profile.catalog(), mtg_core::Feature::Hosting)
        .map_err(err)?;
    println!(
        "{}",
        serde_json::json!({"profile": profile.name, "catalog_sha256": profile.catalog_sha256, "engine_build_id":mtg_core::BUILD_ID, "server_build_id":*crate::SERVER_BUILD_ID, "decks":profile.decks})
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn binding() -> Binding {
        let p = match_profile("commander-foundations-v1").unwrap();
        Binding {
            version: 1,
            generation: 0,
            id: "a".repeat(32),
            profile: p.name.clone(),
            engine_build_id: mtg_core::BUILD_ID.into(),
            server_build_id: crate::SERVER_BUILD_ID.clone(),
            catalog_sha256: p.catalog_sha256.clone(),
            deck_sha256: "b".repeat(64),
            deck: p.decks[0].clone(),
            human_origin: "http://127.0.0.1:8787".into(),
        }
    }
    #[test]
    fn lifecycle_exact_identity_and_whole_pod_admission() {
        assert!(check(&binding()).is_ok());
        let mut b = binding();
        b.engine_build_id.push('0');
        assert!(check(&b).is_err());
        let mut b = binding();
        b.server_build_id.push('0');
        assert!(check(&b).is_err());
        let mut b = binding();
        b.catalog_sha256.push('0');
        assert!(check(&b).is_err());
        let mut b = binding();
        b.deck.cards[0].count = 0;
        assert!(check(&b).is_err());
        let mut b = binding();
        b.id = "../bad".into();
        assert!(check(&b).is_err());
    }
    #[test]
    fn lifecycle_stop_generations_fence_old_ensure_but_allow_explicit_recovery() {
        let mut bytes = [0; 16];
        getrandom::fill(&mut bytes).unwrap();
        let path = std::env::temp_dir().join(format!(
            "mtg-lifecycle-fence-{}",
            bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
        ));
        private_dir(&path).unwrap();
        let alive = AtomicBool::new(true);
        assert!(!fenced(&path, &alive, 0));
        stop(&path).unwrap();
        assert!(fenced(&path, &alive, 0));
        assert!(!fenced(&path, &alive, 1));
        stop(&path).unwrap();
        assert!(fenced(&path, &alive, 1));
        assert!(!fenced(&path, &alive, 2));
        alive.store(false, Ordering::SeqCst);
        assert!(fenced(&path, &alive, 2));
        assert_eq!(
            fs::metadata(path.join("generation.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
}
