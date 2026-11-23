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

mod roster_review;
use roster_review::Review;

pub const DYNAMIC_PROFILE: &str = "commander-dynamic-headless-v1";
const ADMISSION_DOMAIN: &str = "commander-dynamic-pair-admission-v2";
pub const DYNAMIC_MAX_DECK_ROWS: usize = 100;
const DECK_SIZE: u64 = 100;
const PHYSICAL_SOURCES: usize = 400;
const POLICY_SHA256: &str = "71b1aa15189c83820fe654933e5ad28d79897b7e46c3f71155a16cd3430424da";
const POLICY: &str = include_str!("../../../../tests/fixtures/commander-banned-2026-10-15.json");

// The exact 21-definition Foundations projection exercises these requirements.
// Expanding this set needs dynamic capacity and runtime qualification.
const REVIEWED: &[Capability] = &[
    Capability::AnyPlayerTargetV1,
    Capability::BaseCreatureV1,
    Capability::BasicLandManaV1,
    Capability::CastCreatureV1,
    Capability::CastInstantV1,
    Capability::CastSorceryV1,
    Capability::CommanderIdentityV1,
    Capability::ControllerDrawV1,
    Capability::CreatureDiesV1,
    Capability::CreatureTargetV1,
    Capability::DefenderV1,
    Capability::LandPlayV1,
    Capability::LifeGainV1,
    Capability::LifeLossV1,
    Capability::NormalLayoutV1,
    Capability::OpponentTargetV1,
    Capability::OtherCreatureEntersBattlefieldV1,
    Capability::SelfEntersBattlefieldV1,
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
    let digest = digest(&(
        ADMISSION_DOMAIN,
        POLICY_SHA256,
        REVIEWED,
        roster_review::POLICY_INPUT,
    ))?;
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
    let review = roster_review::seat(artifact.catalog().catalog(), rows);
    Ok(inspect(artifact, rows, policy()?, review))
}

fn inspect(
    artifact: &BulkArtifact,
    rows: &[DynamicDeckEntry],
    policy: &DynamicPolicy,
    review: Option<Review>,
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
                review,
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
    review: Option<Review>,
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
    // The pinned roster review covers these exact compiled definitions.
    // Pod admission still requires the accompanying four-seat capacity proof.
    if review == Some(Review::Copy) {
        return;
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
        Self { commanders, cards }
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
pub struct DynamicPodTemplate {
    decks: [NormalizedDeck; 4],
    projection: ValidatedCatalog,
    bulk_manifest_sha256: String,
    policy_digest: String,
}

impl DynamicPodTemplate {
    pub fn admit(
        artifact: &BulkArtifact,
        decks: &[Vec<DynamicDeckEntry>; 4],
    ) -> Result<Self, DynamicAdmissionError> {
        let policy = policy().map_err(unavailable)?;
        let catalog = artifact.catalog().catalog();
        let reviews = decks
            .each_ref()
            .map(|rows| roster_review::seat(catalog, rows));
        let inspections =
            std::array::from_fn::<_, 4, _>(|i| inspect(artifact, &decks[i], policy, reviews[i]));
        if inspections.iter().any(|r| !r.admissible()) {
            return Err(DynamicAdmissionError::DeckGaps {
                decks: Vec::from(inspections),
            });
        }
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
        let copy_roster = reviews.contains(&Some(Review::Copy));
        let fits = if copy_roster {
            roster_review::capacity_supported(&projection, reviews)
        } else {
            capacity_supported(&projection, native_triggers)
        };
        if !fits {
            return Err(DynamicAdmissionError::PodCapacity);
        }
        Ok(Self {
            decks,
            projection: ValidatedCatalog::new(projection).map_err(unavailable)?,
            bulk_manifest_sha256: artifact.manifest_sha256().into(),
            policy_digest: policy.digest.clone(),
        })
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
                decks.map(|deck| (deck.commanders.as_slice(), deck.cards.as_slice())),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(types: &str, oracle_text: &str, keywords: &[&str]) -> BulkCardFacts {
        BulkCardFacts::from_source(&serde_json::json!({
            "name": "Fixture", "layout": "normal", "type_line": types, "oracle_text": oracle_text,
            "keywords": keywords, "color_identity": ["W"], "legalities": {"commander":"legal"},
        }))
        .unwrap()
    }

    #[test]
    fn dynamic_pairing_requires_the_exact_pregame_keyword_and_doctor_types() {
        let creature = "Legendary Creature — Human";
        let partner = facts(
            creature,
            "Flying\nPartner (You can have two commanders if both have partner.)",
            &["Partner"],
        );
        let second = facts(creature, "Partner", &["Partner"]);
        assert_eq!(pairing(&partner, &second), Some(CommanderPairing::Partner));
        for text in [
            "Partner with Fixture",
            "Partner—Friends forever",
            "Choose a Background",
            "Create a token with partner.",
        ] {
            let variant = facts(creature, text, &["Partner"]);
            assert_eq!(pairing(&partner, &variant), None);
        }
        let companion = facts(
            creature,
            "Doctor's companion (You can have two commanders if the other is the Doctor.)",
            &["Doctor's companion"],
        );
        let doctor = facts("Legendary Creature — Time Lord Doctor", "Flying", &[]);
        assert_eq!(
            pairing(&companion, &doctor),
            Some(CommanderPairing::DoctorsCompanion)
        );
        assert_eq!(pairing(&doctor, &companion), pairing(&companion, &doctor));
        assert_eq!(pairing(&companion, &companion), None);
        let extra = facts("Legendary Creature — Time Lord Doctor Elf", "", &[]);
        let changeling = facts(
            "Legendary Creature — Time Lord Doctor",
            "Changeling",
            &["Changeling"],
        );
        let nonlegendary = facts("Creature — Time Lord Doctor", "", &[]);
        for invalid in [extra, changeling, nonlegendary, partner] {
            assert_eq!(pairing(&companion, &invalid), None);
        }
    }

    // Exact executable Foundations definitions isolate the physical rules.
    // Pairing legality is covered separately by authenticated archive facts.
    fn mixed_roster() -> DynamicPodTemplate {
        let profile = match_profile("commander-foundations-v1").unwrap();
        let config = MatchConfig::for_profile(&profile.name, [19; 32], PlayerId(0)).unwrap();
        let mut rows = DynamicMatchConfig::from_match_config(&config).decks;
        let primary = config.decks[0].commander.as_str();
        let second = profile
            .commanders
            .iter()
            .find(|id| id.as_str() != primary)
            .unwrap();
        let basic = rows[0].iter_mut().find(|row| row.quantity > 1).unwrap();
        basic.quantity -= 1;
        rows[0].push(DynamicDeckEntry {
            oracle_id: second.clone(),
            quantity: 1,
            role: DeckRole::Commander,
        });
        let decks = rows.each_ref().map(|r| NormalizedDeck::new(r));
        let mut projection = profile.catalog().clone();
        projection.definitions.retain(|d| {
            rows.iter()
                .flatten()
                .any(|row| row.oracle_id == d.oracle_id)
        });
        DynamicPodTemplate {
            decks,
            projection: ValidatedCatalog::new(projection).unwrap(),
            bulk_manifest_sha256: "test-bulk".into(),
            policy_digest: policy().unwrap().digest.clone(),
        }
    }

    #[test]
    fn dynamic_pair_runtime_preserves_split_ledger_union_and_mixed_opening_replay() {
        let template = mixed_roster();
        let pod = template.prepare(PlayerId(0), [19; 32], 0).unwrap();
        let same = template.prepare(PlayerId(0), [19; 32], 0).unwrap();
        assert_eq!(pod.identity, same.identity);
        assert!(Arc::ptr_eq(&pod.prepared.catalog, &same.prepared.catalog));
        assert!(pod.prepared.match_config().is_none());
        assert_eq!(
            pod.normalized_config().decks[0]
                .iter()
                .filter(|row| row.role == DeckRole::Commander)
                .count(),
            2
        );
        assert_eq!(
            pod.prepared.config.commanders,
            [0, 1, 100, 200, 300].map(CardId)
        );
        assert!(template.prepare(PlayerId(0), [19; 32], 4).is_err());
        let rotated = template.prepare(PlayerId(1), [19; 32], 1).unwrap();
        assert_eq!(
            rotated.prepared.config.commanders,
            [0, 100, 101, 200, 300].map(CardId)
        );
        assert_ne!(
            rotated.identity.admission_digest,
            pod.identity.admission_digest
        );

        let prepared = pod.prepared;
        let mut game = Game::with_trace(prepared.clone(), TraceMode::Off);
        assert_eq!(game.opening_library_sizes, [98, 99, 99, 99]);
        assert_eq!(
            game.mana_colors(PlayerId(0), &ManaProduction::CommanderColor)
                .unwrap(),
            vec![Color::W, Color::B, Color::G]
        );
        assert!(matches!(
            game.observe(PlayerId(1), Budget::unlimited()),
            QueryResult::Rejected(QueryRejection::UnavailableDuringSetup)
        ));
        let mut ledger = game.commander_ledger().clone();
        let caster = PlayerId(0);
        let defender = PlayerId(1);
        ledger
            .record_cast(CardId(0), caster, Zone::Command { owner: caster })
            .unwrap();
        assert_eq!(ledger.tax(CardId(0), caster).unwrap(), Quantity::from(2));
        assert_eq!(ledger.tax(CardId(1), caster).unwrap(), Quantity::zero());
        ledger
            .record_damage(CardId(0), defender, true, &Quantity::from(11))
            .unwrap();
        ledger
            .record_damage(CardId(1), defender, true, &Quantity::from(10))
            .unwrap();
        assert!(ledger.combat_loss_candidates().is_empty());
        ledger
            .record_damage(CardId(0), defender, true, &Quantity::from(10))
            .unwrap();
        assert_eq!(ledger.combat_loss_candidates(), vec![defender]);

        assert!(matches!(
            game.advance(Budget::new(1)),
            Advance::Suspended { .. }
        ));
        let resumed = Game::restore(prepared.clone(), &game.snapshot().unwrap()).unwrap();
        assert_eq!(
            resumed.state_digest().unwrap(),
            game.state_digest().unwrap()
        );
        for _ in 0..64 {
            let Advance::AwaitingDecision(request) = game.advance(Budget::new(20_000)) else {
                panic!("bounded opening should reach a decision");
            };
            if matches!(request, Request::Priority { player, .. } if player == caster)
                && game.turn().step == TurnStep::PrecombatMain
            {
                let QueryResult::Complete(QueryValue::Observation(observed)) =
                    game.observe(PlayerId(1), Budget::unlimited())
                else {
                    panic!("precombat observation");
                };
                let rows: Vec<_> = observed
                    .view
                    .commanders
                    .iter()
                    .filter(|c| c.history.owner == PlayerId(0))
                    .collect();
                assert_eq!(rows.len(), 2);
                assert_ne!(rows[0].designation, rows[1].designation);
                let journal = game.replay_journal();
                assert!(journal.decisions.len() > 4);
                let replay = Game::replay_with_trace(prepared, &journal, TraceMode::Off).unwrap();
                assert_eq!(replay.state_digest().unwrap(), game.state_digest().unwrap());
                return;
            }
            let response = match &request {
                Request::Mulligan {
                    player,
                    mulligans_taken,
                    ..
                } => Response::Mulligan {
                    take: *player == caster && *mulligans_taken == 0,
                },
                Request::Priority { .. } => Response::Pass,
                other => panic!("unexpected opening decision {other:?}"),
            };
            game.respond(request.player(), request.decision_id(), response)
                .unwrap();
        }
        panic!("mixed opening did not finish");
    }

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

impl std::error::Error for DynamicAdmissionError {}
