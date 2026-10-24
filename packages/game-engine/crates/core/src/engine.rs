mod advance_report;
mod search;
pub use search::*;
mod match_admission;
mod snapshot_codec;
mod state_digest;
pub use advance_report::*;
pub use match_admission::*;
mod commander_policy;
pub use commander_policy::{
    COMMANDER_POLICY_DATE, COMMANDER_POLICY_HTML_SHA256, COMMANDER_POLICY_URL, policy_digest,
};
mod four_colors;
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
        "6bbea845e4e380c0703085223bdb5fb9eea19ea4649dc6d07f3fd6a414219c3e",
    ),
    (
        DISFIGURE,
        "516f503391926f85c1a7a732e582fa49ed29f9a3f2c167497e030daff7bdb030",
    ),
    (
        FOG,
        "59c499cf85a89ef7db83c176584ea0b9fa579f035a725c2e4ebd60b80027def5",
    ),
    (
        HEALER,
        "bd103f545e9c1ffdbf5e2334b01b755aab3861a1b638d98ba1f8f702624ffc71",
    ),
    (
        FURNACE,
        "cbd6c7a82dc08a7555163343133d72d61e4b03b4ba6fc72b8436037ffbc72d09",
    ),
    (
        OX,
        "162847c12e95065221e53c1fe1f4352276d06ae8dbf0d89ded65dfaee0ea1da6",
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
const PLAINS_DIGEST: &str = "a72923bdb28920f11bacc3fb63f30f2eef3b26dbd08b61946c4f781edc0236e6";
const TIME_EBB: &str = "30cc8f7b-3c28-40f5-8f8f-157e8212280b";
const TIME_EBB_DIGEST: &str = "598891468a018c726036a5c65d64c81df04f18c0f087fa5774e40d38b2372d00";
const YARGLE: &str = "980e1721-3717-4425-95a4-938dbf2ac661";
const YARGLE_DIGEST: &str = "ab78799601f1a0f903ebaa63b1a4ce9681fe85b22bb1df3bb73b91a28df6f41e";
const EDICT: &str = "10c585c4-bf5b-4d8f-94a9-e9a5036a688f";
const HAPLESS: &str = "240a349a-79f8-4e11-9a05-a78148e70a62";
const SWAMP: &str = "56719f6a-1a6c-4c0a-8d21-18f7d7350b68";
const ACTION_EXTRA: [(&str, &str); 3] = [
    (
        EDICT,
        "7f66bd84aeeee6a89aa72b761a093d3172f78005c9e6de9f382cf63601e96232",
    ),
    (
        HAPLESS,
        "b1e84823c79eb501fb3db927369b5331450da5758b4edeb147db38fa6065bbb6",
    ),
    (
        SWAMP,
        "083e4e186b3c2885ba9617f83afd3c1cda3f380d076512764c2855b0d1694942",
    ),
];
const LOOTER: &str = "67362406-b1ca-49e2-800d-9050bfe8742a";
const STIFLE: &str = "b3b00911-ece7-4484-bc36-f211ce72b6cc";
const AZURE: &str = "bbf9fd47-10e1-4749-b06f-4fe72200afc7";
const COUNTERSPELL: &str = "cc187110-1148-4090-bbb8-e205694a39f5";
const STACK_EXTRA: [(&str, &str); 4] = [
    (
        LOOTER,
        "ff9886c1d4cdeb3c8a229c4212a6fa4194f96a36c76de16fd0caa9eccb792733",
    ),
    (
        STIFLE,
        "a50b3d38b292e933dd53a2a4480387d81ff55371ae1bf922edcf530b7d3d2062",
    ),
    (
        AZURE,
        "aa6c9379bdfc6611cd0f59d895fde22611b834dcc2d00f699c3b47b15bd5d370",
    ),
    (
        COUNTERSPELL,
        "6f411a166df92427d9c9f48bc4d76c08b311ce28cdab1171a62d12980e39fde7",
    ),
];
const ISAMARU: &str = "cfb756dd-6fe1-430a-8f04-8b10cac62a86";
// Reviewed canonical definition digests, independent of compiler success.
const ALLOWED: [(&str, &str); 5] = [
    (
        ISAMARU,
        "af28942c4c8ef05823184ba4c7d8c5a992194756ce103960e3d1dab3a5408af1",
    ),
    (
        ISLAND,
        "ddfebd6862a2efa3dac3801845ff4c8ddde157d4497b8ac7475e35a8e12f9e15",
    ),
    (
        FOREST,
        "13cf7a38a355b770de69d6d9a248eeadd5064e752bc6d02a818a62ae0b29b608",
    ),
    (
        SPELL,
        "88c77ff446ee901d24b6e3582645cef8745f993a8cf203db00f4f13ae4960f74",
    ),
    (
        BEARS,
        "572ff55cf5fedc97884e3a801b7e2c8f6de5c13e93c24bebce95f4d16432b4c8",
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
        "a72bfc445d20cebb02e6022c944e73912477ee06d654660257fc95a6c1feee86",
    ),
    (
        PRODIGAL,
        "f8f6c6248f5f2d4075a6ec5352f4004c241f551391ffea9db2b1701d6eb3f838",
    ),
    (
        SOUL_FEAST,
        "ed3a6f08c3c12042b36bf7d87cfcc8718e52f9a84e7b257bc4b3ae6727aabb08",
    ),
    (
        HEALING_HANDS,
        "3b25896a67ab51b7ff118bf6c35155229934fb276d6a10939df942f90f42ed27",
    ),
    (
        NECTAR,
        "9a4e61aaecdb646f4d570b6c6108d2819d5b5fa9b5745c03744424bdbcf92d2f",
    ),
    (
        MOUNTAIN,
        "bcbf6d64c8467946e262fd55cbdc47a2cfbdaf1084a3a8dab833b591aef05a77",
    ),
];
const BLOOD_ARTIST: &str = "310f141c-7f37-4729-aed6-dd9c09db448d";
const VISIONARY: &str = "c6a3a882-a127-4590-93d7-679ef4313efe";
const WARDEN: &str = "6ca2a89e-7032-4864-b4e9-66f3178f90ab";
const TRIGGER_EXTRA: [(&str, &str); 3] = [
    (
        BLOOD_ARTIST,
        "e1f020572802bac63e03b592c6f3aaa019786980e698613e98b86524b79afdfb",
    ),
    (
        VISIONARY,
        "6870ff3e6406a42c3896da21c56d121bf5abd8c37bb09df462fbff2bf7162333",
    ),
    (
        WARDEN,
        "3b1c640edf5130e97a5d8784c0e201d7bb5172d7d254295bbe82744b6c89fcb2",
    ),
];
mod damage_life;
#[cfg(test)]
mod damage_life_tests;
pub use damage_life::{NoncombatDamageOccurrence, TargetEntity};
#[cfg(test)]
mod trigger_tests;
mod triggers;
pub use triggers::TriggerId;
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
}
impl ScenarioConfig {
    pub fn turns_permanents() -> Self {
        Self {
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
        if config != ScenarioConfig::land_return()
            && config != ScenarioConfig::commander_foundation()
            && config != ScenarioConfig::stack_response()
            && config != ScenarioConfig::casting_reversal()
            && config != ScenarioConfig::turns_permanents()
            && config != ScenarioConfig::combat_game()
            && config != ScenarioConfig::combat_interactions()
            && config != ScenarioConfig::command_casting()
            && config != ScenarioConfig::commander_lifecycle()
            && config != ScenarioConfig::commander_exile()
            && config != ScenarioConfig::commander_damage()
            && config != ScenarioConfig::commander_damage_borrowed()
            && config != ScenarioConfig::commander_library()
            && config != ScenarioConfig::actions()
            && config != ScenarioConfig::actions_borrowed()
            && config != ScenarioConfig::actions_empty_library()
            && config != ScenarioConfig::mixed_mana()
            && config != ScenarioConfig::choices()
            && config != ScenarioConfig::choices_furnace()
            && config != ScenarioConfig::conditions()
            && config != ScenarioConfig::conditions_nonempty()
            && config != ScenarioConfig::conditions_borrowed()
            && config != ScenarioConfig::conditions_no_targets()
            && config != ScenarioConfig::effects()
            && config != ScenarioConfig::effects_furnace()
            && config != ScenarioConfig::effects_two_furnaces()
            && config != ScenarioConfig::effects_borrowed()
            && config != ScenarioConfig::effects_low()
            && config != ScenarioConfig::triggers()
            && config != ScenarioConfig::triggers_low()
            && config != ScenarioConfig::triggers_borrowed()
            && config != ScenarioConfig::triggers_empty_library()
            && config != ScenarioConfig::triggers_nonactive()
            && config != ScenarioConfig::damage_life()
            && config != ScenarioConfig::damage_life_low()
            && config != ScenarioConfig::damage_life_borrowed()
            && config != ScenarioConfig::damage_life_empty_library()
            && config != ScenarioConfig::damage_life_library()
            && config != ScenarioConfig::stack_costs()
            && config != ScenarioConfig::stack_costs_borrowed()
            && config != ScenarioConfig::stack_costs_empty_library()
            && config != ScenarioConfig::stack_costs_library()
        {
            return Err(
                "configuration is outside the reviewed four-seat scenario; no match admission"
                    .into(),
            );
        }
        if catalog.schema_version
            != if config.choices_profile() {
                10
            } else if config.effects_profile() {
                9
            } else if config.triggers_profile() {
                8
            } else if config.damage_life_profile() {
                7
            } else if config.stack_costs_profile() {
                6
            } else if config.actions_profile() {
                5
            } else if config.library_profile() {
                4
            } else {
                3
            }
        {
            return Err("catalog version is not admitted by this exact profile".into());
        }
        let mut allowed = ALLOWED.to_vec();
        if config.commander_profile() {
            allowed.push((PLAINS, PLAINS_DIGEST));
        }
        if config.damage_profile() || config.actions_profile() {
            allowed.push((YARGLE, YARGLE_DIGEST));
        }
        if config.library_profile() || config.actions_profile() {
            allowed.push((TIME_EBB, TIME_EBB_DIGEST));
        }
        if config.actions_profile() {
            allowed.extend(ACTION_EXTRA);
        }
        if config.stack_costs_profile() {
            allowed.extend(STACK_EXTRA);
        }
        if config.damage_life_profile() {
            allowed.extend(DAMAGE_LIFE_EXTRA);
        }
        if config.triggers_profile() {
            allowed.extend(TRIGGER_EXTRA);
        }
        if config.effects_profile() {
            allowed.extend(EFFECT_EXTRA);
        }
        if config.choices_profile() {
            allowed.extend(CHOICE_EXTRA);
        }
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
struct Resolving {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    condition_check: Option<InterveningCheck>,
    stack_id: StackId,
    spell: StackInstance,
    cursor: u32,
    legal_targets: Vec<bool>,
    outcomes: Vec<InstructionOutcome>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "stage", rename_all = "snake_case", deny_unknown_fields)]
enum Frame {
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
    ManaColor {
        color: Color,
    },
    SearchLands {
        objects: Vec<ObjectId>,
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
            Self::ManaColor { decision_id, .. }
            | Self::SearchLands { decision_id, .. }
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
            Self::ManaColor { player, .. }
            | Self::SearchLands { player, .. }
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
    ReversePermanentMana {
        receipt: Box<PermanentManaUndo>,
    },
    FinishLibraryShuffle {
        player: PlayerId,
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
        occurrence: OccurrenceId,
        subject: ObjectId,
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    tokens: BTreeMap<CardId, tokens::TokenIdentity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    typed_fault: Option<RuleFault>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    match_state: Option<MatchState>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    effects: Vec<ResolvedEffect>,
    #[serde(default, skip_serializing_if = "triggers::is_zero")]
    next_effect_id: u64,
    #[serde(default, skip_serializing_if = "triggers::is_zero")]
    next_damage_transaction_id: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pending_triggers: Vec<PendingTrigger>,
    #[serde(default, skip_serializing_if = "triggers::is_zero")]
    next_trigger_id: u64,
    libraries: BTreeMap<PlayerId, Vec<CardId>>,
    combat: Vec<Attacker>,
    combat_declared: bool,
    damage_choices: BTreeMap<ObjectId, Vec<(ObjectId, Quantity)>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    player_damage_choices: BTreeMap<ObjectId, (PlayerId, Quantity)>,
    /// CR510.4/702.7b: incarnation-bound first-step eligibility, retained even
    /// after combat departure. Empty in every historical profile.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    first_strike_participants: BTreeSet<ObjectId>,
    /// CR506.4/509.1h: first/double strikers still blocking after their
    /// attacker left combat. They assign no damage (510.1d) but still start a
    /// first-strike damage step (510.4). Empty in every historical profile.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    orphan_first_strike_blockers: BTreeSet<ObjectId>,
    cleanup_repeat: bool,
    simultaneous_losses: bool,
    losing_players: Vec<PlayerId>,
    revision: u64,
    rule_steps: u64,
    visibility: Vec<visibility::Knowledge>,
    players: Vec<Player>,
    cards: BTreeMap<CardId, Card>,
    objects: BTreeMap<CardId, Object>,
    active: Option<PlayerId>,
    turn_owner: PlayerId,
    turn: TurnState,
    original_seating: Vec<PlayerId>,
    commanders: CommanderLedger,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    commander_returns: BTreeSet<ObjectId>,
    control_effects: Vec<ControlEffect>,
    external_inputs: Vec<ExternalInput>,
    pending_concessions: Vec<PlayerId>,
    priority: PlayerId,
    passes: Vec<PlayerId>,
    next_stack_id: u32,
    stack: Vec<StackInstance>,
    frame: Frame,
    pending: Option<AcceptedInput>,
    journal: Vec<AcceptedInput>,
    next_batch_id: u64,
    next_event_sequence: u64,
}
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
            tokens: BTreeMap::new(),
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
            visibility: (0..4).map(|_| visibility::Knowledge::default()).collect(),
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
            external_inputs: vec![],
            pending_concessions: vec![],
            priority: PlayerId(0),
            passes: vec![],
            next_stack_id: 0,
            stack: vec![],
            frame: Frame::Priority,
            pending: None,
            journal: vec![],
            next_batch_id: 0,
            next_event_sequence: 0,
        };
        let mut game = Self {
            prepared: Arc::new(prepared),
            state,
            trace: Self::initial_trace(),
            executor: Arc::new(()),
            state_token: Arc::new(()),
        };
        if let Some(config) = game.prepared.match_config.as_ref() {
            game.state.active = Some(config.starting_player);
            game.state.turn_owner = config.starting_player;
            game.state.priority = config.starting_player;
            game.state.frame = Frame::Setup;
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
        }
    }
    /// Trusted host execution boundary; never a player-facing identifier.
    pub fn rule_boundary(&self) -> u64 {
        self.state.rule_steps
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
    pub fn journal(&self) -> &[AcceptedInput] {
        &self.state.journal
    }
    pub fn object_id(&self, card: CardId) -> Option<ObjectId> {
        self.state.objects.get(&card).map(|o| o.id)
    }
    pub fn zone(&self, object: ObjectId) -> Option<Zone> {
        self.current(object).ok().map(|o| o.zone)
    }
    pub fn mana(&self, player: PlayerId, color: Color) -> Option<Quantity> {
        self.state
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
    pub fn mana_abilities(&self, object: ObjectId) -> Result<Vec<Color>, String> {
        let o = self.current(object)?;
        let d = self.definition(object.card)?;
        if self.mana_enabled() && o.zone == Zone::Battlefield && !d.mana_abilities.is_empty() {
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
        if let Some(token) = self.state.tokens.get(&card) {
            return Ok(&token.definition);
        }
        let physical = self.state.cards.get(&card).ok_or("unknown physical card")?;
        self.prepared
            .catalog
            .definitions
            .iter()
            .find(|d| d.oracle_id == physical.oracle_id)
            .ok_or_else(|| "missing admitted definition".into())
    }
    fn current(&self, id: ObjectId) -> Result<&Object, String> {
        self.state
            .objects
            .get(&id.card)
            .filter(|o| o.id == id)
            .ok_or_else(|| "stale or absent object incarnation".into())
    }
    fn target_legal(&self, id: ObjectId) -> bool {
        self.current(id).is_ok_and(|o| o.zone == Zone::Battlefield)
            && self
                .characteristics(id)
                .is_ok_and(|c| c.kind == CardKind::Creature)
    }
    fn top(&self) -> Result<&StackInstance, String> {
        self.state
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
        if self.state.pending.is_some() || !self.state.pending_concessions.is_empty() {
            return Ok(None);
        }
        if matches!(self.state.frame, Frame::Setup) {
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
        let req = match &self.state.frame {
            Frame::EntityTargets { .. } => Request::EntityTargets {
                decision_id: self.state.revision,
                player: self.top()?.controller,
                occurrences: self.program()?.targets.clone(),
                candidates: self.eligible_entities(),
            },
            Frame::StackTargets { .. } => Request::StackTargets {
                decision_id: self.state.revision,
                player: self.top()?.controller,
                occurrences: self.program()?.targets.clone(),
                candidates: self.eligible_stack_targets(
                    self.program()?.targets[0].domain,
                    Some(self.top()?.id),
                ),
            },
            Frame::TriggerTargets { .. }
                if self.program()?.targets.first().is_some_and(|t| {
                    matches!(
                        t.domain,
                        TargetDomain::BattlefieldCreature | TargetDomain::OwnGraveyardCreatureCard
                    )
                }) =>
            {
                if !self.has_target_domain(self.program()?, self.top()?.controller) {
                    return Ok(None);
                }
                Request::Targets {
                    decision_id: self.state.revision,
                    player: self.top()?.controller,
                    occurrences: self.program()?.targets.clone(),
                    candidates: self
                        .state
                        .objects
                        .values()
                        .filter(|o| self.object_target_legal(self.top().unwrap(), o.id))
                        .map(|o| o.id)
                        .collect(),
                }
            }
            Frame::PlayerTargets { .. } | Frame::TriggerTargets { .. } => Request::PlayerTargets {
                decision_id: self.state.revision,
                player: self.top()?.controller,
                occurrences: self.program()?.targets.clone(),
                candidates: self
                    .eligible_players(self.program()?.targets[0].domain, self.top()?.controller),
            },
            Frame::ManaAbility { activation } => self.permanent_mana_request(activation)?,
            Frame::SpendMana { .. } => self.spend_mana_request()?,
            Frame::CommanderReturn {
                object,
                destination,
                ..
            } => Request::CommanderReturn {
                previous_choices: self.previous_commander_choices(),
                decision_id: self.state.revision,
                player: self.state.cards[&object.card].owner,
                object: *object,
                destination: *destination,
            },
            Frame::Priority => Request::Priority {
                decision_id: self.state.revision,
                player: self.state.priority,
            },
            Frame::Mana { .. } => Request::Mana {
                decision_id: self.state.revision,
                player: self.top()?.controller,
                cost: self.top()?.total_cost.clone(),
            },
            Frame::Targets { .. } => Request::Targets {
                decision_id: self.state.revision,
                player: self.top()?.controller,
                occurrences: self.program()?.targets.clone(),
                candidates: self
                    .state
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
        if matches!(self.state.frame, Frame::Setup) {
            return self.validate_setup_response(player, response);
        }
        if let Frame::Stabilize { resume, .. } = &self.state.frame {
            let mut validation = self.fork();
            validation.state.frame = *resume.clone();
            return validation.validate_response(player, response);
        }
        if let (Frame::DamageEffects { .. }, Response::DamageEffect { effect, damage }) =
            (&self.state.frame, response)
        {
            return self.validate_damage_effect(player, effect, *damage);
        }
        if let (Frame::DamageAllocation { .. }, Response::DamageAllocation { allocations }) =
            (&self.state.frame, response)
        {
            return self.validate_damage_allocation(player, allocations);
        }
        if let (Frame::Modes { .. }, Response::Modes { mode }) = (&self.state.frame, response) {
            return self.validate_mode(player, *mode);
        }
        if let Some(result) = self.validate_actions_response(player, response) {
            return result;
        }
        if let Frame::ManaAbility { activation } = &self.state.frame {
            return self.validate_permanent_mana_response(activation, player, response);
        }
        if let (Frame::SpendMana { .. }, Response::SpendMana { spending }) =
            (&self.state.frame, response)
        {
            return self.validate_spending(player, spending);
        }
        if self.combat_decision_player().is_some() {
            return self.validate_combat_response(player, response);
        }
        match (&self.state.frame, response) {
            (Frame::CommanderReturn { object, .. }, Response::CommanderReturn { .. })
                if self.state.cards[&object.card].owner == player && self.is_live(player) =>
            {
                Ok(())
            }
            (Frame::Priority, Response::PlayLand { card }) if self.state.priority == player => {
                self.validate_land_play(player, *card)
            }
            (Frame::Priority, Response::Pass) if self.state.priority == player => Ok(()),
            (Frame::Priority, Response::ActivateMana { object })
                if self.state.priority == player =>
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
            (Frame::Priority, Response::Cast { card }) if self.state.priority == player => {
                let o = self.state.objects.get(card).ok_or("invalid cast")?;
                if !(o.zone == (Zone::Hand { owner: player })
                    || (self.commander_enabled()
                        && o.zone == (Zone::Command { owner: player })
                        && self.state.commanders.history(*card).is_some()))
                    || self.state.cards[card].owner != player
                    || self.definition(*card)?.spell.is_none()
                    || self.state.stack.len() >= self.stack_limit()
                {
                    return Err("invalid or unsupported cast".into());
                }
                let program = self
                    .definition(*card)?
                    .spell
                    .as_ref()
                    .ok_or("missing cast program")?;
                if program.timing == SpellTiming::MainPhase
                    && (!self.turns_enabled() || !self.main_phase_action(player))
                {
                    return Err("main-phase spell casting requires active-player main-phase priority and empty stack".into());
                }
                // Restrict targeted proposals to spells with an eligible target;
                // the target-stage request never waits for an impossible choice.
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
                        TargetDomain::BattlefieldCreature | TargetDomain::OwnGraveyardCreatureCard
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
        if self.state.pending.is_some()
            || decision_id != self.state.revision
            || self.decision_for(player)?.is_none()
        {
            return Err("stale or unauthorized decision".into());
        }
        self.validate_response(player, &response)?;
        Ok(ValidatedResponse {
            executor: self.executor.clone(),
            revision: self.state.revision,
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
            || receipt.revision != self.state.revision
            || self.state.pending.is_some()
        {
            return Err("stale or foreign validation receipt".into());
        }
        let revision = self
            .state
            .revision
            .checked_add(1)
            .ok_or("structural revision resource limit")?;
        let mut next = self.fork();
        next.state.journal.push(receipt.input.clone());
        next.state.pending = Some(receipt.input);
        next.state.revision = revision;
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
        self.advance_internal::<false>(budget, &mut vec![])
    }
    fn advance_internal<const REPORT: bool>(
        &mut self,
        mut budget: Budget,
        processed: &mut Vec<InputProcessingFact>,
    ) -> Advance {
        loop {
            if let Frame::Finished { winner } = self.state.frame {
                return Advance::Finished { winner };
            }
            if let Frame::Fault { reason } = &self.state.frame {
                return Advance::Fault(reason.clone());
            }
            if matches!(self.state.frame, Frame::Draw) {
                return Advance::Draw;
            }
            if self.state.pending.is_none() && self.state.pending_concessions.is_empty() {
                for p in self.live_players() {
                    match self.decision_for(p) {
                        Ok(Some(r)) => return Advance::AwaitingDecision(r),
                        Ok(None) => {}
                        Err(e) => return Advance::Fault(e),
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
            if let Err(reason) = next.step() {
                self.state.typed_fault = next.state.typed_fault;
                self.state.frame = Frame::Fault {
                    reason: reason.clone(),
                };
                self.state_token = Arc::new(());
                return Advance::Fault(reason);
            }
            let Some(steps) = next.state.rule_steps.checked_add(1) else {
                return Advance::Fault("execution boundary resource limit".into());
            };
            next.state.rule_steps = steps;
            if let Err(reason) = next.refresh_visibility() {
                return Advance::Fault(reason);
            }
            self.state = next.state;
            self.trace = next.trace;
            self.state_token = Arc::new(());
            if let Some(report) = input_report {
                report.committed(self, processed);
            }
        }
    }
    fn step(&mut self) -> Result<(), String> {
        if let Some(player) = self.state.pending_concessions.first().copied() {
            self.state.pending_concessions.remove(0);
            return self.apply_departure(player);
        }
        match self.state.frame.clone() {
            Frame::Setup => self.setup_step()?,
            Frame::DamageAllocation { .. } => self.damage_allocation_step()?,
            Frame::Modes { cast } => self.bind_mode(cast)?,
            Frame::DamageEffects { .. } => self.damage_effect_step()?,
            Frame::Priority => {
                let input = self
                    .state
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
                            self.state.passes.clear();
                            self.stabilize()?;
                        }
                    }
                    Response::Cast { card } => {
                        let cast_from = self
                            .state
                            .objects
                            .get(&card)
                            .ok_or("missing cast source")?
                            .zone;
                        let total_cost = self.casting_cost(card, input.player, cast_from)?;
                        let original_object = self
                            .state
                            .objects
                            .get(&card)
                            .ok_or("missing cast source")?
                            .id;
                        let id = StackId(self.state.next_stack_id);
                        self.state.next_stack_id = self
                            .state
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
                        let effect_source_handles = if self.prepared.config.effects_profile() {
                            self.capture_trigger_source_handles(source)?
                        } else {
                            vec![]
                        };
                        self.state.stack.push(StackInstance {
                            mode: None,
                            effect_source_handles,
                            source_facts: if self.prepared.config.damage_life_profile() {
                                Some(self.object_before(source)?)
                            } else {
                                None
                            },
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
                            origin: AnnouncementOrigin::Spell,
                            spending: None,
                            stack_id: id,
                            original_object,
                            prior_passes: self.state.passes.clone(),
                            mana_activations: vec![],
                            undo_mana: false,
                        };
                        self.state.frame = if !self
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
                        self.state.passes.clear();
                    }
                    Response::Pass => {
                        self.record_event(Event::PriorityPassed {
                            player: input.player,
                        })?;
                        self.state.passes.push(input.player);
                        // CR 117.4 applies to the current live player set.
                        if self.all_live_passed() {
                            self.begin_resolution()?;
                        } else {
                            self.state.priority = self
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
                let top = self.state.stack.last_mut().ok_or("missing stack")?;
                top.targets = objects;
                top.announced_targets = announced;
                self.state.frame = Frame::Mana { cast };
            }
            Frame::Mana { mut cast } => {
                let input = self
                    .state
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
                            self.state.frame = Frame::Mana { cast };
                        }
                    }
                    Response::FinishMana { undo_mana } => {
                        cast.undo_mana = undo_mana;
                        self.state.frame = self.finish_mana_frame(cast)?;
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
                    .is_some_and(|check| !check.passed)
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
                                let owner = self.state.cards[&target.card].owner;
                                let destination = if matches!(
                                    instruction,
                                    Instruction::PutOnTopOwnerLibrary { .. }
                                ) {
                                    Zone::Library { owner }
                                } else {
                                    Zone::Hand { owner }
                                };
                                if self.commander_enabled()
                                    && self.state.commanders.history(target.card).is_some()
                                {
                                    self.state.frame = Frame::CommanderReturn {
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
                    self.state.frame = Frame::Resolve { frame };
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
        let owner = self.state.cards[&stack.source.card].owner;
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
            self.state.stack.pop();
        } else if self.is_live(owner) || self.state.stack.iter().any(|s| s.id == stack.id) {
            return Err("unsupported resolving-source movement".into());
        }
        self.record_event(Event::SpellCompleted {
            stack_id: stack.id,
            resolved,
        })?;
        self.state.frame = Frame::Priority;
        self.state.priority = self
            .state
            .active
            .or_else(|| self.next_live(self.state.turn_owner))
            .ok_or("no live player after resolution")?;
        self.stabilize()
    }
    fn payment_pool(
        &self,
        player: PlayerId,
        cost: &ManaCost,
    ) -> Result<BTreeMap<Color, Quantity>, String> {
        let mut pool = self
            .state
            .players
            .get(player.0 as usize)
            .ok_or("missing paying player")?
            .mana
            .clone();
        for color in &cost.colored {
            let n = pool.entry(*color).or_default();
            if *n < Quantity::from(1) {
                return Err("insufficient mana".into());
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
            return Err("insufficient mana".into());
        }
        Ok(pool)
    }
    fn stabilize(&mut self) -> Result<(), String> {
        if self.combat_enabled() {
            if matches!(self.state.frame, Frame::Priority) {
                self.schedule_stabilization(Frame::Priority, false);
            }
            return Ok(());
        }
        self.validate_legacy_stability()
    }
    fn validate_legacy_stability(&self) -> Result<(), String> {
        // CR 704.3: this entry point runs before granting priority. The closed
        // scenario has no life changes, marked damage, counters, or triggers.
        // Unsupported reachable conditions fault instead of being skipped.
        if self
            .state
            .players
            .iter()
            .any(|p| p.life != Quantity::from(40))
        {
            return Err("unsupported reachable life/SBA state".into());
        }
        for o in self
            .state
            .objects
            .values()
            .filter(|o| o.zone == Zone::Battlefield)
        {
            let c = self.characteristics(o.id)?;
            if c.kind == CardKind::Creature
                && c.toughness.as_ref().is_none_or(|n| n <= &Quantity::zero())
            {
                return Err("unsupported reachable creature SBA state".into());
            }
        }
        Ok(())
    }
    pub fn snapshot(&self) -> Result<String, String> {
        self.snapshot_with_digest().map(|(json, _)| json)
    }
    fn snapshot_ref<'a>(&'a self, state_sha256: &'a str) -> SnapshotRef<'a> {
        SnapshotRef {
            snapshot_version: 18,
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
        if s.snapshot_version != 18
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
        if s.players.len() != 4
            || s.visibility.len() != 4
            || s.turn_owner.0 >= 4
            || s.turn.number == 0
            || s.turn.lands_played > 1
            || (!self.turns_enabled()
                && (s.turn_owner != PlayerId(0)
                    || s.turn
                        != TurnState {
                            number: 1,
                            step: TurnStep::PostcombatMain,
                            lands_played: 0,
                        }))
            || s.active.is_some_and(|p| !self.is_live(p))
            || s.original_seating != (0..4).map(PlayerId).collect::<Vec<_>>()
            || s.priority.0 >= 4
            || (!self.is_live(s.priority) && !matches!(s.frame, Frame::Draw))
            || s.passes.len() > self.live_players().len()
            || s.passes.iter().any(|p| !self.is_live(*p))
            || s.passes.iter().collect::<BTreeSet<_>>().len() != s.passes.len()
            || s.cards.len() != self.prepared.config.cards.len() + s.tokens.len()
            || s.objects.len() > s.cards.len()
            || s.stack.len() > self.stack_limit()
        {
            return Err("snapshot seating/table invariant".into());
        }
        for setup in &self.prepared.config.cards {
            let card = s.cards.get(&setup.id).ok_or("snapshot missing card")?;
            if card.owner != setup.owner || card.oracle_id != setup.oracle_id {
                return Err("snapshot physical identity invariant".into());
            }
            let Some(o) = s.objects.get(&setup.id) else {
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
                || (o.deathtouch_damage
                    && (o.zone != Zone::Battlefield || self.prepared.catalog.schema_version < 12))
            {
                return Err("snapshot object invariant".into());
            }
            if let Zone::Hand { owner } | Zone::Graveyard { owner } = o.zone
                && owner != card.owner
            {
                return Err("snapshot zone-owner invariant".into());
            }
        }
        self.validate_tokens_state()?;
        self.validate_trigger_state()?;
        self.validate_effect_state()?;
        self.validate_choices_state()?;
        self.validate_commander_state()?;
        self.validate_actions_state()?;
        for p in &s.players {
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
        let ids: BTreeSet<_> = s.stack.iter().map(|entry| entry.id).collect();
        let sources: BTreeSet<_> = s
            .stack
            .iter()
            .filter(|entry| entry.kind.is_spell())
            .map(|entry| entry.source.card)
            .collect();
        if ids.len() != s.stack.len()
            || sources.len() != s.stack.iter().filter(|entry| entry.kind.is_spell()).count()
            || !s.stack.windows(2).all(|w| w[0].id < w[1].id)
        {
            return Err("snapshot duplicate or unordered stack identity".into());
        }
        for (index, stack) in s.stack.iter().enumerate() {
            if stack.id.0 >= s.next_stack_id
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
            let incomplete_top = index + 1 == s.stack.len()
                && (matches!(
                    self.underlying_mana_frame(&s.frame),
                    Frame::TriggerTargets { .. }
                        | Frame::Modes { .. }
                        | Frame::EntityTargets { .. }
                        | Frame::Targets { .. }
                        | Frame::StackTargets { .. }
                        | Frame::PlayerTargets { .. }
                        | Frame::Mana { .. }
                        | Frame::SpendMana { .. }
                        | Frame::Payment { .. }
                ) || (matches!(
                    self.underlying_mana_frame(&s.frame),
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
            .stack
            .iter()
            .filter(|entry| entry.kind.is_spell())
            .map(|entry| entry.source.card)
            .collect();
        if s.objects
            .values()
            .any(|o| (o.zone == Zone::Stack) != stack_cards.contains(&o.id.card))
        {
            return Err("snapshot stack/zone invariant".into());
        }
        self.validate_nested_mana_frame(&s.frame)?;
        match self.underlying_mana_frame(&s.frame) {
            Frame::Modes { cast }
            | Frame::EntityTargets { cast }
            | Frame::Targets { cast }
            | Frame::StackTargets { cast }
            | Frame::PlayerTargets { cast }
            | Frame::Mana { cast }
            | Frame::SpendMana { cast }
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
                        self.underlying_mana_frame(&s.frame),
                        Frame::Modes { .. }
                            | Frame::EntityTargets { .. }
                            | Frame::Targets { .. }
                            | Frame::PlayerTargets { .. }
                            | Frame::StackTargets { .. }
                    ) && (!cast.mana_activations.is_empty() || cast.undo_mana))
                    || (matches!(self.underlying_mana_frame(&s.frame), Frame::Mana { .. })
                        && cast.undo_mana)
                {
                    return Err("snapshot casting mana receipt invariant".into());
                }
                if matches!(
                    self.underlying_mana_frame(&s.frame),
                    Frame::Modes { .. }
                        | Frame::EntityTargets { .. }
                        | Frame::Targets { .. }
                        | Frame::PlayerTargets { .. }
                        | Frame::StackTargets { .. }
                ) && self.bound_target_count(top) != 0
                {
                    return Err("snapshot announcement target invariant".into());
                }
                if matches!(
                    self.underlying_mana_frame(&s.frame),
                    Frame::Mana { .. } | Frame::SpendMana { .. } | Frame::Payment { .. }
                ) && self.bound_target_count(top) != self.program()?.targets.len()
                {
                    return Err("snapshot payment target invariant".into());
                }
            }
            Frame::Resolve { frame } | Frame::ResolutionChoice { frame, .. } => {
                self.validate_resolving(frame)?
            }
            _ => {}
        }
        if let Some(pending) = &s.pending {
            if pending.decision_id >= s.revision || s.journal.last() != Some(pending) {
                return Err("snapshot pending-input invariant".into());
            }
            if !matches!(s.frame, Frame::Fault { .. }) {
                self.validate_response(pending.player, &pending.response)?;
            }
        }
        if self.combat_enabled() {
            self.validate_combat_state()
        } else {
            self.validate_legacy_stability()
        }
    }
    pub fn replay(prepared: Prepared, journal: &ReplayJournal) -> Result<Self, String> {
        Self::replay_to(prepared, journal, None, TraceMode::Detailed)
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
        let Advance::AwaitingDecision(r) = game.advance(Budget::unlimited()) else {
            panic!("expected decision")
        };
        game.respond(r.player(), r.decision_id(), response).unwrap();
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
            game.state.objects[&CardId(2)].zone,
            Zone::Hand { owner: PlayerId(1) }
        );
        assert_eq!(game.state.objects[&CardId(2)].id.incarnation, 1);
        assert_eq!(
            game.state.objects[&CardId(1)].zone,
            Zone::Graveyard { owner: PlayerId(0) }
        );
        assert_eq!(game.state.objects[&CardId(1)].id.incarnation, 2);
        assert_eq!(game.mana(PlayerId(0), Color::U), Some(Quantity::zero()));
        assert_eq!(game.state.priority, PlayerId(0));
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
                game.state.revision,
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
        assert!(game.state.stack.is_empty());
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
        snapshot.state.players[0]
            .mana
            .insert(Color::U, Quantity::from(99));
        snapshot.state_sha256 = digest(&snapshot.state).unwrap();
        assert!(Game::restore(p, &canonical(&snapshot).unwrap()).is_err());
    }
    #[test]
    fn forest_intrinsic_is_derived_and_partial_multiblock_legality_keeps_other_block() {
        let mut game = Game::new(prepared());
        game.state.cards.get_mut(&CardId(0)).unwrap().oracle_id = FOREST.into();
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
        game.state.stack.last_mut().unwrap().targets[0].incarnation = 99;
        for _ in 0..4 {
            respond(&mut game, Response::Pass);
        }
        let _ = game.advance(Budget::unlimited());
        assert_eq!(
            game.state.objects[&CardId(2)].zone,
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
        assert_eq!(game.state.objects[&CardId(2)].zone, Zone::Battlefield);
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
            game.state.objects[&CardId(2)].zone,
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
        snapshot.state.objects.get_mut(&CardId(2)).unwrap().id.card = CardId(99);
        snapshot.state_sha256 = digest(&snapshot.state).unwrap();
        assert!(Game::restore(p, &canonical(&snapshot).unwrap()).is_err());
    }
}
