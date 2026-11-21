//! Moved from mtg-core in-source tests; see docs/game-engine-development.md.
use mtg_core::testkit::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
mod attacking_creature_keyword_tests;
mod compound_indestructible_family_tests;
mod compound_indestructible_tests;
mod controlled_creature_protection_tests;
mod creature_set_keyword_tests;
mod deathtouch_window_tests;
mod equipped_indestructible_tests;
mod indestructible_tests;
mod keywords_tests;
mod permanent_set_keyword_kernel_tests;
mod permanent_target_keyword_kernel_tests;
mod scalar_keyword_effect_kernel_tests;
mod set_keyword_effect_tests;
mod static_permanent_indestructible_tests;
mod temporary_combat_keyword_tests;
mod temporary_creature_keyword_family_tests;
mod temporary_deathtouch_tests;
mod temporary_flying_tests;
mod temporary_indestructible_tests;
