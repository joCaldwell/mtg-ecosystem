use std::{
    fs::File,
    io::Read,
    net::SocketAddr,
    path::Path,
    time::{Duration, Instant},
};

fn read_private<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    if std::fs::symlink_metadata(path)
        .map_err(|_| "private input unavailable")?
        .file_type()
        .is_symlink()
    {
        return Err("private input must not be a symlink".into());
    }
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|_| "private input unavailable")?
        .take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "private input unavailable")?;
    if bytes.len() > 4 * 1024 * 1024 {
        return Err("private input exceeds limit".into());
    }
    mtg_contracts::strict_read(std::str::from_utf8(&bytes).map_err(|_| "invalid private input")?)
}
fn schema() -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "{}",
        serde_json::to_string(&serde_json::json!({
            "protocol_version":mtg_server::protocol::PROTOCOL_VERSION,
            "routes":mtg_server::protocol::ROUTES,
            "command":mtg_server::protocol::command_schema(),
            "stream":mtg_server::protocol::stream_schema(),
            "submission":mtg_server::protocol::submission_schema(),
            "execution":mtg_server::protocol::execution_schema(),
            "manifest":mtg_server::protocol::manifest_schema(),
            "resume":mtg_server::protocol::resume_schema(),
        }))?
    );
    Ok(())
}
fn native_info() -> Result<(), Box<dyn std::error::Error>> {
    // Actual linked SQLite build options, not assumed Cargo feature names.
    // https://www.sqlite.org/pragma.html#pragma_compile_options
    let connection = rusqlite::Connection::open_in_memory()?;
    let mut statement = connection.prepare("PRAGMA compile_options")?;
    let mut options = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    options.sort();
    if options.is_empty() {
        return Err("SQLite compile option introspection unavailable".into());
    }
    println!(
        "{}",
        serde_json::to_string(&serde_json::json!({
            "engine_build_id":mtg_core::BUILD_ID,
            "server_build_id":mtg_server::SERVER_BUILD_ID.as_str(),
            "rustc":mtg_core::BUILD_RUSTC,
            "target":mtg_core::BUILD_TARGET,
            "profile":mtg_core::BUILD_PROFILE,
            "opt_level":mtg_core::BUILD_OPT_LEVEL,
            "rustflags":mtg_core::BUILD_RUSTFLAGS,
            "sqlite_version":rusqlite::version(),
            "sqlite_compile_options":options,
        }))?
    );
    Ok(())
}
async fn serve(root: &Path, address: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
    if !address.ip().is_loopback() {
        return Err("only loopback hosting is supported".into());
    }
    let registry = mtg_server::registry::MatchRegistry::open(root)?;
    let registered = registry.load_all_matches()?;
    let auth = registry.snapshot_auth()?;
    if registered.is_empty() {
        return Err("create an admitted match before serving".into());
    }
    let listener =
        mtg_server::network::LimitedListener::new(tokio::net::TcpListener::bind(address).await?)?;
    let binary = std::env::current_exe()?;
    let mut processes = Vec::new();
    for item in &registered {
        let config = mtg_server::process::SpawnConfig {
            expected_identity: mtg_server::worker::WorkerIdentity {
                match_id: item.match_id.clone(),
                engine_build_id: item.metadata.engine_build_id.clone(),
                server_build_id: item.metadata.server_build_id.clone(),
                catalog_sha256: item.metadata.catalog_sha256.clone(),
                profile_sha256: item.metadata.profile_sha256.clone(),
            },
            binary: binary.clone(),
            directory: item.worker_directory.clone(),
            catalog: registry.match_path(&item.match_id)?.join("catalog.json"),
            config: None,
        };
        match mtg_server::process::WorkerProcess::spawn(config.clone(), Duration::from_secs(120))
            .await
        {
            Ok(process) => processes.push((process, config)),
            Err(error) => {
                for (process, _) in processes {
                    let _ = process.shutdown(Duration::from_secs(5)).await;
                }
                return Err(error.into());
            }
        }
    }
    let mut hosted = Vec::new();
    let mut tasks = Vec::new();
    for (item, (process, config)) in registered.into_iter().zip(processes) {
        let manifest = mtg_server::protocol::PublicManifest::new(
            item.match_id.clone(),
            item.catalog,
            &item.config,
        )?;
        let (broker, task) = mtg_server::broker::Broker::start_recovering(process, config);
        tasks.push(task);
        hosted.push(mtg_server::http::HostedMatch::new(
            auth.get(&item.match_id)
                .ok_or("missing capability verifier")?
                .clone(),
            manifest,
            broker,
        )?);
    }
    let host = mtg_server::http::Host::new(hosted)?;
    let stopping = host.clone();
    let result = axum::serve(listener, host.clone().router())
        .with_graceful_shutdown(async move {
            let _ = tokio::signal::ctrl_c().await;
            stopping.stop();
        })
        .await;
    host.stop();
    for mut task in tasks {
        if tokio::time::timeout(Duration::from_secs(125), &mut task)
            .await
            .is_err()
        {
            task.abort();
            let _ = task.await;
        }
    }
    drop(registry);
    result?;
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    match args.first().and_then(|a|a.to_str()) {
        Some("schema") if args.len()==1=>schema(),
        Some("native-info") if args.len()==1=>native_info(),
        Some("bootstrap-schema") if args.len()==1=>{println!("{}",serde_json::to_string(&mtg_server::protocol::bootstrap_schema())?);Ok(())},
        Some("--worker") if (4..=5).contains(&args.len())=>{
            mtg_server::ipc::run_child(Path::new(&args[1]),Path::new(&args[2]),args[3].to_str().ok_or("invalid match ID")?,args.get(4).map(Path::new))?;Ok(())
        }
        Some("create") if args.len()==6=>{
            let id=args[2].to_str().ok_or("invalid match ID")?;
            let catalog=read_private(Path::new(&args[3]))?;
            let config=read_private(Path::new(&args[4]))?;
            let mut registry=mtg_server::registry::MatchRegistry::open(Path::new(&args[1]))?;
            registry.create(id,&catalog,&config,Path::new(&args[5]),|directory,catalog,config| {
                let worker=mtg_server::worker::Worker::create(directory,catalog.clone(),config.clone(),id,Instant::now())?;drop(worker);Ok(())
            })?;Ok(())
        }
        Some("export-seat") if args.len()==7=>{
            let registry=mtg_server::registry::MatchRegistry::open(Path::new(&args[1]))?;
            registry.export_seat(args[2].to_str().ok_or("invalid match ID")?,args[3].to_str().ok_or("invalid seat")?.parse()?,args[4].to_str().ok_or("invalid origin")?,Path::new(&args[5]),Path::new(&args[6]))?;Ok(())
        }
        Some("rotate") if args.len()==5=>{
            let mut registry=mtg_server::registry::MatchRegistry::open(Path::new(&args[1]))?;
            registry.rotate(args[2].to_str().ok_or("invalid match ID")?,args[3].to_str().ok_or("invalid seat")?.parse()?,Path::new(&args[4]))?;Ok(())
        }
        Some("revoke") if args.len()==4=>{
            let mut registry=mtg_server::registry::MatchRegistry::open(Path::new(&args[1]))?;
            registry.revoke(args[2].to_str().ok_or("invalid match ID")?,args[3].to_str().ok_or("invalid seat")?.parse()?)?;Ok(())
        }
        Some("serve") if args.len()==3=>{
            let address=args[2].to_str().ok_or("invalid bind address")?.parse()?;
            tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build()?.block_on(serve(Path::new(&args[1]),address))
        }
        _=>Err("usage: mtg-server schema | bootstrap-schema | native-info | export-seat ROOT ID SEAT ORIGIN ISSUED_FILE OUTPUT | create ROOT ID CATALOG CONFIG ISSUED_FILE | serve ROOT LOOPBACK:PORT | rotate ROOT ID SEAT ISSUED_FILE | revoke ROOT ID SEAT".into()),
    }
}
