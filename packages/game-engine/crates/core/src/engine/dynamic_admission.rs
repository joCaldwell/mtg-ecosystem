//! Pure gap inspection and finite four-seat headless admission.
//! CR 903.3 and 903.5: https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::capacities::STACK_LIMIT;
use super::match_admission::{commander_eligible, deck_color_violations, physical_match_config};
use super::*;
use crate::profile_data::parse_commander_bans;
use mtg_contracts::bulk_catalog::{
    BulkArtifact, BulkStatus, CommanderLegality, ValidatedCatalog, is_oracle_id,
};
use std::sync::LazyLock;

pub const DYNAMIC_PROFILE: &str = "commander-dynamic-headless-v1";
pub const DYNAMIC_MAX_DECK_ROWS: usize = 100;
const DECK_SIZE: u64 = 100;
const PHYSICAL_SOURCES: usize = 400;
const POLICY_SHA256: &str = "71b1aa15189c83820fe654933e5ad28d79897b7e46c3f71155a16cd3430424da";
const POLICY: &str = include_str!("../../../../tests/fixtures/commander-banned-2026-10-15.json");

// The exact 21-definition Foundations projection exercises these requirements.
// Expanding this set needs dynamic capacity and runtime qualification.
const REVIEWED: &[Capability] = &[
    Capability::ActivatedAbilityV1,
    Capability::AnyDamageTargetV1,
    Capability::AnyPlayerTargetV1,
    Capability::BaseCreatureV1,
    Capability::BasicLandManaV1,
    Capability::CastCreatureV1,
    Capability::CastInstantV1,
    Capability::CastSorceryV1,
    Capability::CombatDamagePreventionV1,
    Capability::CommanderIdentityV1,
    Capability::ControllerDrawV1,
    Capability::CreatureDiesV1,
    Capability::CreatureTargetV1,
    Capability::DamageBudgetPreventionV1,
    Capability::DefenderV1,
    Capability::InterveningIfV1,
    Capability::LandPlayV1,
    Capability::LifeGainV1,
    Capability::LifeLossV1,
    Capability::ModalChoiceV1,
    Capability::NormalLayoutV1,
    Capability::OpponentTargetV1,
    Capability::OtherCreatureEntersBattlefieldV1,
    Capability::SelfDiesV1,
    Capability::SelfEntersBattlefieldV1,
    Capability::TapSourceCostV1,
    Capability::TargetPlayerSacrificeCreatureV1,
    Capability::TemporaryPowerToughnessV1,
    Capability::TriggeredAbilityV1,
];

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
    let digest = digest(&(DYNAMIC_PROFILE, POLICY_SHA256, REVIEWED))?;
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
    DoctorsCompanion,
    SingletonName,
    UnsupportedRepeatQuantity,
    ColorIdentity,
    BasicLandColor,
    CapabilityUnreviewed,
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DeckInspection {
    pub total_slots: u64,
    pub compiled_slots: u64,
    pub deck_issues: Vec<DeckGapCode>,
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
    let mut commander_rows = rows.iter().filter(|r| r.role == DeckRole::Commander);
    let first_commander = commander_rows.next();
    let multiple_commanders = commander_rows.next().is_some();
    let commander_row = first_commander.filter(|row| row.quantity == 1 && !multiple_commanders);
    if commander_row.is_none() {
        report
            .deck_issues
            .push(DeckGapCode::UnsupportedCommanderConfiguration);
    }
    if rows.iter().any(|r| r.role == DeckRole::Companion) {
        report.deck_issues.push(DeckGapCode::UnsupportedCompanion);
    }
    let commander = commander_row
        .filter(|r| {
            identities[r.oracle_id.as_str()] == 1
                && !policy.banned_ids.contains(&r.oracle_id)
                && artifact.lookup(&r.oracle_id).is_some_and(|row| {
                    row.facts.commander_legality == CommanderLegality::Legal
                        && !row.facts.conspiracy
                        && !row.facts.ante
                })
        })
        .and_then(|r| catalog.definition_by_oracle_id(&r.oracle_id))
        .filter(|definition| {
            commander_eligible(
                &definition.characteristics,
                CommanderEligibility::LegendaryCreature,
            )
        });

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
            inspect_index_row(
                row,
                role == DeckRole::Commander && multiple_commanders,
                &mut issues,
            );
        } else {
            issues.push(CardGapIssue::new(CardGapCode::ArchiveMissing));
        }
        if let Some(definition) = catalog.definition_by_oracle_id(id) {
            if role != DeckRole::Companion {
                report.compiled_slots += quantity;
            }
            inspect_definition(definition, commander, role, quantity, &names, &mut issues);
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

fn inspect_index_row(
    row: &mtg_contracts::bulk_catalog::BulkIndexRow,
    paired_commander: bool,
    issues: &mut Vec<CardGapIssue>,
) {
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
    if paired_commander && row.facts.doctors_companion {
        issues.push(CardGapIssue::new(CardGapCode::DoctorsCompanion));
    }
}

fn inspect_definition(
    d: &Definition,
    commander: Option<&Definition>,
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
    if let Some(commander) = commander {
        let (identity, land) =
            deck_color_violations(&d.characteristics, &commander.characteristics);
        if identity {
            issues.push(CardGapIssue::new(CardGapCode::ColorIdentity));
        }
        if land {
            issues.push(CardGapIssue::new(CardGapCode::BasicLandColor));
        }
    }
    let requirements = d.requirements();
    let unreviewed: Vec<_> = requirements
        .iter()
        .copied()
        .filter(|c| !REVIEWED.contains(c))
        .collect();
    let copies: Vec<_> = requirements
        .iter()
        .copied()
        .filter(|c| copy_capacity_unreviewed(*c))
        .collect();
    for (code, capabilities) in [
        (CardGapCode::CapabilityUnreviewed, unreviewed),
        (CardGapCode::CopyCapacityUnreviewed, copies),
    ] {
        if !capabilities.is_empty() {
            issues.push(CardGapIssue {
                code,
                reason_code: None,
                capabilities,
            });
        }
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
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum DynamicAdmissionError {
    InvalidStartingPlayer,
    DeckGaps { decks: Vec<DeckInspection> },
    PodCapacity,
    Unavailable { detail: String },
}
impl std::fmt::Display for DynamicAdmissionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for DynamicAdmissionError {}

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
    let policy = policy().map_err(unavailable)?;
    let inspections = config
        .decks
        .each_ref()
        .map(|rows| inspect(artifact, rows, policy));
    if inspections.iter().any(|r| !r.admissible()) {
        return Err(DynamicAdmissionError::DeckGaps {
            decks: Vec::from(inspections),
        });
    }
    let decks = config.decks.each_ref().map(|rows| {
        let mut cards = Vec::with_capacity(rows.len());
        let mut commander = String::new();
        for row in rows {
            if row.role == DeckRole::Commander {
                commander.clone_from(&row.oracle_id);
            }
            cards.push(DeckEntry {
                oracle_id: row.oracle_id.clone(),
                count: row.quantity,
            });
        }
        cards.sort_unstable_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
        Decklist { commander, cards }
    });

    let catalog = artifact.catalog().catalog();
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
    let projection = ValidatedCatalog::new(projection).map_err(unavailable)?;
    let normalized = MatchConfig {
        profile: DYNAMIC_PROFILE.into(),
        decks,
        starting_player: config.starting_player,
        seed: config.seed,
    };
    let admission_digest = digest(&(
        DYNAMIC_PROFILE,
        artifact.manifest_sha256(),
        &policy.digest,
        projection.digest(),
        &normalized,
    ))
    .map_err(unavailable)?;
    let identity = DynamicAdmissionIdentity {
        bulk_manifest_sha256: artifact.manifest_sha256().into(),
        policy_digest: policy.digest.clone(),
        catalog_digest: projection.digest().into(),
        admission_digest,
    };
    let prepared = Prepared {
        config: physical_match_config(DYNAMIC_PROFILE, &normalized.decks),
        match_config: Some(normalized),
        catalog: projection.shared_catalog(),
        catalog_digest: identity.catalog_digest.clone(),
        profile_digest: identity.admission_digest.clone(),
    };
    Ok(DynamicPod { prepared, identity })
}

fn capacity_supported(catalog: &Catalog, native_triggers: usize) -> bool {
    catalog
        .cast_trigger_fanout_bound(PHYSICAL_SOURCES)
        .is_some_and(|bound| bound < STACK_LIMIT / 2)
        && (PHYSICAL_SOURCES + TOKEN_CAPACITY as usize)
            .checked_mul(native_triggers)
            .is_some_and(|wave| wave < STACK_LIMIT / 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage1d_dynamic_capacity_and_copy_gates_are_fail_closed() {
        let profile = match_profile("commander-foundations-v1").unwrap();
        let factor = PHYSICAL_SOURCES + TOKEN_CAPACITY as usize;
        let boundary = (STACK_LIMIT / 2).div_ceil(factor);
        assert!(capacity_supported(profile.catalog(), boundary - 1));
        assert!(!capacity_supported(profile.catalog(), boundary));
        assert!(!capacity_supported(profile.catalog(), usize::MAX));
        assert!(copy_capacity_unreviewed(
            Capability::EnterAsCopyOfPermanentEnteredThisTurnV1
        ));
        assert!(copy_capacity_unreviewed(
            Capability::ControlledCreatureSpellCopyV1
        ));
        assert!(match_profile(DYNAMIC_PROFILE).is_none());
    }

    #[test]
    fn stage1d_dynamic_policy_matches_reviewed_foundations_projection() {
        let profile = match_profile("commander-foundations-v1").unwrap();
        assert_eq!(profile.policy_digest(), POLICY_SHA256);
        assert!(policy().is_ok());
        let ids: BTreeSet<_> = profile
            .decks
            .iter()
            .flat_map(|d| d.cards.iter().map(|row| &row.oracle_id))
            .collect();
        let requirements: BTreeSet<_> = ids
            .iter()
            .flat_map(|id| {
                profile
                    .catalog()
                    .definition_by_oracle_id(id)
                    .unwrap()
                    .requirements()
            })
            .collect();
        assert_eq!(requirements, REVIEWED.iter().copied().collect());
    }
}
