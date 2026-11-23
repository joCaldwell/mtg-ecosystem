//! Moved from mtg-core in-source tests; see docs/game-engine-development.md.
use mtg_core::testkit::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
mod kernel_set_pump_tests;
mod nonland_exile_life_tests;
mod permanent_owner_hand_tests;
mod removal_tests;
mod tap_source_destroy_tests;
mod wipes_tests;
