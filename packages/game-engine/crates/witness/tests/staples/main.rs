//! Moved from mtg-core in-source tests; see docs/game-engine-development.md.
use mtg_core::testkit::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
mod staples_tests;
mod choices {
    use super::*;
    mod fixed_target_mode_tests;
}
mod copiable_identity {
    use super::*;
    mod modal_land_tests;
}
