mod advance_report;
mod capacities;
#[cfg(test)]
mod data_rules_tests;
mod search;
// Missing/malformed generated outputs remain runtime admission failures rather
// than making the catalog diagnostic CLI impossible to compile.
macro_rules! definition_digest {
    ($name:literal) => {
        match option_env!($name) {
            Some(value) => value,
            None => "",
        }
    };
}
pub use search::*;
mod match_admission;
mod snapshot_codec;
mod state_digest;
pub use advance_report::*;
pub use match_admission::*;
mod commander_policy;
pub use commander_policy::policy_digest;
mod removal;
#[cfg(test)]
mod removal_tests;
mod sacrifice;
#[cfg(test)]
mod sacrifice_tests;
mod wipes;
#[cfg(test)]
mod wipes_tests;
use removal::TargetFacts;
mod attachments;
#[cfg(test)]
mod attachments_tests;
#[cfg(test)]
mod four_colors_tests;
mod staples;
mod tokens;
pub use staples::VisibleSearchLand;
pub use tokens::{RuleFault, TOKEN_CAPACITY, VisibleToken};
mod keywords;
#[cfg(test)]
mod staples_tests;
#[cfg(test)]
mod tokens_tests;
pub use keywords::LifelinkGain;
#[cfg(test)]
mod combat_keywords_tests;
#[cfg(test)]
mod keywords_tests;
mod setup;
#[cfg(test)]
mod setup_tests;
pub use setup::*;
const GROWTH: &str = "5748ebf1-24e3-499d-ab7c-c2cebd462a24";
const DISFIGURE: &str = "77eafe49-b9c5-461d-89c5-cec217dd2974";
const FOG: &str = "27e9db49-7af7-4bef-ad4c-bf5dfb92030d";
const HEALER: &str = "95a0ca48-d924-47f4-86ed-42c673ee778c";
const FURNACE: &str = "68715465-6cf9-4006-87e9-31f227fe9ed3";
const OX: &str = "19b4cf0b-e1e4-4e50-80bc-673bc6ef8bb8";
const EFFECT_EXTRA: [(&str, &str); 6] = [
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
mod allocation;
pub use allocation::*;
mod choices;
#[cfg(test)]
mod choices_tests;
pub use choices::*;
mod effects;
#[cfg(test)]
mod effects_tests;
use crate::BUILD_ID;
pub use effects::*;
use mtg_contracts::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

const ISLAND: &str = "b2c6aa39-2d2a-459c-a555-fb48ba993373";
const FOREST: &str = "b34bb2dc-c1af-4d77-b0b3-a0fb342a5fc6";
const SPELL: &str = "837182db-1bf3-4a2c-bd01-1af9d9873561";
const BEARS: &str = "14c8f55d-d177-4c25-a931-ebeb9e6062a0";
const PLAINS: &str = "bc71ebf6-2056-41f7-be35-b2e5c34afa99";
const PLAINS_DIGEST: &str =
    definition_digest!("MTG_DEFINITION_commander_bc71ebf6-2056-41f7-be35-b2e5c34afa99");
const TIME_EBB: &str = "30cc8f7b-3c28-40f5-8f8f-157e8212280b";
const TIME_EBB_DIGEST: &str =
    definition_digest!("MTG_DEFINITION_library_30cc8f7b-3c28-40f5-8f8f-157e8212280b");
const YARGLE: &str = "980e1721-3717-4425-95a4-938dbf2ac661";
const YARGLE_DIGEST: &str =
    definition_digest!("MTG_DEFINITION_damage_980e1721-3717-4425-95a4-938dbf2ac661");
const EDICT: &str = "10c585c4-bf5b-4d8f-94a9-e9a5036a688f";
const HAPLESS: &str = "240a349a-79f8-4e11-9a05-a78148e70a62";
const SWAMP: &str = "56719f6a-1a6c-4c0a-8d21-18f7d7350b68";
const ACTION_EXTRA: [(&str, &str); 3] = [
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
const LOOTER: &str = "67362406-b1ca-49e2-800d-9050bfe8742a";
const STIFLE: &str = "b3b00911-ece7-4484-bc36-f211ce72b6cc";
const AZURE: &str = "bbf9fd47-10e1-4749-b06f-4fe72200afc7";
const COUNTERSPELL: &str = "cc187110-1148-4090-bbb8-e205694a39f5";
const STACK_EXTRA: [(&str, &str); 4] = [
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
const ISAMARU: &str = "cfb756dd-6fe1-430a-8f04-8b10cac62a86";
// Reviewed canonical definition digests, independent of compiler success.
const ALLOWED: [(&str, &str); 5] = [
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
const BOLT: &str = "4457ed35-7c10-48c8-9776-456485fdf070";
const PRODIGAL: &str = "5e961d15-5972-4e4b-9385-1cd7cd7c6bbe";
const SOUL_FEAST: &str = "8186fd80-015f-470c-9e1c-cbf45764a057";
const HEALING_HANDS: &str = "3cc48835-3ac0-4774-b380-f9b21d2dc974";
const NECTAR: &str = "30870ee5-6ad7-48a9-983e-d3b018f2344f";
const MOUNTAIN: &str = "a3fb7228-e76b-4e96-a40e-20b5fed75685";
const DAMAGE_LIFE_EXTRA: [(&str, &str); 6] = [
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
const BLOOD_ARTIST: &str = "310f141c-7f37-4729-aed6-dd9c09db448d";
const VISIONARY: &str = "c6a3a882-a127-4590-93d7-679ef4313efe";
const WARDEN: &str = "6ca2a89e-7032-4864-b4e9-66f3178f90ab";
const TRIGGER_EXTRA: [(&str, &str); 3] = [
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
mod damage_life;
#[cfg(test)]
mod damage_life_tests;
pub use damage_life::{NoncombatDamageOccurrence, TargetEntity};
#[cfg(test)]
mod attack_trigger_tests;
#[cfg(test)]
mod attacking_creature_keyword_tests;
#[cfg(test)]
mod cast_prohibition_tests;
#[cfg(test)]
mod cast_trigger_tests;
#[cfg(test)]
mod controlled_creature_damage_replacement_tests;
#[cfg(test)]
mod damage_controller_discard_tests;
#[cfg(test)]
mod damage_match_appearance_tests;
#[cfg(test)]
mod damage_match_tests;
#[cfg(test)]
mod damage_match_v2_tests;
#[cfg(test)]
mod damage_quantity_draw_tests;
#[cfg(test)]
mod damage_trigger_tests;
#[cfg(test)]
mod event_player_discard_tests;
#[cfg(test)]
mod event_player_sacrifice_tests;
#[cfg(test)]
mod filtered_damage_trigger_tests;
#[cfg(test)]
mod grouped_damage_trigger_tests;
#[cfg(test)]
mod opponent_damage_trigger_tests;
#[cfg(test)]
mod optional_draw_tests;
#[cfg(test)]
mod step_trigger_tests;
#[cfg(test)]
mod trigger_tests;
mod triggers;
pub use triggers::{
    CastOccurrence, CastOccurrenceId, GroupedDamageMember, TriggerEventWitness, TriggerId,
    TriggerOccurrenceId,
};
use triggers::{PendingTrigger, TriggerPlacement};
mod history;
pub use history::History;
mod recovery;
pub use recovery::SnapshotVerifier;
mod actions;
mod casting;
mod mana_permanents;
#[cfg(test)]
mod mana_tests;
pub use mana_permanents::PermanentManaUndo;
use mana_permanents::{ManaActivation, zero_mana_cost};
mod stack_costs;
use stack_costs::AnnouncementOrigin;
#[cfg(test)]
mod stack_costs_tests;
use actions::{ResolutionChoice, StackKind};
#[cfg(test)]
mod actions_tests;
mod commander;
mod payments;
#[cfg(test)]
mod spending_tests;
use commander::CommanderContinuation;
mod combat;
#[cfg(test)]
mod combat_tests;
#[cfg(test)]
mod commander_damage_library_tests;
#[cfg(test)]
mod commander_tests;
mod legend;
mod scenarios;
pub use combat::*;
mod turns;
pub use turns::*;
mod events;
mod trace;
#[cfg(test)]
mod turns_tests;
pub use events::*;
pub use trace::*;
mod multiplayer;
pub use multiplayer::*;
mod permitted_history;
mod stored_history;
pub use permitted_history::PermittedHistory;
mod visibility;
mod work;
pub use visibility::*;
pub use work::*;
#[cfg(test)]
mod definition_diversity_tests;
#[cfg(test)]
mod dense_kernel_tests;
#[cfg(test)]
mod departure_tests;
#[cfg(test)]
mod event_tests;
#[cfg(test)]
mod legal_branch_tests;
#[cfg(test)]
mod p2_tests;
#[cfg(test)]
mod payment_event_tests;
#[cfg(test)]
mod payment_query_tests;
#[cfg(test)]
mod payment_tests;
#[cfg(test)]
mod prepared_sharing_tests;
#[cfg(test)]
mod stack_tests;

macro_rules! id {
    ($name:ident,$inner:ty) => {
        #[derive(
            Clone,
            Copy,
            Debug,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Serialize,
            Deserialize,
            schemars::JsonSchema,
        )]
        #[serde(transparent)]
        pub struct $name(pub $inner);
    };
}
id!(PlayerId, u8);
id!(CardId, u32);
id!(StackId, u32);
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectId {
    pub card: CardId,
    pub incarnation: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "zone", rename_all = "snake_case", deny_unknown_fields)]
pub enum Zone {
    Battlefield,
    Hand { owner: PlayerId },
    Graveyard { owner: PlayerId },
    Library { owner: PlayerId },
    Command { owner: PlayerId },
    Exile,
    Stack,
}
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
#[derive(Clone)]
pub struct Prepared {
    match_config: Option<MatchConfig>,
    catalog: Arc<Catalog>,
    catalog_digest: String,
    config: ScenarioConfig,
    profile_digest: String,
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
        self.match_config.as_ref()
    }
    pub fn admit(catalog: Catalog, config: ScenarioConfig) -> Result<Self, String> {
        catalog.validate()?;
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Player {
    failed_draw: bool,
    life: Quantity,
    mana: BTreeMap<Color, Quantity>,
    status: PlayerStatus,
    last_known: Option<PlayerLastKnown>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Card {
    owner: PlayerId,
    oracle_id: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Object {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    attached_to: Option<ObjectId>,
    #[serde(default, skip_serializing_if = "tokens::quantity_is_zero")]
    plus_one_counters: Quantity,
    marked_damage: Quantity,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    deathtouch_damage: bool,
    attack_ready: bool,
    id: ObjectId,
    controller: PlayerId,
    default_controller: PlayerId,
    zone: Zone,
    tapped: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StackInstance {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    x: Option<Quantity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    death_controller: Option<PlayerId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sacrificed: Option<ObjectBefore>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mode: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    effect_source_handles: Vec<ObjectHandle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source_facts: Option<ObjectBefore>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    entity_targets: Vec<TargetEntity>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    announced_entity_targets: Vec<Vec<Option<ObjectHandle>>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    stack_targets: Vec<StackId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    announced_stack_targets: Vec<Vec<StackHandle>>,
    #[serde(default, skip_serializing_if = "StackKind::is_spell")]
    kind: StackKind,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    player_targets: Vec<PlayerId>,
    id: StackId,
    source: ObjectId,
    controller: PlayerId,
    targets: Vec<ObjectId>,
    announced_targets: Vec<Vec<ObjectHandle>>,
    paid: Option<ManaCost>,
    total_cost: ManaCost,
    cast_from: Zone,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Casting {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sacrifice: Option<ObjectId>,
    #[serde(default, skip_serializing_if = "AnnouncementOrigin::is_spell")]
    origin: AnnouncementOrigin,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    spending: Option<BTreeMap<Color, Quantity>>,
    stack_id: StackId,
    original_object: ObjectId,
    prior_passes: Vec<PlayerId>,
    mana_activations: Vec<ManaActivationUndo>,
    undo_mana: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManaActivationUndo {
    object: ObjectId,
    player: PlayerId,
    color: Color,
    amount: Quantity,
    mana_was_absent: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    permanent: Option<Box<PermanentManaUndo>>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SetDamageFacts {
    cursor: u32,
    amount: Quantity,
    targets: Vec<TargetEntity>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DamageDrawProgress {
    cursor: u32,
    source_block: u32,
    completed: Quantity,
    remaining: Quantity,
    failed: Quantity,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Resolving {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    damage_draw: Option<DamageDrawProgress>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    set_damage: Option<SetDamageFacts>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    condition_check: Option<InterveningCheck>,
    stack_id: StackId,
    spell: StackInstance,
    cursor: u32,
    legal_targets: Vec<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    target_facts: Vec<Option<TargetFacts>>,
    outcomes: Vec<InstructionOutcome>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "stage", rename_all = "snake_case", deny_unknown_fields)]
enum Frame {
    LegendRule {
        resume: Box<Frame>,
        groups: Vec<Vec<ObjectId>>,
        kept: Vec<ObjectId>,
    },
    AnnounceX {
        cast: Casting,
    },
    SacrificeCost {
        cast: Casting,
    },
    ForcedSacrifice {
        frame: Resolving,
        remaining: Vec<PlayerId>,
        choices: Vec<(PlayerId, ObjectId)>,
        source_block: u32,
    },
    OptionalDamage {
        frame: Resolving,
    },
    ManaAbility {
        activation: Box<ManaActivation>,
    },
    Setup,
    DamageAllocation {
        transaction: DamageTransaction,
        effect: EffectKey,
    },
    Modes {
        cast: Casting,
    },
    DamageEffects {
        transaction: DamageTransaction,
    },
    TriggerOrder {
        placement: TriggerPlacement,
    },
    TriggerTargets {
        placement: TriggerPlacement,
        stack_id: StackId,
    },
    EntityTargets {
        cast: Casting,
    },
    StackTargets {
        cast: Casting,
    },
    PlayerTargets {
        cast: Casting,
    },
    SpendMana {
        cast: Casting,
    },
    ResolutionChoice {
        frame: Resolving,
        choice: ResolutionChoice,
    },
    CommanderReturn {
        object: ObjectId,
        destination: Zone,
        continuation: Box<CommanderContinuation>,
    },
    Attackers,
    Blockers {
        remaining: Vec<PlayerId>,
    },
    Damage,
    DamageChoice {
        source: ObjectId,
    },
    Discard {
        player: PlayerId,
        count: usize,
    },
    CleanupClear,
    Stabilize {
        resume: Box<Frame>,
        cleanup: bool,
        changed: bool,
    },
    Draw,
    Priority,
    Transition,
    EnterStep,
    Targets {
        cast: Casting,
    },
    Mana {
        cast: Casting,
    },
    Payment {
        cast: Casting,
    },
    Resolve {
        frame: Resolving,
    },
    Fault {
        reason: String,
    },
    Finished {
        winner: PlayerId,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "response", rename_all = "snake_case", deny_unknown_fields)]
pub enum Response {
    LegendRule {
        keep: ObjectId,
    },
    AnnounceX {
        value: Quantity,
    },
    SacrificeCost {
        object: ObjectId,
    },
    ForcedSacrifice {
        object: ObjectId,
    },
    ManaColor {
        color: Color,
    },
    SearchLands {
        objects: Vec<ObjectId>,
    },
    OptionalEffect {
        perform: bool,
    },
    OptionalRecursion {
        perform: bool,
    },
    Mulligan {
        take: bool,
    },
    BottomOpening {
        objects: Vec<ObjectId>,
    },
    DamageAllocation {
        allocations: Vec<(DamageId, Quantity)>,
    },
    Modes {
        mode: u32,
    },
    DamageEffect {
        effect: EffectKey,
        damage: DamageId,
    },
    OrderTriggers {
        order: Vec<TriggerId>,
    },
    EntityTargets {
        entities: Vec<TargetEntity>,
    },
    StackTargets {
        entries: Vec<StackId>,
    },
    ActivateAbility {
        object: ObjectId,
        ability: u32,
    },
    PlayerTargets {
        players: Vec<PlayerId>,
    },
    Sacrifice {
        object: ObjectId,
    },
    ResolveDiscard {
        objects: Vec<ObjectId>,
    },
    SpendMana {
        spending: BTreeMap<Color, Quantity>,
    },
    CommanderReturn {
        to_command: bool,
    },
    Attackers {
        attacks: Vec<(ObjectId, PlayerId)>,
    },
    Blockers {
        blocks: Vec<(ObjectId, ObjectId)>,
    },
    Damage {
        assignments: Vec<(ObjectId, Quantity)>,
    },
    TrampleDamage {
        assignments: Vec<(ObjectId, Quantity)>,
        player: PlayerId,
        amount: Quantity,
    },
    Discard {
        objects: Vec<ObjectId>,
    },
    PlayLand {
        card: CardId,
    },
    ActivateMana {
        object: ObjectId,
    },
    Cast {
        card: CardId,
    },
    Pass,
    FinishMana {
        undo_mana: bool,
    },
    Targets {
        objects: Vec<ObjectId>,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptedInput {
    pub decision_id: u64,
    pub player: PlayerId,
    pub response: Response,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "request", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    LegendRule {
        decision_id: u64,
        player: PlayerId,
        candidates: Vec<ObjectId>,
        previous_choices: Vec<ObjectId>,
    },
    AnnounceX {
        decision_id: u64,
        player: PlayerId,
    },
    SacrificeCost {
        decision_id: u64,
        player: PlayerId,
        candidates: Vec<ObjectId>,
    },
    ForcedSacrifice {
        decision_id: u64,
        player: PlayerId,
        candidates: Vec<ObjectId>,
        previous_choices: Vec<(PlayerId, ObjectId)>,
    },
    ManaColor {
        decision_id: u64,
        player: PlayerId,
        colors: Vec<Color>,
    },
    SearchLands {
        decision_id: u64,
        player: PlayerId,
        maximum: u32,
        candidates: Vec<ObjectId>,
    },
    OptionalEffect {
        decision_id: u64,
        player: PlayerId,
    },
    OptionalRecursion {
        decision_id: u64,
        player: PlayerId,
    },
    Mulligan {
        decision_id: u64,
        player: PlayerId,
        mulligans_taken: u8,
        bottom_if_mulligan: u8,
        can_mulligan: bool,
    },
    BottomOpening {
        decision_id: u64,
        player: PlayerId,
        count: usize,
        candidates: Vec<ObjectId>,
    },
    DamageAllocation {
        decision_id: u64,
        player: PlayerId,
        recipient: TargetEntity,
        effect: EffectKey,
        remaining: Quantity,
        required: Quantity,
        candidates: Vec<DamageAllocationCandidate>,
    },
    Modes {
        decision_id: u64,
        player: PlayerId,
        candidates: Vec<u32>,
    },
    DamageEffect {
        decision_id: u64,
        player: PlayerId,
        recipient: TargetEntity,
        candidates: Vec<DamageEffectCandidate>,
    },
    OrderTriggers {
        decision_id: u64,
        player: PlayerId,
        candidates: Vec<TriggerId>,
    },
    EntityTargets {
        decision_id: u64,
        player: PlayerId,
        occurrences: Vec<TargetSpec>,
        candidates: Vec<TargetEntity>,
    },
    StackTargets {
        decision_id: u64,
        player: PlayerId,
        occurrences: Vec<TargetSpec>,
        candidates: Vec<StackId>,
    },
    PlayerTargets {
        decision_id: u64,
        player: PlayerId,
        occurrences: Vec<TargetSpec>,
        candidates: Vec<PlayerId>,
    },
    Sacrifice {
        decision_id: u64,
        player: PlayerId,
        candidates: Vec<ObjectId>,
    },
    ResolveDiscard {
        decision_id: u64,
        player: PlayerId,
        count: usize,
        candidates: Vec<ObjectId>,
    },
    SpendMana {
        decision_id: u64,
        player: PlayerId,
        cost: ManaCost,
        available: BTreeMap<Color, Quantity>,
    },
    CommanderReturn {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        previous_choices: Vec<(ObjectId, bool)>,
        decision_id: u64,
        player: PlayerId,
        object: ObjectId,
        destination: Zone,
    },
    Attackers {
        decision_id: u64,
        player: PlayerId,
        candidates: Vec<ObjectId>,
        defenders: Vec<PlayerId>,
    },
    Blockers {
        decision_id: u64,
        player: PlayerId,
        candidates: Vec<ObjectId>,
        attackers: Vec<ObjectId>,
    },
    Damage {
        decision_id: u64,
        player: PlayerId,
        source: ObjectId,
        power: Quantity,
        recipients: Vec<ObjectId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        player_recipient: Option<PlayerId>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        lethal_requirements: Vec<(ObjectId, Quantity)>,
    },
    Discard {
        decision_id: u64,
        player: PlayerId,
        count: usize,
        candidates: Vec<ObjectId>,
    },
    Mana {
        decision_id: u64,
        player: PlayerId,
        cost: ManaCost,
    },
    Priority {
        decision_id: u64,
        player: PlayerId,
    },
    Targets {
        decision_id: u64,
        player: PlayerId,
        occurrences: Vec<TargetSpec>,
        candidates: Vec<ObjectId>,
    },
}
impl Request {
    pub fn decision_id(&self) -> u64 {
        match self {
            Self::AnnounceX { decision_id, .. }
            | Self::ManaColor { decision_id, .. }
            | Self::SearchLands { decision_id, .. }
            | Self::OptionalEffect { decision_id, .. }
            | Self::OptionalRecursion { decision_id, .. }
            | Self::Mulligan { decision_id, .. }
            | Self::BottomOpening { decision_id, .. }
            | Self::DamageAllocation { decision_id, .. }
            | Self::Modes { decision_id, .. }
            | Self::DamageEffect { decision_id, .. }
            | Self::OrderTriggers { decision_id, .. }
            | Self::EntityTargets { decision_id, .. }
            | Self::StackTargets { decision_id, .. }
            | Self::PlayerTargets { decision_id, .. }
            | Self::Sacrifice { decision_id, .. }
            | Self::SacrificeCost { decision_id, .. }
            | Self::ForcedSacrifice { decision_id, .. }
            | Self::LegendRule { decision_id, .. }
            | Self::ResolveDiscard { decision_id, .. }
            | Self::SpendMana { decision_id, .. }
            | Self::CommanderReturn { decision_id, .. }
            | Self::Attackers { decision_id, .. }
            | Self::Blockers { decision_id, .. }
            | Self::Damage { decision_id, .. }
            | Self::Discard { decision_id, .. }
            | Self::Priority { decision_id, .. }
            | Self::Targets { decision_id, .. }
            | Self::Mana { decision_id, .. } => *decision_id,
        }
    }
    pub fn player(&self) -> PlayerId {
        match self {
            Self::AnnounceX { player, .. }
            | Self::ManaColor { player, .. }
            | Self::SearchLands { player, .. }
            | Self::OptionalEffect { player, .. }
            | Self::OptionalRecursion { player, .. }
            | Self::Mulligan { player, .. }
            | Self::BottomOpening { player, .. }
            | Self::DamageAllocation { player, .. }
            | Self::Modes { player, .. }
            | Self::DamageEffect { player, .. }
            | Self::OrderTriggers { player, .. }
            | Self::EntityTargets { player, .. }
            | Self::StackTargets { player, .. }
            | Self::PlayerTargets { player, .. }
            | Self::Sacrifice { player, .. }
            | Self::SacrificeCost { player, .. }
            | Self::ForcedSacrifice { player, .. }
            | Self::LegendRule { player, .. }
            | Self::ResolveDiscard { player, .. }
            | Self::SpendMana { player, .. }
            | Self::CommanderReturn { player, .. }
            | Self::Attackers { player, .. }
            | Self::Blockers { player, .. }
            | Self::Damage { player, .. }
            | Self::Discard { player, .. }
            | Self::Priority { player, .. }
            | Self::Targets { player, .. }
            | Self::Mana { player, .. } => *player,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "proposal", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProposedEvent {
    Attach {
        object: ObjectId,
        target: Option<ObjectId>,
    },
    ReversePermanentMana {
        receipt: Box<PermanentManaUndo>,
    },
    FinishLibraryShuffle {
        player: PlayerId,
    },
    DestroyPermanent {
        object: ObjectId,
    },
    DestroyCreature {
        object: ObjectId,
    },
    SearchLandEntry {
        object: ObjectId,
        player: PlayerId,
    },
    ShuffleLibraryWord {
        player: PlayerId,
        index: u32,
    },
    CreateCreatureTokens {
        controller: PlayerId,
        source: ObjectId,
        count: u32,
        token: Box<mtg_contracts::CreatureToken>,
    },
    PutPlusOneCounters {
        object: ObjectId,
        amount: Quantity,
    },
    CeaseToken {
        object: ObjectId,
    },
    Setup {
        action: SetupAction,
    },
    AllocateDamageEffect {
        transaction: DamageTransactionId,
        effect: EffectKey,
        allocations: Vec<(DamageId, Quantity)>,
    },
    CreateEffect {
        effect: Box<ResolvedEffect>,
    },
    ApplyDamageEffect {
        transaction: DamageTransactionId,
        damage: DamageId,
        effect: EffectKey,
    },
    CommitDamage {
        transaction: DamageTransactionId,
    },
    CleanupEffects,
    NoncombatDamage {
        damage: Box<NoncombatDamageOccurrence>,
    },
    GainLife {
        player: PlayerId,
        amount: Quantity,
    },
    LoseLife {
        player: PlayerId,
        amount: Quantity,
    },
    CounterStackObject {
        target: StackId,
        countering: StackId,
        domain: TargetDomain,
    },
    Sacrifice {
        object: ObjectId,
        player: PlayerId,
    },
    DiscardCard {
        object: ObjectId,
        player: PlayerId,
    },
    PayManaSelected {
        player: PlayerId,
        cost: ManaCost,
        spending: BTreeMap<Color, Quantity>,
    },
    PutOnTopOwnerLibrary {
        object: ObjectId,
    },
    Lose {
        player: PlayerId,
        reason: PlayerLossReason,
    },
    DeclareAttackers {
        attacks: Vec<(ObjectId, PlayerId)>,
    },
    DeclareBlockers {
        player: PlayerId,
        blocks: Vec<(ObjectId, ObjectId)>,
    },
    CombatDamage {
        damage: Vec<DamageOccurrence>,
    },
    FailedDraw {
        player: PlayerId,
    },
    ClearDamage,
    StateBasedGraveyard {
        object: ObjectId,
        reason: CreatureLossReason,
    },
    Untap {
        object: ObjectId,
    },
    ClearMana {
        player: PlayerId,
    },
    LandPlayed {
        player: PlayerId,
    },
    TurnTransition {
        next: TurnState,
        owner: PlayerId,
    },
    ReverseMana {
        player: PlayerId,
        color: Color,
        amount: Quantity,
        source: Option<ObjectId>,
        remove_zero: bool,
    },
    Tap {
        object: ObjectId,
    },
    AddMana {
        player: PlayerId,
        color: Color,
        amount: Quantity,
        source: ObjectId,
    },
    PayMana {
        player: PlayerId,
        cost: ManaCost,
    },
    Move {
        object: ObjectId,
        destination: Zone,
    },
    LeaveGame {
        object: ObjectId,
        departing_owner: PlayerId,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MoveOutcome {
    pub previous: ObjectId,
    pub successor: ObjectId,
    pub from: Zone,
    pub destination: Zone,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum EventOutcome {
    CounterPrevented {
        target: StackId,
    },
    AttachmentChanged {
        object: ObjectId,
        previous: Option<ObjectId>,
        target: Option<ObjectId>,
    },
    PermanentManaReversed {
        player: PlayerId,
        source: ObjectId,
    },
    LibraryShuffled {
        player: PlayerId,
        count: u64,
    },
    LibraryShuffledWord {
        player: PlayerId,
        index: u32,
        word: u32,
        selected: Option<u32>,
        before: RngPosition,
        after: RngPosition,
    },
    TokensCreated {
        objects: Vec<ObjectId>,
        controller: PlayerId,
        token: Box<mtg_contracts::CreatureToken>,
    },
    PlusOneCountersPlaced {
        object: ObjectId,
        amount: Quantity,
        previous: Quantity,
        current: Quantity,
    },
    TokenCeased {
        object: ObjectId,
    },
    Setup {
        actual: SetupOutcome,
    },
    DamageEffectAllocated {
        transaction: DamageTransactionId,
        effect: EffectKey,
        allocations: Vec<(DamageId, Quantity)>,
        applications: Vec<DamageApplication>,
    },
    EffectCreated {
        effect: Box<ResolvedEffect>,
    },
    DamageEffectApplied {
        application: Box<DamageApplication>,
    },
    DamageCommitted {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        life_gains: Vec<LifelinkGain>,
        transaction: DamageTransactionId,
        actual: Vec<ActualDamage>,
        results: Vec<DamageResult>,
        commander_damage: Vec<CommanderDamageResult>,
        cancelled: Vec<DamageId>,
    },
    EffectsCleaned {
        expired: Vec<EffectId>,
        previous: Vec<(ObjectId, Quantity)>,
    },
    NoncombatDamageDealt {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        life_gains: Vec<LifelinkGain>,
        damage: Box<NoncombatDamageOccurrence>,
        result: DamageResult,
    },
    LifeGained {
        player: PlayerId,
        amount: Quantity,
        previous: Quantity,
        current: Quantity,
    },
    LifeLost {
        player: PlayerId,
        amount: Quantity,
        previous: Quantity,
        current: Quantity,
    },
    StackObjectCountered {
        target: StackId,
        source: ObjectId,
        spell: bool,
        movement: Option<MoveOutcome>,
    },
    Sacrificed {
        player: PlayerId,
        movement: MoveOutcome,
    },
    CardDiscarded {
        player: PlayerId,
        movement: MoveOutcome,
    },
    ManaPaidSelected {
        player: PlayerId,
        cost: ManaCost,
        spending: BTreeMap<Color, Quantity>,
    },
    LibraryTopMoved {
        movement: MoveOutcome,
        previous_top: Option<CardId>,
    },
    Lost {
        player: PlayerId,
        reason: PlayerLossReason,
    },
    AttackersDeclared {
        attacks: Vec<(ObjectId, PlayerId)>,
    },
    BlockersDeclared {
        player: PlayerId,
        blocks: Vec<(ObjectId, ObjectId)>,
    },
    DamageDealt {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        life_gains: Vec<LifelinkGain>,
        damage: Vec<DamageOccurrence>,
        results: Vec<DamageResult>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        commander_damage: Vec<CommanderDamageResult>,
    },
    DrawFailed {
        player: PlayerId,
    },
    DamageCleared {
        previous: Vec<(ObjectId, Quantity)>,
    },
    StateBasedMoved {
        movement: MoveOutcome,
        reason: CreatureLossReason,
    },
    Untapped {
        object: ObjectId,
    },
    ManaCleared {
        player: PlayerId,
        previous: BTreeMap<Color, Quantity>,
    },
    LandPlayCounted {
        player: PlayerId,
        previous: u32,
        current: u32,
    },
    TurnTransitioned {
        previous: TurnState,
        current: TurnState,
        previous_owner: PlayerId,
        owner: PlayerId,
    },
    ManaReversed {
        player: PlayerId,
        color: Color,
        amount: Quantity,
        source: Option<ObjectId>,
        removed_entry: bool,
    },
    LeftGame {
        object: ObjectId,
        owner: PlayerId,
    },
    Tapped {
        object: ObjectId,
    },
    ManaAdded {
        player: PlayerId,
        color: Color,
        amount: Quantity,
    },
    ManaPaid {
        player: PlayerId,
        cost: ManaCost,
    },
    Moved {
        movement: MoveOutcome,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
enum InstructionOutcome {
    CounterPrevented {
        target: StackId,
    },
    SetMoved {
        movements: Vec<MoveOutcome>,
        destroyed_creatures: usize,
    },
    Effect {
        actual: Option<EventOutcome>,
    },
    Damage {
        actual: Option<EventOutcome>,
    },
    Life {
        actual: Option<EventOutcome>,
    },
    StackTargetIllegal {
        target: StackId,
    },
    StackObjectNoLongerPresent {
        target: StackId,
    },
    Countered {
        target: StackId,
        movement: Option<MoveOutcome>,
    },
    PlayerActionSkipped {
        player: PlayerId,
    },
    Sacrificed {
        player: PlayerId,
        movement: Option<MoveOutcome>,
    },
    Drawn {
        player: PlayerId,
        movement: Option<MoveOutcome>,
        failed: bool,
    },
    DamageDrawn {
        player: PlayerId,
        attempted: Quantity,
        failed: Quantity,
    },
    Discarded {
        player: PlayerId,
        movements: Vec<MoveOutcome>,
    },
    Moved {
        movement: MoveOutcome,
    },
    IllegalTarget {
        target: ObjectId,
    },
    ObjectNoLongerPresent {
        target: ObjectId,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
pub enum Event {
    XAnnounced {
        stack_id: StackId,
        player: PlayerId,
        value: Quantity,
    },
    ModeSelected {
        stack_id: StackId,
        player: PlayerId,
        mode: u32,
    },
    TriggerConditionChecked {
        source: ObjectId,
        controller: PlayerId,
        ability: u32,
        stage: InterveningStage,
        check: InterveningCheck,
    },
    TriggeredAbilityNotPlaced {
        trigger_id: TriggerId,
        stack_id: StackId,
        reason: TriggerPlacementFailure,
    },
    TriggerCollected {
        trigger_id: TriggerId,
        source: ObjectId,
        player: PlayerId,
        ability: u32,
        occurrence: TriggerOccurrenceId,
        witness: TriggerEventWitness,
        source_facts: Box<ObjectBefore>,
    },
    TriggersOrdered {
        player: PlayerId,
        order: Vec<TriggerId>,
    },
    TriggerPlaced {
        trigger_id: TriggerId,
        stack_id: StackId,
        player: PlayerId,
    },
    TriggeredAbilityCompleted {
        stack_id: StackId,
        resolved: bool,
    },
    AbilityActivated {
        stack_id: StackId,
        source: ObjectId,
        player: PlayerId,
        ability: u32,
    },
    AbilityCompleted {
        stack_id: StackId,
    },
    GameDrawn,
    PlayersLost {
        losses: Vec<(PlayerId, PlayerLossReason)>,
    },
    PlayerDeparted {
        player: PlayerId,
    },
    ObjectLeftGame {
        object: ObjectId,
        owner: PlayerId,
    },
    ControlEffectEnded {
        effect_id: u32,
    },
    ControlChanged {
        object: ObjectId,
        previous: PlayerId,
        controller: PlayerId,
    },
    StackRemovedForDeparture {
        stack_id: StackId,
        player: PlayerId,
    },
    GameWon {
        winner: PlayerId,
    },
    Committed {
        proposed: ProposedEvent,
        actual: EventOutcome,
    },
    SpellProposed {
        card: CardId,
        stack_id: StackId,
    },
    SpellCast {
        stack_id: StackId,
    },
    PriorityPassed {
        player: PlayerId,
    },
    SpellCompleted {
        stack_id: StackId,
        resolved: bool,
    },
    CastReversed {
        stack_id: StackId,
        reason: String,
    },
}
mod player_session;
pub use player_session::PlayerSession;
#[cfg(test)]
mod architecture_transaction_tests;
mod state;
#[cfg(test)]
mod state_ownership_tests;
use state::{AuditState, KnowledgeState, RulesState, State};

/// Full raw snapshot output receipt, returned only after the caller sink flushes.
/// Host metadata only; not a snapshot field, schema or semantic State input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotWriteInfo {
    pub bytes: u64,
    pub snapshot_sha256: String,
    pub state_sha256: String,
}
pub struct Game {
    // Admitted metadata is immutable for this executor and all staged forks.
    prepared: Arc<Prepared>,
    state: State,
    trace: TraceEvidence,
    executor: Arc<()>,
    state_token: Arc<()>,
    // Private staging scope: nested event batches share the outer rollback.
    in_rule_step: bool,
}
pub struct ValidatedResponse {
    executor: Arc<()>,
    revision: u64,
    state_token: Arc<()>,
    input: AcceptedInput,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    snapshot_version: u32,
    build_id: String,
    catalog_digest: String,
    profile_digest: String,
    state_sha256: String,
    state: State,
    trace: TraceEvidence,
}
#[derive(Serialize)]
struct SnapshotRef<'a> {
    snapshot_version: u32,
    build_id: &'a str,
    catalog_digest: &'a str,
    profile_digest: &'a str,
    state_sha256: &'a str,
    state: &'a State,
    trace: &'a TraceEvidence,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Advance {
    Draw,
    Finished { winner: PlayerId },
    AwaitingDecision(Request),
    Fault(String),
    Suspended { required_minimum: u64 },
}

impl Game {
    pub fn new(prepared: Prepared) -> Self {
        let cards = prepared
            .config
            .cards
            .iter()
            .map(|c| {
                (
                    c.id,
                    Card {
                        owner: c.owner,
                        oracle_id: c.oracle_id.clone(),
                    },
                )
            })
            .collect();
        let objects = prepared
            .config
            .cards
            .iter()
            .map(|c| {
                (
                    c.id,
                    Object {
                        attached_to: None,
                        plus_one_counters: Quantity::zero(),
                        marked_damage: Quantity::zero(),
                        deathtouch_damage: false,
                        attack_ready: c.zone == Zone::Battlefield,
                        id: ObjectId {
                            card: c.id,
                            incarnation: 0,
                        },
                        controller: c.controller,
                        default_controller: c.controller,
                        zone: c.zone,
                        tapped: false,
                    },
                )
            })
            .collect();
        let state = State {
            rules: RulesState {
                tokens: BTreeMap::new(),
                retained_auxiliary_charge: 0,
                typed_fault: None,
                match_state: prepared.match_config.as_ref().map(MatchState::new),
                effects: vec![],
                next_effect_id: 0,
                next_damage_transaction_id: 0,
                pending_triggers: vec![],
                next_trigger_id: 0,
                libraries: prepared.config.initial_libraries(),
                combat: vec![],
                combat_declared: false,
                damage_choices: BTreeMap::new(),
                player_damage_choices: BTreeMap::new(),
                first_strike_participants: BTreeSet::new(),
                orphan_first_strike_blockers: BTreeSet::new(),
                cleanup_repeat: false,
                simultaneous_losses: false,
                losing_players: vec![],
                revision: 0,
                rule_steps: 0,
                players: (0..4)
                    .map(|p| Player {
                        failed_draw: false,
                        life: prepared.config.initial_life(PlayerId(p)),
                        mana: BTreeMap::new(),
                        status: PlayerStatus::Live,
                        last_known: None,
                    })
                    .collect(),
                cards,
                objects,
                active: Some(PlayerId(0)),
                turn_owner: PlayerId(0),
                turn: TurnState {
                    number: 1,
                    step: prepared.config.initial_turn_step(),
                    lands_played: 0,
                },
                original_seating: (0..4).map(PlayerId).collect(),
                commanders: CommanderLedger::new(),
                commander_returns: BTreeSet::new(),
                control_effects: vec![],
                pending_concessions: vec![],
                priority: PlayerId(0),
                passes: vec![],
                next_stack_id: 0,
                stack: vec![],
                frame: Frame::Priority,
                pending: None,
                next_batch_id: 0,
                next_event_sequence: 0,
            },
            knowledge: KnowledgeState {
                visibility: (0..4).map(|_| visibility::Knowledge::default()).collect(),
            },
            audit: AuditState {
                external_inputs: History::default(),
                journal: History::default(),
            },
        };
        let mut game = Self {
            prepared: Arc::new(prepared),
            state,
            trace: Self::initial_trace(),
            executor: Arc::new(()),
            state_token: Arc::new(()),
            in_rule_step: false,
        };
        if let Some(config) = game.prepared.match_config.as_ref() {
            game.state.rules.active = Some(config.starting_player);
            game.state.rules.turn_owner = config.starting_player;
            game.state.rules.priority = config.starting_player;
            game.state.rules.frame = Frame::Setup;
        }
        game.initialize_commanders()
            .expect("admitted commander configuration");
        game.refresh_visibility()
            .expect("admitted initial visibility");
        game
    }
    pub fn fork(&self) -> Self {
        Self {
            prepared: self.prepared.clone(),
            state: self.state.clone(),
            trace: self.trace.clone(),
            executor: Arc::new(()),
            state_token: Arc::new(()),
            in_rule_step: false,
        }
    }
    fn history_charge(&self) -> usize {
        // Charge actual retained allocations, independently of trace mode.
        self.state
            .knowledge
            .visibility
            .iter()
            .fold(self.state.rules.retained_auxiliary_charge, |n, k| {
                n.saturating_add(k.history.retention_charge())
            })
    }
    fn check_history_capacity(&mut self) -> Result<(), String> {
        let requested = self.history_charge();
        if requested > capacities::HISTORY_LIMIT {
            return Err(self.capacity_error(RuleFault::HistoryCapacityExceeded {
                capacity: capacities::HISTORY_LIMIT,
                requested,
            }));
        }
        Ok(())
    }
    /// Trusted host execution boundary; never a player-facing identifier.
    pub fn rule_boundary(&self) -> u64 {
        self.state.rules.rule_steps
    }
    pub fn state_digest(&self) -> Result<String, String> {
        state_digest::state_digest(&self.state)
    }
    pub fn events(&self) -> &[Event] {
        self.trace.events.as_slice()
    }
    pub fn config(&self) -> &ScenarioConfig {
        self.prepared.config()
    }
    /// Borrow the persistent input log without materializing a flat copy.
    /// Use `iter()` for scans and explicitly collect when owned entries are needed.
    pub fn journal(&self) -> &History<AcceptedInput> {
        &self.state.audit.journal
    }
    pub fn object_id(&self, card: CardId) -> Option<ObjectId> {
        self.state.rules.objects.get(&card).map(|o| o.id)
    }
    pub fn zone(&self, object: ObjectId) -> Option<Zone> {
        self.current(object).ok().map(|o| o.zone)
    }
    pub fn mana(&self, player: PlayerId, color: Color) -> Option<Quantity> {
        self.state
            .rules
            .players
            .get(player.0 as usize)
            .map(|p| p.mana.get(&color).cloned().unwrap_or_default())
    }
    pub fn characteristics(&self, object: ObjectId) -> Result<Characteristics, String> {
        self.current(object)?;
        self.derived_characteristics(object)
    }
    // Characteristics supply intrinsic behavior; reminder text never supplies
    // runtime code. CR 305.6 / 113.6 / 605.3b, pinned official rules:
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    /// Colors the object's mana ability can produce. In `commander-mana-v1`
    /// this describes production only, not current activatability: a tapped
    /// or summoning-sick source still reports its symbols. Use the mana
    /// window's offered sources to decide what can be activated now.
    pub fn mana_abilities(&self, object: ObjectId) -> Result<Vec<Color>, String> {
        let o = self.current(object)?;
        let d = self.definition(object.card)?;
        if o.zone == Zone::Battlefield && !d.mana_abilities.is_empty() {
            return Ok(self.mana_colors(o.controller, &d.mana_abilities[0].production));
        }
        if o.zone != Zone::Battlefield || d.intrinsic_zone != Some(FunctioningZone::Battlefield) {
            return Ok(vec![]);
        }
        let c = self.characteristics(object)?;
        Ok(if c.kind == CardKind::Land {
            c.subtypes
                .iter()
                .filter_map(|s| basic_land_color(s))
                .collect()
        } else {
            vec![]
        })
    }
    fn definition(&self, card: CardId) -> Result<&Definition, String> {
        if let Some(token) = self.state.rules.tokens.get(&card) {
            return Ok(&token.definition);
        }
        let physical = self
            .state
            .rules
            .cards
            .get(&card)
            .ok_or("unknown physical card")?;
        self.prepared
            .catalog
            .definition_by_oracle_id(&physical.oracle_id)
            .ok_or_else(|| "missing admitted definition".into())
    }
    fn current(&self, id: ObjectId) -> Result<&Object, String> {
        self.state
            .rules
            .objects
            .get(&id.card)
            .filter(|o| o.id == id)
            .ok_or_else(|| "stale or absent object incarnation".into())
    }
    fn is_battlefield_creature(&self, id: ObjectId) -> bool {
        self.current(id).is_ok_and(|o| o.zone == Zone::Battlefield)
            && self
                .characteristics(id)
                .is_ok_and(|c| c.kind == CardKind::Creature)
    }
    fn top(&self) -> Result<&StackInstance, String> {
        self.state
            .rules
            .stack
            .last()
            .ok_or_else(|| "missing stack frame".into())
    }
    fn program(&self) -> Result<&SpellProgram, String> {
        self.program_for(self.top()?)
    }
    fn program_for(&self, stack: &StackInstance) -> Result<&SpellProgram, String> {
        let definition = self.definition(stack.source.card)?;
        let root = match &stack.kind {
            StackKind::Spell => definition
                .spell
                .as_ref()
                .ok_or_else(|| "missing spell program".to_string()),
            StackKind::Triggered { ability, .. } => definition
                .triggered
                .get(*ability as usize)
                .map(|a| &a.program)
                .ok_or_else(|| "missing triggered program".to_string()),
            StackKind::Activated { ability, .. } => definition
                .activated
                .get(*ability as usize)
                .map(|ability| &ability.program)
                .ok_or_else(|| "missing activated program".to_string()),
        }?;
        self.selected_program(root, stack.mode)
    }
    pub fn decision_for(&self, viewer: PlayerId) -> Result<Option<Request>, String> {
        if self.state.rules.pending.is_some() || !self.state.rules.pending_concessions.is_empty() {
            return Ok(None);
        }
        if matches!(self.state.rules.frame, Frame::Setup) {
            return Ok(self.setup_request()?.filter(|r| r.player() == viewer));
        }
        if let Some(req) = self.damage_allocation_request()? {
            return Ok((req.player() == viewer).then_some(req));
        }
        if let Some(req) = self.modes_request()? {
            return Ok((req.player() == viewer).then_some(req));
        }
        if let Some(req) = self.damage_effect_request()? {
            return Ok((req.player() == viewer).then_some(req));
        }
        if let Some(req) = self.trigger_request()? {
            return Ok((req.player() == viewer).then_some(req));
        }
        if let Some(req) = self.actions_request()? {
            return Ok((req.player() == viewer).then_some(req));
        }
        if let Some(req) = self.combat_request()? {
            return Ok((req.player() == viewer).then_some(req));
        }
        let req = match &self.state.rules.frame {
            Frame::LegendRule { .. } => {
                return Ok(self.legend_request()?.filter(|r| r.player() == viewer));
            }
            Frame::EntityTargets { .. } => Request::EntityTargets {
                decision_id: self.state.rules.revision,
                player: self.top()?.controller,
                occurrences: self.program()?.targets.clone(),
                candidates: self.eligible_entities(),
            },
            Frame::StackTargets { .. } => Request::StackTargets {
                decision_id: self.state.rules.revision,
                player: self.top()?.controller,
                occurrences: self.program()?.targets.clone(),
                candidates: self
                    .eligible_stack_targets_for(&self.program()?.targets[0], Some(self.top()?.id)),
            },
            Frame::TriggerTargets { .. }
                if self.program()?.targets.first().is_some_and(|t| {
                    matches!(
                        t.domain,
                        TargetDomain::ControlledBattlefieldCreature
                            | TargetDomain::BattlefieldPermanent
                            | TargetDomain::BattlefieldCreature
                            | TargetDomain::OwnGraveyardCreatureCard
                    )
                }) =>
            {
                if !self.has_target_domain(self.program()?, self.top()?.controller) {
                    return Ok(None);
                }
                Request::Targets {
                    decision_id: self.state.rules.revision,
                    player: self.top()?.controller,
                    occurrences: self.program()?.targets.clone(),
                    candidates: self
                        .state
                        .rules
                        .objects
                        .values()
                        .filter(|o| self.object_target_legal(self.top().unwrap(), o.id))
                        .map(|o| o.id)
                        .collect(),
                }
            }
            Frame::TriggerTargets { .. }
                if self
                    .program()?
                    .targets
                    .first()
                    .is_some_and(|t| t.domain == TargetDomain::AnyDamageTarget) =>
            {
                Request::EntityTargets {
                    decision_id: self.state.rules.revision,
                    player: self.top()?.controller,
                    occurrences: self.program()?.targets.clone(),
                    candidates: self.eligible_entities(),
                }
            }
            Frame::PlayerTargets { .. } | Frame::TriggerTargets { .. } => Request::PlayerTargets {
                decision_id: self.state.rules.revision,
                player: self.top()?.controller,
                occurrences: self.program()?.targets.clone(),
                candidates: self
                    .eligible_players(self.program()?.targets[0].domain, self.top()?.controller),
            },
            Frame::SacrificeCost { .. }
            | Frame::ForcedSacrifice { .. }
            | Frame::OptionalDamage { .. } => {
                return Ok(self.sacrifice_request()?.filter(|r| r.player() == viewer));
            }
            Frame::ManaAbility { activation } => self.permanent_mana_request(activation)?,
            Frame::SpendMana { .. } => self.spend_mana_request()?,
            Frame::CommanderReturn {
                object,
                destination,
                ..
            } => Request::CommanderReturn {
                previous_choices: self.previous_commander_choices(),
                decision_id: self.state.rules.revision,
                player: self.state.rules.cards[&object.card].owner,
                object: *object,
                destination: *destination,
            },
            Frame::Priority => Request::Priority {
                decision_id: self.state.rules.revision,
                player: self.state.rules.priority,
            },
            Frame::AnnounceX { .. } => Request::AnnounceX {
                decision_id: self.state.rules.revision,
                player: self.top()?.controller,
            },
            Frame::Mana { .. } => Request::Mana {
                decision_id: self.state.rules.revision,
                player: self.top()?.controller,
                cost: self.top()?.total_cost.clone(),
            },
            Frame::Targets { .. } => Request::Targets {
                decision_id: self.state.rules.revision,
                player: self.top()?.controller,
                occurrences: self.program()?.targets.clone(),
                candidates: self
                    .state
                    .rules
                    .objects
                    .values()
                    .filter(|o| self.object_target_legal(self.top().unwrap(), o.id))
                    .map(|o| o.id)
                    .collect(),
            },
            _ => return Ok(None),
        };
        Ok((req.player() == viewer).then_some(req))
    }
    fn validate_response(&self, player: PlayerId, response: &Response) -> Result<(), String> {
        if matches!(self.state.rules.frame, Frame::LegendRule { .. }) {
            return self.validate_legend_response(player, response);
        }
        if let Frame::AnnounceX { .. } = self.state.rules.frame {
            return if matches!(response, Response::AnnounceX { value } if !value.is_negative())
                && self.top()?.controller == player
            {
                Ok(())
            } else {
                Err("invalid X announcement".into())
            };
        }
        if matches!(
            self.state.rules.frame,
            Frame::SacrificeCost { .. }
                | Frame::ForcedSacrifice { .. }
                | Frame::OptionalDamage { .. }
        ) {
            return self.validate_sacrifice_response(player, response);
        }
        if matches!(self.state.rules.frame, Frame::Setup) {
            return self.validate_setup_response(player, response);
        }
        if let Frame::Stabilize { resume, .. } = &self.state.rules.frame {
            let mut validation = self.fork();
            validation.state.rules.frame = *resume.clone();
            return validation.validate_response(player, response);
        }
        if let (Frame::DamageEffects { .. }, Response::DamageEffect { effect, damage }) =
            (&self.state.rules.frame, response)
        {
            return self.validate_damage_effect(player, effect, *damage);
        }
        if let (Frame::DamageAllocation { .. }, Response::DamageAllocation { allocations }) =
            (&self.state.rules.frame, response)
        {
            return self.validate_damage_allocation(player, allocations);
        }
        if let (Frame::Modes { .. }, Response::Modes { mode }) = (&self.state.rules.frame, response)
        {
            return self.validate_mode(player, *mode);
        }
        if let Some(result) = self.validate_actions_response(player, response) {
            return result;
        }
        if let Frame::ManaAbility { activation } = &self.state.rules.frame {
            return self.validate_permanent_mana_response(activation, player, response);
        }
        if let (Frame::SpendMana { .. }, Response::SpendMana { spending }) =
            (&self.state.rules.frame, response)
        {
            return self.validate_spending(player, spending);
        }
        if self.combat_decision_player().is_some() {
            return self.validate_combat_response(player, response);
        }
        match (&self.state.rules.frame, response) {
            (Frame::CommanderReturn { object, .. }, Response::CommanderReturn { .. })
                if self.state.rules.cards[&object.card].owner == player && self.is_live(player) =>
            {
                Ok(())
            }
            (Frame::Priority, Response::PlayLand { card })
                if self.state.rules.priority == player =>
            {
                self.validate_land_play(player, *card)
            }
            (Frame::Priority, Response::Pass) if self.state.rules.priority == player => Ok(()),
            (Frame::Priority, Response::ActivateMana { object })
                if self.state.rules.priority == player =>
            {
                self.validate_mana_activation(player, *object)
            }
            (Frame::Mana { .. }, Response::ActivateMana { object })
                if self.top()?.controller == player =>
            {
                self.validate_mana_activation(player, *object)
            }
            (Frame::Mana { .. }, Response::FinishMana { .. })
                if self.top()?.controller == player =>
            {
                Ok(())
            }
            (Frame::Priority, Response::Cast { card }) if self.state.rules.priority == player => {
                self.validate_cast_permission(player)?;
                let o = self.state.rules.objects.get(card).ok_or("invalid cast")?;
                if !(o.zone == (Zone::Hand { owner: player })
                    || (o.zone == (Zone::Command { owner: player })
                        && self.state.rules.commanders.history(*card).is_some()))
                    || self.state.rules.cards[card].owner != player
                    || self.definition(*card)?.spell.is_none()
                {
                    return Err("invalid or unsupported cast".into());
                }
                let program = self
                    .definition(*card)?
                    .spell
                    .as_ref()
                    .ok_or("missing cast program")?;
                // CR 702.8a / 601.2: intrinsic flash lowers to instant timing;
                // catalog validation binds that permission to the whole spell.
                // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
                if program.timing == SpellTiming::MainPhase && (!self.main_phase_action(player)) {
                    return Err("main-phase spell casting requires active-player main-phase priority and empty stack".into());
                }
                // Restrict targeted proposals to spells with an eligible target;
                // the target-stage request never waits for an impossible choice.
                if program
                    .additional_sacrifice
                    .as_ref()
                    .is_some_and(|f| self.sacrifice_candidates(player, f).is_empty())
                {
                    return Err("no payable sacrifice cost".into());
                }
                if !self.has_target_domain(program, player) {
                    return Err("no legal target for this proposal".into());
                }
                Ok(())
            }
            (
                Frame::Targets { .. } | Frame::TriggerTargets { .. },
                Response::Targets { objects },
            ) if self.top()?.controller == player => {
                if self.program()?.targets.iter().any(|t| {
                    !matches!(
                        t.domain,
                        TargetDomain::ControlledBattlefieldCreature
                            | TargetDomain::BattlefieldPermanent
                            | TargetDomain::BattlefieldCreature
                            | TargetDomain::OwnGraveyardCreatureCard
                    )
                }) || objects.len() != self.program()?.targets.len()
                    || objects
                        .iter()
                        .any(|o| !self.object_target_legal(self.top().unwrap(), *o))
                {
                    return Err("invalid targets".into());
                }
                Ok(())
            }
            _ => Err("invalid or unauthorized decision response".into()),
        }
    }
    pub fn check_response(
        &self,
        player: PlayerId,
        decision_id: u64,
        response: Response,
    ) -> Result<ValidatedResponse, String> {
        if self.state.rules.pending.is_some()
            || decision_id != self.state.rules.revision
            || self.decision_for(player)?.is_none()
        {
            return Err("stale or unauthorized decision".into());
        }
        self.validate_response(player, &response)?;
        Ok(ValidatedResponse {
            executor: self.executor.clone(),
            revision: self.state.rules.revision,
            state_token: self.state_token.clone(),
            input: AcceptedInput {
                decision_id,
                player,
                response,
            },
        })
    }
    /// Submit is a bounded state transition; automatic rules run in advance.
    pub fn submit(&mut self, receipt: ValidatedResponse) -> Result<(), String> {
        if !Arc::ptr_eq(&receipt.executor, &self.executor)
            || !Arc::ptr_eq(&receipt.state_token, &self.state_token)
            || receipt.revision != self.state.rules.revision
            || self.state.rules.pending.is_some()
        {
            return Err("stale or foreign validation receipt".into());
        }
        let revision = self
            .state
            .rules
            .revision
            .checked_add(1)
            .ok_or("structural revision resource limit")?;
        let mut next = self.fork();
        next.state.rules.retained_auxiliary_charge = next
            .state
            .rules
            .retained_auxiliary_charge
            .saturating_add(stored_history::payload_charge(&receipt.input));
        next.state.audit.journal.push(receipt.input.clone());
        next.state.rules.pending = Some(receipt.input);
        next.state.rules.revision = revision;
        next.refresh_visibility()?;
        self.state = next.state;
        self.trace = next.trace;
        self.state_token = Arc::new(());
        Ok(())
    }
    pub fn respond(
        &mut self,
        player: PlayerId,
        decision_id: u64,
        response: Response,
    ) -> Result<(), String> {
        let r = self.check_response(player, decision_id, response)?;
        self.submit(r)
    }
    pub fn advance(&mut self, budget: Budget) -> Advance {
        self.advance_internal::<false, true>(budget, &mut vec![])
    }
    fn advance_internal<const REPORT: bool, const COALESCE: bool>(
        &mut self,
        mut budget: Budget,
        processed: &mut Vec<InputProcessingFact>,
    ) -> Advance {
        loop {
            if let Frame::Finished { winner } = self.state.rules.frame {
                return Advance::Finished { winner };
            }
            if let Frame::Fault { reason } = &self.state.rules.frame {
                return Advance::Fault(reason.clone());
            }
            if matches!(self.state.rules.frame, Frame::Draw) {
                return Advance::Draw;
            }
            if self.state.rules.pending.is_none() && self.state.rules.pending_concessions.is_empty()
            {
                for p in self.live_players() {
                    match self.decision_for(p) {
                        Ok(Some(r)) => return Advance::AwaitingDecision(r),
                        Ok(None) => {}
                        Err(e) => return self.enter_fault(e, None),
                    }
                }
            }
            if !budget.consume() {
                return Advance::Suspended {
                    required_minimum: 1,
                };
            }
            // A rules step commits atomically. Resource/invariant failures retain
            // its pre-step state and become faults, never wins/losses/draws.
            let input_report = REPORT.then(|| advance_report::StepInputCapture::capture(self));
            let mut next = self.fork();
            next.in_rule_step = COALESCE;
            if let Err(reason) = next.check_history_capacity().and_then(|()| next.step()) {
                return self.enter_fault(reason, next.state.rules.typed_fault);
            }
            let Some(steps) = next.state.rules.rule_steps.checked_add(1) else {
                return self.enter_fault("execution boundary resource limit".into(), None);
            };
            next.state.rules.rule_steps = steps;
            if let Err(reason) = next
                .refresh_visibility()
                .and_then(|()| next.check_history_capacity())
            {
                return self.enter_fault(reason, next.state.rules.typed_fault);
            }
            self.state = next.state;
            self.trace = next.trace;
            self.state_token = Arc::new(());
            if let Some(report) = input_report {
                report.committed(self, processed);
            }
        }
    }
    fn enter_fault(&mut self, reason: String, typed: Option<RuleFault>) -> Advance {
        self.state.rules.typed_fault = typed;
        self.state.rules.frame = Frame::Fault {
            reason: reason.clone(),
        };
        self.state_token = Arc::new(());
        Advance::Fault(reason)
    }
    fn step(&mut self) -> Result<(), String> {
        self.check_history_capacity()?;
        if let Some(player) = self.state.rules.pending_concessions.first().copied() {
            self.state.rules.pending_concessions.remove(0);
            return self.apply_departure(player);
        }
        match self.state.rules.frame.clone() {
            Frame::LegendRule { .. } => self.legend_step()?,
            Frame::SacrificeCost { .. }
            | Frame::ForcedSacrifice { .. }
            | Frame::OptionalDamage { .. } => self.sacrifice_step()?,
            Frame::Setup => self.setup_step()?,
            Frame::DamageAllocation { .. } => self.damage_allocation_step()?,
            Frame::AnnounceX { cast } => self.bind_x(cast)?,
            Frame::Modes { cast } => self.bind_mode(cast)?,
            Frame::DamageEffects { .. } => self.damage_effect_step()?,
            Frame::Priority => {
                let input = self
                    .state
                    .rules
                    .pending
                    .take()
                    .ok_or("missing accepted priority input")?;
                match input.response {
                    Response::ActivateAbility { object, ability } => {
                        self.activate_ability(input.player, input.decision_id, object, ability)?
                    }
                    Response::PlayLand { card } => {
                        self.play_land(input.player, input.decision_id, card)?;
                    }
                    Response::ActivateMana { object } => {
                        if self.is_permanent_mana(object) {
                            self.begin_permanent_mana(&input, object, Frame::Priority)?;
                        } else {
                            self.activate_intrinsic_mana(&input, object)?;
                            self.state.rules.passes.clear();
                            self.stabilize()?;
                        }
                    }
                    Response::Cast { card } => {
                        self.validate_cast_permission(input.player)?;
                        self.check_stack_capacity(1)?;
                        let cast_from = self
                            .state
                            .rules
                            .objects
                            .get(&card)
                            .ok_or("missing cast source")?
                            .zone;
                        let total_cost = self.casting_cost(card, input.player, cast_from)?;
                        let original_object = self
                            .state
                            .rules
                            .objects
                            .get(&card)
                            .ok_or("missing cast source")?
                            .id;
                        let id = StackId(self.state.rules.next_stack_id);
                        self.state.rules.next_stack_id = self
                            .state
                            .rules
                            .next_stack_id
                            .checked_add(1)
                            .ok_or("stack ID resource limit")?;
                        self.record_event(Event::SpellProposed { card, stack_id: id })?;
                        let source = self
                            .move_object_for(
                                original_object,
                                Zone::Stack,
                                BatchCause::Casting {
                                    stack_id: id,
                                    player: input.player,
                                    source: original_object,
                                },
                            )?
                            .successor;
                        let effect_source_handles =
                            { self.capture_trigger_source_handles(source)? };
                        self.state.rules.stack.push(StackInstance {
                            x: None,
                            death_controller: None,
                            sacrificed: None,
                            mode: None,
                            effect_source_handles,
                            source_facts: { Some(self.object_before(source)?) },
                            entity_targets: vec![],
                            announced_entity_targets: vec![],
                            stack_targets: vec![],
                            announced_stack_targets: vec![],
                            kind: StackKind::Spell,
                            player_targets: vec![],
                            id,
                            source,
                            controller: input.player,
                            targets: vec![],
                            announced_targets: vec![vec![]; 4],
                            paid: None,
                            total_cost,
                            cast_from,
                        });
                        let cast = Casting {
                            sacrifice: None,
                            origin: AnnouncementOrigin::Spell,
                            spending: None,
                            stack_id: id,
                            original_object,
                            prior_passes: self.state.rules.passes.clone(),
                            mana_activations: vec![],
                            undo_mana: false,
                        };
                        self.state.rules.frame = if self.definition(card)?.x_mana_cost {
                            Frame::AnnounceX { cast }
                        } else if !self
                            .definition(card)?
                            .spell
                            .as_ref()
                            .ok_or("missing spell")?
                            .modes
                            .is_empty()
                        {
                            Frame::Modes { cast }
                        } else if self.program()?.targets.is_empty() {
                            Frame::Mana { cast }
                        } else if matches!(
                            self.program()?.targets[0].domain,
                            TargetDomain::StackSpell
                                | TargetDomain::StackActivatedOrTriggeredAbility
                        ) {
                            Frame::StackTargets { cast }
                        } else if self.program()?.targets[0].domain == TargetDomain::AnyDamageTarget
                        {
                            Frame::EntityTargets { cast }
                        } else if matches!(
                            self.program()?.targets[0].domain,
                            TargetDomain::OpponentPlayer | TargetDomain::AnyPlayer
                        ) {
                            Frame::PlayerTargets { cast }
                        } else {
                            Frame::Targets { cast }
                        };
                        self.state.rules.passes.clear();
                    }
                    Response::Pass => {
                        self.record_event(Event::PriorityPassed {
                            player: input.player,
                        })?;
                        self.state.rules.passes.push(input.player);
                        // CR 117.4 applies to the current live player set.
                        if self.all_live_passed() {
                            self.begin_resolution()?;
                        } else {
                            self.state.rules.priority = self
                                .next_live(input.player)
                                .ok_or("no live priority recipient")?;
                            self.stabilize()?;
                        }
                    }
                    _ => return Err("response/frame invariant".into()),
                }
            }
            Frame::Targets { cast } => {
                let input = self
                    .state
                    .rules
                    .pending
                    .take()
                    .ok_or("missing accepted target input")?;
                let Response::Targets { objects } = input.response else {
                    return Err("target response/frame invariant".into());
                };
                let announced = (0..4)
                    .map(|v| {
                        objects
                            .iter()
                            .map(|object| self.handle_for(PlayerId(v), *object))
                            .collect::<Result<Vec<_>, _>>()
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let top = self.state.rules.stack.last_mut().ok_or("missing stack")?;
                top.targets = objects;
                top.announced_targets = announced;
                self.state.rules.frame = Frame::Mana { cast };
            }
            Frame::Mana { mut cast } => {
                let input = self
                    .state
                    .rules
                    .pending
                    .take()
                    .ok_or("missing accepted mana input")?;
                match input.response {
                    Response::ActivateMana { object } => {
                        if self.is_permanent_mana(object) {
                            self.begin_permanent_mana(&input, object, Frame::Mana { cast })?;
                        } else {
                            let receipt = self.activate_intrinsic_mana(&input, object)?;
                            cast.mana_activations.push(receipt);
                            self.state.rules.frame = Frame::Mana { cast };
                        }
                    }
                    Response::FinishMana { undo_mana } => {
                        cast.undo_mana = undo_mana;
                        self.state.rules.frame = self.finish_mana_frame(cast)?;
                    }
                    _ => return Err("mana response/frame invariant".into()),
                }
            }
            Frame::ManaAbility { activation } => self.permanent_mana_step(*activation)?,
            Frame::Payment { cast } => self.payment_step(cast)?,
            Frame::SpendMana { cast } => self.spend_mana_step(cast)?,
            Frame::TriggerOrder { .. } | Frame::TriggerTargets { .. } => self.trigger_step()?,
            Frame::EntityTargets { .. }
            | Frame::PlayerTargets { .. }
            | Frame::StackTargets { .. }
            | Frame::ResolutionChoice { .. } => self.actions_step()?,
            Frame::Resolve { mut frame } => {
                let stack = frame.spell.clone();
                let program = self.program_for(&stack)?.clone();
                // One spell-wide legality check before any block resolves.
                // CR 608.2b: if all targets are illegal, the entire spell does
                // not resolve, including its otherwise untargeted blocks.
                if frame
                    .condition_check
                    .as_ref()
                    .is_some_and(|check| !check.passed())
                    || (!frame.legal_targets.is_empty() && !frame.legal_targets.iter().any(|l| *l))
                {
                    self.finish_spell(&stack, false)?;
                } else if let Some(instruction) = program.instructions.get(frame.cursor as usize) {
                    match instruction {
                        Instruction::ReturnToOwnerHand {
                            target_slot,
                            source_block,
                        }
                        | Instruction::PutOnTopOwnerLibrary {
                            target_slot,
                            source_block,
                        } => {
                            let target = stack.targets[*target_slot as usize];
                            let outcome = if !frame.legal_targets[*target_slot as usize] {
                                InstructionOutcome::IllegalTarget { target }
                            } else if self.current(target).is_err() {
                                InstructionOutcome::ObjectNoLongerPresent { target }
                            } else {
                                let owner = self.state.rules.cards[&target.card].owner;
                                let destination = if matches!(
                                    instruction,
                                    Instruction::PutOnTopOwnerLibrary { .. }
                                ) {
                                    Zone::Library { owner }
                                } else {
                                    Zone::Hand { owner }
                                };
                                if self.state.rules.commanders.history(target.card).is_some() {
                                    self.state.rules.frame = Frame::CommanderReturn {
                                        object: target,
                                        destination,
                                        continuation: Box::new(CommanderContinuation::SpellMove {
                                            frame,
                                            source_block: *source_block,
                                        }),
                                    };
                                    return Ok(());
                                }
                                InstructionOutcome::Moved {
                                    movement: self.move_instruction_object(
                                        target,
                                        destination,
                                        BatchCause::Instruction {
                                            stack_id: stack.id,
                                            player: stack.controller,
                                            source: stack.source,
                                            cursor: frame.cursor,
                                            source_block: *source_block,
                                        },
                                    )?,
                                }
                            };
                            frame.outcomes.push(outcome);
                        }
                        other => return self.execute_action_instruction(frame, other.clone()),
                    }
                    frame.cursor = frame
                        .cursor
                        .checked_add(1)
                        .ok_or("instruction cursor resource limit")?;
                    self.state.rules.frame = Frame::Resolve { frame };
                } else {
                    self.finish_spell(&stack, true)?;
                }
            }
            Frame::CommanderReturn {
                object,
                destination,
                continuation,
            } => self.answer_commander_return(object, destination, *continuation)?,
            Frame::Attackers
            | Frame::Blockers { .. }
            | Frame::Damage
            | Frame::DamageChoice { .. }
            | Frame::Discard { .. }
            | Frame::CleanupClear
            | Frame::Stabilize { .. } => self.combat_step()?,
            Frame::Draw => {}
            Frame::Transition => self.transition_turn_step()?,
            Frame::EnterStep => self.enter_turn_step()?,
            Frame::Fault { reason } => return Err(reason),
            Frame::Finished { .. } => {}
        }
        Ok(())
    }
    fn finish_spell(&mut self, stack: &StackInstance, resolved: bool) -> Result<(), String> {
        if !stack.kind.is_spell() {
            return self.finish_ability(stack, resolved);
        }
        let owner = self.state.rules.cards[&stack.source.card].owner;
        if self.current(stack.source).is_ok() {
            if self.top()?.id != stack.id {
                return Err("completion is not the top stack entry".into());
            }
            // CR 608.3a: a resolving permanent spell becomes a permanent;
            // targetless spells do not fail the targeted-spell legality test.
            // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
            let destination =
                if resolved && self.program_for(stack)?.resolution == SpellResolution::Permanent {
                    Zone::Battlefield
                } else {
                    Zone::Graveyard { owner }
                };
            self.move_object_for(
                stack.source,
                destination,
                BatchCause::SpellCompletion {
                    stack_id: stack.id,
                    player: stack.controller,
                    source: stack.source,
                },
            )?;
            if destination == Zone::Battlefield
                && self.definition(stack.source.card)?.characteristics.subtypes == ["Aura"]
            {
                let object = self.state.rules.objects[&stack.source.card].id;
                self.commit(
                    ProposedEvent::Attach {
                        object,
                        target: stack.targets.first().copied(),
                    },
                    BatchCause::SpellCompletion {
                        stack_id: stack.id,
                        player: stack.controller,
                        source: stack.source,
                    },
                )?;
            }
            self.state.rules.stack.pop();
        } else if self.is_live(owner) || self.state.rules.stack.iter().any(|s| s.id == stack.id) {
            return Err("unsupported resolving-source movement".into());
        }
        self.record_event(Event::SpellCompleted {
            stack_id: stack.id,
            resolved,
        })?;
        self.state.rules.frame = Frame::Priority;
        self.state.rules.priority = self
            .state
            .rules
            .active
            .or_else(|| self.next_live(self.state.rules.turn_owner))
            .ok_or("no live player after resolution")?;
        self.stabilize()
    }
    fn payment_pool(
        &self,
        player: PlayerId,
        cost: &ManaCost,
    ) -> Result<BTreeMap<Color, Quantity>, payments::PaymentError> {
        use payments::PaymentError;
        if cost.generic.is_negative() {
            return Err(PaymentError::Invariant("negative mana cost"));
        }
        let mut pool = self
            .state
            .rules
            .players
            .get(player.0 as usize)
            .ok_or(PaymentError::Invariant("missing paying player"))?
            .mana
            .clone();
        if pool.values().any(Quantity::is_negative) {
            return Err(PaymentError::Invariant("negative mana pool"));
        }
        for color in &cost.colored {
            let n = pool.entry(*color).or_default();
            if *n < Quantity::from(1) {
                return Err(PaymentError::InsufficientMana);
            }
            n.subtract(&Quantity::from(1));
        }
        let mut generic = cost.generic.clone();
        for n in pool.values_mut() {
            let spent = std::cmp::min(n.clone(), generic.clone());
            n.subtract(&spent);
            generic.subtract(&spent);
        }
        if generic > Quantity::zero() {
            return Err(PaymentError::InsufficientMana);
        }
        Ok(pool)
    }
    fn stabilize(&mut self) -> Result<(), String> {
        if matches!(self.state.rules.frame, Frame::Priority) {
            self.schedule_stabilization(Frame::Priority, false);
        }
        Ok(())
    }

    pub fn snapshot(&self) -> Result<String, String> {
        self.snapshot_with_digest().map(|(json, _)| json)
    }
    fn snapshot_ref<'a>(&'a self, state_sha256: &'a str) -> SnapshotRef<'a> {
        SnapshotRef {
            snapshot_version: 19,
            build_id: BUILD_ID,
            catalog_digest: &self.prepared.catalog_digest,
            profile_digest: &self.prepared.profile_digest,
            state_sha256,
            state: &self.state,
            trace: &self.trace,
        }
    }
    /// Snapshot JSON and its semantic digest, computed once at this boundary.
    /// String convenience still retains the complete output allocation.
    pub fn snapshot_with_digest(&self) -> Result<(String, String), String> {
        let state_sha256 = self.state_digest()?;
        let json = snapshot_codec::encode(&self.snapshot_ref(&state_sha256))?;
        Ok((json, state_sha256))
    }
    /// Write every canonical snapshot byte without a complete output String.
    /// The sink may contain a prefix on Err; the host owns its publication.
    pub fn write_snapshot<W: std::io::Write>(
        &self,
        sink: &mut W,
    ) -> Result<SnapshotWriteInfo, String> {
        let state_sha256 = self.state_digest()?;
        let raw = snapshot_codec::write_counted(&self.snapshot_ref(&state_sha256), sink)?;
        Ok(SnapshotWriteInfo {
            bytes: raw.bytes,
            snapshot_sha256: raw.sha256,
            state_sha256,
        })
    }
    fn read_snapshot(prepared: Prepared, json: &str) -> Result<(Self, String), String> {
        Self::admit_decoded_snapshot(prepared, strict_read(json)?)
    }
    fn read_snapshot_from_reader<R: std::io::Read + std::io::Seek>(
        prepared: Prepared,
        source: &mut R,
    ) -> Result<(Self, String), String> {
        Self::admit_decoded_snapshot(prepared, strict_read_seekable(source)?)
    }
    // Exactly one typed verification body for string and seekable input. No
    // Game is returned before unchanged identity/hash/invariant checks pass.
    fn admit_decoded_snapshot(prepared: Prepared, s: Snapshot) -> Result<(Self, String), String> {
        if s.snapshot_version != 19
            || s.build_id != BUILD_ID
            || s.catalog_digest != prepared.catalog_digest
            || s.profile_digest != prepared.profile_digest
            || state_digest::state_digest(&s.state)? != s.state_sha256
        {
            return Err("snapshot version/build/catalog/profile/hash mismatch".into());
        }
        let game = Self {
            prepared: Arc::new(prepared),
            state: s.state,
            trace: s.trace,
            executor: Arc::new(()),
            state_token: Arc::new(()),
            in_rule_step: false,
        };
        game.validate_state()?;
        Ok((game, s.state_sha256))
    }
    fn prove_decoded_snapshot((game, digest): (Self, String)) -> Result<Self, String> {
        SnapshotVerifier::new(game.prepared.as_ref().clone(), game.trace.mode)
            .verify_game(game, digest)
    }
    pub fn restore(prepared: Prepared, json: &str) -> Result<Self, String> {
        Self::prove_decoded_snapshot(Self::read_snapshot(prepared, json)?)
    }
    /// Strict duplicate scan and typed decode bind the same complete raw bytes
    /// from current offset through EOF. Reader position on Err is unspecified.
    /// All ordinary identity/hash/invariant/accepted-input replay proofs remain.
    pub fn restore_from_reader<R: std::io::Read + std::io::Seek>(
        prepared: Prepared,
        source: &mut R,
    ) -> Result<Self, String> {
        Self::prove_decoded_snapshot(Self::read_snapshot_from_reader(prepared, source)?)
    }
    fn validate_state(&self) -> Result<(), String> {
        self.validate_match_state()?;
        let s = &self.state;
        if s.rules.players.len() != 4
            || s.knowledge.visibility.len() != 4
            || s.rules.turn_owner.0 >= 4
            || s.rules.turn.number == 0
            || s.rules.turn.lands_played > 1
            || s.rules.active.is_some_and(|p| !self.is_live(p))
            || s.rules.original_seating != (0..4).map(PlayerId).collect::<Vec<_>>()
            || s.rules.priority.0 >= 4
            || (!self.is_live(s.rules.priority) && !matches!(s.rules.frame, Frame::Draw))
            || s.rules.passes.len() > self.live_players().len()
            || s.rules.passes.iter().any(|p| !self.is_live(*p))
            || s.rules.passes.iter().collect::<BTreeSet<_>>().len() != s.rules.passes.len()
            || s.rules.cards.len() != self.prepared.config.cards.len() + s.rules.tokens.len()
            || s.rules.objects.len() > s.rules.cards.len()
            || s.rules.stack.len() > self.stack_limit()
        {
            return Err("snapshot seating/table invariant".into());
        }
        for setup in &self.prepared.config.cards {
            let card = s
                .rules
                .cards
                .get(&setup.id)
                .ok_or("snapshot missing card")?;
            if card.owner != setup.owner || card.oracle_id != setup.oracle_id {
                return Err("snapshot physical identity invariant".into());
            }
            let Some(o) = s.rules.objects.get(&setup.id) else {
                if self.is_live(setup.owner) {
                    return Err("snapshot missing live owner's object".into());
                }
                continue;
            };
            if !self.is_live(card.owner) {
                return Err("departed owner still has an object".into());
            }
            if o.id.card != setup.id
                || o.controller.0 >= 4
                || o.default_controller.0 >= 4
                || !self.is_live(o.controller)
                || (o.tapped && o.zone != Zone::Battlefield)
                || (o.deathtouch_damage && (o.zone != Zone::Battlefield))
            {
                return Err("snapshot object invariant".into());
            }
            if let Zone::Hand { owner } | Zone::Graveyard { owner } = o.zone
                && owner != card.owner
            {
                return Err("snapshot zone-owner invariant".into());
            }
        }
        for o in s.rules.objects.values() {
            if let Some(target) = o.attached_to
                && (o.zone != Zone::Battlefield
                    || o.id == target
                    || !self
                        .definition(o.id.card)?
                        .characteristics
                        .subtypes
                        .iter()
                        .any(|s| s == "Equipment" || s == "Aura")
                    || self.definition(target.card)?.characteristics.kind != CardKind::Creature)
            {
                return Err("snapshot attachment invariant".into());
            }
        }
        self.validate_tokens_state()?;
        self.validate_trigger_state()?;
        self.validate_effect_state()?;
        self.validate_choices_state()?;
        self.validate_commander_state()?;
        self.validate_actions_state()?;
        for p in &s.rules.players {
            if (p.status == PlayerStatus::Left) != p.last_known.is_some()
                || p.last_known
                    .as_ref()
                    .is_some_and(|last| last.life != p.life || last.mana != p.mana)
            {
                return Err("snapshot departed-player history invariant".into());
            }
            if p.mana.values().any(|n| n.is_negative()) {
                return Err("snapshot negative mana".into());
            }
        }
        let ids: BTreeSet<_> = s.rules.stack.iter().map(|entry| entry.id).collect();
        let sources: BTreeSet<_> = s
            .rules
            .stack
            .iter()
            .filter(|entry| entry.kind.is_spell())
            .map(|entry| entry.source.card)
            .collect();
        if ids.len() != s.rules.stack.len()
            || sources.len()
                != s.rules
                    .stack
                    .iter()
                    .filter(|entry| entry.kind.is_spell())
                    .count()
            || !s.rules.stack.windows(2).all(|w| w[0].id < w[1].id)
        {
            return Err("snapshot duplicate or unordered stack identity".into());
        }
        for (index, stack) in s.rules.stack.iter().enumerate() {
            if stack.id.0 >= s.rules.next_stack_id
                || !self.is_live(stack.controller)
                || (stack.kind.is_spell() && self.current(stack.source)?.zone != Zone::Stack)
            {
                return Err("snapshot stack identity invariant".into());
            }
            self.validate_stack_bindings(stack)?;
            let n = self.program_for(stack)?.targets.len();
            if stack.announced_targets.len() != 4
                || stack
                    .announced_targets
                    .iter()
                    .any(|a| a.len() != stack.targets.len())
            {
                return Err("snapshot target announcement invariant".into());
            }
            let incomplete_top = index + 1 == s.rules.stack.len()
                && (matches!(
                    self.underlying_mana_frame(&s.rules.frame),
                    Frame::TriggerTargets { .. }
                        | Frame::AnnounceX { .. }
                        | Frame::Modes { .. }
                        | Frame::EntityTargets { .. }
                        | Frame::Targets { .. }
                        | Frame::StackTargets { .. }
                        | Frame::PlayerTargets { .. }
                        | Frame::Mana { .. }
                        | Frame::SpendMana { .. }
                        | Frame::SacrificeCost { .. }
                        | Frame::Payment { .. }
                ) || (matches!(
                    self.underlying_mana_frame(&s.rules.frame),
                    Frame::Finished { .. } | Frame::Fault { .. }
                ) && stack.paid.is_none()));
            if !incomplete_top
                && (self.bound_target_count(stack) != n
                    || stack.paid.as_ref() != Some(&stack.total_cost))
            {
                return Err("snapshot completed lower/top cast invariant".into());
            }
        }
        let stack_cards: BTreeSet<_> = s
            .rules
            .stack
            .iter()
            .filter(|entry| entry.kind.is_spell())
            .map(|entry| entry.source.card)
            .collect();
        if s.rules
            .objects
            .values()
            .any(|o| (o.zone == Zone::Stack) != stack_cards.contains(&o.id.card))
        {
            return Err("snapshot stack/zone invariant".into());
        }
        self.validate_sacrifice_state()?;
        self.validate_nested_mana_frame(&s.rules.frame)?;
        match self.underlying_mana_frame(&s.rules.frame) {
            Frame::AnnounceX { cast }
            | Frame::Modes { cast }
            | Frame::EntityTargets { cast }
            | Frame::Targets { cast }
            | Frame::StackTargets { cast }
            | Frame::PlayerTargets { cast }
            | Frame::Mana { cast }
            | Frame::SpendMana { cast }
            | Frame::SacrificeCost { cast }
            | Frame::Payment { cast } => {
                let top = self.top()?;
                if cast.origin.is_spell() != top.kind.is_spell()
                    || cast.stack_id != top.id
                    || cast.original_object.card != top.source.card
                    || if cast.origin.is_spell() {
                        cast.original_object.incarnation.checked_add(1)
                            != Some(top.source.incarnation)
                    } else {
                        cast.original_object != top.source
                    }
                    || top.paid.is_some()
                {
                    return Err("snapshot casting frame invariant".into());
                }
                let sources: BTreeSet<_> = cast.mana_activations.iter().map(|a| a.object).collect();
                if sources.len() != cast.mana_activations.len()
                    || cast.mana_activations.len() > self.prepared.config.cards.len()
                    || cast.mana_activations.iter().any(|a| {
                        a.player != top.controller
                            || (a.permanent.is_none() && a.amount != Quantity::from(1))
                    })
                    || (matches!(
                        self.underlying_mana_frame(&s.rules.frame),
                        Frame::AnnounceX { .. }
                            | Frame::Modes { .. }
                            | Frame::EntityTargets { .. }
                            | Frame::Targets { .. }
                            | Frame::PlayerTargets { .. }
                            | Frame::StackTargets { .. }
                    ) && (!cast.mana_activations.is_empty() || cast.undo_mana))
                    || (matches!(
                        self.underlying_mana_frame(&s.rules.frame),
                        Frame::Mana { .. }
                    ) && cast.undo_mana)
                {
                    return Err("snapshot casting mana receipt invariant".into());
                }
                if matches!(
                    self.underlying_mana_frame(&s.rules.frame),
                    Frame::AnnounceX { .. }
                        | Frame::Modes { .. }
                        | Frame::EntityTargets { .. }
                        | Frame::Targets { .. }
                        | Frame::PlayerTargets { .. }
                        | Frame::StackTargets { .. }
                ) && self.bound_target_count(top) != 0
                {
                    return Err("snapshot announcement target invariant".into());
                }
                if matches!(
                    self.underlying_mana_frame(&s.rules.frame),
                    Frame::Mana { .. }
                        | Frame::SpendMana { .. }
                        | Frame::SacrificeCost { .. }
                        | Frame::Payment { .. }
                ) && self.bound_target_count(top) != self.program()?.targets.len()
                {
                    return Err("snapshot payment target invariant".into());
                }
            }
            Frame::Resolve { frame }
            | Frame::ResolutionChoice { frame, .. }
            | Frame::ForcedSacrifice { frame, .. }
            | Frame::OptionalDamage { frame } => self.validate_resolving(frame)?,
            _ => {}
        }
        if let Some(pending) = &s.rules.pending {
            if pending.decision_id >= s.rules.revision || s.audit.journal.last() != Some(pending) {
                return Err("snapshot pending-input invariant".into());
            }
            if !matches!(s.rules.frame, Frame::Fault { .. }) {
                self.validate_response(pending.player, &pending.response)?;
            }
        }
        self.validate_legend_state()?;
        self.validate_combat_state()
    }
    pub fn replay(prepared: Prepared, journal: &ReplayJournal) -> Result<Self, String> {
        Self::replay_to(prepared, journal, None, TraceMode::Detailed)
    }
}

#[cfg(test)]
impl Game {
    /// Historical scenario drivers now answer the common explicit spending request.
    fn complete_test_mana(&mut self) {
        if let Advance::AwaitingDecision(Request::SpendMana {
            player,
            decision_id,
            cost,
            available,
        }) = self.advance(Budget::unlimited())
        {
            let remaining = self.payment_pool(player, &cost).unwrap();
            let spending = available
                .into_iter()
                .filter_map(|(color, mut amount)| {
                    amount.subtract(&remaining.get(&color).cloned().unwrap_or_default());
                    (amount > Quantity::zero()).then_some((color, amount))
                })
                .collect();
            self.respond(player, decision_id, Response::SpendMana { spending })
                .unwrap();
            self.advance(Budget::unlimited());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn prepared() -> Prepared {
        Prepared::admit(
            Catalog::read(include_str!("../../../tests/fixtures/expected-ir.json")).unwrap(),
            ScenarioConfig::land_return(),
        )
        .unwrap()
    }
    fn respond(game: &mut Game, response: Response) {
        let finish = matches!(response, Response::FinishMana { .. });
        let Advance::AwaitingDecision(r) = game.advance(Budget::unlimited()) else {
            panic!("expected decision")
        };
        game.respond(r.player(), r.decision_id(), response).unwrap();
        if finish {
            game.complete_test_mana();
        }
    }
    fn cast(game: &mut Game) {
        respond(
            game,
            Response::ActivateMana {
                object: game.object_id(CardId(0)).unwrap(),
            },
        );
        respond(game, Response::Cast { card: CardId(1) });
    }
    fn complete(game: &mut Game) {
        respond(
            game,
            Response::Targets {
                objects: vec![game.object_id(CardId(2)).unwrap()],
            },
        );
        respond(game, Response::FinishMana { undo_mana: false });
        for _ in 0..4 {
            respond(game, Response::Pass);
        }
        assert!(matches!(
            game.advance(Budget::unlimited()),
            Advance::AwaitingDecision(_)
        ));
    }
    #[test]
    fn complete_source_scenario_preserves_owner_and_incarnations() {
        let mut game = Game::new(prepared());
        cast(&mut game);
        complete(&mut game);
        assert_eq!(
            game.state.rules.objects[&CardId(2)].zone,
            Zone::Hand { owner: PlayerId(1) }
        );
        assert_eq!(game.state.rules.objects[&CardId(2)].id.incarnation, 1);
        assert_eq!(
            game.state.rules.objects[&CardId(1)].zone,
            Zone::Graveyard { owner: PlayerId(0) }
        );
        assert_eq!(game.state.rules.objects[&CardId(1)].id.incarnation, 2);
        assert_eq!(game.mana(PlayerId(0), Color::U), Some(Quantity::zero()));
        assert_eq!(game.state.rules.priority, PlayerId(0));
        assert_eq!(game.events().len(), 13);
    }
    #[test]
    fn snapshot_fork_and_replay_agree_and_receipts_do_not_cross_executors() {
        let p = prepared();
        let mut game = Game::new(p.clone());
        cast(&mut game);
        let _ = game.advance(Budget::unlimited());
        let mut restored = Game::restore(p.clone(), &game.snapshot().unwrap()).unwrap();
        let mut fork = game.fork();
        let response = Response::Targets {
            objects: vec![game.object_id(CardId(2)).unwrap()],
        };
        let req = game.decision_for(PlayerId(0)).unwrap().unwrap();
        let receipt = game
            .check_response(PlayerId(0), req.decision_id(), response)
            .unwrap();
        assert!(fork.submit(receipt).is_err());
        complete(&mut game);
        complete(&mut restored);
        complete(&mut fork);
        assert_eq!(game.state, restored.state);
        assert_eq!(game.state, fork.state);
        let replay = Game::replay(p, &game.replay_journal()).unwrap();
        assert_eq!(game.state, replay.state);
    }
    #[test]
    fn queries_and_invalid_responses_leave_state_unchanged() {
        let mut game = Game::new(prepared());
        let before = game.state_digest().unwrap();
        for _ in 0..3 {
            for viewer in 0..4 {
                let _ = game.decision_for(PlayerId(viewer)).unwrap();
            }
            let _ = game
                .characteristics(game.object_id(CardId(2)).unwrap())
                .unwrap();
            let _ = game
                .mana_abilities(game.object_id(CardId(0)).unwrap())
                .unwrap();
        }
        assert!(game.respond(PlayerId(1), 0, Response::Pass).is_err());
        assert!(game.respond(PlayerId(0), 99, Response::Pass).is_err());
        assert!(
            game.respond(
                PlayerId(0),
                0,
                Response::ActivateMana {
                    object: ObjectId {
                        card: CardId(0),
                        incarnation: 9
                    }
                }
            )
            .is_err()
        );
        assert_eq!(game.state_digest().unwrap(), before);
        cast(&mut game);
        let _ = game.advance(Budget::unlimited());
        let before = game.state_digest().unwrap();
        assert!(
            game.respond(
                PlayerId(0),
                game.state.rules.revision,
                Response::Targets {
                    objects: vec![ObjectId {
                        card: CardId(2),
                        incarnation: 9
                    }]
                }
            )
            .is_err()
        );
        assert_eq!(game.state_digest().unwrap(), before);
    }
    #[test]
    fn payment_reversal_retains_inputs_and_original_incarnation() {
        let mut game = Game::new(prepared());
        let original = game.object_id(CardId(1)).unwrap();
        respond(&mut game, Response::Cast { card: CardId(1) });
        respond(
            &mut game,
            Response::Targets {
                objects: vec![ObjectId {
                    card: CardId(2),
                    incarnation: 0,
                }],
            },
        );
        respond(&mut game, Response::FinishMana { undo_mana: true });
        let _ = game.advance(Budget::unlimited());
        assert_eq!(game.zone(original), Some(Zone::Hand { owner: PlayerId(0) }));
        assert!(game.state.rules.stack.is_empty());
        assert_eq!(game.journal().len(), 3);
        assert!(matches!(
            game.events().last(),
            Some(Event::CastReversed { .. })
        ));
        let replay = Game::replay(prepared(), &game.replay_journal()).unwrap();
        assert_eq!(replay.state, game.state);
    }
    #[test]
    fn pending_accepted_input_survives_restore_and_rehashed_impossible_state_rejects() {
        let p = prepared();
        let mut game = Game::new(p.clone());
        let Advance::AwaitingDecision(request) = game.advance(Budget::unlimited()) else {
            panic!("expected decision")
        };
        game.respond(
            PlayerId(0),
            request.decision_id(),
            Response::ActivateMana {
                object: ObjectId {
                    card: CardId(0),
                    incarnation: 0,
                },
            },
        )
        .unwrap();
        let mut restored = Game::restore(p.clone(), &game.snapshot().unwrap()).unwrap();
        assert_eq!(game.state, restored.state);
        let _ = game.advance(Budget::unlimited());
        let _ = restored.advance(Budget::unlimited());
        assert_eq!(game.state, restored.state);
        let mut snapshot: Snapshot = strict_read(&game.snapshot().unwrap()).unwrap();
        snapshot.state.rules.players[0]
            .mana
            .insert(Color::U, Quantity::from(99));
        snapshot.state_sha256 = digest(&snapshot.state).unwrap();
        assert!(Game::restore(p, &canonical(&snapshot).unwrap()).is_err());
    }
    #[test]
    fn forest_intrinsic_is_derived_and_partial_multiblock_legality_keeps_other_block() {
        let mut game = Game::new(prepared());
        game.state
            .rules
            .cards
            .get_mut(&CardId(0))
            .unwrap()
            .oracle_id = FOREST.into();
        assert_eq!(
            game.mana_abilities(game.object_id(CardId(0)).unwrap())
                .unwrap(),
            [Color::G]
        );
        let mut p = prepared();
        let mut c = (*p.catalog).clone();
        let program = c
            .definitions
            .iter_mut()
            .find(|d| d.oracle_id == SPELL)
            .unwrap()
            .spell
            .as_mut()
            .unwrap();
        let mut second = program.targets[0].clone();
        second.slot = 1;
        second.selection_id = "synthetic:second".into();
        program.targets.push(second);
        program.instructions.push(Instruction::ReturnToOwnerHand {
            target_slot: 1,
            source_block: 1,
        });
        p.catalog = Arc::new(c);
        let mut game = Game::new(p);
        cast(&mut game);
        respond(
            &mut game,
            Response::Targets {
                objects: vec![
                    ObjectId {
                        card: CardId(2),
                        incarnation: 0
                    };
                    2
                ],
            },
        );
        respond(&mut game, Response::FinishMana { undo_mana: false });
        let _ = game.advance(Budget::unlimited());
        // Test-only inaccessible mutation makes one occurrence stale. Both
        // blocks still share one resolution legality check and stack object.
        game.state.rules.stack.last_mut().unwrap().targets[0].incarnation = 99;
        for _ in 0..4 {
            respond(&mut game, Response::Pass);
        }
        let _ = game.advance(Budget::unlimited());
        assert_eq!(
            game.state.rules.objects[&CardId(2)].zone,
            Zone::Hand { owner: PlayerId(1) }
        );
        assert!(matches!(
            game.events().last(),
            Some(Event::SpellCompleted { resolved: true, .. })
        ));
    }
    #[test]
    fn unsupported_configuration_and_modified_ir_reject_admission() {
        let p = prepared();
        let mut config = ScenarioConfig::land_return();
        config.mode = Mode::CommanderMatch;
        assert!(Prepared::admit((*p.catalog).clone(), config).is_err());
        let mut c = (*p.catalog).clone();
        c.definitions[0].name = "modified definition".into();
        assert!(Prepared::admit(c, ScenarioConfig::land_return()).is_err());
    }
    #[test]
    fn stale_resolution_target_does_not_follow_physical_card() {
        let mut game = Game::new(prepared());
        cast(&mut game);
        respond(
            &mut game,
            Response::Targets {
                objects: vec![ObjectId {
                    card: CardId(2),
                    incarnation: 0,
                }],
            },
        );
        respond(&mut game, Response::FinishMana { undo_mana: false });
        let _ = game.advance(Budget::unlimited());
        let target = game.object_id(CardId(2)).unwrap();
        let moved = game
            .move_object(target, Zone::Hand { owner: PlayerId(1) })
            .unwrap();
        game.move_object(moved.successor, Zone::Battlefield)
            .unwrap();
        for _ in 0..4 {
            respond(&mut game, Response::Pass);
        }
        let _ = game.advance(Budget::unlimited());
        assert_eq!(game.state.rules.objects[&CardId(2)].zone, Zone::Battlefield);
        assert!(matches!(
            game.events().last(),
            Some(Event::SpellCompleted {
                resolved: false,
                ..
            })
        ));
    }
    #[test]
    fn synthetic_multiple_blocks_have_one_target_check_and_stack_frame() {
        let mut p = prepared();
        let mut c = (*p.catalog).clone();
        let d = c
            .definitions
            .iter_mut()
            .find(|d| d.oracle_id == SPELL)
            .unwrap();
        let program = d.spell.as_mut().unwrap();
        program.instructions.push(Instruction::ReturnToOwnerHand {
            target_slot: 0,
            source_block: 1,
        });
        // Test-only interpreter fixture. No public admission bypass is provided.
        p.catalog = Arc::new(c);
        let mut game = Game::new(p);
        cast(&mut game);
        complete(&mut game);
        let target_moves=game.events().iter().filter(|e|matches!(e,Event::Committed{proposed:ProposedEvent::Move{object,..},..} if object.card==CardId(2))).count();
        assert_eq!(target_moves, 1);
        assert_eq!(
            game.state.rules.objects[&CardId(2)].zone,
            Zone::Hand { owner: PlayerId(1) }
        );
    }
    #[test]
    fn zone_intrinsics_and_corrupt_snapshot_fail_closed() {
        let p = prepared();
        let mut game = Game::new(p.clone());
        let id = game.object_id(CardId(0)).unwrap();
        assert_eq!(game.mana_abilities(id).unwrap(), vec![Color::U]);
        let movement = game
            .move_object(id, Zone::Hand { owner: PlayerId(0) })
            .unwrap();
        assert!(game.mana_abilities(movement.successor).unwrap().is_empty());
        assert!(
            Game::restore(
                p.clone(),
                &game.snapshot().unwrap().replace(BUILD_ID, "wrong-build")
            )
            .is_err()
        );
        let mut snapshot: Snapshot =
            strict_read(&Game::new(p.clone()).snapshot().unwrap()).unwrap();
        snapshot
            .state
            .rules
            .objects
            .get_mut(&CardId(2))
            .unwrap()
            .id
            .card = CardId(99);
        snapshot.state_sha256 = digest(&snapshot.state).unwrap();
        assert!(Game::restore(p, &canonical(&snapshot).unwrap()).is_err());
    }
}
