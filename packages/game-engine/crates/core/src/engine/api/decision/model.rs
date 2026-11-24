//! The same ordered decision in rules identity space and in one seat's
//! permitted identity space. Only the latter is exported to the host schema.
use super::shape::Shape;
use crate::engine::*;
use mtg_contracts::{Color, HybridPayment, ManaCost, Quantity, TargetSpec};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::fmt::Debug;

pub trait Identity: Clone + Debug + Eq + Serialize + DeserializeOwned {}
impl<T: Clone + Debug + Eq + Serialize + DeserializeOwned> Identity for T {}

pub trait Space: Clone + Debug + Eq {
    type Id: Identity;
    type Object: Identity;
    type Trigger: Identity;
    type Damage: Identity;
    type Effect: Identity;
    type Entity: Identity;
    type Commander: Identity;
    type PreviousCommanderChoice: Identity;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rules {}
#[derive(Clone, Debug, PartialEq, Eq, JsonSchema)]
pub enum Viewer {}

impl Space for Rules {
    type Id = u64;
    type Object = ObjectId;
    type Trigger = TriggerId;
    type Damage = DamageId;
    type Effect = EffectKey;
    type Entity = TargetEntity;
    type Commander = CardId;
    type PreviousCommanderChoice = (ObjectId, bool);
}
impl Space for Viewer {
    type Id = DecisionToken;
    type Object = ObjectHandle;
    type Trigger = TriggerHandle;
    type Damage = DamageHandle;
    type Effect = EffectHandle;
    type Entity = PlayerTargetEntity;
    type Commander = String;
    type PreviousCommanderChoice = VisibleCommanderReturnChoice;
}

/// A permitted observation cannot supply a trusted rules decision.
///
/// ```compile_fail
/// use mtg_core::{Decision, Rules, Viewer};
/// fn trusted(input: Decision<Rules>) {}
/// fn submit(observed: Decision<Viewer>) { trusted(observed); }
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, bound = "")]
#[schemars(
    bound = "S: JsonSchema, S::Id: JsonSchema, Candidate<S>: JsonSchema, Reason<S>: JsonSchema"
)]
pub struct Decision<S: Space> {
    #[serde(rename = "token")]
    pub id: S::Id,
    pub player: PlayerId,
    pub reason: Reason<S>,
    pub candidates: Vec<Candidate<S>>,
    pub shape: Shape,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "candidate",
    rename_all = "snake_case",
    deny_unknown_fields,
    bound = ""
)]
#[schemars(
    bound = "S: JsonSchema, S::Object: JsonSchema, S::Effect: JsonSchema, S::Damage: JsonSchema, S::Trigger: JsonSchema"
)]
pub enum Candidate<S: Space> {
    Object {
        object: S::Object,
    },
    Player {
        player: PlayerId,
    },
    Trigger {
        trigger: S::Trigger,
    },
    DamageEffect {
        effect: S::Effect,
        damage: S::Damage,
    },
    Color {
        color: Color,
    },
    ManaAbility {
        ability: ManaAbilityIdentity,
    },
    AdditionalCost {
        id: String,
        mana: ManaCost,
    },
    HybridPayment {
        payment: HybridPayment,
    },
    Flag {
        flag: Flag,
    },
    Action {
        action: Action<S>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Flag {
    Accept,
    Decline,
    Keep,
    Mulligan,
    CommandZone,
    Destination,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "action",
    rename_all = "snake_case",
    deny_unknown_fields,
    bound = ""
)]
#[schemars(bound = "S: JsonSchema, S::Object: JsonSchema")]
pub enum Action<S: Space> {
    Pass,
    PlayLand { object: S::Object },
    Cast { card: S::Object },
    ActivateMana { object: S::Object },
    ActivateAbility { object: S::Object, ability: u32 },
}

/// Display facts. Bounds and joint constraints are carried by the shape.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "reason",
    rename_all = "snake_case",
    deny_unknown_fields,
    bound = ""
)]
#[schemars(
    bound = "S: JsonSchema, S::Object: JsonSchema, S::Entity: JsonSchema, S::Commander: JsonSchema, S::PreviousCommanderChoice: JsonSchema"
)]
pub enum Reason<S: Space> {
    Priority,
    Targets {
        specs: Vec<TargetSpec>,
    },
    Mana {
        source: Option<S::Object>,
    },
    Attackers,
    Blockers,
    CombatDamage {
        source: S::Object,
        power: Quantity,
    },
    Discard,
    CommanderReturn {
        object: Option<S::Object>,
        designation: S::Commander,
        destination: Zone,
        previous: Vec<S::PreviousCommanderChoice>,
    },
    GraveyardReplacementOrder {
        object: S::Object,
        offered: Option<GraveyardOrderOption>,
    },
    OptionalEffect,
    DamageEffect {
        recipient: S::Entity,
    },
    OrderTriggers,
    Mulligan {
        mulligans_taken: u8,
        bottom_if_mulligan: u8,
    },
    BottomOpening,
    Sacrifice,
    SacrificeCost,
    ForcedSacrifice {
        previous: Vec<(PlayerId, S::Object)>,
    },
    AnnounceX,
    LegendRule {
        previous: Vec<S::Object>,
    },
    AdditionalCosts,
    HybridCost,
    ManaAbility {
        source: S::Object,
    },
    ManaColor {
        source: Option<S::Object>,
    },
    SpendMana {
        source: Option<S::Object>,
    },
    ResolveDiscard,
}

impl<S: Space> Reason<S> {
    /// Stable discriminant for a decision in the viewer's stored knowledge.
    pub fn code(&self) -> u8 {
        match self {
            Self::Priority => 0,
            Self::Targets { .. } => 1,
            Self::Mana { .. } => 2,
            Self::Attackers => 3,
            Self::Blockers => 4,
            Self::CombatDamage { .. } => 5,
            Self::Discard => 6,
            Self::CommanderReturn { .. } => 7,
            Self::GraveyardReplacementOrder { .. } => 8,
            Self::OptionalEffect => 9,
            Self::DamageEffect { .. } => 10,
            Self::OrderTriggers => 11,
            Self::Mulligan { .. } => 12,
            Self::BottomOpening => 13,
            Self::Sacrifice => 14,
            Self::SacrificeCost => 15,
            Self::ForcedSacrifice { .. } => 16,
            Self::AnnounceX => 17,
            Self::LegendRule { .. } => 18,
            Self::AdditionalCosts => 19,
            Self::HybridCost => 20,
            Self::ManaAbility { .. } => 21,
            Self::ManaColor { .. } => 22,
            Self::SpendMana { .. } => 23,
            Self::ResolveDiscard => 24,
        }
    }
}
