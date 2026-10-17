//! Finite legal-deck construction, separate from injected scenario admission.
//! CR903.3–.7: https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use std::fmt;

pub const MATCH_PROFILE: &str = "commander-foundations-v1";
pub const MATCH_CATALOG_SHA256: &str =
    "fc4bd3390e63ea352e129ac370fa848dbee11cf0700e7e6781d449fd49244a7f";
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
        Self {
            profile: MATCH_PROFILE.into(),
            decks: [
                white_deck(),
                black_green_deck(),
                white_deck(),
                black_green_deck(),
            ],
            starting_player,
            seed,
        }
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
        digest(&(
            MATCH_PROFILE,
            MATCH_CATALOG_SHA256,
            super::commander_policy::policy_digest(),
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
    if config.profile != MATCH_PROFILE {
        return Err(MatchAdmissionError::UnsupportedProfile);
    }
    if config.starting_player.0 >= 4 {
        return Err(MatchAdmissionError::InvalidStartingPlayer);
    }
    catalog
        .validate()
        .map_err(|detail| MatchAdmissionError::InvalidCatalog { detail })?;
    if catalog.schema_version != 11
        || digest(catalog).map_err(|detail| MatchAdmissionError::InvalidCatalog { detail })?
            != MATCH_CATALOG_SHA256
    {
        return Err(MatchAdmissionError::InvalidCatalog {
            detail: "catalog differs from reviewed thirty-four-definition pool".into(),
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
        if !matches!(deck.commander.as_str(), ISAMARU | YARGLE) {
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
            profile: MATCH_PROFILE.into(),
            commanders,
            cards,
        },
        match_config: normalized,
    })
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
