pub mod advance_report;
pub mod capacities;
pub mod costs;
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
pub use graveyard_replacement::GraveyardReplacement;
pub mod hideaway;
pub use hideaway::HideawayLink;
pub mod impulse_play;
pub mod mana_tap_observer;
pub mod optional_block;
pub mod random_discard;
pub mod removal;
pub mod resolution_mana;
pub mod sacrifice;
pub mod wipes;
pub use each_player_draw::{EachPlayerDrawProgress, PlayerDrawAttempts};
pub use removal::TargetFacts;
pub mod attachments;
pub mod counter_placement;
pub mod equipped_events;
pub mod predefined_token_templates;
pub mod proliferate;
pub mod resolution_payment;
pub mod target_color;
pub use resolution_payment::{ResolutionPayment, ResolutionPaymentStage};
pub mod colored_search;
pub mod entry_copy;
pub mod land_entry;
pub mod opponent_count_entry;
pub mod reveal_land_entry;
pub use colored_search::{ColoredCardAssignment, ColoredCardCandidate};
pub mod staples;
pub use counter_placement::*;
pub use land_entry::{EntryOrigin, PreparedEntryAuthority};
pub mod copiable_identity;
pub mod spell_copy;
pub mod tokens;
pub use staples::VisibleSearchLand;
pub use tokens::{RuleFault, TOKEN_CAPACITY, VisibleToken};
pub mod keywords;
pub use keywords::LifelinkGain;
pub mod setup;
pub use setup::*;
// Oracle identities and definition digests of the frozen scenario catalogs:
// admission data for `Prepared::admit` and for tools that read the exact
// constructors. No rule reads them.
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
pub mod allocation;
pub use allocation::*;
pub mod choices;
pub mod graveyard_targets;
pub use choices::*;
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
pub use damage_life::{NoncombatDamageOccurrence, TargetEntity};
pub mod spell_cascade_grants;
pub use spell_cascade_grants::{CascadeGrantProvenance, EffectiveTriggerAbility};
pub mod triggers;
pub use triggers::{
    CastOccurrence, CastOccurrenceId, GroupedDamageMember, TriggerEventWitness, TriggerId,
    TriggerOccurrenceId,
};
pub use triggers::{PendingTrigger, TriggerPlacement};
pub mod history;
pub use history::History;
pub mod recovery;
pub use recovery::SnapshotVerifier;
pub mod actions;
pub mod divided_counters;
pub use divided_counters::CounterDivision;
pub mod cascade;
pub mod linked_play;
pub use linked_play::LinkedPlayProgress;
pub mod reveal_and_cast;
pub use reveal_and_cast::RevealAndCastProgress;
pub mod frame;
pub use frame::*;
pub mod hand_cast;
pub mod object_facts;
pub use hand_cast::HandCastPermission;
pub mod casting;
pub mod checkland_entry;
pub use cascade::{CascadeProgress, CastCompletion, CastOwner, CastPermission};
pub mod granted_land_mana;
pub mod hand_return;
pub mod intrinsic_mana;
pub mod library_order;
pub mod library_position_memory;
pub mod library_selection;
pub mod look_top_mana;
pub mod mana_permanents;
pub mod mill;
pub mod opponent_mana;
pub mod same_color_mana;
pub mod scry;
pub mod successor_tuck;
pub mod surveil;
pub mod unspent_mana;
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
pub mod targeted_loss;
pub use combat::*;
pub mod saga;
pub mod turns;
pub use saga::{DelayedChapterRegistration, LoreChange};
pub use turns::*;
pub mod events;
pub mod fetched_land_untap;
pub mod trace;
pub use events::*;
pub use trace::*;
pub mod control;
pub mod controlled_land_entry;
pub mod land_reclamation;
pub mod multiplayer;
pub mod restricted_mana;
pub use multiplayer::*;
#[cfg(feature = "testkit")]
pub use publish::permitted_history;
pub use publish::permitted_history::PermittedHistory;
pub use publish::{stored_history, visibility};
pub mod decision_context;
pub use decision_context::*;
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
id!(PlayerId, u8);
id!(CardId, u32);
id!(StackId, u32);
pub mod action_query;
pub use action_query::*;
pub mod player_session;
pub use player_session::PlayerSession;
pub mod api;
pub mod state;
pub mod step;
pub use api::*;
pub use state::*;
