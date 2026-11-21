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
pub mod permitted_history;
pub mod stored_history;
pub use permitted_history::PermittedHistory;
pub mod decision_context;
pub use decision_context::*;
pub mod publish;
pub mod visibility;
pub mod work;
pub use visibility::*;
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
