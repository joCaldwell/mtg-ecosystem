//! Finite legal-deck construction, separate from injected scenario admission.
//! CR903.3–.7: https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use std::fmt;
include!(concat!(env!("OUT_DIR"), "/catalog_sizes.rs"));

pub use crate::profile_data::*;
include!(concat!(env!("OUT_DIR"), "/match_profiles.rs"));
pub fn match_profile(name: &str) -> Option<&'static MatchProfile> {
    if name == super::dynamic_admission::DYNAMIC_PROFILE {
        return None;
    }
    MATCH_PROFILES.iter().find(|p| p.name == name)
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
    pub fn for_profile(
        name: &str,
        seed: [u8; 32],
        starting_player: PlayerId,
    ) -> Result<Self, MatchAdmissionError> {
        let p = match_profile(name).ok_or(MatchAdmissionError::UnsupportedProfile)?;
        Ok(Self {
            profile: p.name.clone(),
            decks: p.decks.clone(),
            starting_player,
            seed,
        })
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

testkit_fields! {
    /// Construction is private; successful validation is the only way to produce
    /// this immutable initialization description. It is not deserialized from a client.
    #[derive(Clone, Debug)]
    pub struct ValidatedPod {
        pub(super) config: ScenarioConfig,
        pub(super) match_config: MatchConfig,
    }
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
            &p.name,
            &p.catalog_sha256,
            p,
            p.policy_digest(),
            &self.match_config,
        ))
    }
}
fn known_ban(
    profile: &MatchProfile,
    player: PlayerId,
    id: &str,
) -> Result<(), MatchAdmissionError> {
    if let Some(name) = profile.banned_commander_card(id) {
        return Err(MatchAdmissionError::BannedCard {
            player,
            oracle_id: id.into(),
            name: name.into(),
        });
    }
    Ok(())
}
pub(super) fn commander_eligible(c: &Characteristics, eligibility: CommanderEligibility) -> bool {
    match eligibility {
        CommanderEligibility::LegendaryCreature => c.legendary && c.kind == CardKind::Creature,
    }
}

pub(super) fn deck_color_violations(
    card: &Characteristics,
    commander_colors: &[Color],
) -> (bool, bool) {
    let outside_identity = card
        .color_identity
        .iter()
        .any(|c| !commander_colors.contains(c));
    // CR 903.5d applies independently, including to nonbasic lands.
    let outside_land_colors = card
        .subtypes
        .iter()
        .filter_map(|s| basic_land_color(s))
        .any(|c| !commander_colors.contains(&c));
    (outside_identity, outside_land_colors)
}

fn validate_deck_colors(
    player: PlayerId,
    id: &str,
    card: &Characteristics,
    commander: &Characteristics,
) -> Result<(), MatchAdmissionError> {
    let (outside_identity, outside_land_colors) =
        deck_color_violations(card, &commander.color_identity);
    if outside_identity {
        return Err(MatchAdmissionError::ColorIdentity {
            player,
            oracle_id: id.into(),
        });
    }
    if outside_land_colors {
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
    if digest(catalog).map_err(|detail| MatchAdmissionError::InvalidCatalog { detail })?
        != profile.catalog_sha256
    {
        return Err(MatchAdmissionError::InvalidCatalog {
            detail: format!(
                "catalog differs from reviewed {} pool: expected {} definitions, received {}",
                profile.name,
                reviewed_catalog_size(&profile.catalog_sha256).ok_or_else(|| {
                    MatchAdmissionError::InvalidCatalog {
                        detail: "missing reviewed catalog metadata".into(),
                    }
                })?,
                catalog.definitions.len(),
            ),
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
        known_ban(profile, player, &deck.commander)?;
        if !profile.commanders.contains(&deck.commander) {
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
        if !commander_eligible(
            &commander.characteristics,
            profile.commander_policy.eligibility,
        ) {
            return Err(MatchAdmissionError::UnsupportedCommander {
                player,
                oracle_id: deck.commander.clone(),
            });
        }
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
            known_ban(profile, player, &row.oracle_id)?;
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
            match profile.commander_policy.color_identity {
                ColorIdentityPolicy::CommanderSubset => validate_deck_colors(
                    player,
                    id,
                    &d.characteristics,
                    &commander.characteristics,
                )?,
            }
        }
        normalized.decks[seat].cards = counts
            .into_iter()
            .map(|(oracle_id, count)| DeckEntry { oracle_id, count })
            .collect();
    }
    Ok(ValidatedPod {
        config: physical_match_config(&profile.name, &normalized.decks),
        match_config: normalized,
    })
}

pub(super) fn physical_match_config(profile: &str, decks: &[Decklist; 4]) -> ScenarioConfig {
    physical_match_rosters(
        profile,
        decks
            .each_ref()
            .map(|deck| (std::slice::from_ref(&deck.commander), deck.cards.as_slice())),
    )
}

pub(super) fn physical_match_rosters(
    profile: &str,
    decks: [(&[String], &[DeckEntry]); 4],
) -> ScenarioConfig {
    let mut cards = Vec::with_capacity(400);
    let mut commanders = Vec::with_capacity(8);
    for (seat, (designations, entries)) in decks.into_iter().enumerate() {
        let owner = PlayerId(seat as u8);
        let mut next = seat as u32 * 100;
        for oracle_id in designations {
            commanders.push(CardId(next));
            cards.push(CardSetup {
                id: CardId(next),
                oracle_id: oracle_id.clone(),
                owner,
                controller: owner,
                zone: Zone::Command { owner },
            });
            next += 1;
        }
        for row in entries {
            if designations.contains(&row.oracle_id) {
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
    ScenarioConfig {
        initial: ScenarioInitial {
            turn_step: TurnStep::Untap,
            ..ScenarioInitial::default()
        },
        mode: Mode::CommanderMatch,
        players: 4,
        profile: profile.into(),
        commanders,
        cards,
    }
}

impl std::error::Error for MatchAdmissionError {}

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
        let input =
            MatchConfig::for_profile("commander-foundations-v1", [7; 32], PlayerId(3)).unwrap();
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
        for all in [
            match_profile("commander-foundations-v1").unwrap().decks[0].clone(),
            match_profile("commander-foundations-v1").unwrap().decks[1].clone(),
        ] {
            let mut x = input.clone();
            x.decks = std::array::from_fn(|_| all.clone());
            assert!(validate_pod(&c, &x).is_ok());
        }
    }
    #[test]
    fn construction_rejects_bad_counts_singletons_colors_and_unsupported_cards() {
        let c = catalog();
        let input =
            MatchConfig::for_profile("commander-foundations-v1", [0; 32], PlayerId(0)).unwrap();
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
    /// Ported from the retired Node contract: the match catalog binds Wall of
    /// Wood's sole Defender keyword, and the profile-selected dated ban policy
    /// reviews every pooled definition against its retained source.
    #[test]
    fn match_catalog_binds_sole_defender_and_the_current_commander_policy() {
        let c = catalog();
        let wall = c
            .definitions
            .iter()
            .find(|d| d.name == "Wall of Wood")
            .unwrap();
        assert_eq!(wall.keywords, vec![KeywordAbility::Defender]);
        assert!(wall.capabilities.contains(&Capability::DefenderV1));
        let value = serde_json::to_value(wall).unwrap();
        assert_eq!(
            value["characteristics"]["mana_cost"],
            serde_json::json!({"generic": "0", "colored": ["G"]})
        );
        assert_eq!(value["characteristics"]["power"], "0");
        assert_eq!(value["characteristics"]["toughness"], "3");
        assert_eq!(wall.characteristics.subtypes, vec!["Wall".to_owned()]);
        let profile = match_profile("commander-foundations-v1").unwrap();
        assert_eq!(profile.catalog(), &c);
        let policy: serde_json::Value = serde_json::from_str(&profile.banned_list_raw).unwrap();
        assert_eq!(policy["policy_version"], 1);
        assert_eq!(policy["categories"]["lutri"], "companion_only");
        assert_eq!(policy["html_sha256"].as_str().unwrap().len(), 64);
        assert_eq!(
            policy["offensive_policy"]["html_sha256"]
                .as_str()
                .unwrap()
                .len(),
            64
        );
        let offensive_names = policy["offensive_policy"]["names"].as_array().unwrap();
        assert!(!profile.bans.is_empty());
        for ban in &profile.bans {
            let record: serde_json::Value = serde_json::from_str(&ban.record_json).unwrap();
            assert_eq!(record["sourceSha256"], ban.source_sha256);
            let source = &record["source"];
            assert!(
                source["name"] == ban.name
                    || source["card_faces"]
                        .as_array()
                        .is_some_and(|faces| faces.iter().any(|f| f["name"] == ban.name))
            );
        }
        for d in &c.definitions {
            let source: serde_json::Value =
                serde_json::from_str(&d.provenance.source_json).unwrap();
            assert_eq!(source["legalities"]["commander"], "legal", "{}", d.name);
            assert!(!source["type_line"].as_str().unwrap().contains("Conspiracy"));
            let text = source["oracle_text"].as_str().unwrap_or("").to_lowercase();
            assert!(
                !text
                    .split(|c: char| !c.is_alphanumeric())
                    .any(|w| w == "ante")
            );
            assert!(profile.banned_commander_card(&d.oracle_id).is_none());
            assert!(!offensive_names.iter().any(|n| source["name"] == *n));
            assert_eq!(
                mtg_contracts::sha256(d.provenance.source_json.as_bytes()),
                d.provenance.source_sha256
            );
        }
    }
    #[test]
    fn known_bans_are_distinct_from_unknown_or_unsupported_pool_and_policy_is_bound() {
        let c = catalog();
        let input =
            MatchConfig::for_profile("commander-foundations-v1", [1; 32], PlayerId(0)).unwrap();
        let policy: serde_json::Value =
            serde_json::from_str(&match_profile(&input.profile).unwrap().banned_list_raw).unwrap();
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
