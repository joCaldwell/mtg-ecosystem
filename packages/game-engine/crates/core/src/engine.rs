pub mod advance_report;
pub mod capacities;
pub mod costs;
mod counter;
pub mod search;
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
pub mod dynamic_admission;
pub use dynamic_admission::*;
pub mod match_admission;
pub mod snapshot_codec;
pub mod state_digest;
pub use advance_report::*;
pub use match_admission::*;
pub mod commander_policy;
mod modal;
pub mod optional_costs;
pub mod phasing;
pub use commander_policy::policy_digest;
pub use phasing::{PhaseReturn, PhasingSchedule};
pub mod each_player_draw;
pub mod graveyard_group;
pub mod graveyard_replacement;
pub mod regeneration;
pub use graveyard_group::{GraveyardGroup, GroupFinish, GroupResult};
pub mod group_discard;
pub mod hideaway;
pub use hideaway::HideawayLink;
mod chosen_cost;
pub mod random_discard;
pub mod removal;
pub mod sacrifice;
pub mod wipes;
pub use each_player_draw::{EachPlayerDrawProgress, PlayerDrawAttempts};
pub use removal::TargetFacts;
pub mod attachments;
pub mod counter_placement;
pub mod predefined_token_templates;
pub mod resolution_payment;

pub use resolution_payment::{ResolutionPayment, ResolutionPaymentStage};
pub mod staples;
pub use counter_placement::*;
pub mod copiable_identity;
pub mod spell_copy;
pub mod tokens;

pub use tokens::{RuleFault, TOKEN_CAPACITY};
pub mod keywords;
pub use keywords::LifelinkGain;
pub mod setup;
pub use setup::*;
// Oracle identities and definition digests of the frozen scenario catalogs:
// admission data for `Prepared::admit` and for tools that read the exact
// constructors. No rule reads them.
#[cfg(any(test, feature = "testkit"))]
pub const OX: &str = "19b4cf0b-e1e4-4e50-80bc-673bc6ef8bb8";

pub const ISLAND: &str = "b2c6aa39-2d2a-459c-a555-fb48ba993373";
pub const FOREST: &str = "b34bb2dc-c1af-4d77-b0b3-a0fb342a5fc6";
pub const SPELL: &str = "837182db-1bf3-4a2c-bd01-1af9d9873561";
pub const BEARS: &str = "14c8f55d-d177-4c25-a931-ebeb9e6062a0";
pub const PLAINS: &str = "bc71ebf6-2056-41f7-be35-b2e5c34afa99";
pub const PLAINS_DIGEST: &str =
    definition_digest!("MTG_DEFINITION_action_bc71ebf6-2056-41f7-be35-b2e5c34afa99");
pub const TIME_EBB: &str = "30cc8f7b-3c28-40f5-8f8f-157e8212280b";
pub const TIME_EBB_DIGEST: &str =
    definition_digest!("MTG_DEFINITION_action_30cc8f7b-3c28-40f5-8f8f-157e8212280b");
pub const YARGLE: &str = "980e1721-3717-4425-95a4-938dbf2ac661";
pub const YARGLE_DIGEST: &str =
    definition_digest!("MTG_DEFINITION_action_980e1721-3717-4425-95a4-938dbf2ac661");
pub const EDICT: &str = "10c585c4-bf5b-4d8f-94a9-e9a5036a688f";
pub const SWAMP: &str = "56719f6a-1a6c-4c0a-8d21-18f7d7350b68";
pub const ACTION_EXTRA: [(&str, &str); 2] = [
    (
        EDICT,
        definition_digest!("MTG_DEFINITION_action_10c585c4-bf5b-4d8f-94a9-e9a5036a688f"),
    ),
    (
        SWAMP,
        definition_digest!("MTG_DEFINITION_action_56719f6a-1a6c-4c0a-8d21-18f7d7350b68"),
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

pub mod choices;
pub mod graveyard_targets;
pub use choices::*;
mod ability_layers;
mod damage_rules;
pub mod effects;
pub use crate::build_id;
// Cargo profile of this test build, recorded by build.rs for diagnostic receipts.
// Profile settings never depend on source, so this adds no rebuild trigger.
#[cfg(feature = "testkit")]
pub const TEST_OPT_LEVEL: &str = env!("MTG_CORE_OPT_LEVEL");
#[cfg(feature = "testkit")]
pub const TEST_PROFILE: &str = env!("MTG_CORE_PROFILE");
pub use effects::*;
use mtg_contracts::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
// Reviewed canonical definition digests, independent of compiler success.
pub mod damage_life;
mod kir;
pub use damage_life::{NoncombatDamageOccurrence, TargetEntity};

pub use triggers::EffectiveTriggerAbility;
mod resolution_triggers;
pub mod triggers;
pub use triggers::{
    CastOccurrence, CastOccurrenceId, TriggerEventWitness, TriggerId, TriggerOccurrenceId,
};
pub use triggers::{PendingTrigger, TriggerPlacement};
pub mod history;
pub use history::History;
pub mod recovery;
pub use recovery::SnapshotVerifier;
pub mod actions;
pub mod divided_counters;

pub mod frame;
pub use frame::*;
pub mod casting;
pub mod hand_return;
pub mod intrinsic_mana;
pub mod library_position_memory;
pub mod mana_permanents;
pub mod mill;
pub mod object_facts;
pub mod scry;
pub use mana_permanents::PermanentManaUndo;
pub use mana_permanents::{ManaActivation, ManaSourceScope, zero_mana_cost};
pub mod stack_costs;
pub use actions::{ResolutionChoice, StackKind};
pub use stack_costs::AnnouncementOrigin;
pub mod commander;
pub mod payments;
pub use commander::CommanderContinuation;
pub mod combat;
pub mod legend;
pub mod scenarios;
pub use combat::*;
pub mod turns;

pub mod events;
pub mod trace;
pub use events::*;
pub use trace::*;
mod battlefield_entry;
pub mod control;
pub mod multiplayer;
pub mod restricted_mana;
pub use multiplayer::*;
#[cfg(feature = "testkit")]
pub use publish::permitted_history;
pub use publish::permitted_history::PermittedHistory;
pub use publish::{stored_history, visibility};
pub mod decision_context;

pub mod publish;
pub mod work;
pub use publish::visibility::*;
pub use work::*;
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
pub use mtg_view::*;
id!(CardId, u32);
id!(StackId, u32);
pub mod player_session;
pub use player_session::PlayerSession;
pub mod api;
pub mod state;
pub mod step;
pub use api::*;
pub use state::*;
