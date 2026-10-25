//! A valid private worker is insufficient unless it is the registered game.
//! These checks use real admitted Workers, without requiring a cgroup runner.
use mtg_contracts::{Catalog, digest};
use mtg_core::{MatchConfig, PlayerId, Prepared};
use mtg_server::{
    process::verify_identity,
    worker::{Worker, WorkerIdentity},
};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!(
            "mtg-worker-identity-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        )))
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if self.0.exists() {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
}
fn catalog() -> Catalog {
    Catalog::read(include_str!(
        "../../game-engine/tests/fixtures/expected-match-ir.json"
    ))
    .unwrap()
}
fn expected(catalog: &Catalog, config: &MatchConfig) -> WorkerIdentity {
    WorkerIdentity {
        match_id: "a1".into(),
        engine_build_id: mtg_core::BUILD_ID.into(),
        server_build_id: mtg_server::SERVER_BUILD_ID.to_string(),
        catalog_sha256: digest(catalog).unwrap(),
        profile_sha256: Prepared::admit_pod(catalog.clone(), config.clone())
            .unwrap()
            .profile_digest()
            .into(),
    }
}

#[test]
fn actual_admitted_worker_identity_survives_close_rename_and_recovery() {
    let catalog = catalog();
    let config =
        MatchConfig::for_profile("commander-foundations-v1", [41; 32], PlayerId(2)).unwrap();
    let expected = expected(&catalog, &config);
    let initial = Directory::new();
    let final_path = Directory::new();
    let worker = Worker::create(&initial.0, catalog.clone(), config, "a1", Instant::now()).unwrap();
    assert_eq!(worker.identity(), &expected);
    assert!(verify_identity(&expected, worker.identity()).is_ok());
    drop(worker);
    std::fs::rename(&initial.0, &final_path.0).unwrap();
    let recovered = Worker::recover(&final_path.0, catalog, "a1", Instant::now()).unwrap();
    assert_eq!(recovered.identity(), &expected);
    assert!(verify_identity(&expected, recovered.identity()).is_ok());
}

#[test]
fn different_valid_seed_start_and_decks_cannot_recover_as_registered_game() {
    let catalog = catalog();
    let registered =
        MatchConfig::for_profile("commander-foundations-v1", [41; 32], PlayerId(2)).unwrap();
    let expected = expected(&catalog, &registered);
    let mut wrong_seed = registered.clone();
    wrong_seed.seed[31] ^= 1;
    let mut wrong_start = registered.clone();
    wrong_start.starting_player = PlayerId(1);
    let mut wrong_decks = registered;
    wrong_decks.decks = std::array::from_fn(|_| {
        mtg_core::match_profile("commander-foundations-v1")
            .unwrap()
            .decks[0]
            .clone()
    });
    for actual_config in [wrong_seed, wrong_start, wrong_decks] {
        let directory = Directory::new();
        let worker = Worker::create(
            &directory.0,
            catalog.clone(),
            actual_config.clone(),
            "a1",
            Instant::now(),
        )
        .unwrap();
        let actual = worker.identity().clone();
        assert_eq!(actual, self::expected(&catalog, &actual_config));
        assert_eq!(actual.engine_build_id, expected.engine_build_id);
        assert_eq!(actual.catalog_sha256, expected.catalog_sha256);
        assert_ne!(actual.profile_sha256, expected.profile_sha256);
        assert!(verify_identity(&expected, &actual).is_err());
        drop(worker);
        let recovered =
            Worker::recover(&directory.0, catalog.clone(), "a1", Instant::now()).unwrap();
        assert_eq!(recovered.identity(), &actual);
        assert!(verify_identity(&expected, recovered.identity()).is_err());
    }
}

#[test]
fn identical_config_copied_under_a_different_registered_match_id_rejects() {
    let catalog = catalog();
    let config =
        MatchConfig::for_profile("commander-foundations-v1", [41; 32], PlayerId(2)).unwrap();
    let original = Directory::new();
    let copied = Directory::new();
    let worker = Worker::create(
        &original.0,
        catalog.clone(),
        config.clone(),
        "a1",
        Instant::now(),
    )
    .unwrap();
    let actual = worker.identity().clone();
    let mut other_registration = expected(&catalog, &config);
    other_registration.match_id = "b2".into();
    assert_eq!(actual.profile_sha256, other_registration.profile_sha256);
    assert!(verify_identity(&other_registration, &actual).is_err());
    drop(worker);
    // Real closed bootstrap/checkpoint/SQLite bytes, not an invented identity.
    std::fs::create_dir(&copied.0).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&copied.0, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    for entry in std::fs::read_dir(&original.0).unwrap() {
        let entry = entry.unwrap();
        assert!(entry.file_type().unwrap().is_file());
        std::fs::copy(entry.path(), copied.0.join(entry.file_name())).unwrap();
    }
    let database_before = std::fs::read(copied.0.join("match.sqlite3")).unwrap();
    assert!(Worker::recover(&copied.0, catalog.clone(), "b2", Instant::now()).is_err());
    assert_eq!(
        std::fs::read(copied.0.join("match.sqlite3")).unwrap(),
        database_before,
        "wrong host namespace must reject before opening/changing durable generation"
    );
    let recovered = Worker::recover(&copied.0, catalog, "a1", Instant::now()).unwrap();
    assert_eq!(recovered.identity(), &actual);
    assert!(verify_identity(&other_registration, recovered.identity()).is_err());
    for invalid in ["", "A1", "a/1", "a1\n"] {
        let mut invalid_identity = actual.clone();
        invalid_identity.match_id = invalid.into();
        assert!(verify_identity(&invalid_identity, &invalid_identity).is_err());
    }
}

#[test]
fn every_required_binding_and_canonical_hash_encoding_is_checked() {
    let expected = expected(
        &catalog(),
        &MatchConfig::for_profile("commander-foundations-v1", [41; 32], PlayerId(2)).unwrap(),
    );
    for index in 0..4 {
        for malformed in [
            String::new(),
            "0".repeat(63),
            "0".repeat(65),
            "A".repeat(64),
            "g".repeat(64),
            format!("{}\n", "0".repeat(64)),
        ] {
            let mut actual = expected.clone();
            match index {
                0 => actual.engine_build_id = malformed,
                1 => actual.catalog_sha256 = malformed,
                2 => actual.profile_sha256 = malformed,
                _ => actual.server_build_id = malformed,
            }
            assert!(verify_identity(&expected, &actual).is_err());
            assert!(verify_identity(&actual, &expected).is_err());
            assert!(verify_identity(&actual, &actual).is_err());
        }
        let mut actual = expected.clone();
        match index {
            0 => actual.engine_build_id = "0".repeat(64),
            1 => actual.catalog_sha256 = "0".repeat(64),
            2 => actual.profile_sha256 = "0".repeat(64),
            _ => actual.server_build_id = "0".repeat(64),
        }
        assert_ne!(actual, expected);
        assert!(verify_identity(&expected, &actual).is_err());
    }
}

#[test]
fn changed_server_identity_rejects_recovery_before_opening_database() {
    let catalog = catalog();
    let config =
        MatchConfig::for_profile("commander-foundations-v1", [41; 32], PlayerId(2)).unwrap();
    let directory = Directory::new();
    let worker =
        Worker::create(&directory.0, catalog.clone(), config, "a1", Instant::now()).unwrap();
    let expected = worker.identity().clone();
    assert_eq!(expected.server_build_id, *mtg_server::SERVER_BUILD_ID);
    let mut different_host = expected.clone();
    different_host.server_build_id = "0".repeat(64);
    assert!(verify_identity(&expected, &different_host).is_err());
    drop(worker);
    let path = directory.0.join("bootstrap.json");
    let original = std::fs::read(&path).unwrap();
    let mut bootstrap: serde_json::Value = serde_json::from_slice(&original).unwrap();
    bootstrap["server_build_id"] = "0".repeat(64).into();
    std::fs::write(&path, mtg_contracts::canonical(&bootstrap).unwrap()).unwrap();
    let database_before = std::fs::read(directory.0.join("match.sqlite3")).unwrap();
    let error = Worker::recover(&directory.0, catalog.clone(), "a1", Instant::now())
        .err()
        .unwrap();
    assert_eq!(error, "worker bootstrap identity mismatch");
    assert_eq!(
        database_before,
        std::fs::read(directory.0.join("match.sqlite3")).unwrap()
    );
    std::fs::write(path, original).unwrap();
    assert_eq!(
        Worker::recover(&directory.0, catalog, "a1", Instant::now())
            .unwrap()
            .identity(),
        &expected
    );
}
