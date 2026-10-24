//! Finite legal-deck construction, separate from injected scenario admission.
//! CR903.3–.7: https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use std::fmt;

pub const MATCH_PROFILE: &str = "commander-foundations-v1";
pub const MATCH_CATALOG_SHA256: &str =
    "fc4bd3390e63ea352e129ac370fa848dbee11cf0700e7e6781d449fd49244a7f";
pub const KEYWORDS_PROFILE: &str = "commander-keywords-v1";
pub const KEYWORDS_CATALOG_SHA256: &str =
    "72691c17b1bc73b5735f1ced8c5217b51df4daae0f42c9c883665933cfcdf550";
pub const COMBAT_KEYWORDS_PROFILE: &str = "commander-combat-keywords-v1";
pub const COMBAT_KEYWORDS_CATALOG_SHA256: &str =
    "ae5779a29b61fabe147a6ffd7dde6122442201a778e6b36eb9b0992a38ede55c";
pub const TOKENS_PROFILE: &str = "commander-tokens-v1";
pub const TOKENS_CATALOG_SHA256: &str =
    "44ca001781d3109d713f9df037d83a74f8f854a0d179a053c34a7dcb82326120";
pub const STAPLES_PROFILE: &str = "commander-staples-v1";
pub const STAPLES_CATALOG_SHA256: &str =
    "c0c9d9d238c60d00f3cad4b40e6f95b65225e67244b526f4a188b0faf33fff2a";
pub const MANA_PROFILE: &str = "commander-mana-v1";
pub const MANA_CATALOG_SHA256: &str =
    "1df5823602dd4aacea6ab8e69766dc750e425686ba02221857b443c415b37794";
pub const SACRIFICE_PROFILE: &str = "commander-sacrifice-v1";
pub const SACRIFICE_CATALOG_SHA256: &str =
    "9ae8605e83e6538ff900227bde4f43b91d06cb6acbc8afd22fd35ef2388c0a88";
pub const REMOVAL_PROFILE: &str = "commander-removal-v1";
pub const REMOVAL_CATALOG_SHA256: &str =
    "0ff60a7e1dc37cd0a76dbdb6b6d01ea0303d7c29ed0f6c2df20229484e6ea3c6";
pub const ATTACHMENTS_PROFILE: &str = "commander-attachments-v1";
pub const ATTACHMENTS_CATALOG_SHA256: &str =
    "56affddedfe7b2482ad48238b95b3f8c30bfc39c701125cff883fa6ea9c404df";
pub const FOUR_COLORS_PROFILE: &str = "commander-four-colors-v1";
pub const FOUR_COLORS_CATALOG_SHA256: &str =
    "0ca81434d87be618d60f1267783fe3d89983f397a01bed7c0640ade57a4dd93b";
/// All reviewed legal-profile identity and construction inputs live here.
/// MATCH_PROFILES is an ordered lineage: each entry inherits earlier mechanics.
pub struct MatchProfile {
    pub name: &'static str,
    pub catalog_sha256: &'static str,
    pub ir_version: u32,
    pub policy_json: &'static str,
    pub commanders: &'static [&'static str],
    pub white_deck: fn() -> Decklist,
    pub black_green_deck: fn() -> Decklist,
    pub default_pod: [fn() -> Decklist; 4],
}
pub static MATCH_PROFILES: [MatchProfile; 10] = [
    MatchProfile {
        name: MATCH_PROFILE,
        catalog_sha256: MATCH_CATALOG_SHA256,
        ir_version: 11,
        policy_json: super::commander_policy::COMMANDER_POLICY_JSON,
        commanders: &[ISAMARU, YARGLE],
        white_deck,
        black_green_deck,
        default_pod: [white_deck, black_green_deck, white_deck, black_green_deck],
    },
    MatchProfile {
        name: KEYWORDS_PROFILE,
        catalog_sha256: KEYWORDS_CATALOG_SHA256,
        ir_version: 12,
        policy_json: include_str!("../../../../tests/fixtures/commander-keywords-policy.json"),
        commanders: &[ISAMARU, YARGLE],
        white_deck: keywords_white_deck,
        black_green_deck: keywords_black_green_deck,
        default_pod: [
            keywords_white_deck,
            keywords_black_green_deck,
            keywords_white_deck,
            keywords_black_green_deck,
        ],
    },
    MatchProfile {
        name: COMBAT_KEYWORDS_PROFILE,
        catalog_sha256: COMBAT_KEYWORDS_CATALOG_SHA256,
        ir_version: 13,
        policy_json: include_str!(
            "../../../../tests/fixtures/commander-combat-keywords-policy.json"
        ),
        commanders: &[ISAMARU, YARGLE],
        white_deck: combat_keywords_white_deck,
        black_green_deck: combat_keywords_black_green_deck,
        default_pod: [
            combat_keywords_white_deck,
            combat_keywords_black_green_deck,
            combat_keywords_white_deck,
            combat_keywords_black_green_deck,
        ],
    },
    MatchProfile {
        name: TOKENS_PROFILE,
        catalog_sha256: TOKENS_CATALOG_SHA256,
        ir_version: 14,
        policy_json: include_str!("../../../../tests/fixtures/commander-tokens-policy.json"),
        commanders: &[ISAMARU, YARGLE],
        white_deck: tokens_white_deck,
        black_green_deck: tokens_black_green_deck,
        default_pod: [
            tokens_white_deck,
            tokens_black_green_deck,
            tokens_white_deck,
            tokens_black_green_deck,
        ],
    },
    MatchProfile {
        name: STAPLES_PROFILE,
        catalog_sha256: STAPLES_CATALOG_SHA256,
        ir_version: 15,
        policy_json: include_str!("../../../../tests/fixtures/commander-staples-policy.json"),
        commanders: &[ISAMARU, YARGLE],
        white_deck: staples_white_deck,
        black_green_deck: staples_black_green_deck,
        default_pod: [
            staples_white_deck,
            staples_black_green_deck,
            staples_white_deck,
            staples_black_green_deck,
        ],
    },
    MatchProfile {
        name: MANA_PROFILE,
        catalog_sha256: MANA_CATALOG_SHA256,
        ir_version: 16,
        policy_json: include_str!("../../../../tests/fixtures/commander-mana-policy.json"),
        commanders: &[ISAMARU, YARGLE],
        white_deck: mana_white_deck,
        black_green_deck: mana_black_green_deck,
        default_pod: [
            mana_white_deck,
            mana_black_green_deck,
            mana_white_deck,
            mana_black_green_deck,
        ],
    },
    MatchProfile {
        name: FOUR_COLORS_PROFILE,
        catalog_sha256: FOUR_COLORS_CATALOG_SHA256,
        ir_version: 16,
        policy_json: include_str!("../../../../tests/fixtures/commander-four-colors-policy.json"),
        commanders: &[
            ISAMARU,
            YARGLE,
            "4bbc5df9-60d7-46c1-8856-91c9c74abf18",
            "48fe0f52-1676-49d5-acf6-5476081e8e1b",
        ],
        white_deck: mana_white_deck,
        black_green_deck: mana_black_green_deck,
        default_pod: [
            mana_white_deck,
            mana_black_green_deck,
            four_colors_blue_deck,
            four_colors_red_deck,
        ],
    },
    MatchProfile {
        name: ATTACHMENTS_PROFILE,
        catalog_sha256: ATTACHMENTS_CATALOG_SHA256,
        ir_version: 17,
        policy_json: include_str!("../../../../tests/fixtures/commander-attachments-policy.json"),
        commanders: &[
            ISAMARU,
            YARGLE,
            "4bbc5df9-60d7-46c1-8856-91c9c74abf18",
            "48fe0f52-1676-49d5-acf6-5476081e8e1b",
        ],
        white_deck: attachments_white_deck,
        black_green_deck: attachments_black_green_deck,
        default_pod: [
            attachments_white_deck,
            attachments_black_green_deck,
            attachments_blue_deck,
            attachments_red_deck,
        ],
    },
    MatchProfile {
        name: REMOVAL_PROFILE,
        catalog_sha256: REMOVAL_CATALOG_SHA256,
        ir_version: 18,
        policy_json: include_str!("../../../../tests/fixtures/commander-removal-policy.json"),
        commanders: &[
            ISAMARU,
            YARGLE,
            "4bbc5df9-60d7-46c1-8856-91c9c74abf18",
            "48fe0f52-1676-49d5-acf6-5476081e8e1b",
        ],
        white_deck: removal_white_deck,
        black_green_deck: removal_black_green_deck,
        default_pod: [
            removal_white_deck,
            removal_black_green_deck,
            removal_blue_deck,
            removal_red_deck,
        ],
    },
    MatchProfile {
        name: SACRIFICE_PROFILE,
        catalog_sha256: SACRIFICE_CATALOG_SHA256,
        ir_version: 19,
        policy_json: include_str!("../../../../tests/fixtures/commander-sacrifice-policy.json"),
        commanders: &[
            ISAMARU,
            YARGLE,
            "4bbc5df9-60d7-46c1-8856-91c9c74abf18",
            "48fe0f52-1676-49d5-acf6-5476081e8e1b",
        ],
        white_deck: sacrifice_white_deck,
        black_green_deck: sacrifice_black_green_deck,
        default_pod: [
            sacrifice_white_deck,
            sacrifice_black_green_deck,
            removal_blue_deck,
            sacrifice_red_deck,
        ],
    },
];
pub fn match_profile(name: &str) -> Option<&'static MatchProfile> {
    MATCH_PROFILES.iter().find(|p| p.name == name)
}
/// Whether a legal profile is at or after `minimum` in the reviewed lineage.
/// Unknown names (including historical scenario profiles) never inherit it.
pub fn profile_at_least(profile: &str, minimum: &str) -> bool {
    MATCH_PROFILES
        .iter()
        .position(|p| p.name == minimum)
        .is_some_and(|start| MATCH_PROFILES[start..].iter().any(|p| p.name == profile))
}
impl Game {
    pub(super) fn profile_at_least(&self, minimum: &str) -> bool {
        let profile = self.prepared.config.profile.as_str();
        profile_at_least(profile, minimum)
            && match_profile(profile)
                .is_some_and(|p| p.ir_version == self.prepared.catalog.schema_version)
    }
}

const WALL_OF_WOOD: &str = "aa5e6894-8c97-46f8-a0fd-07c6db51e9a7";
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MatchConfig {
    pub profile: String,
    pub decks: [Decklist; 4],
    pub starting_player: PlayerId,
    /// Trusted host input. Never include actual seed/order in player views.
    pub seed: [u8; 32],
}
impl MatchConfig {
    pub fn foundations(seed: [u8; 32], starting_player: PlayerId) -> Self {
        Self::for_profile(MATCH_PROFILE, seed, starting_player).expect("reviewed profile")
    }
    pub fn combat_keywords(seed: [u8; 32], starting_player: PlayerId) -> Self {
        Self::for_profile(COMBAT_KEYWORDS_PROFILE, seed, starting_player).expect("reviewed profile")
    }
    pub fn tokens(seed: [u8; 32], starting_player: PlayerId) -> Self {
        Self::for_profile(TOKENS_PROFILE, seed, starting_player).expect("reviewed profile")
    }
    pub fn keywords(seed: [u8; 32], starting_player: PlayerId) -> Self {
        Self::for_profile(KEYWORDS_PROFILE, seed, starting_player).expect("reviewed profile")
    }
    pub fn for_profile(
        name: &str,
        seed: [u8; 32],
        starting_player: PlayerId,
    ) -> Result<Self, MatchAdmissionError> {
        let p = match_profile(name).ok_or(MatchAdmissionError::UnsupportedProfile)?;
        Ok(Self {
            profile: p.name.into(),
            decks: p.default_pod.map(|deck| deck()),
            starting_player,
            seed,
        })
    }
}
fn entries(rows: &[(&str, u32)]) -> Vec<DeckEntry> {
    let mut result = rows
        .iter()
        .map(|(id, count)| DeckEntry {
            oracle_id: (*id).into(),
            count: *count,
        })
        .collect::<Vec<_>>();
    result.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    result
}
pub fn white_deck() -> Decklist {
    Decklist {
        commander: ISAMARU.into(),
        cards: entries(&[
            (ISAMARU, 1),
            (OX, 1),
            (HEALER, 1),
            (NECTAR, 1),
            (HEALING_HANDS, 1),
            (SALVE, 1),
            (PLAINS, 94),
        ]),
    }
}
pub fn black_green_deck() -> Decklist {
    Decklist {
        commander: YARGLE.into(),
        cards: entries(&[
            (YARGLE, 1),
            (EDICT, 1),
            (BEARS, 1),
            (FOG, 1),
            (BLOOD_ARTIST, 1),
            (GROWTH, 1),
            (WARDEN, 1),
            (DISFIGURE, 1),
            (SOUL_FEAST, 1),
            (BOUNCER, 1),
            (VISIONARY, 1),
            (WALL_OF_WOOD, 1),
            (FOREST, 44),
            (SWAMP, 44),
        ]),
    }
}
pub fn keywords_white_deck() -> Decklist {
    let mut deck = white_deck();
    for row in &mut deck.cards {
        if row.count > 1 {
            row.count = 89;
        }
    }
    deck.cards.push(DeckEntry {
        oracle_id: "4b7ac066-e5c7-43e6-9e7e-2739b24a905d".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "28a52ba1-95da-44e1-8ac5-0dc23c902394".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "31009c45-afba-45f4-a6ca-9dfdb9990e72".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "6a0419c3-4bfa-46c1-932b-fe10f2fa2ad7".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "4502b24f-604b-4e36-9168-31c1a1ab4dab".into(),
        count: 1,
    });
    deck.cards.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    deck
}
pub fn keywords_black_green_deck() -> Decklist {
    let mut deck = black_green_deck();
    for row in &mut deck.cards {
        if row.count > 1 {
            row.count = if row.oracle_id == FOREST { 39 } else { 38 };
        }
    }
    deck.cards.push(DeckEntry {
        oracle_id: "feb244f8-bcb1-44cf-9940-2719221a7309".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "d6ee6cc1-902d-4f56-afa5-6fa4813bfbbc".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "63d846e2-4bef-4911-9508-273c75a4a040".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "72a53d9e-0645-4024-a90f-73b05c2284c6".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "c650a7bc-e350-44a0-a698-d4a233d66156".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "e740ce2f-2134-473c-afa1-1b6d2d1e38ef".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "37f3733e-cc4e-4d84-b29b-d474f6e254a2".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "6b941291-1802-4ced-9869-39fa43825550".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "8178c546-84f8-4563-a804-6489ec016660".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "913dbce2-6805-453f-a9c3-64f1cc658627".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "51ce0158-c9a5-4fa5-9704-46ab02f01b86".into(),
        count: 1,
    });
    deck.cards.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    deck
}
pub fn staples_white_deck() -> Decklist {
    let mut deck = tokens_white_deck();
    for row in &mut deck.cards {
        if row.count > 1 {
            row.count = 76;
        }
    }
    deck.cards.push(DeckEntry {
        oracle_id: "d057289d-5e28-43d5-8ff3-4a1bc723477d".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "e3886fe8-9b76-4613-8891-4ec74657c087".into(),
        count: 1,
    });
    deck.cards.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    deck
}
pub fn staples_black_green_deck() -> Decklist {
    let mut deck = tokens_black_green_deck();
    for row in &mut deck.cards {
        if row.count > 1 {
            row.count = if row.oracle_id == FOREST { 29 } else { 30 };
        }
    }
    deck.cards.push(DeckEntry {
        oracle_id: "938b4e2c-88d9-4637-bc00-e228920c9a78".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "cbc9c731-181a-4f00-a7b0-eb7e56eac2ea".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "1a2030cc-d7ee-4059-b2d7-fb95ea8e267b".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "7ffae8f8-3006-4969-a339-6d30678f87ea".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "c6207f6a-a624-4754-88f5-dbe700c841ff".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "8539f295-5d58-4436-a73a-b9277c4c7795".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "a0abd957-01a0-4aa9-8fc8-0e6840d21606".into(),
        count: 1,
    });
    deck.cards.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    deck
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case", deny_unknown_fields)]
pub enum MatchAdmissionError {
    UnsupportedProfile,
    InvalidStartingPlayer,
    InvalidCatalog {
        detail: String,
    },
    InvalidCount {
        player: PlayerId,
        oracle_id: String,
        count: u32,
    },
    InvalidDeckSize {
        player: PlayerId,
        total: u64,
    },
    BannedCard {
        player: PlayerId,
        oracle_id: String,
        name: String,
    },
    UnsupportedCard {
        player: PlayerId,
        oracle_id: String,
    },
    UnsupportedCommander {
        player: PlayerId,
        oracle_id: String,
    },
    CommanderCount {
        player: PlayerId,
        count: u32,
    },
    SingletonName {
        player: PlayerId,
        name: String,
    },
    ColorIdentity {
        player: PlayerId,
        oracle_id: String,
    },
    BasicLandColor {
        player: PlayerId,
        oracle_id: String,
    },
}
impl fmt::Display for MatchAdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for MatchAdmissionError {}
/// Construction is private; successful validation is the only way to produce
/// this immutable initialization description. It is not deserialized from a client.
#[derive(Clone, Debug)]
pub struct ValidatedPod {
    pub(super) config: ScenarioConfig,
    pub(super) match_config: MatchConfig,
}
impl ValidatedPod {
    pub fn normalized_config(&self) -> &MatchConfig {
        &self.match_config
    }
    pub fn physical_config(&self) -> &ScenarioConfig {
        &self.config
    }
    pub fn identity_digest(&self) -> Result<String, String> {
        let p = match_profile(&self.match_config.profile).ok_or("unknown profile")?;
        digest(&(
            p.name,
            p.catalog_sha256,
            sha256(p.policy_json.as_bytes()),
            &self.match_config,
        ))
    }
}
fn known_ban(player: PlayerId, id: &str) -> Result<(), MatchAdmissionError> {
    if let Some(name) = super::commander_policy::banned_commander_card(id) {
        return Err(MatchAdmissionError::BannedCard {
            player,
            oracle_id: id.into(),
            name: name.into(),
        });
    }
    Ok(())
}
fn validate_deck_colors(
    player: PlayerId,
    id: &str,
    card: &Characteristics,
    commander: &Characteristics,
) -> Result<(), MatchAdmissionError> {
    if card
        .color_identity
        .iter()
        .any(|c| !commander.color_identity.contains(c))
    {
        return Err(MatchAdmissionError::ColorIdentity {
            player,
            oracle_id: id.into(),
        });
    }
    //903.5d is independent of color identity, including for nonbasic lands.
    if card
        .subtypes
        .iter()
        .filter_map(|s| basic_land_color(s))
        .any(|c| !commander.color_identity.contains(&c))
    {
        return Err(MatchAdmissionError::BasicLandColor {
            player,
            oracle_id: id.into(),
        });
    }
    Ok(())
}
pub fn validate_pod(
    catalog: &Catalog,
    config: &MatchConfig,
) -> Result<ValidatedPod, MatchAdmissionError> {
    let profile = match_profile(&config.profile).ok_or(MatchAdmissionError::UnsupportedProfile)?;
    if config.starting_player.0 >= 4 {
        return Err(MatchAdmissionError::InvalidStartingPlayer);
    }
    catalog
        .validate()
        .map_err(|detail| MatchAdmissionError::InvalidCatalog { detail })?;
    if catalog.schema_version != profile.ir_version
        || digest(catalog).map_err(|detail| MatchAdmissionError::InvalidCatalog { detail })?
            != profile.catalog_sha256
    {
        return Err(MatchAdmissionError::InvalidCatalog {
            detail: if profile.name == SACRIFICE_PROFILE {
                "catalog differs from reviewed one-hundred-fifty-three-definition sacrifice pool"
            } else if profile.name == REMOVAL_PROFILE {
                "catalog differs from reviewed one-hundred-thirty-five-definition removal pool"
            } else if profile.name == MATCH_PROFILE {
                "catalog differs from reviewed thirty-four-definition pool"
            } else if profile.name == FOUR_COLORS_PROFILE {
                "catalog differs from reviewed 106-definition four-colors pool"
            } else if profile.name == STAPLES_PROFILE {
                "catalog differs from reviewed eighty-one-definition staples pool"
            } else if profile.name == TOKENS_PROFILE {
                "catalog differs from reviewed seventy-two-definition tokens pool"
            } else if profile.name == MANA_PROFILE {
                "catalog differs from reviewed ninety-definition mana pool"
            } else if profile.name == COMBAT_KEYWORDS_PROFILE {
                "catalog differs from reviewed fifty-nine-definition combat keywords pool"
            } else {
                "catalog differs from reviewed fifty-definition keywords pool"
            }
            .into(),
        });
    }
    let definitions = catalog
        .definitions
        .iter()
        .map(|d| (d.oracle_id.as_str(), d))
        .collect::<BTreeMap<_, _>>();
    let mut normalized = config.clone();
    for (seat, deck) in config.decks.iter().enumerate() {
        let player = PlayerId(seat as u8);
        known_ban(player, &deck.commander)?;
        if !profile.commanders.contains(&deck.commander.as_str()) {
            // A different rule-eligible commander (including some legendary
            // Vehicles under903.3) is unsupported here, not universally illegal.
            return Err(MatchAdmissionError::UnsupportedCommander {
                player,
                oracle_id: deck.commander.clone(),
            });
        }
        let commander = definitions.get(deck.commander.as_str()).ok_or_else(|| {
            MatchAdmissionError::UnsupportedCommander {
                player,
                oracle_id: deck.commander.clone(),
            }
        })?;
        let mut total = 0u64;
        let mut counts = BTreeMap::<String, u32>::new();
        for row in &deck.cards {
            if row.count == 0 {
                return Err(MatchAdmissionError::InvalidCount {
                    player,
                    oracle_id: row.oracle_id.clone(),
                    count: row.count,
                });
            }
            total = total.checked_add(u64::from(row.count)).ok_or(
                MatchAdmissionError::InvalidDeckSize {
                    player,
                    total: u64::MAX,
                },
            )?;
            if total > 100 {
                return Err(MatchAdmissionError::InvalidDeckSize { player, total });
            }
            known_ban(player, &row.oracle_id)?;
            let count = counts.entry(row.oracle_id.clone()).or_default();
            *count = count
                .checked_add(row.count)
                .ok_or(MatchAdmissionError::InvalidDeckSize { player, total })?;
        }
        if total != 100 {
            return Err(MatchAdmissionError::InvalidDeckSize { player, total });
        }
        let commander_count = counts.get(&deck.commander).copied().unwrap_or(0);
        if commander_count != 1 {
            return Err(MatchAdmissionError::CommanderCount {
                player,
                count: commander_count,
            });
        }
        let mut names = BTreeSet::new();
        for (id, count) in &counts {
            let d = definitions.get(id.as_str()).ok_or_else(|| {
                MatchAdmissionError::UnsupportedCard {
                    player,
                    oracle_id: id.clone(),
                }
            })?;
            if !d.characteristics.basic && (*count != 1 || !names.insert(&d.name)) {
                return Err(MatchAdmissionError::SingletonName {
                    player,
                    name: d.name.clone(),
                });
            }
            validate_deck_colors(player, id, &d.characteristics, &commander.characteristics)?;
        }
        normalized.decks[seat].cards = counts
            .into_iter()
            .map(|(oracle_id, count)| DeckEntry { oracle_id, count })
            .collect();
    }
    let mut cards = Vec::with_capacity(400);
    let mut commanders = Vec::with_capacity(4);
    for (seat, deck) in normalized.decks.iter().enumerate() {
        let owner = PlayerId(seat as u8);
        let mut next = seat as u32 * 100;
        commanders.push(CardId(next));
        cards.push(CardSetup {
            id: CardId(next),
            oracle_id: deck.commander.clone(),
            owner,
            controller: owner,
            zone: Zone::Command { owner },
        });
        next += 1;
        for row in &deck.cards {
            if row.oracle_id == deck.commander {
                continue;
            }
            for _ in 0..row.count {
                cards.push(CardSetup {
                    id: CardId(next),
                    oracle_id: row.oracle_id.clone(),
                    owner,
                    controller: owner,
                    zone: Zone::Library { owner },
                });
                next += 1;
            }
        }
        debug_assert_eq!(next, (seat as u32 + 1) * 100);
    }
    Ok(ValidatedPod {
        config: ScenarioConfig {
            mode: Mode::CommanderMatch,
            players: 4,
            profile: profile.name.into(),
            commanders,
            cards,
        },
        match_config: normalized,
    })
}

pub fn combat_keywords_white_deck() -> Decklist {
    let mut deck = keywords_white_deck();
    deck.cards
        .iter_mut()
        .find(|r| r.oracle_id == "bc71ebf6-2056-41f7-be35-b2e5c34afa99")
        .unwrap()
        .count = 85;
    deck.cards.push(DeckEntry {
        oracle_id: "ef2a24f5-ce5e-4054-843a-2cae0c66318a".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "4e6c71e8-aeaf-404e-84c2-c79f2a1cbe4b".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "2f810936-2ba6-4c2b-84a0-ff4c1deb026b".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "0c07d09e-e127-4573-b827-6c50246f7a31".into(),
        count: 1,
    });
    deck.cards.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    deck
}

pub fn combat_keywords_black_green_deck() -> Decklist {
    let mut deck = keywords_black_green_deck();
    deck.cards
        .iter_mut()
        .find(|r| r.oracle_id == "b34bb2dc-c1af-4d77-b0b3-a0fb342a5fc6")
        .unwrap()
        .count = 37;
    deck.cards
        .iter_mut()
        .find(|r| r.oracle_id == "56719f6a-1a6c-4c0a-8d21-18f7d7350b68")
        .unwrap()
        .count = 35;
    deck.cards.push(DeckEntry {
        oracle_id: "fc1256e9-eb92-43b6-abfc-ce19d910a929".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "88b50194-bc39-47a6-968b-983553d89cf9".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "fb9af562-7384-4575-93bb-34f1ada23735".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "08c7db90-c0cf-4482-b7ee-bb033e5996d2".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "9a74a1dd-c36f-4900-b835-fc39dd725bde".into(),
        count: 1,
    });
    deck.cards.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    deck
}

pub fn tokens_white_deck() -> Decklist {
    let mut deck = combat_keywords_white_deck();
    deck.cards
        .iter_mut()
        .find(|r| r.oracle_id == PLAINS)
        .unwrap()
        .count = 78;
    deck.cards.push(DeckEntry {
        oracle_id: "5b2364d7-a811-4595-a1b4-224c70555ffa".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "5e4c02c2-2e54-4e8d-8d94-6b2454027e7e".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "87e4c6ff-f83f-412b-9d23-aac7a57ef6db".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "a79e21fc-89bd-4547-8bef-5510554e1431".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "b3eb8f65-5eba-42a9-a1b4-4f37b14d03d9".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "b500c460-2814-47a4-b307-17ca519f3565".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "fab5f694-4a9c-4e72-b164-f236e8212ff7".into(),
        count: 1,
    });
    deck.cards.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    deck
}

pub fn tokens_black_green_deck() -> Decklist {
    let mut deck = combat_keywords_black_green_deck();
    deck.cards
        .iter_mut()
        .find(|r| r.oracle_id == FOREST)
        .unwrap()
        .count = 31;
    deck.cards.push(DeckEntry {
        oracle_id: "3b1f8108-6911-49e9-8f78-f950bb58cb6c".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "49f3bd5a-9c2a-45b8-aa10-9c6471028a0f".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "650bf82e-7f83-470c-bf4a-34281bfe9341".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "86c0ddaa-6926-450d-949a-6c89f43c71c2".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "8fb599a7-2c61-446b-a7a7-cd5367f9b796".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "a1112750-8d38-49e0-ab1c-77110b2bbc4d".into(),
        count: 1,
    });
    deck.cards.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    deck
}

pub fn mana_white_deck() -> Decklist {
    let mut deck = staples_white_deck();
    for row in &mut deck.cards {
        if row.count > 1 {
            row.count = 71;
        }
    }
    deck.cards.push(DeckEntry {
        oracle_id: "0895c9b7-ae7d-4bb3-af17-3b75deb50a25".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "0b67c4e2-f88b-4e01-85a1-9d5f5b8db13b".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "0bc7f093-bef0-4f1a-852c-4b75ebf54838".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "6ad8011d-3471-4369-9d68-b264cc027487".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "c97361b5-af16-4a7b-af85-a429dbaf4ad2".into(),
        count: 1,
    });
    deck.cards.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    deck
}

pub fn mana_black_green_deck() -> Decklist {
    let mut deck = staples_black_green_deck();
    for row in &mut deck.cards {
        if row.count > 1 {
            row.count = if row.oracle_id == FOREST { 24 } else { 26 };
        }
    }
    deck.cards.push(DeckEntry {
        oracle_id: "0895c9b7-ae7d-4bb3-af17-3b75deb50a25".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "0b67c4e2-f88b-4e01-85a1-9d5f5b8db13b".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "0bc7f093-bef0-4f1a-852c-4b75ebf54838".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "1cf51f50-24e4-48d0-95b3-1dad3ffa4bf5".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "3f3b2c10-21f8-4e13-be83-4ef3fa36e123".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "68954295-54e3-4303-a6bc-fc4547a4e3a3".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "6ad8011d-3471-4369-9d68-b264cc027487".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "c97361b5-af16-4a7b-af85-a429dbaf4ad2".into(),
        count: 1,
    });
    deck.cards.push(DeckEntry {
        oracle_id: "d3a0b660-358c-41bd-9cd2-41fbf3491b1a".into(),
        count: 1,
    });
    deck.cards.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    deck
}

pub fn four_colors_blue_deck() -> Decklist {
    Decklist {
        commander: "4bbc5df9-60d7-46c1-8856-91c9c74abf18".into(),
        cards: entries(&[
            ("4bbc5df9-60d7-46c1-8856-91c9c74abf18", 1), // Caelorna, Coral Tyrant
            ("cc187110-1148-4090-bbb8-e205694a39f5", 1), // Counterspell
            ("b3b00911-ece7-4484-bc36-f211ce72b6cc", 1), // Stifle
            ("837182db-1bf3-4a2c-bd01-1af9d9873561", 1), // Unsummon
            ("30cc8f7b-3c28-40f5-8f8f-157e8212280b", 1), // Time Ebb
            ("67362406-b1ca-49e2-800d-9050bfe8742a", 1), // Merfolk Looter
            ("bbf9fd47-10e1-4749-b06f-4fe72200afc7", 1), // Azure Mage
            ("5e961d15-5972-4e4b-9385-1cd7cd7c6bbe", 1), // Prodigal Sorcerer
            ("240a349a-79f8-4e11-9a05-a78148e70a62", 1), // Hapless Researcher
            ("000d5588-5a4c-434e-988d-396632ade42c", 1), // Storm Crow
            ("11f77158-0240-4882-94a4-6bd6b3d94bcc", 1), // Zephyr Sprite
            ("73115500-4468-43b3-9151-13e3da9050c5", 1), // Goliath Sphinx
            ("b2d3da40-e2f7-4480-9da9-33019d6f4071", 1), // Wall of Air
            ("a09cce6f-c957-49d7-af99-a7b2bce786e8", 1), // Hover Barrier
            ("f328012f-4696-47bb-ab43-247bdcb13f03", 1), // Dragoon's Wyvern
            ("6ad8011d-3471-4369-9d68-b264cc027487", 1), // Sol Ring
            ("0bc7f093-bef0-4f1a-852c-4b75ebf54838", 1), // Arcane Signet
            ("0b67c4e2-f88b-4e01-85a1-9d5f5b8db13b", 1), // Commander's Sphere
            ("c97361b5-af16-4a7b-af85-a429dbaf4ad2", 1), // Mind Stone
            ("0895c9b7-ae7d-4bb3-af17-3b75deb50a25", 1), // Command Tower
            ("b2c6aa39-2d2a-459c-a555-fb48ba993373", 80), // Island
        ]),
    }
}

pub fn four_colors_red_deck() -> Decklist {
    Decklist {
        commander: "48fe0f52-1676-49d5-acf6-5476081e8e1b".into(),
        cards: entries(&[
            ("48fe0f52-1676-49d5-acf6-5476081e8e1b", 1), // Rorix Bladewing
            ("4457ed35-7c10-48c8-9776-456485fdf070", 1), // Lightning Bolt
            ("68715465-6cf9-4006-87e9-31f227fe9ed3", 1), // Furnace of Rath
            ("30997b43-fc13-41d3-8064-1ccc2cb6fd2b", 1), // Raging Goblin
            ("38f66ef9-6afc-478b-a1d4-a9d42c31dfc9", 1), // Goblin Trailblazer
            ("21dd60d5-76c8-469e-8c4e-112e377d2d59", 1), // Charging Monstrosaur
            ("6fbaa853-48d4-4a3b-b22d-27415cc266d7", 1), // Quaketusk Boar
            ("8b022754-6d16-470e-b754-4df6e4f4709e", 1), // Goblin Instigator
            ("f5c5f64c-6911-430c-a825-b32b96d39c7d", 1), // Beetleback Chief
            ("f6b499d5-0e39-46ce-9de9-f64a2cc2cb32", 1), // Chimney Rabble
            ("328a0691-9453-4f2d-a9c3-5c69da252d20", 1), // Dragon Trainer
            ("6ad8011d-3471-4369-9d68-b264cc027487", 1), // Sol Ring
            ("0bc7f093-bef0-4f1a-852c-4b75ebf54838", 1), // Arcane Signet
            ("0b67c4e2-f88b-4e01-85a1-9d5f5b8db13b", 1), // Commander's Sphere
            ("c97361b5-af16-4a7b-af85-a429dbaf4ad2", 1), // Mind Stone
            ("0895c9b7-ae7d-4bb3-af17-3b75deb50a25", 1), // Command Tower
            ("a3fb7228-e76b-4e96-a40e-20b5fed75685", 84), // Mountain
        ]),
    }
}

fn add_attachments(mut deck: Decklist, additions: &[&str], basics: &[(&str, u32)]) -> Decklist {
    for (id, count) in basics {
        deck.cards
            .iter_mut()
            .find(|r| r.oracle_id == *id)
            .unwrap()
            .count = *count;
    }
    for id in additions {
        deck.cards.push(DeckEntry {
            oracle_id: (*id).into(),
            count: 1,
        });
    }
    deck.cards.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    deck
}
pub fn attachments_white_deck() -> Decklist {
    add_attachments(
        mana_white_deck(),
        &[
            "452e3f5f-ce17-4682-966b-5cc100210aee",
            "e842a4e5-ea0b-43ae-9ed8-93e30f4e7b8a",
            "12a8adc4-927f-4314-b2ef-9c647ace68d5",
            "dba35ac5-7ad3-488a-a006-6b9a1d54eea5",
            "c8b143ad-43ec-4e0d-a440-e348daa31391",
            "ca204b66-8d0c-431a-8d34-282f7c2d17da",
            "9ad4f730-a18e-4a7c-a468-a926c718c741",
            "9357de36-f8be-4f49-b2c8-9fe9eaf82b07",
            "5f5e0b10-c8cf-450c-bfd3-bcb0528ec330",
        ],
        &[(PLAINS, 62)],
    )
}
pub fn attachments_black_green_deck() -> Decklist {
    add_attachments(
        mana_black_green_deck(),
        &[
            "452e3f5f-ce17-4682-966b-5cc100210aee",
            "e842a4e5-ea0b-43ae-9ed8-93e30f4e7b8a",
            "12a8adc4-927f-4314-b2ef-9c647ace68d5",
            "dba35ac5-7ad3-488a-a006-6b9a1d54eea5",
            "c8b143ad-43ec-4e0d-a440-e348daa31391",
            "ca204b66-8d0c-431a-8d34-282f7c2d17da",
            "9ad4f730-a18e-4a7c-a468-a926c718c741",
            "090d88a9-7f2d-4bd1-a30a-7c48d05068be",
            "9d2d6479-531c-4ce1-b52b-00e36fa63b64",
            "ad73e5eb-197c-45a3-9d20-a406ba126c79",
        ],
        &[(FOREST, 20), (SWAMP, 20)],
    )
}
pub fn attachments_blue_deck() -> Decklist {
    add_attachments(
        four_colors_blue_deck(),
        &[
            "452e3f5f-ce17-4682-966b-5cc100210aee",
            "e842a4e5-ea0b-43ae-9ed8-93e30f4e7b8a",
            "12a8adc4-927f-4314-b2ef-9c647ace68d5",
            "dba35ac5-7ad3-488a-a006-6b9a1d54eea5",
            "c8b143ad-43ec-4e0d-a440-e348daa31391",
            "ca204b66-8d0c-431a-8d34-282f7c2d17da",
            "9ad4f730-a18e-4a7c-a468-a926c718c741",
        ],
        &[(ISLAND, 73)],
    )
}
pub fn attachments_red_deck() -> Decklist {
    add_attachments(
        four_colors_red_deck(),
        &[
            "452e3f5f-ce17-4682-966b-5cc100210aee",
            "e842a4e5-ea0b-43ae-9ed8-93e30f4e7b8a",
            "12a8adc4-927f-4314-b2ef-9c647ace68d5",
            "dba35ac5-7ad3-488a-a006-6b9a1d54eea5",
            "c8b143ad-43ec-4e0d-a440-e348daa31391",
            "ca204b66-8d0c-431a-8d34-282f7c2d17da",
            "9ad4f730-a18e-4a7c-a468-a926c718c741",
        ],
        &[("a3fb7228-e76b-4e96-a40e-20b5fed75685", 77)],
    )
}

pub fn removal_white_deck() -> Decklist {
    add_attachments(
        attachments_white_deck(),
        &[
            "289e1a7c-b301-4395-baf8-e9a1c6d38aab",
            "a7e97fa9-4b72-4548-b854-5be5f18a6f1a",
            "b1544f21-7e98-461b-aed5-e748b0168c52",
            "fae37e28-e137-4177-b973-fa8b4dd8f409",
        ],
        &[(PLAINS, 58)],
    )
}

pub fn removal_black_green_deck() -> Decklist {
    add_attachments(
        attachments_black_green_deck(),
        &[
            "032ec6e2-6cc3-4a97-9cc7-3233f5e11904",
            "2f092562-9e17-43cd-aeb8-d0567f99363e",
            "5825997b-10d7-4a36-972c-a80ddd90b8ed",
            "59e7f2ae-4535-4191-98be-3e65b6b2befa",
            "6d4e558e-9109-4918-a082-fdcbaffd516b",
            "7735eeba-693b-47e2-bd51-414379cf1016",
            "7b459306-149b-4f43-abc1-2dd70c748c0e",
            "9410ae41-b6db-47c6-ae0a-4e33aad1d3ce",
            "bdb3ca68-ec1f-4e16-81cc-d23f8f52c728",
        ],
        &[(FOREST, 16), (SWAMP, 15)],
    )
}

pub fn removal_blue_deck() -> Decklist {
    add_attachments(
        attachments_blue_deck(),
        &[
            "05849bd6-8f38-4031-be2b-e2aa03beb8cc",
            "8ddfc283-c9b4-41a5-af88-cf0068e986cc",
        ],
        &[(ISLAND, 71)],
    )
}

pub fn removal_red_deck() -> Decklist {
    add_attachments(
        attachments_red_deck(),
        &[
            "84d32e68-72b9-432d-a023-7bc6a26b8ad0",
            "e0583f29-c344-4b56-b0dc-bef1089a95c7",
        ],
        &[("a3fb7228-e76b-4e96-a40e-20b5fed75685", 75)],
    )
}

pub fn sacrifice_white_deck() -> Decklist {
    add_attachments(
        removal_white_deck(),
        &["59f10ddb-8287-4d2c-8c60-1080b108fa78"],
        &[(PLAINS, 57)],
    )
}

pub fn sacrifice_black_green_deck() -> Decklist {
    add_attachments(
        removal_black_green_deck(),
        &[
            "0c936582-d4c0-4da8-bc7e-49da5a433939",
            "11ad664e-36d3-4d5b-8a87-59ea715877e0",
            "11cb509d-21af-48f1-b355-135ebd3e4bd1",
            "365548fb-5acc-4a8a-b20b-26d28b7d029f",
            "76b003e0-15af-4f22-bdf2-1ade5430964a",
            "8cdfd9df-4783-4138-a286-828a97f516aa",
            "a1cc5e37-b09a-4b7f-afd5-77c1c35aa425",
            "a85197ab-dc94-4b72-9716-8dbdbbe90ff8",
            "aa76cc50-5be0-4844-b3a4-fb33fca3cdd3",
            "c7f33cea-2ec8-4081-9208-a5b1d86721b3",
            "e3afc704-220f-498f-9eaa-0821b17dc24c",
            "e8c7566d-7cc0-48af-a986-83223ec7e06c",
        ],
        &[(FOREST, 9), (SWAMP, 10)],
    )
}

pub fn sacrifice_red_deck() -> Decklist {
    add_attachments(
        removal_red_deck(),
        &[
            "24227761-b50e-4b9e-93a2-e82d053b3e3d",
            "7efd5d62-6da2-428a-98e3-5b842f668656",
            "c1177f22-a1cf-4da3-a68d-ff954e878403",
            "ddc7f59a-bbb1-4ba1-82c8-6813fd191940",
            "edad60c6-80de-4033-af1b-a703ac332983",
        ],
        &[("a3fb7228-e76b-4e96-a40e-20b5fed75685", 70)],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn catalog() -> Catalog {
        Catalog::read(include_str!(
            "../../../../tests/fixtures/expected-match-ir.json"
        ))
        .unwrap()
    }
    #[test]
    fn default_pod_is_canonical_four_legal_hundred_card_decks() {
        let c = catalog();
        let input = MatchConfig::foundations([7; 32], PlayerId(3));
        let pod = validate_pod(&c, &input).unwrap();
        assert_eq!(pod.config.cards.len(), 400);
        assert_eq!(
            pod.config.commanders,
            vec![CardId(0), CardId(100), CardId(200), CardId(300)]
        );
        assert_eq!(pod.config.mode, Mode::CommanderMatch);
        for seat in 0..4 {
            let owner = PlayerId(seat);
            let own = pod
                .config
                .cards
                .iter()
                .filter(|c| c.owner == owner)
                .collect::<Vec<_>>();
            assert_eq!(own.len(), 100);
            assert_eq!(
                own.iter()
                    .filter(|c| c.zone == Zone::Command { owner })
                    .count(),
                1
            );
            assert_eq!(
                own.iter()
                    .filter(|c| c.zone == Zone::Library { owner })
                    .count(),
                99
            );
            assert!(own.iter().all(|c| c.controller == owner));
        }
        let mut reordered = input.clone();
        for deck in &mut reordered.decks {
            deck.cards.reverse();
            let i = deck.cards.iter().position(|r| r.count > 1).unwrap();
            let id = deck.cards[i].oracle_id.clone();
            deck.cards[i].count -= 1;
            deck.cards.push(DeckEntry {
                oracle_id: id,
                count: 1,
            });
        }
        let equivalent = validate_pod(&c, &reordered).unwrap();
        assert_eq!(pod.match_config, equivalent.match_config);
        assert_eq!(pod.config, equivalent.config);
        assert_eq!(
            pod.identity_digest().unwrap(),
            equivalent.identity_digest().unwrap()
        );
        let mut changed = input.clone();
        changed.seed[0] ^= 1;
        assert_ne!(
            pod.identity_digest().unwrap(),
            validate_pod(&c, &changed)
                .unwrap()
                .identity_digest()
                .unwrap()
        );
        changed = input.clone();
        changed.starting_player = PlayerId(2);
        assert_ne!(
            pod.identity_digest().unwrap(),
            validate_pod(&c, &changed)
                .unwrap()
                .identity_digest()
                .unwrap()
        );
        for all in [white_deck(), black_green_deck()] {
            let mut x = input.clone();
            x.decks = std::array::from_fn(|_| all.clone());
            assert!(validate_pod(&c, &x).is_ok());
        }
    }
    #[test]
    fn construction_rejects_bad_counts_singletons_colors_and_unsupported_cards() {
        let c = catalog();
        let input = MatchConfig::foundations([0; 32], PlayerId(0));
        let mut x = input.clone();
        x.decks[0].cards[0].count = 0;
        assert!(matches!(
            validate_pod(&c, &x),
            Err(MatchAdmissionError::InvalidCount { .. })
        ));
        x = input.clone();
        x.decks[0].cards[0].count = u32::MAX;
        assert!(matches!(
            validate_pod(&c, &x),
            Err(MatchAdmissionError::InvalidDeckSize { .. })
        ));
        x = input.clone();
        x.decks[0]
            .cards
            .iter_mut()
            .find(|r| r.oracle_id == PLAINS)
            .unwrap()
            .count -= 1;
        assert!(matches!(
            validate_pod(&c, &x),
            Err(MatchAdmissionError::InvalidDeckSize { total: 99, .. })
        ));
        x = input.clone();
        x.decks[0]
            .cards
            .iter_mut()
            .find(|r| r.oracle_id == PLAINS)
            .unwrap()
            .count -= 1;
        x.decks[0].cards.push(DeckEntry {
            oracle_id: OX.into(),
            count: 1,
        });
        assert!(matches!(
            validate_pod(&c, &x),
            Err(MatchAdmissionError::SingletonName { .. })
        ));
        x = input.clone();
        x.decks[0]
            .cards
            .iter_mut()
            .find(|r| r.oracle_id == OX)
            .unwrap()
            .oracle_id = FOREST.into();
        assert!(matches!(
            validate_pod(&c, &x),
            Err(MatchAdmissionError::ColorIdentity { .. })
        ));
        x = input.clone();
        x.decks[0]
            .cards
            .iter_mut()
            .find(|r| r.oracle_id == OX)
            .unwrap()
            .oracle_id = "unknown-oracle".into();
        assert!(matches!(
            validate_pod(&c, &x),
            Err(MatchAdmissionError::UnsupportedCard { .. })
        ));
        x = input.clone();
        x.decks[0].commander = BEARS.into();
        assert!(matches!(
            validate_pod(&c, &x),
            Err(MatchAdmissionError::UnsupportedCommander { .. })
        ));
        x = input.clone();
        x.decks[0]
            .cards
            .iter_mut()
            .find(|r| r.oracle_id == ISAMARU)
            .unwrap()
            .oracle_id = PLAINS.into();
        assert!(matches!(
            validate_pod(&c, &x),
            Err(MatchAdmissionError::CommanderCount { count: 0, .. })
        ));
        x = input.clone();
        x.decks[0]
            .cards
            .iter_mut()
            .find(|r| r.oracle_id == PLAINS)
            .unwrap()
            .count -= 1;
        x.decks[0].cards.push(DeckEntry {
            oracle_id: ISAMARU.into(),
            count: 1,
        });
        assert!(matches!(
            validate_pod(&c, &x),
            Err(MatchAdmissionError::CommanderCount { count: 2, .. })
        ));
        x = input.clone();
        x.profile = "unreviewed".into();
        assert!(matches!(
            validate_pod(&c, &x),
            Err(MatchAdmissionError::UnsupportedProfile)
        ));
        x = input.clone();
        x.starting_player = PlayerId(4);
        assert!(matches!(
            validate_pod(&c, &x),
            Err(MatchAdmissionError::InvalidStartingPlayer)
        ));
    }
    #[test]
    fn basic_land_mana_restriction_is_independent_of_color_identity_data() {
        let c = catalog();
        let white = &c
            .definitions
            .iter()
            .find(|d| d.oracle_id == ISAMARU)
            .unwrap()
            .characteristics;
        let mut synthetic = c
            .definitions
            .iter()
            .find(|d| d.oracle_id == FOREST)
            .unwrap()
            .characteristics
            .clone();
        synthetic.color_identity.clear();
        assert!(matches!(
            validate_deck_colors(PlayerId(0), "private-domain-fixture", &synthetic, white),
            Err(MatchAdmissionError::BasicLandColor { .. })
        ));
        synthetic.basic = false;
        assert!(matches!(
            validate_deck_colors(PlayerId(0), "nonbasic-domain-fixture", &synthetic, white),
            Err(MatchAdmissionError::BasicLandColor { .. })
        ));
    }
    #[test]
    fn known_bans_are_distinct_from_unknown_or_unsupported_pool_and_policy_is_bound() {
        let c = catalog();
        let input = MatchConfig::foundations([1; 32], PlayerId(0));
        let policy: serde_json::Value =
            serde_json::from_str(super::super::commander_policy::COMMANDER_POLICY_JSON).unwrap();
        let named = policy["named_bans"].as_array().unwrap();
        let offensive = policy["offensive_policy"]["records"].as_array().unwrap();
        for ban in named.iter().chain(offensive) {
            let id = ban["oracle_id"].as_str().unwrap();
            let mut x = input.clone();
            x.decks[0]
                .cards
                .iter_mut()
                .find(|r| r.oracle_id == OX)
                .unwrap()
                .oracle_id = id.into();
            assert!(
                matches!(
                    validate_pod(&c, &x),
                    Err(MatchAdmissionError::BannedCard { .. })
                ),
                "{id}"
            );
        }
        let mut invalid = c.clone();
        invalid.definitions[0].name.push_str(" changed");
        assert!(matches!(
            validate_pod(&invalid, &input),
            Err(MatchAdmissionError::InvalidCatalog { .. })
        ));
        let mut raw = serde_json::to_value(input).unwrap();
        raw["decks"][0]["sideboard"] = serde_json::json!([]);
        assert!(strict_read::<MatchConfig>(&canonical(&raw).unwrap()).is_err());
        raw["decks"][0].as_object_mut().unwrap().remove("sideboard");
        raw["seed"][0] = (-1).into();
        assert!(strict_read::<MatchConfig>(&canonical(&raw).unwrap()).is_err());
    }
}
