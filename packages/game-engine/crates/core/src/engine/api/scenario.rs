//! Scenario and pod admission: the configuration a game is prepared from.
use crate::engine::*;

// Oracle identities and definition digests of the frozen scenario catalogs.
// They are admission data for `Prepared::admit`: a scenario names its cards by
// exact identity and digest. No rule reads them.
pub const GROWTH: &str = "5748ebf1-24e3-499d-ab7c-c2cebd462a24";
pub const DISFIGURE: &str = "77eafe49-b9c5-461d-89c5-cec217dd2974";
pub const FOG: &str = "27e9db49-7af7-4bef-ad4c-bf5dfb92030d";
pub const HEALER: &str = "95a0ca48-d924-47f4-86ed-42c673ee778c";
pub const FURNACE: &str = "68715465-6cf9-4006-87e9-31f227fe9ed3";
pub const OX: &str = "19b4cf0b-e1e4-4e50-80bc-673bc6ef8bb8";
pub const EFFECT_EXTRA: [(&str, &str); 6] = [
    (
        GROWTH,
        definition_digest!("MTG_DEFINITION_effects_5748ebf1-24e3-499d-ab7c-c2cebd462a24"),
    ),
    (
        DISFIGURE,
        definition_digest!("MTG_DEFINITION_effects_77eafe49-b9c5-461d-89c5-cec217dd2974"),
    ),
    (
        FOG,
        definition_digest!("MTG_DEFINITION_effects_27e9db49-7af7-4bef-ad4c-bf5dfb92030d"),
    ),
    (
        HEALER,
        definition_digest!("MTG_DEFINITION_effects_95a0ca48-d924-47f4-86ed-42c673ee778c"),
    ),
    (
        FURNACE,
        definition_digest!("MTG_DEFINITION_effects_68715465-6cf9-4006-87e9-31f227fe9ed3"),
    ),
    (
        OX,
        definition_digest!("MTG_DEFINITION_effects_19b4cf0b-e1e4-4e50-80bc-673bc6ef8bb8"),
    ),
];
pub const ISLAND: &str = "b2c6aa39-2d2a-459c-a555-fb48ba993373";
pub const FOREST: &str = "b34bb2dc-c1af-4d77-b0b3-a0fb342a5fc6";
pub const SPELL: &str = "837182db-1bf3-4a2c-bd01-1af9d9873561";
pub const BEARS: &str = "14c8f55d-d177-4c25-a931-ebeb9e6062a0";
pub const PLAINS: &str = "bc71ebf6-2056-41f7-be35-b2e5c34afa99";
pub const PLAINS_DIGEST: &str =
    definition_digest!("MTG_DEFINITION_commander_bc71ebf6-2056-41f7-be35-b2e5c34afa99");
pub const TIME_EBB: &str = "30cc8f7b-3c28-40f5-8f8f-157e8212280b";
pub const TIME_EBB_DIGEST: &str =
    definition_digest!("MTG_DEFINITION_library_30cc8f7b-3c28-40f5-8f8f-157e8212280b");
pub const YARGLE: &str = "980e1721-3717-4425-95a4-938dbf2ac661";
pub const YARGLE_DIGEST: &str =
    definition_digest!("MTG_DEFINITION_damage_980e1721-3717-4425-95a4-938dbf2ac661");
pub const EDICT: &str = "10c585c4-bf5b-4d8f-94a9-e9a5036a688f";
pub const HAPLESS: &str = "240a349a-79f8-4e11-9a05-a78148e70a62";
pub const SWAMP: &str = "56719f6a-1a6c-4c0a-8d21-18f7d7350b68";
pub const ACTION_EXTRA: [(&str, &str); 3] = [
    (
        EDICT,
        definition_digest!("MTG_DEFINITION_action_10c585c4-bf5b-4d8f-94a9-e9a5036a688f"),
    ),
    (
        HAPLESS,
        definition_digest!("MTG_DEFINITION_action_240a349a-79f8-4e11-9a05-a78148e70a62"),
    ),
    (
        SWAMP,
        definition_digest!("MTG_DEFINITION_action_56719f6a-1a6c-4c0a-8d21-18f7d7350b68"),
    ),
];
pub const LOOTER: &str = "67362406-b1ca-49e2-800d-9050bfe8742a";
pub const STIFLE: &str = "b3b00911-ece7-4484-bc36-f211ce72b6cc";
pub const AZURE: &str = "bbf9fd47-10e1-4749-b06f-4fe72200afc7";
pub const COUNTERSPELL: &str = "cc187110-1148-4090-bbb8-e205694a39f5";
pub const STACK_EXTRA: [(&str, &str); 4] = [
    (
        LOOTER,
        definition_digest!("MTG_DEFINITION_stack_67362406-b1ca-49e2-800d-9050bfe8742a"),
    ),
    (
        STIFLE,
        definition_digest!("MTG_DEFINITION_stack_b3b00911-ece7-4484-bc36-f211ce72b6cc"),
    ),
    (
        AZURE,
        definition_digest!("MTG_DEFINITION_stack_bbf9fd47-10e1-4749-b06f-4fe72200afc7"),
    ),
    (
        COUNTERSPELL,
        definition_digest!("MTG_DEFINITION_stack_cc187110-1148-4090-bbb8-e205694a39f5"),
    ),
];
pub const ISAMARU: &str = "cfb756dd-6fe1-430a-8f04-8b10cac62a86";
pub const ALLOWED: [(&str, &str); 5] = [
    (
        ISAMARU,
        definition_digest!("MTG_DEFINITION_base_cfb756dd-6fe1-430a-8f04-8b10cac62a86"),
    ),
    (
        ISLAND,
        definition_digest!("MTG_DEFINITION_base_b2c6aa39-2d2a-459c-a555-fb48ba993373"),
    ),
    (
        FOREST,
        definition_digest!("MTG_DEFINITION_base_b34bb2dc-c1af-4d77-b0b3-a0fb342a5fc6"),
    ),
    (
        SPELL,
        definition_digest!("MTG_DEFINITION_base_837182db-1bf3-4a2c-bd01-1af9d9873561"),
    ),
    (
        BEARS,
        definition_digest!("MTG_DEFINITION_base_14c8f55d-d177-4c25-a931-ebeb9e6062a0"),
    ),
];
pub const BOLT: &str = "4457ed35-7c10-48c8-9776-456485fdf070";
pub const PRODIGAL: &str = "5e961d15-5972-4e4b-9385-1cd7cd7c6bbe";
pub const SOUL_FEAST: &str = "8186fd80-015f-470c-9e1c-cbf45764a057";
pub const HEALING_HANDS: &str = "3cc48835-3ac0-4774-b380-f9b21d2dc974";
pub const NECTAR: &str = "30870ee5-6ad7-48a9-983e-d3b018f2344f";
pub const MOUNTAIN: &str = "a3fb7228-e76b-4e96-a40e-20b5fed75685";
pub const DAMAGE_LIFE_EXTRA: [(&str, &str); 6] = [
    (
        BOLT,
        definition_digest!("MTG_DEFINITION_damage-life_4457ed35-7c10-48c8-9776-456485fdf070"),
    ),
    (
        PRODIGAL,
        definition_digest!("MTG_DEFINITION_damage-life_5e961d15-5972-4e4b-9385-1cd7cd7c6bbe"),
    ),
    (
        SOUL_FEAST,
        definition_digest!("MTG_DEFINITION_damage-life_8186fd80-015f-470c-9e1c-cbf45764a057"),
    ),
    (
        HEALING_HANDS,
        definition_digest!("MTG_DEFINITION_damage-life_3cc48835-3ac0-4774-b380-f9b21d2dc974"),
    ),
    (
        NECTAR,
        definition_digest!("MTG_DEFINITION_damage-life_30870ee5-6ad7-48a9-983e-d3b018f2344f"),
    ),
    (
        MOUNTAIN,
        definition_digest!("MTG_DEFINITION_damage-life_a3fb7228-e76b-4e96-a40e-20b5fed75685"),
    ),
];
pub const BLOOD_ARTIST: &str = "310f141c-7f37-4729-aed6-dd9c09db448d";
pub const VISIONARY: &str = "c6a3a882-a127-4590-93d7-679ef4313efe";
pub const WARDEN: &str = "6ca2a89e-7032-4864-b4e9-66f3178f90ab";
pub const TRIGGER_EXTRA: [(&str, &str); 3] = [
    (
        BLOOD_ARTIST,
        definition_digest!("MTG_DEFINITION_trigger_310f141c-7f37-4729-aed6-dd9c09db448d"),
    ),
    (
        VISIONARY,
        definition_digest!("MTG_DEFINITION_trigger_c6a3a882-a127-4590-93d7-679ef4313efe"),
    ),
    (
        WARDEN,
        definition_digest!("MTG_DEFINITION_trigger_6ca2a89e-7032-4864-b4e9-66f3178f90ab"),
    ),
];

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
}
impl Default for ScenarioInitial {
    fn default() -> Self {
        Self {
            life: std::array::from_fn(|_| Quantity::from(40)),
            turn_step: TurnStep::PostcombatMain,
            libraries: None,
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
    pub(in crate::engine) fn has_target_color_set(&self) -> bool {
        self.catalog
            .definitions
            .iter()
            .any(|d| d.capabilities.contains(&Capability::TargetColorSetV1))
    }
    pub(in crate::engine) fn has_nonland_color_grant(&self) -> bool {
        self.catalog.definitions.iter().any(|d| {
            d.capabilities
                .contains(&Capability::ControllerNonlandColorsV1)
        })
    }
    pub(in crate::engine) fn includes_opening_placement(&self) -> bool {
        self.catalog
            .definitions
            .iter()
            .any(|d| d.capabilities.contains(&Capability::OpeningPlacementV1))
    }
    // Existing reviewed catalogs keep their exact private snapshot envelope.
    // The headless land-type family can retain new unpaid selection frames.
    pub(in crate::engine) fn has_land_type_grant(&self) -> bool {
        self.catalog.definitions.iter().any(|d| {
            d.capabilities
                .contains(&Capability::ControllerLandBasicTypesV1)
        })
    }
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
        if catalog.definitions.iter().any(|d| {
            d.static_abilities
                .iter()
                .any(|a| matches!(a.effect, StaticEffect::SelfGraveyardRevealShuffle))
        }) {
            return Err("printed graveyard shuffle requires a seeded match".into());
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
                ],
                [ALLOWED.as_slice(), &[(PLAINS, PLAINS_DIGEST)]].concat(),
            ),
            (
                vec![
                    ScenarioConfig::commander_damage(),
                    ScenarioConfig::commander_damage_borrowed(),
                ],
                [
                    ALLOWED.as_slice(),
                    &[(PLAINS, PLAINS_DIGEST)],
                    &[(YARGLE, YARGLE_DIGEST)],
                ]
                .concat(),
            ),
            (
                vec![ScenarioConfig::commander_library()],
                [
                    ALLOWED.as_slice(),
                    &[(PLAINS, PLAINS_DIGEST)],
                    &[(TIME_EBB, TIME_EBB_DIGEST)],
                ]
                .concat(),
            ),
            (
                vec![
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
            (
                vec![
                    ScenarioConfig::choices(),
                    ScenarioConfig::choices_furnace(),
                    ScenarioConfig::conditions(),
                    ScenarioConfig::conditions_nonempty(),
                    ScenarioConfig::conditions_borrowed(),
                    ScenarioConfig::conditions_no_targets(),
                ],
                [
                    ALLOWED.as_slice(),
                    &[(PLAINS, PLAINS_DIGEST)],
                    &[(YARGLE, YARGLE_DIGEST)],
                    &[(TIME_EBB, TIME_EBB_DIGEST)],
                    ACTION_EXTRA.as_slice(),
                    STACK_EXTRA.as_slice(),
                    DAMAGE_LIFE_EXTRA.as_slice(),
                    TRIGGER_EXTRA.as_slice(),
                    EFFECT_EXTRA.as_slice(),
                    CHOICE_EXTRA.as_slice(),
                ]
                .concat(),
            ),
            (
                vec![
                    ScenarioConfig::effects(),
                    ScenarioConfig::effects_furnace(),
                    ScenarioConfig::effects_two_furnaces(),
                    ScenarioConfig::effects_borrowed(),
                    ScenarioConfig::effects_low(),
                ],
                [
                    ALLOWED.as_slice(),
                    &[(PLAINS, PLAINS_DIGEST)],
                    &[(YARGLE, YARGLE_DIGEST)],
                    &[(TIME_EBB, TIME_EBB_DIGEST)],
                    ACTION_EXTRA.as_slice(),
                    STACK_EXTRA.as_slice(),
                    DAMAGE_LIFE_EXTRA.as_slice(),
                    TRIGGER_EXTRA.as_slice(),
                    EFFECT_EXTRA.as_slice(),
                ]
                .concat(),
            ),
            (
                vec![
                    ScenarioConfig::triggers(),
                    ScenarioConfig::triggers_low(),
                    ScenarioConfig::triggers_borrowed(),
                    ScenarioConfig::triggers_empty_library(),
                    ScenarioConfig::triggers_nonactive(),
                ],
                [
                    ALLOWED.as_slice(),
                    &[(PLAINS, PLAINS_DIGEST)],
                    &[(YARGLE, YARGLE_DIGEST)],
                    &[(TIME_EBB, TIME_EBB_DIGEST)],
                    ACTION_EXTRA.as_slice(),
                    STACK_EXTRA.as_slice(),
                    DAMAGE_LIFE_EXTRA.as_slice(),
                    TRIGGER_EXTRA.as_slice(),
                ]
                .concat(),
            ),
            (
                vec![
                    ScenarioConfig::damage_life(),
                    ScenarioConfig::damage_life_low(),
                    ScenarioConfig::damage_life_borrowed(),
                    ScenarioConfig::damage_life_empty_library(),
                    ScenarioConfig::damage_life_library(),
                ],
                [
                    ALLOWED.as_slice(),
                    &[(PLAINS, PLAINS_DIGEST)],
                    &[(YARGLE, YARGLE_DIGEST)],
                    &[(TIME_EBB, TIME_EBB_DIGEST)],
                    ACTION_EXTRA.as_slice(),
                    STACK_EXTRA.as_slice(),
                    DAMAGE_LIFE_EXTRA.as_slice(),
                ]
                .concat(),
            ),
            (
                vec![
                    ScenarioConfig::stack_costs(),
                    ScenarioConfig::stack_costs_borrowed(),
                    ScenarioConfig::stack_costs_empty_library(),
                    ScenarioConfig::stack_costs_library(),
                ],
                [
                    ALLOWED.as_slice(),
                    &[(PLAINS, PLAINS_DIGEST)],
                    &[(YARGLE, YARGLE_DIGEST)],
                    &[(TIME_EBB, TIME_EBB_DIGEST)],
                    ACTION_EXTRA.as_slice(),
                    STACK_EXTRA.as_slice(),
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
