use mtg_core::testkit::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
mod architecture_transaction_tests;
mod budget_query_tests;
mod data_rules_tests;
mod formats_tests;
mod kernel_control_tests;
mod prepared_sharing_tests;
mod state_ownership_tests;
mod action_query {
    use super::*;
    mod tests;
}
mod advance_report {
    use super::*;
    mod tests;
}
mod history {
    use super::*;
    pub use mtg_core::testkit::history::*;
    mod tests;
}
mod match_admission {
    use super::*;
    mod match_profile_validation_tests;
}
mod permitted_history {
    use super::*;
    mod tests;
}
mod snapshot_codec {
    use super::*;
    pub use mtg_core::testkit::snapshot_codec::*;
    mod streaming_tests;
    mod tests;
}
mod state_digest {
    use super::*;
    pub use mtg_core::testkit::state_digest::*;
    mod cache_tests;
}
mod stored_history {
    use super::*;
    pub use mtg_core::testkit::stored_history::*;
    mod tests;
}
mod visibility {
    use super::*;
    mod index_tests;
}

pub(super) use kernel_control_tests::suspended_control;
