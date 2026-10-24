#![cfg(unix)]
use super::*;
use std::{
    os::unix::fs::{PermissionsExt, symlink},
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Area(PathBuf);
impl Area {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "mtg-registry-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o700)).unwrap();
        fs::create_dir(p.join("issued")).unwrap();
        fs::set_permissions(p.join("issued"), fs::Permissions::from_mode(0o700)).unwrap();
        Self(p)
    }
    fn root(&self) -> PathBuf {
        self.0.join("registry")
    }
    fn out(&self, s: &str) -> PathBuf {
        self.0.join("issued").join(s)
    }
}
impl Drop for Area {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn catalog() -> Catalog {
    Catalog::read(include_str!(
        "../../game-engine/tests/fixtures/expected-match-ir.json"
    ))
    .unwrap()
}
fn config() -> MatchConfig {
    MatchConfig::foundations([17; 32], mtg_core::PlayerId(0))
}
fn init(path: &Path, _: &Catalog, _: &MatchConfig) -> Result<(), String> {
    fs::create_dir(path).map_err(error)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(error)?;
    fs::write(path.join("closed-worker"), b"initialized").map_err(error)
}
fn add(r: &mut MatchRegistry, a: &Area, id: &str) -> RegisteredMatch {
    r.create(id, &catalog(), &config(), &a.out(id), init)
        .unwrap()
}
fn json(p: &Path) -> serde_json::Value {
    serde_json::from_slice(&fs::read(p).unwrap()).unwrap()
}
fn rewrite(p: &Path, v: &serde_json::Value) {
    fs::write(p, canonical(v).unwrap()).unwrap();
}
fn keys(p: &Path) -> Vec<String> {
    let v = json(p);
    if let Some(i) = v.get("initial") {
        i["seats"]
            .as_array()
            .unwrap()
            .iter()
            .chain(std::iter::once(&i["operator"]))
            .map(|s| s.as_str().unwrap().into())
            .collect()
    } else {
        vec![v["capability"].as_str().unwrap().into()]
    }
}
fn no_keys(p: &Path, keys: &[String]) {
    for e in fs::read_dir(p).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            no_keys(&p, keys)
        } else {
            let b = fs::read(p).unwrap();
            for k in keys {
                assert!(!b.windows(k.len()).any(|x| x == k.as_bytes()));
            }
        }
    }
}
#[test]
fn private_bound_capacity_lock_and_restart() {
    let a = Area::new();
    let mut r = MatchRegistry::open(&a.root()).unwrap();
    assert!(MatchRegistry::open(&a.root()).is_err());
    let record = add(&mut r, &a, "a1");
    assert!(record.config == config());
    assert_eq!(record.metadata.engine_build_id, mtg_core::BUILD_ID);
    assert_eq!(record.metadata.catalog_sha256, digest(&catalog()).unwrap());
    assert_eq!(record.metadata.config_sha256, digest(&config()).unwrap());
    assert_eq!(
        record.metadata.commander_policy_sha256,
        mtg_core::policy_digest()
    );
    assert_eq!(record.worker_directory, r.worker_path("a1").unwrap());
    let keys = keys(&a.out("a1"));
    let auth = r.snapshot_auth().unwrap();
    for (i, k) in keys[..4].iter().enumerate() {
        assert_eq!(auth["a1"].authenticate_seat(k), Some(i as u8));
    }
    assert!(auth["a1"].authenticate_operator(&keys[4]));
    assert_eq!(
        fs::metadata(a.out("a1")).unwrap().permissions().mode() & 0o777,
        0o600
    );
    no_keys(&a.root(), &keys);
    add(&mut r, &a, "b2");
    let mut called = false;
    assert!(
        r.create("c3", &catalog(), &config(), &a.out("c3"), |_, _, _| {
            called = true;
            Ok(())
        })
        .is_err()
    );
    assert!(!called);
    assert!(!a.out("c3").exists());
    assert_eq!(
        r.load_all_matches()
            .unwrap()
            .iter()
            .map(|x| x.match_id.as_str())
            .collect::<Vec<_>>(),
        vec!["a1", "b2"]
    );
    drop(r);
    let r = MatchRegistry::open(&a.root()).unwrap();
    assert!(r.snapshot_auth().unwrap()["a1"] == auth["a1"]);
    assert!(r.load_all_matches().unwrap()[0].config == config());
}
#[test]
fn durable_rotation_revocation_and_create_new_output() {
    let a = Area::new();
    let mut r = MatchRegistry::open(&a.root()).unwrap();
    add(&mut r, &a, "aa");
    let before = r.snapshot_auth().unwrap()["aa"].clone();
    let old = keys(&a.out("aa"));
    let rotated = r.rotate("aa", 1, &a.out("rotate")).unwrap();
    let new = keys(&a.out("rotate"));
    assert_eq!(rotated.authenticate_seat(&old[1]), None);
    assert_eq!(rotated.authenticate_seat(&new[0]), Some(1));
    for i in [0, 2, 3] {
        assert!(before.seats[i] == rotated.seats[i]);
    }
    assert!(before.operator == rotated.operator);
    let output = fs::read(a.out("rotate")).unwrap();
    assert!(r.rotate("aa", 1, &a.out("rotate")).is_err());
    assert_eq!(fs::read(a.out("rotate")).unwrap(), output);
    assert!(r.snapshot_auth().unwrap()["aa"] == rotated);
    assert!(r.rotate("aa", 4, &a.out("bad")).is_err());
    assert!(!a.out("bad").exists());
    assert!(r.revoke("unknown", 0).is_err());
    let revoked = r.revoke("aa", 1).unwrap();
    assert_eq!(revoked.authenticate_seat(&new[0]), None);
    assert!(r.revoke("aa", 1).unwrap() == revoked);
    no_keys(&a.root(), &old);
    no_keys(&a.root(), &new);
    drop(r);
    assert!(
        MatchRegistry::open(&a.root())
            .unwrap()
            .snapshot_auth()
            .unwrap()["aa"]
            == revoked
    );
}
#[test]
fn failed_preparation_does_not_publish_or_activate() {
    let a = Area::new();
    let mut r = MatchRegistry::open(&a.root()).unwrap();
    assert!(
        r.create("a", &catalog(), &config(), &a.out("fail"), |_, _, _| Err(
            "initializer failed".into()
        ))
        .is_err()
    );
    assert!(r.load_all_matches().unwrap().is_empty());
    assert!(!a.out("fail").exists());
    fs::write(a.out("existing"), b"preserve").unwrap();
    assert!(
        r.create("a", &catalog(), &config(), &a.out("existing"), init)
            .is_err()
    );
    assert_eq!(fs::read(a.out("existing")).unwrap(), b"preserve");
    assert!(r.load_all_matches().unwrap().is_empty());
    assert!(
        r.create(
            "a",
            &catalog(),
            &config(),
            &a.root().join("plaintext"),
            init
        )
        .is_err()
    );
    assert!(!a.root().join("plaintext").exists());
    let mut bad = config();
    bad.decks[0].cards.clear();
    let mut called = false;
    assert!(
        r.create("a", &catalog(), &bad, &a.out("bad"), |_, _, _| {
            called = true;
            Ok(())
        })
        .is_err()
    );
    assert!(!called);
    for id in ["", "../a", "a/b", "ABC", ".", "aa..", &"a".repeat(65)] {
        assert!(
            r.create(id, &catalog(), &config(), &a.out("bad-id"), init)
                .is_err()
        );
    }
    assert!(r.load_all_matches().unwrap().is_empty());
    add(&mut r, &a, "a");
}
#[test]
fn interrupted_credentials_keep_old_auth_until_durable_publication() {
    let a = Area::new();
    let mut r = MatchRegistry::open(&a.root()).unwrap();
    add(&mut r, &a, "aa");
    let before = r.snapshot_auth().unwrap()["aa"].clone();
    let dir = r.match_path("aa").unwrap();
    write_new(
        &dir.join(".credentials.pending"),
        b"interrupted",
        r.owner_uid,
    )
    .unwrap();
    let disk = fs::read(dir.join("credentials.json")).unwrap();
    assert!(r.rotate("aa", 0, &a.out("unused")).is_err());
    assert!(r.snapshot_auth().unwrap()["aa"] == before);
    assert_eq!(before.authenticate_seat(&keys(&a.out("unused"))[0]), None);
    assert_eq!(fs::read(dir.join("credentials.json")).unwrap(), disk);
    assert!(r.revoke("aa", 0).is_err());
    assert!(!r.is_halted());
    fs::remove_file(dir.join(".credentials.pending")).unwrap();
    r.revoke("aa", 0).unwrap();
}
#[test]
fn strict_read_and_all_pinned_identity_fields_reject_tampering() {
    let a = Area::new();
    let mut r = MatchRegistry::open(&a.root()).unwrap();
    add(&mut r, &a, "aa");
    let dir = r.match_path("aa").unwrap();
    drop(r);
    for name in [
        "record.json",
        "config.json",
        "catalog.json",
        "credentials.json",
    ] {
        let p = dir.join(name);
        let bytes = fs::read(&p).unwrap();
        let mut v = json(&p);
        v.as_object_mut()
            .unwrap()
            .insert("unknown".into(), true.into());
        rewrite(&p, &v);
        assert!(MatchRegistry::open(&a.root()).is_err(), "{name}");
        fs::write(&p, &bytes).unwrap();
        let body = String::from_utf8(bytes.clone()).unwrap();
        fs::write(
            &p,
            format!("{{\"duplicate\":0,\"duplicate\":1,{}", &body[1..]),
        )
        .unwrap();
        assert!(MatchRegistry::open(&a.root()).is_err());
        fs::write(&p, bytes).unwrap();
    }
    let p = dir.join("record.json");
    let bytes = fs::read(&p).unwrap();
    for field in [
        "match_id",
        "engine_build_id",
        "catalog_sha256",
        "config_sha256",
        "rules_sha256",
        "commander_policy_sha256",
        "profile_sha256",
    ] {
        let mut v = json(&p);
        v[field] = "incorrect".into();
        rewrite(&p, &v);
        assert!(MatchRegistry::open(&a.root()).is_err(), "{field}");
        fs::write(&p, &bytes).unwrap();
    }
    let mut v = json(&p);
    v["registry_version"] = 2.into();
    rewrite(&p, &v);
    assert!(MatchRegistry::open(&a.root()).is_err());
    fs::write(&p, bytes).unwrap();
    let p = dir.join("credentials.json");
    let bytes = fs::read(&p).unwrap();
    for field in ["match_id", "zero", "duplicate", "version"] {
        let mut v = json(&p);
        match field {
            "match_id" => v["credentials"]["match_id"] = "bb".into(),
            "zero" => v["credentials"]["operator"] = serde_json::to_value([0; 32]).unwrap(),
            "duplicate" => v["credentials"]["operator"] = v["credentials"]["seats"][0].clone(),
            _ => v["registry_version"] = 2.into(),
        };
        rewrite(&p, &v);
        assert!(MatchRegistry::open(&a.root()).is_err(), "{field}");
        fs::write(&p, &bytes).unwrap();
    }
    assert!(MatchRegistry::open(&a.root()).is_ok());
}
#[test]
fn symlinks_permissions_hardlinks_and_bounded_reads_fail_closed() {
    let a = Area::new();
    let mut r = MatchRegistry::open(&a.root()).unwrap();
    add(&mut r, &a, "a");
    let dir = r.match_path("a").unwrap();
    drop(r);
    for p in [
        a.root().join("registry.lock"),
        dir.join("record.json"),
        dir.join("config.json"),
        dir.join("catalog.json"),
        dir.join("credentials.json"),
    ] {
        let backup = p.with_extension("backup");
        fs::rename(&p, &backup).unwrap();
        symlink(&backup, &p).unwrap();
        assert!(MatchRegistry::open(&a.root()).is_err());
        fs::remove_file(&p).unwrap();
        fs::rename(&backup, &p).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(MatchRegistry::open(&a.root()).is_err());
        fs::set_permissions(&p, fs::Permissions::from_mode(0o600)).unwrap();
        fs::hard_link(&p, &backup).unwrap();
        assert!(MatchRegistry::open(&a.root()).is_err());
        fs::remove_file(backup).unwrap();
    }
    let p = dir.join("config.json");
    let bytes = fs::read(&p).unwrap();
    OpenOptions::new()
        .write(true)
        .open(&p)
        .unwrap()
        .set_len(MAX_REGISTRY_FILE_BYTES + 1)
        .unwrap();
    assert!(MatchRegistry::open(&a.root()).is_err());
    fs::write(&p, bytes).unwrap();
    for p in [dir.join("worker"), dir.clone(), a.root().join("matches")] {
        let backup = p.with_extension("backup");
        fs::rename(&p, &backup).unwrap();
        symlink(&backup, &p).unwrap();
        assert!(MatchRegistry::open(&a.root()).is_err());
        fs::remove_file(&p).unwrap();
        fs::rename(&backup, &p).unwrap();
    }
    let mut r = MatchRegistry::open(&a.root()).unwrap();
    let alias = a.0.join("alias");
    symlink(a.0.join("issued"), &alias).unwrap();
    assert!(r.rotate("a", 0, &alias.join("key")).is_err());
    assert!(r.rotate("a", 0, &a.0.join("issued/../escape")).is_err());
}
#[test]
fn unsafe_parents_initializers_and_orphan_registration_are_rejected() {
    let a = Area::new();
    fs::set_permissions(&a.0, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(MatchRegistry::open(&a.root()).is_err());
    fs::set_permissions(&a.0, fs::Permissions::from_mode(0o700)).unwrap();
    let mut r = MatchRegistry::open(&a.root()).unwrap();
    assert!(
        r.create(
            "aa",
            &catalog(),
            &config(),
            &a.out("unsafe"),
            |path, _, _| {
                init(path, &catalog(), &config())?;
                symlink("/etc/passwd", path.join("link")).map_err(error)
            }
        )
        .is_err()
    );
    assert!(r.load_all_matches().unwrap().is_empty());
    assert!(!a.out("unsafe").exists());
    create_directory(&a.root().join("matches/.new-dead"), r.owner_uid).unwrap();
    drop(r);
    assert!(MatchRegistry::open(&a.root()).is_err());
}
#[test]
fn actual_worker_closes_before_rename_and_recovers_at_final_path() {
    let a = Area::new();
    let mut r = MatchRegistry::open(&a.root()).unwrap();
    let record = r
        .create("ff", &catalog(), &config(), &a.out("ff"), |p, c, m| {
            drop(crate::worker::Worker::create(
                p,
                c.clone(),
                m.clone(),
                "ff",
                Instant::now(),
            )?);
            Ok(())
        })
        .unwrap();
    drop(
        crate::worker::Worker::recover(
            &record.worker_directory,
            record.catalog.clone(),
            &record.match_id,
            Instant::now(),
        )
        .unwrap(),
    );
    drop(r);
    assert_eq!(
        MatchRegistry::open(&a.root())
            .unwrap()
            .load_all_matches()
            .unwrap()
            .len(),
        1
    );
}
#[test]
fn exactly_foundations_and_attachments_are_hosted() {
    for profile in mtg_core::MATCH_PROFILES.iter() {
        let config =
            MatchConfig::for_profile(profile.name, [17; 32], mtg_core::PlayerId(0)).unwrap();
        let fixture = match profile.name {
            mtg_core::MATCH_PROFILE => "expected-match-ir.json",
            mtg_core::ATTACHMENTS_PROFILE => "expected-attachments-ir.json",
            mtg_core::KEYWORDS_PROFILE => "expected-keywords-ir.json",
            mtg_core::COMBAT_KEYWORDS_PROFILE => "expected-combat-keywords-ir.json",
            mtg_core::TOKENS_PROFILE => "expected-tokens-ir.json",
            mtg_core::STAPLES_PROFILE => "expected-staples-ir.json",
            mtg_core::MANA_PROFILE => "expected-mana-ir.json",
            mtg_core::FOUR_COLORS_PROFILE => "expected-four-colors-ir.json",
            mtg_core::REMOVAL_PROFILE => "expected-removal-ir.json",
            mtg_core::SACRIFICE_PROFILE => "expected-sacrifice-ir.json",
            _ => panic!("review new legal profile hosting scope"),
        };
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../game-engine/tests/fixtures")
            .join(fixture);
        let catalog = Catalog::read(&fs::read_to_string(path).unwrap()).unwrap();
        Prepared::admit_pod(catalog.clone(), config.clone()).unwrap();
        let area = Area::new();
        let mut registry = MatchRegistry::open(&area.root()).unwrap();
        let result = registry.create("cafe", &catalog, &config, &area.out("cafe"), init);
        if matches!(
            profile.name,
            mtg_core::MATCH_PROFILE | mtg_core::ATTACHMENTS_PROFILE
        ) {
            let registered = result.unwrap();
            assert_eq!(registered.config, config);
            drop(registry);
            let loaded = MatchRegistry::open(&area.root())
                .unwrap()
                .load_all_matches()
                .unwrap();
            assert_eq!(loaded.len(), 1);
            assert_eq!(loaded[0].config, config);
        } else {
            let Err(error) = result else {
                panic!("unexpected hosted profile")
            };
            assert!(error.contains("registry hosts only"), "{error}");
            assert!(!area.out("cafe").exists());
        }
    }
    let mut unknown = config();
    unknown.profile = "commander-future-v1".into();
    assert!(hosted_profile(&unknown).is_err());
}
