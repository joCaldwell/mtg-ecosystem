//! Moved from mtg-core in-source tests; see docs/game-engine-development.md.
use mtg_core::testkit::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
mod cast_prohibition_tests;
mod controlled_creature_damage_replacement_tests;
mod damage_controller_discard_tests;
mod damage_match_appearance_tests;
mod damage_quantity_draw_tests;
mod damage_trigger_tests;
mod event_player_discard_tests;
mod filtered_damage_trigger_tests;
mod grouped_damage_trigger_tests;
mod hand_cast_lki_tests;
mod opponent_damage_trigger_tests;
mod optional_draw_tests;
mod query_tests;
mod source_damage_counters_tests;
