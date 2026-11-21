//! Moved from mtg-core in-source tests; see docs/game-engine-development.md.
use mtg_core::testkit::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
mod definition_diversity_tests;
mod dense_kernel_tests;
mod legal_branch_tests;
mod search {
    use super::*;
    mod shared_history_tests;
    mod tests;
}
mod work {
    use super::*;
    mod initialization_tests;
    pub(crate) mod optimization_tests;
}
