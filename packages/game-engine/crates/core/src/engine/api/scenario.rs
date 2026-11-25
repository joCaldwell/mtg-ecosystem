//! Scenario and pod admission: the configuration a game is prepared from.
use crate::engine::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardSetup {
    pub id: CardId,
    pub oracle_id: String,
    pub owner: PlayerId,
    pub controller: PlayerId,
    pub zone: Zone,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Scenario,
    CommanderMatch,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioConfig {
    pub mode: Mode,
    pub players: u8,
    pub profile: String,
    pub commanders: Vec<CardId>,
    pub cards: Vec<CardSetup>,
    pub initial: ScenarioInitial,
}
/// Scenario inputs; these facts do not enable or disable rules.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioInitial {
    pub life: [Quantity; 4],
    pub turn_step: TurnStep,
    pub libraries: Option<BTreeMap<PlayerId, Vec<CardId>>>,
    /// The seed of a private board's library shuffles (CR 701.24a). A board
    /// without one has no randomness, and a shuffle there faults.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<[u8; 32]>,
}
impl Default for ScenarioInitial {
    fn default() -> Self {
        Self {
            life: std::array::from_fn(|_| Quantity::from(40)),
            turn_step: TurnStep::PostcombatMain,
            libraries: None,
            seed: None,
        }
    }
}
impl ScenarioConfig {
    pub fn turns_permanents() -> Self {
        Self {
            initial: ScenarioInitial::default(),
            mode: Mode::Scenario,
            players: 4,
            profile: "four-seat-turns-permanents-v1".into(),
            commanders: vec![],
            cards: (0..8)
                .map(|id| {
                    let owner = PlayerId(if id < 5 { 0 } else { (id - 4) as u8 });
                    CardSetup {
                        id: CardId(id),
                        oracle_id: if id == 2 { BEARS } else { FOREST }.into(),
                        owner,
                        controller: owner,
                        zone: match id {
                            0 => Zone::Battlefield,
                            1..=3 => Zone::Hand { owner },
                            _ => Zone::Library { owner },
                        },
                    }
                })
                .collect(),
        }
    }

    pub fn casting_reversal() -> Self {
        let mut config = Self::land_return();
        config.profile = "casting-mana-reversal-v1".into();
        config.cards[0].oracle_id = FOREST.into();
        config.cards.push(CardSetup {
            id: CardId(3),
            oracle_id: FOREST.into(),
            owner: PlayerId(0),
            controller: PlayerId(0),
            zone: Zone::Battlefield,
        });
        config
    }
    pub fn stack_response() -> Self {
        let mut config = Self::land_return();
        config.profile = "four-seat-instant-response-v1".into();
        for player in 1..4 {
            let land = CardId(2 * u32::from(player) + 1);
            let spell = CardId(land.0 + 1);
            config.cards.push(CardSetup {
                id: land,
                oracle_id: ISLAND.into(),
                owner: PlayerId(player),
                controller: PlayerId(player),
                zone: Zone::Battlefield,
            });
            config.cards.push(CardSetup {
                id: spell,
                oracle_id: SPELL.into(),
                owner: PlayerId(player),
                controller: PlayerId(player),
                zone: Zone::Hand {
                    owner: PlayerId(player),
                },
            });
        }
        config
    }

    pub fn commander_foundation() -> Self {
        let mut config = Self::land_return();
        config.profile = "commander-foundation-scenario-v1".into();
        config.commanders.push(CardId(3));
        config.cards.push(CardSetup {
            id: CardId(3),
            oracle_id: ISAMARU.into(),
            owner: PlayerId(0),
            controller: PlayerId(0),
            zone: Zone::Command { owner: PlayerId(0) },
        });
        config
    }
    pub fn land_return() -> Self {
        Self {
            initial: ScenarioInitial::default(),
            mode: Mode::Scenario,
            players: 4,
            profile: SCENARIO_PROFILE.into(),
            commanders: vec![],
            cards: vec![
                CardSetup {
                    id: CardId(0),
                    oracle_id: ISLAND.into(),
                    owner: PlayerId(0),
                    controller: PlayerId(0),
                    zone: Zone::Battlefield,
                },
                CardSetup {
                    id: CardId(1),
                    oracle_id: SPELL.into(),
                    owner: PlayerId(0),
                    controller: PlayerId(0),
                    zone: Zone::Hand { owner: PlayerId(0) },
                },
                CardSetup {
                    id: CardId(2),
                    oracle_id: BEARS.into(),
                    owner: PlayerId(1),
                    controller: PlayerId(2),
                    zone: Zone::Battlefield,
                },
            ],
        }
    }
}
testkit_fields! {
    #[derive(Clone)]
    pub struct Prepared {
        pub(in crate::engine) match_config: Option<MatchConfig>,
        pub(in crate::engine) catalog: Arc<Catalog>,
        pub(in crate::engine) catalog_digest: String,
        pub(in crate::engine) config: ScenarioConfig,
        pub(in crate::engine) profile_digest: String,
    }
}
impl Prepared {
    pub fn admit_pod(catalog: Catalog, config: MatchConfig) -> Result<Self, MatchAdmissionError> {
        let pod = validate_pod(&catalog, &config)?;
        let profile_digest = pod
            .identity_digest()
            .map_err(|detail| MatchAdmissionError::InvalidCatalog { detail })?;
        let catalog_digest =
            digest(&catalog).map_err(|detail| MatchAdmissionError::InvalidCatalog { detail })?;
        Ok(Self {
            catalog: Arc::new(catalog),
            catalog_digest,
            config: pod.config,
            profile_digest,
            match_config: Some(pod.match_config),
        })
    }
    pub fn match_config(&self) -> Option<&MatchConfig> {
        self.match_config.as_ref().filter(|config| {
            config.profile != DYNAMIC_PROFILE
                || self.config.commanders.len() == self.config.players as usize
        })
    }
    pub fn admit(catalog: Catalog, config: ScenarioConfig) -> Result<Self, String> {
        catalog.validate()?;
        if catalog.definitions.iter().any(|definition| {
            definition
                .capabilities
                .contains(&Capability::RandomPlayerDiscardV1)
        }) {
            return Err("random discard requires a seeded match".into());
        }
        // Admission-only exact fixture/pin table. It never selects runtime rules.
        let allowed = [
            (
                vec![
                    ScenarioConfig::land_return(),
                    ScenarioConfig::commander_foundation(),
                    ScenarioConfig::stack_response(),
                    ScenarioConfig::casting_reversal(),
                    ScenarioConfig::turns_permanents(),
                    ScenarioConfig::combat_game(),
                    ScenarioConfig::combat_interactions(),
                ],
                [ALLOWED.as_slice()].concat(),
            ),
            (
                vec![
                    ScenarioConfig::command_casting(),
                    ScenarioConfig::commander_lifecycle(),
                    ScenarioConfig::commander_exile(),
                    ScenarioConfig::commander_damage(),
                    ScenarioConfig::commander_damage_borrowed(),
                    ScenarioConfig::commander_library(),
                    ScenarioConfig::actions(),
                    ScenarioConfig::actions_borrowed(),
                    ScenarioConfig::actions_empty_library(),
                    ScenarioConfig::mixed_mana(),
                ],
                [
                    ALLOWED.as_slice(),
                    &[(PLAINS, PLAINS_DIGEST)],
                    &[(YARGLE, YARGLE_DIGEST)],
                    &[(TIME_EBB, TIME_EBB_DIGEST)],
                    ACTION_EXTRA.as_slice(),
                ]
                .concat(),
            ),
        ]
        .into_iter()
        .find_map(|(candidate, allowed)| {
            candidate
                .iter()
                .any(|candidate| candidate == &config)
                .then_some(allowed)
        })
        .ok_or("configuration is outside the reviewed four-seat scenario; no match admission")?;
        if catalog.definitions.len() != allowed.len() {
            return Err("definition pool is not the closed reviewed scenario pool".into());
        }
        for d in &catalog.definitions {
            let expected = allowed
                .iter()
                .find(|(id, _)| *id == d.oracle_id)
                .ok_or("definition is not admitted")?
                .1;
            if digest(d)? != expected {
                return Err("definition digest is not admitted by this build/profile".into());
            }
        }
        let catalog_digest = digest(&catalog)?;
        let profile_digest = digest(&(SCENARIO_PROFILE, &allowed, &config))?;
        Ok(Self {
            match_config: None,
            catalog: Arc::new(catalog),
            catalog_digest,
            config,
            profile_digest,
        })
    }
    pub fn catalog_digest(&self) -> &str {
        &self.catalog_digest
    }
    pub fn profile_digest(&self) -> &str {
        &self.profile_digest
    }
    pub fn config(&self) -> &ScenarioConfig {
        &self.config
    }
}
