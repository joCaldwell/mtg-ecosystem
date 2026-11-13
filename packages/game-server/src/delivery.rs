//! Immutable public artifacts, loaded once before workers start. No request path
//! is ever joined to a filesystem root. The offline producer owns metadata shape.
use crate::protocol::{ClientConfig, PresentationDescriptor};
use axum::{
    body::Body,
    http::{HeaderMap, Method, StatusCode, header},
    response::{IntoResponse, Response},
};
use mtg_contracts::{Catalog, sha256, strict_read};
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::Read,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    path::{Component, Path},
    sync::Arc,
};

pub const MAX_PRESENTATION_BYTES: usize = 4 * 1024 * 1024;
const MAX_FILE_BYTES: usize = 16 * 1024 * 1024;
const MAX_BUNDLE_BYTES: usize = 64 * 1024 * 1024;
pub const PRESENTATION_SCHEMA: &str = include_str!("../tools/presentation.schema.json");
// https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Content-Security-Policy
pub const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self' https://cards.scryfall.io; font-src 'self'; connect-src 'self'; base-uri 'none'; object-src 'none'; frame-ancestors 'none'; form-action 'none'; worker-src 'none'";
const SPA_PATHS: [&str; 6] = [
    "/",
    "/join",
    "/table",
    "/about",
    "/fixtures/opening",
    "/fixtures/missing-metadata",
];
const INVALID: &str = "invalid public delivery artifact";
#[derive(Clone)]
struct Asset {
    bytes: Arc<[u8]>,
    mime: &'static str,
    immutable: bool,
}
#[derive(Clone, Default)]
pub struct Delivery {
    assets: BTreeMap<String, Asset>,
    presentations: BTreeMap<String, PresentationDescriptor>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FilePin {
    path: String,
    sha256: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Bundle {
    bundle_version: u32,
    files: Vec<FilePin>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactManifest {
    artifact_version: u32,
    producer_sha256: String,
    catalog_file_sha256: String,
    catalog_sha256: String,
    protocol_schema_sha256: String,
    files: Vec<FilePin>,
}
pub fn is_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
/// Reject symlinks in every component, special files, and unbounded reads. The
/// operator owns these paths; startup snapshots remove request-time file races.
fn read(path: &Path, maximum: usize) -> Result<Vec<u8>, String> {
    if !path.is_absolute()
        || path
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
    {
        return Err(INVALID.into());
    }
    for ancestor in path.ancestors() {
        if fs::symlink_metadata(ancestor)
            .map_err(|_| INVALID)?
            .file_type()
            .is_symlink()
        {
            return Err(INVALID.into());
        }
    }
    // Check before open: opening a FIFO can block before descriptor metadata is
    // available. Operator-owned inputs must be regular files even on failure.
    let before = fs::symlink_metadata(path).map_err(|_| INVALID)?;
    if !before.is_file() || before.len() > maximum as u64 {
        return Err(INVALID.into());
    }
    let mut file = File::open(path).map_err(|_| INVALID)?;
    let metadata = file.metadata().map_err(|_| INVALID)?;
    if !metadata.is_file() || metadata.len() > maximum as u64 {
        return Err(INVALID.into());
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| INVALID)?;
    if bytes.len() > maximum {
        return Err(INVALID.into());
    }
    Ok(bytes)
}
fn pinned(path: &Path, digest: &str, maximum: usize) -> Result<Vec<u8>, String> {
    if !is_hash(digest) {
        return Err(INVALID.into());
    }
    let bytes = read(path, maximum)?;
    if sha256(&bytes) != digest {
        return Err(INVALID.into());
    }
    Ok(bytes)
}
fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, String> {
    strict_read(std::str::from_utf8(bytes).map_err(|_| INVALID)?).map_err(|_| INVALID.into())
}
fn media(path: &str) -> Option<&'static str> {
    if path == "index.html" {
        return Some("text/html; charset=utf-8");
    }
    let name = path.strip_prefix("assets/")?;
    if name.is_empty()
        || name.len() > 160
        || name.starts_with('.')
        || name.contains("..")
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
    {
        return None;
    }
    match name.rsplit('.').next()? {
        "js" => Some("text/javascript; charset=utf-8"),
        "css" => Some("text/css; charset=utf-8"),
        "woff2" => Some("font/woff2"),
        "png" => Some("image/png"),
        "webp" => Some("image/webp"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        _ => None,
    }
}
impl Delivery {
    pub fn load_bundle(root: &Path, digest: &str, registry: &Path) -> Result<Self, String> {
        // Separate roots, including the inverse overlap. No registry or operator
        // file is a candidate public asset, even if a manifest is misconfigured.
        let registry = fs::canonicalize(registry).map_err(|_| INVALID)?;
        let resolved = fs::canonicalize(root).map_err(|_| INVALID)?;
        if resolved.starts_with(&registry) || registry.starts_with(&resolved) {
            return Err(INVALID.into());
        }
        let manifest: Bundle = decode(&pinned(&root.join("client-assets.json"), digest, 65_536)?)?;
        if manifest.bundle_version != 1 || manifest.files.is_empty() || manifest.files.len() > 128 {
            return Err(INVALID.into());
        }
        let mut result = Self::default();
        let mut total = 0;
        for file in manifest.files {
            let mime = media(&file.path).ok_or(INVALID)?;
            let bytes = pinned(&root.join(&file.path), &file.sha256, MAX_FILE_BYTES)?;
            total += bytes.len();
            if total > MAX_BUNDLE_BYTES {
                return Err(INVALID.into());
            }
            let asset = Asset {
                bytes: bytes.into(),
                mime,
                immutable: false,
            };
            if result
                .assets
                .insert(format!("/{}", file.path), asset)
                .is_some()
            {
                return Err(INVALID.into());
            }
        }
        let index = result.assets.get("/index.html").ok_or(INVALID)?.clone();
        for route in SPA_PATHS {
            result.assets.insert(route.into(), index.clone());
        }
        Ok(result)
    }
    pub fn add_presentation(
        &mut self,
        root: &Path,
        digest: &str,
        catalog: &Catalog,
    ) -> Result<(), String> {
        let manifest: ArtifactManifest =
            decode(&pinned(&root.join("artifacts.json"), digest, 65_536)?)?;
        let catalog_hash = mtg_contracts::digest(catalog)?;
        if manifest.artifact_version != 1
            || manifest.producer_sha256 != env!("MTG_PRESENTATION_PRODUCER_SHA256")
            || manifest.catalog_sha256 != catalog_hash
            || !is_hash(&manifest.catalog_file_sha256)
            || !is_hash(&manifest.protocol_schema_sha256)
            || manifest.files.len() != 3
        {
            return Err(INVALID.into());
        }
        let mut pins = BTreeMap::new();
        for file in manifest.files {
            if ![
                "presentation.json",
                "presentation.schema.json",
                "request-inventory.json",
            ]
            .contains(&file.path.as_str())
                || !is_hash(&file.sha256)
                || pins.insert(file.path, file.sha256).is_some()
            {
                return Err(INVALID.into());
            }
        }
        let schema_digest = sha256(PRESENTATION_SCHEMA.as_bytes());
        if pins.get("presentation.schema.json") != Some(&schema_digest) {
            return Err(INVALID.into());
        }
        let schema_bytes = pinned(
            &root.join("presentation.schema.json"),
            &schema_digest,
            65_536,
        )?;
        let schema: Value = decode(&schema_bytes)?;
        let bytes = pinned(
            &root.join("presentation.json"),
            &pins["presentation.json"],
            MAX_PRESENTATION_BYTES,
        )?;
        let artifact: Value = decode(&bytes)?;
        jsonschema::draft7::options()
            .build(&schema)
            .map_err(|_| INVALID)?
            .validate(&artifact)
            .map_err(|_| INVALID)?;
        if artifact["catalog_sha256"] != catalog_hash {
            return Err(INVALID.into());
        }
        let cards = artifact["cards"].as_array().ok_or(INVALID)?;
        if cards.len() != catalog.definitions.len() {
            return Err(INVALID.into());
        }
        let sources: BTreeMap<_, _> = catalog
            .definitions
            .iter()
            .map(|d| {
                (
                    d.oracle_id.as_str(),
                    (&d.provenance.printing_id, &d.provenance.source_sha256),
                )
            })
            .collect();
        let mut seen = BTreeSet::new();
        for card in cards {
            let id = card["oracle_id"].as_str().ok_or(INVALID)?;
            let (printing, source) = sources.get(id).ok_or(INVALID)?;
            if !seen.insert(id)
                || card["printing_id"] != **printing
                || card["source_sha256"] != **source
            {
                return Err(INVALID.into());
            }
            for (index, face) in card["faces"].as_array().ok_or(INVALID)?.iter().enumerate() {
                if face["face_index"] != index {
                    return Err(INVALID.into());
                }
                if let Some(image) = face["image_url"].as_str() {
                    let uri: axum::http::Uri = image.parse().map_err(|_| INVALID)?;
                    if uri.scheme_str() != Some("https")
                        || uri.authority().is_none_or(|a| a.as_str().contains('@'))
                    {
                        return Err(INVALID.into());
                    }
                }
            }
        }
        let hash = pins["presentation.json"].clone();
        let path = format!("/presentation/{hash}.json");
        let descriptor = PresentationDescriptor {
            presentation_version: 1,
            catalog_sha256: catalog_hash.clone(),
            presentation_sha256: hash,
            presentation_schema_sha256: schema_digest,
            producer_sha256: manifest.producer_sha256,
            path: path.clone(),
        };
        if let Some(previous) = self.presentations.get(&catalog_hash) {
            if previous != &descriptor {
                return Err(INVALID.into());
            }
            return Ok(());
        }
        self.assets.insert(
            path,
            Asset {
                bytes: bytes.into(),
                mime: "application/json",
                immutable: true,
            },
        );
        self.presentations.insert(catalog_hash, descriptor);
        Ok(())
    }
    pub fn descriptor(&self, catalog: &str) -> Option<&PresentationDescriptor> {
        self.presentations.get(catalog)
    }
    pub fn response(&self, path: &str, method: &Method) -> Option<Response> {
        let asset = self.assets.get(path)?;
        // Only digest-addressed metadata is immutable. Production asset names
        // are pinned but may be reused by another build: revalidate via no-store.
        let cache = if asset.immutable {
            "public, max-age=31536000, immutable"
        } else {
            "no-store"
        };
        let mut response = (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, asset.mime),
                (header::CACHE_CONTROL, cache),
            ],
            Body::from(asset.bytes.to_vec()),
        )
            .into_response();
        response
            .headers_mut()
            .insert(header::CONTENT_LENGTH, asset.bytes.len().into());
        if method == Method::HEAD {
            *response.body_mut() = Body::empty();
        }
        Some(response)
    }
}
#[derive(Clone)]
pub struct OriginPolicy {
    pub config: ClientConfig,
    authority: String,
    browser_origin: String,
}
impl OriginPolicy {
    pub fn new(address: SocketAddr) -> Result<Self, String> {
        if address.port() == 0
            || ![
                IpAddr::V4(Ipv4Addr::LOCALHOST),
                IpAddr::V6(Ipv6Addr::LOCALHOST),
            ]
            .contains(&address.ip())
        {
            return Err("canonical numeric loopback address required".into());
        }
        let authority = address.to_string();
        // Browsers omit the default port in Host and Origin. Bootstrap/config
        // retain the explicit SocketAddr spelling, including :80.
        let browser_authority = if address.port() == 80 {
            match address.ip() {
                IpAddr::V4(_) => "127.0.0.1".into(),
                IpAddr::V6(_) => "[::1]".into(),
            }
        } else {
            authority.clone()
        };
        Ok(Self {
            config: ClientConfig {
                client_config_version: 1,
                server_origin: format!("http://{authority}"),
            },
            authority: browser_authority.clone(),
            browser_origin: format!("http://{browser_authority}"),
        })
    }
    pub fn permits(&self, headers: &HeaderMap) -> bool {
        fn one<'a>(headers: &'a HeaderMap, name: &str) -> Result<Option<&'a str>, ()> {
            let mut values = headers.get_all(name).iter();
            let first = values
                .next()
                .map(|v| v.to_str().map_err(|_| ()))
                .transpose()?;
            if values.next().is_some() {
                return Err(());
            }
            Ok(first)
        }
        one(headers, "host") == Ok(Some(self.authority.as_str()))
            && matches!(one(headers, "origin"), Ok(None) | Ok(Some(_)))
            && one(headers, "origin").is_ok_and(|v| v.is_none_or(|v| v == self.browser_origin))
            && one(headers, "sec-fetch-site")
                .is_ok_and(|v| v.is_none_or(|v| matches!(v, "same-origin" | "none")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    struct Scratch(std::path::PathBuf);
    impl Scratch {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!(
                "mtg-delivery-{}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&p).unwrap();
            Self(p)
        }
        fn put(&self, name: &str, bytes: &[u8]) {
            fs::write(self.0.join(name), bytes).unwrap();
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    #[test]
    fn origin_requires_exact_host_and_same_origin_and_rejects_rebinding() {
        let policy = OriginPolicy::new("127.0.0.1:18741".parse().unwrap()).unwrap();
        let mut h = HeaderMap::new();
        assert!(!policy.permits(&h));
        h.insert("host", "127.0.0.1:18741".parse().unwrap());
        assert!(policy.permits(&h));
        for origin in [
            "null",
            "https://evil.example",
            "http://localhost:18741",
            "http://127.0.0.1:18742",
        ] {
            h.insert("origin", origin.parse().unwrap());
            assert!(!policy.permits(&h));
        }
        h.insert("origin", "http://127.0.0.1:18741".parse().unwrap());
        assert!(policy.permits(&h));
        h.append("origin", "http://127.0.0.1:18741".parse().unwrap());
        assert!(!policy.permits(&h));
        h.remove("origin");
        h.insert("sec-fetch-site", "cross-site".parse().unwrap());
        assert!(!policy.permits(&h));
        h.remove("sec-fetch-site");
        h.insert("host", "evil.example:18741".parse().unwrap());
        assert!(!policy.permits(&h));
        for addr in ["0.0.0.0:123", "127.0.0.2:123", "127.0.0.1:0"] {
            assert!(OriginPolicy::new(addr.parse().unwrap()).is_err());
        }
        let p = OriginPolicy::new("[::1]:80".parse().unwrap()).unwrap();
        h.insert("host", "[::1]".parse().unwrap());
        h.insert("origin", "http://[::1]".parse().unwrap());
        assert!(p.permits(&h));
    }
    #[test]
    fn public_bundle_is_allowlisted_pinned_bounded_and_snapshotted() {
        let dir = Scratch::new();
        let registry = Scratch::new();
        fs::create_dir(dir.0.join("assets")).unwrap();
        dir.put("index.html", b"<html>public</html>");
        dir.put("assets/app.js", b"export {};");
        dir.put("operator.json", b"secret");
        let manifest = serde_json::json!({"bundle_version":1,"files":[{"path":"index.html","sha256":sha256(b"<html>public</html>")},{"path":"assets/app.js","sha256":sha256(b"export {};")}]});
        let bytes = serde_json::to_vec(&manifest).unwrap();
        dir.put("client-assets.json", &bytes);
        let loaded = Delivery::load_bundle(&dir.0, &sha256(&bytes), &registry.0).unwrap();
        for path in SPA_PATHS {
            assert!(loaded.response(path, &Method::GET).is_some());
        }
        for path in [
            "/operator.json",
            "/client-assets.json",
            "/assets/app.js.map",
            "/../operator.json",
            "/%2e%2e/operator.json",
            "/unknown",
        ] {
            assert!(loaded.response(path, &Method::GET).is_none());
        }
        assert_eq!(
            loaded
                .response("/assets/app.js", &Method::HEAD)
                .unwrap()
                .headers()[header::CONTENT_TYPE],
            "text/javascript; charset=utf-8"
        );
        dir.put("index.html", b"changed");
        assert!(Delivery::load_bundle(&dir.0, &sha256(&bytes), &registry.0).is_err());
        assert_eq!(&*loaded.assets["/"].bytes, b"<html>public</html>");
        dir.put("index.html", b"<html>public</html>");
        fs::remove_file(dir.0.join("assets/app.js")).unwrap();
        std::os::unix::fs::symlink(dir.0.join("operator.json"), dir.0.join("assets/app.js"))
            .unwrap();
        assert!(Delivery::load_bundle(&dir.0, &sha256(&bytes), &registry.0).is_err());
        assert!(Delivery::load_bundle(&dir.0, &sha256(&bytes), &dir.0).is_err());
        assert!(pinned(&dir.0.join("operator.json"), &sha256(b"secret"), 2).is_err());
        for path in [
            "assets/../../secret.js",
            "assets/file.json",
            "assets/file.db",
            "assets/.env.js",
            "assets/file.js.map",
            "assets/file.svg",
            "catalog.json",
            "bootstrap.json",
        ] {
            assert!(media(path).is_none());
        }
    }
    #[test]
    fn rejects_fifo_before_open_without_blocking_startup() {
        let dir = Scratch::new();
        let fifo = dir.0.join("fifo.json");
        assert!(
            std::process::Command::new("mkfifo")
                .arg(&fifo)
                .status()
                .unwrap()
                .success()
        );
        let (send, receive) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = send.send(read(&fifo, 4096));
        });
        assert!(
            receive
                .recv_timeout(std::time::Duration::from_secs(2))
                .expect("nonregular public input must not block")
                .is_err()
        );
    }
    #[test]
    fn presentation_pins_producer_schema_catalog_and_source_and_excludes_other_files() {
        let catalog: Catalog = strict_read(include_str!(
            "../../game-engine/tests/fixtures/expected-match-ir.json"
        ))
        .unwrap();
        let dir = Scratch::new();
        let mut delivery = Delivery::default();
        let cards: Vec<Value> = catalog.definitions.iter().map(|d| { let source:Value=serde_json::from_str(&d.provenance.source_json).unwrap(); serde_json::json!({"oracle_id":d.oracle_id,"printing_id":d.provenance.printing_id,"source_sha256":d.provenance.source_sha256,"ability_mapping":"unavailable","faces":[{"face_index":0,"name":source["name"],"oracle_text":null,"mana_cost":null,"type_line":null,"image_url":null}]}) }).collect();
        let mut artifact = serde_json::json!({"presentation_version":1,"catalog_sha256":mtg_contracts::digest(&catalog).unwrap(),"cards":cards});
        let write = |artifact: &Value, producer: &str| {
            let bytes = serde_json::to_vec(artifact).unwrap();
            dir.put("presentation.json", &bytes);
            dir.put("presentation.schema.json", PRESENTATION_SCHEMA.as_bytes());
            let manifest = serde_json::json!({"artifact_version":1,"producer_sha256":producer,"catalog_file_sha256":"a".repeat(64),"catalog_sha256":mtg_contracts::digest(&catalog).unwrap(),"protocol_schema_sha256":"b".repeat(64),"files":[{"path":"presentation.json","sha256":sha256(&bytes)},{"path":"presentation.schema.json","sha256":sha256(PRESENTATION_SCHEMA.as_bytes())},{"path":"request-inventory.json","sha256":"c".repeat(64)}]});
            let bytes = serde_json::to_vec(&manifest).unwrap();
            dir.put("artifacts.json", &bytes);
            sha256(&bytes)
        };
        let digest = write(&artifact, env!("MTG_PRESENTATION_PRODUCER_SHA256"));
        delivery
            .add_presentation(&dir.0, &digest, &catalog)
            .unwrap();
        let descriptor = delivery
            .descriptor(&mtg_contracts::digest(&catalog).unwrap())
            .unwrap();
        assert_eq!(
            delivery
                .response(&descriptor.path, &Method::GET)
                .unwrap()
                .headers()[header::CACHE_CONTROL],
            "public, max-age=31536000, immutable"
        );
        assert!(delivery.response("/artifacts.json", &Method::GET).is_none());
        let digest = write(&artifact, &"0".repeat(64));
        assert!(
            Delivery::default()
                .add_presentation(&dir.0, &digest, &catalog)
                .is_err()
        );
        artifact["cards"][0]["source_sha256"] = "0".repeat(64).into();
        let digest = write(&artifact, env!("MTG_PRESENTATION_PRODUCER_SHA256"));
        assert!(
            Delivery::default()
                .add_presentation(&dir.0, &digest, &catalog)
                .is_err()
        );
        artifact["private_seed"] = 1.into();
        let digest = write(&artifact, env!("MTG_PRESENTATION_PRODUCER_SHA256"));
        assert!(
            Delivery::default()
                .add_presentation(&dir.0, &digest, &catalog)
                .is_err()
        );
    }
}
