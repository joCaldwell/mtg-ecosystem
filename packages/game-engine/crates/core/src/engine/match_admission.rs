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
/// All reviewed legal-profile identity and construction inputs live here.
pub struct MatchProfile {
    pub name: &'static str,
    pub catalog_sha256: &'static str,
    pub ir_version: u32,
    pub policy_json: &'static str,
    pub commanders: &'static [&'static str],
    pub white_deck: fn() -> Decklist,
    pub black_green_deck: fn() -> Decklist,
}
pub static MATCH_PROFILES: [MatchProfile; 3] = [
    MatchProfile {
        name: MATCH_PROFILE,
        catalog_sha256: MATCH_CATALOG_SHA256,
        ir_version: 11,
        policy_json: super::commander_policy::COMMANDER_POLICY_JSON,
        commanders: &[ISAMARU, YARGLE],
        white_deck,
        black_green_deck,
    },
    MatchProfile {
        name: KEYWORDS_PROFILE,
        catalog_sha256: KEYWORDS_CATALOG_SHA256,
        ir_version: 12,
        policy_json: include_str!("../../../../tests/fixtures/commander-keywords-policy.json"),
        commanders: &[ISAMARU, YARGLE],
        white_deck: keywords_white_deck,
        black_green_deck: keywords_black_green_deck,
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
    },
];
pub fn match_profile(name: &str) -> Option<&'static MatchProfile> {
    MATCH_PROFILES.iter().find(|p| p.name == name)
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
            decks: [
                (p.white_deck)(),
                (p.black_green_deck)(),
                (p.white_deck)(),
                (p.black_green_deck)(),
            ],
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
            detail: if profile.name == MATCH_PROFILE {
                "catalog differs from reviewed thirty-four-definition pool"
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
