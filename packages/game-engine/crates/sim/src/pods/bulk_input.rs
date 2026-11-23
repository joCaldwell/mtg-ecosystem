//! Offline artifact loading and batch deck inspection.
use mtg_contracts::{
    JsonByteProof,
    bulk_catalog::{BulkArtifact, BulkManifest, MANIFEST_MAX_BYTES, is_sha256},
    canonical, strict_read_seekable_verified,
};
use mtg_core::BuildIdentity;
use mtg_core::{DeckInspection, DeckRole, DynamicDeckEntry, inspect_dynamic_deck};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    fs::{self, File},
    path::Path,
    sync::Arc,
};

const SNAPSHOT_MAX: u64 = 1024 * 1024;
const DECK_MAX: usize = 64 * 1024;

fn read_file<T: DeserializeOwned>(path: &Path, limit: u64) -> Result<(T, JsonByteProof), String> {
    let metadata = fs::symlink_metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if !metadata.is_file() || metadata.len() > limit {
        return Err(format!(
            "invalid regular input file or byte limit: {}",
            path.display()
        ));
    }
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    strict_read_seekable_verified(&mut file, limit).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn load(build: BuildIdentity, directory: &Path, pin: &str) -> Result<BulkArtifact, String> {
    if !is_sha256(pin) || !fs::symlink_metadata(directory).is_ok_and(|m| m.is_dir()) {
        return Err(
            "expected a regular artifact directory and a lowercase manifest SHA-256".into(),
        );
    }
    let manifest: (BulkManifest, _) =
        read_file(&directory.join("manifest.json"), MANIFEST_MAX_BYTES)?;
    if manifest.1.sha256 != pin {
        return Err("bulk manifest pin mismatch".into());
    }
    manifest.0.validate(build.id)?;
    let catalog = read_file(&directory.join("catalog.json"), manifest.0.catalog.bytes)?;
    let index = read_file(&directory.join("index.json"), manifest.0.index.bytes)?;
    BulkArtifact::verify(manifest, pin, build.id, catalog, index)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PodDecks {
    decks: [Vec<DynamicDeckEntry>; 4],
}

pub fn read_pod_decks(path: &Path) -> Result<[Vec<DynamicDeckEntry>; 4], String> {
    let (pod, _): (PodDecks, _) = read_file(path, 4 * DECK_MAX as u64)?;
    Ok(pod.decks)
}

// Accept the existing frozen production snapshot shape. Only exact identity,
// quantity and role participate in admission. Display metadata never supplies facts.
#[derive(Deserialize)]
struct DeckSnapshot {
    #[serde(rename = "deckCount")]
    deck_count: usize,
    decks: Vec<SnapshotDeck>,
}
#[derive(Deserialize)]
struct SnapshotDeck {
    id: u64,
    name: String,
    cards: Vec<SnapshotCard>,
}
#[derive(Deserialize)]
struct SnapshotCard {
    oracle_id: String,
    quantity: u32,
    is_commander: bool,
}
#[derive(Serialize)]
pub struct InspectionRow {
    id: u64,
    name: String,
    status: &'static str,
    #[serde(flatten)]
    inspection: DeckInspection,
}
#[derive(Serialize)]
pub struct InspectionBatch {
    format_version: u32,
    bulk_manifest_sha256: String,
    catalog_digest: String,
    pub decks: Vec<InspectionRow>,
}
pub fn inspect_snapshot(artifact: &BulkArtifact, path: &Path) -> Result<InspectionBatch, String> {
    let (mut snapshot, _): (DeckSnapshot, _) = read_file(path, SNAPSHOT_MAX)?;
    snapshot.decks.sort_unstable_by_key(|d| d.id);
    if snapshot.decks.len() != snapshot.deck_count
        || !(1..=64).contains(&snapshot.deck_count)
        || snapshot.decks.windows(2).any(|r| r[0].id == r[1].id)
    {
        return Err(
            "deck snapshot count is inconsistent, exceeds the bound, or repeats an ID".into(),
        );
    }
    let cached = artifact.catalog().shared_catalog();
    let mut decks = Vec::with_capacity(snapshot.decks.len());
    for deck in snapshot.decks {
        let rows = deck
            .cards
            .into_iter()
            .map(|card| DynamicDeckEntry {
                oracle_id: card.oracle_id,
                quantity: card.quantity,
                role: if card.is_commander {
                    DeckRole::Commander
                } else {
                    DeckRole::Main
                },
            })
            .collect::<Vec<_>>();
        if deck.name.len() > 256 || canonical(&rows)?.len() > DECK_MAX {
            return Err(format!(
                "deck inspection request exceeds its byte limit for {}",
                deck.id
            ));
        }
        let inspection = inspect_dynamic_deck(artifact, &rows)?;
        let entry = InspectionRow {
            id: deck.id,
            name: deck.name,
            status: if inspection.admissible() {
                "admitted"
            } else {
                "rejected"
            },
            inspection,
        };
        if canonical(&entry)?.len() > DECK_MAX {
            return Err(format!(
                "deck inspection response exceeds its byte limit for {}",
                deck.id
            ));
        }
        decks.push(entry);
    }
    if !Arc::ptr_eq(&cached, &artifact.catalog().shared_catalog()) {
        return Err("inspection did not retain the loaded catalog identity".into());
    }
    Ok(InspectionBatch {
        format_version: 2,
        bulk_manifest_sha256: artifact.manifest_sha256().into(),
        catalog_digest: artifact.catalog().digest().into(),
        decks,
    })
}
