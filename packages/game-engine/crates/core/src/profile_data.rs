//! Admission data embedded by build.rs. The same typed loader that runtime lazy
//! initialization uses validates every embedded profile in a unit test.
use mtg_contracts::{Capability, Catalog, digest, kir, sha256, strict_read};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
/// Data is discovered and embedded at build time; core performs no filesystem I/O.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommanderPolicy {
    pub color_identity: ColorIdentityPolicy,
    pub eligibility: CommanderEligibility,
    pub banned_list: String,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorIdentityPolicy {
    CommanderSubset,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommanderEligibility {
    LegendaryCreature,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompletionLayout {
    pub rotate: bool,
    pub homogeneous_seats: [usize; 2],
}
testkit_fields! {
    #[derive(Clone, Debug, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct MatchProfile {
        pub name: String,
        pub catalog_manifest: String,
        pub decks: [Decklist; 4],
        pub commanders: Vec<String>,
        pub commander_policy: CommanderPolicy,
        pub hosted: bool,
        pub search: bool,
        pub completion: CompletionLayout,
        #[serde(skip)]
        pub catalog_sha256: String,
        #[serde(skip)]
        pub catalog: Option<Catalog>,
        #[serde(skip)]
        pub(crate) banned_list_raw: String,
        #[serde(skip)]
        pub(crate) bans: Vec<BanRecord>,
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeckEntry {
    pub oracle_id: String,
    pub count: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Decklist {
    /// This physical designation is included in the hundred cards below.
    pub commander: String,
    pub cards: Vec<DeckEntry>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Feature {
    Search,
    Hosting,
}
impl Feature {
    /// The tier a definition earns to be admitted to this feature.
    pub fn tier(self) -> kir::ExecutionTier {
        match self {
            Self::Search => kir::ExecutionTier::Search,
            Self::Hosting => kir::ExecutionTier::Hosting,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum ProfileError {
    InvalidProfile {
        detail: String,
    },
    FeatureNotRequested {
        feature: Feature,
    },
    UnreviewedCapabilities {
        feature: Feature,
        capabilities: BTreeSet<Capability>,
    },
    /// Every surface earned the tier but some kernel cell did not.
    UnqualifiedCells {
        feature: Feature,
        cells: BTreeSet<kir::Cell>,
    },
}
impl std::fmt::Display for ProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for ProfileError {}
impl From<String> for ProfileError {
    fn from(detail: String) -> Self {
        Self::InvalidProfile { detail }
    }
}
#[derive(Clone, Debug, Deserialize)]
pub struct BanRecord {
    pub name: String,
    pub oracle_id: String,
    pub record_json: String,
    pub record_sha256: String,
    pub source_sha256: String,
}
#[derive(Deserialize)]
struct OffensivePolicy {
    records: Vec<BanRecord>,
}
#[derive(Deserialize)]
struct BannedList {
    retrieved_date: String,
    url: String,
    html_sha256: String,
    named_bans: Vec<BanRecord>,
    offensive_policy: OffensivePolicy,
}
pub(crate) fn parse_commander_bans(banned_raw: &str) -> Result<Vec<BanRecord>, ProfileError> {
    let banned: BannedList = strict_read(banned_raw)?;
    if banned.retrieved_date.is_empty() || banned.url.is_empty() || banned.html_sha256.len() != 64 {
        return Err("invalid banned-list source metadata".to_owned().into());
    }
    let bans: Vec<_> = banned
        .named_bans
        .into_iter()
        .chain(banned.offensive_policy.records)
        .collect();
    let mut ids = BTreeSet::new();
    for ban in &bans {
        #[derive(Deserialize)]
        struct Record {
            source: Box<serde_json::value::RawValue>,
            #[serde(rename = "oracleIdentity")]
            oracle_id: String,
        }
        let record: Record = strict_read(&ban.record_json)?;
        if !ids.insert(&ban.oracle_id)
            || ban.name.is_empty()
            || sha256(ban.record_json.as_bytes()) != ban.record_sha256
            || sha256(record.source.get().as_bytes()) != ban.source_sha256
            || record.oracle_id != ban.oracle_id
        {
            return Err(format!("invalid banned-list source record: {}", ban.oracle_id).into());
        }
    }
    Ok(bans)
}
impl MatchProfile {
    pub fn load(raw: &str, catalog_raw: &str, banned_raw: &str) -> Result<Self, ProfileError> {
        let mut p: Self = strict_read(raw)?;
        let catalog = Catalog::read(catalog_raw)?;
        if p.name.is_empty()
            || p.commanders.is_empty()
            || p.completion
                .homogeneous_seats
                .iter()
                .any(|i| *i >= p.decks.len())
        {
            return Err(
                "empty profile identity/commanders or invalid completion deck index"
                    .to_owned()
                    .into(),
            );
        }
        p.bans = parse_commander_bans(banned_raw)?;
        p.banned_list_raw = banned_raw.to_owned();
        p.catalog_sha256 = digest(&catalog)?;
        p.validate_requested_features(&catalog)?;
        p.catalog = Some(catalog);
        Ok(p)
    }
    pub fn catalog(&self) -> &Catalog {
        self.catalog.as_ref().expect("loaded embedded catalog")
    }
    pub fn policy_digest(&self) -> String {
        sha256(self.banned_list_raw.as_bytes())
    }
    pub fn banned_commander_card(&self, id: &str) -> Option<&str> {
        self.bans
            .iter()
            .find(|b| b.oracle_id == id)
            .map(|b| b.name.as_str())
    }
    /// A hosted or searched profile admits only definitions whose every
    /// surface and kernel cell has earned that feature's tier
    /// (`mtg_contracts::kir::tiers`).
    pub fn require_feature(&self, catalog: &Catalog, feature: Feature) -> Result<(), ProfileError> {
        if !match feature {
            Feature::Search => self.search,
            Feature::Hosting => self.hosted,
        } {
            return Err(ProfileError::FeatureNotRequested { feature });
        }
        let tier = feature.tier();
        let mut capabilities = BTreeSet::new();
        let mut cells = BTreeSet::new();
        for d in &catalog.definitions {
            let (surfaces, kernel) = kir::unqualified(d, tier)
                .map_err(|e| ProfileError::from(format!("{}: {e}", d.name)))?;
            capabilities.extend(surfaces);
            cells.extend(kernel);
        }
        if !capabilities.is_empty() {
            return Err(ProfileError::UnreviewedCapabilities {
                feature,
                capabilities,
            });
        }
        if !cells.is_empty() {
            return Err(ProfileError::UnqualifiedCells { feature, cells });
        }
        Ok(())
    }
    pub fn validate_requested_features(&self, catalog: &Catalog) -> Result<(), ProfileError> {
        if self.search {
            self.require_feature(catalog, Feature::Search)?;
        }
        if self.hosted {
            self.require_feature(catalog, Feature::Hosting)?;
        }
        Ok(())
    }
}
