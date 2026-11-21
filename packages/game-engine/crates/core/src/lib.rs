//! Deterministic scenario kernel. See frozen first-scenario brief for admission.
mod build_identity;
pub use build_identity::{BuildIdentity, build_id, build_identity, install_build_identity};
mod engine;
pub mod formats;
#[cfg(test)]
mod formats_tests;
mod profile_data;
pub use engine::*;
