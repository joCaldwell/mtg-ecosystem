//! Moved from mtg-core in-source tests; see docs/game-engine-development.md.
use mtg_core::testkit::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
mod attachments_tests;
mod attack_trigger_tests;
mod cast_trigger_tests;
mod four_colors_tests;
mod removal_information_tests;
mod step_trigger_tests;
mod tokens_tests;
mod trigger_tests;
