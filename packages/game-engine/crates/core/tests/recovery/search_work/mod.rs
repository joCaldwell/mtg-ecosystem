//! Search contexts and saved query Work through the public testkit surface.
use mtg_core::testkit::*;
use serde::Serialize;
use std::{collections::BTreeMap, sync::Arc};
mod search {
    use super::*;
    mod shared_history_tests;
    mod tests;
}
mod work {
    use super::*;
    mod initialization_tests;
    mod saved_work_tests;
}
