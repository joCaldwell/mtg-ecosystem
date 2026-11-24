//! Pure gap inspection and finite four-seat headless admission.
//! CR 702.124 and 903.3–.5: https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::capacities::STACK_LIMIT;
use super::match_admission::{commander_eligible, deck_color_violations, physical_match_rosters};
use super::*;
use crate::profile_data::parse_commander_bans;
use mtg_contracts::bulk_catalog::{
    BulkArtifact, BulkCardFacts, BulkIndexRow, BulkStatus, CommanderLegality, ValidatedCatalog,
    is_oracle_id,
};
use std::sync::LazyLock;

pub const DYNAMIC_PROFILE: &str = "commander-dynamic-headless-v1";
// v4 admits every definition whose cells execute (the `Executes` tier):
// no hand-listed capability set gates headless pods. A runtime fault is a
// pod's `RulesFault`; analytics, search and hosting require higher tiers.
const ADMISSION_DOMAIN: &str = "commander-dynamic-pair-admission-v4";
/// The tier a definition earns to play in a dynamic headless pod.
pub const HEADLESS_TIER: mtg_contracts::kir::ExecutionTier =
    mtg_contracts::kir::ExecutionTier::Executes;
pub const DYNAMIC_MAX_DECK_ROWS: usize = 100;
const DECK_SIZE: u64 = 100;
const PHYSICAL_SOURCES: usize = 400;
const POLICY_SHA256: &str = "71b1aa15189c83820fe654933e5ad28d79897b7e46c3f71155a16cd3430424da";
const POLICY: &str = include_str!("../../../../tests/fixtures/commander-banned-2026-10-15.json");

struct DynamicPolicy {
    banned_ids: BTreeSet<String>,
    digest: String,
}
static POLICY_STATE: LazyLock<Result<DynamicPolicy, String>> = LazyLock::new(|| {
    if sha256(POLICY.as_bytes()) != POLICY_SHA256 {
        return Err("dynamic Commander ban policy pin mismatch".into());
    }
    let banned_ids = parse_commander_bans(POLICY)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|ban| ban.oracle_id)
        .collect();
    let digest = digest(&(ADMISSION_DOMAIN, POLICY_SHA256, HEADLESS_TIER))?;
    Ok(DynamicPolicy { banned_ids, digest })
});
fn policy() -> Result<&'static DynamicPolicy, String> {
    POLICY_STATE.as_ref().map_err(Clone::clone)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeckRole {
    Main,
    Commander,
    Companion,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DynamicDeckEntry {
    pub oracle_id: String,
    pub quantity: u32,
    pub role: DeckRole,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DynamicMatchConfig {
    pub decks: [Vec<DynamicDeckEntry>; 4],
    pub starting_player: PlayerId,
    pub seed: [u8; 32],
}
impl DynamicMatchConfig {
    pub fn from_match_config(config: &MatchConfig) -> Self {
        Self {
            decks: config.decks.each_ref().map(|deck| {
                deck.cards
                    .iter()
                    .map(|row| DynamicDeckEntry {
                        oracle_id: row.oracle_id.clone(),
                        quantity: row.count,
                        role: if row.oracle_id == deck.commander {
                            DeckRole::Commander
                        } else {
                            DeckRole::Main
                        },
                    })
                    .collect()
            }),
            starting_player: config.starting_player,
            seed: config.seed,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeckGapCode {
    TooManyRows,
    InvalidDeckSize,
    UnsupportedCommanderConfiguration,
    InvalidCommanderPair,
    UnsupportedCompanion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardGapCode {
    InvalidOracleId,
    InvalidQuantity,
    DuplicateOracleId,
    ArchiveMissing,
    ArchiveExcluded,
    Unparsed,
    CompilerUnsupported,
    CompilerInvalid,
    NotCommanderLegal,
    Banned,
    FinitePoolExclusion,
    UnsupportedCommanderType,
    SingletonName,
    UnsupportedRepeatQuantity,
    ColorIdentity,
    BasicLandColor,
    CopyCapacityUnreviewed,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct CardGapIssue {
    pub code: CardGapCode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason_code: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<Capability>,
}
impl CardGapIssue {
    fn new(code: CardGapCode) -> Self {
        Self {
            code,
            reason_code: None,
            capabilities: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CardGaps {
    pub oracle_id: String,
    pub role: DeckRole,
    pub quantity: u64,
    pub issues: Vec<CardGapIssue>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommanderPairing {
    Partner,
    DoctorsCompanion,
}
fn pairing(first: &BulkCardFacts, second: &BulkCardFacts) -> Option<CommanderPairing> {
    if !first.legendary_creature || !second.legendary_creature {
        return None;
    }
    if first.partner && second.partner {
        return Some(CommanderPairing::Partner);
    }
    if (first.doctors_companion && second.time_lord_doctor)
        || (second.doctors_companion && first.time_lord_doctor)
    {
        return Some(CommanderPairing::DoctorsCompanion);
    }
    None
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DeckInspection {
    pub total_slots: u64,
    pub compiled_slots: u64,
    pub deck_issues: Vec<DeckGapCode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commander_pairing: Option<CommanderPairing>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commander_color_identity: Option<Vec<Color>>,
    pub card_gaps: Vec<CardGaps>,
}
impl DeckInspection {
    pub fn admissible(&self) -> bool {
        self.deck_issues.is_empty() && self.card_gaps.is_empty()
    }
}

pub fn inspect_dynamic_deck(
    artifact: &BulkArtifact,
    rows: &[DynamicDeckEntry],
) -> Result<DeckInspection, String> {
    Ok(inspect(artifact, rows, policy()?))
}

fn inspect(
    artifact: &BulkArtifact,
    rows: &[DynamicDeckEntry],
    policy: &DynamicPolicy,
) -> DeckInspection {
    let mut report = DeckInspection {
        total_slots: 0,
        compiled_slots: 0,
        deck_issues: Vec::new(),
        commander_pairing: None,
        commander_color_identity: None,
        card_gaps: Vec::new(),
    };
    if rows.len() > DYNAMIC_MAX_DECK_ROWS {
        report.deck_issues.push(DeckGapCode::TooManyRows);
        return report;
    }

    let catalog = artifact.catalog().catalog();
    let mut identities = BTreeMap::<&str, usize>::new();
    let mut groups = BTreeMap::<(&str, DeckRole), (u64, bool)>::new();
    let mut names = BTreeMap::<&str, BTreeSet<&str>>::new();
    for row in rows {
        let id = row.oracle_id.as_str();
        *identities.entry(id).or_default() += 1;
        let (quantity, zero) = groups.entry((id, row.role)).or_default();
        *quantity += u64::from(row.quantity);
        *zero |= row.quantity == 0;
        if row.role != DeckRole::Companion {
            report.total_slots += u64::from(row.quantity);
        }
        if let Some(d) = catalog.definition_by_oracle_id(id)
            && !d.characteristics.basic
        {
            names.entry(&d.name).or_default().insert(id);
        }
    }

    if report.total_slots != DECK_SIZE {
        report.deck_issues.push(DeckGapCode::InvalidDeckSize);
    }
    let commanders: Vec<_> = rows
        .iter()
        .filter(|r| r.role == DeckRole::Commander)
        .collect();
    if !(1..=2).contains(&commanders.len())
        || commanders
            .iter()
            .any(|row| row.quantity != 1 || identities[row.oracle_id.as_str()] != 1)
    {
        report
            .deck_issues
            .push(DeckGapCode::UnsupportedCommanderConfiguration);
    } else if let Some(records) = commanders
        .iter()
        .map(|row| artifact.lookup(&row.oracle_id))
        .collect::<Option<Vec<_>>>()
    {
        let allowed_pair = if let [first, second] = records.as_slice() {
            report.commander_pairing = pairing(&first.facts, &second.facts);
            if report.commander_pairing.is_none()
                && first.facts.legendary_creature
                && second.facts.legendary_creature
            {
                report.deck_issues.push(DeckGapCode::InvalidCommanderPair);
            }
            report.commander_pairing.is_some()
        } else {
            true
        };
        if allowed_pair
            && records.iter().all(|row| {
                row.facts.legendary_creature
                    && !policy.banned_ids.contains(&row.oracle_id)
                    && row.facts.commander_legality == CommanderLegality::Legal
                    && !row.facts.conspiracy
                    && !row.facts.ante
            })
        {
            report.commander_color_identity = Some(
                records
                    .iter()
                    .flat_map(|row| row.facts.color_identity.iter().copied())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect(),
            );
        }
    }
    if rows.iter().any(|r| r.role == DeckRole::Companion) {
        report.deck_issues.push(DeckGapCode::UnsupportedCompanion);
    }

    for ((id, role), (quantity, includes_zero)) in groups {
        let mut issues = Vec::new();
        if includes_zero || quantity > DECK_SIZE {
            issues.push(CardGapIssue::new(CardGapCode::InvalidQuantity));
        }
        if identities[id] != 1 {
            issues.push(CardGapIssue::new(CardGapCode::DuplicateOracleId));
        }
        if policy.banned_ids.contains(id) {
            issues.push(CardGapIssue::new(CardGapCode::Banned));
        }
        if !is_oracle_id(id) {
            issues.push(CardGapIssue::new(CardGapCode::InvalidOracleId));
        } else if let Some(row) = artifact.lookup(id) {
            inspect_index_row(row, role, &mut issues);
        } else {
            issues.push(CardGapIssue::new(CardGapCode::ArchiveMissing));
        }
        if let Some(definition) = catalog.definition_by_oracle_id(id) {
            if role != DeckRole::Companion {
                report.compiled_slots += quantity;
            }
            inspect_definition(
                definition,
                report.commander_color_identity.as_deref(),
                role,
                quantity,
                &names,
                &mut issues,
            );
        }
        if !issues.is_empty() {
            issues.sort_unstable();
            issues.dedup();
            report.card_gaps.push(CardGaps {
                // Preserve identity for valid IDs and bound invalid input strings in diagnostics.
                oracle_id: id.chars().take(36).collect(),
                role,
                quantity,
                issues,
            });
        }
    }
    report
}

fn inspect_index_row(row: &BulkIndexRow, role: DeckRole, issues: &mut Vec<CardGapIssue>) {
    let code = match row.status {
        BulkStatus::LoweredValidated => None,
        BulkStatus::Unsupported => Some(CardGapCode::CompilerUnsupported),
        BulkStatus::Invalid => Some(CardGapCode::CompilerInvalid),
        BulkStatus::Unparsed => Some(
            if row
                .reason_code
                .as_deref()
                .is_some_and(|s| s.starts_with("archive_excluded:"))
            {
                CardGapCode::ArchiveExcluded
            } else {
                CardGapCode::Unparsed
            },
        ),
    };
    if let Some(code) = code {
        issues.push(CardGapIssue {
            code,
            reason_code: row.reason_code.clone(),
            capabilities: Vec::new(),
        });
    }
    match row.facts.commander_legality {
        CommanderLegality::Legal => {}
        CommanderLegality::NotLegal => {
            issues.push(CardGapIssue::new(CardGapCode::NotCommanderLegal))
        }
        CommanderLegality::Banned => issues.push(CardGapIssue::new(CardGapCode::Banned)),
    }
    if row.facts.ante || row.facts.conspiracy {
        issues.push(CardGapIssue::new(CardGapCode::FinitePoolExclusion));
    }
    if role == DeckRole::Commander && !row.facts.legendary_creature {
        issues.push(CardGapIssue::new(CardGapCode::UnsupportedCommanderType));
    }
}

fn inspect_definition(
    d: &Definition,
    commander_colors: Option<&[Color]>,
    role: DeckRole,
    quantity: u64,
    names: &BTreeMap<&str, BTreeSet<&str>>,
    issues: &mut Vec<CardGapIssue>,
) {
    if role == DeckRole::Commander
        && !commander_eligible(&d.characteristics, CommanderEligibility::LegendaryCreature)
    {
        issues.push(CardGapIssue::new(CardGapCode::UnsupportedCommanderType));
    }
    if !d.characteristics.basic {
        if quantity > 1 {
            issues.push(CardGapIssue::new(CardGapCode::UnsupportedRepeatQuantity));
        }
        if names.get(d.name.as_str()).is_some_and(|ids| ids.len() > 1) {
            issues.push(CardGapIssue::new(CardGapCode::SingletonName));
        }
    }
    if let Some(colors) = commander_colors {
        let (identity, land) = deck_color_violations(&d.characteristics, colors);
        if identity {
            issues.push(CardGapIssue::new(CardGapCode::ColorIdentity));
        }
        if land {
            issues.push(CardGapIssue::new(CardGapCode::BasicLandColor));
        }
    }
    // Every validated definition executes (CR 113.3: each ability is a cell
    // with an executor), so the headless tier adds no card gap of its own.
    let copies: Vec<_> = d
        .requirements()
        .into_iter()
        .filter(|c| copy_capacity_unreviewed(*c))
        .collect();
    if !copies.is_empty() {
        issues.push(CardGapIssue {
            code: CardGapCode::CopyCapacityUnreviewed,
            reason_code: None,
            capabilities: copies,
        });
    }
}

fn copy_capacity_unreviewed(capability: Capability) -> bool {
    matches!(
        capability,
        Capability::EnterAsCopyOfPermanentEnteredThisTurnV1
            | Capability::ControlledCreatureSpellCopyV1
    )
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DynamicAdmissionIdentity {
    pub bulk_manifest_sha256: String,
    pub policy_digest: String,
    pub catalog_digest: String,
    pub admission_digest: String,
}

pub struct DynamicPod {
    pub prepared: Prepared,
    pub identity: DynamicAdmissionIdentity,
    normalized: DynamicMatchConfig,
}
impl DynamicPod {
    pub fn normalized_config(&self) -> &DynamicMatchConfig {
        &self.normalized
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum DynamicAdmissionError {
    InvalidStartingPlayer,
    InvalidSeatRotation,
    DeckGaps { decks: Vec<DeckInspection> },
    PodCapacity,
    Unavailable { detail: String },
}
impl std::fmt::Display for DynamicAdmissionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

fn unavailable(detail: String) -> DynamicAdmissionError {
    DynamicAdmissionError::Unavailable { detail }
}

pub fn admit_dynamic_pod(
    artifact: &BulkArtifact,
    config: &DynamicMatchConfig,
) -> Result<DynamicPod, DynamicAdmissionError> {
    if config.starting_player.0 >= 4 {
        return Err(DynamicAdmissionError::InvalidStartingPlayer);
    }
    DynamicPodTemplate::admit(artifact, &config.decks)?.prepare(
        config.starting_player,
        config.seed,
        0,
    )
}

struct NormalizedDeck {
    physical: Vec<String>,
    commanders: Vec<String>,
    cards: Vec<DeckEntry>,
}
impl NormalizedDeck {
    fn new(rows: &[DynamicDeckEntry]) -> Self {
        let mut commanders = Vec::with_capacity(2);
        let mut cards = Vec::with_capacity(rows.len());
        for row in rows {
            if row.role == DeckRole::Commander {
                commanders.push(row.oracle_id.clone());
            }
            cards.push(DeckEntry {
                oracle_id: row.oracle_id.clone(),
                count: row.quantity,
            });
        }
        commanders.sort_unstable();
        cards.sort_unstable_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
        let physical = super::match_admission::physical_order(&commanders, &cards);
        Self {
            commanders,
            cards,
            physical,
        }
    }

    fn dynamic_rows(&self) -> Vec<DynamicDeckEntry> {
        self.cards
            .iter()
            .map(|row| DynamicDeckEntry {
                oracle_id: row.oracle_id.clone(),
                quantity: row.count,
                role: if self.commanders.contains(&row.oracle_id) {
                    DeckRole::Commander
                } else {
                    DeckRole::Main
                },
            })
            .collect()
    }
}

/// Validated immutable rosters and their shared executable projection. Rotation
/// only changes seat assignment, so it cannot change the vetted capability
/// set, physical multiplicities or trigger capacity proof.
pub struct DynamicPodTemplate<'a> {
    artifact: &'a BulkArtifact,
    decks: [NormalizedDeck; 4],
    projection: ValidatedCatalog,
    bulk_manifest_sha256: String,
    policy_digest: String,
}

impl<'a> DynamicPodTemplate<'a> {
    pub fn admit(
        artifact: &'a BulkArtifact,
        decks: &[Vec<DynamicDeckEntry>; 4],
    ) -> Result<Self, DynamicAdmissionError> {
        let policy = policy().map_err(unavailable)?;
        let inspections = std::array::from_fn::<_, 4, _>(|i| inspect(artifact, &decks[i], policy));
        if inspections.iter().any(|r| !r.admissible()) {
            return Err(DynamicAdmissionError::DeckGaps {
                decks: Vec::from(inspections),
            });
        }
        Self::project(artifact, decks, policy)
    }

    /// Normalize inspected decks and build their shared projection and
    /// capacity proof. Row order never reaches the template.
    fn project(
        artifact: &'a BulkArtifact,
        decks: &[Vec<DynamicDeckEntry>; 4],
        policy: &DynamicPolicy,
    ) -> Result<Self, DynamicAdmissionError> {
        let catalog = artifact.catalog().catalog();
        let decks = decks.each_ref().map(|rows| NormalizedDeck::new(rows));
        let mut selected = BTreeMap::<&str, &Definition>::new();
        let mut native_triggers = 0usize;
        for row in decks.iter().flat_map(|deck| &deck.cards) {
            let definition = catalog
                .definition_by_oracle_id(&row.oracle_id)
                .ok_or_else(|| unavailable("validated deck definition vanished".into()))?;
            selected.insert(&row.oracle_id, definition);
            native_triggers = (row.count as usize)
                .checked_mul(definition.triggered.len())
                .and_then(|count| native_triggers.checked_add(count))
                .ok_or(DynamicAdmissionError::PodCapacity)?;
        }
        let projection = Catalog {
            schema_version: catalog.schema_version,
            rules_edition: catalog.rules_edition.clone(),
            rules_sha256: catalog.rules_sha256.clone(),
            definitions: selected.into_values().cloned().collect(),
        };
        if !capacity_supported(&projection, native_triggers) {
            return Err(DynamicAdmissionError::PodCapacity);
        }
        Ok(Self {
            decks,
            projection: ValidatedCatalog::new(projection).map_err(unavailable)?,
            artifact,
            bulk_manifest_sha256: artifact.manifest_sha256().into(),
            policy_digest: policy.digest.clone(),
        })
    }

    /// Replace one physical copy, preserving the role of that copy. Recheck
    /// legality, projection and capacity against the authenticated bulk input.
    pub fn substitute(
        &self,
        deck_slot: usize,
        remove: &str,
        add: &str,
    ) -> Result<Self, DynamicAdmissionError> {
        self.substitute_many(deck_slot, &[(remove, add)])
    }

    /// All removals name distinct copies of the original roster. Validate the
    /// resulting deck once, including when swaps exchange commander roles.
    pub fn substitute_many(
        &self,
        deck_slot: usize,
        swaps: &[(&str, &str)],
    ) -> Result<Self, DynamicAdmissionError> {
        let deck = self
            .decks
            .get(deck_slot)
            .ok_or_else(|| unavailable("invalid deck slot".into()))?;
        let mut replacements = BTreeMap::new();
        for &(remove, add) in swaps {
            let offset = deck
                .physical
                .iter()
                .enumerate()
                .find(|(i, id)| id.as_str() == remove && !replacements.contains_key(i))
                .map(|(i, _)| i)
                .ok_or_else(|| unavailable(format!("removed copy of {remove} absent")))?;
            replacements.insert(offset, add);
        }
        let mut physical = deck.physical.clone();
        let mut counts = BTreeMap::new();
        for (offset, id) in physical.iter_mut().enumerate() {
            let role = if deck.commanders.contains(id) {
                DeckRole::Commander
            } else {
                DeckRole::Main
            };
            if let Some(&add) = replacements.get(&offset) {
                *id = add.to_owned();
            }
            *counts.entry((id.clone(), role)).or_insert(0) += 1;
        }
        let mut rows = self.rows();
        rows[deck_slot] = counts
            .into_iter()
            .map(|((oracle_id, role), quantity)| DynamicDeckEntry {
                oracle_id,
                role,
                quantity,
            })
            .collect();
        let mut next = Self::admit(self.artifact, &rows)?;
        for (target, original) in next.decks.iter_mut().zip(&self.decks) {
            target.physical.clone_from(&original.physical);
        }
        next.decks[deck_slot].physical = physical;
        Ok(next)
    }

    pub fn rows(&self) -> [Vec<DynamicDeckEntry>; 4] {
        self.decks.each_ref().map(NormalizedDeck::dynamic_rows)
    }
    pub fn catalog(&self) -> &ValidatedCatalog {
        &self.projection
    }
    pub fn policy_digest(&self) -> &str {
        &self.policy_digest
    }
    pub fn prepare(
        &self,
        starting_player: PlayerId,
        seed: [u8; 32],
        rotation: usize,
    ) -> Result<DynamicPod, DynamicAdmissionError> {
        if starting_player.0 >= 4 {
            return Err(DynamicAdmissionError::InvalidStartingPlayer);
        }
        if rotation >= 4 {
            return Err(DynamicAdmissionError::InvalidSeatRotation);
        }
        let mut decks = self.decks.each_ref();
        decks.rotate_right(rotation);
        let normalized = DynamicMatchConfig {
            decks: decks.map(NormalizedDeck::dynamic_rows),
            starting_player,
            seed,
        };
        let admission_digest = digest(&(
            ADMISSION_DOMAIN,
            &self.bulk_manifest_sha256,
            &self.policy_digest,
            self.projection.digest(),
            &normalized,
        ))
        .map_err(unavailable)?;
        // Ordinary rosters retain their admission identity. A noncanonical
        // physical order also binds the permutation used by replay.
        let admission_digest = if decks.iter().all(|deck| {
            deck.physical.starts_with(&deck.commanders)
                && deck.physical[deck.commanders.len()..].is_sorted()
        }) {
            admission_digest
        } else {
            digest(&(
                "physical-slots-v1",
                admission_digest,
                decks.map(|deck| &deck.physical),
            ))
            .map_err(unavailable)?
        };
        let identity = DynamicAdmissionIdentity {
            bulk_manifest_sha256: self.bulk_manifest_sha256.clone(),
            policy_digest: self.policy_digest.clone(),
            catalog_digest: self.projection.digest().into(),
            admission_digest,
        };
        // Match initialization uses the private legacy carrier for seed and
        // starting seat. The role-tagged config above is the replay authority.
        let match_config = MatchConfig {
            profile: DYNAMIC_PROFILE.into(),
            decks: decks.map(|deck| Decklist {
                commander: deck.commanders[0].clone(),
                cards: deck.cards.clone(),
            }),
            starting_player,
            seed,
        };
        let prepared = Prepared {
            config: physical_match_rosters(
                DYNAMIC_PROFILE,
                decks.map(|deck| (deck.commanders.as_slice(), deck.physical.as_slice())),
            ),
            match_config: Some(match_config),
            catalog: self.projection.shared_catalog(),
            catalog_digest: identity.catalog_digest.clone(),
            profile_digest: identity.admission_digest.clone(),
        };
        Ok(DynamicPod {
            prepared,
            identity,
            normalized,
        })
    }
}

fn capacity_supported(catalog: &Catalog, native_triggers: usize) -> bool {
    catalog
        .cast_trigger_fanout_bound(PHYSICAL_SOURCES)
        .is_some_and(|bound| bound < STACK_LIMIT / 2)
        && (PHYSICAL_SOURCES + TOKEN_CAPACITY as usize)
            .checked_mul(native_triggers)
            .is_some_and(|wave| wave < STACK_LIMIT / 2)
}

impl std::error::Error for DynamicAdmissionError {}

#[cfg(test)]
mod tests;
