//! Moved from mtg-core in-source tests; see docs/game-engine-development.md.
use mtg_core::testkit::*;
use std::{collections::BTreeMap, sync::Arc};
mod actions_tests;
mod casting_foundation_tests;

mod combat_tests;
mod commander_damage_library_tests;
mod commander_tests;
mod damage_life_tests;

mod departure_tests;

mod event_tests;
mod p2_tests;
mod payment_event_tests;
mod payment_query_tests;
mod payment_tests;
mod setup_tests;
mod spending_tests;
mod stack_tests;
mod turns_tests;
