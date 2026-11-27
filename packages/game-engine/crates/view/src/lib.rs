//! Player-permitted observation and decision wire contracts.
use mtg_contracts::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
mod model;
pub use model::*;
mod shape;
pub use shape::*;
mod check;
pub use check::{
    combat_allocation_to_blockers, subtract_restricted_mana, subtract_selected_mana,
    validate_selected_mana_cost,
};
mod visible;
pub use visible::*;
mod effects;
pub use effects::*;
#[cfg(feature = "test-intents")]
mod intent;
#[cfg(feature = "test-intents")]
pub use intent::TestRulesSpace;
mod payment_choices;
pub use payment_choices::{distinct_payment_choices, payment_choice_minimum};
