//! Moved from mtg-core in-source tests; see docs/game-engine-development.md.
use mtg_core::testkit::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
mod activated_permanent_return_tests;
mod activated_permanent_tap_tests;
mod activated_spell_counter_tests;
mod attachment_foundation_tests;
mod mana_tests;
mod stack_costs_tests;
mod tap_source_untap_tests;
mod tap_tests;
mod tapped_target_tests;
mod untap_tests;
mod control {
    use super::*;
    pub use mtg_core::testkit::control::*;
    mod tests;
}
mod phasing {
    use super::*;
    pub use mtg_core::testkit::phasing::*;
    mod tests;
}
mod targeted_loss {
    use super::*;
    mod tests;
}
mod unspent_mana {
    use super::*;
    mod tests;
}
mod graveyard_targets {
    use super::*;
    mod tests;
}
