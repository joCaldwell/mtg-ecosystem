//! Host-private offline match registration and capability management.
//!
//! The finite localhost threat model requires pre-existing private host-owned
//! parents and excludes a concurrent same-UID filesystem adversary (who could
//! also inspect process memory). We reject symlinks in every supplied path
//! component, but do NOT claim race-free operation under adversarial parent
//! renames. One persistent OS lock fences supervisors and offline CLI changes.
//! Rotation/revocation require this lock; hot rotation while serving is absent.
use crate::auth::{CredentialHashes, IssuedCapabilities};
use mtg_contracts::{Catalog, canonical, digest};
use mtg_core::{MatchConfig, Prepared};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

pub const MAX_REGISTERED_MATCHES: usize = 2;
pub const MAX_REGISTRY_FILE_BYTES: u64 = 4 * 1024 * 1024;
const VERSION: u32 = 1;
const MAX_INITIALIZER_FILES: usize = 128;
const MAX_INITIALIZER_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistryMetadata {
    pub registry_version: u32,
    pub match_id: String,
    pub engine_build_id: String,
    pub server_build_id: String,
    pub catalog_sha256: String,
    pub config_sha256: String,
    pub rules_sha256: String,
    pub commander_policy_sha256: String,
    /// Host-private: this binds the seed and must not enter a public manifest.
    pub profile_sha256: String,
}
/// Intentionally neither Debug nor Serialize: config contains a private seed.
#[derive(Clone)]
pub struct RegisteredMatch {
    pub match_id: String,
    pub catalog: Catalog,
    pub config: MatchConfig,
    pub metadata: RegistryMetadata,
    pub worker_directory: PathBuf,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CredentialFile {
    registry_version: u32,
    credentials: CredentialHashes,
}
#[derive(Serialize)]
struct IssuedFile<'a> {
    registry_version: u32,
    match_id: &'a str,
    purpose: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    initial: Option<&'a IssuedCapabilities>,
    #[serde(skip_serializing_if = "Option::is_none")]
    seat: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    capability: Option<&'a str>,
}
// Input mirrors only the two issued-file shapes; never format capability data.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InitialCapabilities {
    seats: [String; 4],
    operator: String,
}
#[derive(Deserialize)]
#[serde(tag = "purpose", deny_unknown_fields)]
enum ExportInput {
    #[serde(rename = "create")]
    Create {
        registry_version: u32,
        match_id: String,
        initial: InitialCapabilities,
    },
    #[serde(rename = "rotate_seat")]
    Rotate {
        registry_version: u32,
        match_id: String,
        seat: u8,
        capability: String,
    },
}
fn valid_bootstrap_origin(origin: &str) -> bool {
    let Some(address) = origin
        .strip_prefix("http://")
        .and_then(|s| s.parse::<std::net::SocketAddr>().ok())
    else {
        return false;
    };
    (address.ip() == std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
        || address.ip() == std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST))
        && address.port() != 0
        && origin == format!("http://{address}")
}
struct Entry {
    registered: RegisteredMatch,
    credentials: CredentialHashes,
}
pub struct MatchRegistry {
    root: PathBuf,
    owner_uid: u32,
    owner_lock: File,
    entries: BTreeMap<String, Entry>,
    halted: bool,
}
fn error(value: impl std::fmt::Display) -> String {
    value.to_string()
}
fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn absolute(path: &Path) -> Result<PathBuf, String> {
    if path.as_os_str().is_empty()
        || path.components().any(|part| {
            matches!(
                part,
                Component::ParentDir | Component::CurDir | Component::Prefix(_)
            )
        })
    {
        return Err("registry paths require explicit components without traversal".into());
    }
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        std::env::current_dir()
            .map(|current| current.join(path))
            .map_err(error)
    }
}
fn no_symlinks(path: &Path) -> Result<(), String> {
    let mut prefix = PathBuf::new();
    for component in path.components() {
        prefix.push(component.as_os_str());
        let metadata = fs::symlink_metadata(&prefix).map_err(error)?;
        if metadata.file_type().is_symlink() {
            return Err("registry path contains a symlink".into());
        }
    }
    Ok(())
}
#[cfg(unix)]
fn private_directory(path: &Path, owner: Option<u32>) -> Result<u32, String> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    no_symlinks(path)?;
    let metadata = fs::symlink_metadata(path).map_err(error)?;
    if !metadata.is_dir()
        || metadata.permissions().mode() & 0o777 != 0o700
        || owner.is_some_and(|expected| expected != metadata.uid())
    {
        return Err("registry directory must be private and host-owned".into());
    }
    Ok(metadata.uid())
}
#[cfg(not(unix))]
fn private_directory(_: &Path, _: Option<u32>) -> Result<u32, String> {
    Err("registry requires Unix private directory semantics".into())
}
fn create_directory(path: &Path, owner: u32) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        fs::DirBuilder::new()
            .mode(0o700)
            .create(path)
            .map_err(error)?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(error)?;
        private_directory(path, Some(owner))?;
        sync_directory(path.parent().ok_or("missing directory parent")?)
    }
    #[cfg(not(unix))]
    {
        let _ = (path, owner);
        Err("registry requires Unix".into())
    }
}
fn sync_directory(path: &Path) -> Result<(), String> {
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(error)
}
#[cfg(unix)]
fn private_file(path: &Path, owner: u32) -> Result<fs::Metadata, String> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    no_symlinks(path)?;
    let metadata = fs::symlink_metadata(path).map_err(error)?;
    if !metadata.is_file()
        || metadata.permissions().mode() & 0o777 != 0o600
        || metadata.uid() != owner
        || metadata.nlink() != 1
    {
        return Err("registry file must be private, regular, host-owned and single-link".into());
    }
    Ok(metadata)
}
#[cfg(not(unix))]
fn private_file(_: &Path, _: u32) -> Result<fs::Metadata, String> {
    Err("registry requires Unix".into())
}
fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        left.dev() == right.dev() && left.ino() == right.ino()
    }
    #[cfg(not(unix))]
    {
        let _ = (left, right);
        false
    }
}
fn read_private(path: &Path, owner: u32) -> Result<String, String> {
    let expected = private_file(path, owner)?;
    if expected.len() > MAX_REGISTRY_FILE_BYTES {
        return Err("registry file exceeds bound".into());
    }
    let file = File::open(path).map_err(error)?;
    if !same_file(&expected, &file.metadata().map_err(error)?)
        || !same_file(&expected, &private_file(path, owner)?)
    {
        return Err("registry file changed while opening".into());
    }
    let mut bytes = vec![];
    file.take(MAX_REGISTRY_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(error)?;
    if bytes.len() as u64 > MAX_REGISTRY_FILE_BYTES {
        return Err("registry file grew beyond bound".into());
    }
    String::from_utf8(bytes).map_err(error)
}
fn write_new(path: &Path, bytes: &[u8], owner: u32) -> Result<(), String> {
    if bytes.len() as u64 > MAX_REGISTRY_FILE_BYTES {
        return Err("registry file exceeds bound".into());
    }
    private_directory(path.parent().ok_or("missing file parent")?, Some(owner))?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(error)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(error)?;
    }
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(error)?;
    let metadata = private_file(path, owner)?;
    if !same_file(&metadata, &file.metadata().map_err(error)?) {
        return Err("registry file changed while writing".into());
    }
    sync_directory(path.parent().ok_or("missing file parent")?)
}
fn validate_credentials(credentials: &CredentialHashes, id: &str) -> Result<(), String> {
    if credentials.match_id != id || !valid_id(id) {
        return Err("credential match identity mismatch".into());
    }
    let mut hashes = BTreeSet::new();
    for hash in credentials
        .seats
        .iter()
        .flatten()
        .chain(std::iter::once(&credentials.operator))
    {
        if *hash == [0; 32] || !hashes.insert(*hash) {
            return Err("invalid or ambiguous credential verifiers".into());
        }
    }
    Ok(())
}
/// Hosting admission is selected by profile data; catalog/config integrity is
/// still enforced by Prepared::admit_pod before any durable state is written.
fn hosted_profile(config: &MatchConfig) -> Result<(), String> {
    let p = mtg_core::match_profile(&config.profile).ok_or("unknown hosting profile")?;
    p.require_feature(p.catalog(), mtg_core::Feature::Hosting)
        .map_err(|e| e.to_string())
}
fn validate_metadata(
    metadata: &RegistryMetadata,
    id: &str,
    catalog: &Catalog,
    config: &MatchConfig,
) -> Result<(), String> {
    hosted_profile(config)?;
    let prepared = Prepared::admit_pod(catalog.clone(), config.clone())
        .map_err(|e| format!("registry match admission: {e:?}"))?;
    if metadata.registry_version != VERSION
        || metadata.match_id != id
        || metadata.engine_build_id != mtg_core::BUILD_ID
        || metadata.server_build_id != *crate::SERVER_BUILD_ID
        || metadata.catalog_sha256 != digest(catalog)?
        || metadata.config_sha256 != digest(config)?
        || metadata.rules_sha256 != catalog.rules_sha256
        || metadata.commander_policy_sha256 != mtg_core::policy_digest(&config.profile)?
        || metadata.profile_sha256 != prepared.profile_digest()
    {
        return Err("registry engine/catalog/config identity mismatch".into());
    }
    Ok(())
}
/// Fsync a bounded fresh initializer tree. Only directories need700 here:
/// SQLite's own files are protected by their enclosing private worker directory.
fn sync_initializer_tree(
    path: &Path,
    owner: u32,
    depth: usize,
    files: &mut usize,
    bytes: &mut u64,
) -> Result<(), String> {
    if depth > 8 {
        return Err("initializer tree exceeds depth bound".into());
    }
    private_directory(path, Some(owner))?;
    for entry in fs::read_dir(path).map_err(error)? {
        let entry = entry.map_err(error)?;
        let child = entry.path();
        let metadata = fs::symlink_metadata(&child).map_err(error)?;
        if metadata.file_type().is_symlink() {
            return Err("initializer tree contains a symlink".into());
        }
        *files = files.checked_add(1).ok_or("initializer count overflow")?;
        if *files > MAX_INITIALIZER_FILES {
            return Err("initializer tree exceeds file bound".into());
        }
        if metadata.is_dir() {
            sync_initializer_tree(&child, owner, depth + 1, files, bytes)?;
        } else if metadata.is_file() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                if metadata.uid() != owner || metadata.nlink() != 1 {
                    return Err("initializer file ownership/link mismatch".into());
                }
            }
            *bytes = bytes
                .checked_add(metadata.len())
                .ok_or("initializer byte count overflow")?;
            if *bytes > MAX_INITIALIZER_BYTES {
                return Err("initializer tree exceeds byte bound".into());
            }
            File::open(child)
                .and_then(|file| file.sync_all())
                .map_err(error)?;
        } else {
            return Err("initializer tree contains a nonregular entry".into());
        }
    }
    sync_directory(path)
}
impl MatchRegistry {
    pub fn open(root: &Path) -> Result<Self, String> {
        let root = absolute(root)?;
        let parent = root.parent().ok_or("registry root requires a parent")?;
        let owner_uid = private_directory(parent, None)?;
        match fs::symlink_metadata(&root) {
            Ok(_) => {
                private_directory(&root, Some(owner_uid))?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                create_directory(&root, owner_uid)?
            }
            Err(e) => return Err(error(e)),
        }
        let lock_path = root.join("registry.lock");
        if !lock_path.try_exists().map_err(error)? {
            write_new(&lock_path, b"", owner_uid)?;
        }
        let expected = private_file(&lock_path, owner_uid)?;
        let owner_lock = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&lock_path)
            .map_err(error)?;
        if !same_file(&expected, &owner_lock.metadata().map_err(error)?)
            || !same_file(&expected, &private_file(&lock_path, owner_uid)?)
        {
            return Err("registry owner lock changed".into());
        }
        owner_lock
            .try_lock()
            .map_err(|_| "registry already owned by another supervisor")?;
        let matches = root.join("matches");
        match fs::symlink_metadata(&matches) {
            Ok(_) => {
                private_directory(&matches, Some(owner_uid))?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                create_directory(&matches, owner_uid)?
            }
            Err(e) => return Err(error(e)),
        }
        let mut registry = Self {
            root,
            owner_uid,
            owner_lock,
            entries: BTreeMap::new(),
            halted: false,
        };
        registry.entries = registry.scan()?;
        Ok(registry)
    }
    fn ready(&self) -> Result<(), String> {
        if self.halted {
            return Err("registry halted after ambiguous persistence failure".into());
        }
        private_directory(&self.root, Some(self.owner_uid))?;
        let lock = private_file(&self.root.join("registry.lock"), self.owner_uid)?;
        if !same_file(&lock, &self.owner_lock.metadata().map_err(error)?) {
            return Err("registry lock identity changed".into());
        }
        Ok(())
    }
    fn directory(&self, id: &str) -> Result<PathBuf, String> {
        if !valid_id(id) {
            return Err("invalid match identifier".into());
        }
        Ok(self.root.join("matches").join(id))
    }
    fn scan(&self) -> Result<BTreeMap<String, Entry>, String> {
        self.ready()?;
        let matches = self.root.join("matches");
        private_directory(&matches, Some(self.owner_uid))?;
        let mut result = BTreeMap::new();
        for entry in fs::read_dir(&matches).map_err(error)? {
            let entry = entry.map_err(error)?;
            let id = entry
                .file_name()
                .into_string()
                .map_err(|_| "invalid registry directory name")?;
            if !valid_id(&id) {
                return Err(
                    "incomplete or unexpected registry entry; operator inspection required".into(),
                );
            }
            if result.len() >= MAX_REGISTERED_MATCHES {
                return Err("registry exceeds match capacity".into());
            }
            let directory = self.directory(&id)?;
            private_directory(&directory, Some(self.owner_uid))?;
            let metadata: RegistryMetadata = crate::codec::exact(&read_private(
                &directory.join("record.json"),
                self.owner_uid,
            )?)?;
            let catalog: Catalog = crate::codec::exact(&read_private(
                &directory.join("catalog.json"),
                self.owner_uid,
            )?)?;
            catalog.validate()?;
            let config: MatchConfig = crate::codec::exact(&read_private(
                &directory.join("config.json"),
                self.owner_uid,
            )?)?;
            validate_metadata(&metadata, &id, &catalog, &config)?;
            let credentials: CredentialFile = crate::codec::exact(&read_private(
                &directory.join("credentials.json"),
                self.owner_uid,
            )?)?;
            if credentials.registry_version != VERSION {
                return Err("unsupported registry credential version".into());
            }
            validate_credentials(&credentials.credentials, &id)?;
            let worker_directory = directory.join("worker");
            private_directory(&worker_directory, Some(self.owner_uid))?;
            result.insert(
                id.clone(),
                Entry {
                    registered: RegisteredMatch {
                        match_id: id,
                        catalog,
                        config,
                        metadata,
                        worker_directory,
                    },
                    credentials: credentials.credentials,
                },
            );
        }
        Ok(result)
    }
    fn verify_current(&self) -> Result<(), String> {
        let current = self.scan()?;
        if current.len() != self.entries.len()
            || current.iter().any(|(id, entry)| {
                self.entries.get(id).is_none_or(|known| {
                    known.registered.metadata != entry.registered.metadata
                        || known.credentials != entry.credentials
                })
            })
        {
            return Err("registry changed outside its owner".into());
        }
        Ok(())
    }
    pub fn is_halted(&self) -> bool {
        self.halted
    }
    pub fn load_all_matches(&self) -> Result<Vec<RegisteredMatch>, String> {
        self.verify_current()?;
        Ok(self
            .entries
            .values()
            .map(|entry| entry.registered.clone())
            .collect())
    }
    pub fn snapshot_auth(&self) -> Result<BTreeMap<String, CredentialHashes>, String> {
        self.verify_current()?;
        Ok(self
            .entries
            .iter()
            .map(|(id, entry)| (id.clone(), entry.credentials.clone()))
            .collect())
    }
    pub fn match_path(&self, id: &str) -> Result<PathBuf, String> {
        self.ready()?;
        if !self.entries.contains_key(id) {
            return Err("unknown registry match".into());
        }
        let directory = self.directory(id)?;
        private_directory(&directory, Some(self.owner_uid))?;
        Ok(directory)
    }
    pub fn worker_path(&self, id: &str) -> Result<PathBuf, String> {
        let worker = self.match_path(id)?.join("worker");
        private_directory(&worker, Some(self.owner_uid))?;
        Ok(worker)
    }
    fn issue_file(&self, path: &Path, value: &IssuedFile<'_>) -> Result<(), String> {
        let path = absolute(path)?;
        if path.starts_with(&self.root) {
            return Err("issued capability file must be outside the registry".into());
        }
        private_directory(
            path.parent().ok_or("missing issued-file parent")?,
            Some(self.owner_uid),
        )?;
        write_new(&path, canonical(value)?.as_bytes(), self.owner_uid)
    }
    /// Offline export of an already issued, still valid seat credential. Does
    /// not rotate authority. Registry lock and private create-new output apply.
    pub fn export_seat(
        &self,
        id: &str,
        seat: u8,
        origin: &str,
        issued: &Path,
        output: &Path,
    ) -> Result<(), String> {
        self.verify_current()?;
        if seat >= 4 || !valid_bootstrap_origin(origin) {
            return Err("invalid bootstrap destination or seat".into());
        }
        let entry = self.entries.get(id).ok_or("unknown registry match")?;
        let input = read_private(&absolute(issued)?, self.owner_uid)?;
        // Do not propagate deserialization diagnostics: unknown keys can contain secrets.
        let input: ExportInput =
            mtg_contracts::strict_read(&input).map_err(|_| "invalid issued capability file")?;
        let capability = match input {
            ExportInput::Create {
                registry_version,
                match_id,
                initial,
            } if registry_version == VERSION && match_id == id => {
                if !entry.credentials.authenticate_operator(&initial.operator) {
                    return Err("issued file authorization mismatch".into());
                }
                initial.seats[usize::from(seat)].clone()
            }
            ExportInput::Rotate {
                registry_version,
                match_id,
                seat: input_seat,
                capability,
            } if registry_version == VERSION && match_id == id && input_seat == seat => capability,
            _ => return Err("issued file identity mismatch".into()),
        };
        if entry.credentials.authenticate_seat(&capability) != Some(seat) {
            return Err("issued seat is no longer authorized".into());
        }
        let metadata = &entry.registered.metadata;
        let bootstrap = crate::protocol::SeatBootstrap {
            bootstrap_version: 1,
            protocol_version: crate::protocol::PROTOCOL_VERSION,
            server_origin: origin.into(),
            match_id: id.into(),
            viewer: mtg_core::PlayerId(seat),
            capability,
            engine_build_id: metadata.engine_build_id.clone(),
            server_build_id: metadata.server_build_id.clone(),
            catalog_sha256: metadata.catalog_sha256.clone(),
        };
        let body = canonical(&bootstrap)?;
        if body.len() > crate::protocol::MAX_BOOTSTRAP_BYTES {
            return Err("bootstrap exceeds bound".into());
        }
        let output = absolute(output)?;
        if output.starts_with(&self.root) {
            return Err("bootstrap must be outside registry".into());
        }
        write_new(&output, body.as_bytes(), self.owner_uid)
    }
    /// Offline only. Initializer must create and CLOSE its Worker at the supplied
    /// path before returning; SQLite handles must not remain open across rename.
    pub fn create(
        &mut self,
        id: &str,
        catalog: &Catalog,
        config: &MatchConfig,
        issued_path: &Path,
        initializer: impl FnOnce(&Path, &Catalog, &MatchConfig) -> Result<(), String>,
    ) -> Result<RegisteredMatch, String> {
        self.verify_current()?;
        if !valid_id(id) {
            return Err("invalid match identifier".into());
        }
        if self.entries.len() >= MAX_REGISTERED_MATCHES {
            return Err("registry match capacity reached".into());
        }
        if self.entries.contains_key(id) {
            return Err("match already registered".into());
        }
        hosted_profile(config)?;
        let prepared = Prepared::admit_pod(catalog.clone(), config.clone())
            .map_err(|e| format!("registry match admission: {e:?}"))?;
        let catalog_body = canonical(catalog)?;
        let config_body = canonical(config)?;
        if catalog_body.len() as u64 > MAX_REGISTRY_FILE_BYTES
            || config_body.len() as u64 > MAX_REGISTRY_FILE_BYTES
        {
            return Err("registry configuration exceeds bound".into());
        }
        let metadata = RegistryMetadata {
            registry_version: VERSION,
            match_id: id.into(),
            engine_build_id: mtg_core::BUILD_ID.into(),
            server_build_id: crate::SERVER_BUILD_ID.to_string(),
            catalog_sha256: digest(catalog)?,
            config_sha256: digest(config)?,
            rules_sha256: catalog.rules_sha256.clone(),
            commander_policy_sha256: mtg_core::policy_digest(&config.profile)?,
            profile_sha256: prepared.profile_digest().into(),
        };
        let (credentials, issued) = CredentialHashes::issue(id)?;
        validate_credentials(&credentials, id)?;
        let staging = self.root.join("matches").join(format!(".new-{id}"));
        let destination = self.directory(id)?;
        create_directory(&staging, self.owner_uid)?;
        let preparation = (|| {
            write_new(
                &staging.join("catalog.json"),
                catalog_body.as_bytes(),
                self.owner_uid,
            )?;
            write_new(
                &staging.join("config.json"),
                config_body.as_bytes(),
                self.owner_uid,
            )?;
            write_new(
                &staging.join("credentials.json"),
                canonical(&CredentialFile {
                    registry_version: VERSION,
                    credentials: credentials.clone(),
                })?
                .as_bytes(),
                self.owner_uid,
            )?;
            initializer(&staging.join("worker"), catalog, config)?;
            sync_initializer_tree(&staging.join("worker"), self.owner_uid, 0, &mut 0, &mut 0)?;
            write_new(
                &staging.join("record.json"),
                canonical(&metadata)?.as_bytes(),
                self.owner_uid,
            )?;
            sync_directory(&staging)?;
            self.issue_file(
                issued_path,
                &IssuedFile {
                    registry_version: VERSION,
                    match_id: id,
                    purpose: "create",
                    initial: Some(&issued),
                    seat: None,
                    capability: None,
                },
            )
        })();
        if let Err(reason) = preparation {
            // This directory was created by this call under the held lock and
            // trusted private parent. Never remove an existing registered match.
            if fs::remove_dir_all(&staging).is_err()
                || sync_directory(staging.parent().ok_or("missing staging parent")?).is_err()
            {
                self.halted = true;
            }
            return Err(reason);
        }
        if destination.try_exists().map_err(error)? {
            self.halted = true;
            return Err("registry destination appeared unexpectedly".into());
        }
        if let Err(reason) = fs::rename(&staging, &destination)
            .map_err(error)
            .and_then(|()| sync_directory(destination.parent().ok_or("missing match parent")?))
        {
            self.halted = true;
            return Err(reason);
        }
        let registered = RegisteredMatch {
            match_id: id.into(),
            catalog: catalog.clone(),
            config: config.clone(),
            metadata,
            worker_directory: destination.join("worker"),
        };
        self.entries.insert(
            id.into(),
            Entry {
                registered: registered.clone(),
                credentials,
            },
        );
        Ok(registered)
    }
    fn persist_credentials(
        &mut self,
        id: &str,
        next: CredentialHashes,
    ) -> Result<CredentialHashes, String> {
        let directory = self.directory(id)?;
        let temporary = directory.join(".credentials.pending");
        write_new(
            &temporary,
            canonical(&CredentialFile {
                registry_version: VERSION,
                credentials: next.clone(),
            })?
            .as_bytes(),
            self.owner_uid,
        )?;
        private_file(&directory.join("credentials.json"), self.owner_uid)?;
        if let Err(reason) = fs::rename(&temporary, directory.join("credentials.json"))
            .map_err(error)
            .and_then(|()| sync_directory(&directory))
        {
            self.halted = true;
            return Err(reason);
        }
        self.entries
            .get_mut(id)
            .ok_or("unknown registry match")?
            .credentials = next.clone();
        Ok(next)
    }
    /// Offline management: the registry-wide supervisor lock prevents hot
    /// rotation while serving. The next serve snapshots the durable verifiers.
    pub fn rotate(
        &mut self,
        id: &str,
        seat: u8,
        issued_path: &Path,
    ) -> Result<CredentialHashes, String> {
        self.verify_current()?;
        let current = &self
            .entries
            .get(id)
            .ok_or("unknown registry match")?
            .credentials;
        let (next, capability) = current.rotate(seat)?;
        validate_credentials(&next, id)?;
        self.issue_file(
            issued_path,
            &IssuedFile {
                registry_version: VERSION,
                match_id: id,
                purpose: "rotate_seat",
                initial: None,
                seat: Some(seat),
                capability: Some(&capability),
            },
        )?;
        self.persist_credentials(id, next)
    }
    pub fn revoke(&mut self, id: &str, seat: u8) -> Result<CredentialHashes, String> {
        self.verify_current()?;
        let current = &self
            .entries
            .get(id)
            .ok_or("unknown registry match")?
            .credentials;
        let mut next = current.clone();
        next.revoke(seat)?;
        if next == *current {
            return Ok(next);
        }
        self.persist_credentials(id, next)
    }
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
